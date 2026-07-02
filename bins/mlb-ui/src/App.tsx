import { useState, useEffect, useRef } from 'react'
import { Meta, GameData, ArbData, BestEvData, PropData, BookmakerData, fmtOdds, ChatMessage, RiskLevel, ChatResponse, ChatErrorBody } from './types'

const ALLOWED_BOOKS = ['FanDuel', 'DraftKings']

// ─── Data Loading ─────────────────────────────────────────────────────────────

async function fetchJson<T>(url: string, fallback: T): Promise<T> {
  try {
    const res = await fetch(url)
    if (!res.ok) return fallback
    return (await res.json()) as T
  } catch {
    return fallback
  }
}

function useData() {
  const [meta, setMeta]     = useState<Meta | null>(null)
  const [games, setGames]   = useState<GameData[]>([])
  const [arbs, setArbs]     = useState<ArbData[]>([])
  const [bestEv, setBestEv] = useState<BestEvData[]>([])
  const [props, setProps]   = useState<PropData[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const base = import.meta.env.BASE_URL
    Promise.all([
      fetch(`${base}data/meta.json`).then(r => r.json()),
      fetch(`${base}data/games.json`).then(r => r.json()),
      fetchJson<ArbData[]>(`${base}data/arb.json`, []),
      fetchJson<BestEvData[]>(`${base}data/best_ev.json`, []),
      fetchJson<PropData[]>(`${base}data/props.json`, []),
    ])
      .then(([m, g, a, ev, p]) => { setMeta(m); setGames(g); setArbs(a); setBestEv(ev); setProps(p) })
      .catch(() => setError('No data found. Run `mlb fetch odds && mlb export` first.'))
      .finally(() => setLoading(false))
  }, [])

  return { meta, games, arbs, bestEv, props, loading, error }
}

// ─── Icons (minimal line icons — no emoji) ────────────────────────────────────

function IconEdge() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 17l6-6 4 4 8-8" />
      <path d="M15 7h6v6" />
    </svg>
  )
}
function IconLayers() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polygon points="12 2 2 7 12 12 22 7 12 2" />
      <polyline points="2 17 12 22 22 17" />
      <polyline points="2 12 12 17 22 12" />
    </svg>
  )
}
function IconScale() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3v18M5 8l-3 6a4 4 0 0 0 6 0zM19 8l-3 6a4 4 0 0 0 6 0zM3 8h18M5 3h4" />
    </svg>
  )
}
function IconKey() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="7.5" cy="15.5" r="4.5" />
      <path d="M10.5 12.5L20 3M17 6l3 3M13 10l2 2" />
    </svg>
  )
}
function IconChat() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  )
}
function IconSend() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="22" y1="2" x2="11" y2="13" />
      <polygon points="22 2 15 22 11 13 2 9 22 2" />
    </svg>
  )
}
function IconChevron({ open }: { open: boolean }) {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"
      style={{ transform: open ? 'rotate(180deg)' : 'none', transition: 'transform 0.15s' }}>
      <polyline points="6 9 12 15 18 9" />
    </svg>
  )
}

// ─── App Root ─────────────────────────────────────────────────────────────────

type Tab = 'best-ev' | 'arb' | 'games' | 'props' | 'chat'

export default function App() {
  const { meta, games, arbs, bestEv, props, loading, error } = useData()
  const [tab, setTab] = useState<Tab>('best-ev')

  return (
    <div className="min-h-screen bg-[#060b14] relative">
      <div className="bg-grid absolute top-0 left-0 right-0 h-[480px] pointer-events-none" />

      <Header meta={meta} />

      <main className="max-w-7xl mx-auto px-4 py-8 space-y-8 relative">
        {loading && <LoadingState />}
        {error   && <ErrorState message={error} />}

        {!loading && !error && (
          <>
            <StatsRow
              totalGames={games.length}
              bestEvCount={bestEv.length}
              arbCount={arbs.length}
              requestsLeft={meta?.requests_remaining ?? null}
            />

            <div className="flex items-center gap-1 p-1 bg-[#0c1420] border border-white/[0.07] rounded-xl w-fit overflow-x-auto">
              <TabBtn label="Best EV" count={bestEv.length} active={tab === 'best-ev'} onClick={() => setTab('best-ev')} />
              <TabBtn label="Arbitrage" count={arbs.length} active={tab === 'arb'} onClick={() => setTab('arb')} />
              <TabBtn label="Games" count={games.length} active={tab === 'games'} onClick={() => setTab('games')} />
              {props.length > 0 && (
                <TabBtn label="Player Props" count={props.length} active={tab === 'props'} onClick={() => setTab('props')} />
              )}
              <TabBtn label="Parlay Assistant" count={0} active={tab === 'chat'} onClick={() => setTab('chat')} />
            </div>

            {tab === 'best-ev' && <BestEvSection bets={bestEv} />}
            {tab === 'arb'     && <ArbSection arbs={arbs} games={games} />}
            {tab === 'games'   && <GamesSection games={games} />}
            {tab === 'props'   && <PropsSection props={props} />}
            {tab === 'chat'    && <ChatSection hasGames={games.length > 0} />}
          </>
        )}
      </main>

      <footer className="border-t border-white/[0.06] mt-16 py-6 text-center text-slate-600 label-mono">
        <span className="normal-case tracking-normal text-[13px] font-normal text-slate-500">
          MLB Edge · Rust + React · Odds via{' '}
          <a href="https://the-odds-api.com" target="_blank" rel="noreferrer" className="text-sky-400 hover:underline">
            The Odds API
          </a>
          {' '}· FanDuel &amp; DraftKings only
        </span>
      </footer>
    </div>
  )
}

// ─── Header ───────────────────────────────────────────────────────────────────

function Header({ meta }: { meta: Meta | null }) {
  return (
    <header className="sticky top-0 z-50 border-b border-white/[0.07] bg-[#060b14]/85 backdrop-blur-md">
      <div className="max-w-7xl mx-auto px-4 py-3.5 flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <span className="w-8 h-8 rounded-lg border border-sky-400/25 bg-sky-400/10 grid place-items-center text-sky-400">
            <IconEdge />
          </span>
          <span className="text-[15px] font-bold tracking-tight text-slate-100">
            MLB <span className="gradient-text">Edge</span>
          </span>
        </div>
        <div className="flex items-center gap-4">
          {meta?.exported_at && (
            <span className="label-mono text-slate-600 hidden sm:inline">
              Updated <span className="text-slate-400">{meta.exported_at}</span>
            </span>
          )}
          <span className="tag-cyan">
            <span className="w-1.5 h-1.5 rounded-full bg-sky-400 pulse-dot" />
            Live Data
          </span>
        </div>
      </div>
    </header>
  )
}

// ─── Stats Row ────────────────────────────────────────────────────────────────

function StatsRow({
  totalGames, bestEvCount, arbCount, requestsLeft,
}: {
  totalGames: number; bestEvCount: number; arbCount: number; requestsLeft: number | null
}) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4 fade-in">
      <StatCard icon={<IconLayers />} label="Games Today" value={String(totalGames)} accent="cyan" />
      <StatCard icon={<IconEdge />}   label="Best EV Bets" value={String(bestEvCount)} accent={bestEvCount > 0 ? 'green' : 'cyan'} />
      <StatCard icon={<IconScale />}  label="Arb Opportunities" value={String(arbCount)} accent={arbCount > 0 ? 'green' : 'cyan'} />
      <StatCard icon={<IconKey />}    label="API Requests Left" value={requestsLeft != null ? String(requestsLeft) : '—'} accent="violet" />
    </div>
  )
}

function StatCard({ icon, label, value, accent }: { icon: React.ReactNode; label: string; value: string; accent: 'cyan' | 'green' | 'violet' }) {
  const cardClass = accent === 'green' ? 'card-green' : accent === 'violet' ? 'card-violet' : 'card-cyan'
  const valClass  = accent === 'green' ? 'text-emerald-400' : accent === 'violet' ? 'text-violet-300' : 'text-sky-400'
  const iconClass = accent === 'green' ? 'text-emerald-400 border-emerald-400/25 bg-emerald-400/10' : accent === 'violet' ? 'text-violet-300 border-violet-400/25 bg-violet-400/10' : 'text-sky-400 border-sky-400/25 bg-sky-400/10'
  return (
    <div className={`card ${cardClass} p-5 flex flex-col gap-3`}>
      <span className={`w-7 h-7 rounded-md border grid place-items-center ${iconClass}`}>{icon}</span>
      <p className={`stat-num text-2xl ${valClass}`}>{value}</p>
      <p className="label-mono text-slate-500">{label}</p>
    </div>
  )
}

// ─── Tabs ─────────────────────────────────────────────────────────────────────

function TabBtn({ label, count, active, onClick }: { label: string; count: number; active: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} className={`btn-tab whitespace-nowrap ${active ? 'btn-tab-active' : 'btn-tab-inactive'}`}>
      {label}{count > 0 ? ` (${count})` : ''}
    </button>
  )
}

// ─── Best EV Section ──────────────────────────────────────────────────────────

function BestEvSection({ bets }: { bets: BestEvData[] }) {
  if (bets.length === 0) {
    return <EmptyState title="No +EV bets right now" hint="Best EV compares FanDuel vs. DraftKings consensus fair odds against each book's actual line." />
  }

  const shown = bets.slice(0, 40)

  return (
    <div className="space-y-4 fade-in">
      <SectionHeading dotClass="bg-sky-400" title="Best Expected Value" tagClass="tag-cyan" tagText={`${bets.length} ranked`} />
      <p className="text-sm text-slate-500 max-w-2xl">
        Ranked by a multi-book consensus model: each line is de-vigged independently for FanDuel and DraftKings, averaged into a fair
        probability, then compared against every actual price offered to surface the largest edges.
      </p>

      <div className="card overflow-x-auto">
        <table className="w-full text-sm min-w-[820px]">
          <thead>
            <tr className="text-left label-mono text-slate-600 border-b border-white/[0.07]">
              <th className="py-3 pl-4 pr-3 font-medium">Matchup</th>
              <th className="py-3 px-3 font-medium">Market</th>
              <th className="py-3 px-3 font-medium">Selection</th>
              <th className="py-3 px-3 font-medium">Book</th>
              <th className="py-3 px-3 font-medium">Odds</th>
              <th className="py-3 px-3 font-medium">Fair %</th>
              <th className="py-3 px-3 font-medium">Edge</th>
              <th className="py-3 pr-4 pl-3 font-medium text-right">EV</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((bet, i) => (
              <tr key={i} className="border-b border-white/[0.05] last:border-0 hover:bg-white/[0.02] transition-colors">
                <td className="py-3 pl-4 pr-3 text-slate-300 max-w-[220px] truncate">{bet.matchup}</td>
                <td className="py-3 px-3 text-slate-500 text-xs">{bet.market}</td>
                <td className="py-3 px-3 text-slate-200 font-medium">
                  {bet.player ? `${bet.player} — ` : ''}{bet.outcome}{bet.point != null ? ` (${bet.point > 0 ? '+' : ''}${bet.point})` : ''}
                </td>
                <td className="py-3 px-3">
                  <span className="tag-muted">{bet.bookmaker}</span>
                </td>
                <td className="py-3 px-3 stat-num text-slate-200">{fmtOdds(bet.odds)}</td>
                <td className="py-3 px-3 stat-num text-slate-500">{(bet.true_probability * 100).toFixed(1)}%</td>
                <td className="py-3 px-3 stat-num text-slate-500">{bet.edge >= 0 ? '+' : ''}{(bet.edge * 100).toFixed(1)}pp</td>
                <td className="py-3 pr-4 pl-3 text-right">
                  <span className={`stat-num text-sm ${bet.ev_pct >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
                    {bet.ev_pct >= 0 ? '+' : ''}{bet.ev_pct.toFixed(2)}%
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ─── Arbitrage Section ────────────────────────────────────────────────────────

function ArbSection({ arbs, games }: { arbs: ArbData[]; games: GameData[] }) {
  if (arbs.length === 0) {
    return <EmptyState title="No arbitrage opportunities right now" hint="Run mlb fetch odds && mlb export to refresh." />
  }

  return (
    <div className="space-y-4 fade-in">
      <SectionHeading dotClass="bg-emerald-400" title="Arbitrage Opportunities" tagClass="tag-green" tagText={`${arbs.length} found`} />
      <div className="grid md:grid-cols-2 gap-4">
        {arbs.map((arb, i) => <ArbCard key={i} arb={arb} game={games.find(g => g.id === arb.event_id)} />)}
      </div>
    </div>
  )
}

function ArbCard({ arb, game }: { arb: ArbData; game?: GameData }) {
  const time = game?.commence_time ?? ''
  return (
    <div className="card card-green p-5 space-y-4 fade-in">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-semibold text-slate-100 text-base">{arb.matchup}</p>
          {time && <p className="label-mono text-slate-600 mt-1">{time}</p>}
        </div>
        <div className="shrink-0 text-right">
          <p className="stat-num text-2xl text-emerald-400">+{arb.profit_pct.toFixed(2)}%</p>
          <p className="label-mono text-slate-600 mt-1">guaranteed</p>
        </div>
      </div>

      <div className="space-y-2">
        {arb.legs.map((leg, i) => (
          <div key={i} className="flex items-center justify-between bg-white/[0.03] rounded-lg px-3 py-2.5">
            <div className="flex items-center gap-2 text-sm">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" />
              <span className="text-slate-200 font-medium">{leg.outcome}</span>
              <span className="text-slate-600">·</span>
              <span className="tag-muted">{leg.bookmaker}</span>
            </div>
            <div className="flex items-center gap-3 text-sm">
              <span className="stat-num text-emerald-400">{fmtOdds(leg.odds)}</span>
              <span className="label-mono text-slate-600">{leg.stake_pct.toFixed(1)}% stake</span>
            </div>
          </div>
        ))}
      </div>

      <p className="text-xs text-slate-600 border-t border-white/[0.06] pt-3">
        Split your bankroll at the stake percentages above for guaranteed profit regardless of outcome.
      </p>
    </div>
  )
}

// ─── Games Section ────────────────────────────────────────────────────────────

function GamesSection({ games }: { games: GameData[] }) {
  const [openId, setOpenId] = useState<string | null>(null)

  return (
    <div className="space-y-3 fade-in">
      <SectionHeading dotClass="bg-sky-400" title="Today's Games" tagClass="tag-cyan" tagText={`${games.length} games`} />
      {games.map(game => (
        <GameRow
          key={game.id}
          game={game}
          isOpen={openId === game.id}
          onToggle={() => setOpenId(openId === game.id ? null : game.id)}
        />
      ))}
    </div>
  )
}

function GameRow({ game, isOpen, onToggle }: { game: GameData; isOpen: boolean; onToggle: () => void }) {
  const bestHome = game.best_home_odds
  const bestAway = game.best_away_odds

  return (
    <div className={`card overflow-hidden ${isOpen ? 'card-cyan' : ''}`}>
      <button
        onClick={onToggle}
        className="w-full px-5 py-4 flex items-center justify-between gap-4 hover:bg-white/[0.02] transition-colors text-left"
      >
        <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-6 min-w-0">
          <div className="min-w-0">
            <p className="font-medium text-slate-100 truncate">
              <span className="text-slate-400">{game.away_team}</span>
              <span className="text-slate-600 mx-2">@</span>
              <span className="text-slate-100">{game.home_team}</span>
            </p>
            <p className="label-mono text-slate-600 mt-1">{game.commence_time}</p>
          </div>

          <div className="flex items-center gap-4 flex-wrap">
            {bestAway && (
              <div className="flex items-center gap-1.5 text-sm">
                <span className="text-slate-600 text-xs">{bestAway.team.split(' ').pop()}</span>
                <span className={`stat-num ${bestAway.odds > 0 ? 'text-emerald-400' : 'text-slate-300'}`}>
                  {fmtOdds(bestAway.odds)}
                </span>
                <span className="tag-muted">{bestAway.bookmaker}</span>
              </div>
            )}
            {bestHome && (
              <div className="flex items-center gap-1.5 text-sm">
                <span className="text-slate-600 text-xs">{bestHome.team.split(' ').pop()}</span>
                <span className={`stat-num ${bestHome.odds > 0 ? 'text-emerald-400' : 'text-slate-300'}`}>
                  {fmtOdds(bestHome.odds)}
                </span>
                <span className="tag-muted">{bestHome.bookmaker}</span>
              </div>
            )}
          </div>
        </div>

        <div className="flex items-center gap-3 shrink-0 text-slate-600">
          <span className="tag-muted hidden sm:inline-flex">{game.bookmakers.length} books</span>
          <IconChevron open={isOpen} />
        </div>
      </button>

      {isOpen && (
        <div className="border-t border-white/[0.06] px-5 py-4 overflow-x-auto">
          <OddsTable bookmakers={game.bookmakers} homeTeam={game.home_team} awayTeam={game.away_team} />
        </div>
      )}
    </div>
  )
}

// ─── Odds Table ───────────────────────────────────────────────────────────────

function OddsTable({ bookmakers, homeTeam, awayTeam }: { bookmakers: BookmakerData[]; homeTeam: string; awayTeam: string }) {
  const mlAway = bookmakers.flatMap(b => b.markets.find(m => m.key === 'Moneyline')?.outcomes.filter(o => o.name === awayTeam) ?? []).map(o => o.price)
  const mlHome = bookmakers.flatMap(b => b.markets.find(m => m.key === 'Moneyline')?.outcomes.filter(o => o.name === homeTeam) ?? []).map(o => o.price)
  const bestAwayML = mlAway.length ? Math.max(...mlAway) : null
  const bestHomeML = mlHome.length ? Math.max(...mlHome) : null

  return (
    <table className="w-full text-sm min-w-[600px]">
      <thead>
        <tr className="text-left label-mono text-slate-600 border-b border-white/[0.07]">
          <th className="pb-2 pr-4 font-medium">Bookmaker</th>
          <th className="pb-2 px-3 font-medium">{awayTeam.split(' ').slice(-1)[0]} ML</th>
          <th className="pb-2 px-3 font-medium">{homeTeam.split(' ').slice(-1)[0]} ML</th>
          <th className="pb-2 px-3 font-medium">Spread</th>
          <th className="pb-2 px-3 font-medium">Total</th>
        </tr>
      </thead>
      <tbody>
        {bookmakers.map(b => {
          const h2h    = b.markets.find(m => m.key === 'Moneyline')
          const spread = b.markets.find(m => m.key === 'Spread')
          const totals = b.markets.find(m => m.key === 'Totals')

          const awayML = h2h?.outcomes.find(o => o.name === awayTeam)
          const homeML = h2h?.outcomes.find(o => o.name === homeTeam)

          const spreadStr = spread?.outcomes.map(o => {
            const pt = o.point != null ? `(${o.point > 0 ? '+' : ''}${o.point})` : ''
            return `${o.name.split(' ').pop()} ${pt} ${fmtOdds(o.price)}`
          }).join(' / ') ?? '—'

          const over  = totals?.outcomes.find(o => o.name === 'Over')
          const under = totals?.outcomes.find(o => o.name === 'Under')
          const totalStr = over && under
            ? `O${over.point ?? ''} ${fmtOdds(over.price)} / U ${fmtOdds(under.price)}`
            : '—'

          return (
            <tr key={b.key} className="border-b border-white/[0.05] last:border-0 hover:bg-white/[0.02] transition-colors">
              <td className="py-2.5 pr-4">
                <span className="font-medium text-slate-200">{b.title}</span>
              </td>
              <td className="py-2.5 px-3">
                {awayML ? (
                  <span className={`stat-num ${awayML.price === bestAwayML ? 'text-emerald-400' : awayML.price > 0 ? 'text-sky-400' : 'text-slate-300'}`}>
                    {fmtOdds(awayML.price)}
                  </span>
                ) : <span className="text-slate-700">—</span>}
              </td>
              <td className="py-2.5 px-3">
                {homeML ? (
                  <span className={`stat-num ${homeML.price === bestHomeML ? 'text-emerald-400' : homeML.price > 0 ? 'text-sky-400' : 'text-slate-300'}`}>
                    {fmtOdds(homeML.price)}
                  </span>
                ) : <span className="text-slate-700">—</span>}
              </td>
              <td className="py-2.5 px-3 text-slate-500 text-xs">{spreadStr}</td>
              <td className="py-2.5 px-3 text-slate-500 text-xs">{totalStr}</td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

// ─── Player Props Section ─────────────────────────────────────────────────────

function PropsSection({ props }: { props: PropData[] }) {
  if (props.length === 0) {
    return <EmptyState title="No player props loaded" hint="Run mlb fetch odds --with-props && mlb export to pull FanDuel/DraftKings prop lines." />
  }

  const byMatchup = new Map<string, PropData[]>()
  for (const p of props) {
    const list = byMatchup.get(p.matchup) ?? []
    list.push(p)
    byMatchup.set(p.matchup, list)
  }

  return (
    <div className="space-y-6 fade-in">
      <SectionHeading dotClass="bg-violet-400" title="Player Props" tagClass="tag-violet" tagText={`${props.length} lines`} />

      {[...byMatchup.entries()].map(([matchup, lines]) => (
        <div key={matchup} className="card overflow-x-auto">
          <div className="px-5 py-3.5 border-b border-white/[0.07]">
            <p className="font-medium text-slate-200 text-sm">{matchup}</p>
          </div>
          <table className="w-full text-sm min-w-[640px]">
            <thead>
              <tr className="text-left label-mono text-slate-600 border-b border-white/[0.07]">
                <th className="py-2.5 pl-4 pr-3 font-medium">Player</th>
                <th className="py-2.5 px-3 font-medium">Market</th>
                <th className="py-2.5 px-3 font-medium">Line</th>
                {ALLOWED_BOOKS.map(book => (
                  <th key={book} className="py-2.5 px-3 font-medium">{book}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {lines.map((p, i) => (
                <tr key={i} className="border-b border-white/[0.05] last:border-0 hover:bg-white/[0.02] transition-colors">
                  <td className="py-2.5 pl-4 pr-3 text-slate-200 font-medium">{p.player}</td>
                  <td className="py-2.5 px-3 text-slate-500 text-xs">{p.market}</td>
                  <td className="py-2.5 px-3 stat-num text-slate-400">{p.point ?? '—'}</td>
                  {ALLOWED_BOOKS.map(book => {
                    const b = p.books.find(x => x.bookmaker === book)
                    return (
                      <td key={book} className="py-2.5 px-3 text-xs">
                        {b ? (
                          <span className="stat-num text-slate-300">
                            O {b.over_odds != null ? fmtOdds(b.over_odds) : '—'} / U {b.under_odds != null ? fmtOdds(b.under_odds) : '—'}
                          </span>
                        ) : <span className="text-slate-700">—</span>}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  )
}

// ─── Parlay Assistant (chat) ──────────────────────────────────────────────────

const RISK_OPTIONS: { key: RiskLevel; label: string; hint: string }[] = [
  { key: 'low', label: 'Low Risk', hint: 'Favorites, shorter odds, fewer legs' },
  { key: 'balanced', label: 'Balanced', hint: 'Mix of favorites and value picks' },
  { key: 'high', label: 'High Reward', hint: 'Underdogs, props, more legs' },
]

const QUICK_PROMPTS = [
  'Give me a 5 leg strikeouts parlay',
  'Low risk 3 leg moneyline parlay',
  'High risk home run props parlay',
  "Best 4 leg parlay across today's slate",
]

function ChatSection({ hasGames }: { hasGames: boolean }) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [risk, setRisk] = useState<RiskLevel>('balanced')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' })
  }, [messages, loading])

  async function send(text: string) {
    const trimmed = text.trim()
    if (!trimmed || loading) return

    setError(null)
    setInput('')
    setMessages(prev => [...prev, { role: 'user', content: trimmed }])
    setLoading(true)

    try {
      const res = await fetch('/api/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: trimmed, risk }),
      })

      if (!res.ok) {
        const body = (await res.json().catch(() => null)) as ChatErrorBody | null
        throw new Error(body?.error ?? `Request failed (${res.status})`)
      }

      const data = (await res.json()) as ChatResponse
      setMessages(prev => [...prev, { role: 'assistant', content: data.reply }])
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : 'Could not reach the Parlay Assistant server. Is `cargo run --bin mlb-server` running?'
      )
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="space-y-4 fade-in">
      <SectionHeading dotClass="bg-violet-400" title="Parlay Assistant" tagClass="tag-violet" tagText="Claude · live odds" />
      <p className="text-sm text-slate-500 max-w-2xl">
        Ask for a parlay by leg count, market, or risk level. The assistant only uses real FanDuel/DraftKings odds, best-EV, and
        player prop data currently loaded in the database — it will not invent lines.
      </p>

      {!hasGames && (
        <div className="card card-cyan p-4 text-sm text-slate-400">
          No games loaded yet. Run <code className="bg-white/5 text-sky-400 px-1.5 py-0.5 rounded text-xs">mlb fetch odds --with-props</code> first,
          then ask again — no export step needed for chat.
        </div>
      )}

      <div className="card flex flex-col h-[560px]">
        <div className="px-5 py-3.5 border-b border-white/[0.07] flex items-center gap-2 flex-wrap">
          <span className="label-mono text-slate-600 mr-1">Risk profile</span>
          {RISK_OPTIONS.map(opt => (
            <button
              key={opt.key}
              onClick={() => setRisk(opt.key)}
              title={opt.hint}
              className={`btn-tab ${risk === opt.key ? 'btn-tab-active' : 'btn-tab-inactive'}`}
            >
              {opt.label}
            </button>
          ))}
        </div>

        <div ref={scrollRef} className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {messages.length === 0 && (
            <div className="h-full flex flex-col items-center justify-center text-center gap-4">
              <span className="w-10 h-10 rounded-lg border border-violet-400/25 bg-violet-400/10 grid place-items-center text-violet-300">
                <IconChat />
              </span>
              <div>
                <p className="font-semibold text-slate-200">Build a parlay with Claude</p>
                <p className="text-sm text-slate-500 mt-1 max-w-sm">Try one of these, or type your own request below.</p>
              </div>
              <div className="flex flex-wrap gap-2 justify-center max-w-lg">
                {QUICK_PROMPTS.map(q => (
                  <button key={q} onClick={() => send(q)} className="tag-muted hover:bg-white/[0.06] transition-colors text-left">
                    {q}
                  </button>
                ))}
              </div>
            </div>
          )}

          {messages.map((m, i) => (
            <div key={i} className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}>
              <div
                className={
                  m.role === 'user'
                    ? 'max-w-[80%] rounded-xl rounded-tr-sm bg-sky-400/10 border border-sky-400/20 text-slate-100 px-4 py-2.5 text-sm'
                    : 'max-w-[85%] rounded-xl rounded-tl-sm bg-white/[0.03] border border-white/[0.07] text-slate-200 px-4 py-2.5 text-sm whitespace-pre-wrap leading-relaxed'
                }
              >
                {m.content}
              </div>
            </div>
          ))}

          {loading && (
            <div className="flex justify-start">
              <div className="rounded-xl rounded-tl-sm bg-white/[0.03] border border-white/[0.07] px-4 py-2.5 flex items-center gap-2 text-slate-500 label-mono">
                <div className="w-3 h-3 rounded-full border-2 border-violet-400 border-t-transparent animate-spin" />
                Thinking
              </div>
            </div>
          )}

          {error && (
            <div className="rounded-xl border border-rose-500/25 bg-rose-500/5 px-4 py-2.5 text-sm text-rose-400">
              {error}
            </div>
          )}
        </div>

        <div className="border-t border-white/[0.07] p-3.5 flex items-end gap-2">
          <textarea
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                send(input)
              }
            }}
            placeholder="e.g. give me a 5 leg strikeouts parlay"
            rows={1}
            className="flex-1 resize-none bg-[#060b14] border border-white/[0.08] rounded-lg px-3.5 py-2.5 text-sm text-slate-200 placeholder:text-slate-600 focus:outline-none focus:border-violet-400/40"
          />
          <button
            onClick={() => send(input)}
            disabled={loading || !input.trim()}
            className="shrink-0 w-10 h-10 rounded-lg border border-violet-400/25 bg-violet-400/10 grid place-items-center text-violet-300 disabled:opacity-40 disabled:cursor-not-allowed hover:bg-violet-400/20 transition-colors"
          >
            <IconSend />
          </button>
        </div>
      </div>

      <p className="text-xs text-slate-600 max-w-2xl">
        Not financial advice. Odds change quickly — always verify current lines with your sportsbook before placing a bet.
      </p>
    </div>
  )
}

// ─── Shared UI bits ───────────────────────────────────────────────────────────

function SectionHeading({ dotClass, title, tagClass, tagText }: { dotClass: string; title: string; tagClass: string; tagText: string }) {
  return (
    <div className="flex items-center gap-2.5">
      <span className={`w-2 h-2 rounded-full inline-block pulse-dot ${dotClass}`} />
      <h2 className="text-[15px] font-semibold text-slate-100">{title}</h2>
      <span className={tagClass}>{tagText}</span>
    </div>
  )
}

function EmptyState({ title, hint }: { title: string; hint: string }) {
  return (
    <div className="card p-8 flex items-start gap-4 fade-in">
      <span className="w-9 h-9 rounded-lg border border-white/10 bg-white/[0.03] grid place-items-center text-slate-500 shrink-0 mt-0.5">
        <IconLayers />
      </span>
      <div>
        <p className="font-semibold text-slate-200">{title}</p>
        <p className="text-sm text-slate-500 mt-1">
          {hint.includes('mlb ') ? (
            <>Run <code className="bg-white/5 text-sky-400 px-1.5 py-0.5 rounded text-xs">{hint.match(/mlb [^.]+/)?.[0]}</code> to refresh.</>
          ) : hint}
        </p>
      </div>
    </div>
  )
}

function LoadingState() {
  return (
    <div className="flex items-center justify-center py-24 gap-3 text-slate-600 label-mono">
      <div className="w-4 h-4 rounded-full border-2 border-sky-400 border-t-transparent animate-spin" />
      <span>Loading dashboard data</span>
    </div>
  )
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="card card-cyan p-8 flex items-start gap-4 fade-in">
      <span className="w-9 h-9 rounded-lg border border-sky-400/25 bg-sky-400/10 grid place-items-center text-sky-400 shrink-0 mt-0.5">
        <IconEdge />
      </span>
      <div>
        <p className="font-semibold text-slate-200 text-base">No data yet</p>
        <p className="text-slate-500 text-sm mt-1">{message}</p>
        <div className="mt-4 bg-[#060b14] rounded-lg p-3.5 border border-white/[0.07] text-sm font-mono">
          <p className="text-emerald-400 label-mono normal-case tracking-normal"># From the project root</p>
          <p className="text-slate-300 mt-1.5">cargo run --bin mlb -- fetch odds --with-props</p>
          <p className="text-slate-300">cargo run --bin mlb -- export</p>
          <p className="text-emerald-400 mt-2 label-mono normal-case tracking-normal"># Then refresh this page</p>
        </div>
      </div>
    </div>
  )
}

