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
use mlb_predict::{build_history_summary, compute_game_edge, find_standing_by_team_name, get_or_fetch_standings, model_ev_pct, project_prop, stat_group_for_market};
use mlb_stats_client::StatsApiClient;

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

    // ── Model win probability (Pythagorean win expectation + Log5) ──
    // Standings are cheap to fetch (~30 teams, one HTTP call) so unlike
    // player history below, this is allowed to live-fetch-and-cache
    // inline if the cache is stale/missing.
    let stats_client = StatsApiClient::new();
    let season = { use chrono::Datelike; Utc::now().year() };
    if let Ok(standings) = get_or_fetch_standings(db, &stats_client, season).await {
        let mut edge_rows: Vec<String> = Vec::new();
        for (event, odds) in &all_odds {
            let Some(home) = find_standing_by_team_name(&standings, &event.home_team) else { continue };
            let Some(away) = find_standing_by_team_name(&standings, &event.away_team) else { continue };
            if let Some(edge) = compute_game_edge(&event.home_team, &event.away_team, home, away, odds) {
                edge_rows.push(format!(
                    "- {} | {} ({}-{}): model {:.1}% vs market fair {:.1}% (edge {:+.1}pp) | {} ({}-{}): model {:.1}% vs market fair {:.1}% (edge {:+.1}pp)",
                    event.matchup(),
                    event.home_team,
                    home.wins,
                    home.losses,
                    edge.model_home_win_pct * 100.0,
                    edge.market_home_fair_pct * 100.0,
                    edge.home_edge_pct,
                    event.away_team,
                    away.wins,
                    away.losses,
                    edge.model_away_win_pct * 100.0,
                    edge.market_away_fair_pct * 100.0,
                    edge.away_edge_pct,
                ));
            }
        }
        if !edge_rows.is_empty() {
            let _ = writeln!(text, "\n=== MODEL WIN PROBABILITY (Pythagorean win expectation + Log5, sabermetric estimate) ===");
            let _ = writeln!(text, "This is NOT a guarantee — it's a statistical projection from each team's season runs scored/allowed, compared against the market's de-vigged moneyline probability. A positive edge means the model favors that side more than the market does.");
            for row in &edge_rows {
                let _ = writeln!(text, "{}", row);
            }
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
    // Rows are collected per (event, market) bucket — not into one flat
    // list, and not just per-event either — so that a fair round-robin
    // interleave can be applied before the MAX_PROP_ROWS cap is enforced
    // below. Grouping by event alone isn't enough: within a single game,
    // a market with many players (e.g. Hits, ~15 rows) would still fully
    // consume that game's share of the budget before a market with fewer
    // players (e.g. Stolen Bases, ~6 rows) ever got a row in — causing
    // the LLM to falsely claim a game "has no props" for a market that
    // genuinely exists in the data. Keying by (event, market) guarantees
    // every market in every game gets a fair shot at the row budget.
    let mut prop_rows_by_bucket: Vec<((String, MarketType), Vec<String>)> = Vec::new();
    // (player_name, market, event) they appear in, so we can resolve the
    // history summary (trend + opponent matchup) below.
    let mut history_targets: Vec<(String, MarketType, Event)> = Vec::new();
    for (event, odds) in &all_odds {
        for book in &odds.bookmakers {
            for market in &book.markets {
                if !market.key.is_player_prop() {
                    continue;
                }
                let bucket_key = (event.id.clone(), market.key);
                let bucket_idx = match prop_rows_by_bucket.iter().position(|(k, _)| k == &bucket_key) {
                    Some(idx) => idx,
                    None => {
                        prop_rows_by_bucket.push((bucket_key, Vec::new()));
                        prop_rows_by_bucket.len() - 1
                    }
                };
                let mut by_player_point: std::collections::HashMap<(String, String), Vec<&mlb_core::models::Outcome>> =
                    std::collections::HashMap::new();
                for o in &market.outcomes {
                    let player = o.description.clone().unwrap_or_default();
                    // Drop lines outside a realistic range for this market
                    // (e.g. a pitcher strikeout prop posted at 11.5) —
                    // likely upstream data glitches that shouldn't be
                    // presented to the LLM as real, bettable lines.
                    if let Some(pt) = o.point {
                        if !mlb_core::analysis::is_plausible_prop_point(market.key, pt) {
                            continue;
                        }
                        if o.name == "Over" && !mlb_core::analysis::is_plausible_prop_odds(market.key, pt, o.price) {
                            continue;
                        }
                    }
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
                        // Recent-form Poisson projection for this exact
                        // line — gives a model-implied edge even though
                        // only one bookmaker quotes most prop lines (so
                        // there's no cross-book consensus to compare
                        // against). Cache-only, never hits the live API.
                        let mut model_str = String::new();
                        if let Some(point_f64) = o.point.and_then(|p| p.to_string().parse::<f64>().ok()) {
                            if let Some(proj) = project_prop(db, &player, market.key, point_f64).await {
                                let over_price = o.price.to_string().parse::<f64>().unwrap_or(0.0);
                                let under_price = u.price.to_string().parse::<f64>().unwrap_or(0.0);
                                let over_ev = model_ev_pct(proj.over_probability, over_price) * 100.0;
                                let under_ev = model_ev_pct(proj.under_probability, under_price) * 100.0;
                                let (best_side, best_ev, best_price, best_prob) = if over_ev >= under_ev {
                                    ("Over", over_ev, over_price, proj.over_probability)
                                } else {
                                    ("Under", under_ev, under_price, proj.under_probability)
                                };
                                let caution = if mlb_predict::is_high_divergence(best_prob, best_price) { " [LARGE GAP - verify lineup]" } else { "" };
                                let _ = write!(
                                    model_str,
                                    " | model proj {:.1} ({}g) -> {} model-EV {:+.1}%{}",
                                    proj.projected_value, proj.sample_games, best_side, best_ev, caution
                                );
                            }
                        }
                        prop_rows_by_bucket[bucket_idx].1.push(format!(
                            "{} | {} | {} {} O{:+} / U{:+} @ {}{}",
                            event.matchup(),
                            market.key,
                            player,
                            o.point.map(|p| p.to_string()).unwrap_or_default(),
                            o.price,
                            u.price,
                            book.title,
                            model_str,
                        ));
                        if stat_group_for_market(market.key).is_some() {
                            history_targets.push((player.clone(), market.key, event.clone()));
                        }
                    }
                }
            }
        }
    }

    // Round-robin interleave across (event, market) buckets — one row per
    // bucket per pass — so that every market in every game gets fair
    // representation up to MAX_PROP_ROWS, instead of a high-player-count
    // market (e.g. Hits) consuming a game's whole share of the budget and
    // starving out a low-player-count market (e.g. Stolen Bases) in that
    // same game.
    let total_prop_rows: usize = prop_rows_by_bucket.iter().map(|(_, rows)| rows.len()).sum();
    let mut prop_rows: Vec<String> = Vec::with_capacity(total_prop_rows.min(MAX_PROP_ROWS));
    let mut cursors = vec![0usize; prop_rows_by_bucket.len()];
    'rr: loop {
        let mut made_progress = false;
        for (i, (_, rows)) in prop_rows_by_bucket.iter().enumerate() {
            if cursors[i] < rows.len() {
                prop_rows.push(rows[cursors[i]].clone());
                cursors[i] += 1;
                made_progress = true;
                if prop_rows.len() >= MAX_PROP_ROWS {
                    break 'rr;
                }
            }
        }
        if !made_progress {
            break;
        }
    }

    let _ = writeln!(text, "\n=== PLAYER PROPS (FanDuel + DraftKings) ===");
    let _ = writeln!(text, "Format: matchup | market | player line O/U odds @ book | model proj X (Ng) -> Side model-EV +Y%");
    let _ = writeln!(text, "\"model proj\" is a recent-form projection (average of the player's last N cached games for that exact stat) fed through a Poisson model to estimate the true Over/Under probability against the posted line — independent of a second bookmaker. Use it to justify \"alternate\"-style recommendations, e.g. if the line is 5.5 and model proj is 9.0, you can say the model projects well above the line and the Over is priced favorably.");
    if total_prop_rows > MAX_PROP_ROWS {
        let num_games = all_odds.len();
        let _ = writeln!(text, "(showing {} of {} total prop rows, fairly sampled across every market in all {} games so nothing is starved out)", prop_rows.len(), total_prop_rows, num_games);
    }
    for row in &prop_rows {
        let _ = writeln!(text, "{}", row);
    }
    if prop_rows.is_empty() {
        let _ = writeln!(text, "(none loaded — run `mlb fetch odds --with-props` first)");
    }

    // ── Player history: cached MLB Stats API season trend + matchup ─
    // Read-only lookups against the local cache only — never calls out
    // to statsapi.mlb.com during a chat request, so this stays fast.
    // Populate via `mlb history sync-props` / `mlb history fetch`.
    // Shared with the CLI export (dashboard Props tab) via
    // `mlb_predict::build_history_summary`, so the two surfaces never
    // drift out of sync.
    //
    // Dedupe (player, market) targets first, then group by market and
    // round-robin across markets before applying MAX_HISTORY_PLAYERS —
    // otherwise a market with many players (e.g. Hits, ~15+ players
    // across a slate) can consume the whole budget before a market with
    // fewer players (e.g. Stolen Bases) ever gets a single player's
    // cached history rendered, even though that player's data is fully
    // cached and already used elsewhere (e.g. the model-proj tag). This
    // is the same fairness bug already fixed above for MAX_PROP_ROWS.
    let mut seen: HashSet<(String, MarketType)> = HashSet::new();
    let mut targets_by_market: Vec<(MarketType, Vec<(String, Event)>)> = Vec::new();
    for (player, market, event) in history_targets {
        let key = (player.to_lowercase(), market);
        if !seen.insert(key) {
            continue;
        }
        let bucket_idx = match targets_by_market.iter().position(|(m, _)| *m == market) {
            Some(idx) => idx,
            None => {
                targets_by_market.push((market, Vec::new()));
                targets_by_market.len() - 1
            }
        };
        targets_by_market[bucket_idx].1.push((player, event));
    }

    let mut selected_targets: Vec<(String, MarketType, Event)> = Vec::new();
    let mut hist_cursors = vec![0usize; targets_by_market.len()];
    'hist_rr: loop {
        let mut made_progress = false;
        for (i, (market, players)) in targets_by_market.iter().enumerate() {
            if hist_cursors[i] < players.len() {
                let (player, event) = players[hist_cursors[i]].clone();
                selected_targets.push((player, *market, event));
                hist_cursors[i] += 1;
                made_progress = true;
                if selected_targets.len() >= MAX_HISTORY_PLAYERS {
                    break 'hist_rr;
                }
            }
        }
        if !made_progress {
            break;
        }
    }

    let mut history_rows: Vec<String> = Vec::new();
    for (player, market, event) in selected_targets {
        let Some(summary) = build_history_summary(db, &player, market, &event.home_team, &event.away_team).await else {
            continue;
        };

        let mut block = String::new();
        let _ = write!(block, "{} ({})", summary.player_name, market);

        if let Some(trend) = &summary.trend {
            let series: Vec<String> = trend.points.iter().map(|(season, rate)| format!("{}:{:.2}", season, rate)).collect();
            let _ = write!(block, " | {} {} -> {}", trend.stat_label, series.join(","), trend.direction);
        }

        if let Some(opponent_name) = &summary.opponent_name {
            if !summary.last_meetings.is_empty() {
                let _ = write!(block, " | last {} vs {}:", summary.last_meetings.len(), opponent_name);
                for meeting in &summary.last_meetings {
                    let _ = write!(block, " [{} {}]", meeting.date, meeting.line);
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
