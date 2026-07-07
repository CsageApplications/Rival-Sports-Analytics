//! Pre-game win-probability projections using published sabermetric
//! formulas — **not** a black-box machine-learning model. Two well-known
//! methods are combined:
//!
//! 1. **Pythagorean win expectation** ("Pythagenpat", exponent 1.83) —
//!    estimates a team's "true" winning percentage from its season runs
//!    scored/allowed, which is a more stable signal than raw win-loss
//!    record over a partial season.
//! 2. **Log5** (Bill James) — combines two teams' win percentages into a
//!    single head-to-head win probability for the team on the left.
//!
//! A small additive home-field-advantage nudge is applied on top, per
//! the commonly-cited MLB home win rate of ~54%.
//!
//! The resulting model probability is then compared against the
//! bookmakers' de-vigged ("fair") moneyline probability to surface an
//! edge, reusing the existing no-vig calculation from `mlb-core`.

use mlb_core::analysis::calculate_no_vig_probability;
use mlb_core::models::{EventOdds, MarketType};
use mlb_stats_client::TeamStanding;
use rust_decimal::prelude::ToPrimitive;

/// Pythagenpat exponent. 2.0 is the classic Bill James constant; 1.83 is
/// a commonly used refinement that fits actual MLB outcomes slightly
/// better across a full season.
pub const PYTHAGOREAN_EXPONENT: f64 = 1.83;

/// Additive nudge applied to the home team's Log5 win probability,
/// reflecting MLB's historical ~54% home win rate.
pub const HOME_FIELD_ADVANTAGE: f64 = 0.04;

/// Estimates a team's "true" winning percentage from season runs
/// scored/allowed. Returns `None` if both are zero (no data yet, e.g.
/// very early season).
pub fn pythagorean_win_pct(runs_scored: f64, runs_allowed: f64) -> Option<f64> {
    if runs_scored <= 0.0 && runs_allowed <= 0.0 {
        return None;
    }
    let rs = runs_scored.max(0.0).powf(PYTHAGOREAN_EXPONENT);
    let ra = runs_allowed.max(0.0).powf(PYTHAGOREAN_EXPONENT);
    let denom = rs + ra;
    if denom <= 0.0 {
        return None;
    }
    Some(rs / denom)
}

/// Bill James' Log5 formula: given team A's and team B's independent win
/// percentages, returns team A's probability of beating team B head to
/// head. Falls back to 0.5 if the formula is undefined (both teams at
/// 0% or both at 100%).
pub fn log5(prob_a: f64, prob_b: f64) -> f64 {
    let denom = prob_a + prob_b - 2.0 * prob_a * prob_b;
    if denom.abs() < f64::EPSILON {
        return 0.5;
    }
    let raw = (prob_a - prob_a * prob_b) / denom;
    raw.clamp(0.0, 1.0)
}

/// A single game's model-projected win probabilities for both sides.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameProjection {
    pub home_team: String,
    pub away_team: String,
    /// Final projected win probability, including home-field advantage.
    pub home_win_pct: f64,
    pub away_win_pct: f64,
    /// The underlying Pythagorean win percentages used as Log5 inputs,
    /// surfaced for transparency/debugging.
    pub home_pythag_pct: f64,
    pub away_pythag_pct: f64,
}

/// Projects a game's win probabilities from each team's standings.
/// Prefers Pythagorean win expectation (runs scored/allowed); falls back
/// to the team's actual season win percentage if run data isn't
/// available yet.
pub fn project_game(home: &TeamStanding, away: &TeamStanding) -> GameProjection {
    let home_pythag = home
        .runs_scored
        .zip(home.runs_allowed)
        .and_then(|(rs, ra)| pythagorean_win_pct(rs as f64, ra as f64))
        .unwrap_or(home.win_pct);
    let away_pythag = away
        .runs_scored
        .zip(away.runs_allowed)
        .and_then(|(rs, ra)| pythagorean_win_pct(rs as f64, ra as f64))
        .unwrap_or(away.win_pct);

    let raw_home_win = log5(home_pythag, away_pythag);
    let home_win_pct = (raw_home_win + HOME_FIELD_ADVANTAGE).clamp(0.01, 0.99);
    let away_win_pct = 1.0 - home_win_pct;

    GameProjection {
        home_team: home.team_name.clone(),
        away_team: away.team_name.clone(),
        home_win_pct,
        away_win_pct,
        home_pythag_pct: home_pythag,
        away_pythag_pct: away_pythag,
    }
}

/// Model projection compared against the market's de-vigged moneyline
/// probability, in percentage-point edge terms (model minus market).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameEdge {
    pub home_team: String,
    pub away_team: String,
    pub model_home_win_pct: f64,
    pub model_away_win_pct: f64,
    pub market_home_fair_pct: f64,
    pub market_away_fair_pct: f64,
    /// Percentage points: model − market, for the home side.
    pub home_edge_pct: f64,
    /// Percentage points: model − market, for the away side.
    pub away_edge_pct: f64,
}

/// Combines a [`GameProjection`] with the slate's moneyline odds to
/// compute an edge over the market. Returns `None` if no bookmaker
/// offers a two-sided H2H market naming both teams (e.g. odds not
/// posted yet).
pub fn compute_game_edge(home_team: &str, away_team: &str, home: &TeamStanding, away: &TeamStanding, odds: &EventOdds) -> Option<GameEdge> {
    let projection = project_game(home, away);

    for book in &odds.bookmakers {
        let Some(market) = book.markets.iter().find(|m| m.key == MarketType::H2h) else {
            continue;
        };
        let home_outcome = market.outcomes.iter().find(|o| o.name == home_team);
        let away_outcome = market.outcomes.iter().find(|o| o.name == away_team);
        if let (Some(home_o), Some(away_o)) = (home_outcome, away_outcome) {
            let (fair_home, fair_away) = calculate_no_vig_probability(home_o, away_o);
            let market_home_fair_pct: f64 = fair_home.to_f64().unwrap_or(0.5);
            let market_away_fair_pct: f64 = fair_away.to_f64().unwrap_or(0.5);

            return Some(GameEdge {
                home_team: home_team.to_string(),
                away_team: away_team.to_string(),
                model_home_win_pct: projection.home_win_pct,
                model_away_win_pct: projection.away_win_pct,
                market_home_fair_pct,
                market_away_fair_pct,
                home_edge_pct: (projection.home_win_pct - market_home_fair_pct) * 100.0,
                away_edge_pct: (projection.away_win_pct - market_away_fair_pct) * 100.0,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing(name: &str, wins: i32, losses: i32, rs: i32, ra: i32) -> TeamStanding {
        TeamStanding {
            team_id: 1,
            team_name: name.to_string(),
            wins,
            losses,
            win_pct: wins as f64 / (wins + losses).max(1) as f64,
            runs_scored: Some(rs),
            runs_allowed: Some(ra),
        }
    }

    #[test]
    fn better_run_differential_favors_home_team() {
        let home = standing("Dodgers", 90, 60, 750, 550);
        let away = standing("Rockies", 60, 90, 550, 750);
        let projection = project_game(&home, &away);
        assert!(projection.home_win_pct > 0.7, "expected strong home favorite, got {}", projection.home_win_pct);
        assert!(projection.home_win_pct + projection.away_win_pct - 1.0 < 1e-9);
    }

    #[test]
    fn evenly_matched_teams_favor_home_via_hfa() {
        let home = standing("Team A", 81, 81, 700, 700);
        let away = standing("Team B", 81, 81, 700, 700);
        let projection = project_game(&home, &away);
        // Log5 of two identical teams is exactly 0.5, so the only edge
        // should be the home-field-advantage constant.
        assert!((projection.home_win_pct - (0.5 + HOME_FIELD_ADVANTAGE)).abs() < 1e-9);
    }

    #[test]
    fn missing_runs_data_falls_back_to_win_pct() {
        let home = TeamStanding {
            team_id: 1,
            team_name: "Home".to_string(),
            wins: 60,
            losses: 40,
            win_pct: 0.6,
            runs_scored: None,
            runs_allowed: None,
        };
        let away = TeamStanding {
            team_id: 2,
            team_name: "Away".to_string(),
            wins: 40,
            losses: 60,
            win_pct: 0.4,
            runs_scored: None,
            runs_allowed: None,
        };
        let projection = project_game(&home, &away);
        assert_eq!(projection.home_pythag_pct, 0.6);
        assert_eq!(projection.away_pythag_pct, 0.4);
        assert!(projection.home_win_pct > 0.5);
    }
}
