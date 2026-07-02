//! Player props command

use anyhow::Result;
use clap::Subcommand;

use mlb_core::models::MarketType;
use mlb_core::repository::EventRepository;
use mlb_db::Database;
use super::AppContext;

#[derive(Subcommand)]
pub enum PropsCommands {
    /// List player props for a game (by event ID or team name fragment)
    Show {
        /// Event ID or partial team name (e.g. "yankees", "dodgers")
        query: String,

        /// Only show pitcher props
        #[arg(long)]
        pitchers: bool,

        /// Only show batter props
        #[arg(long)]
        batters: bool,
    },
}

impl PropsCommands {
    pub async fn run(self) -> Result<()> {
        match self {
            PropsCommands::Show { query, pitchers, batters } => {
                show_props(&query, pitchers, batters).await
            }
        }
    }
}

async fn show_props(query: &str, pitchers_only: bool, batters_only: bool) -> Result<()> {
    let ctx = AppContext::new().await?;

    // Resolve query → event_id
    let event_id = resolve_event(&ctx.db, query).await?;

    println!("Fetching player props... (costs 1 API request)");
    let (event, odds) = ctx.client.get_mlb_player_props(&event_id).await?;

    println!("\n{} — Player Props", event.matchup());
    println!("{}", "═".repeat(72));
    println!("  Requests remaining: {}\n", ctx.client.rate_limiter().remaining());

    // Group by market type
    let prop_markets = [
        (false, MarketType::PitcherStrikeouts),
        (false, MarketType::PitcherHitsAllowed),
        (false, MarketType::PitcherEarnedRuns),
        (false, MarketType::PitcherWalks),
        (true,  MarketType::BatterHits),
        (true,  MarketType::BatterHomeRuns),
        (true,  MarketType::BatterRbis),
        (true,  MarketType::BatterStolenBases),
        (true,  MarketType::BatterStrikeouts),
        (true,  MarketType::BatterWalks),
    ];

    for (is_batter, market) in &prop_markets {
        if pitchers_only && *is_batter { continue; }
        if batters_only  && !is_batter { continue; }

        // Collect all outcomes for this market across bookmakers
        let mut player_lines: std::collections::HashMap<String, Vec<(String, String, f64, f64)>> =
            std::collections::HashMap::new();

        for book in &odds.bookmakers {
            if let Some(m) = book.markets.iter().find(|m| m.key == *market) {
                // Outcomes come in pairs: Over/Under for the same player
                // They share the same `description` (player name)
                let mut i = 0;
                while i < m.outcomes.len() {
                    let o = &m.outcomes[i];
                    let player = o.description.clone()
                        .unwrap_or_else(|| "Unknown".to_string());

                    let line = o.point
                        .map(|p| format!("{}", p))
                        .unwrap_or_default();

                    let odds_str = if o.price >= rust_decimal::Decimal::ZERO {
                        format!("+{}", o.price)
                    } else {
                        format!("{}", o.price)
                    };

                    let price_f: f64 = o.price.to_string().parse().unwrap_or(0.0);
                    let line_f: f64 = o.point.map(|p| p.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0);

                    let entry = format!("{} {} ({})", o.name, line, odds_str);
                    player_lines
                        .entry(player)
                        .or_default()
                        .push((book.title.clone(), entry, price_f, line_f));

                    i += 1;
                }
            }
        }

        if player_lines.is_empty() { continue; }

        let category = if *is_batter { "Batter" } else { "Pitcher" };
        println!("{}  ─  {}", category, market);
        println!("{}", "─".repeat(60));

        let mut players: Vec<String> = player_lines.keys().cloned().collect();
        players.sort();

        for player in &players {
            let lines = &player_lines[player];
            println!("  {}", player);
            for (book, outcome_str, _, _) in lines {
                println!("    {:20}  {}", book, outcome_str);
            }
            println!();
        }
    }

    Ok(())
}

async fn resolve_event(db: &Database, query: &str) -> Result<String> {
    // First try as exact event ID (long hex string)
    if query.len() == 32 {
        return Ok(query.to_string());
    }

    // Otherwise search by team name
    let events = db.get_upcoming_events().await?;
    let q = query.to_lowercase();

    let matches: Vec<_> = events.iter().filter(|e| {
        e.home_team.to_lowercase().contains(&q) ||
        e.away_team.to_lowercase().contains(&q)
    }).collect();

    match matches.len() {
        0 => anyhow::bail!(
            "No upcoming game found matching '{}'. Run `mlb events list` to see available games.",
            query
        ),
        1 => Ok(matches[0].id.clone()),
        _ => {
            println!("Multiple games match '{}', pick one:\n", query);
            for (i, e) in matches.iter().enumerate() {
                println!("  [{}] {} — {} ({})", i + 1, e.matchup(), e.commence_time.format("%a %b %d %I:%M %p"), &e.id[..8]);
            }
            anyhow::bail!("Refine your search or use the full event ID.");
        }
    }
}
