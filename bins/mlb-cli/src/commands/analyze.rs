//! Analysis commands

use anyhow::Result;
use clap::Subcommand;
use rust_decimal::Decimal;
use std::str::FromStr;

use mlb_core::analysis::{calculate_ev, calculate_no_vig_probability, calculate_vig, scan_for_arbitrage, scan_best_ev};
use mlb_core::models::MarketType;
use mlb_core::repository::{EventRepository, OddsRepository};
use mlb_db::Database;
use mlb_predict::{compute_game_edge, find_standing_by_team_name, get_or_fetch_standings};
use mlb_stats_client::StatsApiClient;
use crate::config::Config;

#[derive(Subcommand)]
pub enum AnalyzeCommands {
    /// Analyze expected value for an event
    Ev {
        /// Event ID (or "all" for all upcoming events)
        #[arg(default_value = "all")]
        event_id: String,
        
        /// Your estimated probability for home team (e.g., 0.55)
        #[arg(short, long)]
        probability: Option<f64>,
    },
    /// Scan for arbitrage opportunities
    Arb,
    /// Scan every book/market for the best expected-value bets, using a
    /// multi-bookmaker consensus (de-vigged) fair-probability model
    BestEv {
        /// Event ID (or "all" for all upcoming events)
        #[arg(default_value = "all")]
        event_id: String,

        /// Maximum number of bets to show
        #[arg(short, long, default_value_t = 15)]
        limit: usize,

        /// Only show positive expected-value bets
        #[arg(long, default_value_t = true)]
        positive_only: bool,
    },
    /// Show best odds across all bookmakers
    Best {
        /// Event ID (or "all")
        #[arg(default_value = "all")]
        event_id: String,
    },
    /// Show vig/juice for each bookmaker
    Vig {
        /// Event ID
        event_id: String,
    },
    /// Project pre-game win probabilities from team standings
    /// (Pythagorean win expectation + Log5), compared against the
    /// market's de-vigged moneyline probability.
    Predict {
        /// Event ID (or "all" for all upcoming events)
        #[arg(default_value = "all")]
        event_id: String,
    },
}

impl AnalyzeCommands {
    pub async fn run(self) -> Result<()> {
        let config = Config::from_env()?;
        let db = Database::new(&config.db_path).await?;
        db.migrate().await?;

        match self {
            AnalyzeCommands::Ev { event_id, probability } => analyze_ev(&db, &event_id, probability).await,
            AnalyzeCommands::Arb => scan_arb(&db).await,
            AnalyzeCommands::BestEv { event_id, limit, positive_only } => best_ev(&db, &event_id, limit, positive_only).await,
            AnalyzeCommands::Best { event_id } => show_best_odds(&db, &event_id).await,
            AnalyzeCommands::Vig { event_id } => show_vig(&db, &event_id).await,
            AnalyzeCommands::Predict { event_id } => predict_games(&db, &event_id).await,
        }
    }
}

async fn analyze_ev(db: &Database, event_id: &str, user_prob: Option<f64>) -> Result<()> {
    let events = if event_id == "all" {
        db.get_upcoming_events().await?
    } else {
        vec![db.get_event(event_id).await?]
    };

    if events.is_empty() {
        println!("No events found. Run `mlb fetch odds` first.");
        return Ok(());
    }

    println!("Expected Value Analysis\n");
    println!("{:=<80}", "");

    for event in events {
        let odds = match db.get_latest_odds(&event.id).await {
            Ok(o) => o,
            Err(_) => continue,
        };

        println!("\n{}", event.matchup());
        println!("  {}\n", event.commence_time.format("%a %b %d %I:%M %p"));

        // Get h2h market from first bookmaker for no-vig calculation
        let fair_probs = odds.bookmakers.first()
            .and_then(|b| b.get_market(MarketType::H2h))
            .filter(|m| m.outcomes.len() >= 2)
            .map(|m| calculate_no_vig_probability(&m.outcomes[0], &m.outcomes[1]));

        if let Some((home_fair, away_fair)) = fair_probs {
            println!("  Fair odds (no-vig): {} {:.1}% | {} {:.1}%", 
                event.home_team, home_fair * Decimal::from(100),
                event.away_team, away_fair * Decimal::from(100));
        }

        // Use user probability or fair probability
        let home_prob = user_prob
            .map(|p| Decimal::from_str(&format!("{:.4}", p)).unwrap())
            .or_else(|| fair_probs.map(|(h, _)| h));

        if let Some(true_prob) = home_prob {
            println!("\n  Using {:.1}% estimated probability for {}\n", 
                true_prob * Decimal::from(100), event.home_team);

            // Find best odds for home team
            if let Some((book, outcome)) = odds.best_odds(MarketType::H2h, &event.home_team) {
                let ev_result = calculate_ev(true_prob, outcome);
                let ev_pct = ev_result.ev * Decimal::from(100);
                
                let ev_indicator = if ev_result.is_positive() { "✓ +EV" } else { "✗ -EV" };
                
                println!("  {} @ {} ({:+}): {:+.2}% EV {}", 
                    event.home_team, &book.title, outcome.price, ev_pct, ev_indicator);
            }

            // Away team
            let away_prob = Decimal::ONE - true_prob;
            if let Some((book, outcome)) = odds.best_odds(MarketType::H2h, &event.away_team) {
                let ev_result = calculate_ev(away_prob, outcome);
                let ev_pct = ev_result.ev * Decimal::from(100);
                
                let ev_indicator = if ev_result.is_positive() { "✓ +EV" } else { "✗ -EV" };
                
                println!("  {} @ {} ({:+}): {:+.2}% EV {}", 
                    event.away_team, &book.title, outcome.price, ev_pct, ev_indicator);
            }
        }
    }

    Ok(())
}

async fn scan_arb(db: &Database) -> Result<()> {
    let events = db.get_upcoming_events().await?;
    
    if events.is_empty() {
        println!("No events found. Run `mlb fetch odds` first.");
        return Ok(());
    }

    println!("Scanning for arbitrage opportunities...\n");

    let mut found = 0;

    for event in events {
        let odds = match db.get_latest_odds(&event.id).await {
            Ok(o) => o,
            Err(_) => continue,
        };

        let arbs = scan_for_arbitrage(&odds);
        
        for arb in arbs {
            found += 1;
            println!("ARBITRAGE FOUND: {}", event.matchup());
            println!("   Profit: {:.2}%", arb.profit_percentage * Decimal::from(100));
            
            for leg in &arb.legs {
                println!("   {} {} @ {} ({:+}) - stake {:.1}%",
                    leg.outcome, leg.bookmaker, leg.odds,
                    leg.odds,
                    leg.stake_percentage * Decimal::from(100));
            }
            println!();
        }
    }

    if found == 0 {
        println!("No arbitrage opportunities found.");
        println!("\nTip: Arb opportunities are rare. Consider:");
        println!("  - Fetching odds more frequently");
        println!("  - Including more regions (UK, EU books often have different lines)");
    } else {
        println!("Found {} arbitrage opportunities!", found);
    }

    Ok(())
}

async fn best_ev(db: &Database, event_id: &str, limit: usize, positive_only: bool) -> Result<()> {
    let events = if event_id == "all" {
        db.get_upcoming_events().await?
    } else {
        vec![db.get_event(event_id).await?]
    };

    if events.is_empty() {
        println!("No events found. Run `mlb fetch odds` first.");
        return Ok(());
    }

    println!("Best Expected Value (FanDuel + DraftKings consensus model)\n");
    println!("{:=<80}", "");

    let mut all_bets = Vec::new();

    for event in &events {
        let odds = match db.get_latest_odds(&event.id).await {
            Ok(o) => o,
            Err(_) => continue,
        };

        for bet in scan_best_ev(&event.id, &odds) {
            if positive_only && bet.ev_pct <= Decimal::ZERO {
                continue;
            }
            all_bets.push((event.matchup(), bet));
        }
    }

    all_bets.sort_by(|a, b| b.1.ev_pct.cmp(&a.1.ev_pct));

    if all_bets.is_empty() {
        println!("No {}EV bets found.", if positive_only { "positive-" } else { "" });
        return Ok(());
    }

    for (matchup, bet) in all_bets.iter().take(limit) {
        let player_str = bet.player.as_ref().map(|p| format!("{} — ", p)).unwrap_or_default();
        let point_str = bet.point.map(|p| format!(" ({:+})", p)).unwrap_or_default();
        let indicator = if bet.ev_pct > Decimal::ZERO { "+EV" } else { "-EV" };

        println!(
            "  [{:+.2}% EV] {} — {}{}{} @ {} ({:+}) — {}",
            bet.ev_pct,
            matchup,
            player_str,
            bet.outcome_name,
            point_str,
            bet.bookmaker_title,
            bet.odds,
            indicator,
        );
        println!(
            "      fair {:.1}% vs implied {:.1}% · edge {:+.2}pp · market: {}",
            bet.true_probability * Decimal::from(100),
            bet.implied_probability * Decimal::from(100),
            bet.edge * Decimal::from(100),
            bet.market,
        );
    }

    Ok(())
}

async fn show_best_odds(db: &Database, event_id: &str) -> Result<()> {
    let events = if event_id == "all" {
        db.get_upcoming_events().await?
    } else {
        vec![db.get_event(event_id).await?]
    };

    if events.is_empty() {
        println!("No events found. Run `mlb fetch odds` first.");
        return Ok(());
    }

    println!("Best Available Odds\n");
    println!("{:=<80}", "");

    for event in events {
        let odds = match db.get_latest_odds(&event.id).await {
            Ok(o) => o,
            Err(_) => continue,
        };

        println!("\n{}", event.matchup());
        println!("  {}\n", event.commence_time.format("%a %b %d %I:%M %p"));

        // Best moneyline
        if let Some((book, out)) = odds.best_odds(MarketType::H2h, &event.home_team) {
            println!("  Moneyline {} {:+} @ {}", event.home_team, out.price, &book.title);
        }
        if let Some((book, out)) = odds.best_odds(MarketType::H2h, &event.away_team) {
            println!("  Moneyline {} {:+} @ {}", event.away_team, out.price, &book.title);
        }
    }

    Ok(())
}

async fn show_vig(db: &Database, event_id: &str) -> Result<()> {
    let event = db.get_event(event_id).await?;
    let odds = db.get_latest_odds(event_id).await?;

    println!("Vig Analysis: {}\n", event.matchup());

    for bookmaker in &odds.bookmakers {
        if let Some(market) = bookmaker.get_market(MarketType::H2h) {
            let vig = calculate_vig(&market.outcomes);
            let vig_pct = vig * Decimal::from(100);
            
            println!("  {}: {:.2}% vig", bookmaker.title, vig_pct);
        }
    }

    println!("\n  Lower vig = better value for bettors");

    Ok(())
}

/// Current MLB season year. Standings for the in-progress season are
/// what we want year-round (there's no "next season" data until pitchers
/// and catchers report), so this is just the current calendar year.
fn current_season() -> i32 {
    use chrono::Datelike;
    chrono::Utc::now().year()
}

async fn predict_games(db: &Database, event_id: &str) -> Result<()> {
    let events = if event_id == "all" {
        db.get_upcoming_events().await?
    } else {
        vec![db.get_event(event_id).await?]
    };

    if events.is_empty() {
        println!("No events found. Run `mlb fetch odds` first.");
        return Ok(());
    }

    println!("Pre-Game Win Probability Model");
    println!("(Pythagorean win expectation + Log5, vs. market de-vigged odds)\n");
    println!("{:=<80}", "");

    let stats_client = StatsApiClient::new();
    let season = current_season();
    let standings = get_or_fetch_standings(db, &stats_client, season).await?;

    if standings.is_empty() {
        println!("No standings data available for season {}.", season);
        return Ok(());
    }

    let mut shown = 0;
    for event in &events {
        let odds = match db.get_latest_odds(&event.id).await {
            Ok(o) => o,
            Err(_) => continue,
        };

        let Some(home) = find_standing_by_team_name(&standings, &event.home_team) else {
            println!("\n{} — no standings match for '{}', skipping.", event.matchup(), event.home_team);
            continue;
        };
        let Some(away) = find_standing_by_team_name(&standings, &event.away_team) else {
            println!("\n{} — no standings match for '{}', skipping.", event.matchup(), event.away_team);
            continue;
        };

        let Some(edge) = compute_game_edge(&event.home_team, &event.away_team, home, away, &odds) else {
            println!("\n{} — no moneyline odds posted yet, skipping.", event.matchup());
            continue;
        };

        shown += 1;
        println!("\n{}", event.matchup());
        println!("  {}\n", event.commence_time.format("%a %b %d %I:%M %p"));
        println!(
            "  {} ({}-{}, {:.0} RS/{:.0} RA): model {:.1}% | market fair {:.1}% | edge {:+.1}pp",
            event.home_team,
            home.wins,
            home.losses,
            home.runs_scored.unwrap_or(0) as f64,
            home.runs_allowed.unwrap_or(0) as f64,
            edge.model_home_win_pct * 100.0,
            edge.market_home_fair_pct * 100.0,
            edge.home_edge_pct,
        );
        println!(
            "  {} ({}-{}, {:.0} RS/{:.0} RA): model {:.1}% | market fair {:.1}% | edge {:+.1}pp",
            event.away_team,
            away.wins,
            away.losses,
            away.runs_scored.unwrap_or(0) as f64,
            away.runs_allowed.unwrap_or(0) as f64,
            edge.model_away_win_pct * 100.0,
            edge.market_away_fair_pct * 100.0,
            edge.away_edge_pct,
        );
    }

    if shown == 0 {
        println!("\nNo games could be projected (missing standings match or odds).");
    }

    println!("\nNote: this is a sabermetric estimate (Pythagorean win expectation + Log5), not a guarantee.");

    Ok(())
}
