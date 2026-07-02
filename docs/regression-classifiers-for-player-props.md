# From Box Score to Trend Line: Regression Classifiers for MLB Player Props

*Internal technical note — MLB Edge analytics platform*
*July 2026*

## Abstract

I have always loved baseball. I played in high school. But when I started understanding the analytics, the sport stopped being just balls and strikes to me — it became a stream of numbers that, if you looked at them the right way, would tell you things a box score never could: not just what a pitcher did last night, but where he's *headed*. That question — is a player getting better or worse, and does that trend hold up against the specific team he's about to face — is the subject of this note. We describe a small but complete system that pulls a player's historical season statistics and per-game logs from the free MLB Stats API, fits an ordinary least squares (OLS) regression line to a chosen rate statistic across seasons, and classifies the result into a simple three-state label — Improving, Regressing, or Stable — that a downstream large language model uses to reason about player-prop parlays. We also describe a matchup-history component that isolates a player's most recent meetings against a specific opponent. The system is implemented natively in Rust, requires no external ML runtime, and is validated against live data for real MLB players.

## 1. Introduction

Sportsbooks price player props — strikeouts, home runs, hits — using their own models, but those posted lines rarely explain *why* a number is what it is. A bettor (or an LLM assistant standing in for one) benefits from two additional pieces of context that odds alone don't provide:

1. **Season-over-season trend.** Is this player's relevant rate statistic (e.g., strikeouts per 9 innings for a pitcher) moving up, down, or holding steady across recent seasons?
2. **Matchup history.** How has this specific player performed the last few times he faced this specific opponent?

Neither of these requires a sophisticated model to be useful — they require *correct* data, a defensible statistical method, and careful handling of the fact that "improving" means different things for different stats (a rising strikeout rate is good; a rising ERA is bad). This note documents how we built exactly that, as the first concrete piece of the predictive-analytics layer of the MLB Edge platform, ahead of a planned full outcome-prediction model.

## 2. System Overview

The implementation spans three new components:

- **`mlb-stats-client`** — a thin, dependency-light HTTP client for `statsapi.mlb.com`, the free, public, unauthenticated MLB Stats API. It resolves a player name to a numeric player ID, fetches year-by-year season stat lines, and fetches per-game logs (including opponent team ID) for a configurable number of past seasons.
- **`mlb-predict`** — the analytical core. It has no knowledge of odds or props; its only job is to answer "how has this player performed historically." It contains the regression/trend engine and the matchup-filtering logic described below.
- **A SQLite-backed cache** (`player_stats_cache`) — historical stats change slowly (at most once per game played), so results are cached locally with a 12-hour freshness window rather than re-queried from the public API on every chat request.

These feed into the existing Parlay Assistant chat pipeline: when the assistant discusses a player who has cached history, that history — trend and matchup — is included as grounding context for the LLM, with an explicit instruction to never fabricate a trend for a player whose data hasn't been synced.

## 3. Methodology

### 3.1 Rate-Stat Extraction

A raw season stat line is not directly comparable across players or usable as a regression target — a full-season starter and a September call-up both have a "strikeout total," but only a rate normalizes for playing time. We therefore define, per market, a rate-stat extractor:

| Market | Rate statistic | Formula |
|---|---|---|
| Pitcher strikeouts | K/9 | `strikeouts / innings_pitched * 9` |
| Pitcher walks | BB/9 | `walks / innings_pitched * 9` |
| Pitcher hits allowed | H/9 | `hits_allowed / innings_pitched * 9` |
| Pitcher earned runs | ERA | reported directly |
| Batter home runs | HR/G | `home_runs / games` |
| Batter hits | Hits/G | `hits / games` |
| Batter RBIs | RBI/G | `rbi / games` |
| Batter stolen bases | SB/G | `stolen_bases / games` |
| Batter average | AVG | reported directly |

One subtlety worth documenting: the MLB Stats API reports innings pitched in a "6.1 / 6.2" notation, where the digit after the decimal point represents *thirds of an inning* (1 = one out, 2 = two outs), not tenths. Naively parsing `"6.1"` as the float `6.1` overstates innings pitched by roughly 2%, which compounds into a meaningfully wrong K/9. We implemented a dedicated parser that converts this into true outs (`whole * 3 + partial_outs`) before dividing.

### 3.2 Trend Regression

For a player with $n$ seasons of a given rate statistic $(x_i, y_i)$, where $x_i$ is the season year and $y_i$ is the rate, we fit an ordinary least squares line:

$$
\hat{y} = \beta_0 + \beta_1 x, \qquad \beta_1 = \frac{\sum_i (x_i - \bar{x})(y_i - \bar{y})}{\sum_i (x_i - \bar{x})^2}
$$

$\beta_1$ (the slope, in rate-units per season) is the trend signal. Rather than exposing a raw slope to the LLM — which has no inherent sense of scale for an arbitrary stat — we classify it into one of three labels using a threshold scaled to the stat's own mean magnitude:

$$
\text{direction} =
\begin{cases}
\text{Increasing} & \beta_1 > 0.03 \cdot \max(|\bar{y}|, 0.5) \\
\text{Decreasing} & \beta_1 < -0.03 \cdot \max(|\bar{y}|, 0.5) \\
\text{Stable} & \text{otherwise}
\end{cases}
$$

Players with fewer than two qualifying seasons are labeled `InsufficientData` rather than forced through a meaningless single-point "trend."

### 3.3 From Direction to Judgment: Handling Polarity

The most important correctness detail in this system — and the one bug we caught and fixed during implementation — is that **"increasing" is not the same as "improving."** A rising K/9 is good for a pitcher; a rising ERA or BB/9 is bad. Our first implementation collapsed these into a single `Improving`/`Regressing` label based purely on slope sign, which silently mislabeled every ERA and walk-rate trend in the system.

The corrected design separates the *statistical* direction (`Increasing` / `Decreasing` / `Stable`) from the *value judgment*, which is a function of both direction and a `higher_is_better` flag supplied per-statistic:

```rust
pub fn judge(&self, higher_is_better: bool) -> &'static str {
    match (self, higher_is_better) {
        (Increasing, true)  => "IMPROVING",
        (Increasing, false) => "REGRESSING",
        (Decreasing, true)  => "REGRESSING",
        (Decreasing, false) => "IMPROVING",
        (Stable, _)         => "STABLE",
        (InsufficientData, _) => "INSUFFICIENT DATA",
    }
}
```

Five unit tests lock this behavior in, including an explicit regression test asserting that a falling ERA is reported as `IMPROVING`.

### 3.4 Matchup History

Independently of trend, we filter a player's merged multi-season game log down to games played against a specific opponent (resolved via a static 30-team name-to-ID table), sort by date descending, and truncate to the three most recent meetings — matching the exact use case of "how has he done the last 3 times against this team."

## 4. Worked Example

Querying the live system for a real pitcher (Chase Burns) against the Houston Astros, restricted to pitching stats over the last four seasons, produces:

```
K/9      2025: 13.92 | 2026: 11.00 -> REGRESSING (-2.92/season)
ERA      2025: 4.57  | 2026: 2.36  -> IMPROVING  (-2.21/season)
BB/9     2025: 3.32  | 2026: 2.85  -> IMPROVING  (-0.48/season)

Last 3 meetings vs Astros:
  2026-05-09  6.0 IP, 2 K, 3 BB, 1 ER
```

This output demonstrates the polarity-correction described in §3.3: K/9 fell and is correctly flagged as a *negative* development (fewer strikeouts), while ERA and BB/9 also fell but are correctly flagged as *positive* developments, since lower is better for both. Only a single career meeting against this opponent exists in the data, which the system reports honestly rather than padding to a fixed count of three.

## 5. Limitations and Future Work

This system performs **regression for trend estimation**, not prediction of a specific game's outcome — it does not yet answer "how many strikeouts will this pitcher record tomorrow," only "is he trending up or down, and how has he fared against this opponent historically." The natural next step, already scoped for a future iteration, is a genuine predictive model: a multivariate regression (or lightweight classifier, via `linfa`/`smartcore` in native Rust) trained on opponent-adjusted, park-adjusted historical performance to produce a projected prop value or win probability, rather than a directional label. Additional near-term improvements include extending polarity-aware trend judgments to batter strikeouts and walks (currently unmapped due to API field ambiguity), and surfacing this history directly in the dashboard's Player Props tab rather than only within the chat assistant.

## 6. Conclusion

Even a simple, well-implemented regression can be genuinely useful if it's built with care — chiefly, by getting units right and by not conflating "the number went up" with "this is good news." That distinction, more than the regression math itself, was the real engineering problem here, and it's now covered by tests. What started as a stat-nerd's curiosity about a pitcher's strikeout trend against Houston is, as of this note, a working, tested, cached, and LLM-integrated feature of the platform — the first real piece of the "let the data tell you who's actually good" promise this project set out to make.
