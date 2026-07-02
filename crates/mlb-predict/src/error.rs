//! Error types for `mlb-predict`

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredictError {
    #[error("MLB Stats API error: {0}")]
    Stats(#[from] mlb_stats_client::StatsApiError),

    #[error("database error: {0}")]
    Db(#[from] mlb_db::DbError),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
