import { useCallback, useRef, useState } from 'react'
import { useQuery, keepPreviousData } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import { fetchCDRs, fetchCDRSummary, exportCDRs } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import QuickRangeBar from '../components/QuickRangeBar'
import { FileText, Download, Search, Filter, X, AlertTriangle, Copy } from 'lucide-react'
import type { CDR, CDRFilters, CdrSummary } from '../types'
import { callOutcome } from '../lib/callStatus'
import {
  DEFAULT_RANGE,
  describeRange,
  formatLimaDateTime,
  isRangeKey,
  resolveRange,
  toApiTimestamp,
  toLimaLocal,
  type RangeKey,
} from '../lib/cdrTimeRanges'

// Interconnection SBC: this screen serves traffic reconciliation with carriers
// (Integratel), keyed on MSISDN, date/time, status and duration. Filters live
// in the URL (/cdr?range=yesterday&direction=inbound) so a view can be pasted
// into a ticket or the NOC chat. All of them are applied in SQL.
const URL_FILTERS = ['caller', 'callee', 'direction', 'outcome', 'min_billsec'] as const
type UrlFilter = (typeof URL_FILTERS)[number]

// Same thresholds the backend uses (CDR_LONG_CALL_SECS / CDR_SHORT_CALL_SECS);
// the summary returns them, these are only the first-render fallback
const DEFAULT_LONG_CALL_SECS = 7200
const DEFAULT_SHORT_CALL_SECS = 3

const inputClass =
  'w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]'
const labelClass = 'block text-sm font-medium text-[var(--color-text-secondary)] mb-1'

export default function CDRPage() {
  const [page, setPage] = useState(1)
  const [showFilters, setShowFilters] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [exporting, setExporting] = useState(false)
  const [exportError, setExportError] = useState<string | null>(null)
  const [searchParams, setSearchParams] = useSearchParams()
  // Newest CDR id when page 1 was read. Later pages send it back as max_id,
  // so CDRs written while paging don't shift rows (no repeats, no gaps).
  const snapshotRef = useRef<number | undefined>(undefined)
  const [snapshot, setSnapshot] = useState<number | undefined>(undefined)

  const rawRange = searchParams.get('range')
  const range: RangeKey = isRangeKey(rawRange) ? rawRange : DEFAULT_RANGE
  const customFrom = searchParams.get('from') ?? ''
  const customTo = searchParams.get('to') ?? ''
  const filters: Pick<CDRFilters, UrlFilter> = Object.fromEntries(
    URL_FILTERS.flatMap((k) => {
      const v = searchParams.get(k)
      return v ? [[k, v]] : []
    })
  )

  const updateParams = useCallback(
    (changes: Record<string, string | undefined>) => {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev)
          Object.entries(changes).forEach(([k, v]) => (v ? next.set(k, v) : next.delete(k)))
          return next
        },
        { replace: true }
      )
      setPage(1)
    },
    [setSearchParams]
  )

  const selectRange = useCallback(
    (key: Exclude<RangeKey, 'custom'>) => updateParams({ range: key, from: undefined, to: undefined }),
    [updateParams]
  )

  // The range is resolved when the query runs, not when the button is clicked,
  // so "Hoy" / "Última hora" keep moving on every refresh.
  const buildFilters = (
    nowMs: number
  ): { apiFilters: CDRFilters; resolved: ReturnType<typeof resolveRange> } => {
    const resolved = resolveRange(range, nowMs, { from: customFrom, to: customTo })
    return {
      resolved,
      apiFilters: {
        ...filters,
        start_date: resolved.start ? toApiTimestamp(resolved.start) : undefined,
        end_date: resolved.end ? toApiTimestamp(resolved.end) : undefined,
      },
    }
  }

  const openEnded = range === 'custom' ? !customTo : resolveRange(range, 0).end === null

  const { data, isLoading, isFetching } = useQuery({
    queryKey: ['cdrs', page, range, customFrom, customTo, filters],
    queryFn: async () => {
      const { apiFilters, resolved } = buildFilters(Date.now())
      const maxId = page > 1 ? snapshotRef.current : undefined
      const result = await fetchCDRs({ ...apiFilters, max_id: maxId }, page, 50)
      if (page === 1) {
        snapshotRef.current = result.snapshot
        setSnapshot(result.snapshot)
      }
      return { ...result, rangeLabel: describeRange(resolved), apiFilters }
    },
    placeholderData: keepPreviousData,
    // New CDRs only appear in ranges that end "now", and only on page 1: a
    // refresh takes a new snapshot, later pages stay on theirs
    refetchInterval: autoRefresh && openEnded && page === 1 ? 30_000 : false,
  })

  // Totals for the same rows the table pages through (same snapshot)
  const { data: summary } = useQuery({
    queryKey: ['cdrs-summary', snapshot, range, customFrom, customTo, filters],
    queryFn: () => fetchCDRSummary({ ...data!.apiFilters, max_id: snapshot }),
    enabled: !!data && snapshot !== undefined,
    placeholderData: keepPreviousData,
  })

  const longSecs = summary?.long_call_secs ?? DEFAULT_LONG_CALL_SECS
  const shortSecs = summary?.short_call_secs ?? DEFAULT_SHORT_CALL_SECS

  const handleExport = async () => {
    setExporting(true)
    setExportError(null)
    try {
      const { blob, filename } = await exportCDRs(buildFilters(Date.now()).apiFilters)
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = filename
      a.click()
      window.URL.revokeObjectURL(url)
    } catch (error) {
      setExportError(error instanceof Error ? error.message : 'No se pudo generar el export')
    } finally {
      setExporting(false)
    }
  }

  const sub = 'text-xs text-[var(--color-text-tertiary)]'

  const columns = [
    {
      key: 'start_time',
      header: 'Inicio (GMT-5)',
      // Lima time, like the filters and the CSV, whatever the PC's zone
      render: (cdr: CDR) => {
        const [date, time] = formatLimaDateTime(new Date(cdr.start_time))
        return (
          <div className="whitespace-nowrap">
            <div className="text-[var(--color-text-primary)]">{date}</div>
            <div className={sub}>{time}</div>
          </div>
        )
      },
    },
    {
      key: 'caller',
      header: 'Origen',
      render: (cdr: CDR) => {
        const raw = cdr.caller || cdr.caller_number || ''
        const shown = cdr.caller_e164 || raw || '-'
        return (
          <div className="whitespace-nowrap" title={cdr.source_ip ? `INVITE desde ${cdr.source_ip}` : undefined}>
            <div className="font-mono text-[var(--color-text-primary)]">{shown}</div>
            {raw && raw !== shown && <div className={`${sub} font-mono`}>recibido: {raw}</div>}
            {!cdr.caller_e164 && raw && <div className="text-xs text-amber-600">no es un número E.164</div>}
          </div>
        )
      },
    },
    {
      key: 'callee',
      header: 'Destino',
      render: (cdr: CDR) => {
        const raw = cdr.callee || cdr.callee_number || ''
        const shown = cdr.destination || raw || '-'
        return (
          <div className="whitespace-nowrap">
            <div className="font-mono text-[var(--color-text-secondary)]">{shown}</div>
            {raw && raw !== shown && <div className={`${sub} font-mono`}>marcado: {raw}</div>}
          </div>
        )
      },
    },
    {
      key: 'direction',
      header: 'Dir.',
      render: (cdr: CDR) => (
        <Badge variant={cdr.direction === 'outbound' ? 'info' : cdr.direction === 'inbound' ? 'success' : 'default'}>
          {cdr.direction === 'outbound' ? 'Saliente' : cdr.direction === 'inbound' ? 'Entrante' : 'Interna'}
        </Badge>
      ),
    },
    {
      key: 'billsec',
      header: 'Conv. / Total',
      render: (cdr: CDR) => {
        const answered = !!cdr.answer_time
        const long = answered && cdr.billsec >= longSecs
        const short = answered && cdr.billsec <= shortSecs
        return (
          <div className="whitespace-nowrap text-right">
            <div className="text-[var(--color-text-primary)]">{formatDuration(cdr.billsec ?? 0)}</div>
            <div className={sub}>total {formatDuration(cdr.duration ?? 0)}</div>
            {long && (
              <span title={`Contestada y de ${formatDuration(longSecs)} o más: revisar`}>
                <Badge variant="warning">Larga</Badge>
              </span>
            )}
            {short && (
              <span title={`Contestada y de ${shortSecs} s o menos: revisar`}>
                <Badge variant="warning">Muy corta</Badge>
              </span>
            )}
          </div>
        )
      },
      className: 'text-right',
    },
    {
      key: 'hangup_cause',
      header: 'Estado',
      render: (cdr: CDR) => {
        const outcome = callOutcome(cdr)
        return (
          <div className="min-w-[9rem]">
            <Badge variant={outcome.variant}>{outcome.label}</Badge>
            <div className={`${sub} font-mono mt-0.5`}>
              {cdr.hangup_cause || '—'}
              {' · SIP '}
              {cdr.sip_code ?? <span title="El código SIP se registra desde el 25/10/2026">n/d</span>}
            </div>
          </div>
        )
      },
    },
    {
      key: 'call_uuid',
      header: 'ID llamada',
      render: (cdr: CDR) => (
        <button
          type="button"
          onClick={() => navigator.clipboard?.writeText(cdr.call_uuid)}
          title={`${cdr.call_uuid}\nCDR #${cdr.id} · clic para copiar`}
          className="flex items-center font-mono text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]"
        >
          {cdr.call_uuid.slice(0, 8)}
          <Copy className="w-3 h-3 ml-1" />
        </button>
      ),
    },
  ]

  const clearFilters = () =>
    updateParams(Object.fromEntries(URL_FILTERS.map((k) => [k, undefined])))

  const activeFilterCount = Object.keys(filters).length
  const hasActiveFilters = activeFilterCount > 0

  // Custom range: datetime-local values are Lima time. Starting from a quick
  // range pre-fills its current bounds.
  const setCustomBound = (bound: 'from' | 'to', value: string) => {
    const current = resolveRange(range, Date.now(), { from: customFrom, to: customTo })
    updateParams({
      range: 'custom',
      from: bound === 'from' ? value : customFrom || (current.start ? toLimaLocal(current.start) : ''),
      to: bound === 'to' ? value : range === 'custom' ? customTo : current.end ? toLimaLocal(current.end) : '',
    })
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">
            Registros de Llamadas (CDR)
          </h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Historial detallado de todas las llamadas procesadas
          </p>
        </div>
        <div className="flex items-center space-x-3">
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center px-4 py-2 rounded-lg border transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-[#fff3ed] border-[#ffbb9a] text-[#d8450b]'
                : 'border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]'
            }`}
          >
            <Filter className="w-4 h-4 mr-2" />
            Filtros
            {hasActiveFilters && (
              <span className="ml-2 bg-[#ff5a0a] text-white text-xs px-2 py-0.5 rounded-full">
                {activeFilterCount}
              </span>
            )}
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="flex items-center px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors disabled:opacity-60 disabled:cursor-wait"
          >
            <Download className="w-4 h-4 mr-2" />
            {exporting ? 'Exportando…' : 'Exportar CSV'}
          </button>
        </div>
      </div>

      {exportError && (
        <div className="flex items-start justify-between gap-3 rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-950/40 dark:text-red-300">
          <div className="flex items-start">
            <AlertTriangle className="w-4 h-4 mr-2 mt-0.5 shrink-0" />
            <span>
              <strong>No se pudo exportar:</strong> {exportError}
            </span>
          </div>
          <button onClick={() => setExportError(null)} aria-label="Cerrar">
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      <QuickRangeBar
        value={range}
        onSelect={selectRange}
        onCustom={() => {
          setShowFilters(true)
          if (range !== 'custom') setCustomBound('from', '')
        }}
        description={data?.rangeLabel}
        openEnded={openEnded}
        autoRefresh={autoRefresh}
        onToggleAutoRefresh={() => setAutoRefresh((v) => !v)}
        refreshing={isFetching}
      />

      {summary && summary.system_cut > 0 && filters.outcome !== 'shutdown' && (
        <OutageBanner summary={summary} onShow={() => updateParams({ outcome: 'shutdown' })} />
      )}

      {/* Filters Panel */}
      {showFilters && (
        <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="font-semibold text-[var(--color-text-primary)]">Filtros de Búsqueda</h3>
            {hasActiveFilters && (
              <button
                onClick={clearFilters}
                className="text-sm text-red-600 hover:text-red-700 flex items-center"
              >
                <X className="w-4 h-4 mr-1" />
                Limpiar filtros
              </button>
            )}
          </div>
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            <div>
              <label className={labelClass}>Desde (hora Lima)</label>
              <input
                type="datetime-local"
                value={range === 'custom' ? customFrom : ''}
                onChange={(e) => setCustomBound('from', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>Hasta (vacío = ahora)</label>
              <input
                type="datetime-local"
                value={range === 'custom' ? customTo : ''}
                onChange={(e) => setCustomBound('to', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>Número Origen</label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={filters.caller || ''}
                  onChange={(e) => updateParams({ caller: e.target.value })}
                  placeholder="Ej: 17486128"
                  className={`${inputClass} pl-10`}
                />
              </div>
            </div>
            <div>
              <label className={labelClass}>Número Destino</label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={filters.callee || ''}
                  onChange={(e) => updateParams({ callee: e.target.value })}
                  placeholder="Ej: 993171615"
                  className={`${inputClass} pl-10`}
                />
              </div>
            </div>
            <div>
              <label className={labelClass}>Dirección</label>
              <select
                value={filters.direction || ''}
                onChange={(e) => updateParams({ direction: e.target.value })}
                className={inputClass}
              >
                <option value="">Todas</option>
                <option value="outbound">Saliente</option>
                <option value="inbound">Entrante</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>Estado</label>
              <select
                value={filters.outcome || ''}
                onChange={(e) => updateParams({ outcome: e.target.value })}
                className={inputClass}
              >
                <option value="">Todos</option>
                <option value="answered">Contestadas</option>
                <option value="unanswered">No contestadas</option>
                <option value="shutdown">Cortadas por el sistema</option>
                <option value="flagged">Marcadas para revisión</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>Conversación mínima (s)</label>
              <input
                type="number"
                min={0}
                value={filters.min_billsec || ''}
                onChange={(e) => updateParams({ min_billsec: e.target.value })}
                placeholder="Ej: 60"
                className={inputClass}
              />
            </div>
          </div>
        </div>
      )}

      {summary && <SummaryStrip summary={summary} longSecs={longSecs} shortSecs={shortSecs} />}

      {/* Summary */}
      {data && (
        <div className="flex items-center justify-between bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)] p-4">
          <div className="flex items-center">
            <FileText className="w-5 h-5 text-[var(--color-text-muted)] mr-2" />
            <span className="text-[var(--color-text-secondary)]">
              Página {page}: {(data.data ?? data.items ?? []).length} de {data.total.toLocaleString('es-PE')}{' '}
              registros
              {page > 1 && ' · lista fija desde la página 1 (las llamadas nuevas aparecen al volver a ella)'}
            </span>
          </div>
        </div>
      )}

      {/* CDR Table */}
      <DataTable
        columns={columns}
        data={data?.data ?? data?.items ?? []}
        loading={isLoading}
        emptyMessage="No se encontraron registros de llamadas"
        pagination={
          data
            ? {
                page,
                totalPages: data.total_pages,
                onPageChange: setPage,
              }
            : undefined
        }
      />
    </div>
  )
}

/** ASR, ACD, minutes and review counters of the whole filtered set */
function SummaryStrip({ summary, longSecs, shortSecs }: { summary: CdrSummary; longSecs: number; shortSecs: number }) {
  const nf = new Intl.NumberFormat('es-PE')
  const pct = (n: number, d: number) => (d > 0 ? `${new Intl.NumberFormat('es-PE', { maximumFractionDigits: 1 }).format((100 * n) / d)} %` : '—')
  const tiles: { label: string; value: string; hint: string; tone?: 'warn' | 'bad' }[] = [
    { label: 'Llamadas', value: nf.format(summary.total), hint: 'Intentos en el rango y filtros' },
    {
      label: 'Contestadas (ASR)',
      value: pct(summary.answered, summary.total),
      hint: `${nf.format(summary.answered)} con hora de contestación`,
    },
    {
      label: 'Duración media (ACD)',
      value: summary.answered > 0 ? formatDuration(Math.round(summary.billsec / summary.answered)) : '—',
      hint: 'Conversación promedio de las contestadas',
    },
    {
      label: 'Minutos conversados',
      value: nf.format(Math.round(summary.billsec / 60)),
      hint: 'Suma de la conversación de las contestadas',
    },
    {
      label: 'Marcadas para revisión',
      value: nf.format(summary.long_calls + summary.short_answered),
      hint: `${nf.format(summary.long_calls)} de ${formatDuration(longSecs)} o más · ${nf.format(summary.short_answered)} contestadas de ${shortSecs} s o menos`,
      tone: summary.long_calls + summary.short_answered > 0 ? 'warn' : undefined,
    },
    {
      label: 'Cortadas por el sistema',
      value: nf.format(summary.system_cut),
      hint: `${nf.format(summary.system_cut_answered)} estaban contestadas`,
      tone: summary.system_cut > 0 ? 'bad' : undefined,
    },
  ]
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 xl:grid-cols-6 gap-3">
      {tiles.map((t) => (
        <div
          key={t.label}
          title={t.hint}
          className="bg-[var(--color-bg-card)] rounded-lg border border-[var(--color-border-primary)] p-3"
        >
          <div className="text-xs text-[var(--color-text-tertiary)]">{t.label}</div>
          <div
            className={`text-xl font-semibold tabular-nums ${
              t.tone === 'bad' ? 'text-red-600' : t.tone === 'warn' ? 'text-amber-600' : 'text-[var(--color-text-primary)]'
            }`}
          >
            {t.value}
          </div>
          <div className="text-xs text-[var(--color-text-tertiary)] truncate">{t.hint}</div>
        </div>
      ))}
    </div>
  )
}

/** Calls cut by platform restarts in the range, grouped into outages */
function OutageBanner({ summary, onShow }: { summary: CdrSummary; onShow: () => void }) {
  const hhmm = (iso: string) => formatLimaDateTime(new Date(iso))[1].slice(0, 5)
  const day = (iso: string) => formatLimaDateTime(new Date(iso))[0].slice(0, 5)
  return (
    <div className="flex items-start justify-between gap-3 rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-950/40 dark:text-red-300">
      <div className="flex items-start">
        <AlertTriangle className="w-4 h-4 mr-2 mt-0.5 shrink-0" />
        <div>
          <strong>
            {summary.system_cut.toLocaleString('es-PE')} llamadas cortadas por reinicio o caída del sistema
          </strong>{' '}
          ({summary.system_cut_answered.toLocaleString('es-PE')} estaban contestadas; afectan duración facturable y SLA)
          <ul className="mt-1 list-disc pl-5">
            {summary.outages.map((o) => (
              <li key={o.from}>
                {day(o.from)} {hhmm(o.from)}–{hhmm(o.to)} (GMT-5): {o.calls} llamadas, {o.answered} contestadas
              </li>
            ))}
          </ul>
        </div>
      </div>
      <button onClick={onShow} className="shrink-0 underline font-medium">
        Ver llamadas
      </button>
    </div>
  )
}

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const mins = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60

  if (hours > 0) {
    return `${hours}:${mins.toString().padStart(2, '0')}:${secs
      .toString()
      .padStart(2, '0')}`
  }
  return `${mins}:${secs.toString().padStart(2, '0')}`
}
