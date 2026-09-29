import { useState, useEffect, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  fetchEndpointStatus,
  fetchProbeResults,
  downloadEvidence,
  downloadEvidenceReport,
} from '../api/client'
import Badge from '../components/Badge'
import {
  Activity,
  Download,
  RefreshCw,
  Wifi,
  WifiOff,
  Server,
  AlertCircle,
  Search,
  Calendar,
} from 'lucide-react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts'

// Time range presets
const TIME_RANGES = [
  { label: '1h', value: 3600 },
  { label: '6h', value: 6 * 3600 },
  { label: '24h', value: 24 * 3600 },
  { label: '7d', value: 7 * 24 * 3600 },
]

// Colors for chart lines
const CHART_COLORS = [
  '#38BDF8', // cyan
  '#34D399', // emerald
  '#F472B6', // pink
  '#FBBF24', // amber
  '#A78BFA', // purple
  '#FB7185', // red
  '#2DD4BF', // teal
  '#818CF8', // indigo
]

export default function EndpointMonitorPage() {
  const [selectedEndpoint, setSelectedEndpoint] = useState<string>('')
  const [searchTerm, setSearchTerm] = useState('')
  const [timeRange, setTimeRange] = useState(3600) // 1 hour default
  const [customFrom, setCustomFrom] = useState('')
  const [customTo, setCustomTo] = useState('')
  const [isCustomRange, setIsCustomRange] = useState(false)
  const [isLive, setIsLive] = useState(true)

  // Calculate from/to timestamps
  const { frm, to } = useMemo(() => {
    if (isCustomRange && customFrom && customTo) {
      return {
        frm: Math.floor(new Date(customFrom).getTime() / 1000),
        to: Math.floor(new Date(customTo).getTime() / 1000),
      }
    }
    const now = Math.floor(Date.now() / 1000)
    return {
      frm: now - timeRange,
      to: now,
    }
  }, [timeRange, isCustomRange, customFrom, customTo])

  // Query for endpoint status (dashboard cards)
  const {
    data: statusData,
    isLoading: statusLoading,
    refetch: refetchStatus,
  } = useQuery({
    queryKey: ['endpoint-status'],
    queryFn: fetchEndpointStatus,
    refetchInterval: isLive ? 30000 : false, // 30s auto-refresh when live
  })

  // Query for probe results (chart + table)
  const {
    data: resultsData,
    isLoading: resultsLoading,
    refetch: refetchResults,
  } = useQuery({
    queryKey: ['probe-results', selectedEndpoint, searchTerm, frm, to],
    queryFn: () => fetchProbeResults({
      name: selectedEndpoint || (searchTerm && !/^\d+\.\d+/.test(searchTerm) ? searchTerm : undefined),
      ip: searchTerm && /^\d+\.\d+/.test(searchTerm) ? searchTerm : undefined,
      frm,
      to,
      limit: 500,
    }),
    refetchInterval: isLive ? 30000 : false,
  })

  // Disable live mode when using custom range
  useEffect(() => {
    if (isCustomRange) {
      setIsLive(false)
    }
  }, [isCustomRange])

  // Manual refresh
  const handleRefresh = () => {
    refetchStatus()
    refetchResults()
  }

  // Download evidence for a single probe
  const handleDownloadEvidence = async (probeId: number) => {
    try {
      const blob = await downloadEvidence(probeId)
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `evidence_${probeId}.txt`
      a.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Failed to download evidence:', error)
    }
  }

  // Download consolidated report
  const handleDownloadReport = async () => {
    try {
      const blob = await downloadEvidenceReport({
        name: selectedEndpoint || undefined,
        ip: searchTerm && /^\d+\.\d+/.test(searchTerm) ? searchTerm : undefined,
        frm,
        to,
        full: true,
      })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `endpoint_report_${new Date().toISOString().split('T')[0]}.txt`
      a.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Failed to download report:', error)
    }
  }

  // Format timestamp
  const formatTime = (ts: number) => {
    return new Date(ts * 1000).toLocaleString('es-ES', {
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
  }

  // Format timestamp for chart - show date if range > 24h
  const formatChartTime = (ts: number) => {
    const date = new Date(ts * 1000)
    // If range is more than 24 hours, show date too
    if (timeRange > 24 * 3600 || isCustomRange) {
      return date.toLocaleDateString('es-ES', {
        day: '2-digit',
        month: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      })
    }
    return date.toLocaleTimeString('es-ES', {
      hour: '2-digit',
      minute: '2-digit',
    })
  }

  // Prepare chart data - merge all series into a single dataset
  const chartData = useMemo(() => {
    if (!resultsData?.series?.length) return []

    // Collect all unique timestamps
    const allTimestamps = new Set<number>()
    resultsData.series.forEach(s => {
      s.points.forEach(p => allTimestamps.add(p.t))
    })

    // Sort timestamps
    const sortedTs = Array.from(allTimestamps).sort((a, b) => a - b)

    // Build data points with all series
    return sortedTs.map(t => {
      const point: Record<string, number | string | null> = { time: t }
      resultsData.series.forEach(s => {
        const key = `${s.name}_rtt`
        const statusKey = `${s.name}_status`
        const matchingPoint = s.points.find(p => p.t === t)
        if (matchingPoint) {
          point[key] = matchingPoint.rtt ? Number(matchingPoint.rtt) : null
          point[statusKey] = matchingPoint.status
        } else {
          point[key] = null
          point[statusKey] = null
        }
      })
      return point
    })
  }, [resultsData])

  // Get unique endpoints for dropdown
  const endpoints = useMemo(() => {
    if (!statusData?.endpoints) return []
    return statusData.endpoints.map(e => ({
      id: e.trunk_id,
      name: e.name,
      ip: e.ip,
    }))
  }, [statusData])

  // Filter endpoints for cards display
  const filteredEndpoints = useMemo(() => {
    if (!statusData?.endpoints) return []
    let filtered = statusData.endpoints
    if (selectedEndpoint) {
      filtered = filtered.filter(e => e.trunk_id === selectedEndpoint)
    }
    if (searchTerm) {
      const term = searchTerm.toLowerCase()
      filtered = filtered.filter(e =>
        e.name.toLowerCase().includes(term) ||
        e.ip.includes(term)
      )
    }
    return filtered
  }, [statusData, selectedEndpoint, searchTerm])

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-[var(--color-text-primary)]">
            Monitor de Endpoints
          </h1>
          <p className="text-sm text-[var(--color-text-muted)]">
            Monitoreo de conectividad SIP via OPTIONS
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* Live indicator */}
          <div className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border ${
            isLive
              ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
              : 'bg-[var(--color-bg-tertiary)] border-[var(--color-border-primary)] text-[var(--color-text-muted)]'
          }`}>
            <div className={`w-2 h-2 rounded-full ${isLive ? 'bg-emerald-500 animate-pulse' : 'bg-gray-500'}`} />
            <span className="text-xs font-medium">
              {isLive ? 'EN VIVO' : 'PAUSADO'}
            </span>
          </div>

          <button
            onClick={handleRefresh}
            disabled={statusLoading || resultsLoading}
            className="p-2 rounded-lg bg-[var(--color-bg-tertiary)] border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:border-[#0099D9]/50 transition-colors disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 ${statusLoading || resultsLoading ? 'animate-spin' : ''}`} />
          </button>

          <button
            onClick={handleDownloadReport}
            className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[#0099D9]/10 border border-[#0099D9]/30 text-[#38BDF8] hover:bg-[#0099D9]/20 transition-colors"
          >
            <Download className="w-4 h-4" />
            <span className="text-sm">Descargar Reporte</span>
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 p-4 rounded-lg bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)]">
        {/* Endpoint selector */}
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
            Endpoint
          </label>
          <select
            value={selectedEndpoint}
            onChange={(e) => setSelectedEndpoint(e.target.value)}
            className="w-full px-3 py-2 rounded-lg bg-[var(--color-bg-primary)] border border-[var(--color-border-primary)] text-[var(--color-text-primary)] text-sm focus:outline-none focus:border-[#0099D9]/50"
          >
            <option value="">Todos los endpoints</option>
            {endpoints.map(ep => (
              <option key={ep.id} value={ep.id}>
                {ep.name} ({ep.ip})
              </option>
            ))}
          </select>
        </div>

        {/* Search */}
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
            Buscar
          </label>
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Nombre o IP..."
              className="w-full pl-9 pr-3 py-2 rounded-lg bg-[var(--color-bg-primary)] border border-[var(--color-border-primary)] text-[var(--color-text-primary)] text-sm placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[#0099D9]/50"
            />
          </div>
        </div>

        {/* Time range */}
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
            Rango de tiempo
          </label>
          <div className="flex gap-1">
            {TIME_RANGES.map(range => (
              <button
                key={range.value}
                onClick={() => {
                  setTimeRange(range.value)
                  setIsCustomRange(false)
                  setIsLive(true)
                }}
                className={`flex-1 px-2 py-2 rounded-lg text-sm font-medium transition-colors ${
                  !isCustomRange && timeRange === range.value
                    ? 'bg-[#0099D9]/20 text-[#38BDF8] border border-[#0099D9]/30'
                    : 'bg-[var(--color-bg-primary)] text-[var(--color-text-secondary)] border border-[var(--color-border-primary)] hover:border-[#0099D9]/30'
                }`}
              >
                {range.label}
              </button>
            ))}
          </div>
        </div>

        {/* Custom range toggle */}
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
            Personalizado
          </label>
          <button
            onClick={() => setIsCustomRange(!isCustomRange)}
            className={`w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
              isCustomRange
                ? 'bg-[#0099D9]/20 text-[#38BDF8] border border-[#0099D9]/30'
                : 'bg-[var(--color-bg-primary)] text-[var(--color-text-secondary)] border border-[var(--color-border-primary)] hover:border-[#0099D9]/30'
            }`}
          >
            <Calendar className="w-4 h-4" />
            {isCustomRange ? 'Activo' : 'Seleccionar'}
          </button>
        </div>

        {/* Custom date inputs */}
        {isCustomRange && (
          <>
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
                Desde
              </label>
              <input
                type="datetime-local"
                value={customFrom}
                onChange={(e) => setCustomFrom(e.target.value)}
                className="w-full px-3 py-2 rounded-lg bg-[var(--color-bg-primary)] border border-[var(--color-border-primary)] text-[var(--color-text-primary)] text-sm focus:outline-none focus:border-[#0099D9]/50"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--color-text-muted)] mb-1.5">
                Hasta
              </label>
              <input
                type="datetime-local"
                value={customTo}
                onChange={(e) => setCustomTo(e.target.value)}
                className="w-full px-3 py-2 rounded-lg bg-[var(--color-bg-primary)] border border-[var(--color-border-primary)] text-[var(--color-text-primary)] text-sm focus:outline-none focus:border-[#0099D9]/50"
              />
            </div>
          </>
        )}
      </div>

      {/* Dashboard Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
        {statusLoading ? (
          // Loading skeletons
          Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="p-4 rounded-lg bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] animate-pulse"
            >
              <div className="h-4 bg-[var(--color-bg-tertiary)] rounded mb-2 w-3/4" />
              <div className="h-3 bg-[var(--color-bg-tertiary)] rounded mb-3 w-1/2" />
              <div className="h-6 bg-[var(--color-bg-tertiary)] rounded w-full" />
            </div>
          ))
        ) : filteredEndpoints.length === 0 ? (
          <div className="col-span-full text-center py-8 text-[var(--color-text-muted)]">
            <AlertCircle className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No hay endpoints disponibles</p>
            <p className="text-xs mt-1">Asegura que el servicio de monitoreo esté activo</p>
          </div>
        ) : (
          filteredEndpoints.map(endpoint => (
            <button
              key={endpoint.trunk_id}
              onClick={() => {
                setSelectedEndpoint(
                  selectedEndpoint === endpoint.trunk_id ? '' : endpoint.trunk_id
                )
              }}
              className={`p-4 rounded-lg text-left transition-all cursor-pointer ${
                endpoint.status === 'up'
                  ? 'bg-emerald-500/5 border-2 border-emerald-500/30 hover:border-emerald-500/50'
                  : 'bg-red-500/5 border-2 border-red-500/30 hover:border-red-500/50'
              } ${selectedEndpoint === endpoint.trunk_id ? 'ring-2 ring-[#0099D9]/50' : ''}`}
            >
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-medium text-[var(--color-text-primary)] truncate">
                  {endpoint.name}
                </span>
                {endpoint.status === 'up' ? (
                  <Wifi className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                ) : (
                  <WifiOff className="w-4 h-4 text-red-400 flex-shrink-0" />
                )}
              </div>
              <div className="text-xs text-[var(--color-text-muted)] mb-2">
                {endpoint.ip}:{endpoint.port}
              </div>
              <div className="flex items-center justify-between">
                <Badge
                  variant={endpoint.status === 'up' ? 'success' : 'error'}
                  size="sm"
                >
                  {endpoint.status === 'up' ? 'Activo' : 'Caído'}
                </Badge>
                {endpoint.rtt_ms && (
                  <span className={`text-xs font-mono ${
                    Number(endpoint.rtt_ms) > 150
                      ? 'text-amber-400'
                      : 'text-emerald-400'
                  }`}>
                    {Number(endpoint.rtt_ms).toFixed(1)} ms
                  </span>
                )}
              </div>
            </button>
          ))
        )}
      </div>

      {/* RTT Chart */}
      <div className="p-4 rounded-lg bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)]">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-[var(--color-text-primary)]">
            Latencia (RTT)
          </h2>
          {resultsData && (
            <span className="text-xs text-[var(--color-text-muted)]">
              Bucket: {resultsData.bucket_seconds}s
            </span>
          )}
        </div>

        {resultsLoading ? (
          <div className="h-64 flex items-center justify-center">
            <RefreshCw className="w-6 h-6 animate-spin text-[var(--color-text-muted)]" />
          </div>
        ) : chartData.length === 0 ? (
          <div className="h-64 flex items-center justify-center text-[var(--color-text-muted)]">
            <div className="text-center">
              <Activity className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <p>Sin datos de latencia</p>
            </div>
          </div>
        ) : (
          <div className="h-64">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-primary)" />
                <XAxis
                  dataKey="time"
                  type="number"
                  domain={[frm, to]}
                  tickFormatter={formatChartTime}
                  stroke="var(--color-text-muted)"
                  fontSize={11}
                  scale="time"
                />
                <YAxis
                  stroke="var(--color-text-muted)"
                  fontSize={11}
                  tickFormatter={(v) => `${v} ms`}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: 'var(--color-bg-secondary)',
                    border: '1px solid var(--color-border-primary)',
                    borderRadius: '8px',
                  }}
                  labelFormatter={(ts) => formatTime(ts as number)}
                  formatter={(value, name) => {
                    if (value === null || value === undefined) return ['Caído', String(name).replace('_rtt', '')]
                    return [`${Number(value).toFixed(2)} ms`, String(name).replace('_rtt', '')]
                  }}
                />
                <Legend />
                {resultsData?.series.map((s, idx) => (
                  <Line
                    key={s.trunk_id}
                    type="monotone"
                    dataKey={`${s.name}_rtt`}
                    name={s.name}
                    stroke={CHART_COLORS[idx % CHART_COLORS.length]}
                    strokeWidth={2}
                    dot={(props: any) => {
                      const { cx, cy, payload } = props
                      const statusKey = `${s.name}_status`
                      const status = payload?.[statusKey]
                      if (status === 'down') {
                        return (
                          <circle
                            cx={cx}
                            cy={cy || 0}
                            r={5}
                            fill="#EF4444"
                            stroke="#EF4444"
                          />
                        )
                      }
                      return null
                    }}
                    connectNulls={false}
                  />
                ))}
              </LineChart>
            </ResponsiveContainer>
          </div>
        )}
      </div>

      {/* Probes Table */}
      <div className="p-4 rounded-lg bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)]">
        <h2 className="text-lg font-semibold text-[var(--color-text-primary)] mb-4">
          Sondas Recientes
        </h2>

        {resultsLoading ? (
          <div className="h-32 flex items-center justify-center">
            <RefreshCw className="w-6 h-6 animate-spin text-[var(--color-text-muted)]" />
          </div>
        ) : !resultsData?.rows?.length ? (
          <div className="text-center py-8 text-[var(--color-text-muted)]">
            <Server className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>Sin registros de sondas</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--color-border-primary)]">
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    Hora
                  </th>
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    Endpoint
                  </th>
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    IP
                  </th>
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    Estado
                  </th>
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    Código
                  </th>
                  <th className="text-left py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    RTT
                  </th>
                  <th className="text-right py-3 px-4 text-[var(--color-text-muted)] font-medium">
                    Traza
                  </th>
                </tr>
              </thead>
              <tbody>
                {resultsData.rows.map(row => (
                  <tr
                    key={row.id}
                    className="border-b border-[var(--color-border-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors"
                  >
                    <td className="py-3 px-4 text-[var(--color-text-primary)] font-mono text-xs">
                      {formatTime(row.ts)}
                    </td>
                    <td className="py-3 px-4 text-[var(--color-text-primary)]">
                      {row.name}
                    </td>
                    <td className="py-3 px-4 text-[var(--color-text-muted)] font-mono">
                      {row.ip}:{row.port}
                    </td>
                    <td className="py-3 px-4">
                      <Badge
                        variant={row.status === 'up' ? 'success' : 'error'}
                        size="sm"
                      >
                        {row.status === 'up' ? 'Activo' : 'Caído'}
                      </Badge>
                    </td>
                    <td className="py-3 px-4 text-[var(--color-text-muted)] font-mono">
                      {row.resp_code || '-'}
                    </td>
                    <td className="py-3 px-4">
                      {row.rtt_ms ? (
                        <span className={`font-mono ${
                          Number(row.rtt_ms) > 150
                            ? 'text-amber-400'
                            : 'text-emerald-400'
                        }`}>
                          {Number(row.rtt_ms).toFixed(2)} ms
                        </span>
                      ) : (
                        <span className="text-[var(--color-text-muted)]">-</span>
                      )}
                    </td>
                    <td className="py-3 px-4 text-right">
                      <button
                        onClick={() => handleDownloadEvidence(row.id)}
                        className="p-1.5 rounded hover:bg-[var(--color-bg-primary)] text-[var(--color-text-muted)] hover:text-[#38BDF8] transition-colors"
                        title="Descargar traza"
                      >
                        <Download className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  )
}
