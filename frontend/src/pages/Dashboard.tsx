import { useQuery } from '@tanstack/react-query'
import {
  fetchStats,
  fetchActiveCalls,
  fetchCallsByHour,
  fetchRevenueByDay,
  fetchCallsByType,
  fetchCallsByZone,
  fetchTrafficByDirection,
} from '../api/client'
import StatCard from '../components/StatCard'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import {
  Users,
  Phone,
  TrendingUp,
  Activity,
  BarChart3,
  ArrowDownLeft,
  ArrowUpRight,
  RefreshCw,
  PhoneCall,
  Globe,
} from 'lucide-react'
import type { ActiveCall } from '../types'
import {
  LineChart,
  Line,
  BarChart,
  Bar,
  Area,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from 'recharts'
import { useState, useEffect } from 'react'

const CHART_COLORS = {
  primary: '#3b82f6',
  secondary: '#10b981',
  tertiary: '#8b5cf6',
  quaternary: '#f59e0b',
  neutral: '#64748b',
}

const PIE_COLORS = ['#3b82f6', '#10b981', '#8b5cf6', '#f59e0b', '#ec4899', '#06b6d4']

const ZONE_COLORS = [
  '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6',
  '#ec4899', '#14b8a6', '#f97316', '#6366f1',
  '#84cc16', '#06b6d4'
]

function CustomTooltip({ active, payload, label, formatter }: any) {
  if (!active || !payload || !payload.length) return null

  return (
    <div className="bg-[var(--color-bg-card)] backdrop-blur-sm border border-[var(--color-border-primary)] rounded-xl shadow-xl p-3 min-w-[140px]">
      <p className="text-xs font-medium text-[var(--color-text-tertiary)] mb-2">{label}</p>
      {payload.map((entry: any, index: number) => (
        <div key={index} className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-2">
            <div
              className="w-2.5 h-2.5 rounded-full"
              style={{ backgroundColor: entry.color }}
            />
            <span className="text-sm text-[var(--color-text-secondary)]">{entry.name}</span>
          </div>
          <span className="text-sm font-semibold text-[var(--color-text-primary)]">
            {formatter ? formatter(entry.value) : entry.value}
          </span>
        </div>
      ))}
    </div>
  )
}

function ChartCard({
  title,
  icon: Icon,
  children,
  isEmpty,
  emptyIcon: EmptyIcon,
  emptyMessage,
}: {
  title: string
  icon: React.ComponentType<{ className?: string }>
  children: React.ReactNode
  isEmpty?: boolean
  emptyIcon?: React.ComponentType<{ className?: string }>
  emptyMessage?: string
}) {
  return (
    <div className="bg-[var(--color-bg-card)] rounded-2xl shadow-sm border border-[var(--color-border-primary)] p-6 hover:shadow-md transition-all duration-300">
      <div className="flex items-center justify-between mb-6">
        <h3 className="text-base font-semibold text-[var(--color-text-primary)]">{title}</h3>
        <div className="p-2 bg-[var(--color-bg-tertiary)] rounded-lg">
          <Icon className="w-4 h-4 text-[var(--color-text-tertiary)]" />
        </div>
      </div>
      {isEmpty ? (
        <div className="flex flex-col items-center justify-center h-[250px] text-[var(--color-text-tertiary)]">
          {EmptyIcon && <EmptyIcon className="w-12 h-12 mb-3 opacity-40" />}
          <p className="text-sm">{emptyMessage || 'No hay datos disponibles'}</p>
        </div>
      ) : (
        children
      )}
    </div>
  )
}

function LoadingPulse() {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
      {[...Array(4)].map((_, i) => (
        <div
          key={i}
          className="bg-[var(--color-bg-card)] rounded-2xl shadow-sm border border-[var(--color-border-primary)] p-6 h-[140px]"
        >
          <div className="animate-pulse space-y-3">
            <div className="h-4 bg-[var(--color-bg-tertiary)] rounded w-24" />
            <div className="h-8 bg-[var(--color-bg-tertiary)] rounded w-32" />
            <div className="h-3 bg-[var(--color-bg-secondary)] rounded w-20" />
          </div>
        </div>
      ))}
    </div>
  )
}

export default function Dashboard() {
  const [currentTime, setCurrentTime] = useState(new Date())

  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  const { data: stats, isLoading: statsLoading, isRefetching: statsRefetching } = useQuery({
    queryKey: ['stats'],
    queryFn: fetchStats,
    refetchInterval: 10000,
  })

  const { data: activeCallsData = [] } = useQuery({
    queryKey: ['activeCalls'],
    queryFn: fetchActiveCalls,
    refetchInterval: 5000,
  })

  const activeCalls = activeCallsData.map(call => ({ ...call, id: call.uuid }))

  const { data: callsByHourData = [] } = useQuery({
    queryKey: ['callsByHour'],
    queryFn: fetchCallsByHour,
    refetchInterval: 60000,
  })

  const { data: revenueByDayData = [] } = useQuery({
    queryKey: ['revenueByDay'],
    queryFn: fetchRevenueByDay,
    refetchInterval: 60000,
  })

  const { data: callsByTypeData = [] } = useQuery({
    queryKey: ['callsByType'],
    queryFn: fetchCallsByType,
    refetchInterval: 60000,
  })

  const { data: callsByZoneData = [] } = useQuery({
    queryKey: ['callsByZone'],
    queryFn: fetchCallsByZone,
    refetchInterval: 60000,
  })

  const { data: trafficData } = useQuery({
    queryKey: ['trafficByDirection'],
    queryFn: fetchTrafficByDirection,
    refetchInterval: 10000,
  })

  // Transform data for charts
  const llamadasPorHora = callsByHourData.map((stat) => ({
    hora: stat.hour_label,
    llamadas: stat.call_count,
  }))

  const consumoPorDia = revenueByDayData.map((stat) => ({
    dia: stat.day_label,
    consumo: Number(stat.revenue),
  }))

  const llamadasPorTipo = callsByTypeData.map((stat) => ({
    nombre: stat.label,
    valor: stat.call_count,
    porcentaje: stat.percentage.toFixed(1),
  }))

  const llamadasPorZona = callsByZoneData.map((stat) => ({
    zona: stat.zone_name,
    llamadas: stat.call_count,
    porcentaje: stat.percentage.toFixed(1),
  }))

  // Generate sparkline data from trends
  const callsSparkline = callsByHourData.slice(-12).map(d => d.call_count)

  const callColumns = [
    {
      key: 'caller_number',
      header: 'Origen',
      render: (call: ActiveCall) => (
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-full bg-blue-500/10 flex items-center justify-center">
            <Phone className="w-4 h-4 text-blue-600" />
          </div>
          <span className="font-mono text-sm font-medium text-[var(--color-text-primary)]">{call.caller_number}</span>
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
      header: 'Tipo',
      render: (call: ActiveCall) => (
        <div className="flex items-center gap-1.5">
          {call.direction === 'outbound' ? (
            <ArrowUpRight className="w-4 h-4 text-blue-500" />
          ) : call.direction === 'inbound' ? (
            <ArrowDownLeft className="w-4 h-4 text-emerald-500" />
          ) : (
            <RefreshCw className="w-4 h-4 text-[var(--color-text-tertiary)]" />
          )}
          <span className="text-sm text-[var(--color-text-secondary)] capitalize">
            {call.direction === 'outbound' ? 'Saliente' : call.direction === 'inbound' ? 'Entrante' : 'Interna'}
          </span>
        </div>
      ),
    },
    {
      key: 'duration_seconds',
      header: 'Duración',
      render: (call: ActiveCall) => (
        <span className="font-mono text-sm tabular-nums">
          {formatDuration(call.duration_seconds ?? call.duration ?? 0)}
        </span>
      ),
      className: 'text-right',
    },
    {
      key: 'status',
      header: 'Estado',
      render: (call: ActiveCall) => (
        <Badge
          variant={
            call.status === 'answered'
              ? 'success'
              : call.status === 'ringing'
              ? 'warning'
              : 'info'
          }
        >
          {call.status === 'answered'
            ? 'En curso'
            : call.status === 'ringing'
            ? 'Timbrando'
            : 'Marcando'}
        </Badge>
      ),
    },
    {
      key: 'estimated_cost',
      header: 'Costo Est.',
      render: (call: ActiveCall) => (
        <span className="font-mono text-sm font-medium text-[var(--color-text-secondary)]">
          {call.estimated_cost ? `S/${call.estimated_cost.toFixed(4)}` : '-'}
        </span>
      ),
      className: 'text-right',
    },
  ]

  return (
    <div className="space-y-8">
      {/* Header Section */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">Panel de Control</h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">Monitor en tiempo real del sistema de facturación</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-4 py-2 bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] shadow-sm">
            {statsRefetching && (
              <RefreshCw className="w-4 h-4 text-blue-500 animate-spin" />
            )}
            {!statsRefetching && (
              <Activity className="w-4 h-4 text-emerald-500" />
            )}
            <span className="text-sm font-medium text-[var(--color-text-secondary)]">
              {currentTime.toLocaleTimeString('es-ES')}
            </span>
          </div>
        </div>
      </div>

      {/* Stats Grid */}
      {statsLoading ? (
        <LoadingPulse />
      ) : stats ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <StatCard
            title="Cuentas Activas"
            value={stats.active_accounts}
            subtitle={`de ${stats.total_accounts} cuentas totales`}
            icon={Users}
            color="blue"
          />
          <StatCard
            title="Llamadas Activas"
            value={stats.active_calls || activeCalls.length}
            icon={PhoneCall}
            color="green"
            sparklineData={callsSparkline}
          />
          <StatCard
            title="Tráfico Entrante"
            value={`${Number(trafficData?.inbound?.total_minutes ?? 0).toFixed(1)} min`}
            subtitle={`S/${Number(trafficData?.inbound?.total_revenue ?? 0).toFixed(2)} | ${trafficData?.inbound?.total_calls ?? 0} llamadas`}
            icon={ArrowDownLeft}
            color="cyan"
          />
          <StatCard
            title="Tráfico Saliente"
            value={`${Number(trafficData?.outbound?.total_minutes ?? 0).toFixed(1)} min`}
            subtitle={`S/${Number(trafficData?.outbound?.total_revenue ?? 0).toFixed(2)} | ${trafficData?.outbound?.total_calls ?? 0} llamadas`}
            icon={ArrowUpRight}
            color="indigo"
          />
        </div>
      ) : (
        <div className="bg-amber-500/10 border border-amber-500/30 rounded-2xl p-6">
          <div className="flex items-start gap-4">
            <div className="p-2 bg-amber-500/20 rounded-lg">
              <Activity className="w-5 h-5 text-amber-600" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-amber-600">Sistema de estadísticas inactivo</h3>
              <p className="text-sm text-[var(--color-text-secondary)] mt-1">
                No se pudieron cargar las estadísticas. Verifica que el motor de facturación esté activo.
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Charts Grid - Row 1 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <ChartCard title="Llamadas por Hora (Hoy)" icon={BarChart3}>
          <ResponsiveContainer width="100%" height={280}>
            <LineChart data={llamadasPorHora}>
              <defs>
                <linearGradient id="callsGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor={CHART_COLORS.primary} stopOpacity={0.2}/>
                  <stop offset="95%" stopColor={CHART_COLORS.primary} stopOpacity={0}/>
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-primary)" vertical={false} />
              <XAxis
                dataKey="hora"
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={{ stroke: 'var(--color-border-primary)' }}
                tickLine={false}
              />
              <YAxis
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={false}
                tickLine={false}
                width={40}
              />
              <Tooltip content={<CustomTooltip />} />
              <Area
                type="monotone"
                dataKey="llamadas"
                stroke="transparent"
                fill="url(#callsGradient)"
              />
              <Line
                type="monotone"
                dataKey="llamadas"
                name="Llamadas"
                stroke={CHART_COLORS.primary}
                strokeWidth={2.5}
                dot={false}
                activeDot={{ r: 6, fill: CHART_COLORS.primary, strokeWidth: 2, stroke: '#fff' }}
              />
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Consumo de los últimos 7 días" icon={TrendingUp}>
          <ResponsiveContainer width="100%" height={280}>
            <BarChart data={consumoPorDia} barSize={32}>
              <defs>
                <linearGradient id="barGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={CHART_COLORS.secondary} />
                  <stop offset="100%" stopColor="#059669" />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-primary)" vertical={false} />
              <XAxis
                dataKey="dia"
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={{ stroke: 'var(--color-border-primary)' }}
                tickLine={false}
              />
              <YAxis
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={false}
                tickLine={false}
                width={50}
                tickFormatter={(value) => `S/${value}`}
              />
              <Tooltip
                content={<CustomTooltip formatter={(value: number) => `S/${value.toFixed(2)}`} />}
              />
              <Bar
                dataKey="consumo"
                name="Consumo"
                fill="url(#barGradient)"
                radius={[6, 6, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      {/* Charts Grid - Row 2 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <ChartCard
          title="Llamadas por Tipo (7 días)"
          icon={BarChart3}
          isEmpty={llamadasPorTipo.length === 0}
          emptyIcon={BarChart3}
          emptyMessage="No hay datos de llamadas por tipo"
        >
          <ResponsiveContainer width="100%" height={280}>
            <PieChart>
              <Pie
                data={llamadasPorTipo}
                cx="50%"
                cy="50%"
                innerRadius={70}
                outerRadius={100}
                paddingAngle={4}
                dataKey="valor"
                nameKey="nombre"
                strokeWidth={0}
              >
                {llamadasPorTipo.map((_, index) => (
                  <Cell key={`cell-type-${index}`} fill={PIE_COLORS[index % PIE_COLORS.length]} />
                ))}
              </Pie>
              <Tooltip
                content={({ active, payload }) => {
                  if (!active || !payload?.length) return null
                  const data = payload[0].payload
                  return (
                    <div className="bg-[var(--color-bg-card)] backdrop-blur-sm border border-[var(--color-border-primary)] rounded-xl shadow-xl p-3">
                      <p className="text-sm font-semibold text-[var(--color-text-primary)]">{data.nombre}</p>
                      <p className="text-sm text-[var(--color-text-secondary)]">{data.valor} llamadas ({data.porcentaje}%)</p>
                    </div>
                  )
                }}
              />
              <Legend
                verticalAlign="bottom"
                height={36}
                iconType="circle"
                formatter={(value) => <span className="text-sm text-[var(--color-text-secondary)]">{value}</span>}
              />
            </PieChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="Llamadas por Zona (Top 10)"
          icon={Globe}
          isEmpty={llamadasPorZona.length === 0}
          emptyIcon={Globe}
          emptyMessage="No hay datos de llamadas por zona"
        >
          <ResponsiveContainer width="100%" height={280}>
            <BarChart data={llamadasPorZona} layout="vertical" barSize={16}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-primary)" horizontal={false} />
              <XAxis
                type="number"
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={{ stroke: 'var(--color-border-primary)' }}
                tickLine={false}
              />
              <YAxis
                dataKey="zona"
                type="category"
                width={100}
                tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                axisLine={false}
                tickLine={false}
              />
              <Tooltip
                content={({ active, payload }) => {
                  if (!active || !payload?.length) return null
                  const data = payload[0].payload
                  return (
                    <div className="bg-[var(--color-bg-card)] backdrop-blur-sm border border-[var(--color-border-primary)] rounded-xl shadow-xl p-3">
                      <p className="text-sm font-semibold text-[var(--color-text-primary)]">{data.zona}</p>
                      <p className="text-sm text-[var(--color-text-secondary)]">{data.llamadas} llamadas ({data.porcentaje}%)</p>
                    </div>
                  )
                }}
              />
              <Bar
                dataKey="llamadas"
                radius={[0, 4, 4, 0]}
              >
                {llamadasPorZona.map((_, index) => (
                  <Cell key={`cell-zone-${index}`} fill={ZONE_COLORS[index % ZONE_COLORS.length]} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      {/* Active Calls Table */}
      <div className="bg-[var(--color-bg-card)] rounded-2xl shadow-sm border border-[var(--color-border-primary)] p-6">
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-emerald-500/10 rounded-lg">
              <PhoneCall className="w-5 h-5 text-emerald-600" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-[var(--color-text-primary)]">Llamadas Activas</h2>
              <p className="text-sm text-[var(--color-text-tertiary)]">{activeCalls.length} llamadas en curso</p>
            </div>
          </div>
          {activeCalls.length > 0 && (
            <Badge variant="success" className="animate-pulse">
              En vivo
            </Badge>
          )}
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

function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}
