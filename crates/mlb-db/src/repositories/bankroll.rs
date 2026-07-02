//! Bankroll repository implementation

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use std::str::FromStr;

use crate::{Database, DbError};
use mlb_core::models::{Bankroll, Transaction, TransactionType};
use mlb_core::repository::{BankrollRepository, RepoResult};

impl BankrollRepository for Database {
    async fn get_bankroll(&self) -> RepoResult<Bankroll> {
        get_bankroll_impl(self).await.map_err(Into::into)
    }

    async fn update_bankroll(&self, bankroll: &Bankroll) -> RepoResult<()> {
        update_bankroll_impl(self, bankroll).await.map_err(Into::into)
    }

    async fn add_transaction(&self, transaction: &Transaction) -> RepoResult<i64> {
        add_transaction_impl(self, transaction).await.map_err(Into::into)
    }

    async fn get_transactions(&self, limit: usize) -> RepoResult<Vec<Transaction>> {
        get_transactions_impl(self, limit).await.map_err(Into::into)
    }
}

async fn get_bankroll_impl(db: &Database) -> Result<Bankroll, DbError> {
    let row = sqlx::query(
        r#"
        SELECT balance, total_deposited, total_withdrawn, pending_risk, updated_at
        FROM bankroll
        WHERE id = 1
        "#,
    )
    .fetch_optional(db.pool())
    .await?;

    match row {
        Some(row) => Ok(Bankroll {
            balance: Decimal::from_str(row.get::<&str, _>("balance")).unwrap_or_default(),
            total_deposited: Decimal::from_str(row.get::<&str, _>("total_deposited")).unwrap_or_default(),
            total_withdrawn: Decimal::from_str(row.get::<&str, _>("total_withdrawn")).unwrap_or_default(),
            pending_risk: Decimal::from_str(row.get::<&str, _>("pending_risk")).unwrap_or_default(),
            updated_at: parse_datetime(row.get("updated_at")),
        }),
        None => {
            // Initialize with zero balance
            let bankroll = Bankroll::new(Decimal::ZERO);
            update_bankroll_impl(db, &bankroll).await?;
            Ok(bankroll)
        }
    }
}

async fn update_bankroll_impl(db: &Database, bankroll: &Bankroll) -> Result<(), DbError> {
    sqlx::query(
        r#"
        INSERT INTO bankroll (id, balance, total_deposited, total_withdrawn, pending_risk, updated_at)
        VALUES (1, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
            balance = excluded.balance,
            total_deposited = excluded.total_deposited,
            total_withdrawn = excluded.total_withdrawn,
            pending_risk = excluded.pending_risk,
            updated_at = datetime('now')
        "#,
    )
    .bind(bankroll.balance.to_string())
    .bind(bankroll.total_deposited.to_string())
    .bind(bankroll.total_withdrawn.to_string())
    .bind(bankroll.pending_risk.to_string())
    .execute(db.pool())
    .await?;

    Ok(())
}

async fn add_transaction_impl(db: &Database, transaction: &Transaction) -> Result<i64, DbError> {
    let result = sqlx::query(
        r#"
        INSERT INTO transactions (transaction_type, amount, balance_after, bet_id, description)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(transaction_type_to_str(transaction.transaction_type))
    .bind(transaction.amount.to_string())
    .bind(transaction.balance_after.to_string())
    .bind(transaction.bet_id)
    .bind(&transaction.description)
    .execute(db.pool())
    .await?;

    Ok(result.last_insert_rowid())
}

async fn get_transactions_impl(db: &Database, limit: usize) -> Result<Vec<Transaction>, DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, transaction_type, amount, balance_after, bet_id, description, created_at
        FROM transactions
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit as i64)
    .fetch_all(db.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Transaction {
            id: row.get("id"),
            transaction_type: str_to_transaction_type(row.get("transaction_type")),
            amount: Decimal::from_str(row.get::<&str, _>("amount")).unwrap_or_default(),
            balance_after: Decimal::from_str(row.get::<&str, _>("balance_after")).unwrap_or_default(),
            bet_id: row.get("bet_id"),
            description: row.get("description"),
            created_at: parse_datetime(row.get("created_at")),
        })
        .collect())
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn transaction_type_to_str(t: TransactionType) -> &'static str {
    match t {
        TransactionType::Deposit => "deposit",
        TransactionType::Withdrawal => "withdrawal",
        TransactionType::BetPlaced => "bet_placed",
        TransactionType::BetWon => "bet_won",
        TransactionType::BetPush => "bet_push",
        TransactionType::Adjustment => "adjustment",
    }
}

fn str_to_transaction_type(s: &str) -> TransactionType {
    match s {
        "withdrawal" => TransactionType::Withdrawal,
        "bet_placed" => TransactionType::BetPlaced,
        "bet_won" => TransactionType::BetWon,
        "bet_push" => TransactionType::BetPush,
        "adjustment" => TransactionType::Adjustment,
        _ => TransactionType::Deposit,
    }
}
