//! Ordinary-least-squares linear regression over a player's
//! season-by-season rate stats (e.g. K/9), used to answer "is this
//! player trending up or down?"

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonPoint {
    pub season: i32,
    pub rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// The rate is trending upward season over season.
    Increasing,
    /// The rate is trending downward season over season.
    Decreasing,
    Stable,
    InsufficientData,
}

impl TrendDirection {
    /// Converts a raw (value-neutral) direction into a player-quality
    /// judgment for a specific stat. Some rate stats are "higher is
    /// better" (K/9, home runs, batting average); others are "lower is
    /// better" (ERA, walks allowed) — this makes sure "improving" always
    /// means what it says regardless of which kind of stat is passed in.
    pub fn judge(&self, higher_is_better: bool) -> &'static str {
        match (self, higher_is_better) {
            (TrendDirection::Increasing, true) => "IMPROVING",
            (TrendDirection::Increasing, false) => "REGRESSING",
            (TrendDirection::Decreasing, true) => "REGRESSING",
            (TrendDirection::Decreasing, false) => "IMPROVING",
            (TrendDirection::Stable, _) => "STABLE",
            (TrendDirection::InsufficientData, _) => "INSUFFICIENT DATA",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendResult {
    pub points: Vec<SeasonPoint>,
    pub direction: TrendDirection,
    /// Slope of the fitted line, in rate-units per season.
    pub slope_per_season: f64,
    pub current_season_rate: Option<f64>,
    pub prior_season_rate: Option<f64>,
}

/// Fits an ordinary-least-squares line (rate ~ season) across the given
/// points and classifies the direction. Needs at least 2 seasons of data;
/// fewer returns `InsufficientData`.
pub fn linear_regression_trend(points: &[SeasonPoint]) -> TrendResult {
    let mut pts = points.to_vec();
    pts.sort_by_key(|p| p.season);

    let current_season_rate = pts.last().map(|p| p.rate);
    let prior_season_rate = if pts.len() >= 2 { Some(pts[pts.len() - 2].rate) } else { None };

    if pts.len() < 2 {
        return TrendResult {
            points: pts,
            direction: TrendDirection::InsufficientData,
            slope_per_season: 0.0,
            current_season_rate,
            prior_season_rate,
        };
    }

    let n = pts.len() as f64;
    let mean_x: f64 = pts.iter().map(|p| p.season as f64).sum::<f64>() / n;
    let mean_y: f64 = pts.iter().map(|p| p.rate).sum::<f64>() / n;

    let mut num = 0.0;
    let mut den = 0.0;
    for p in &pts {
        let dx = p.season as f64 - mean_x;
        num += dx * (p.rate - mean_y);
        den += dx * dx;
    }

    let slope = if den.abs() < f64::EPSILON { 0.0 } else { num / den };

    // Direction threshold scaled to the stat's own magnitude, so this
    // works whether the rate stat is K/9 (single digits) or something
    // like ISO (tenths).
    let scale = mean_y.abs().max(0.5);
    let epsilon = scale * 0.03; // ~3% of the mean, per season

    let direction = if slope > epsilon {
        TrendDirection::Increasing
    } else if slope < -epsilon {
        TrendDirection::Decreasing
    } else {
        TrendDirection::Stable
    };

    TrendResult {
        points: pts,
        direction,
        slope_per_season: slope,
        current_season_rate,
        prior_season_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(rates: &[(i32, f64)]) -> Vec<SeasonPoint> {
        rates.iter().map(|(season, rate)| SeasonPoint { season: *season, rate: *rate }).collect()
    }

    #[test]
    fn rising_k9_is_improving() {
        let trend = linear_regression_trend(&pts(&[(2023, 8.0), (2024, 9.0), (2025, 10.0), (2026, 11.0)]));
        assert_eq!(trend.direction, TrendDirection::Increasing);
        assert_eq!(trend.direction.judge(true), "IMPROVING");
        assert_eq!(trend.direction.judge(false), "REGRESSING");
    }

    #[test]
    fn falling_era_is_improving() {
        // ERA is "lower is better" — a falling trend should be IMPROVING.
        let trend = linear_regression_trend(&pts(&[(2023, 4.50), (2024, 3.80), (2025, 3.10), (2026, 2.40)]));
        assert_eq!(trend.direction, TrendDirection::Decreasing);
        assert_eq!(trend.direction.judge(false), "IMPROVING");
        assert_eq!(trend.direction.judge(true), "REGRESSING");
    }

    #[test]
    fn flat_series_is_stable() {
        let trend = linear_regression_trend(&pts(&[(2023, 9.0), (2024, 9.05), (2025, 8.95), (2026, 9.0)]));
        assert_eq!(trend.direction, TrendDirection::Stable);
    }

    #[test]
    fn single_point_is_insufficient_data() {
        let trend = linear_regression_trend(&pts(&[(2026, 9.0)]));
        assert_eq!(trend.direction, TrendDirection::InsufficientData);
        assert_eq!(trend.current_season_rate, Some(9.0));
        assert_eq!(trend.prior_season_rate, None);
    }

    #[test]
    fn current_and_prior_season_rates_are_last_two_points() {
        let trend = linear_regression_trend(&pts(&[(2023, 8.0), (2025, 10.0), (2024, 9.0)]));
        // Points get sorted by season internally regardless of input order.
        assert_eq!(trend.current_season_rate, Some(10.0));
        assert_eq!(trend.prior_season_rate, Some(9.0));
    }
}
