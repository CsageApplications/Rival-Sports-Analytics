//! Filters a player's merged game log down to their most recent meetings
//! against a specific opponent — e.g. "the last 3 times this pitcher
//! faced the Astros."

use mlb_stats_client::GameLogEntry;

pub fn last_n_meetings(games: &[GameLogEntry], opponent_id: i64, limit: usize) -> Vec<GameLogEntry> {
    let mut matches: Vec<GameLogEntry> = games
        .iter()
        .filter(|g| g.opponent_id == Some(opponent_id))
        .cloned()
        .collect();
    matches.sort_by(|a, b| b.date.cmp(&a.date));
    matches.truncate(limit);
    matches
}
