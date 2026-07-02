//! Repository traits for data persistence
//!
//! These traits define the interface for storing and retrieving data.
//! Implementations live in the `mlb-db` crate.

use std::future::Future;
use crate::models::{Event, EventOdds, Bet, BetStatus, Bankroll, Transaction};

/// Error type for repository operations
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Record not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Constraint violation: {0}")]
    Constraint(String),
}

pub type RepoResult<T> = Result<T, RepositoryError>;

/// Repository for events
pub trait EventRepository {
    fn save_event(&self, event: &Event) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_event(&self, id: &str) -> impl Future<Output = RepoResult<Event>> + Send;
    fn get_upcoming_events(&self) -> impl Future<Output = RepoResult<Vec<Event>>> + Send;
    fn save_events(&self, events: &[Event]) -> impl Future<Output = RepoResult<()>> + Send;
}

/// Repository for odds data
pub trait OddsRepository {
    fn save_odds(&self, odds: &EventOdds) -> impl Future<Output = RepoResult<()>> + Send;
    fn get_latest_odds(&self, event_id: &str) -> impl Future<Output = RepoResult<EventOdds>> + Send;
    fn get_odds_history(&self, event_id: &str) -> impl Future<Output = RepoResult<Vec<EventOdds>>> + Send;
}

/// Repository for bets
pub trait BetRepository {
    fn save_bet(&self, bet: &Bet) -> impl Future<Output = RepoResult<i64>> + Send;
    fn get_bet(&self, id: i64) -> impl Future<Output = RepoResult<Bet>> + Send;
    fn get_bets_by_status(&self, status: BetStatus) -> impl Future<Output = RepoResult<Vec<Bet>>> + Send;
    fn get_bets_for_event(&self, event_id: &str) -> impl Future<Output = RepoResult<Vec<Bet>>> + Send;
    fn update_bet_status(&self, id: i64, status: BetStatus, payout: Option<rust_decimal::Decimal>) -> impl Future<Output = RepoResult<()>> + Send;
}

/// Repository for bankroll and transactions
pub trait BankrollRepository {
    fn get_bankroll(&self) -> impl Future<Output = RepoResult<Bankroll>> + Send;
    fn update_bankroll(&self, bankroll: &Bankroll) -> impl Future<Output = RepoResult<()>> + Send;
    fn add_transaction(&self, transaction: &Transaction) -> impl Future<Output = RepoResult<i64>> + Send;
    fn get_transactions(&self, limit: usize) -> impl Future<Output = RepoResult<Vec<Transaction>>> + Send;
}
