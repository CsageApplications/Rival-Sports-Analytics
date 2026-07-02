//! Expected Value calculations

use rust_decimal::Decimal;
use crate::models::Outcome;

/// Result of an expected value calculation
#[derive(Debug, Clone)]
pub struct EvResult {
    /// The calculated expected value (as percentage of stake)
    pub ev: Decimal,
    /// True probability used in calculation
    pub true_probability: Decimal,
    /// Implied probability from odds
    pub implied_probability: Decimal,
    /// Edge over the bookmaker (true_prob - implied_prob)
    pub edge: Decimal,
}

impl EvResult {
    /// Returns true if this is a positive expected value bet
    pub fn is_positive(&self) -> bool {
        self.ev > Decimal::ZERO
    }
}

/// Calculate expected value given true probability and odds
///
/// EV = (probability × potential_profit) - ((1 - probability) × stake)
/// Simplified: EV = (probability × decimal_odds) - 1
///
/// # Arguments
/// * `true_probability` - Your estimated probability of the outcome (0.0 to 1.0)
/// * `outcome` - The betting outcome with odds
///
/// # Returns
/// EV as a decimal (e.g., 0.05 means +5% expected return per dollar wagered)
pub fn calculate_ev(true_probability: Decimal, outcome: &Outcome) -> EvResult {
    let decimal_odds = outcome.decimal_odds();
    let implied_probability = outcome.implied_probability();
    
    // EV = (P × O) - 1, where P is probability and O is decimal odds
    let ev = (true_probability * decimal_odds) - Decimal::ONE;
    let edge = true_probability - implied_probability;

    EvResult {
        ev,
        true_probability,
        implied_probability,
        edge,
    }
}

/// Calculate the no-vig (fair) probability for a two-way market
///
/// Removes the bookmaker's margin to find "true" implied odds
pub fn calculate_no_vig_probability(outcome1: &Outcome, outcome2: &Outcome) -> (Decimal, Decimal) {
    let prob1 = outcome1.implied_probability();
    let prob2 = outcome2.implied_probability();
    
    // Total implied probability (will be > 1 due to vig)
    let total = prob1 + prob2;
    
    // Normalize to remove vig
    let fair_prob1 = prob1 / total;
    let fair_prob2 = prob2 / total;
    
    (fair_prob1, fair_prob2)
}

/// Calculate the bookmaker's margin (vig/juice)
pub fn calculate_vig(outcomes: &[Outcome]) -> Decimal {
    let total_implied: Decimal = outcomes.iter()
        .map(|o| o.implied_probability())
        .sum();
    
    // Vig is the amount over 100%
    total_implied - Decimal::ONE
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_positive_ev() {
        // +150 odds, we think 50% chance (book implies 40%)
        let outcome = Outcome::new("Team A", dec!(150), None);
        let result = calculate_ev(dec!(0.50), &outcome);
        
        assert!(result.is_positive());
        assert!(result.edge > Decimal::ZERO);
    }

    #[test]
    fn test_negative_ev() {
        // -150 odds, we think 50% chance (book implies 60%)
        let outcome = Outcome::new("Team A", dec!(-150), None);
        let result = calculate_ev(dec!(0.50), &outcome);
        
        assert!(!result.is_positive());
        assert!(result.edge < Decimal::ZERO);
    }

    #[test]
    fn test_vig_calculation() {
        // Standard -110/-110 market
        let outcomes = vec![
            Outcome::new("Team A", dec!(-110), None),
            Outcome::new("Team B", dec!(-110), None),
        ];
        let vig = calculate_vig(&outcomes);
        
        // Should be ~4.5% vig
        assert!(vig > dec!(0.04) && vig < dec!(0.05));
    }
}
