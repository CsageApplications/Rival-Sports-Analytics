//! Per-market rate-stat extractors: maps a raw season stat line to the
//! single rate number relevant to a given prop market, so trend
//! regression can be run generically across pitching and hitting props.

use mlb_stats_client::SeasonStatLine;

/// Strikeouts per 9 innings — used for pitcher strikeout props.
pub fn k_per_9(s: &SeasonStatLine) -> Option<f64> {
    let ip = s.innings_pitched.filter(|v| *v > 0.0)?;
    let k = s.strikeouts? as f64;
    Some(k / ip * 9.0)
}

/// Earned run average — general pitcher-quality trend.
pub fn era(s: &SeasonStatLine) -> Option<f64> {
    s.era
}

/// Walks per 9 innings — used for pitcher walks props.
pub fn walks_per_9(s: &SeasonStatLine) -> Option<f64> {
    let ip = s.innings_pitched.filter(|v| *v > 0.0)?;
    let bb = s.walks? as f64;
    Some(bb / ip * 9.0)
}

/// Hits allowed per 9 innings — used for pitcher hits-allowed props.
/// (`SeasonStatLine.hits` is contextual: for a pitching-group query it's
/// hits allowed, for a hitting-group query it's hits collected.)
pub fn hits_allowed_per_9(s: &SeasonStatLine) -> Option<f64> {
    let ip = s.innings_pitched.filter(|v| *v > 0.0)?;
    let h = s.hits? as f64;
    Some(h / ip * 9.0)
}

/// Home runs per game played — used for batter home run props.
pub fn hr_per_game(s: &SeasonStatLine) -> Option<f64> {
    let g = s.games.filter(|v| *v > 0)? as f64;
    let hr = s.home_runs? as f64;
    Some(hr / g)
}

/// Hits per game played — used for batter hits props.
pub fn hits_per_game(s: &SeasonStatLine) -> Option<f64> {
    let g = s.games.filter(|v| *v > 0)? as f64;
    let h = s.hits? as f64;
    Some(h / g)
}

/// Batting average — general batter-quality trend.
pub fn batting_avg(s: &SeasonStatLine) -> Option<f64> {
    s.avg
}

/// RBIs per game played — used for batter RBI props.
pub fn rbi_per_game(s: &SeasonStatLine) -> Option<f64> {
    let g = s.games.filter(|v| *v > 0)? as f64;
    let rbi = s.rbi? as f64;
    Some(rbi / g)
}

/// Stolen bases per game played — used for batter stolen base props.
pub fn sb_per_game(s: &SeasonStatLine) -> Option<f64> {
    let g = s.games.filter(|v| *v > 0)? as f64;
    let sb = s.stolen_bases? as f64;
    Some(sb / g)
}

/// Strikeouts per game played (as a batter) — used for batter strikeout props.
pub fn k_per_game_batter(s: &SeasonStatLine) -> Option<f64> {
    let g = s.games.filter(|v| *v > 0)? as f64;
    let k = s.batter_strikeouts? as f64;
    Some(k / g)
}

/// Walks drawn per game played (as a batter) — used for batter walks props.
pub fn walks_per_game_batter(s: &SeasonStatLine) -> Option<f64> {
    // Batter walks aren't separately tracked on SeasonStatLine (it reuses
    // `walks` for pitching only); OBP is the closest available proxy.
    s.obp
}
