//! Bets repository implementation

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use std::str::FromStr;

use crate::{Database, DbError};
use mlb_core::models::{Bet, BetStatus, MarketType};
use mlb_core::repository::{BetRepository, RepoResult};

impl BetRepository for Database {
    async fn save_bet(&self, bet: &Bet) -> RepoResult<i64> {
        save_bet_impl(self, bet).await.map_err(Into::into)
    }

    async fn get_bet(&self, id: i64) -> RepoResult<Bet> {
        get_bet_impl(self, id).await.map_err(Into::into)
    }

    async fn get_bets_by_status(&self, status: BetStatus) -> RepoResult<Vec<Bet>> {
        get_bets_by_status_impl(self, status).await.map_err(Into::into)
    }

    async fn get_bets_for_event(&self, event_id: &str) -> RepoResult<Vec<Bet>> {
        get_bets_for_event_impl(self, event_id).await.map_err(Into::into)
    }

    async fn update_bet_status(&self, id: i64, status: BetStatus, payout: Option<Decimal>) -> RepoResult<()> {
        update_bet_status_impl(self, id, status, payout).await.map_err(Into::into)
    }
}

async fn save_bet_impl(db: &Database, bet: &Bet) -> Result<i64, DbError> {
    let result = sqlx::query(
        r#"
        INSERT INTO bets (event_id, bookmaker, market, selection, point, odds, stake, potential_payout, status, actual_payout, placed_at, settled_at, notes)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&bet.event_id)
    .bind(&bet.bookmaker)
    .bind(market_to_str(bet.market))
    .bind(&bet.selection)
    .bind(bet.point.map(|p| p.to_string()))
    .bind(bet.odds.to_string())
    .bind(bet.stake.to_string())
    .bind(bet.potential_payout.to_string())
    .bind(status_to_str(bet.status))
    .bind(bet.actual_payout.map(|p| p.to_string()))
    .bind(bet.placed_at.to_rfc3339())
    .bind(bet.settled_at.map(|dt| dt.to_rfc3339()))
    .bind(&bet.notes)
    .execute(db.pool())
    .await?;

    Ok(result.last_insert_rowid())
}

async fn get_bet_impl(db: &Database, id: i64) -> Result<Bet, DbError> {
    let row = sqlx::query(
        r#"
        SELECT id, event_id, bookmaker, market, selection, point, odds, stake, potential_payout, status, actual_payout, placed_at, settled_at, notes
        FROM bets
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?
    .ok_or_else(|| DbError::NotFound(format!("Bet {}", id)))?;

    parse_bet_row(row)
}

async fn get_bets_by_status_impl(db: &Database, status: BetStatus) -> Result<Vec<Bet>, DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, event_id, bookmaker, market, selection, point, odds, stake, potential_payout, status, actual_payout, placed_at, settled_at, notes
        FROM bets
        WHERE status = ?
        ORDER BY placed_at DESC
        "#,
    )
    .bind(status_to_str(status))
    .fetch_all(db.pool())
    .await?;

    rows.into_iter().map(parse_bet_row).collect()
}

async fn get_bets_for_event_impl(db: &Database, event_id: &str) -> Result<Vec<Bet>, DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, event_id, bookmaker, market, selection, point, odds, stake, potential_payout, status, actual_payout, placed_at, settled_at, notes
        FROM bets
        WHERE event_id = ?
        ORDER BY placed_at DESC
        "#,
    )
    .bind(event_id)
    .fetch_all(db.pool())
    .await?;

    rows.into_iter().map(parse_bet_row).collect()
}

async fn update_bet_status_impl(db: &Database, id: i64, status: BetStatus, payout: Option<Decimal>) -> Result<(), DbError> {
    let settled_at = if matches!(status, BetStatus::Won | BetStatus::Lost | BetStatus::Push | BetStatus::Void) {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };

    sqlx::query(
        r#"
        UPDATE bets
        SET status = ?, actual_payout = ?, settled_at = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(status_to_str(status))
    .bind(payout.map(|p| p.to_string()))
    .bind(settled_at)
    .bind(id)
    .execute(db.pool())
    .await?;

    Ok(())
}

fn parse_bet_row(row: sqlx::sqlite::SqliteRow) -> Result<Bet, DbError> {
    Ok(Bet {
        id: row.get("id"),
        event_id: row.get("event_id"),
        bookmaker: row.get("bookmaker"),
        market: str_to_market(row.get("market")),
        selection: row.get("selection"),
        point: row.get::<Option<String>, _>("point").and_then(|s| Decimal::from_str(&s).ok()),
        odds: Decimal::from_str(row.get::<&str, _>("odds")).unwrap_or_default(),
        stake: Decimal::from_str(row.get::<&str, _>("stake")).unwrap_or_default(),
        potential_payout: Decimal::from_str(row.get::<&str, _>("potential_payout")).unwrap_or_default(),
        status: str_to_status(row.get("status")),
        actual_payout: row.get::<Option<String>, _>("actual_payout").and_then(|s| Decimal::from_str(&s).ok()),
        placed_at: parse_datetime(row.get("placed_at")),
        settled_at: row.get::<Option<String>, _>("settled_at").map(|s| parse_datetime(s)),
        notes: row.get("notes"),
    })
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn status_to_str(status: BetStatus) -> &'static str {
    match status {
        BetStatus::Pending => "pending",
        BetStatus::Won => "won",
        BetStatus::Lost => "lost",
        BetStatus::Push => "push",
        BetStatus::Void => "void",
    }
}

fn str_to_status(s: &str) -> BetStatus {
    match s {
        "won" => BetStatus::Won,
        "lost" => BetStatus::Lost,
        "push" => BetStatus::Push,
        "void" => BetStatus::Void,
        _ => BetStatus::Pending,
    }
}

fn market_to_str(market: MarketType) -> &'static str {
    market.as_api_key()
}

fn str_to_market(s: &str) -> MarketType {
    match s {
        "spreads" => MarketType::Spreads,
        "totals" => MarketType::Totals,
        "outrights" => MarketType::Outrights,
        "batter_hits" => MarketType::BatterHits,
        "batter_home_runs" => MarketType::BatterHomeRuns,
        "batter_rbis" => MarketType::BatterRbis,
        "batter_stolen_bases" => MarketType::BatterStolenBases,
        "batter_strikeouts" => MarketType::BatterStrikeouts,
        "batter_walks" => MarketType::BatterWalks,
        "pitcher_strikeouts" => MarketType::PitcherStrikeouts,
        "pitcher_hits_allowed" => MarketType::PitcherHitsAllowed,
        "pitcher_walks" => MarketType::PitcherWalks,
        "pitcher_earned_runs" => MarketType::PitcherEarnedRuns,
        _ => MarketType::H2h,
    }
}
