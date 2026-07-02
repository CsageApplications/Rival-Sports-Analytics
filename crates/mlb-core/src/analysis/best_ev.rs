//! Best Expected Value scanner
//!
//! For every two-sided line (moneyline, run line, total, or player prop
//! over/under) offered by more than one bookmaker, this module de-vigs each
//! bookmaker's price independently and averages the resulting probabilities
//! into a single **consensus fair probability** — a simple ensemble estimate
//! of the "true" win probability that regresses individual bookmaker bias
//! toward the multi-book average. Every bookmaker's actual price is then
//! re-evaluated against that consensus to surface the highest edge bets
//! across the whole slate.

use rust_decimal::Decimal;
use std::collections::HashMap;

use super::expected_value::{calculate_ev, calculate_no_vig_probability};
use crate::models::{EventOdds, MarketType, Outcome};

/// A single ranked best-EV betting opportunity.
#[derive(Debug, Clone)]
pub struct BestEvBet {
    pub event_id: String,
    pub market: MarketType,
    /// "Over" / "Under" / team name, depending on market
    pub outcome_name: String,
    /// Player name for prop markets
    pub player: Option<String>,
    /// Line/point for spreads, totals, and props
    pub point: Option<Decimal>,
    pub bookmaker_key: String,
    pub bookmaker_title: String,
    /// American odds price
    pub odds: Decimal,
    /// Consensus (multi-book, de-vigged) true probability estimate
    pub true_probability: Decimal,
    pub implied_probability: Decimal,
    pub edge: Decimal,
    /// Expected value as a percentage of stake
    pub ev_pct: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PairKey {
    market: MarketType,
    player: Option<String>,
    point_key: Option<String>,
}

struct BookSide<'a> {
    bookmaker_key: String,
    bookmaker_title: String,
    outcome: &'a Outcome,
}

/// Scan a single event's odds for best-EV opportunities using the
/// multi-bookmaker consensus de-vig model described above.
///
/// Results are sorted by descending expected value.
pub fn scan_best_ev(event_id: &str, odds: &EventOdds) -> Vec<BestEvBet> {
    let mut groups: HashMap<PairKey, HashMap<String, Vec<BookSide>>> = HashMap::new();

    for book in &odds.bookmakers {
        for market in &book.markets {
            if market.outcomes.len() < 2 {
                continue;
            }

            if market.key.is_player_prop() {
                // Player props: pair Over/Under outcomes that share the same
                // player + point line within this bookmaker's own market.
                let mut by_player_point: HashMap<(String, String), Vec<&Outcome>> = HashMap::new();
                for o in &market.outcomes {
                    let player = o.description.clone().unwrap_or_default();
                    let point = o.point.map(|p| p.to_string()).unwrap_or_default();
                    by_player_point.entry((player, point)).or_default().push(o);
                }

                for ((player, point), outs) in by_player_point {
                    if outs.len() != 2 {
                        continue;
                    }
                    let key = PairKey {
                        market: market.key,
                        player: Some(player),
                        point_key: Some(point),
                    };
                    for o in outs {
                        groups
                            .entry(key.clone())
                            .or_default()
                            .entry(o.name.clone())
                            .or_default()
                            .push(BookSide {
                                bookmaker_key: book.key.clone(),
                                bookmaker_title: book.title.clone(),
                                outcome: o,
                            });
                    }
                }
            } else {
                // Moneyline / run line / totals: build a canonical signature
                // for this bookmaker's exact line — every side's name AND
                // point, sorted deterministically. This ensures we only ever
                // average/compare bookmakers that are quoting the *identical*
                // line. If two books disagree about which side is favored
                // (e.g. FanDuel has Team A -1.5 while DraftKings has Team B
                // -1.5), they are fundamentally different, mutually
                // exclusive bets — not complementary sides of the same
                // wager — so they must never be pooled into one consensus.
                let mut sig_parts: Vec<String> = market
                    .outcomes
                    .iter()
                    .map(|o| {
                        format!(
                            "{}@{}",
                            o.name,
                            o.point.map(|p| p.to_string()).unwrap_or_default()
                        )
                    })
                    .collect();
                sig_parts.sort();
                let signature = sig_parts.join("|");

                let key = PairKey {
                    market: market.key,
                    player: None,
                    point_key: Some(signature),
                };
                for o in &market.outcomes {
                    groups
                        .entry(key.clone())
                        .or_default()
                        .entry(o.name.clone())
                        .or_default()
                        .push(BookSide {
                            bookmaker_key: book.key.clone(),
                            bookmaker_title: book.title.clone(),
                            outcome: o,
                        });
                }
            }
        }
    }

    let mut results = Vec::new();

    for (key, sides) in &groups {
        if sides.len() != 2 {
            continue;
        }

        let mut side_names: Vec<&String> = sides.keys().collect();
        side_names.sort();
        let name_a = side_names[0];
        let name_b = side_names[1];
        let books_a = &sides[name_a];
        let books_b = &sides[name_b];

        // Consensus: average the no-vig probability across every bookmaker
        // that quotes both sides of this exact line.
        let mut probs_a = Vec::new();
        let mut probs_b = Vec::new();

        for a in books_a {
            if let Some(b) = books_b.iter().find(|b| b.bookmaker_key == a.bookmaker_key) {
                let (pa, pb) = calculate_no_vig_probability(a.outcome, b.outcome);
                probs_a.push(pa);
                probs_b.push(pb);
            }
        }

        if probs_a.is_empty() {
            continue;
        }

        let consensus_a = average(&probs_a);
        let consensus_b = average(&probs_b);

        emit_side(event_id, &key, name_a, consensus_a, books_a, &mut results);
        emit_side(event_id, &key, name_b, consensus_b, books_b, &mut results);
    }

    results.sort_by(|a, b| b.ev_pct.cmp(&a.ev_pct));
    results
}

fn average(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }
    let sum: Decimal = values.iter().sum();
    sum / Decimal::from(values.len() as i64)
}

fn emit_side(
    event_id: &str,
    key: &PairKey,
    side_name: &str,
    consensus_prob: Decimal,
    books: &[BookSide],
    out: &mut Vec<BestEvBet>,
) {
    for b in books {
        let ev_result = calculate_ev(consensus_prob, b.outcome);
        out.push(BestEvBet {
            event_id: event_id.to_string(),
            market: key.market,
            outcome_name: side_name.to_string(),
            player: key.player.clone(),
            point: b.outcome.point,
            bookmaker_key: b.bookmaker_key.clone(),
            bookmaker_title: b.bookmaker_title.clone(),
            odds: b.outcome.price,
            true_probability: consensus_prob,
            implied_probability: ev_result.implied_probability,
            edge: ev_result.edge,
            ev_pct: ev_result.ev * Decimal::from(100),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bookmaker, Market};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn make_odds() -> EventOdds {
        EventOdds {
            event_id: "evt1".into(),
            captured_at: Utc::now(),
            bookmakers: vec![
                Bookmaker {
                    key: "fanduel".into(),
                    title: "FanDuel".into(),
                    markets: vec![Market {
                        key: MarketType::H2h,
                        last_update: Utc::now(),
                        outcomes: vec![
                            Outcome::new("Home", dec!(-120), None),
                            Outcome::new("Away", dec!(110), None),
                        ],
                    }],
                },
                Bookmaker {
                    key: "draftkings".into(),
                    title: "DraftKings".into(),
                    markets: vec![Market {
                        key: MarketType::H2h,
                        last_update: Utc::now(),
                        outcomes: vec![
                            Outcome::new("Home", dec!(-105), None),
                            Outcome::new("Away", dec!(100), None),
                        ],
                    }],
                },
            ],
        }
    }

    #[test]
    fn finds_best_ev_across_books() {
        let odds = make_odds();
        let results = scan_best_ev("evt1", &odds);
        assert_eq!(results.len(), 4);
        // Highest EV should be sorted first
        assert!(results[0].ev_pct >= results[1].ev_pct);
    }
}
