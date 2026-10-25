import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { Activity, ArrowDownLeft, ArrowUpRight, ChevronRight, PhoneCall, RefreshCw, Phone } from 'lucide-react'
import { fetchActiveCalls, fetchNocOverview, getCurrentUser } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import KpiTile, { type KpiDelta } from '../components/noc/KpiTile'
import TrafficChart from '../components/noc/TrafficChart'
import CausesPanel from '../components/noc/CausesPanel'
import RangesTable from '../components/noc/RangesTable'
import QualityPanel from '../components/noc/QualityPanel'
import {
  acd,
  asr,
  compareLabel,
  fmtCompact,
  fmtInt,
  fmtMmss,
  fmtPct,
  maskNumber,
  minutes,
  ner,
  pctChange,
  thresholdStatus,
} from '../components/noc/format'
import type { ActiveCall, NocCompare, NocSlot, NocSlotValues } from '../types'

/** Active call with talk time in seconds (null while dialing/ringing) */
type LiveCall = ActiveCall & { talk: number | null }

const signed = (v: number, digits: number, suffix: string) =>
  `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(digits)}${suffix}`

// One rule for every tile: more traffic, longer calls and higher ratios are
// good news. Changes under 2% (or 0.5 pp) stay grey as noise.
const pctDelta = (now: number, before: number): KpiDelta | null => {
  const v = pctChange(now, before)
  return v == null ? null : { text: signed(v, 1, '%'), value: v, upIsGood: true, flatBelow: 2 }
}

const ppDelta = (now: number | null, before: number | null, flatBelow: number): KpiDelta | null =>
  now == null || before == null
    ? null
    : { text: signed(now - before, 1, ' pp'), value: now - before, upIsGood: true, flatBelow }

/** 24 hourly values of today for a tile sparkline (null = hour not started) */
const hourlyTrend = (series: NocSlot[], metric: keyof NocSlotValues): Array<number | null> =>
  Array.from({ length: 24 }, (_, h) => {
    const done = series.slice(h * 4, h * 4 + 4).filter(s => s.today).map(s => (s.today as NocSlotValues)[metric])
    if (!done.length) return null
    return metric === 'concurrent' ? Math.max(...done) : done.reduce((a, b) => a + b, 0)
  })

function TileSkeleton() {
  return (
    <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-4">
      {Array.from({ length: 6 }).map((_, i) => (
        <div key={i} className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-4 h-[132px] animate-pulse">
          <div className="h-3 bg-[var(--color-bg-tertiary)] rounded w-20" />
          <div className="h-7 bg-[var(--color-bg-tertiary)] rounded w-24 mt-4" />
        </div>
      ))}
    </div>
  )
}

const LIVE_ROWS = 10
const COMPARE_KEY = 'noc.compare'

// Same weekday last week by default: comparing a Saturday against a Friday
// mostly measures the weekend effect.
const initialCompare = (): NocCompare => {
  try {
    const saved = localStorage.getItem(COMPARE_KEY)
    if (saved === 'yesterday' || saved === 'week') return saved
  } catch {
    /* storage blocked */
  }
  return 'week'
}

export default function Dashboard() {
  const [compare, setCompareState] = useState<NocCompare>(initialCompare)
  // Drives the live call durations and the data age; the wall clock is in the header
  const [now, setNow] = useState(new Date())
  const setCompare = (c: NocCompare) => {
    setCompareState(c)
    try {
      localStorage.setItem(COMPARE_KEY, c)
    } catch {
      /* storage blocked */
    }
  }

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  const {
    data: noc,
    isLoading,
    isError,
    isFetching,
    isPlaceholderData,
  } = useQuery({
    queryKey: ['nocOverview', compare],
    queryFn: () => fetchNocOverview(compare),
    refetchInterval: 30000,
    placeholderData: keepPreviousData,
  })

  const { data: activeCallsData = [], dataUpdatedAt: callsFetchedAt } = useQuery({
    queryKey: ['activeCalls'],
    queryFn: fetchActiveCalls,
    refetchInterval: 5000,
  })
  const { data: user } = useQuery({ queryKey: ['currentUser'], queryFn: getCurrentUser, staleTime: 60000 })
  const canEditMarks = user?.role === 'superadmin' || user?.role === 'admin'
  const showNumber = (n: string) => (user?.role === 'superadmin' ? n : maskNumber(n))

  // The API gives talk time at fetch; advance it locally between polls so it
  // ticks every second without trusting the browser's clock against the server's
  const sinceFetch = callsFetchedAt ? Math.max(0, Math.floor((now.getTime() - callsFetchedAt) / 1000)) : 0
  const activeCalls = activeCallsData.map(call => ({
    ...call,
    id: call.call_uuid,
    talk: call.status === 'answered' ? (call.duration_seconds ?? 0) + sinceFetch : null,
  }))
  // One definition everywhere: "in progress" = connected + still dialing/ringing
  const connected = activeCalls.filter(c => c.status === 'answered').length
  const dialing = activeCalls.length - connected
  const liveBreakdown = `${fmtInt(connected)} conectadas + ${fmtInt(dialing)} marcando`

  const vs = `vs ${compareLabel(compare)} a esta hora`
  const ageSec = noc ? Math.max(0, Math.round((now.getTime() - new Date(noc.generated_at).getTime()) / 1000)) : null

  // Two missed refreshes in a row: the backend or the query is struggling
  const stale = ageSec != null && ageSec > 90

  const tiles = (() => {
    if (!noc) return null
    const T = noc.kpis.today
    const C = noc.kpis.compare_same_time
    const F = noc.kpis.compare_full_day
    const done = noc.series.filter(s => s.today)
    const peak = done.reduce<NocSlot | null>(
      (best, s) => (!best || (s.today as NocSlotValues).concurrent > (best.today as NocSlotValues).concurrent ? s : best),
      null,
    )
    const compareNow = noc.series[Math.min(noc.current_slot, noc.series.length - 1)]?.compare.concurrent ?? 0
    const nerToday = ner(T)
    const asrToday = asr(T)
    const th = noc.thresholds
    const maxCh = noc.capacity.max_channels
    const chUse = maxCh ? (100 * activeCalls.length) / maxCh : null

    return (
      <div className={`grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-4 transition-opacity ${isPlaceholderData ? 'opacity-60' : ''}`}>
        <KpiTile
          label="Llamadas en curso"
          value={fmtInt(activeCalls.length)}
          unit={maxCh ? `/ ${fmtInt(maxCh)} canales` : undefined}
          delta={pctDelta(activeCalls.length, compareNow)}
          deltaContext={vs}
          detail={liveBreakdown}
          status={chUse == null ? null : chUse >= 95 ? { level: 'critical', label: `${chUse.toFixed(0)}% uso` } : chUse >= 80 ? { level: 'warning', label: `${chUse.toFixed(0)}% uso` } : { level: 'good', label: `${chUse.toFixed(0)}% uso` }}
          trend={hourlyTrend(noc.series, 'concurrent')}
          title={`En vivo: conectadas + marcando/timbrando.${peak ? ` Pico hoy (CDR, 15 min): ${fmtInt((peak.today as NocSlotValues).concurrent)} a las ${peak.label}.` : ''}${maxCh ? '' : ' Define la capacidad en system_settings.noc_max_channels.'}`}
        />
        <KpiTile
          label="Intentos hoy"
          value={fmtInt(T.attempts)}
          delta={pctDelta(T.attempts, C.attempts)}
          deltaContext={vs}
          detail={`${compareLabel(compare).replace(/^./, c => c.toUpperCase())} (día completo): ${fmtCompact(F.attempts)}`}
          trend={hourlyTrend(noc.series, 'attempts')}
        />
        <KpiTile
          label="ASR"
          value={fmtPct(asrToday)}
          delta={ppDelta(asrToday, asr(C), 0.5)}
          deltaContext={vs}
          detail={`${fmtInt(T.answered)} contestadas de ${fmtInt(T.attempts)}`}
          status={thresholdStatus(asrToday, th.asr_warn, th.asr_crit)}
          title={`Answer-Seizure Ratio: llamadas contestadas / intentos. Vigilar bajo ${th.asr_warn}%, crítico bajo ${th.asr_crit}%.`}
        />
        <KpiTile
          label="ACD"
          value={fmtMmss(acd(T))}
          unit="min"
          delta={(() => {
            const a = acd(T)
            const b = acd(C)
            return a == null || b == null ? null : pctDelta(a, b)
          })()}
          deltaContext={vs}
          detail="Duración media por llamada contestada"
          title="Average Call Duration: segundos de conversación / llamadas contestadas"
        />
        <KpiTile
          label="Minutos cursados"
          value={fmtCompact(minutes(T))}
          delta={pctDelta(minutes(T), minutes(C))}
          deltaContext={vs}
          detail={`${compareLabel(compare).replace(/^./, c => c.toUpperCase())} (día completo): ${fmtCompact(minutes(F))}`}
          title="Minutes of Use: segundos de conversación de llamadas contestadas / 60"
        />
        <KpiTile
          label="NER"
          value={fmtPct(nerToday)}
          delta={ppDelta(nerToday, ner(C), 0.1)}
          deltaContext={vs}
          detail={`${fmtInt(T.network_failures)} fallas de red`}
          status={thresholdStatus(nerToday, th.ner_warn, th.ner_crit)}
          title={`Network Effectiveness Ratio (ITU-T E.425): 100% menos las llamadas que falló la red (timeouts, congestión, destino caído). Las no contestadas por decisión del usuario no cuentan. Vigilar bajo ${th.ner_warn}%, crítico bajo ${th.ner_crit}%.`}
        />
      </div>
    )
  })()

  const callColumns = [
    {
      key: 'caller_number',
      header: 'Origen',
      render: (call: ActiveCall) => (
        <div className="flex items-center gap-2">
          <Phone className="w-3.5 h-3.5 text-[var(--color-text-tertiary)]" aria-hidden="true" />
          <span className="font-mono text-sm text-[var(--color-text-primary)]">{showNumber(call.caller_number)}</span>
        </div>
      ),
    },
    {
      key: 'callee_number',
      header: 'Destino',
      render: (call: ActiveCall) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{showNumber(call.callee_number)}</span>
      ),
    },
    {
      key: 'direction',
      header: 'Sentido',
      render: (call: ActiveCall) => (
        <span className="flex items-center gap-1.5 text-sm text-[var(--color-text-secondary)]">
          {call.direction === 'outbound' ? (
            <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
          ) : call.direction === 'inbound' ? (
            <ArrowDownLeft className="w-4 h-4" aria-hidden="true" />
          ) : (
            <RefreshCw className="w-4 h-4" aria-hidden="true" />
          )}
          {call.direction === 'outbound' ? 'Saliente' : call.direction === 'inbound' ? 'Entrante' : 'Interna'}
        </span>
      ),
    },
    {
      key: 'duration_seconds',
      header: 'Duración',
      render: (call: LiveCall) => (
        <span className="font-mono text-sm tabular-nums">{call.talk == null ? '—' : fmtMmss(call.talk)}</span>
      ),
      className: 'text-right',
    },
    {
      key: 'status',
      header: 'Estado',
      render: (call: ActiveCall) => (
        <Badge variant={call.status === 'answered' ? 'success' : call.status === 'ringing' ? 'warning' : 'info'}>
          {call.status === 'answered' ? 'En curso' : call.status === 'ringing' ? 'Timbrando' : 'Marcando'}
        </Badge>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      {/* Header + the one filter row that scopes everything below */}
      <div className="flex flex-col lg:flex-row lg:items-end lg:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">Panel de Control</h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1">
            Centro de operaciones · tráfico del SBC en hora local{noc ? ` (${noc.timezone})` : ''}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <div role="radiogroup" aria-label="Comparar con" className="flex items-center rounded-lg border border-[var(--color-border-primary)] bg-[var(--color-bg-card)] p-0.5">
            <span className="text-xs text-[var(--color-text-tertiary)] px-2">Comparar con</span>
            {(['yesterday', 'week'] as NocCompare[]).map(c => (
              <button
                key={c}
                role="radio"
                aria-checked={compare === c}
                onClick={() => setCompare(c)}
                className={`px-3 py-1.5 text-xs font-medium rounded-md cursor-pointer transition-colors ${
                  compare === c
                    ? 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)]'
                    : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
                }`}
              >
                {c === 'yesterday' ? 'Ayer' : 'Mismo día, sem. pasada'}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-2 px-3 py-1.5 bg-[var(--color-bg-card)] rounded-lg border border-[var(--color-border-primary)]">
            {isFetching ? (
              <RefreshCw className="w-3.5 h-3.5 text-[var(--color-text-tertiary)] animate-spin" aria-hidden="true" />
            ) : (
              <Activity className="w-3.5 h-3.5" style={{ color: stale ? 'var(--viz-status-critical)' : 'var(--viz-status-good)' }} aria-hidden="true" />
            )}
            <span
              className="text-xs tabular-nums"
              style={{ color: stale ? 'var(--viz-status-critical)' : 'var(--color-text-secondary)' }}
              title="Los indicadores se recalculan cada 30 s; las llamadas en curso, cada 5 s."
            >
              {ageSec == null ? 'Cargando…' : `${stale ? 'Datos atrasados: ' : 'Datos actualizados hace '}${ageSec}s`}
            </span>
          </div>
        </div>
      </div>

      {isLoading ? (
        <TileSkeleton />
      ) : isError || !noc ? (
        <div className="rounded-xl border p-5" style={{ borderColor: 'var(--viz-status-warning)' }}>
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">No se pudieron cargar las estadísticas</h3>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1">
            Verifica que el backend (apolo-backend) y PostgreSQL estén activos.
          </p>
        </div>
      ) : (
        <>
          {tiles}
          <div className={`transition-opacity ${isPlaceholderData ? 'opacity-60' : ''}`}>
            <TrafficChart
              series={noc.series}
              currentSlot={noc.current_slot}
              compare={noc.compare}
              dayStart={noc.day_start}
              compareDayStart={noc.compare_day_start}
              events={noc.events}
              canEdit={canEditMarks}
            />
          </div>
          <QualityPanel quality={noc.quality} capacity={noc.capacity} />
          <div className={`grid grid-cols-1 xl:grid-cols-2 gap-4 transition-opacity ${isPlaceholderData ? 'opacity-60' : ''}`}>
            <CausesPanel causes={noc.causes} compare={noc.compare} />
            <RangesTable
              ranges={noc.ranges}
              compare={noc.compare}
              minAttempts={noc.thresholds.min_range_attempts}
              currentSlot={noc.current_slot}
            />
          </div>
        </>
      )}

      {/* Live calls */}
      <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <PhoneCall className="w-4 h-4 text-[var(--color-text-tertiary)]" aria-hidden="true" />
            <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Llamadas en curso</h2>
            <span className="text-sm text-[var(--color-text-tertiary)] tabular-nums">
              {fmtInt(activeCalls.length)} · {liveBreakdown}
            </span>
          </div>
          {activeCalls.length > 0 && <Badge variant="success">En vivo</Badge>}
        </div>
        <DataTable
          columns={callColumns}
          data={activeCalls.slice(0, LIVE_ROWS)}
          emptyMessage="No hay llamadas activas en este momento"
        />
        {activeCalls.length > LIVE_ROWS && (
          <div className="flex items-center justify-between mt-3 text-xs text-[var(--color-text-tertiary)]">
            <span>Mostrando las {LIVE_ROWS} más recientes de {fmtInt(activeCalls.length)}</span>
            <Link to="/calls" className="inline-flex items-center gap-0.5 font-medium text-[var(--viz-today)] hover:underline">
              Ver todas en Llamadas Activas
              <ChevronRight className="w-3.5 h-3.5" aria-hidden="true" />
            </Link>
          </div>
        )}
        {user && user.role !== 'superadmin' && (
          <p className="text-[11px] text-[var(--color-text-tertiary)] mt-2">Los números se muestran enmascarados para tu rol.</p>
        )}
      </div>
    </div>
  )
}
