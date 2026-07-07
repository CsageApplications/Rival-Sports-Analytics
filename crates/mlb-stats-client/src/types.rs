//! Domain + raw API types for the MLB Stats API client.

use serde::{Deserialize, Serialize};

/// Which stat category to pull — pitching or hitting. Most players only
/// have meaningful data in one of these, but two-way players (etc.) can
/// be queried for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatGroup {
    Pitching,
    Hitting,
}

impl StatGroup {
    pub fn as_api_key(&self) -> &'static str {
        match self {
            StatGroup::Pitching => "pitching",
            StatGroup::Hitting => "hitting",
        }
    }
}

/// A resolved player from a name search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSearchResult {
    pub id: i64,
    pub full_name: String,
    pub position: Option<String>,
    pub team_id: Option<i64>,
    pub team_name: Option<String>,
}

/// One season's aggregated stat line. Pitching-only and hitting-only
/// fields are `None` when irrelevant to the queried `StatGroup`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeasonStatLine {
    pub season: i32,
    pub team_name: Option<String>,

    // ── Pitching ──
    pub games: Option<i32>,
    pub games_started: Option<i32>,
    pub innings_pitched: Option<f64>,
    pub strikeouts: Option<i32>,
    pub walks: Option<i32>,
    pub earned_runs: Option<i32>,
    pub era: Option<f64>,
    pub whip: Option<f64>,

    // ── Hitting ──
    pub plate_appearances: Option<i32>,
    pub at_bats: Option<i32>,
    pub hits: Option<i32>,
    pub home_runs: Option<i32>,
    pub rbi: Option<i32>,
    pub avg: Option<f64>,
    pub obp: Option<f64>,
    pub slg: Option<f64>,
    pub ops: Option<f64>,
    pub batter_strikeouts: Option<i32>,
    pub stolen_bases: Option<i32>,
}

/// A single game's stat line, with opponent info so it can be filtered
/// down to matchup history against a specific team.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameLogEntry {
    pub date: String,
    pub season: i32,
    pub opponent_id: Option<i64>,
    pub opponent_name: Option<String>,
    pub is_home: bool,

    // ── Pitching ──
    pub innings_pitched: Option<f64>,
    pub strikeouts: Option<i32>,
    pub walks: Option<i32>,
    pub earned_runs: Option<i32>,
    pub hits_allowed: Option<i32>,

    // ── Hitting ──
    pub at_bats: Option<i32>,
    pub hits: Option<i32>,
    pub home_runs: Option<i32>,
    pub rbi: Option<i32>,
    pub batter_strikeouts: Option<i32>,
    pub stolen_bases: Option<i32>,
    pub walks_drawn: Option<i32>,
}

/// Parses MLB's innings-pitched notation (e.g. `"6.1"` = 6 and 1/3
/// innings, `"6.2"` = 6 and 2/3 innings — NOT decimal tenths) into a true
/// decimal value (6.333..., 6.666...).
pub fn parse_innings_pitched(raw: &str) -> Option<f64> {
    let value: f64 = raw.parse().ok()?;
    let whole = value.trunc();
    let frac_digit = ((value - whole) * 10.0).round() as i64;
    let outs = (whole as i64) * 3 + frac_digit.clamp(0, 2);
    Some(outs as f64 / 3.0)
}

/// A team's current season win/loss record and run differential, used as
/// input to the Pythagorean win-expectation projection in `mlb-predict`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStanding {
    pub team_id: i64,
    pub team_name: String,
    pub wins: i32,
    pub losses: i32,
    pub win_pct: f64,
    pub runs_scored: Option<i32>,
    pub runs_allowed: Option<i32>,
}

// ── Raw API response shapes (deserialize-only) ──────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct SearchResponse {
    pub people: Option<Vec<RawPerson>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPerson {
    pub id: i64,
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[serde(rename = "primaryPosition")]
    pub primary_position: Option<RawPosition>,
    #[serde(rename = "currentTeam")]
    pub current_team: Option<RawTeamRef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawPosition {
    pub abbreviation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawTeamRef {
    pub id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StatsResponse {
    pub stats: Option<Vec<RawStatGroup>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawStatGroup {
    pub splits: Option<Vec<RawSplit>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSplit {
    pub season: Option<String>,
    pub date: Option<String>,
    pub team: Option<RawTeamRef>,
    pub opponent: Option<RawTeamRef>,
    #[serde(rename = "isHome")]
    pub is_home: Option<bool>,
    pub stat: Option<RawStat>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct RawStat {
    #[serde(rename = "gamesPlayed")]
    pub games_played: Option<i32>,
    #[serde(rename = "gamesStarted")]
    pub games_started: Option<i32>,
    #[serde(rename = "inningsPitched")]
    pub innings_pitched: Option<String>,
    #[serde(rename = "strikeOuts")]
    pub strike_outs: Option<i32>,
    #[serde(rename = "baseOnBalls")]
    pub base_on_balls: Option<i32>,
    #[serde(rename = "earnedRuns")]
    pub earned_runs: Option<i32>,
    pub era: Option<String>,
    pub whip: Option<String>,
    pub hits: Option<i32>,
    #[serde(rename = "atBats")]
    pub at_bats: Option<i32>,
    #[serde(rename = "plateAppearances")]
    pub plate_appearances: Option<i32>,
    #[serde(rename = "homeRuns")]
    pub home_runs: Option<i32>,
    pub rbi: Option<i32>,
    pub avg: Option<String>,
    pub obp: Option<String>,
    pub slg: Option<String>,
    pub ops: Option<String>,
    #[serde(rename = "stolenBases")]
    pub stolen_bases: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StandingsResponse {
    pub records: Option<Vec<RawStandingsRecord>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawStandingsRecord {
    #[serde(rename = "teamRecords")]
    pub team_records: Option<Vec<RawTeamRecord>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawTeamRecord {
    pub team: Option<RawTeamRef>,
    pub wins: Option<i32>,
    pub losses: Option<i32>,
    #[serde(rename = "winningPercentage")]
    pub winning_percentage: Option<String>,
    #[serde(rename = "runsScored")]
    pub runs_scored: Option<i32>,
    #[serde(rename = "runsAllowed")]
    pub runs_allowed: Option<i32>,
}
