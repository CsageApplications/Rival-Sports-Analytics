//! Error types for the Claude client

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Anthropic API returned error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Failed to parse API response: {0}")]
    Parse(String),
}
