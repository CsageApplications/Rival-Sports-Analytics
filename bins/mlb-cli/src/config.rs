//! Configuration management

use std::env;
use std::fmt;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// The Odds API key
    pub api_key: String,
    /// Database file path
    pub db_path: PathBuf,
}

impl Config {
    /// Load configuration from environment
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = env::var("ODDS_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;

        let db_path = env::var("MLB_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("mlb_betting.db"));

        Ok(Self { api_key, db_path })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingApiKey,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingApiKey => write!(
                f,
                "ODDS_API_KEY environment variable not set. Get your API key from https://the-odds-api.com"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}
