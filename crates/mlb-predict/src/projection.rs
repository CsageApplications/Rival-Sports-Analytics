//! Recent-form player-prop projections.
//!
//! Independent of any second bookmaker, this module answers "how many
//! [hits/strikeouts/etc.] does this player's recent form suggest for
//! tonight?" by averaging their last several games' raw counting stat,
//! then converts that projection into a model-implied Over/Under
//! probability via a Poisson count model (the standard approach for
//! low-count discrete sports stats like hits, home runs, strikeouts).
//!
//! This lets every player-prop line get a model-driven edge estimate —
//! including the (very common) case where only one bookmaker quotes a
//! given line and there's no cross-book consensus to compare against.
//!
//! Example: a pitcher who has struck out 8, 10, 9, 7, 11 batters in his
//! last 5 starts projects to `lambda = 9.0`. Against a posted Over/Under
//! of 5.5, the Poisson model says the Over is priced far too generously
//! — this is the quantitative version of "the model predicts 9, so we
//! should bet the Over 5.5."

use serde::{Deserialize, Serialize};

use mlb_core::models::MarketType;
use mlb_db::Database;
use mlb_stats_client::GameLogEntry;

use crate::report::get_cached_report;
use crate::summary::stat_group_for_market;

/// How many of the player's most recent games (in the relevant stat
/// group) to average for the projection. Recent form is a materially
/// better predictor of "tonight" than a full-season rate, especially for
/// usage-sensitive counting stats like strikeouts or hits.
pub const RECENT_GAMES_WINDOW: usize = 10;
/// Minimum number of recent games required before a projection is
/// trusted; below this, results are too noisy to act on.
pub const MIN_GAMES_FOR_PROJECTION: usize = 3;

/// Maps a player-prop market to the extractor that pulls the raw
/// per-game count (NOT a rate) out of a single game log entry.
fn recent_count_extractor(market: MarketType) -> Option<fn(&GameLogEntry) -> Option<i32>> {
    use MarketType::*;
    match market {
        BatterHits => Some(|g| g.hits),
        BatterHomeRuns => Some(|g| g.home_runs),
        BatterRbis => Some(|g| g.rbi),
        BatterStolenBases => Some(|g| g.stolen_bases),
        BatterStrikeouts => Some(|g| g.batter_strikeouts),
        BatterWalks => Some(|g| g.walks_drawn),
        PitcherStrikeouts => Some(|g| g.strikeouts),
        PitcherHitsAllowed => Some(|g| g.hits_allowed),
        PitcherWalks => Some(|g| g.walks),
        PitcherEarnedRuns => Some(|g| g.earned_runs),
        H2h | Spreads | Totals | Outrights => None,
    }
}

/// Projects a player's expected count for `market` tonight as the simple
/// average of their most recent games (most-recent-first `games`).
/// Returns `(projected_mean, sample_size)`, or `None` if there isn't
/// enough recent data or the market has no projectable stat.
pub fn project_recent_average(games: &[GameLogEntry], market: MarketType) -> Option<(f64, usize)> {
    let extractor = recent_count_extractor(market)?;
    let values: Vec<f64> = games.iter().filter_map(|g| extractor(g)).map(|v| v as f64).take(RECENT_GAMES_WINDOW).collect();
    if values.len() < MIN_GAMES_FOR_PROJECTION {
        return None;
    }
    let n = values.len();
    Some((values.iter().sum::<f64>() / n as f64, n))
}

/// P(X > point) under a Poisson(lambda) count model, for a half-integer
/// prop line (e.g. 1.5, 5.5) where a push is impossible.
fn poisson_over_probability(lambda: f64, point: f64) -> f64 {
    if lambda <= 0.0 {
        return 0.0;
    }
    let k = point.floor(); // e.g. 5.5 -> 5.0; Over means X >= 6
    if k < 0.0 {
        return 1.0;
    }
    let k = k as i64;
    let mut term = (-lambda).exp(); // P(X = 0)
    let mut cdf = term;
    let mut i: i64 = 1;
    while i <= k {
        term *= lambda / i as f64;
        cdf += term;
        i += 1;
    }
    (1.0 - cdf).clamp(0.0, 1.0)
}

/// A player's recent-form projection for one prop line.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PropProjection {
    /// Projected expected count for tonight (recent-games average).
    pub projected_value: f64,
    /// How many recent games this projection is based on.
    pub sample_games: usize,
    /// Model-implied probability the Over hits, under a Poisson model.
    pub over_probability: f64,
    pub under_probability: f64,
}

/// Builds a `PropProjection` from a player's recent game log for a
/// specific market + line.
pub fn build_prop_projection(games: &[GameLogEntry], market: MarketType, point: f64) -> Option<PropProjection> {
    let (lambda, sample_games) = project_recent_average(games, market)?;
    let over_probability = poisson_over_probability(lambda, point);
    Some(PropProjection {
        projected_value: lambda,
        sample_games,
        over_probability,
        under_probability: 1.0 - over_probability,
    })
}

/// Convenience wrapper: looks up the player's cached recent-games report
/// (never hits statsapi.mlb.com — cache-only, safe for exports/chat) and
/// builds their projection for `market` at `point`. Returns `None` if the
/// player isn't cached yet, the market isn't projectable, or there's
/// insufficient recent-game data.
pub async fn project_prop(db: &Database, player_name: &str, market: MarketType, point: f64) -> Option<PropProjection> {
    let group = stat_group_for_market(market)?;
    let report = get_cached_report(db, player_name, group, None).await.ok().flatten()?;
    build_prop_projection(&report.recent_games, market, point)
}

fn implied_probability_from_american(price: f64) -> f64 {
    if price >= 0.0 {
        100.0 / (price + 100.0)
    } else {
        price.abs() / (price.abs() + 100.0)
    }
}

fn decimal_odds_from_american(price: f64) -> f64 {
    if price >= 0.0 {
        price / 100.0 + 1.0
    } else {
        100.0 / price.abs() + 1.0
    }
}

/// Expected value (as a fraction of stake, e.g. 0.05 = +5%) of a bet at
/// `price` (American odds) given a model true-probability estimate.
/// Mirrors `mlb_core::analysis::calculate_ev`'s formula (`P * decimal_odds
/// - 1`), duplicated here in plain f64 since this is a heuristic model
/// estimate rather than the core Decimal-precision consensus EV.
pub fn model_ev_pct(probability: f64, price: f64) -> f64 {
    probability * decimal_odds_from_american(price) - 1.0
}

/// The model's edge over the market's implied probability at `price`
/// (percentage points, e.g. 0.08 = +8pp).
pub fn model_edge(probability: f64, price: f64) -> f64 {
    probability - implied_probability_from_american(price)
}

/// Beyond this model-vs-market probability gap, a real two-sided market
/// disagreeing this much with a simple recent-form count model is far
/// more likely explained by context we don't have (the player isn't
/// actually confirmed in tonight's starting lineup, a role change, a
/// thin/stale line on a low-profile player) than genuine, safely-bettable
/// mispricing. Callers should surface this as a "verify the lineup/role
/// before betting" caution rather than presenting the raw EV number as a
/// confident edge.
pub const HIGH_DIVERGENCE_EDGE_THRESHOLD: f64 = 0.30;
/// On long-shot-priced lines (common for "at least 1" props on bench/
/// part-time players, e.g. +550), even a moderate probability gap
/// leverages into an enormous EV% because of how long American odds
/// payouts scale. A resulting EV this large (50%+) is essentially never
/// a real, safely-bettable edge in a liquid market — flag it the same
/// way as a large raw probability gap.
pub const HIGH_DIVERGENCE_EV_THRESHOLD: f64 = 0.50;

/// Returns `true` if the model's probability estimate diverges from the
/// market's implied probability at `price` by more than
/// `HIGH_DIVERGENCE_EDGE_THRESHOLD` percentage points, OR the resulting
/// EV% exceeds `HIGH_DIVERGENCE_EV_THRESHOLD` — either is a signal to
/// caution the user rather than present the edge at face value.
pub fn is_high_divergence(probability: f64, price: f64) -> bool {
    model_edge(probability, price).abs() > HIGH_DIVERGENCE_EDGE_THRESHOLD
        || model_ev_pct(probability, price).abs() > HIGH_DIVERGENCE_EV_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(field_setter: impl Fn(&mut GameLogEntry)) -> GameLogEntry {
        let mut g = GameLogEntry::default();
        field_setter(&mut g);
        g
    }

    #[test]
    fn projects_recent_average_for_pitcher_strikeouts() {
        let games: Vec<GameLogEntry> = vec![8, 10, 9, 7, 11].into_iter().map(|k| game(|g| g.strikeouts = Some(k))).collect();
        let (mean, n) = project_recent_average(&games, MarketType::PitcherStrikeouts).unwrap();
        assert_eq!(n, 5);
        assert!((mean - 9.0).abs() < 1e-9);
    }

    #[test]
    fn insufficient_games_returns_none() {
        let games: Vec<GameLogEntry> = vec![8, 10].into_iter().map(|k| game(|g| g.strikeouts = Some(k))).collect();
        assert!(project_recent_average(&games, MarketType::PitcherStrikeouts).is_none());
    }

    #[test]
    fn high_projection_favors_over_on_low_line() {
        // Model projects 9.0 Ks; line is 5.5 -> Over should be heavily favored.
        let p = poisson_over_probability(9.0, 5.5);
        assert!(p > 0.8, "expected strong Over probability, got {p}");
    }

    #[test]
    fn low_projection_favors_under_on_high_line() {
        // Model projects 3.0 Ks; line is 7.5 -> Under should be heavily favored.
        let p = poisson_over_probability(3.0, 7.5);
        assert!(p < 0.05, "expected very low Over probability, got {p}");
    }

    #[test]
    fn zero_projection_gives_zero_over_probability() {
        assert_eq!(poisson_over_probability(0.0, 0.5), 0.0);
    }

    #[test]
    fn model_ev_pct_matches_manual_calc() {
        // P=0.9 at +100 (decimal 2.0): EV = 0.9*2.0 - 1 = 0.8
        let ev = model_ev_pct(0.9, 100.0);
        assert!((ev - 0.8).abs() < 1e-9);
    }

    #[test]
    fn extreme_model_vs_market_gap_flagged_high_divergence() {
        // Model says 59% Over, but market prices it at +550 (implied ~15%) —
        // a 44pp gap, well past the 30pp caution threshold.
        assert!(is_high_divergence(0.59, 550.0));
    }

    #[test]
    fn close_model_vs_market_gap_not_flagged() {
        // Model says 55%, market implies ~52% (-108ish) — normal, small gap.
        assert!(!is_high_divergence(0.55, -110.0));
    }

    #[test]
    fn long_shot_price_with_moderate_prob_gap_flagged_via_ev_cap() {
        // Model 39% Over vs a long-shot +601 price (implied ~14%, a 25pp
        // gap under the 30pp threshold) still produces a huge EV% due to
        // odds leverage — must be caught by the EV cap regardless.
        assert!(is_high_divergence(0.393, 601.0));
    }

    #[test]
    fn full_projection_builds_expected_shape() {
        let games: Vec<GameLogEntry> = vec![8, 10, 9, 7, 11].into_iter().map(|k| game(|g| g.strikeouts = Some(k))).collect();
        let proj = build_prop_projection(&games, MarketType::PitcherStrikeouts, 5.5).unwrap();
        assert_eq!(proj.sample_games, 5);
        assert!((proj.projected_value - 9.0).abs() < 1e-9);
        assert!((proj.over_probability + proj.under_probability - 1.0).abs() < 1e-9);
        assert!(proj.over_probability > 0.8);
    }
}
