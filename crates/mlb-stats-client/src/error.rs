//! Error types for the MLB Stats API client

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatsApiError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    Parse(String),

    #[error("No player found matching '{0}'")]
    PlayerNotFound(String),

    #[error("Ambiguous player name '{0}': {1} matches found, please be more specific")]
    AmbiguousPlayer(String, usize),
}
