//! Shared player-history-summary builder, consumed by both the CLI
//! export (dashboard Props tab) and the chat context builder
//! (`mlb-server`), so the two surfaces never drift out of sync on how
//! trend/matchup data is computed or formatted.

use serde::{Deserialize, Serialize};

use mlb_core::models::MarketType;
use mlb_db::Database;
use mlb_stats_client::{find_team_id_by_name, GameLogEntry, SeasonStatLine, StatGroup};

use crate::report::get_cached_report;
use crate::stats;
use crate::trend::{linear_regression_trend, SeasonPoint};
use crate::matchup::last_n_meetings;

/// Maps a player-prop market to the single rate stat most relevant to
/// it: a display label, whether a rising value is "better" for that
/// stat, and the extractor function pulling the rate out of a season
/// stat line.
pub fn rate_extractor_for_market(market: MarketType) -> Option<(&'static str, bool, fn(&SeasonStatLine) -> Option<f64>)> {
    use MarketType::*;
    match market {
        BatterHits => Some(("Hits/G", true, stats::hits_per_game)),
        BatterHomeRuns => Some(("HR/G", true, stats::hr_per_game)),
        BatterRbis => Some(("RBI/G", true, stats::rbi_per_game)),
        BatterStolenBases => Some(("SB/G", true, stats::sb_per_game)),
        BatterStrikeouts => Some(("K/G", false, stats::k_per_game_batter)),
        BatterWalks => Some(("BB/G", true, stats::walks_per_game_batter)),
        PitcherStrikeouts => Some(("K/9", true, stats::k_per_9)),
        PitcherHitsAllowed => Some(("H/9", false, stats::hits_allowed_per_9)),
        PitcherWalks => Some(("BB/9", false, stats::walks_per_9)),
        PitcherEarnedRuns => Some(("ERA", false, stats::era)),
        _ => None,
    }
}

/// Which cached stat group (pitching vs. hitting) a prop market draws
/// its history from.
pub fn stat_group_for_market(market: MarketType) -> Option<StatGroup> {
    use MarketType::*;
    match market {
        BatterHits | BatterHomeRuns | BatterRbis | BatterStolenBases | BatterStrikeouts | BatterWalks => Some(StatGroup::Hitting),
        PitcherStrikeouts | PitcherHitsAllowed | PitcherWalks | PitcherEarnedRuns => Some(StatGroup::Pitching),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendSummary {
    pub stat_label: String,
    /// "IMPROVING" / "REGRESSING" / "STABLE" / "INSUFFICIENT DATA"
    pub direction: String,
    /// (season, rate) pairs, oldest first.
    pub points: Vec<(i32, f64)>,
    pub slope_per_season: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingSummary {
    pub date: String,
    /// Pre-formatted game line, e.g. "6.0 IP, 7 K, 2 BB, 1 ER".
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHistorySummary {
    pub player_name: String,
    pub trend: Option<TrendSummary>,
    pub opponent_name: Option<String>,
    pub last_meetings: Vec<MeetingSummary>,
}

/// Builds a player's history summary for a given prop market, reading
/// only from the local cache (never calls out to statsapi.mlb.com — safe
/// to use inline during a chat request or export). Returns `None` if the
/// market has no associated stat group, or the player isn't cached yet.
pub async fn build_history_summary(db: &Database, player_name: &str, market: MarketType, home_team: &str, away_team: &str) -> Option<PlayerHistorySummary> {
    let group = stat_group_for_market(market)?;
    let (label, higher_is_better, extractor) = rate_extractor_for_market(market)?;

    let report = get_cached_report(db, player_name, group, None).await.ok().flatten()?;

    let points: Vec<SeasonPoint> = report.seasons.iter().filter_map(|s| extractor(s).map(|rate| SeasonPoint { season: s.season, rate })).collect();
    let trend = if points.is_empty() {
        None
    } else {
        let t = linear_regression_trend(&points);
        Some(TrendSummary {
            stat_label: label.to_string(),
            direction: t.direction.judge(higher_is_better).to_string(),
            points: t.points.iter().map(|p| (p.season, p.rate)).collect(),
            slope_per_season: t.slope_per_season,
        })
    };

    let mut opponent_name: Option<String> = None;
    let mut last_meetings: Vec<MeetingSummary> = Vec::new();

    // Resolve opponent: whichever of the event's two teams isn't the
    // player's own team (fuzzy-matched, since naming can differ slightly
    // between the Odds API and the MLB Stats API).
    if let Some(team_name) = &report.team_name {
        let tn = team_name.to_lowercase();
        let resolved_opponent = if home_team.to_lowercase().contains(&tn) || tn.contains(&home_team.to_lowercase()) {
            Some(away_team)
        } else if away_team.to_lowercase().contains(&tn) || tn.contains(&away_team.to_lowercase()) {
            Some(home_team)
        } else {
            None
        };

        if let Some(opp) = resolved_opponent {
            if let Some(opponent_id) = find_team_id_by_name(opp) {
                let meetings = last_n_meetings(&report.recent_games, opponent_id, 3);
                if !meetings.is_empty() {
                    opponent_name = Some(opp.to_string());
                    last_meetings = meetings.iter().map(|g| MeetingSummary { date: g.date.clone(), line: format_game_line(g, group) }).collect();
                }
            }
        }
    }

    Some(PlayerHistorySummary {
        player_name: report.player_name.clone(),
        trend,
        opponent_name,
        last_meetings,
    })
}

fn format_game_line(g: &GameLogEntry, group: StatGroup) -> String {
    match group {
        StatGroup::Pitching => format!("{:.1} IP, {} K, {} BB, {} ER", g.innings_pitched.unwrap_or(0.0), g.strikeouts.unwrap_or(0), g.walks.unwrap_or(0), g.earned_runs.unwrap_or(0)),
        StatGroup::Hitting => format!("{} AB, {} H, {} HR, {} RBI", g.at_bats.unwrap_or(0), g.hits.unwrap_or(0), g.home_runs.unwrap_or(0), g.rbi.unwrap_or(0)),
    }
}
