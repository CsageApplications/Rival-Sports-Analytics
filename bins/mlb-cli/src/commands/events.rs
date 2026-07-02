//! Events commands

use anyhow::Result;
use clap::Subcommand;

use mlb_core::repository::EventRepository;
use mlb_db::Database;
use crate::config::Config;

#[derive(Subcommand)]
pub enum EventCommands {
    /// List upcoming events from database
    List {
        /// Maximum number of events to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Show details for a specific event
    Show {
        /// Event ID
        event_id: String,
    },
}

impl EventCommands {
    pub async fn run(self) -> Result<()> {
        let config = Config::from_env()?;
        let db = Database::new(&config.db_path).await?;
        db.migrate().await?;

        match self {
            EventCommands::List { limit } => list_events(&db, limit).await,
            EventCommands::Show { event_id } => show_event(&db, &event_id).await,
        }
    }
}

async fn list_events(db: &Database, limit: usize) -> Result<()> {
    let events = db.get_upcoming_events().await?;
    
    if events.is_empty() {
        println!("No upcoming events in database. Run `mlb fetch events` first.");
        return Ok(());
    }

    println!("Upcoming MLB Games:\n");
    
    for event in events.iter().take(limit) {
        println!("  {} | {} | {}", 
            event.commence_time.format("%a %b %d %I:%M %p"),
            event.matchup(),
            &event.id[..8]
        );
    }

    if events.len() > limit {
        println!("\n  ... and {} more", events.len() - limit);
    }

    Ok(())
}

async fn show_event(db: &Database, event_id: &str) -> Result<()> {
    let event = db.get_event(event_id).await?;
    
    println!("Event Details:\n");
    println!("  ID:       {}", event.id);
    println!("  Matchup:  {}", event.matchup());
    println!("  Time:     {}", event.commence_time.format("%a %b %d, %Y %I:%M %p UTC"));
    println!("  Sport:    {} ({})", event.sport_title, event.sport_key);

    // Try to get latest odds
    use mlb_core::repository::OddsRepository;
    if let Ok(odds) = db.get_latest_odds(event_id).await {
        println!("\nLatest Odds (captured {}):", odds.captured_at.format("%I:%M %p"));
        for bookmaker in &odds.bookmakers {
            println!("\n  {}:", bookmaker.title);
            for market in &bookmaker.markets {
                print!("    {}: ", market.key);
                for (i, outcome) in market.outcomes.iter().enumerate() {
                    if i > 0 {
                        print!(" | ");
                    }
                    if let Some(point) = outcome.point {
                        print!("{} ({:+}) {:+}", outcome.name, point, outcome.price);
                    } else {
                        print!("{} {:+}", outcome.name, outcome.price);
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}
