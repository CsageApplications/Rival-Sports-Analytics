//! Export command — dumps DB data to JSON for the React dashboard

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use mlb_core::analysis::{scan_best_ev, scan_for_arbitrage};
use mlb_core::models::{EventOdds, MarketType};
use mlb_core::repository::{EventRepository, OddsRepository};
use mlb_db::Database;
use mlb_odds_client::ALLOWED_BOOKMAKERS;
use crate::config::Config;

#[derive(Serialize, Deserialize)]
pub struct GameExport {
    pub id: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub commence_time: String,
    pub commence_timestamp: i64,
    pub bookmakers: Vec<BookmakerExport>,
    pub best_home_odds: Option<BestOddsExport>,
    pub best_away_odds: Option<BestOddsExport>,
}

#[derive(Serialize, Deserialize)]
pub struct BookmakerExport {
    pub key: String,
    pub title: String,
    pub markets: Vec<MarketExport>,
}

#[derive(Serialize, Deserialize)]
pub struct MarketExport {
    pub key: String,
    pub outcomes: Vec<OutcomeExport>,
}

#[derive(Serialize, Deserialize)]
pub struct OutcomeExport {
    pub name: String,
    pub price: f64,
    pub point: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct BestOddsExport {
    pub team: String,
    pub odds: f64,
    pub bookmaker: String,
}

#[derive(Serialize, Deserialize)]
pub struct ArbExport {
    pub event_id: String,
    pub matchup: String,
    pub profit_pct: f64,
    pub legs: Vec<ArbLegExport>,
}

#[derive(Serialize, Deserialize)]
pub struct ArbLegExport {
    pub outcome: String,
    pub bookmaker: String,
    pub odds: f64,
    pub stake_pct: f64,
}

#[derive(Serialize, Deserialize)]
pub struct MetaExport {
    pub exported_at: String,
    pub total_games: usize,
    pub arb_count: usize,
    pub best_ev_count: usize,
    pub prop_count: usize,
    pub requests_remaining: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct BestEvExport {
    pub event_id: String,
    pub matchup: String,
    pub market: String,
    pub outcome: String,
    pub player: Option<String>,
    pub point: Option<f64>,
    pub bookmaker: String,
    pub odds: f64,
    pub true_probability: f64,
    pub implied_probability: f64,
    pub edge: f64,
    pub ev_pct: f64,
}

#[derive(Serialize, Deserialize)]
pub struct PropBookExport {
    pub bookmaker: String,
    pub over_odds: Option<f64>,
    pub under_odds: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct PropExport {
    pub event_id: String,
    pub matchup: String,
    pub market: String,
    pub player: String,
    pub point: Option<f64>,
    pub books: Vec<PropBookExport>,
}

pub async fn run_export(output_dir: Option<PathBuf>) -> Result<()> {
    let config = Config::from_env()?;
    let db = Database::new(&config.db_path).await?;
    db.migrate().await?;

    // Default output to bins/mlb-ui/public/data/
    let out_dir = output_dir.unwrap_or_else(|| PathBuf::from("bins/mlb-ui/public/data"));
    fs::create_dir_all(&out_dir)?;

    let events = db.get_upcoming_events().await?;
    if events.is_empty() {
        println!("No events in DB. Run `mlb fetch odds` first.");
        return Ok(());
    }

    let mut games: Vec<GameExport>     = Vec::new();
    let mut arbs:  Vec<ArbExport>      = Vec::new();
    let mut best_evs: Vec<BestEvExport> = Vec::new();
    // (event_id, market, player, point) -> bookmaker -> (over_odds, under_odds)
    let mut prop_groups: HashMap<(String, MarketType, String, Option<String>), HashMap<String, (Option<f64>, Option<f64>)>> = HashMap::new();

    for event in &events {
        let raw_odds = match db.get_latest_odds(&event.id).await {
            Ok(o)  => o,
            Err(_) => continue,
        };

        // Defensively restrict to FanDuel/DraftKings only, in case older
        // snapshots in the DB included other bookmakers.
        let odds = EventOdds {
            event_id: raw_odds.event_id,
            captured_at: raw_odds.captured_at,
            bookmakers: raw_odds
                .bookmakers
                .into_iter()
                .filter(|b| ALLOWED_BOOKMAKERS.contains(&b.key.as_str()))
                .collect(),
        };

        let best_home = odds.best_odds(MarketType::H2h, &event.home_team).map(|(b, o)| BestOddsExport {
            team: event.home_team.clone(),
            odds: o.price.to_string().parse::<f64>().unwrap_or(0.0),
            bookmaker: b.title.clone(),
        });
        let best_away = odds.best_odds(MarketType::H2h, &event.away_team).map(|(b, o)| BestOddsExport {
            team: event.away_team.clone(),
            odds: o.price.to_string().parse::<f64>().unwrap_or(0.0),
            bookmaker: b.title.clone(),
        });

        let bookmakers: Vec<BookmakerExport> = odds.bookmakers.iter().map(|b| BookmakerExport {
            key: b.key.clone(),
            title: b.title.clone(),
            markets: b.markets.iter().filter(|m| !m.key.is_player_prop()).map(|m| MarketExport {
                key: m.key.to_string(),
                outcomes: m.outcomes.iter().map(|o| OutcomeExport {
                    name: o.name.clone(),
                    price: o.price.to_string().parse::<f64>().unwrap_or(0.0),
                    point: o.point.and_then(|p| p.to_string().parse::<f64>().ok()),
                }).collect(),
            }).collect(),
        }).collect();

        games.push(GameExport {
            id: event.id.clone(),
            home_team: event.home_team.clone(),
            away_team: event.away_team.clone(),
            matchup: event.matchup(),
            commence_time: event.commence_time.format("%a %b %d %I:%M %p UTC").to_string(),
            commence_timestamp: event.commence_time.timestamp(),
            bookmakers,
            best_home_odds: best_home,
            best_away_odds: best_away,
        });

        for arb in scan_for_arbitrage(&odds) {
            arbs.push(ArbExport {
                event_id: event.id.clone(),
                matchup: event.matchup(),
                profit_pct: arb.profit_percentage.to_string().parse::<f64>().unwrap_or(0.0) * 100.0,
                legs: arb.legs.iter().map(|leg| ArbLegExport {
                    outcome: leg.outcome.clone(),
                    bookmaker: leg.bookmaker.clone(),
                    odds: leg.odds.to_string().parse::<f64>().unwrap_or(0.0),
                    stake_pct: leg.stake_percentage.to_string().parse::<f64>().unwrap_or(0.0) * 100.0,
                }).collect(),
            });
        }

        for bet in scan_best_ev(&event.id, &odds) {
            // Only surface genuinely positive-EV bets — with just two sharp
            // books (FanDuel/DraftKings) sharing similar vig, most lines
            // legitimately have no edge, and it would be misleading to
            // present negative-EV bets as "opportunities."
            if bet.ev_pct <= rust_decimal::Decimal::ZERO {
                continue;
            }
            best_evs.push(BestEvExport {
                event_id: event.id.clone(),
                matchup: event.matchup(),
                market: bet.market.to_string(),
                outcome: bet.outcome_name.clone(),
                player: bet.player.clone(),
                point: bet.point.and_then(|p| p.to_string().parse::<f64>().ok()),
                bookmaker: bet.bookmaker_title.clone(),
                odds: bet.odds.to_string().parse::<f64>().unwrap_or(0.0),
                true_probability: bet.true_probability.to_string().parse::<f64>().unwrap_or(0.0),
                implied_probability: bet.implied_probability.to_string().parse::<f64>().unwrap_or(0.0),
                edge: bet.edge.to_string().parse::<f64>().unwrap_or(0.0),
                ev_pct: bet.ev_pct.to_string().parse::<f64>().unwrap_or(0.0),
            });
        }

        // Player props: group Over/Under outcomes per player+line across books.
        for book in &odds.bookmakers {
            for market in &book.markets {
                if !market.key.is_player_prop() {
                    continue;
                }
                for outcome in &market.outcomes {
                    let player = match &outcome.description {
                        Some(p) => p.clone(),
                        None => continue,
                    };
                    let point_key = outcome.point.map(|p| p.to_string());
                    let group_key = (event.id.clone(), market.key, player, point_key);
                    let price = outcome.price.to_string().parse::<f64>().unwrap_or(0.0);

                    let entry = prop_groups.entry(group_key).or_default()
                        .entry(book.title.clone()).or_insert((None, None));

                    match outcome.name.as_str() {
                        "Over"  => entry.0 = Some(price),
                        "Under" => entry.1 = Some(price),
                        _ => {}
                    }
                }
            }
        }
    }

    let mut props: Vec<PropExport> = Vec::new();
    for ((event_id, market, player, point_key), books) in &prop_groups {
        let matchup = games.iter().find(|g| &g.id == event_id).map(|g| g.matchup.clone()).unwrap_or_default();
        props.push(PropExport {
            event_id: event_id.clone(),
            matchup,
            market: market.to_string(),
            player: player.clone(),
            point: point_key.as_ref().and_then(|p| p.parse::<f64>().ok()),
            books: books.iter().map(|(bookmaker, (over_odds, under_odds))| PropBookExport {
                bookmaker: bookmaker.clone(),
                over_odds: *over_odds,
                under_odds: *under_odds,
            }).collect(),
        });
    }

    best_evs.sort_by(|a, b| b.ev_pct.partial_cmp(&a.ev_pct).unwrap_or(std::cmp::Ordering::Equal));
    props.sort_by(|a, b| a.matchup.cmp(&b.matchup).then(a.player.cmp(&b.player)));

    let arb_count     = arbs.len();
    let total_games   = games.len();
    let best_ev_count = best_evs.len();
    let prop_count    = props.len();

    let meta = MetaExport {
        exported_at: chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
        total_games,
        arb_count,
        best_ev_count,
        prop_count,
        requests_remaining: None,
    };

    fs::write(out_dir.join("games.json"),   serde_json::to_string_pretty(&games)?)?;
    fs::write(out_dir.join("arb.json"),     serde_json::to_string_pretty(&arbs)?)?;
    fs::write(out_dir.join("best_ev.json"), serde_json::to_string_pretty(&best_evs)?)?;
    fs::write(out_dir.join("props.json"),   serde_json::to_string_pretty(&props)?)?;
    fs::write(out_dir.join("meta.json"),    serde_json::to_string_pretty(&meta)?)?;

    println!(
        "Exported {} games, {} arb opportunities, {} best-EV bets, {} prop lines -> {}",
        total_games, arb_count, best_ev_count, prop_count, out_dir.display()
    );
    println!("\nNext: cd bins/mlb-ui && npm run dev");

    Ok(())
}
