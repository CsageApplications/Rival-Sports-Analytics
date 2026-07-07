//! Database connection and management

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

use crate::error::DbError;

/// Database connection wrapper
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy();
        
        info!(path = %path_str, "Connecting to database");

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path_str))
            .map_err(|e| DbError::Sqlx(e.into()))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// Create an in-memory database (useful for testing)
    pub async fn in_memory() -> Result<Self, DbError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<(), DbError> {
        info!("Running database migrations");
        
        // Create tables manually since we're not using sqlx-cli
        sqlx::query(SCHEMA)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get the connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Database schema
const SCHEMA: &str = r#"
-- Events table
CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    sport_key TEXT NOT NULL,
    sport_title TEXT NOT NULL,
    commence_time TEXT NOT NULL,
    home_team TEXT NOT NULL,
    away_team TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_commence_time ON events(commence_time);
CREATE INDEX IF NOT EXISTS idx_events_sport_key ON events(sport_key);

-- Odds snapshots table (stores full JSON for flexibility)
CREATE TABLE IF NOT EXISTS odds_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL REFERENCES events(id),
    captured_at TEXT NOT NULL,
    data JSON NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_odds_event_id ON odds_snapshots(event_id);
CREATE INDEX IF NOT EXISTS idx_odds_captured_at ON odds_snapshots(captured_at);

-- Bets table
CREATE TABLE IF NOT EXISTS bets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL REFERENCES events(id),
    bookmaker TEXT NOT NULL,
    market TEXT NOT NULL,
    selection TEXT NOT NULL,
    point TEXT,
    odds TEXT NOT NULL,
    stake TEXT NOT NULL,
    potential_payout TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    actual_payout TEXT,
    placed_at TEXT NOT NULL,
    settled_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_bets_event_id ON bets(event_id);
CREATE INDEX IF NOT EXISTS idx_bets_status ON bets(status);

-- Bankroll table (single row)
CREATE TABLE IF NOT EXISTS bankroll (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    balance TEXT NOT NULL,
    total_deposited TEXT NOT NULL,
    total_withdrawn TEXT NOT NULL,
    pending_risk TEXT NOT NULL DEFAULT '0',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_type TEXT NOT NULL,
    amount TEXT NOT NULL,
    balance_after TEXT NOT NULL,
    bet_id INTEGER REFERENCES bets(id),
    description TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_transactions_bet_id ON transactions(bet_id);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at);

-- Historical player stats cache (MLB Stats API). Stores an opaque JSON
-- payload (season stat lines + recent game logs) keyed by player + stat
-- group, so we don't re-fetch statsapi.mlb.com on every chat request.
CREATE TABLE IF NOT EXISTS player_stats_cache (
    player_id INTEGER NOT NULL,
    player_name TEXT NOT NULL,
    stat_group TEXT NOT NULL,
    payload JSON NOT NULL,
    fetched_at TEXT NOT NULL,
    PRIMARY KEY (player_id, stat_group)
);

CREATE INDEX IF NOT EXISTS idx_player_stats_name ON player_stats_cache(player_name);

-- Team standings cache (MLB Stats API), one row per season snapshot
-- (win/loss record + runs scored/allowed per team, used by the game
-- win-probability projection model).
CREATE TABLE IF NOT EXISTS standings_cache (
    season INTEGER PRIMARY KEY,
    payload JSON NOT NULL,
    fetched_at TEXT NOT NULL
);
"#;
