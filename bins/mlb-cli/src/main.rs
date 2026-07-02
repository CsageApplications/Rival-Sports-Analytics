//! MLB Sports Betting CLI

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;

use commands::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("mlb=info".parse()?))
        .init();

    // Parse CLI arguments and run
    let cli = Cli::parse();
    cli.run().await
}
