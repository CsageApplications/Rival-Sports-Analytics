//! Plausibility bounds for player-prop lines.
//!
//! Betting APIs occasionally surface corrupted or mis-scraped lines from
//! upstream bookmakers (e.g. a pitcher strikeout prop posted at 11.5,
//! when realistic starter lines run roughly 2.5-9.5). Rather than
//! silently trusting any number that successfully parses as a
//! `Decimal`, we bound-check prop points against generous, real-world
//! ranges before they're shown on the dashboard or handed to the LLM as
//! grounding data. This does NOT flag every unusual-but-real line (e.g.
//! an ace projected for 9.5 Ks is fine) — only ranges no sportsbook
//! would realistically post for a single game.

use rust_decimal::Decimal;

use crate::models::MarketType;

/// Returns `true` if `point` falls within a plausible range for `market`.
/// Markets without a defined bound (e.g. moneyline/spreads/totals) always
/// return `true` — this check only applies to player-prop markets.
pub fn is_plausible_prop_point(market: MarketType, point: Decimal) -> bool {
    let Some((min, max)) = plausible_range(market) else {
        return true;
    };
    point >= Decimal::from(min) && point <= Decimal::from(max)
}

/// Inclusive (min, max) whole-number bounds per player-prop market.
/// Lines are generally quoted at X.5, so integer bounds comfortably
/// cover the realistic range without needing decimal precision here.
fn plausible_range(market: MarketType) -> Option<(i64, i64)> {
    use MarketType::*;
    match market {
        PitcherStrikeouts => Some((1, 10)),
        PitcherHitsAllowed => Some((1, 10)),
        PitcherWalks => Some((0, 6)),
        PitcherEarnedRuns => Some((0, 6)),
        BatterHits => Some((0, 4)),
        BatterHomeRuns => Some((0, 2)),
        BatterRbis => Some((0, 4)),
        BatterStolenBases => Some((0, 2)),
        BatterStrikeouts => Some((0, 4)),
        BatterWalks => Some((0, 3)),
        H2h | Spreads | Totals | Outrights => None,
    }
}

/// Returns `true` if it's realistic for `market`'s Over side to be priced
/// as the favorite (negative American odds) at `point`.
///
/// Upstream player-prop feeds occasionally post a stale or mis-keyed line
/// where the Over on a "multi-occurrence" counting stat (2+ hits, 2+ RBI,
/// 2+ stolen bases, a home run) is priced as the odds-on favorite. In real
/// MLB markets this essentially never happens once the required count
/// reaches 2 — even elite everyday hitters have a multi-hit-game rate
/// well under 50%, so a book would never make that Over the favorite. We
/// treat `over_price` more favored than pick'em (<= -100) as implausible
/// for these markets once `point >= 1.5`, and drop the line rather than
/// surface a distorted edge to the dashboard or the LLM.
///
/// Markets where higher counts are common in a single game (e.g. pitcher
/// strikeouts across 6+ innings, or batter strikeouts for free-swingers)
/// are excluded — an Over favorite there is completely normal.
pub fn is_plausible_prop_odds(market: MarketType, point: Decimal, over_price: Decimal) -> bool {
    use MarketType::*;
    let applies = matches!(market, BatterHits | BatterHomeRuns | BatterRbis | BatterStolenBases);
    if !applies {
        return true;
    }
    if point < Decimal::new(15, 1) {
        // point < 1.5: "at least 1" props (e.g. Over 0.5 hits) are
        // routinely favored for good hitters — that's normal.
        return true;
    }
    over_price > Decimal::new(-100, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_strikeout_line_is_plausible() {
        assert!(is_plausible_prop_point(MarketType::PitcherStrikeouts, Decimal::new(65, 1))); // 6.5
    }

    #[test]
    fn eleven_point_five_strikeouts_is_implausible() {
        assert!(!is_plausible_prop_point(MarketType::PitcherStrikeouts, Decimal::new(115, 1))); // 11.5
    }

    #[test]
    fn non_prop_markets_are_always_plausible() {
        assert!(is_plausible_prop_point(MarketType::Totals, Decimal::new(500, 1))); // 50.0, absurd for totals but not a prop
    }

    #[test]
    fn favored_over_on_two_plus_hits_is_implausible() {
        // Over 1.5 hits at -130 (favorite) — essentially never real.
        assert!(!is_plausible_prop_odds(MarketType::BatterHits, Decimal::new(15, 1), Decimal::new(-130, 0)));
    }

    #[test]
    fn underdog_over_on_two_plus_hits_is_plausible() {
        // Over 1.5 hits at +160 (underdog) — normal.
        assert!(is_plausible_prop_odds(MarketType::BatterHits, Decimal::new(15, 1), Decimal::new(160, 0)));
    }

    #[test]
    fn favored_over_on_at_least_one_hit_is_plausible() {
        // Over 0.5 hits at -188 (favorite) — normal for a good hitter.
        assert!(is_plausible_prop_odds(MarketType::BatterHits, Decimal::new(5, 1), Decimal::new(-188, 0)));
    }

    #[test]
    fn favored_over_on_pitcher_strikeouts_is_unaffected() {
        // Pitcher Ks: Over favored at higher points is completely normal.
        assert!(is_plausible_prop_odds(MarketType::PitcherStrikeouts, Decimal::new(65, 1), Decimal::new(-150, 0)));
    }
}
