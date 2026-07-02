//! Odds repository implementation

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{Database, DbError};
use mlb_core::models::EventOdds;
use mlb_core::repository::{OddsRepository, RepoResult};

impl OddsRepository for Database {
    async fn save_odds(&self, odds: &EventOdds) -> RepoResult<()> {
        save_odds_impl(self, odds).await.map_err(Into::into)
    }

    async fn get_latest_odds(&self, event_id: &str) -> RepoResult<EventOdds> {
        get_latest_odds_impl(self, event_id).await.map_err(Into::into)
    }

    async fn get_odds_history(&self, event_id: &str) -> RepoResult<Vec<EventOdds>> {
        get_odds_history_impl(self, event_id).await.map_err(Into::into)
    }
}

async fn save_odds_impl(db: &Database, odds: &EventOdds) -> Result<(), DbError> {
    let data = serde_json::to_string(&odds.bookmakers)?;

    sqlx::query(
        r#"
        INSERT INTO odds_snapshots (event_id, captured_at, data)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&odds.event_id)
    .bind(odds.captured_at.to_rfc3339())
    .bind(&data)
    .execute(db.pool())
    .await?;

    Ok(())
}

async fn get_latest_odds_impl(db: &Database, event_id: &str) -> Result<EventOdds, DbError> {
    let row = sqlx::query(
        r#"
        SELECT event_id, captured_at, data
        FROM odds_snapshots
        WHERE event_id = ?
        ORDER BY captured_at DESC
        LIMIT 1
        "#,
    )
    .bind(event_id)
    .fetch_optional(db.pool())
    .await?
    .ok_or_else(|| DbError::NotFound(format!("Odds for event {}", event_id)))?;

    parse_odds_row(row)
}

async fn get_odds_history_impl(db: &Database, event_id: &str) -> Result<Vec<EventOdds>, DbError> {
    let rows = sqlx::query(
        r#"
        SELECT event_id, captured_at, data
        FROM odds_snapshots
        WHERE event_id = ?
        ORDER BY captured_at DESC
        "#,
    )
    .bind(event_id)
    .fetch_all(db.pool())
    .await?;

    rows.into_iter().map(parse_odds_row).collect()
}

fn parse_odds_row(row: sqlx::sqlite::SqliteRow) -> Result<EventOdds, DbError> {
    let event_id: String = row.get("event_id");
    let captured_at: String = row.get("captured_at");
    let data: String = row.get("data");

    let bookmakers = serde_json::from_str(&data)?;

    Ok(EventOdds {
        event_id,
        captured_at: DateTime::parse_from_rfc3339(&captured_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        bookmakers,
    })
}
