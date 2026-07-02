//! Arbitrage detection

use rust_decimal::Decimal;
use std::collections::HashMap;
use crate::models::{EventOdds, MarketType, Outcome};

/// An arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    /// Event ID
    pub event_id: String,
    /// Market type (usually h2h)
    pub market: MarketType,
    /// The two legs of the arbitrage
    pub legs: Vec<ArbLeg>,
    /// Guaranteed profit percentage
    pub profit_percentage: Decimal,
    /// Total implied probability (will be < 1 for arb)
    pub total_implied: Decimal,
}

/// One leg of an arbitrage bet
#[derive(Debug, Clone)]
pub struct ArbLeg {
    pub bookmaker: String,
    pub outcome: String,
    pub odds: Decimal,
    /// Stake as percentage of total bankroll
    pub stake_percentage: Decimal,
}

/// Check if arbitrage exists in a two-way market
///
/// Arbitrage exists when: 1/odds1 + 1/odds2 < 1
/// (using decimal odds)
pub fn find_two_way_arbitrage(
    event_id: &str,
    market: MarketType,
    outcome1_best: (&str, &Outcome), // (bookmaker, outcome)
    outcome2_best: (&str, &Outcome),
) -> Option<ArbitrageOpportunity> {
    let (book1, out1) = outcome1_best;
    let (book2, out2) = outcome2_best;

    let decimal1 = out1.decimal_odds();
    let decimal2 = out2.decimal_odds();

    let implied1 = Decimal::ONE / decimal1;
    let implied2 = Decimal::ONE / decimal2;
    let total_implied = implied1 + implied2;

    // Arbitrage exists if total implied probability < 100%
    if total_implied >= Decimal::ONE {
        return None;
    }

    // Calculate profit percentage
    let profit_percentage = (Decimal::ONE / total_implied) - Decimal::ONE;

    // Calculate optimal stakes
    // stake1 = implied1 / total_implied
    let stake1 = implied1 / total_implied;
    let stake2 = implied2 / total_implied;

    Some(ArbitrageOpportunity {
        event_id: event_id.to_string(),
        market,
        legs: vec![
            ArbLeg {
                bookmaker: book1.to_string(),
                outcome: out1.name.clone(),
                odds: out1.price,
                stake_percentage: stake1,
            },
            ArbLeg {
                bookmaker: book2.to_string(),
                outcome: out2.name.clone(),
                odds: out2.price,
                stake_percentage: stake2,
            },
        ],
        profit_percentage,
        total_implied,
    })
}

/// Scan event odds for arbitrage opportunities
///
/// Checks moneyline (h2h) across all bookmakers, plus run line (spreads) and
/// totals — but for spreads/totals, only outcomes that share the *exact
/// same line* across books are ever paired together. Two books disagreeing
/// on which side is favored (e.g. Team A -1.5 at one book vs Team B -1.5 at
/// another) are NOT complementary bets and can never form real arbitrage,
/// so they are correctly excluded.
pub fn scan_for_arbitrage(event_odds: &EventOdds) -> Vec<ArbitrageOpportunity> {
    let mut opportunities = Vec::new();

    // Moneyline (h2h): always a single line, safe to compare best-of across
    // all bookmakers directly.
    if let Some((book1, out1)) = find_best_odds_for_outcome(event_odds, MarketType::H2h, |_| true) {
        if let Some((book2, out2)) = find_best_odds_for_outcome(event_odds, MarketType::H2h, |o| o.name != out1.name) {
            if let Some(arb) = find_two_way_arbitrage(
                &event_odds.event_id,
                MarketType::H2h,
                (book1, out1),
                (book2, out2),
            ) {
                opportunities.push(arb);
            }
        }
    }

    // Spreads and totals: group every bookmaker's outcomes by their exact
    // line signature (all side names + points, sorted) so we only compare
    // bookmakers quoting the identical line, then take the best price for
    // each side within that matching group.
    for market_type in [MarketType::Spreads, MarketType::Totals] {
        let mut groups: HashMap<String, HashMap<String, (&str, &Outcome)>> = HashMap::new();

        for bookmaker in &event_odds.bookmakers {
            if let Some(market) = bookmaker.get_market(market_type) {
                if market.outcomes.len() != 2 {
                    continue;
                }

                let mut sig_parts: Vec<String> = market
                    .outcomes
                    .iter()
                    .map(|o| format!("{}@{}", o.name, o.point.map(|p| p.to_string()).unwrap_or_default()))
                    .collect();
                sig_parts.sort();
                let signature = sig_parts.join("|");

                let sides = groups.entry(signature).or_default();
                for outcome in &market.outcomes {
                    match sides.get(&outcome.name) {
                        Some((_, current)) if outcome.decimal_odds() <= current.decimal_odds() => {}
                        _ => {
                            sides.insert(outcome.name.clone(), (&bookmaker.key, outcome));
                        }
                    }
                }
            }
        }

        for sides in groups.values() {
            if sides.len() != 2 {
                continue;
            }
            let mut entries: Vec<_> = sides.values().collect();
            entries.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            let (book1, out1) = entries[0];
            let (book2, out2) = entries[1];

            if let Some(arb) = find_two_way_arbitrage(
                &event_odds.event_id,
                market_type,
                (book1, out1),
                (book2, out2),
            ) {
                opportunities.push(arb);
            }
        }
    }

    opportunities
}

/// Find the best odds for any outcome matching a predicate
fn find_best_odds_for_outcome<'a, F>(
    event_odds: &'a EventOdds,
    market_type: MarketType,
    predicate: F,
) -> Option<(&'a str, &'a Outcome)>
where
    F: Fn(&Outcome) -> bool,
{
    let mut best: Option<(&str, &Outcome)> = None;

    for bookmaker in &event_odds.bookmakers {
        if let Some(market) = bookmaker.get_market(market_type) {
            for outcome in &market.outcomes {
                if predicate(outcome) {
                    match &best {
                        None => best = Some((&bookmaker.key, outcome)),
                        Some((_, current)) => {
                            if outcome.decimal_odds() > current.decimal_odds() {
                                best = Some((&bookmaker.key, outcome));
                            }
                        }
                    }
                }
            }
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_arbitrage_exists() {
        let out1 = Outcome::new("Team A", dec!(200), None); // 2.0 decimal = 50%
        let out2 = Outcome::new("Team B", dec!(220), None); // 2.2 decimal = 45.5%
        // Total = 95.5% = arbitrage!

        let arb = find_two_way_arbitrage(
            "event1",
            MarketType::H2h,
            ("book1", &out1),
            ("book2", &out2),
        );

        assert!(arb.is_some());
        let arb = arb.unwrap();
        assert!(arb.profit_percentage > Decimal::ZERO);
    }

    #[test]
    fn test_no_arbitrage() {
        let out1 = Outcome::new("Team A", dec!(-110), None); // ~52.4%
        let out2 = Outcome::new("Team B", dec!(-110), None); // ~52.4%
        // Total = 104.8% = no arbitrage

        let arb = find_two_way_arbitrage(
            "event1",
            MarketType::H2h,
            ("book1", &out1),
            ("book2", &out2),
        );

        assert!(arb.is_none());
    }
}
