//! Minimal Anthropic Claude client for the parlay-building chat assistant.
//!
//! This intentionally only implements the single Messages API call this
//! app needs — a system prompt + one user turn, non-streaming — rather
//! than pulling in a full-featured SDK.

mod error;

pub use error::LlmError;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default model used if `ANTHROPIC_MODEL` is not set in the environment.
pub const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

/// Client for calling the Anthropic Messages API.
#[derive(Debug, Clone)]
pub struct ClaudeClient {
    http: Client,
    api_key: String,
    model: String,
}

impl ClaudeClient {
    /// Create a new client with the given API key and model identifier.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Send a single system-prompt + user-message turn and return Claude's
    /// full text reply (concatenation of all text content blocks).
    pub async fn ask(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        let body = MessagesRequest {
            model: &self.model,
            max_tokens: 1536,
            system: system_prompt,
            messages: vec![Message {
                role: "user",
                content: user_message,
            }],
        };

        let response = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(LlmError::Request)?;

        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let text = response.text().await.map_err(LlmError::Request)?;
        let parsed: MessagesResponse = serde_json::from_str(&text)
            .map_err(|e| LlmError::Parse(format!("{e}: {}", &text[..text.len().min(300)])))?;

        let reply = parsed
            .content
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text),
                ContentBlock::Other => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if reply.trim().is_empty() {
            return Err(LlmError::Parse("empty response from model".into()));
        }

        Ok(reply)
    }
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<Message<'a>>,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(other)]
    Other,
}
