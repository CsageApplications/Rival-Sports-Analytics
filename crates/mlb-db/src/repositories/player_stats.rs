//! Cache for historical player stats pulled from the MLB Stats API.
//!
//! Stored as an opaque JSON payload (season stat lines + recent game
//! logs) keyed by player_id + stat group, mirroring the `odds_snapshots`
//! "store flexible JSON" pattern used elsewhere in this crate. The
//! `mlb-predict` crate owns the JSON shape; this module only
//! persists/retrieves it.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{Database, DbError};

#[derive(Debug, Clone)]
pub struct PlayerStatsCacheRow {
    pub player_id: i64,
    pub player_name: String,
    pub stat_group: String,
    pub payload_json: String,
    pub fetched_at: DateTime<Utc>,
}

impl Database {
    /// Upsert cached stats for a player + stat group.
    pub async fn save_player_stats_cache(
        &self,
        player_id: i64,
        player_name: &str,
        stat_group: &str,
        payload_json: &str,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO player_stats_cache (player_id, player_name, stat_group, payload, fetched_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(player_id, stat_group) DO UPDATE SET
                player_name = excluded.player_name,
                payload = excluded.payload,
                fetched_at = excluded.fetched_at
            "#,
        )
        .bind(player_id)
        .bind(player_name)
        .bind(stat_group)
        .bind(payload_json)
        .bind(fetched_at.to_rfc3339())
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Exact lookup by player_id + stat group.
    pub async fn get_player_stats_cache(&self, player_id: i64, stat_group: &str) -> Result<PlayerStatsCacheRow, DbError> {
        let row = sqlx::query(
            r#"
            SELECT player_id, player_name, stat_group, payload, fetched_at
            FROM player_stats_cache
            WHERE player_id = ? AND stat_group = ?
            "#,
        )
        .bind(player_id)
        .bind(stat_group)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Cached stats for player {} ({})", player_id, stat_group)))?;

        parse_row(row)
    }

    /// Fuzzy lookup by player display name (case-insensitive substring
    /// match). Used by the chat context builder, which only has a
    /// display name from prop data, not an MLB Stats API player ID.
    pub async fn find_player_stats_cache_by_name(&self, player_name: &str, stat_group: &str) -> Result<PlayerStatsCacheRow, DbError> {
        let pattern = format!("%{}%", player_name.to_lowercase());
        let row = sqlx::query(
            r#"
            SELECT player_id, player_name, stat_group, payload, fetched_at
            FROM player_stats_cache
            WHERE LOWER(player_name) LIKE ? AND stat_group = ?
            ORDER BY fetched_at DESC
            LIMIT 1
            "#,
        )
        .bind(pattern)
        .bind(stat_group)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Cached stats for player '{}' ({})", player_name, stat_group)))?;

        parse_row(row)
    }

    /// List every cached player (used by `mlb history list`).
    pub async fn list_player_stats_cache(&self) -> Result<Vec<PlayerStatsCacheRow>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT player_id, player_name, stat_group, payload, fetched_at
            FROM player_stats_cache
            ORDER BY player_name ASC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(parse_row).collect()
    }
}

fn parse_row(row: sqlx::sqlite::SqliteRow) -> Result<PlayerStatsCacheRow, DbError> {
    let fetched_at: String = row.get("fetched_at");
    Ok(PlayerStatsCacheRow {
        player_id: row.get("player_id"),
        player_name: row.get("player_name"),
        stat_group: row.get("stat_group"),
        payload_json: row.get("payload"),
        fetched_at: DateTime::parse_from_rfc3339(&fetched_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}
