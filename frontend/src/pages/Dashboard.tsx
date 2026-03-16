import { useQuery } from '@tanstack/react-query'
import {
  fetchStats,
  fetchActiveCalls,
  fetchActiveReservations,
  fetchCallsByHour,
  fetchRevenueByDay,
  fetchBalanceTrend,
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
  DollarSign,
  TrendingUp,
  Clock,
  Activity,
  BarChart3,
} from 'lucide-react'
import { formatDistanceToNow } from 'date-fns'
import { es } from 'date-fns/locale'
import type { ActiveCall, Reservation } from '../types'
import {
  LineChart,
  Line,
  BarChart,
  Bar,
  AreaChart,
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

export default function Dashboard() {
  const { data: stats, isLoading: statsLoading } = useQuery({
    queryKey: ['stats'],
    queryFn: fetchStats,
    refetchInterval: 10000,
  })

  const { data: activeCallsData = [] } = useQuery({
    queryKey: ['activeCalls'],
    queryFn: fetchActiveCalls,
    refetchInterval: 5000,
  })

  const { data: reservations = [] } = useQuery({
    queryKey: ['activeReservations'],
    queryFn: fetchActiveReservations,
    refetchInterval: 5000,
  })

  // Agregar id para compatibilidad con DataTable
  const activeCalls = activeCallsData.map(call => ({ ...call, id: call.uuid }))

  // Cargar datos reales de estadísticas
  const { data: callsByHourData = [] } = useQuery({
    queryKey: ['callsByHour'],
    queryFn: fetchCallsByHour,
    refetchInterval: 60000, // Actualizar cada minuto
  })

  const { data: revenueByDayData = [] } = useQuery({
    queryKey: ['revenueByDay'],
    queryFn: fetchRevenueByDay,
    refetchInterval: 60000,
  })

  const { data: balanceTrendData = [] } = useQuery({
    queryKey: ['balanceTrend'],
    queryFn: fetchBalanceTrend,
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
    refetchInterval: 10000, // Actualizar cada 10 segundos (datos de hoy)
  })

  // Transformar datos del backend para las gráficas
  const llamadasPorHora = callsByHourData.map((stat) => ({
    hora: stat.hour_label,
    llamadas: stat.call_count,
  }))

  const ingresosPorDia = revenueByDayData.map((stat) => ({
    dia: stat.day_label,
    ingresos: Number(stat.revenue),
  }))

  const distribucionTipoLlamada = [
    { nombre: 'Salientes', valor: activeCalls.filter(c => c.direction === 'outbound').length, color: '#3b82f6' },
    { nombre: 'Entrantes', valor: activeCalls.filter(c => c.direction === 'inbound').length, color: '#10b981' },
    { nombre: 'Internas', valor: activeCalls.filter(c => c.direction === 'internal').length, color: '#64748b' },
  ]

  const tendenciaSaldos = balanceTrendData.map((point) => ({
    dia: point.day,
    saldo: Number(point.total_balance),
  }))

  // Datos para gráfico de llamadas por tipo (últimos 30 días)
  const llamadasPorTipo = callsByTypeData.map((stat) => ({
    nombre: stat.label,
    valor: stat.call_count,
    porcentaje: stat.percentage.toFixed(1),
  }))

  // Colores para el gráfico de tipo
  const TIPO_COLORS = ['#3b82f6', '#10b981', '#64748b', '#f59e0b']

  // Datos para gráfico de llamadas por zona (top 10)
  const llamadasPorZona = callsByZoneData.map((stat) => ({
    zona: stat.zone_name,
    llamadas: stat.call_count,
    porcentaje: stat.percentage.toFixed(1),
  }))

  // Colores para el gráfico de zonas
  const ZONA_COLORS = [
    '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6',
    '#ec4899', '#14b8a6', '#f97316', '#6366f1',
    '#84cc16', '#06b6d4'
  ]

  const callColumns = [
    {
      key: 'caller_number',
      header: 'Origen',
      render: (call: ActiveCall) => (
        <span className="font-mono text-slate-900">{call.caller_number}</span>
      ),
    },
    {
      key: 'callee_number',
      header: 'Destino',
      render: (call: ActiveCall) => (
        <span className="font-mono text-slate-600">{call.callee_number}</span>
      ),
    },
    {
      key: 'duration_seconds',
      header: 'Duración',
      render: (call: ActiveCall) => formatDuration(call.duration_seconds ?? call.duration ?? 0),
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
      render: (call: ActiveCall) =>
        call.estimated_cost ? `S/${call.estimated_cost.toFixed(4)}` : '-',
      className: 'text-right',
    },
  ]

  const reservationColumns = [
    {
      key: 'call_uuid',
      header: 'Llamada',
      render: (r: Reservation) => (
        <span className="font-mono text-xs">{r.call_uuid.slice(0, 8)}...</span>
      ),
    },
    {
      key: 'account_id',
      header: 'Cuenta',
    },
    {
      key: 'destination_prefix',
      header: 'Destino',
    },
    {
      key: 'reserved_amount',
      header: 'Reservado',
      render: (r: Reservation) => (
        <span className="text-blue-600 font-medium">
          S/{r.reserved_amount.toFixed(4)}
        </span>
      ),
      className: 'text-right',
    },
    {
      key: 'consumed_amount',
      header: 'Consumido',
      render: (r: Reservation) => (
        <span className="text-green-600">S/{r.consumed_amount.toFixed(4)}</span>
      ),
      className: 'text-right',
    },
    {
      key: 'expires_at',
      header: 'Expira',
      render: (r: Reservation) =>
        formatDistanceToNow(new Date(r.expires_at), {
          addSuffix: true,
          locale: es,
        }),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">Panel de Control</h1>
          <p className="text-slate-500">Monitor en tiempo real del sistema de facturación</p>
        </div>
        <div className="flex items-center space-x-2 text-sm text-slate-500">
          <Activity className="w-4 h-4" />
          <span>Última actualización: {new Date().toLocaleTimeString('es-ES')}</span>
        </div>
      </div>

      {/* Stats Grid */}
      {statsLoading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {[...Array(4)].map((_, i) => (
            <div
              key={i}
              className="bg-white rounded-xl shadow-sm p-6 h-32 animate-pulse"
            />
          ))}
        </div>
      ) : stats ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <StatCard
            title="Cuentas Activas"
            value={stats.active_accounts}
            subtitle={`${stats.total_accounts} total`}
            icon={Users}
            color="blue"
          />
          <StatCard
            title="Llamadas Activas"
            value={stats.active_calls || activeCalls.length}
            icon={Phone}
            color="green"
          />
          <StatCard
            title="Saldo Total"
            value={`S/${stats.total_balance.toLocaleString('es-ES', {
              minimumFractionDigits: 2,
            })}`}
            icon={DollarSign}
            color="purple"
          />
          <StatCard
            title="Ingresos Hoy"
            value={`S/${(stats.revenue_today ?? 0).toFixed(2)}`}
            subtitle={`${stats.cdrs_today ?? stats.calls_today ?? 0} llamadas`}
            icon={TrendingUp}
            color="green"
          />
        </div>
      ) : (
        <div className="bg-yellow-50 border border-yellow-200 rounded-xl p-4">
          <p className="text-yellow-800">
            No se pudieron cargar las estadísticas. Verifica que el motor de facturación esté activo.
          </p>
        </div>
      )}

      {/* Secondary Stats */}
      {stats && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          <StatCard
            title="Reservas Activas"
            value={stats.active_reservations || reservations.length}
            subtitle={`S/${(stats.reserved_amount ?? 0).toFixed(2)} reservado`}
            icon={Clock}
            color="yellow"
          />
          <StatCard
            title="Tráfico Entrante (Hoy)"
            value={`${Number(trafficData?.inbound?.total_minutes ?? 0).toFixed(1)} min`}
            subtitle={`S/${Number(trafficData?.inbound?.total_revenue ?? 0).toFixed(2)} | ${trafficData?.inbound?.total_calls ?? 0} llamadas`}
            icon={Phone}
            color="green"
          />
          <StatCard
            title="Tráfico Saliente (Hoy)"
            value={`${Number(trafficData?.outbound?.total_minutes ?? 0).toFixed(1)} min`}
            subtitle={`S/${Number(trafficData?.outbound?.total_revenue ?? 0).toFixed(2)} | ${trafficData?.outbound?.total_calls ?? 0} llamadas`}
            icon={Phone}
            color="blue"
          />
        </div>
      )}

      {/* Gráficos Interactivos */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Gráfico de Líneas: Llamadas por Hora */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Llamadas por Hora</h3>
            <BarChart3 className="w-5 h-5 text-slate-400" />
          </div>
          <ResponsiveContainer width="100%" height={250}>
            <LineChart data={llamadasPorHora}>
              <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
              <XAxis
                dataKey="hora"
                tick={{ fontSize: 12 }}
                stroke="#64748b"
              />
              <YAxis tick={{ fontSize: 12 }} stroke="#64748b" />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#fff',
                  border: '1px solid #e2e8f0',
                  borderRadius: '8px',
                }}
              />
              <Line
                type="monotone"
                dataKey="llamadas"
                stroke="#2563eb"
                strokeWidth={2}
                dot={{ fill: '#2563eb', r: 4 }}
                activeDot={{ r: 6 }}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Gráfico de Barras: Ingresos por Día */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Ingresos por Día</h3>
            <TrendingUp className="w-5 h-5 text-slate-400" />
          </div>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={ingresosPorDia}>
              <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
              <XAxis
                dataKey="dia"
                tick={{ fontSize: 12 }}
                stroke="#64748b"
              />
              <YAxis tick={{ fontSize: 12 }} stroke="#64748b" />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#fff',
                  border: '1px solid #e2e8f0',
                  borderRadius: '8px',
                }}
                formatter={(value) => `S/${value}`}
              />
              <Bar
                dataKey="ingresos"
                fill="#10b981"
                radius={[8, 8, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Gráfico de Dona: Distribución por Tipo de Llamadas Activas */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Llamadas Activas por Tipo</h3>
            <Activity className="w-5 h-5 text-slate-400" />
          </div>
          {activeCalls.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <PieChart>
                <Pie
                  data={distribucionTipoLlamada}
                  cx="50%"
                  cy="50%"
                  innerRadius={60}
                  outerRadius={90}
                  paddingAngle={5}
                  dataKey="valor"
                  nameKey="nombre"
                  label
                >
                  {distribucionTipoLlamada.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
                <Tooltip />
                <Legend
                  verticalAlign="bottom"
                  height={36}
                  formatter={(value) => value}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex flex-col items-center justify-center h-[250px] text-slate-400">
              <Activity className="w-12 h-12 mb-2" />
              <p className="text-sm">No hay llamadas activas en este momento</p>
            </div>
          )}
        </div>

        {/* Gráfico de Área: Tendencia de Saldos */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Tendencia de Saldos (30 días)</h3>
            <DollarSign className="w-5 h-5 text-slate-400" />
          </div>
          <ResponsiveContainer width="100%" height={250}>
            <AreaChart data={tendenciaSaldos}>
              <defs>
                <linearGradient id="colorSaldo" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#8b5cf6" stopOpacity={0.8}/>
                  <stop offset="95%" stopColor="#8b5cf6" stopOpacity={0.1}/>
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
              <XAxis
                dataKey="dia"
                tick={{ fontSize: 12 }}
                stroke="#64748b"
              />
              <YAxis tick={{ fontSize: 12 }} stroke="#64748b" />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#fff',
                  border: '1px solid #e2e8f0',
                  borderRadius: '8px',
                }}
                formatter={(value) => `S/${Number(value).toFixed(2)}`}
              />
              <Area
                type="monotone"
                dataKey="saldo"
                stroke="#8b5cf6"
                strokeWidth={2}
                fillOpacity={1}
                fill="url(#colorSaldo)"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Gráfico de Dona: Llamadas por Tipo (últimos 30 días) */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Llamadas por Tipo (30 días)</h3>
            <BarChart3 className="w-5 h-5 text-slate-400" />
          </div>
          {llamadasPorTipo.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <PieChart>
                <Pie
                  data={llamadasPorTipo}
                  cx="50%"
                  cy="50%"
                  innerRadius={60}
                  outerRadius={90}
                  paddingAngle={5}
                  dataKey="valor"
                  nameKey="nombre"
                  label
                >
                  {llamadasPorTipo.map((_, index) => (
                    <Cell key={`cell-type-${index}`} fill={TIPO_COLORS[index % TIPO_COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip
                  formatter={(value, _, props) => [
                    `${value} llamadas (${props.payload.porcentaje}%)`,
                    props.payload.nombre
                  ]}
                />
                <Legend
                  verticalAlign="bottom"
                  height={36}
                  formatter={(value) => value}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex flex-col items-center justify-center h-[250px] text-slate-400">
              <BarChart3 className="w-12 h-12 mb-2" />
              <p className="text-sm">No hay datos de llamadas por tipo</p>
            </div>
          )}
        </div>

        {/* Gráfico de Barras Horizontales: Llamadas por Zona (Top 10) */}
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-slate-900">Llamadas por Zona (Top 10)</h3>
            <Activity className="w-5 h-5 text-slate-400" />
          </div>
          {llamadasPorZona.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={llamadasPorZona} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                <XAxis type="number" tick={{ fontSize: 12 }} stroke="#64748b" />
                <YAxis
                  dataKey="zona"
                  type="category"
                  width={120}
                  tick={{ fontSize: 11 }}
                  stroke="#64748b"
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#fff',
                    border: '1px solid #e2e8f0',
                    borderRadius: '8px',
                  }}
                  formatter={(value, _, props) => [
                    `${value} llamadas (${props.payload.porcentaje}%)`,
                    'Llamadas'
                  ]}
                />
                <Bar
                  dataKey="llamadas"
                  fill="#3b82f6"
                  radius={[0, 8, 8, 0]}
                >
                  {llamadasPorZona.map((_, index) => (
                    <Cell key={`cell-zone-${index}`} fill={ZONA_COLORS[index % ZONA_COLORS.length]} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex flex-col items-center justify-center h-[250px] text-slate-400">
              <Activity className="w-12 h-12 mb-2" />
              <p className="text-sm">No hay datos de llamadas por zona</p>
            </div>
          )}
        </div>
      </div>

      {/* Active Calls Table */}
      <div>
        <h2 className="text-lg font-semibold text-slate-900 mb-4">
          Llamadas Activas ({activeCalls.length})
        </h2>
        <DataTable
          columns={callColumns}
          data={activeCalls}
          emptyMessage="No hay llamadas activas en este momento"
          searchable={true}
          searchPlaceholder="Buscar por número origen o destino..."
        />
      </div>

      {/* Active Reservations Table */}
      <div>
        <h2 className="text-lg font-semibold text-slate-900 mb-4">
          Reservas de Saldo ({reservations.length})
        </h2>
        <DataTable
          columns={reservationColumns}
          data={reservations.slice(0, 10)}
          emptyMessage="No hay reservas activas"
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
