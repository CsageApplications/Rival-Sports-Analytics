//! # MLB Odds Client
//!
//! HTTP client for The Odds API v4.
//!
//! ## Usage
//!
//! ```no_run
//! use mlb_odds_client::{OddsClient, Region};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = OddsClient::new("your-api-key")?;
//!     
//!     let events = client.get_events("baseball_mlb").await?;
//!     println!("Found {} upcoming MLB games", events.len());
//!     
//!     Ok(())
//! }
//! ```

mod client;
mod types;
mod error;
mod rate_limiter;

pub use client::OddsClient;
pub use client::ALLOWED_BOOKMAKERS;
pub use types::*;
pub use error::OddsApiError;
pub use rate_limiter::RateLimiter;
