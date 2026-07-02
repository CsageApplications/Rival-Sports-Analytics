//! API response types

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use mlb_core::models::{
    Bookmaker as DomainBookmaker, Event as DomainEvent, EventOdds as DomainEventOdds,
    Market as DomainMarket, MarketType, Outcome as DomainOutcome,
};

/// Region for bookmakers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    #[default]
    Us,
    Us2,
    Uk,
    Au,
    Eu,
}

impl Region {
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::Us => "us",
            Region::Us2 => "us2",
            Region::Uk => "uk",
            Region::Au => "au",
            Region::Eu => "eu",
        }
    }
}

/// Odds format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OddsFormat {
    #[default]
    American,
    Decimal,
}

impl OddsFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OddsFormat::American => "american",
            OddsFormat::Decimal => "decimal",
        }
    }
}

/// Sport from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSport {
    pub key: String,
    pub group: String,
    pub title: String,
    pub description: String,
    pub active: bool,
    pub has_outrights: bool,
}

/// Event from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEvent {
    pub id: String,
    pub sport_key: String,
    pub sport_title: String,
    pub commence_time: DateTime<Utc>,
    pub home_team: String,
    pub away_team: String,
}

impl From<ApiEvent> for DomainEvent {
    fn from(api: ApiEvent) -> Self {
        DomainEvent::new(
            api.id,
            api.sport_key,
            api.sport_title,
            api.commence_time,
            api.home_team,
            api.away_team,
        )
    }
}

/// Outcome from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiOutcome {
    pub name: String,
    pub price: Decimal,
    #[serde(default)]
    pub point: Option<Decimal>,
    /// Player name for player prop markets
    #[serde(default)]
    pub description: Option<String>,
}

impl From<ApiOutcome> for DomainOutcome {
    fn from(api: ApiOutcome) -> Self {
        let mut outcome = DomainOutcome::new(api.name, api.price, api.point);
        outcome.description = api.description;
        outcome
    }
}

/// Market from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMarket {
    pub key: String,
    /// Some endpoints (notably per-event player-prop responses) omit this
    /// field entirely; fall back to "now" when absent.
    #[serde(default = "Utc::now")]
    pub last_update: DateTime<Utc>,
    pub outcomes: Vec<ApiOutcome>,
}

impl ApiMarket {
    pub fn to_domain(&self) -> Option<DomainMarket> {
        let key = match self.key.as_str() {
            "h2h"                  => MarketType::H2h,
            "spreads"              => MarketType::Spreads,
            "totals"               => MarketType::Totals,
            "outrights"            => MarketType::Outrights,
            "batter_hits"          => MarketType::BatterHits,
            "batter_home_runs"     => MarketType::BatterHomeRuns,
            "batter_rbis"          => MarketType::BatterRbis,
            "batter_stolen_bases"  => MarketType::BatterStolenBases,
            "batter_strikeouts"    => MarketType::BatterStrikeouts,
            "batter_walks"         => MarketType::BatterWalks,
            "pitcher_strikeouts"   => MarketType::PitcherStrikeouts,
            "pitcher_hits_allowed" => MarketType::PitcherHitsAllowed,
            "pitcher_walks"        => MarketType::PitcherWalks,
            "pitcher_earned_runs"  => MarketType::PitcherEarnedRuns,
            _ => return None,
        };

        Some(DomainMarket {
            key,
            last_update: self.last_update,
            outcomes: self.outcomes.iter().cloned().map(Into::into).collect(),
        })
    }
}

/// Bookmaker from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBookmaker {
    pub key: String,
    pub title: String,
    #[serde(default = "Utc::now")]
    pub last_update: DateTime<Utc>,
    pub markets: Vec<ApiMarket>,
}

impl From<ApiBookmaker> for DomainBookmaker {
    fn from(api: ApiBookmaker) -> Self {
        DomainBookmaker {
            key: api.key,
            title: api.title,
            markets: api.markets.iter().filter_map(|m| m.to_domain()).collect(),
        }
    }
}

/// Event with odds from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEventOdds {
    pub id: String,
    pub sport_key: String,
    pub sport_title: String,
    pub commence_time: DateTime<Utc>,
    pub home_team: String,
    pub away_team: String,
    #[serde(default)]
    pub bookmakers: Vec<ApiBookmaker>,
}

impl ApiEventOdds {
    pub fn to_domain(&self) -> (DomainEvent, DomainEventOdds) {
        let event = DomainEvent::new(
            &self.id,
            &self.sport_key,
            &self.sport_title,
            self.commence_time,
            &self.home_team,
            &self.away_team,
        );

        let odds = DomainEventOdds {
            event_id: self.id.clone(),
            captured_at: Utc::now(),
            bookmakers: self.bookmakers.iter().cloned().map(Into::into).collect(),
        };

        (event, odds)
    }
}

/// Score from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiScore {
    pub id: String,
    pub sport_key: String,
    pub sport_title: String,
    pub commence_time: DateTime<Utc>,
    pub completed: bool,
    pub home_team: String,
    pub away_team: String,
    pub scores: Option<Vec<ApiTeamScore>>,
    pub last_update: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTeamScore {
    pub name: String,
    pub score: String,
}
