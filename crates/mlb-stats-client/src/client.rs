//! HTTP client for the free MLB Stats API (statsapi.mlb.com). No API key
//! required. Used to pull historical season stats and per-game logs for
//! trend/matchup analysis.

use tracing::{debug, instrument};

use crate::error::StatsApiError;
use crate::types::*;

const BASE_URL: &str = "https://statsapi.mlb.com/api/v1";

#[derive(Debug, Clone)]
pub struct StatsApiClient {
    http: reqwest::Client,
}

impl Default for StatsApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsApiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("mlb-edge/0.1 (+statsapi client)")
            .build()
            .expect("failed to build HTTP client");
        Self { http }
    }

    /// Search for a player by name. Returns every match — callers that
    /// want a single best guess should use [`Self::find_player`].
    #[instrument(skip(self))]
    pub async fn search_player(&self, name: &str) -> Result<Vec<PlayerSearchResult>, StatsApiError> {
        let url = format!("{}/people/search", BASE_URL);
        let resp: SearchResponse = self.get(&url, &[("names", name)]).await?;
        let people = resp.people.unwrap_or_default();
        Ok(people
            .into_iter()
            .map(|p| PlayerSearchResult {
                id: p.id,
                full_name: p.full_name,
                position: p.primary_position.and_then(|pos| pos.abbreviation),
                team_id: p.current_team.as_ref().map(|t| t.id),
                team_name: p.current_team.and_then(|t| t.name),
            })
            .collect())
    }

    /// Resolve a name to a single player, erroring if there's no match or
    /// too many ambiguous matches to pick automatically.
    pub async fn find_player(&self, name: &str) -> Result<PlayerSearchResult, StatsApiError> {
        let mut matches = self.search_player(name).await?;
        if matches.is_empty() {
            return Err(StatsApiError::PlayerNotFound(name.to_string()));
        }
        if matches.len() == 1 {
            return Ok(matches.remove(0));
        }

        let q = name.to_lowercase();
        if let Some(pos) = matches.iter().position(|m| m.full_name.to_lowercase() == q) {
            return Ok(matches.remove(pos));
        }

        Err(StatsApiError::AmbiguousPlayer(name.to_string(), matches.len()))
    }

    /// Season-by-season stat lines for the last `seasons_back` seasons
    /// (one HTTP request covers every available season).
    #[instrument(skip(self))]
    pub async fn get_year_by_year(
        &self,
        player_id: i64,
        group: StatGroup,
        seasons_back: u32,
    ) -> Result<Vec<SeasonStatLine>, StatsApiError> {
        let url = format!("{}/people/{}/stats", BASE_URL, player_id);
        let resp: StatsResponse = self
            .get(&url, &[("stats", "yearByYear"), ("group", group.as_api_key())])
            .await?;

        let current_year = current_season_year();
        let min_year = current_year - seasons_back as i32 + 1;

        let mut lines: Vec<SeasonStatLine> = Vec::new();
        for stat_group in resp.stats.unwrap_or_default() {
            for split in stat_group.splits.unwrap_or_default() {
                let Some(season_str) = split.season else { continue };
                let Ok(season) = season_str.parse::<i32>() else { continue };
                if season < min_year {
                    continue;
                }
                let stat = split.stat.unwrap_or_default();
                lines.push(to_season_line(season, split.team.and_then(|t| t.name), &stat, group));
            }
        }
        lines.sort_by_key(|l| l.season);
        Ok(lines)
    }

    /// Per-game stat lines for one season, including opponent info.
    #[instrument(skip(self))]
    pub async fn get_game_log(
        &self,
        player_id: i64,
        group: StatGroup,
        season: i32,
    ) -> Result<Vec<GameLogEntry>, StatsApiError> {
        let url = format!("{}/people/{}/stats", BASE_URL, player_id);
        let season_str = season.to_string();
        let resp: StatsResponse = self
            .get(
                &url,
                &[("stats", "gameLog"), ("group", group.as_api_key()), ("season", &season_str)],
            )
            .await?;

        let mut games = Vec::new();
        for stat_group in resp.stats.unwrap_or_default() {
            for split in stat_group.splits.unwrap_or_default() {
                let stat = split.stat.unwrap_or_default();
                games.push(GameLogEntry {
                    date: split.date.unwrap_or_default(),
                    season,
                    opponent_id: split.opponent.as_ref().map(|t| t.id),
                    opponent_name: split.opponent.and_then(|t| t.name),
                    is_home: split.is_home.unwrap_or(false),
                    innings_pitched: stat.innings_pitched.as_deref().and_then(parse_innings_pitched),
                    strikeouts: if group == StatGroup::Pitching { stat.strike_outs } else { None },
                    walks: if group == StatGroup::Pitching { stat.base_on_balls } else { None },
                    earned_runs: stat.earned_runs,
                    hits_allowed: if group == StatGroup::Pitching { stat.hits } else { None },
                    at_bats: stat.at_bats,
                    hits: if group == StatGroup::Hitting { stat.hits } else { None },
                    home_runs: stat.home_runs,
                    rbi: stat.rbi,
                    batter_strikeouts: if group == StatGroup::Hitting { stat.strike_outs } else { None },
                    stolen_bases: stat.stolen_bases,
                    walks_drawn: if group == StatGroup::Hitting { stat.base_on_balls } else { None },
                });
            }
        }
        Ok(games)
    }

    /// Fetches game logs across the last `seasons_back` seasons and
    /// merges them into one list, most recent game first. Seasons with no
    /// data (e.g. a rookie's pre-debut years) are skipped silently.
    pub async fn get_recent_game_logs(
        &self,
        player_id: i64,
        group: StatGroup,
        seasons_back: u32,
    ) -> Result<Vec<GameLogEntry>, StatsApiError> {
        let current_year = current_season_year();
        let mut all = Vec::new();
        for i in 0..seasons_back {
            let season = current_year - i as i32;
            if let Ok(mut games) = self.get_game_log(player_id, group, season).await {
                all.append(&mut games);
            }
        }
        all.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(all)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str, params: &[(&str, &str)]) -> Result<T, StatsApiError> {
        debug!(url, "Making MLB Stats API request");
        let response = self.http.get(url).query(params).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(StatsApiError::Parse(format!("HTTP {}: {}", status, &text[..text.len().min(200)])));
        }
        serde_json::from_str(&text).map_err(|e| StatsApiError::Parse(format!("{}: {}", e, &text[..text.len().min(200)])))
    }
}

fn current_season_year() -> i32 {
    chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(2026)
}

fn to_season_line(season: i32, team_name: Option<String>, stat: &RawStat, group: StatGroup) -> SeasonStatLine {
    SeasonStatLine {
        season,
        team_name,
        games: stat.games_played,
        games_started: stat.games_started,
        innings_pitched: stat.innings_pitched.as_deref().and_then(parse_innings_pitched),
        strikeouts: if group == StatGroup::Pitching { stat.strike_outs } else { None },
        walks: if group == StatGroup::Pitching { stat.base_on_balls } else { None },
        earned_runs: stat.earned_runs,
        era: stat.era.as_deref().and_then(|s| s.parse().ok()),
        whip: stat.whip.as_deref().and_then(|s| s.parse().ok()),
        plate_appearances: stat.plate_appearances,
        at_bats: stat.at_bats,
        hits: stat.hits,
        home_runs: stat.home_runs,
        rbi: stat.rbi,
        avg: stat.avg.as_deref().and_then(|s| s.parse().ok()),
        obp: stat.obp.as_deref().and_then(|s| s.parse().ok()),
        slg: stat.slg.as_deref().and_then(|s| s.parse().ok()),
        ops: stat.ops.as_deref().and_then(|s| s.parse().ok()),
        batter_strikeouts: if group == StatGroup::Hitting { stat.strike_outs } else { None },
        stolen_bases: stat.stolen_bases,
    }
}
