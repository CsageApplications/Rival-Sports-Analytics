//! Historical player stats + matchup/trend analysis (MLB Stats API)

use anyhow::Result;
use clap::Subcommand;
use std::collections::HashSet;

use mlb_core::models::MarketType;
use mlb_core::repository::{EventRepository, OddsRepository};
use mlb_predict::stats;
use mlb_predict::{get_or_fetch_report, last_n_meetings, linear_regression_trend, SeasonPoint};
use mlb_stats_client::{find_team_id_by_name, StatGroup, StatsApiClient};

use super::AppContext;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum GroupArg {
    Pitching,
    Hitting,
}

impl GroupArg {
    fn to_stat_group(self) -> StatGroup {
        match self {
            GroupArg::Pitching => StatGroup::Pitching,
            GroupArg::Hitting => StatGroup::Hitting,
        }
    }
}

#[derive(Subcommand)]
pub enum HistoryCommands {
    /// Fetch + cache a player's season stats and recent game logs
    Fetch {
        /// Player full name, e.g. "Chase Burns"
        player: String,
        #[arg(long, value_enum, default_value_t = GroupArg::Pitching)]
        group: GroupArg,
        /// How many past seasons to pull
        #[arg(long, default_value_t = 4)]
        seasons: u32,
    },
    /// Show a player's last 3 meetings vs an opponent + season trend
    Matchup {
        /// Player full name, e.g. "Chase Burns"
        player: String,
        /// Opponent team name/fragment, e.g. "Astros"
        #[arg(long = "vs")]
        opponent: String,
        #[arg(long, value_enum, default_value_t = GroupArg::Pitching)]
        group: GroupArg,
    },
    /// List every player currently cached
    List,
    /// Fetch + cache history for every player currently in loaded player props
    SyncProps {
        #[arg(long, default_value_t = 4)]
        seasons: u32,
    },
}

impl HistoryCommands {
    pub async fn run(self) -> Result<()> {
        match self {
            HistoryCommands::Fetch { player, group, seasons } => fetch_player(&player, group, seasons).await,
            HistoryCommands::Matchup { player, opponent, group } => show_matchup(&player, &opponent, group).await,
            HistoryCommands::List => list_cached().await,
            HistoryCommands::SyncProps { seasons } => sync_props(seasons).await,
        }
    }
}

async fn fetch_player(player: &str, group: GroupArg, seasons: u32) -> Result<()> {
    let ctx = AppContext::new().await?;
    let stats_client = StatsApiClient::new();

    println!("Searching MLB Stats API for '{}'...", player);
    let resolved = stats_client.find_player(player).await?;
    println!("Found: {} (id {})\n", resolved.full_name, resolved.id);

    println!("Fetching last {} seasons of {:?} stats + game logs...", seasons, group.to_stat_group());
    let report = mlb_predict::fetch_and_cache_report(&ctx.db, &stats_client, &resolved, group.to_stat_group(), seasons).await?;

    println!(
        "Cached {} season(s) and {} game log entries for {}.",
        report.seasons.len(),
        report.recent_games.len(),
        report.player_name
    );
    Ok(())
}

async fn show_matchup(player: &str, opponent: &str, group: GroupArg) -> Result<()> {
    let ctx = AppContext::new().await?;
    let stats_client = StatsApiClient::new();
    let group = group.to_stat_group();

    let opponent_id = find_team_id_by_name(opponent)
        .ok_or_else(|| anyhow::anyhow!("Unrecognized MLB team '{}'. Try a full name like 'Houston Astros'.", opponent))?;

    let report = get_or_fetch_report(&ctx.db, &stats_client, player, group, 4).await?;

    println!("{} — {:?} History", report.player_name, group);
    println!("{}", "=".repeat(60));

    // ── Season trend ──────────────────────────────────────────────
    match group {
        StatGroup::Pitching => {
            print_trend("K/9", &report.seasons, stats::k_per_9, true);
            print_trend("ERA", &report.seasons, stats::era, false);
            print_trend("BB/9", &report.seasons, stats::walks_per_9, false);
        }
        StatGroup::Hitting => {
            print_trend("HR/G", &report.seasons, stats::hr_per_game, true);
            print_trend("AVG", &report.seasons, stats::batting_avg, true);
            print_trend("Hits/G", &report.seasons, stats::hits_per_game, true);
        }
    }

    // ── Matchup history ───────────────────────────────────────────
    println!("\nLast 3 meetings vs {}:", opponent);
    let meetings = last_n_meetings(&report.recent_games, opponent_id, 3);
    if meetings.is_empty() {
        println!("  No meetings found in the last {} seasons of game logs.", 4);
    } else {
        for g in &meetings {
            match group {
                StatGroup::Pitching => {
                    println!(
                        "  {}  {:.1} IP, {} K, {} BB, {} ER",
                        g.date,
                        g.innings_pitched.unwrap_or(0.0),
                        g.strikeouts.unwrap_or(0),
                        g.walks.unwrap_or(0),
                        g.earned_runs.unwrap_or(0),
                    );
                }
                StatGroup::Hitting => {
                    println!(
                        "  {}  {} AB, {} H, {} HR, {} RBI",
                        g.date,
                        g.at_bats.unwrap_or(0),
                        g.hits.unwrap_or(0),
                        g.home_runs.unwrap_or(0),
                        g.rbi.unwrap_or(0),
                    );
                }
            }
        }
    }

    Ok(())
}

fn print_trend(
    label: &str,
    seasons: &[mlb_stats_client::SeasonStatLine],
    extractor: impl Fn(&mlb_stats_client::SeasonStatLine) -> Option<f64>,
    higher_is_better: bool,
) {
    let points: Vec<SeasonPoint> = seasons.iter().filter_map(|s| extractor(s).map(|rate| SeasonPoint { season: s.season, rate })).collect();
    if points.is_empty() {
        println!("{:<8} no data", label);
        return;
    }
    let trend = linear_regression_trend(&points);
    let series: Vec<String> = trend.points.iter().map(|p| format!("{}: {:.2}", p.season, p.rate)).collect();
    let direction = trend.direction.judge(higher_is_better);
    println!("{:<8} {} -> {} ({:+.2}/season)", label, series.join(" | "), direction, trend.slope_per_season);
}

async fn list_cached() -> Result<()> {
    let ctx = AppContext::new().await?;
    let rows = ctx.db.list_player_stats_cache().await?;
    if rows.is_empty() {
        println!("No cached player history yet. Run `mlb history fetch \"<player>\"` or `mlb history sync-props`.");
        return Ok(());
    }
    println!("{:<28} {:<10} fetched_at", "Player", "Group");
    println!("{}", "-".repeat(60));
    for row in rows {
        println!("{:<28} {:<10} {}", row.player_name, row.stat_group, row.fetched_at.format("%Y-%m-%d %H:%M UTC"));
    }
    Ok(())
}

async fn sync_props(seasons: u32) -> Result<()> {
    let ctx = AppContext::new().await?;
    let stats_client = StatsApiClient::new();

    let events = ctx.db.get_upcoming_events().await?;

    let mut targets: HashSet<(String, StatGroup)> = HashSet::new();
    for event in &events {
        let Ok(odds) = ctx.db.get_latest_odds(&event.id).await else { continue };
        for book in &odds.bookmakers {
            for market in &book.markets {
                let Some(group) = market_to_group(market.key) else { continue };
                for outcome in &market.outcomes {
                    if let Some(player) = &outcome.description {
                        targets.insert((player.clone(), group));
                    }
                }
            }
        }
    }

    if targets.is_empty() {
        println!("No player props loaded. Run `mlb fetch odds --with-props` first.");
        return Ok(());
    }

    println!("Syncing history for {} unique player/market pairs...\n", targets.len());

    let mut ok = 0;
    let mut failed = 0;
    for (player, group) in &targets {
        match stats_client.find_player(player).await {
            Ok(resolved) => match mlb_predict::fetch_and_cache_report(&ctx.db, &stats_client, &resolved, *group, seasons).await {
                Ok(report) => {
                    println!("  OK   {} ({:?}) — {} seasons, {} games", report.player_name, group, report.seasons.len(), report.recent_games.len());
                    ok += 1;
                }
                Err(e) => {
                    println!("  FAIL {} ({:?}) — {}", player, group, e);
                    failed += 1;
                }
            },
            Err(e) => {
                println!("  SKIP {} ({:?}) — {}", player, group, e);
                failed += 1;
            }
        }
        // Be polite to the free public API.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    println!("\nDone. {} cached, {} skipped/failed.", ok, failed);
    Ok(())
}

fn market_to_group(m: MarketType) -> Option<StatGroup> {
    use MarketType::*;
    match m {
        BatterHits | BatterHomeRuns | BatterRbis | BatterStolenBases | BatterStrikeouts | BatterWalks => Some(StatGroup::Hitting),
        PitcherStrikeouts | PitcherHitsAllowed | PitcherWalks | PitcherEarnedRuns => Some(StatGroup::Pitching),
        _ => None,
    }
}
