//! # MLB Stats API client
//!
//! Thin, free (no API key required) client for `statsapi.mlb.com`, used
//! to pull historical player season stats and per-game logs so we can
//! answer questions like:
//!
//! - "Is this pitcher's strikeout rate trending up or down over the last
//!   few seasons?" (see [`StatGroup`] + season stats)
//! - "How has this pitcher done the last 3 times he faced this opponent?"
//!   (see game logs + `mlb-predict`'s matchup filtering)

mod client;
mod error;
mod teams;
mod types;

pub use client::StatsApiClient;
pub use error::StatsApiError;
pub use teams::{find_team_id_by_name, team_name_by_id, TeamInfo, MLB_TEAMS};
pub use types::{parse_innings_pitched, GameLogEntry, PlayerSearchResult, SeasonStatLine, StatGroup, TeamStanding};
