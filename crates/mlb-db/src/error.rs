//! Database error types

use thiserror::Error;
use mlb_core::repository::RepositoryError;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Record not found: {0}")]
    NotFound(String),
}

impl From<DbError> for RepositoryError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound(msg) => RepositoryError::NotFound(msg),
            other => RepositoryError::Database(other.to_string()),
        }
    }
}
