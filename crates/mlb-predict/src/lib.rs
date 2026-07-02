//! # MLB Predict
//!
//! Historical trend regression and matchup-history analysis for MLB
//! players, built on top of `mlb-stats-client`. This crate does not fetch
//! odds/props itself — it answers "how has this player performed
//! historically" so that the Parlay Assistant chatbot (and the CLI) can
//! ground picks in real season trends and head-to-head matchup data
//! instead of just current market lines.
//!
//! ## Example
//!
//! ```no_run
//! use mlb_db::Database;
//! use mlb_predict::{get_or_fetch_report, last_n_meetings, linear_regression_trend, stats, SeasonPoint};
//! use mlb_stats_client::{StatGroup, StatsApiClient, find_team_id_by_name};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let db = Database::new("mlb_betting.db").await?;
//! let client = StatsApiClient::new();
//!
//! let report = get_or_fetch_report(&db, &client, "Chase Burns", StatGroup::Pitching, 4).await?;
//!
//! let points: Vec<SeasonPoint> = report.seasons.iter()
//!     .filter_map(|s| stats::k_per_9(s).map(|rate| SeasonPoint { season: s.season, rate }))
//!     .collect();
//! let trend = linear_regression_trend(&points);
//!
//! if let Some(astros_id) = find_team_id_by_name("Houston Astros") {
//!     let last3 = last_n_meetings(&report.recent_games, astros_id, 3);
//!     println!("{:?} {:?}", trend.direction, last3.len());
//! }
//! # Ok(()) }
//! ```

mod error;
mod matchup;
mod report;
pub mod stats;
mod trend;

pub use error::PredictError;
pub use matchup::last_n_meetings;
pub use report::{fetch_and_cache_report, get_cached_report, get_or_fetch_report, PlayerReport, CACHE_TTL_HOURS};
pub use trend::{linear_regression_trend, SeasonPoint, TrendDirection, TrendResult};

// Re-exported for convenience so downstream crates usually only need to
// depend on `mlb-predict` + `mlb-stats-client` directly for types.
pub use mlb_stats_client::StatGroup;
