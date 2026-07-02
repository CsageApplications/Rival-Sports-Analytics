//! Kelly Criterion stake sizing

use rust_decimal::Decimal;

/// Result of Kelly Criterion calculation
#[derive(Debug, Clone)]
pub struct KellyResult {
    /// Full Kelly stake as fraction of bankroll (0.0 to 1.0)
    pub full_kelly: Decimal,
    /// Half Kelly (more conservative)
    pub half_kelly: Decimal,
    /// Quarter Kelly (very conservative)
    pub quarter_kelly: Decimal,
}

impl KellyResult {
    /// Get recommended stake based on risk tolerance
    pub fn recommended_stake(&self, bankroll: Decimal, fraction: KellyFraction) -> Decimal {
        let kelly = match fraction {
            KellyFraction::Full => self.full_kelly,
            KellyFraction::Half => self.half_kelly,
            KellyFraction::Quarter => self.quarter_kelly,
        };
        bankroll * kelly.max(Decimal::ZERO)
    }
}

/// Kelly fraction for risk management
#[derive(Debug, Clone, Copy, Default)]
pub enum KellyFraction {
    Full,
    #[default]
    Half,
    Quarter,
}

/// Calculate Kelly Criterion optimal stake
///
/// Kelly formula: f* = (bp - q) / b
/// Where:
/// - f* = fraction of bankroll to wager
/// - b = decimal odds - 1 (net odds)
/// - p = probability of winning
/// - q = probability of losing (1 - p)
///
/// # Arguments
/// * `win_probability` - Estimated probability of winning (0.0 to 1.0)
/// * `decimal_odds` - The decimal odds offered
///
/// # Returns
/// Kelly stake fractions (full, half, quarter)
pub fn calculate_kelly(win_probability: Decimal, decimal_odds: Decimal) -> KellyResult {
    let one = Decimal::ONE;
    let two = Decimal::from(2);
    let four = Decimal::from(4);

    // b = decimal_odds - 1 (this is the profit multiplier)
    let b = decimal_odds - one;
    
    // q = 1 - p
    let q = one - win_probability;
    
    // Kelly: f* = (bp - q) / b
    let full_kelly = if b > Decimal::ZERO {
        ((b * win_probability) - q) / b
    } else {
        Decimal::ZERO
    };

    KellyResult {
        full_kelly,
        half_kelly: full_kelly / two,
        quarter_kelly: full_kelly / four,
    }
}

/// Calculate Kelly for American odds
pub fn calculate_kelly_american(win_probability: Decimal, american_odds: Decimal) -> KellyResult {
    let hundred = Decimal::from(100);
    let one = Decimal::ONE;
    
    // Convert American to decimal odds
    let decimal_odds = if american_odds >= Decimal::ZERO {
        (american_odds / hundred) + one
    } else {
        (hundred / american_odds.abs()) + one
    };
    
    calculate_kelly(win_probability, decimal_odds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_kelly_positive_edge() {
        // 55% win probability at even money (2.0 decimal)
        let result = calculate_kelly(dec!(0.55), dec!(2.0));
        
        // Kelly should recommend betting 10% of bankroll
        assert!(result.full_kelly > dec!(0.09) && result.full_kelly < dec!(0.11));
    }

    #[test]
    fn test_kelly_no_edge() {
        // 50% at even money = no edge = don't bet
        let result = calculate_kelly(dec!(0.50), dec!(2.0));
        
        assert_eq!(result.full_kelly, Decimal::ZERO);
    }

    #[test]
    fn test_kelly_negative_edge() {
        // 45% at even money = negative EV
        let result = calculate_kelly(dec!(0.45), dec!(2.0));
        
        // Kelly will be negative (don't bet)
        assert!(result.full_kelly < Decimal::ZERO);
    }

    #[test]
    fn test_kelly_american_odds() {
        // 55% probability at -110 odds
        let result = calculate_kelly_american(dec!(0.55), dec!(-110));
        
        // Should recommend a small positive stake
        assert!(result.full_kelly > Decimal::ZERO);
    }
}
