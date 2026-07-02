//! Builds a compact, LLM-friendly context blob from the current database
//! state (upcoming games, best-EV picks, player props) so Claude has real
//! numbers to reason over when building parlays.

use chrono::Utc;
use std::collections::HashSet;
use std::fmt::Write as _;

use mlb_core::analysis::{scan_best_ev, scan_for_arbitrage, BestEvBet};
use mlb_core::models::{Event, EventOdds, MarketType};
use mlb_core::repository::{EventRepository, OddsRepository};
use mlb_db::Database;
use mlb_predict::{get_cached_report, last_n_meetings, linear_regression_trend, stats, SeasonPoint};
use mlb_stats_client::{find_team_id_by_name, StatGroup};

/// Maximum number of best-EV rows and prop lines fed into the prompt.
/// Keeps token usage/cost bounded regardless of slate size.
const MAX_BEST_EV_ROWS: usize = 60;
const MAX_PROP_ROWS: usize = 120;
/// Maximum number of distinct players to render cached history for.
const MAX_HISTORY_PLAYERS: usize = 25;

pub struct SlateContext {
    pub games_count: usize,
    pub text: String,
}

/// Pull all upcoming events + their latest odds snapshot, run the same
/// analysis the dashboard uses (best EV, arbitrage), and render everything
/// as a compact plain-text block suitable for a system/user prompt.
pub async fn build_slate_context(db: &Database) -> anyhow::Result<SlateContext> {
    let events = db.get_upcoming_events().await?;

    let mut all_odds: Vec<(Event, EventOdds)> = Vec::new();
    for event in events {
        if let Ok(odds) = db.get_latest_odds(&event.id).await {
            all_odds.push((event, odds));
        }
    }

    let mut text = String::new();
    let _ = writeln!(
        text,
        "Data snapshot generated {}. Bookmakers covered: FanDuel, DraftKings only.\n",
        Utc::now().format("%Y-%m-%d %H:%M UTC")
    );

    // ── Games + moneyline ────────────────────────────────────────────
    let _ = writeln!(text, "=== UPCOMING GAMES ===");
    for (event, odds) in &all_odds {
        let best_home = odds.best_odds(MarketType::H2h, &event.home_team);
        let best_away = odds.best_odds(MarketType::H2h, &event.away_team);
        let _ = write!(text, "- {} ({})", event.matchup(), event.commence_time.format("%a %b %d %I:%M %p UTC"));
        if let Some((book, out)) = best_away {
            let _ = write!(text, " | {} {:+} @ {}", event.away_team, out.price, book.title);
        }
        if let Some((book, out)) = best_home {
            let _ = write!(text, " | {} {:+} @ {}", event.home_team, out.price, book.title);
        }
        let _ = writeln!(text);
    }

    // ── Arbitrage (rare, but surface if present) ────────────────────
    let mut any_arb = false;
    for (event, odds) in &all_odds {
        for arb in scan_for_arbitrage(odds) {
            if !any_arb {
                let _ = writeln!(text, "\n=== ARBITRAGE OPPORTUNITIES ===");
                any_arb = true;
            }
            let _ = write!(text, "- {} | profit {:.2}% |", event.matchup(), arb.profit_percentage * rust_decimal::Decimal::from(100));
            for leg in &arb.legs {
                let _ = write!(text, " {} @ {} ({:+}, stake {:.1}%)", leg.outcome, leg.bookmaker, leg.odds, leg.stake_percentage * rust_decimal::Decimal::from(100));
            }
            let _ = writeln!(text);
        }
    }

    // ── Best EV across moneyline/spread/totals ──────────────────────
    let mut best_ev_rows: Vec<(String, BestEvBet)> = Vec::new();
    for (event, odds) in &all_odds {
        for bet in scan_best_ev(&event.id, odds) {
            best_ev_rows.push((event.matchup(), bet));
        }
    }
    best_ev_rows.sort_by(|a, b| b.1.ev_pct.cmp(&a.1.ev_pct));

    let _ = writeln!(text, "\n=== BEST EXPECTED VALUE (consensus fair-odds model, ranked) ===");
    let _ = writeln!(text, "Format: EV% | matchup | market | selection (line) | book | american odds | fair% | implied%");
    for (matchup, bet) in best_ev_rows.iter().take(MAX_BEST_EV_ROWS) {
        let point_str = bet.point.map(|p| format!(" ({:+})", p)).unwrap_or_default();
        let _ = writeln!(
            text,
            "{:+.2}% | {} | {} | {}{} | {} | {:+} | fair {:.1}% | implied {:.1}%",
            bet.ev_pct,
            matchup,
            bet.market,
            bet.outcome_name,
            point_str,
            bet.bookmaker_title,
            bet.odds,
            bet.true_probability * rust_decimal::Decimal::from(100),
            bet.implied_probability * rust_decimal::Decimal::from(100),
        );
    }
    if best_ev_rows.is_empty() {
        let _ = writeln!(text, "(none — no priced edges found across current lines)");
    }

    // ── Player props: group Over/Under per player+line across books ─
    let mut prop_rows: Vec<String> = Vec::new();
    // (player_name, stat_group) -> the event they appear in, so we can
    // resolve "who is the opponent" for matchup history below.
    let mut history_targets: Vec<(String, StatGroup, Event)> = Vec::new();
    for (event, odds) in &all_odds {
        for book in &odds.bookmakers {
            for market in &book.markets {
                if !market.key.is_player_prop() {
                    continue;
                }
                let mut by_player_point: std::collections::HashMap<(String, String), Vec<&mlb_core::models::Outcome>> =
                    std::collections::HashMap::new();
                for o in &market.outcomes {
                    let player = o.description.clone().unwrap_or_default();
                    let point = o.point.map(|p| p.to_string()).unwrap_or_default();
                    by_player_point.entry((player, point)).or_default().push(o);
                }
                for ((player, _point), outs) in by_player_point {
                    if player.is_empty() {
                        continue;
                    }
                    let over = outs.iter().find(|o| o.name == "Over");
                    let under = outs.iter().find(|o| o.name == "Under");
                    if let (Some(o), Some(u)) = (over, under) {
                        prop_rows.push(format!(
                            "{} | {} | {} {} O{:+} / U{:+} @ {}",
                            event.matchup(),
                            market.key,
                            player,
                            o.point.map(|p| p.to_string()).unwrap_or_default(),
                            o.price,
                            u.price,
                            book.title,
                        ));
                        if let Some(group) = market_to_stat_group(market.key) {
                            history_targets.push((player.clone(), group, event.clone()));
                        }
                    }
                }
            }
        }
    }

    let _ = writeln!(text, "\n=== PLAYER PROPS (FanDuel + DraftKings) ===");
    let _ = writeln!(text, "Format: matchup | market | player line O/U odds @ book");
    for row in prop_rows.iter().take(MAX_PROP_ROWS) {
        let _ = writeln!(text, "{}", row);
    }
    if prop_rows.is_empty() {
        let _ = writeln!(text, "(none loaded — run `mlb fetch odds --with-props` first)");
    }

    // ── Player history: cached MLB Stats API season trend + matchup ─
    // Read-only lookups against the local cache only — never calls out
    // to statsapi.mlb.com during a chat request, so this stays fast.
    // Populate via `mlb history sync-props` / `mlb history fetch`.
    let mut seen: HashSet<(String, StatGroup)> = HashSet::new();
    let mut history_rows: Vec<String> = Vec::new();
    for (player, group, event) in history_targets {
        if history_rows.len() >= MAX_HISTORY_PLAYERS {
            break;
        }
        let key = (player.to_lowercase(), group);
        if !seen.insert(key) {
            continue;
        }

        let Ok(Some(report)) = get_cached_report(db, &player, group, None).await else {
            continue;
        };

        let mut block = String::new();
        let _ = write!(block, "{} ({:?})", report.player_name, group);

        match group {
            StatGroup::Pitching => {
                if let Some(t) = trend_summary(&report.seasons, stats::k_per_9, true) {
                    let _ = write!(block, " | K/9 {}", t);
                }
            }
            StatGroup::Hitting => {
                if let Some(t) = trend_summary(&report.seasons, stats::hr_per_game, true) {
                    let _ = write!(block, " | HR/G {}", t);
                }
            }
        }

        // Resolve opponent: whichever of the event's two teams isn't the
        // player's own team (fuzzy-matched, since naming can differ
        // slightly between data sources).
        if let Some(team_name) = &report.team_name {
            let tn = team_name.to_lowercase();
            let opponent_name = if event.home_team.to_lowercase().contains(&tn) || tn.contains(&event.home_team.to_lowercase()) {
                Some(&event.away_team)
            } else if event.away_team.to_lowercase().contains(&tn) || tn.contains(&event.away_team.to_lowercase()) {
                Some(&event.home_team)
            } else {
                None
            };

            if let Some(opponent_name) = opponent_name {
                if let Some(opponent_id) = find_team_id_by_name(opponent_name) {
                    let meetings = last_n_meetings(&report.recent_games, opponent_id, 3);
                    if !meetings.is_empty() {
                        let _ = write!(block, " | last {} vs {}:", meetings.len(), opponent_name);
                        for g in &meetings {
                            match group {
                                StatGroup::Pitching => {
                                    let _ = write!(
                                        block,
                                        " [{} {:.1}IP {}K]",
                                        g.date,
                                        g.innings_pitched.unwrap_or(0.0),
                                        g.strikeouts.unwrap_or(0)
                                    );
                                }
                                StatGroup::Hitting => {
                                    let _ = write!(
                                        block,
                                        " [{} {}AB {}H {}HR]",
                                        g.date,
                                        g.at_bats.unwrap_or(0),
                                        g.hits.unwrap_or(0),
                                        g.home_runs.unwrap_or(0)
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        history_rows.push(block);
    }

    if !history_rows.is_empty() {
        let _ = writeln!(text, "\n=== PLAYER HISTORY (cached MLB Stats API data) ===");
        let _ = writeln!(text, "Season trend via linear regression on the rate stat; matchup games are most-recent-first.");
        for row in &history_rows {
            let _ = writeln!(text, "- {}", row);
        }
    }

    Ok(SlateContext {
        games_count: all_odds.len(),
        text,
    })
}

fn market_to_stat_group(m: MarketType) -> Option<StatGroup> {
    use MarketType::*;
    match m {
        BatterHits | BatterHomeRuns | BatterRbis | BatterStolenBases | BatterStrikeouts | BatterWalks => Some(StatGroup::Hitting),
        PitcherStrikeouts | PitcherHitsAllowed | PitcherWalks | PitcherEarnedRuns => Some(StatGroup::Pitching),
        _ => None,
    }
}

fn trend_summary(
    seasons: &[mlb_stats_client::SeasonStatLine],
    extractor: impl Fn(&mlb_stats_client::SeasonStatLine) -> Option<f64>,
    higher_is_better: bool,
) -> Option<String> {
    let points: Vec<SeasonPoint> = seasons.iter().filter_map(|s| extractor(s).map(|rate| SeasonPoint { season: s.season, rate })).collect();
    if points.is_empty() {
        return None;
    }
    let trend = linear_regression_trend(&points);
    let series: Vec<String> = trend.points.iter().map(|p| format!("{}:{:.1}", p.season, p.rate)).collect();
    let direction = trend.direction.judge(higher_is_better);
    Some(format!("{} -> {}", series.join(","), direction))
}
