# Rival Sports Analytics

A command-line tool for MLB sports betting analysis, built in Rust. Integrates with [The Odds API](https://the-odds-api.com/) to fetch real-time odds, analyze expected value, detect arbitrage opportunities, and track your bankroll.

## Features

- 📊 **Fetch odds** from multiple bookmakers (FanDuel, DraftKings, BetMGM, etc.)
- 📈 **Expected value analysis** - find +EV betting opportunities
- 🎯 **Arbitrage detection** - scan for risk-free profit opportunities
- 💰 **Bankroll management** - track deposits, withdrawals, and P/L
- 🗄️ **Historical data** - store odds snapshots for backtesting
- ⚡ **Fast & efficient** - built with async Rust

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) 1.75 or later
- An API key from [The Odds API](https://the-odds-api.com/) (free tier: 500 requests/month)

### Build from source

```bash
git clone https://github.com/CsageApplications/Rival-Sports-Analytics.git
cd Rival-Sports-Analytics
cargo build --release
```

The binary will be at `./target/release/mlb`.

## Configuration

Copy the example environment file and add your API key:

```bash
cp .env.example .env
# Edit .env and add your ODDS_API_KEY
```

Or set the environment variable directly:

```bash
export ODDS_API_KEY=your_api_key_here
```

## Usage

### Check API status

```bash
mlb status
```

### Fetch odds

```bash
# Fetch current MLB odds and save to database
mlb fetch odds

# Also print odds to console
mlb fetch odds --print

# Fetch upcoming events only (free, doesn't cost API credits)
mlb fetch events
```

### View events

```bash
# List upcoming games
mlb events list

# Show details for a specific event
mlb events show <event_id>
```

### Analyze odds

```bash
# Expected value analysis (uses fair odds estimate)
mlb analyze ev

# EV with your own probability estimate (55% for home team)
mlb analyze ev --probability 0.55

# Scan for arbitrage opportunities
mlb analyze arb

# Show best available odds
mlb analyze best

# Show vig/juice for each bookmaker
mlb analyze vig <event_id>
```

### Player history (trend regression + matchup, MLB Stats API)

Pulls free, historical season stats and per-game logs from
[statsapi.mlb.com](https://statsapi.mlb.com) (no API key needed) and caches them
in SQLite. Powers "is this player trending up or down?" (via linear regression
on the relevant rate stat, e.g. K/9) and "how has this player done the last 3
times he's faced this opponent?"

```bash
# Fetch + cache one player (pitching or hitting)
mlb history fetch "Chase Burns" --group pitching --seasons 4

# Season trend + last 3 meetings vs an opponent
mlb history matchup "Chase Burns" --vs Astros --group pitching

# Bulk-sync history for every player currently in loaded props
# (run this after `mlb fetch odds --with-props`)
mlb history sync-props

# See everything currently cached
mlb history list
```

The Parlay Assistant chatbot automatically includes this cached history (never
live-fetched during chat, so responses stay fast) for any player it's already
been synced for — run `mlb history sync-props` periodically to keep it fresh.

### Manage bankroll

```bash
# Check bankroll status
mlb bankroll status

# Deposit funds
mlb bankroll deposit 1000

# Withdraw funds
mlb bankroll withdraw 500

# View transaction history
mlb bankroll history
```

## Project Structure

```
Rival-Sports-Analytics/
├── Cargo.toml                    # Workspace configuration
├── crates/
│   ├── mlb-core/                 # Domain models & business logic
│   │   └── src/
│   │       ├── models/           # Event, Odds, Bet, Bankroll
│   │       ├── analysis/         # EV, Kelly, Arbitrage
│   │       └── repository.rs     # Repository traits
│   │
│   ├── mlb-odds-client/          # The Odds API HTTP client
│   │   └── src/
│   │       ├── client.rs         # API client
│   │       ├── types.rs          # API response types
│   │       └── rate_limiter.rs   # Rate limiting
│   │
│   ├── mlb-db/                   # SQLite persistence
│   │   └── src/
│   │       ├── database.rs       # Connection management
│   │       └── repositories/     # Repository implementations
│   │
│   ├── mlb-llm/                  # Anthropic Claude client (parlay chat)
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── mlb-stats-client/         # Free MLB Stats API client (historical stats)
│   │   └── src/
│   │       ├── client.rs         # Player search, season stats, game logs
│   │       └── teams.rs          # Team name -> statsapi team ID mapping
│   │
│   └── mlb-predict/               # Trend regression + matchup history analysis
│       └── src/
│           ├── trend.rs          # OLS linear regression + IMPROVING/REGRESSING
│           ├── matchup.rs        # Last-N-meetings-vs-opponent filtering
│           └── report.rs         # SQLite-cached player report fetch/build
│
└── bins/
    ├── mlb-cli/                  # CLI application
    │   └── src/
    │       ├── main.rs
    │       └── commands/         # CLI subcommands
    │
    ├── mlb-server/                # HTTP API for the Parlay Assistant chatbot
    │   └── src/
    │       ├── main.rs           # Axum routes (/api/chat, /api/health)
    │       ├── context.rs        # Builds odds/EV/prop context for Claude
    │       └── prompt.rs         # System prompt + risk-profile rules
    │
    └── mlb-ui/                    # React dashboard (Best EV, Arb, Games, Props, Chat)
```

## API Usage

The Odds API has usage-based pricing:
- **Free tier**: 500 requests/month
- Fetching odds costs 1 request per region per market
- Fetching events/sports is free
- Fetching player props (`--with-props`) costs 1 request **per game**

The CLI displays remaining requests after each API call. Use `mlb status` to check your quota.

## Parlay Assistant (chatbot)

The dashboard includes a chat tab that asks Claude to build parlays (e.g. "give me a 5 leg strikeouts parlay" or "low risk 3 leg parlay") using only the real FanDuel/DraftKings odds, best-EV, and player prop data currently in the database — it never invents lines.

1. Get an API key from [console.anthropic.com](https://console.anthropic.com) and add it to `.env`:
   ```bash
   ANTHROPIC_API_KEY=your_key_here
   ```
2. Make sure odds (and ideally props) are loaded, then sync player history so the assistant can cite real trends/matchups:
   ```bash
   cargo run --bin mlb -- fetch odds --with-props
   cargo run --bin mlb -- history sync-props
   ```
3. Start the chat API server:
   ```bash
   cargo run --bin mlb-server
   ```
4. In another terminal, run the dashboard as usual (`cd bins/mlb-ui && npm run dev`) and open the **Parlay Assistant** tab.

The server re-reads the database on every chat request, so re-run `mlb fetch odds` whenever you want the assistant working off fresher lines — no `export` step needed for chat (export is only for the static JSON dashboard tabs).

## Development

```bash
# Run tests
cargo test --workspace

# Run with debug logging
RUST_LOG=mlb=debug cargo run -p mlb-cli -- fetch odds

# Format code
cargo fmt --all

# Lint
cargo clippy --workspace
```

## License

MIT 
