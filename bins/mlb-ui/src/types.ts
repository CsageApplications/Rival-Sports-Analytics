export interface Meta {
  exported_at: string
  total_games: number
  arb_count: number
  best_ev_count: number
  prop_count: number
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
}

export interface PropBook {
  bookmaker: string
  over_odds: number | null
  under_odds: number | null
}

export interface PropData {
  event_id: string
  matchup: string
  market: string
  player: string
  point: number | null
  books: PropBook[]
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
