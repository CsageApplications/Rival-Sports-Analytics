//! Error types for the Odds API client

use thiserror::Error;

#[derive(Debug, Error)]
pub enum OddsApiError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    Parse(String),

    #[error("API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Rate limited - requests remaining: {remaining}")]
    RateLimited { remaining: u32 },

    #[error("Invalid API key")]
    Unauthorized,

    #[error("Configuration error: {0}")]
    Config(String),
}
