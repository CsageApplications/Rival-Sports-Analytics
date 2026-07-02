//! Configuration for the mlb-server binary

use std::env;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Anthropic API key for the parlay chat assistant
    pub anthropic_api_key: String,
    /// Anthropic model identifier (overridable in case of model retirement)
    pub anthropic_model: String,
    /// Database file path (shared with the mlb CLI)
    pub db_path: PathBuf,
    /// Port to bind the HTTP server on
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let anthropic_api_key =
            env::var("ANTHROPIC_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;

        let anthropic_model = env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| mlb_llm::DEFAULT_MODEL.to_string());

        let db_path = env::var("MLB_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("mlb_betting.db"));

        let port = env::var("MLB_SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8787);

        Ok(Self {
            anthropic_api_key,
            anthropic_model,
            db_path,
            port,
        })
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
                "ANTHROPIC_API_KEY environment variable not set. Get a key from https://console.anthropic.com"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}
