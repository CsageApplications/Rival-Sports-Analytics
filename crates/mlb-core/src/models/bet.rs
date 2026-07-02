//! Bet tracking models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::MarketType;

/// Status of a bet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetStatus {
    /// Bet placed, awaiting result
    Pending,
    /// Bet won
    Won,
    /// Bet lost
    Lost,
    /// Bet pushed (tie, money returned)
    Push,
    /// Bet voided/cancelled
    Void,
}

/// A recorded bet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    /// Unique identifier
    pub id: i64,
    /// Associated event ID
    pub event_id: String,
    /// Bookmaker used
    pub bookmaker: String,
    /// Market type
    pub market: MarketType,
    /// Outcome selected (team name or Over/Under)
    pub selection: String,
    /// Point line if applicable
    pub point: Option<Decimal>,
    /// Odds at time of bet (American)
    pub odds: Decimal,
    /// Stake amount
    pub stake: Decimal,
    /// Potential payout (stake + profit)
    pub potential_payout: Decimal,
    /// Current status
    pub status: BetStatus,
    /// Actual payout if settled
    pub actual_payout: Option<Decimal>,
    /// When the bet was placed
    pub placed_at: DateTime<Utc>,
    /// When the bet was settled
    pub settled_at: Option<DateTime<Utc>>,
    /// Notes
    pub notes: Option<String>,
}

impl Bet {
    /// Calculate potential profit (not including stake)
    pub fn potential_profit(&self) -> Decimal {
        self.potential_payout - self.stake
    }

    /// Calculate payout from American odds and stake
    pub fn calculate_payout(odds: Decimal, stake: Decimal) -> Decimal {
        let hundred = Decimal::from(100);
        let profit = if odds >= Decimal::ZERO {
            stake * odds / hundred
        } else {
            stake * hundred / odds.abs()
        };
        stake + profit
    }

    /// Calculate actual profit/loss after settlement
    pub fn profit_loss(&self) -> Option<Decimal> {
        self.actual_payout.map(|payout| payout - self.stake)
    }
}

/// Summary statistics for a collection of bets
#[derive(Debug, Clone, Default)]
pub struct BetStats {
    pub total_bets: u32,
    pub pending: u32,
    pub won: u32,
    pub lost: u32,
    pub pushed: u32,
    pub total_staked: Decimal,
    pub total_returned: Decimal,
    pub net_profit: Decimal,
}

impl BetStats {
    /// Calculate win rate (excluding pending/pushed)
    pub fn win_rate(&self) -> Option<Decimal> {
        let decided = self.won + self.lost;
        if decided == 0 {
            None
        } else {
            Some(Decimal::from(self.won) / Decimal::from(decided))
        }
    }

    /// Calculate ROI (return on investment)
    pub fn roi(&self) -> Option<Decimal> {
        if self.total_staked == Decimal::ZERO {
            None
        } else {
            Some(self.net_profit / self.total_staked)
        }
    }
}
