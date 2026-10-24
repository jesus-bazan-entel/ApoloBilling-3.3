import { useEffect, useState } from 'react'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { Activity, ArrowDownLeft, ArrowUpRight, PhoneCall, RefreshCw, Phone } from 'lucide-react'
import { fetchActiveCalls, fetchNocOverview } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import KpiTile, { type KpiDelta } from '../components/noc/KpiTile'
import TrafficChart from '../components/noc/TrafficChart'
import CausesPanel from '../components/noc/CausesPanel'
import RangesTable from '../components/noc/RangesTable'
import {
  acd,
  asr,
  compareLabel,
  compareLongLabel,
  fmtCompact,
  fmtInt,
  fmtMmss,
  fmtPct,
  minutes,
  ner,
  pctChange,
} from '../components/noc/format'
import type { ActiveCall, NocCompare, NocSlot, NocSlotValues } from '../types'

const signed = (v: number, digits: number, suffix: string) =>
  `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(digits)}${suffix}`

const pctDelta = (now: number, before: number, upIsGood: boolean | null): KpiDelta | null => {
  const v = pctChange(now, before)
  return v == null ? null : { text: signed(v, 1, '%'), value: v, upIsGood, flatBelow: 1 }
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

export default function Dashboard() {
  const [compare, setCompare] = useState<NocCompare>('yesterday')
  const [now, setNow] = useState(new Date())

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
    refetchInterval: 60000,
    placeholderData: keepPreviousData,
  })

  const { data: activeCallsData = [] } = useQuery({
    queryKey: ['activeCalls'],
    queryFn: fetchActiveCalls,
    refetchInterval: 5000,
  })
  const activeCalls = activeCallsData.map(call => ({ ...call, id: call.uuid }))

  const vs = `vs ${compareLabel(compare)} a esta hora`
  const ageSec = noc ? Math.max(0, Math.round((now.getTime() - new Date(noc.generated_at).getTime()) / 1000)) : null

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
    const revenueToday = Number(T.revenue)
    const revenueCompare = Number(C.revenue)

    return (
      <div className={`grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-4 transition-opacity ${isPlaceholderData ? 'opacity-60' : ''}`}>
        <KpiTile
          label="Llamadas simultáneas"
          value={fmtInt(activeCalls.length)}
          delta={pctDelta(activeCalls.length, compareNow, null)}
          deltaContext={vs}
          detail={peak ? `Pico hoy: ${fmtInt((peak.today as NocSlotValues).concurrent)} a las ${peak.label}` : undefined}
          trend={hourlyTrend(noc.series, 'concurrent')}
          title="Llamadas en curso ahora (en vivo). El pico y la tendencia salen de los CDR en intervalos de 15 min."
        />
        <KpiTile
          label="Intentos hoy"
          value={fmtInt(T.attempts)}
          delta={pctDelta(T.attempts, C.attempts, null)}
          deltaContext={vs}
          detail={`${compareLongLabel(compare).replace(/^./, c => c.toUpperCase())}, día completo: ${fmtInt(F.attempts)}`}
          trend={hourlyTrend(noc.series, 'attempts')}
        />
        <KpiTile
          label="ASR"
          value={fmtPct(asr(T))}
          delta={ppDelta(asr(T), asr(C), 0.5)}
          deltaContext={vs}
          detail={`${fmtInt(T.answered)} contestadas de ${fmtInt(T.attempts)}`}
          title="Answer-Seizure Ratio: llamadas contestadas / intentos"
        />
        <KpiTile
          label="ACD"
          value={fmtMmss(acd(T))}
          unit="min"
          delta={(() => {
            const a = acd(T)
            const b = acd(C)
            return a == null || b == null ? null : pctDelta(a, b, true)
          })()}
          deltaContext={vs}
          detail="Duración media por llamada contestada"
          title="Average Call Duration: segundos facturados / llamadas contestadas"
        />
        <KpiTile
          label="Minutos cursados"
          value={fmtCompact(minutes(T))}
          delta={pctDelta(minutes(T), minutes(C), true)}
          deltaContext={vs}
          detail={`${compareLongLabel(compare).replace(/^./, c => c.toUpperCase())}, día completo: ${fmtCompact(minutes(F))}`}
          title="Minutes of Use: segundos facturados de llamadas contestadas / 60"
        />
        <KpiTile
          label="NER"
          value={fmtPct(nerToday)}
          delta={ppDelta(nerToday, ner(C), 0.1)}
          deltaContext={vs}
          detail={`${fmtInt(T.network_failures)} fallas de red`}
          status={
            nerToday == null
              ? null
              : nerToday >= 99
                ? { level: 'good', label: 'OK' }
                : nerToday >= 97
                  ? { level: 'warning', label: 'Vigilar' }
                  : { level: 'critical', label: 'Crítico' }
          }
          title="Network Effectiveness Ratio (ITU-T E.425): 100% menos las llamadas que falló la red (timeouts, congestión, destino caído). Las no contestadas por decisión del usuario no cuentan."
        />
        {(revenueToday > 0 || revenueCompare > 0) && (
          <KpiTile
            label="Facturado hoy"
            value={`S/ ${revenueToday.toFixed(2)}`}
            delta={pctDelta(revenueToday, revenueCompare, true)}
            deltaContext={vs}
          />
        )}
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
          <span className="font-mono text-sm text-[var(--color-text-primary)]">{call.caller_number}</span>
        </div>
      ),
    },
    {
      key: 'callee_number',
      header: 'Destino',
      render: (call: ActiveCall) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{call.callee_number}</span>
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
      render: (call: ActiveCall) => (
        <span className="font-mono text-sm tabular-nums">{fmtMmss(call.duration_seconds ?? call.duration ?? 0)}</span>
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
              <Activity className="w-3.5 h-3.5" style={{ color: 'var(--viz-status-good)' }} aria-hidden="true" />
            )}
            <span className="text-sm font-medium text-[var(--color-text-primary)] tabular-nums">
              {now.toLocaleTimeString('es-PE', { hour12: false })}
            </span>
            {ageSec != null && (
              <span className="text-xs text-[var(--color-text-tertiary)]">· datos de hace {ageSec}s</span>
            )}
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
            <TrafficChart series={noc.series} currentSlot={noc.current_slot} compare={noc.compare} />
          </div>
          <div className={`grid grid-cols-1 xl:grid-cols-2 gap-4 transition-opacity ${isPlaceholderData ? 'opacity-60' : ''}`}>
            <CausesPanel causes={noc.causes} compare={noc.compare} />
            <RangesTable ranges={noc.ranges} compare={noc.compare} />
          </div>
        </>
      )}

      {/* Live calls */}
      <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <PhoneCall className="w-4 h-4 text-[var(--color-text-tertiary)]" aria-hidden="true" />
            <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Llamadas en curso</h2>
            <span className="text-sm text-[var(--color-text-tertiary)] tabular-nums">{fmtInt(activeCalls.length)}</span>
          </div>
          {activeCalls.length > 0 && <Badge variant="success">En vivo</Badge>}
        </div>
        <DataTable
          columns={callColumns}
          data={activeCalls}
          emptyMessage="No hay llamadas activas en este momento"
          searchable={true}
          searchPlaceholder="Buscar por número origen o destino..."
        />
      </div>
    </div>
  )
}
