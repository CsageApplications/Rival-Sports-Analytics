//! Cached team standings lookups, used as input to the game-winner
//! projection model in [`crate::game_model`].
//!
//! Unlike player history (`report.rs`, deliberately cache-only — hundreds
//! of players is too many HTTP calls to do inline), standings is a
//! single cheap request for ~30 teams, so it's safe to live-fetch and
//! cache inline on a stale/missing cache hit, even from within a chat
//! request.

use chrono::Utc;
use mlb_db::Database;
use mlb_stats_client::{StatsApiClient, TeamStanding};

use crate::error::PredictError;

/// How long a cached standings snapshot stays "fresh" before we bother
/// re-fetching. Standings change slowly (once per day of games), so a
/// few hours of staleness is fine.
pub const STANDINGS_CACHE_TTL_HOURS: i64 = 6;

/// Look up cached standings for a season, without calling out to
/// statsapi.mlb.com. Returns `None` if not cached, or cached but older
/// than `max_age_hours` (pass `None` to accept any age).
pub async fn get_cached_standings(db: &Database, season: i32, max_age_hours: Option<i64>) -> Result<Option<Vec<TeamStanding>>, PredictError> {
    let row = match db.get_standings_cache(season).await {
        Ok(row) => row,
        Err(_) => return Ok(None),
    };

    if let Some(max_age) = max_age_hours {
        let age = Utc::now().signed_duration_since(row.fetched_at).num_hours();
        if age > max_age {
            return Ok(None);
        }
    }

    let standings: Vec<TeamStanding> = serde_json::from_str(&row.payload_json)?;
    Ok(Some(standings))
}

/// Fetch fresh standings from statsapi.mlb.com and cache them.
pub async fn fetch_and_cache_standings(db: &Database, stats_client: &StatsApiClient, season: i32) -> Result<Vec<TeamStanding>, PredictError> {
    let standings = stats_client.get_standings(season).await?;
    let payload_json = serde_json::to_string(&standings)?;
    db.save_standings_cache(season, &payload_json, Utc::now()).await?;
    Ok(standings)
}

/// Convenience: use the cache if fresh, otherwise fetch + cache fresh
/// data. This is the entry point most callers (CLI, chat context
/// builder) should use.
pub async fn get_or_fetch_standings(db: &Database, stats_client: &StatsApiClient, season: i32) -> Result<Vec<TeamStanding>, PredictError> {
    if let Some(standings) = get_cached_standings(db, season, Some(STANDINGS_CACHE_TTL_HOURS)).await? {
        return Ok(standings);
    }
    fetch_and_cache_standings(db, stats_client, season).await
}

/// Fuzzy-resolve a team name (e.g. from Odds API event data, "Houston
/// Astros") to its standings row. Tries an exact case-insensitive match
/// first, then falls back to substring matching in either direction.
pub fn find_standing_by_team_name<'a>(teams: &'a [TeamStanding], name: &str) -> Option<&'a TeamStanding> {
    let q = name.to_lowercase();

    if let Some(t) = teams.iter().find(|t| t.team_name.to_lowercase() == q) {
        return Some(t);
    }

    teams.iter().find(|t| {
        let full = t.team_name.to_lowercase();
        full.contains(&q) || q.contains(&full)
    })
}
