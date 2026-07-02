//! Bankroll management models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Type of bankroll transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Initial deposit
    Deposit,
    /// Withdrawal
    Withdrawal,
    /// Bet placed (debit)
    BetPlaced,
    /// Bet won (credit)
    BetWon,
    /// Bet pushed (refund)
    BetPush,
    /// Manual adjustment
    Adjustment,
}

/// A bankroll transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub transaction_type: TransactionType,
    /// Positive for credits, negative for debits
    pub amount: Decimal,
    /// Balance after this transaction
    pub balance_after: Decimal,
    /// Related bet ID if applicable
    pub bet_id: Option<i64>,
    /// Description
    pub description: String,
    pub created_at: DateTime<Utc>,
}

/// Current bankroll state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bankroll {
    /// Current available balance
    pub balance: Decimal,
    /// Total deposited
    pub total_deposited: Decimal,
    /// Total withdrawn
    pub total_withdrawn: Decimal,
    /// Amount currently at risk (pending bets)
    pub pending_risk: Decimal,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

impl Bankroll {
    /// Create a new bankroll with initial deposit
    pub fn new(initial_deposit: Decimal) -> Self {
        Self {
            balance: initial_deposit,
            total_deposited: initial_deposit,
            total_withdrawn: Decimal::ZERO,
            pending_risk: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Check if we can afford a bet
    pub fn can_afford(&self, amount: Decimal) -> bool {
        self.balance >= amount
    }

    /// Calculate total profit/loss
    pub fn total_profit_loss(&self) -> Decimal {
        self.balance + self.total_withdrawn - self.total_deposited
    }

    /// Calculate available balance (excluding pending risk)
    pub fn available_balance(&self) -> Decimal {
        self.balance - self.pending_risk
    }
}

/// Bankroll statistics over time
#[derive(Debug, Clone)]
pub struct BankrollStats {
    pub starting_balance: Decimal,
    pub current_balance: Decimal,
    pub high_watermark: Decimal,
    pub low_watermark: Decimal,
    pub total_profit_loss: Decimal,
    pub roi_percentage: Decimal,
}
