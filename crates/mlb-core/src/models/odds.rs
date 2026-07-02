//! Odds and market models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Market type for betting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketType {
    /// Moneyline (head-to-head winner)
    H2h,
    /// Point spread / run line
    Spreads,
    /// Over/under totals
    Totals,
    /// Futures / outrights
    Outrights,
    // ── Player Props ──────────────────────────────────────
    /// Batter hits Over/Under
    BatterHits,
    /// Batter home runs Over/Under
    BatterHomeRuns,
    /// Batter RBIs Over/Under
    BatterRbis,
    /// Batter stolen bases Over/Under
    BatterStolenBases,
    /// Batter strikeouts Over/Under
    BatterStrikeouts,
    /// Batter walks Over/Under
    BatterWalks,
    /// Pitcher strikeouts Over/Under
    PitcherStrikeouts,
    /// Pitcher hits allowed Over/Under
    PitcherHitsAllowed,
    /// Pitcher walks Over/Under
    PitcherWalks,
    /// Pitcher earned runs Over/Under
    PitcherEarnedRuns,
}

impl MarketType {
    /// Get the API key string for this market
    pub fn as_api_key(&self) -> &'static str {
        match self {
            MarketType::H2h => "h2h",
            MarketType::Spreads => "spreads",
            MarketType::Totals => "totals",
            MarketType::Outrights => "outrights",
            MarketType::BatterHits => "batter_hits",
            MarketType::BatterHomeRuns => "batter_home_runs",
            MarketType::BatterRbis => "batter_rbis",
            MarketType::BatterStolenBases => "batter_stolen_bases",
            MarketType::BatterStrikeouts => "batter_strikeouts",
            MarketType::BatterWalks => "batter_walks",
            MarketType::PitcherStrikeouts => "pitcher_strikeouts",
            MarketType::PitcherHitsAllowed => "pitcher_hits_allowed",
            MarketType::PitcherWalks => "pitcher_walks",
            MarketType::PitcherEarnedRuns => "pitcher_earned_runs",
        }
    }

    /// All available player prop markets for MLB
    pub fn mlb_player_props() -> &'static [MarketType] {
        &[
            MarketType::BatterHits,
            MarketType::BatterHomeRuns,
            MarketType::BatterRbis,
            MarketType::BatterStolenBases,
            MarketType::BatterStrikeouts,
            MarketType::BatterWalks,
            MarketType::PitcherStrikeouts,
            MarketType::PitcherHitsAllowed,
            MarketType::PitcherWalks,
            MarketType::PitcherEarnedRuns,
        ]
    }

    /// Returns true if this is a player prop market
    pub fn is_player_prop(&self) -> bool {
        matches!(
            self,
            MarketType::BatterHits
                | MarketType::BatterHomeRuns
                | MarketType::BatterRbis
                | MarketType::BatterStolenBases
                | MarketType::BatterStrikeouts
                | MarketType::BatterWalks
                | MarketType::PitcherStrikeouts
                | MarketType::PitcherHitsAllowed
                | MarketType::PitcherWalks
                | MarketType::PitcherEarnedRuns
        )
    }
}

impl fmt::Display for MarketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketType::H2h => write!(f, "Moneyline"),
            MarketType::Spreads => write!(f, "Spread"),
            MarketType::Totals => write!(f, "Totals"),
            MarketType::Outrights => write!(f, "Futures"),
            MarketType::BatterHits => write!(f, "Hits"),
            MarketType::BatterHomeRuns => write!(f, "Home Runs"),
            MarketType::BatterRbis => write!(f, "RBIs"),
            MarketType::BatterStolenBases => write!(f, "Stolen Bases"),
            MarketType::BatterStrikeouts => write!(f, "Batter Strikeouts"),
            MarketType::BatterWalks => write!(f, "Walks"),
            MarketType::PitcherStrikeouts => write!(f, "Pitcher Strikeouts"),
            MarketType::PitcherHitsAllowed => write!(f, "Hits Allowed"),
            MarketType::PitcherWalks => write!(f, "Walks Allowed"),
            MarketType::PitcherEarnedRuns => write!(f, "Earned Runs"),
        }
    }
}

/// A single betting outcome (e.g., "Yankees to win at -150")
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    /// Team/player name, or "Over"/"Under" for totals/props
    pub name: String,
    /// American odds (e.g., -150, +130)
    pub price: Decimal,
    /// Point value for spreads/totals/props (e.g., -1.5 run line, 8.5 total, 0.5 HR)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point: Option<Decimal>,
    /// Player name for player prop markets
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

impl Outcome {
    /// Create a new outcome
    pub fn new(name: impl Into<String>, price: Decimal, point: Option<Decimal>) -> Self {
        Self {
            name: name.into(),
            price,
            point,
            description: None,
        }
    }

    /// Create a new player prop outcome (with player name in description)
    pub fn new_prop(
        name: impl Into<String>,
        price: Decimal,
        point: Option<Decimal>,
        player: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            price,
            point,
            description: Some(player.into()),
        }
    }

    /// Convert American odds to implied probability
    ///
    /// - Positive odds (+150): 100 / (odds + 100) = 0.40
    /// - Negative odds (-150): |odds| / (|odds| + 100) = 0.60
    pub fn implied_probability(&self) -> Decimal {
        let hundred = Decimal::from(100);
        if self.price >= Decimal::ZERO {
            hundred / (self.price + hundred)
        } else {
            let abs_price = self.price.abs();
            abs_price / (abs_price + hundred)
        }
    }

    /// Convert American odds to decimal odds
    ///
    /// - Positive odds (+150): (odds / 100) + 1 = 2.50
    /// - Negative odds (-150): (100 / |odds|) + 1 = 1.67
    pub fn decimal_odds(&self) -> Decimal {
        let hundred = Decimal::from(100);
        let one = Decimal::ONE;
        if self.price >= Decimal::ZERO {
            (self.price / hundred) + one
        } else {
            (hundred / self.price.abs()) + one
        }
    }
}

/// A market offered by a bookmaker (e.g., FanDuel's moneyline)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Market {
    /// Market type key
    pub key: MarketType,
    /// Last update time for this market
    pub last_update: DateTime<Utc>,
    /// Available outcomes
    pub outcomes: Vec<Outcome>,
}

/// A bookmaker's odds for an event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bookmaker {
    /// Bookmaker key (e.g., "fanduel")
    pub key: String,
    /// Display name (e.g., "FanDuel")
    pub title: String,
    /// Markets offered
    pub markets: Vec<Market>,
}

impl Bookmaker {
    /// Find a specific market type
    pub fn get_market(&self, market_type: MarketType) -> Option<&Market> {
        self.markets.iter().find(|m| m.key == market_type)
    }
}

/// Complete odds data for an event from multiple bookmakers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventOdds {
    /// Event ID
    pub event_id: String,
    /// When this odds snapshot was captured
    pub captured_at: DateTime<Utc>,
    /// Bookmakers with their odds
    pub bookmakers: Vec<Bookmaker>,
}

impl EventOdds {
    /// Get the best odds for a specific outcome across all bookmakers
    pub fn best_odds(&self, market_type: MarketType, outcome_name: &str) -> Option<(&Bookmaker, &Outcome)> {
        let mut best: Option<(&Bookmaker, &Outcome)> = None;

        for bookmaker in &self.bookmakers {
            if let Some(market) = bookmaker.get_market(market_type) {
                for outcome in &market.outcomes {
                    if outcome.name == outcome_name {
                        match &best {
                            None => best = Some((bookmaker, outcome)),
                            Some((_, current_best)) => {
                                // Higher decimal odds = better for bettor
                                if outcome.decimal_odds() > current_best.decimal_odds() {
                                    best = Some((bookmaker, outcome));
                                }
                            }
                        }
                    }
                }
            }
        }
        best
    }

    /// Merge another odds snapshot's markets into this one, grouped by
    /// bookmaker key. Used to combine a base (h2h/spreads/totals) fetch with
    /// a separate player-props fetch for the same event into a single
    /// snapshot before persisting.
    pub fn merge_bookmakers(&mut self, other: &EventOdds) {
        for other_book in &other.bookmakers {
            match self.bookmakers.iter_mut().find(|b| b.key == other_book.key) {
                Some(existing) => {
                    for market in &other_book.markets {
                        if !existing.markets.iter().any(|m| m.key == market.key) {
                            existing.markets.push(market.clone());
                        }
                    }
                }
                None => self.bookmakers.push(other_book.clone()),
            }
        }
    }
}
