import { useCallback, useState } from 'react'
import { useQuery, keepPreviousData } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import { fetchCDRs, exportCDRs } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import QuickRangeBar from '../components/QuickRangeBar'
import { FileText, Download, Search, Filter, X, AlertTriangle } from 'lucide-react'
import type { CDR, CDRFilters } from '../types'
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

// Filters live in the URL (/cdr?range=yesterday&direction=inbound) so a view can
// be pasted into a ticket or the NOC chat.
const URL_FILTERS = ['caller', 'callee', 'direction'] as const
type UrlFilter = (typeof URL_FILTERS)[number]

export default function CDRPage() {
  const [page, setPage] = useState(1)
  const [showFilters, setShowFilters] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [exporting, setExporting] = useState(false)
  const [exportError, setExportError] = useState<string | null>(null)
  const [searchParams, setSearchParams] = useSearchParams()

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
      const result = await fetchCDRs(apiFilters, page, 50)
      return { ...result, rangeLabel: describeRange(resolved) }
    },
    placeholderData: keepPreviousData,
    // New CDRs only appear in ranges that end "now", and only on page 1: on
    // later pages a refresh would shift rows under the operator (skipped or
    // repeated CDRs while paging)
    refetchInterval: autoRefresh && openEnded && page === 1 ? 30_000 : false,
  })

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

  const columns = [
    {
      key: 'start_time',
      header: 'Fecha/Hora',
      // Lima time, like the filters and the CSV, whatever the PC's zone
      render: (cdr: CDR) => {
        const [date, time] = formatLimaDateTime(new Date(cdr.start_time))
        return (
          <div>
            <div className="text-[var(--color-text-primary)]">{date}</div>
            <div className="text-xs text-[var(--color-text-tertiary)]">{time}</div>
          </div>
        )
      },
    },
    {
      key: 'caller',
      header: 'Origen',
      render: (cdr: CDR) => (
        <span className="font-mono text-[var(--color-text-primary)]">{cdr.caller || cdr.caller_number || '-'}</span>
      ),
    },
    {
      key: 'callee',
      header: 'Destino',
      render: (cdr: CDR) => (
        <div>
          <span className="font-mono text-[var(--color-text-secondary)]">{cdr.callee || cdr.callee_number || '-'}</span>
          {cdr.destination && (
            <span className="text-xs text-[var(--color-text-tertiary)] block">{cdr.destination}</span>
          )}
        </div>
      ),
    },
    {
      key: 'direction',
      header: 'Dirección',
      render: (cdr: CDR) => (
        <Badge
          variant={
            cdr.direction === 'outbound'
              ? 'info'
              : cdr.direction === 'inbound'
              ? 'success'
              : 'default'
          }
        >
          {cdr.direction === 'outbound'
            ? 'Saliente'
            : cdr.direction === 'inbound'
            ? 'Entrante'
            : 'Interna'}
        </Badge>
      ),
    },
    {
      key: 'duration',
      header: 'Duración',
      render: (cdr: CDR) => formatDuration(cdr.duration ?? 0),
      className: 'text-right',
    },
    {
      key: 'billsec',
      header: 'Conversación',
      render: (cdr: CDR) => formatDuration(cdr.billsec ?? 0),
      className: 'text-right',
    },
    {
      key: 'hangup_cause',
      header: 'Estado',
      render: (cdr: CDR) => {
        const outcome = callOutcome(cdr)
        return (
          <span title={cdr.hangup_cause}>
            <Badge variant={outcome.variant}>{outcome.label}</Badge>
          </span>
        )
      },
    },
  ]

  const clearFilters = () =>
    updateParams({ caller: undefined, callee: undefined, direction: undefined })

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
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Desde (hora Lima)
              </label>
              <input
                type="datetime-local"
                value={range === 'custom' ? customFrom : ''}
                onChange={(e) => setCustomBound('from', e.target.value)}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Hasta (vacío = ahora)
              </label>
              <input
                type="datetime-local"
                value={range === 'custom' ? customTo : ''}
                onChange={(e) => setCustomBound('to', e.target.value)}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Número Origen
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={filters.caller || ''}
                  onChange={(e) => updateParams({ caller: e.target.value })}
                  placeholder="Ej: 1001"
                  className="w-full pl-10 pr-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]"
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Número Destino
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={filters.callee || ''}
                  onChange={(e) => updateParams({ callee: e.target.value })}
                  placeholder="Ej: 51987654321"
                  className="w-full pl-10 pr-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]"
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Dirección
              </label>
              <select
                value={filters.direction || ''}
                onChange={(e) => updateParams({ direction: e.target.value })}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a]"
              >
                <option value="">Todas</option>
                <option value="outbound">Saliente</option>
                <option value="inbound">Entrante</option>
                <option value="internal">Interna</option>
              </select>
            </div>
          </div>
        </div>
      )}

      {/* Summary */}
      {data && (
        <div className="flex items-center justify-between bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)] p-4">
          <div className="flex items-center">
            <FileText className="w-5 h-5 text-[var(--color-text-muted)] mr-2" />
            <span className="text-[var(--color-text-secondary)]">
              Mostrando {(data.data ?? data.items ?? []).length} de {data.total} registros
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
        searchable={true}
        searchPlaceholder="Filtrar solo las filas de esta página…"
      />
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
