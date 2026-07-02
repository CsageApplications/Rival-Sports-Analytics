//! Event and team models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A sporting event (MLB game)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    /// Unique identifier from The Odds API
    pub id: String,
    /// Sport key (e.g., "baseball_mlb")
    pub sport_key: String,
    /// Sport title for display
    pub sport_title: String,
    /// When the game starts
    pub commence_time: DateTime<Utc>,
    /// Home team name
    pub home_team: String,
    /// Away team name
    pub away_team: String,
}

impl Event {
    /// Create a new event
    pub fn new(
        id: impl Into<String>,
        sport_key: impl Into<String>,
        sport_title: impl Into<String>,
        commence_time: DateTime<Utc>,
        home_team: impl Into<String>,
        away_team: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            sport_key: sport_key.into(),
            sport_title: sport_title.into(),
            commence_time,
            home_team: home_team.into(),
            away_team: away_team.into(),
        }
    }

    /// Check if the event has started
    pub fn has_started(&self) -> bool {
        Utc::now() >= self.commence_time
    }

    /// Get a display string for the matchup
    pub fn matchup(&self) -> String {
        format!("{} @ {}", self.away_team, self.home_team)
    }
}

/// Score information for a completed or in-progress event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Score {
    pub event_id: String,
    pub home_score: Option<u32>,
    pub away_score: Option<u32>,
    pub completed: bool,
}
