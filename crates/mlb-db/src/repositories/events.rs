//! Events repository implementation

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{Database, DbError};
use mlb_core::models::Event;
use mlb_core::repository::{EventRepository, RepoResult, RepositoryError};

impl EventRepository for Database {
    async fn save_event(&self, event: &Event) -> RepoResult<()> {
        save_event_impl(self, event).await.map_err(Into::into)
    }

    async fn get_event(&self, id: &str) -> RepoResult<Event> {
        get_event_impl(self, id).await.map_err(Into::into)
    }

    async fn get_upcoming_events(&self) -> RepoResult<Vec<Event>> {
        get_upcoming_events_impl(self).await.map_err(Into::into)
    }

    async fn save_events(&self, events: &[Event]) -> RepoResult<()> {
        for event in events {
            save_event_impl(self, event).await?;
        }
        Ok(())
    }
}

async fn save_event_impl(db: &Database, event: &Event) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO events (id, sport_key, sport_title, commence_time, home_team, away_team, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
            sport_key = excluded.sport_key,
            sport_title = excluded.sport_title,
            commence_time = excluded.commence_time,
            home_team = excluded.home_team,
            away_team = excluded.away_team,
            updated_at = datetime('now')
        "#,
    )
    .bind(&event.id)
    .bind(&event.sport_key)
    .bind(&event.sport_title)
    .bind(event.commence_time.to_rfc3339())
    .bind(&event.home_team)
    .bind(&event.away_team)
    .execute(db.pool())
    .await?;

    Ok(())
}

async fn get_event_impl(db: &Database, id: &str) -> Result<Event, DbError> {
    let row = sqlx::query(
        r#"
        SELECT id, sport_key, sport_title, commence_time, home_team, away_team
        FROM events
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?
    .ok_or_else(|| DbError::NotFound(format!("Event {}", id)))?;

    Ok(Event {
        id: row.get("id"),
        sport_key: row.get("sport_key"),
        sport_title: row.get("sport_title"),
        commence_time: parse_datetime(row.get("commence_time")),
        home_team: row.get("home_team"),
        away_team: row.get("away_team"),
    })
}

async fn get_upcoming_events_impl(db: &Database) -> Result<Vec<Event>, DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, sport_key, sport_title, commence_time, home_team, away_team
        FROM events
        WHERE commence_time > datetime('now')
        ORDER BY commence_time ASC
        "#,
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Event {
            id: row.get("id"),
            sport_key: row.get("sport_key"),
            sport_title: row.get("sport_title"),
            commence_time: parse_datetime(row.get("commence_time")),
            home_team: row.get("home_team"),
            away_team: row.get("away_team"),
        })
        .collect())
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
