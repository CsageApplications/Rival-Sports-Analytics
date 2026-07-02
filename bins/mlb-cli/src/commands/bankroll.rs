//! Bankroll commands

use anyhow::{bail, Result};
use clap::Subcommand;
use rust_decimal::Decimal;
use std::str::FromStr;

use mlb_core::models::{Bankroll, Transaction, TransactionType};
use mlb_core::repository::BankrollRepository;
use mlb_db::Database;
use crate::config::Config;

#[derive(Subcommand)]
pub enum BankrollCommands {
    /// Show current bankroll status
    Status,
    /// Deposit funds
    Deposit {
        /// Amount to deposit
        amount: String,
    },
    /// Withdraw funds
    Withdraw {
        /// Amount to withdraw
        amount: String,
    },
    /// Show recent transactions
    History {
        /// Number of transactions to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

impl BankrollCommands {
    pub async fn run(self) -> Result<()> {
        let config = Config::from_env()?;
        let db = Database::new(&config.db_path).await?;
        db.migrate().await?;

        match self {
            BankrollCommands::Status => show_status(&db).await,
            BankrollCommands::Deposit { amount } => deposit(&db, &amount).await,
            BankrollCommands::Withdraw { amount } => withdraw(&db, &amount).await,
            BankrollCommands::History { limit } => show_history(&db, limit).await,
        }
    }
}

async fn show_status(db: &Database) -> Result<()> {
    let bankroll = db.get_bankroll().await?;

    println!("Bankroll Status\n");
    println!("{:=<40}", "");
    println!("  Balance:        ${:.2}", bankroll.balance);
    println!("  Pending Risk:   ${:.2}", bankroll.pending_risk);
    println!("  Available:      ${:.2}", bankroll.available_balance());
    println!("{:-<40}", "");
    println!("  Total Deposited: ${:.2}", bankroll.total_deposited);
    println!("  Total Withdrawn: ${:.2}", bankroll.total_withdrawn);
    println!("  Net P/L:         ${:+.2}", bankroll.total_profit_loss());
    println!("{:=<40}", "");

    if bankroll.total_deposited > Decimal::ZERO {
        let roi = (bankroll.total_profit_loss() / bankroll.total_deposited) * Decimal::from(100);
        println!("  ROI: {:+.2}%", roi);
    }

    Ok(())
}

async fn deposit(db: &Database, amount_str: &str) -> Result<()> {
    let amount = parse_amount(amount_str)?;
    
    if amount <= Decimal::ZERO {
        bail!("Deposit amount must be positive");
    }

    let mut bankroll = db.get_bankroll().await?;
    let new_balance = bankroll.balance + amount;

    // Create transaction
    let transaction = Transaction {
        id: 0, // Will be set by DB
        transaction_type: TransactionType::Deposit,
        amount,
        balance_after: new_balance,
        bet_id: None,
        description: format!("Deposit ${:.2}", amount),
        created_at: chrono::Utc::now(),
    };

    // Update bankroll
    bankroll.balance = new_balance;
    bankroll.total_deposited += amount;
    bankroll.updated_at = chrono::Utc::now();

    db.update_bankroll(&bankroll).await?;
    db.add_transaction(&transaction).await?;

    println!("✓ Deposited ${:.2}", amount);
    println!("  New balance: ${:.2}", new_balance);

    Ok(())
}

async fn withdraw(db: &Database, amount_str: &str) -> Result<()> {
    let amount = parse_amount(amount_str)?;
    
    if amount <= Decimal::ZERO {
        bail!("Withdrawal amount must be positive");
    }

    let mut bankroll = db.get_bankroll().await?;

    if amount > bankroll.available_balance() {
        bail!("Insufficient available balance. Available: ${:.2}", bankroll.available_balance());
    }

    let new_balance = bankroll.balance - amount;

    // Create transaction
    let transaction = Transaction {
        id: 0,
        transaction_type: TransactionType::Withdrawal,
        amount: -amount, // Negative for withdrawals
        balance_after: new_balance,
        bet_id: None,
        description: format!("Withdrawal ${:.2}", amount),
        created_at: chrono::Utc::now(),
    };

    // Update bankroll
    bankroll.balance = new_balance;
    bankroll.total_withdrawn += amount;
    bankroll.updated_at = chrono::Utc::now();

    db.update_bankroll(&bankroll).await?;
    db.add_transaction(&transaction).await?;

    println!("✓ Withdrew ${:.2}", amount);
    println!("  New balance: ${:.2}", new_balance);

    Ok(())
}

async fn show_history(db: &Database, limit: usize) -> Result<()> {
    let transactions = db.get_transactions(limit).await?;

    if transactions.is_empty() {
        println!("No transactions yet.");
        return Ok(());
    }

    println!("Recent Transactions\n");
    println!("{:=<60}", "");

    for txn in transactions {
        let type_icon = match txn.transaction_type {
            TransactionType::Deposit => "⬆️",
            TransactionType::Withdrawal => "⬇️",
            TransactionType::BetPlaced => "🎲",
            TransactionType::BetWon => "✓",
            TransactionType::BetPush => "↩️",
            TransactionType::Adjustment => "📝",
        };

        let amount_str = if txn.amount >= Decimal::ZERO {
            format!("+${:.2}", txn.amount)
        } else {
            format!("-${:.2}", txn.amount.abs())
        };

        println!("  {} {} {:>10} │ Balance: ${:.2}",
            txn.created_at.format("%m/%d %H:%M"),
            type_icon,
            amount_str,
            txn.balance_after
        );
        println!("    {}", txn.description);
    }

    Ok(())
}

fn parse_amount(s: &str) -> Result<Decimal> {
    let cleaned = s.trim_start_matches('$').replace(',', "");
    Decimal::from_str(&cleaned)
        .map_err(|_| anyhow::anyhow!("Invalid amount: {}", s))
}
