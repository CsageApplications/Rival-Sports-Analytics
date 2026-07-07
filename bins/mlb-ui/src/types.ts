export interface Meta {
  exported_at: string
  total_games: number
  arb_count: number
  best_ev_count: number
  prop_count: number
  prediction_count?: number
  requests_remaining: number | null
}

export interface OutcomeData {
  name: string
  price: number
  point: number | null
}

export interface MarketData {
  key: string
  outcomes: OutcomeData[]
}

export interface BookmakerData {
  key: string
  title: string
  markets: MarketData[]
}

export interface BestOdds {
  team: string
  odds: number
  bookmaker: string
}

export interface GameData {
  id: string
  home_team: string
  away_team: string
  matchup: string
  commence_time: string
  commence_timestamp: number
  bookmakers: BookmakerData[]
  best_home_odds: BestOdds | null
  best_away_odds: BestOdds | null
}

export interface ArbLeg {
  outcome: string
  bookmaker: string
  odds: number
  stake_pct: number
}

export interface ArbData {
  event_id: string
  matchup: string
  profit_pct: number
  legs: ArbLeg[]
}

export interface BestEvData {
  event_id: string
  matchup: string
  market: string
  outcome: string
  player: string | null
  point: number | null
  bookmaker: string
  odds: number
  true_probability: number
  implied_probability: number
  edge: number
  ev_pct: number
  /** "Consensus" (FanDuel vs. DraftKings cross-book de-vig agreement) or "Model Projection" (recent-form Poisson projection vs. a single book's price). */
  source?: string
}

export interface PropBook {
  bookmaker: string
  over_odds: number | null
  under_odds: number | null
  /** Model-implied EV (fraction of stake, e.g. 0.08 = +8%) for the Over at this book, if a projection exists. */
  over_ev_pct?: number | null
  /** Model-implied EV (fraction of stake) for the Under at this book, if a projection exists. */
  under_ev_pct?: number | null
}

export interface PropProjection {
  /** Recent-form (last N games) projected count for tonight. */
  projected_value: number
  /** How many recent games the projection is based on. */
  sample_games: number
  /** Model-implied probability the Over hits (Poisson count model). */
  over_probability: number
  under_probability: number
}

export interface TrendSummary {
  stat_label: string
  /** "IMPROVING" | "REGRESSING" | "STABLE" | "INSUFFICIENT DATA" */
  direction: string
  points: [number, number][]
  slope_per_season: number
}

export interface MeetingSummary {
  date: string
  line: string
}

export interface PlayerHistorySummary {
  player_name: string
  trend: TrendSummary | null
  opponent_name: string | null
  last_meetings: MeetingSummary[]
}

export interface PropData {
  event_id: string
  matchup: string
  market: string
  player: string
  point: number | null
  books: PropBook[]
  history?: PlayerHistorySummary | null
  /** Recent-form Poisson projection for this exact market/line, if available. */
  projection?: PropProjection | null
  /** Best model-implied EV (fraction of stake) across all books/sides for this line. */
  best_model_ev_pct?: number | null
  /** Which side ("Over" / "Under") produced `best_model_ev_pct`. */
  best_model_side?: string | null
  /** True if the model's probability diverges sharply (>30pp) from the market's implied probability — likely missing lineup/role context rather than real value. */
  best_model_high_divergence?: boolean | null
}

export interface PredictionData {
  event_id: string
  matchup: string
  home_team: string
  away_team: string
  home_record: string
  away_record: string
  model_home_win_pct: number
  model_away_win_pct: number
  market_home_fair_pct: number
  market_away_fair_pct: number
  home_edge_pct: number
  away_edge_pct: number
}

export type RiskLevel = 'low' | 'balanced' | 'high'

export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

export interface ChatResponse {
  reply: string
}

export interface ChatErrorBody {
  error: string
}

export function fmtOdds(price: number): string {
  return price >= 0 ? `+${Math.round(price)}` : `${Math.round(price)}`
}

export function isUnderdog(price: number): boolean {
  return price > 0
}
