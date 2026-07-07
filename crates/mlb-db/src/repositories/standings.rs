//! Cache for MLB Stats API team standings (win/loss + runs scored/allowed
//! per team), keyed by season.
//!
//! Unlike `player_stats_cache` (hundreds of players, deliberately
//! cache-only), standings is a single cheap HTTP call for ~30 teams, so
//! callers are allowed to live-fetch-and-cache inline on a stale/missing
//! cache hit. This module only persists/retrieves the opaque JSON
//! payload; `mlb-predict` owns the JSON shape (`Vec<TeamStanding>`).

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{Database, DbError};

#[derive(Debug, Clone)]
pub struct StandingsCacheRow {
    pub season: i32,
    pub payload_json: String,
    pub fetched_at: DateTime<Utc>,
}

impl Database {
    /// Upsert the cached standings snapshot for a season.
    pub async fn save_standings_cache(
        &self,
        season: i32,
        payload_json: &str,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO standings_cache (season, payload, fetched_at)
            VALUES (?, ?, ?)
            ON CONFLICT(season) DO UPDATE SET
                payload = excluded.payload,
                fetched_at = excluded.fetched_at
            "#,
        )
        .bind(season)
        .bind(payload_json)
        .bind(fetched_at.to_rfc3339())
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Fetch the cached standings snapshot for a season, if present.
    pub async fn get_standings_cache(&self, season: i32) -> Result<StandingsCacheRow, DbError> {
        let row = sqlx::query(
            r#"
            SELECT season, payload, fetched_at
            FROM standings_cache
            WHERE season = ?
            "#,
        )
        .bind(season)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Cached standings for season {}", season)))?;

        let fetched_at: String = row.get("fetched_at");
        Ok(StandingsCacheRow {
            season: row.get("season"),
            payload_json: row.get("payload"),
            fetched_at: DateTime::parse_from_rfc3339(&fetched_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}
