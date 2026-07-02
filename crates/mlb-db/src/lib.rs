//! # MLB Database
//!
//! SQLite persistence layer for MLB sports betting data.
//!
//! ## Usage
//!
//! ```no_run
//! use mlb_db::Database;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = Database::new("mlb_betting.db").await?;
//!     db.migrate().await?;
//!     
//!     // Use repository methods...
//!     Ok(())
//! }
//! ```

mod database;
mod error;
mod repositories;

pub use database::Database;
pub use error::DbError;
pub use repositories::*;
