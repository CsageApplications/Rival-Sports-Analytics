//! # MLB Core
//!
//! Core domain models and business logic for MLB sports betting analysis.
//! This crate contains no infrastructure dependencies - only pure domain logic.

pub mod models;
pub mod analysis;
pub mod repository;

pub use models::*;
pub use analysis::*;
