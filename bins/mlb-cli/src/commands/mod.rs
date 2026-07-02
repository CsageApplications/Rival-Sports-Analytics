//! CLI commands

mod fetch;
mod events;
mod analyze;
mod bankroll;
mod props;
mod history;
pub mod export;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::Config;
use mlb_db::Database;
use mlb_odds_client::OddsClient;

/// MLB Sports Betting CLI
#[derive(Parser)]
#[command(name = "mlb")]
#[command(author, version, about = "MLB sports betting analysis tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch odds and events from The Odds API
    Fetch {
        #[command(subcommand)]
        cmd: fetch::FetchCommands,
    },
    /// List events
    Events {
        #[command(subcommand)]
        cmd: events::EventCommands,
    },
    /// Analyze odds for betting opportunities
    Analyze {
        #[command(subcommand)]
        cmd: analyze::AnalyzeCommands,
    },
    /// Manage bankroll
    Bankroll {
        #[command(subcommand)]
        cmd: bankroll::BankrollCommands,
    },
    /// Export latest odds + analysis to JSON for the web dashboard
    Export {
        /// Output directory (default: bins/mlb-ui/public/data)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// Fetch and display player props for a game
    Props {
        #[command(subcommand)]
        cmd: props::PropsCommands,
    },
    /// Historical player stats, trend regression, and matchup history (MLB Stats API)
    History {
        #[command(subcommand)]
        cmd: history::HistoryCommands,
    },
    /// Show API usage and rate limits
    Status,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Fetch { cmd } => cmd.run().await,
            Commands::Events { cmd } => cmd.run().await,
            Commands::Analyze { cmd } => cmd.run().await,
            Commands::Bankroll { cmd } => cmd.run().await,
            Commands::Export { output } => export::run_export(output).await,
            Commands::Props { cmd } => cmd.run().await,
            Commands::History { cmd } => cmd.run().await,
            Commands::Status => run_status().await,
        }
    }
}

/// Shared context for commands
pub struct AppContext {
    pub config: Config,
    pub db: Database,
    pub client: OddsClient,
}

impl AppContext {
    pub async fn new() -> Result<Self> {
        let config = Config::from_env()?;
        let db = Database::new(&config.db_path).await?;
        db.migrate().await?;
        let client = OddsClient::new(&config.api_key)?;
        Ok(Self { config, db, client })
    }
}

async fn run_status() -> Result<()> {
    let config = Config::from_env()?;
    let client = OddsClient::new(&config.api_key)?;

    // Make a free API call to check status
    println!("Checking API status...\n");
    
    let sports = client.get_sports().await?;
    let mlb = sports.iter().find(|s| s.key == "baseball_mlb");

    println!("✓ API key is valid");
    println!("  Requests remaining: {}", client.rate_limiter().remaining());
    println!("  Requests used: {}", client.rate_limiter().used());
    println!();

    match mlb {
        Some(sport) if sport.active => {
            println!("✓ MLB ({}) is currently active", sport.title);
        }
        Some(_) => {
            println!("⚠ MLB is currently in off-season");
        }
        None => {
            println!("⚠ MLB not found in available sports");
        }
    }

    Ok(())
}
