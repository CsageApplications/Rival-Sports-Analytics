//! Fetch commands

use anyhow::Result;
use clap::Subcommand;

use super::AppContext;
use mlb_core::repository::{EventRepository, OddsRepository};

#[derive(Subcommand)]
pub enum FetchCommands {
    /// Fetch current MLB odds and save to database
    Odds {
        /// Also print odds to console
        #[arg(short, long)]
        print: bool,

        /// Also fetch player props for every game and merge them into the
        /// same snapshot (costs 1 extra API request per game)
        #[arg(long)]
        with_props: bool,
    },
    /// Fetch upcoming MLB events
    Events,
}

impl FetchCommands {
    pub async fn run(self) -> Result<()> {
        let ctx = AppContext::new().await?;

        match self {
            FetchCommands::Odds { print, with_props } => fetch_odds(&ctx, print, with_props).await,
            FetchCommands::Events => fetch_events(&ctx).await,
        }
    }
}

async fn fetch_odds(ctx: &AppContext, print: bool, with_props: bool) -> Result<()> {
    println!("Fetching MLB odds (FanDuel + DraftKings only)...\n");

    let results = ctx.client.get_mlb_odds().await?;
    
    println!("Found {} games with odds", results.len());

    for (event, mut odds) in results {
        if with_props {
            match ctx.client.get_mlb_player_props(&event.id).await {
                Ok((_, prop_odds)) => odds.merge_bookmakers(&prop_odds),
                Err(e) => println!("  (skipped props for {}: {})", event.matchup(), e),
            }
        }

        // Save event
        ctx.db.save_event(&event).await?;
        // Save odds snapshot
        ctx.db.save_odds(&odds).await?;

        if print {
            println!("\n{}", event.matchup());
            println!("  {} ({})", event.commence_time.format("%a %b %d %I:%M %p"), event.id);
            
            for bookmaker in &odds.bookmakers {
                println!("  {}:", bookmaker.title);
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
    }

    println!("\nSaved event and odds snapshots to database");
    println!("  Requests remaining: {}", ctx.client.rate_limiter().remaining());

    Ok(())
}

async fn fetch_events(ctx: &AppContext) -> Result<()> {
    println!("Fetching upcoming MLB events...\n");

    let events = ctx.client.get_mlb_events().await?;
    
    println!("Found {} upcoming games:\n", events.len());

    for event in &events {
        ctx.db.save_event(event).await?;
        println!("  {} - {}", event.commence_time.format("%a %b %d %I:%M %p"), event.matchup());
    }

    println!("\n✓ Saved {} events to database", events.len());

    Ok(())
}
