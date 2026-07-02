//! Fetches (with SQLite caching) a player's recent season stats + game
//! logs, so trend/matchup analysis doesn't have to hit statsapi.mlb.com
//! on every request.

use chrono::{DateTime, Utc};
use mlb_db::Database;
use mlb_stats_client::{GameLogEntry, PlayerSearchResult, SeasonStatLine, StatGroup, StatsApiClient};

use crate::error::PredictError;

/// How long a cached player's stats stay "fresh" before we bother
/// re-fetching from statsapi.mlb.com.
pub const CACHE_TTL_HOURS: i64 = 12;

#[derive(Debug, Clone)]
pub struct PlayerReport {
    pub player_id: i64,
    pub player_name: String,
    pub group: StatGroup,
    /// The player's current team, if known — used by callers to figure
    /// out who the "opponent" is for a given game on the slate.
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
    pub seasons: Vec<SeasonStatLine>,
    pub recent_games: Vec<GameLogEntry>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedPayload {
    player_name: String,
    #[serde(default)]
    team_id: Option<i64>,
    #[serde(default)]
    team_name: Option<String>,
    seasons: Vec<SeasonStatLine>,
    recent_games: Vec<GameLogEntry>,
}

/// Look up a cached report by player name (case-insensitive substring),
/// without ever calling out to statsapi.mlb.com. Returns `None` if not
/// cached, or if cached but older than `max_age_hours` (pass `None` to
/// accept any age).
pub async fn get_cached_report(
    db: &Database,
    player_name: &str,
    group: StatGroup,
    max_age_hours: Option<i64>,
) -> Result<Option<PlayerReport>, PredictError> {
    let row = match db.find_player_stats_cache_by_name(player_name, group.as_api_key()).await {
        Ok(row) => row,
        Err(_) => return Ok(None),
    };

    if let Some(max_age) = max_age_hours {
        let age = Utc::now().signed_duration_since(row.fetched_at).num_hours();
        if age > max_age {
            return Ok(None);
        }
    }

    let payload: CachedPayload = serde_json::from_str(&row.payload_json)?;
    Ok(Some(PlayerReport {
        player_id: row.player_id,
        player_name: payload.player_name,
        group,
        team_id: payload.team_id,
        team_name: payload.team_name,
        seasons: payload.seasons,
        recent_games: payload.recent_games,
        fetched_at: row.fetched_at,
    }))
}

/// Fetch fresh data from statsapi.mlb.com for a resolved player and cache
/// it in SQLite.
pub async fn fetch_and_cache_report(
    db: &Database,
    stats_client: &StatsApiClient,
    player: &PlayerSearchResult,
    group: StatGroup,
    seasons_back: u32,
) -> Result<PlayerReport, PredictError> {
    let seasons = stats_client.get_year_by_year(player.id, group, seasons_back).await?;
    let recent_games = stats_client.get_recent_game_logs(player.id, group, seasons_back).await?;

    let payload = CachedPayload {
        player_name: player.full_name.clone(),
        team_id: player.team_id,
        team_name: player.team_name.clone(),
        seasons: seasons.clone(),
        recent_games: recent_games.clone(),
    };
    let payload_json = serde_json::to_string(&payload)?;
    let fetched_at = Utc::now();

    db.save_player_stats_cache(player.id, &player.full_name, group.as_api_key(), &payload_json, fetched_at)
        .await?;

    Ok(PlayerReport {
        player_id: player.id,
        player_name: player.full_name.clone(),
        group,
        team_id: player.team_id,
        team_name: player.team_name.clone(),
        seasons,
        recent_games,
        fetched_at,
    })
}

/// Convenience: use the cache if fresh, otherwise resolve the player via
/// name search and fetch + cache fresh data.
pub async fn get_or_fetch_report(
    db: &Database,
    stats_client: &StatsApiClient,
    player_name: &str,
    group: StatGroup,
    seasons_back: u32,
) -> Result<PlayerReport, PredictError> {
    if let Some(report) = get_cached_report(db, player_name, group, Some(CACHE_TTL_HOURS)).await? {
        return Ok(report);
    }

    let player = stats_client.find_player(player_name).await?;
    fetch_and_cache_report(db, stats_client, &player, group, seasons_back).await
}
