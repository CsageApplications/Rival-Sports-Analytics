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
use mlb_predict::{build_history_summary, compute_game_edge, find_standing_by_team_name, get_or_fetch_standings, is_high_divergence, model_ev_pct, project_prop, PlayerHistorySummary, PropProjection};
use mlb_stats_client::StatsApiClient;
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
    #[serde(default)]
    pub prediction_count: usize,
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
    /// "Consensus" (FanDuel vs. DraftKings cross-book de-vig agreement)
    /// or "Model Projection" (single-book price vs. a recent-form Poisson
    /// projection). Lets the dashboard label/filter the two fundamentally
    /// different kinds of edge estimate.
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "Consensus".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct PropBookExport {
    pub bookmaker: String,
    pub over_odds: Option<f64>,
    pub under_odds: Option<f64>,
    /// Model-implied EV% (recent-form Poisson projection vs. this book's
    /// actual Over price), as a fraction of stake (0.08 = +8%). `None` if
    /// there's no model projection or no Over price at this book.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub over_ev_pct: Option<f64>,
    /// Same as `over_ev_pct` but for the Under side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub under_ev_pct: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct PropExport {
    pub event_id: String,
    pub matchup: String,
    pub market: String,
    pub player: String,
    pub point: Option<f64>,
    pub books: Vec<PropBookExport>,
    /// Cached MLB Stats API season trend + last-3-meetings history for
    /// this player, if available. `None` if the player isn't cached yet
    /// (run `mlb history sync-props`) or the market has no associated
    /// stat group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<PlayerHistorySummary>,
    /// Recent-form (last N games) projection for this exact market/line,
    /// derived from a Poisson count model — independent of whether a
    /// second bookmaker exists to build a cross-book consensus. `None` if
    /// the player doesn't have enough cached recent-game data yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<PropProjection>,
    /// The single best model-implied EV% across all books/sides for this
    /// prop line, for convenient sorting. `None` if there's no projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_model_ev_pct: Option<f64>,
    /// Which side ( "Over" / "Under") produced `best_model_ev_pct`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_model_side: Option<String>,
    /// `true` if the model's probability estimate for `best_model_side`
    /// diverges sharply (>30pp) from the market's implied probability.
    /// A real market disagreeing with a simple recent-form model this
    /// much is more likely explained by missing context (lineup/role
    /// status) than genuine free value — the dashboard should caution
    /// the user to verify before betting rather than presenting it as a
    /// confident edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_model_high_divergence: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct PredictionExport {
    pub event_id: String,
    pub matchup: String,
    pub home_team: String,
    pub away_team: String,
    pub home_record: String,
    pub away_record: String,
    /// Model-projected win probability (Pythagorean win expectation +
    /// Log5 + home-field advantage), 0.0-1.0.
    pub model_home_win_pct: f64,
    pub model_away_win_pct: f64,
    /// Market's de-vigged "fair" moneyline probability, 0.0-1.0.
    pub market_home_fair_pct: f64,
    pub market_away_fair_pct: f64,
    /// Percentage points: model minus market.
    pub home_edge_pct: f64,
    pub away_edge_pct: f64,
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
    let mut predictions: Vec<PredictionExport> = Vec::new();
    // (event_id, market, player, point) -> bookmaker -> (over_odds, under_odds)
    let mut prop_groups: HashMap<(String, MarketType, String, Option<String>), HashMap<String, (Option<f64>, Option<f64>)>> = HashMap::new();
    // event_id -> (home_team, away_team), used later to resolve player
    // prop opponents for the history summary.
    let mut event_teams: HashMap<String, (String, String)> = HashMap::new();

    // Team standings (win/loss + runs scored/allowed) for the game-winner
    // projection model — one cheap fetch for the whole slate, cached for
    // STANDINGS_CACHE_TTL_HOURS in SQLite.
    let stats_client = StatsApiClient::new();
    let season = { use chrono::Datelike; chrono::Utc::now().year() };
    let standings = get_or_fetch_standings(&db, &stats_client, season).await.unwrap_or_default();
    if standings.is_empty() {
        println!("  Warning: could not load team standings for season {} — predictions.json will be empty.", season);
    }

    for event in &events {
        event_teams.insert(event.id.clone(), (event.home_team.clone(), event.away_team.clone()));

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

        // Game-winner projection: Pythagorean win expectation + Log5 vs.
        // the market's de-vigged moneyline probability.
        if !standings.is_empty() {
            if let (Some(home), Some(away)) =
                (find_standing_by_team_name(&standings, &event.home_team), find_standing_by_team_name(&standings, &event.away_team))
            {
                if let Some(edge) = compute_game_edge(&event.home_team, &event.away_team, home, away, &odds) {
                    predictions.push(PredictionExport {
                        event_id: event.id.clone(),
                        matchup: event.matchup(),
                        home_team: event.home_team.clone(),
                        away_team: event.away_team.clone(),
                        home_record: format!("{}-{}", home.wins, home.losses),
                        away_record: format!("{}-{}", away.wins, away.losses),
                        model_home_win_pct: edge.model_home_win_pct,
                        model_away_win_pct: edge.model_away_win_pct,
                        market_home_fair_pct: edge.market_home_fair_pct,
                        market_away_fair_pct: edge.market_away_fair_pct,
                        home_edge_pct: edge.home_edge_pct,
                        away_edge_pct: edge.away_edge_pct,
                    });
                }
            }
        }

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
                source: default_source(),
            });
        }

        // Player props: group Over/Under outcomes per player+line across books.
        // Lines outside a realistic range for the market (e.g. a pitcher
        // strikeout prop posted at 11.5) are dropped as likely
        // upstream-data glitches rather than presented as real.
        let mut implausible_skipped = 0usize;
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
                    if let Some(point) = outcome.point {
                        if !mlb_core::analysis::is_plausible_prop_point(market.key, point) {
                            implausible_skipped += 1;
                            continue;
                        }
                        if outcome.name == "Over"
                            && !mlb_core::analysis::is_plausible_prop_odds(market.key, point, outcome.price)
                        {
                            implausible_skipped += 1;
                            continue;
                        }
                    }
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
        if implausible_skipped > 0 {
            println!(
                "  Warning: skipped {} implausible prop outcome(s) for {} (likely bad upstream data)",
                implausible_skipped,
                event.matchup()
            );
        }
    }

    let mut props: Vec<PropExport> = Vec::new();
    for ((event_id, market, player, point_key), books) in &prop_groups {
        let matchup = games.iter().find(|g| &g.id == event_id).map(|g| g.matchup.clone()).unwrap_or_default();
        let point = point_key.as_ref().and_then(|p| p.parse::<f64>().ok());

        // Attach cached MLB Stats API season trend + last-3-meetings
        // history, if available. Cache-only (never calls out to
        // statsapi.mlb.com), so this is safe/fast even with hundreds of
        // prop lines.
        let history = match event_teams.get(event_id) {
            Some((home_team, away_team)) => build_history_summary(&db, player, *market, home_team, away_team).await,
            None => None,
        };

        // Recent-form Poisson projection for this exact line — gives every
        // prop a model-implied edge estimate even when only one bookmaker
        // quotes it (the common case for player props in this data set).
        let projection = match point {
            Some(pt) => project_prop(&db, player, *market, pt).await,
            None => None,
        };

        let books_export: Vec<PropBookExport> = books.iter().map(|(bookmaker, (over_odds, under_odds))| {
            let over_ev_pct = match (over_odds, &projection) {
                (Some(price), Some(proj)) => Some(model_ev_pct(proj.over_probability, *price)),
                _ => None,
            };
            let under_ev_pct = match (under_odds, &projection) {
                (Some(price), Some(proj)) => Some(model_ev_pct(proj.under_probability, *price)),
                _ => None,
            };
            PropBookExport {
                bookmaker: bookmaker.clone(),
                over_odds: *over_odds,
                under_odds: *under_odds,
                over_ev_pct,
                under_ev_pct,
            }
        }).collect();

        let mut best_model_ev_pct: Option<f64> = None;
        let mut best_model_side: Option<String> = None;
        let mut best_model_high_divergence: Option<bool> = None;
        let mut best_model_bookmaker: Option<String> = None;
        let mut best_model_odds: Option<f64> = None;
        let mut best_model_probability: Option<f64> = None;
        if let Some(proj) = &projection {
            for b in &books_export {
                if let Some(over_price) = b.over_odds {
                    if let Some(ev) = b.over_ev_pct {
                        if best_model_ev_pct.map_or(true, |cur| ev > cur) {
                            best_model_ev_pct = Some(ev);
                            best_model_side = Some("Over".to_string());
                            best_model_high_divergence = Some(is_high_divergence(proj.over_probability, over_price));
                            best_model_bookmaker = Some(b.bookmaker.clone());
                            best_model_odds = Some(over_price);
                            best_model_probability = Some(proj.over_probability);
                        }
                    }
                }
                if let Some(under_price) = b.under_odds {
                    if let Some(ev) = b.under_ev_pct {
                        if best_model_ev_pct.map_or(true, |cur| ev > cur) {
                            best_model_ev_pct = Some(ev);
                            best_model_side = Some("Under".to_string());
                            best_model_high_divergence = Some(is_high_divergence(proj.under_probability, under_price));
                            best_model_bookmaker = Some(b.bookmaker.clone());
                            best_model_odds = Some(under_price);
                            best_model_probability = Some(proj.under_probability);
                        }
                    }
                }
            }
        }

        // Surface genuinely positive, non-high-divergence model edges in
        // the Best EV tab too — cross-book consensus (FanDuel vs.
        // DraftKings agreeing on the exact same line) is rare for player
        // props, since most lines are only quoted by one book. The
        // recent-form model gives every prop its own edge estimate
        // regardless, so Best EV shouldn't be limited to the much
        // narrower cross-book case.
        if let (Some(ev), Some(bookmaker), Some(odds), Some(probability), false) = (
            best_model_ev_pct,
            &best_model_bookmaker,
            best_model_odds,
            best_model_probability,
            best_model_high_divergence.unwrap_or(false),
        ) {
            if ev > 0.0 {
                let implied = if odds >= 0.0 { 100.0 / (odds + 100.0) } else { odds.abs() / (odds.abs() + 100.0) };
                best_evs.push(BestEvExport {
                    event_id: event_id.clone(),
                    matchup: games.iter().find(|g| &g.id == event_id).map(|g| g.matchup.clone()).unwrap_or_default(),
                    market: market.to_string(),
                    outcome: best_model_side.clone().unwrap_or_default(),
                    player: Some(player.clone()),
                    point,
                    bookmaker: bookmaker.clone(),
                    odds,
                    true_probability: probability,
                    implied_probability: implied,
                    edge: probability - implied,
                    ev_pct: ev * 100.0,
                    source: "Model Projection".to_string(),
                });
            }
        }

        props.push(PropExport {
            event_id: event_id.clone(),
            matchup,
            market: market.to_string(),
            player: player.clone(),
            point,
            books: books_export,
            history,
            projection,
            best_model_ev_pct,
            best_model_side,
            best_model_high_divergence,
        });
    }

    best_evs.sort_by(|a, b| b.ev_pct.partial_cmp(&a.ev_pct).unwrap_or(std::cmp::Ordering::Equal));
    props.sort_by(|a, b| a.matchup.cmp(&b.matchup).then(a.player.cmp(&b.player)));
    predictions.sort_by(|a, b| a.matchup.cmp(&b.matchup));

    let arb_count     = arbs.len();
    let total_games   = games.len();
    let best_ev_count = best_evs.len();
    let prop_count    = props.len();
    let prediction_count = predictions.len();

    let meta = MetaExport {
        exported_at: chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
        total_games,
        arb_count,
        best_ev_count,
        prop_count,
        prediction_count,
        requests_remaining: None,
    };

    fs::write(out_dir.join("games.json"),       serde_json::to_string_pretty(&games)?)?;
    fs::write(out_dir.join("arb.json"),         serde_json::to_string_pretty(&arbs)?)?;
    fs::write(out_dir.join("best_ev.json"),     serde_json::to_string_pretty(&best_evs)?)?;
    fs::write(out_dir.join("props.json"),       serde_json::to_string_pretty(&props)?)?;
    fs::write(out_dir.join("predictions.json"), serde_json::to_string_pretty(&predictions)?)?;
    fs::write(out_dir.join("meta.json"),        serde_json::to_string_pretty(&meta)?)?;

    println!(
        "Exported {} games, {} arb opportunities, {} best-EV bets, {} prop lines, {} game predictions -> {}",
        total_games, arb_count, best_ev_count, prop_count, prediction_count, out_dir.display()
    );
    println!("\nNext: cd bins/mlb-ui && npm run dev");

    Ok(())
}
