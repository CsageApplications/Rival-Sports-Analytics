//! Static MLB team ID table.
//!
//! statsapi.mlb.com identifies teams by a stable numeric ID rather than
//! name. This table lets us translate the team names we already have
//! (from The Odds API, e.g. "Houston Astros") into that ID so we can
//! filter a player's game log down to games against a specific opponent.

#[derive(Debug, Clone, Copy)]
pub struct TeamInfo {
    pub id: i64,
    pub full_name: &'static str,
    pub abbreviation: &'static str,
}

pub const MLB_TEAMS: &[TeamInfo] = &[
    TeamInfo { id: 108, full_name: "Los Angeles Angels", abbreviation: "LAA" },
    TeamInfo { id: 109, full_name: "Arizona Diamondbacks", abbreviation: "ARI" },
    TeamInfo { id: 110, full_name: "Baltimore Orioles", abbreviation: "BAL" },
    TeamInfo { id: 111, full_name: "Boston Red Sox", abbreviation: "BOS" },
    TeamInfo { id: 112, full_name: "Chicago Cubs", abbreviation: "CHC" },
    TeamInfo { id: 113, full_name: "Cincinnati Reds", abbreviation: "CIN" },
    TeamInfo { id: 114, full_name: "Cleveland Guardians", abbreviation: "CLE" },
    TeamInfo { id: 115, full_name: "Colorado Rockies", abbreviation: "COL" },
    TeamInfo { id: 116, full_name: "Detroit Tigers", abbreviation: "DET" },
    TeamInfo { id: 117, full_name: "Houston Astros", abbreviation: "HOU" },
    TeamInfo { id: 118, full_name: "Kansas City Royals", abbreviation: "KC" },
    TeamInfo { id: 119, full_name: "Los Angeles Dodgers", abbreviation: "LAD" },
    TeamInfo { id: 120, full_name: "Washington Nationals", abbreviation: "WSH" },
    TeamInfo { id: 121, full_name: "New York Mets", abbreviation: "NYM" },
    TeamInfo { id: 133, full_name: "Athletics", abbreviation: "OAK" },
    TeamInfo { id: 134, full_name: "Pittsburgh Pirates", abbreviation: "PIT" },
    TeamInfo { id: 135, full_name: "San Diego Padres", abbreviation: "SD" },
    TeamInfo { id: 136, full_name: "Seattle Mariners", abbreviation: "SEA" },
    TeamInfo { id: 137, full_name: "San Francisco Giants", abbreviation: "SF" },
    TeamInfo { id: 138, full_name: "St. Louis Cardinals", abbreviation: "STL" },
    TeamInfo { id: 139, full_name: "Tampa Bay Rays", abbreviation: "TB" },
    TeamInfo { id: 140, full_name: "Texas Rangers", abbreviation: "TEX" },
    TeamInfo { id: 141, full_name: "Toronto Blue Jays", abbreviation: "TOR" },
    TeamInfo { id: 142, full_name: "Minnesota Twins", abbreviation: "MIN" },
    TeamInfo { id: 143, full_name: "Philadelphia Phillies", abbreviation: "PHI" },
    TeamInfo { id: 144, full_name: "Atlanta Braves", abbreviation: "ATL" },
    TeamInfo { id: 145, full_name: "Chicago White Sox", abbreviation: "CWS" },
    TeamInfo { id: 146, full_name: "Miami Marlins", abbreviation: "MIA" },
    TeamInfo { id: 147, full_name: "New York Yankees", abbreviation: "NYY" },
    TeamInfo { id: 158, full_name: "Milwaukee Brewers", abbreviation: "MIL" },
];

/// Resolve a team name (exact or fuzzy, e.g. "Astros" or "Houston Astros")
/// to its statsapi.mlb.com team ID.
pub fn find_team_id_by_name(name: &str) -> Option<i64> {
    let q = name.to_lowercase();

    // Exact full-name match first.
    if let Some(t) = MLB_TEAMS.iter().find(|t| t.full_name.to_lowercase() == q) {
        return Some(t.id);
    }

    // Then abbreviation match.
    if let Some(t) = MLB_TEAMS.iter().find(|t| t.abbreviation.to_lowercase() == q) {
        return Some(t.id);
    }

    // Then fuzzy substring match in either direction (e.g. "astros" in
    // "houston astros", or a partial query containing the full name).
    MLB_TEAMS
        .iter()
        .find(|t| {
            let full = t.full_name.to_lowercase();
            full.contains(&q) || q.contains(&full)
        })
        .map(|t| t.id)
}

pub fn team_name_by_id(id: i64) -> Option<&'static str> {
    MLB_TEAMS.iter().find(|t| t.id == id).map(|t| t.full_name)
}
