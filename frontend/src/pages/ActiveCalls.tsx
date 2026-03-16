import { useState, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchActiveCalls } from '../api/client'
import { useWebSocket } from '../hooks/useWebSocket'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { Phone, PhoneOff, Wifi, WifiOff } from 'lucide-react'
import type { ActiveCall } from '../types'

export default function ActiveCalls() {
  const { isConnected } = useWebSocket()
  const [, setTick] = useState(0)

  // Update every second to refresh durations
  useEffect(() => {
    const interval = setInterval(() => {
      setTick(t => t + 1)
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  const { data: apiCalls = [], isLoading } = useQuery({
    queryKey: ['activeCalls'],
    queryFn: fetchActiveCalls,
    // Always refetch every 3 seconds to catch call removals
    // WebSocket may not notify when calls end
    refetchInterval: 3000,
  })

  // Siempre usar datos de la API (se refresca cada 3 segundos)
  // El WebSocket se usa solo para indicador de conexión
  // Agregar campo id para compatibilidad con DataTable
  const calls = apiCalls.map(call => ({
    ...call,
    id: call.call_uuid
  }))

  const columns = [
    {
      key: 'call_uuid',
      header: 'UUID',
      render: (call: ActiveCall) => (
        <span className="font-mono text-xs text-slate-500">
          {call.call_uuid.slice(0, 8)}...
        </span>
      ),
    },
    {
      key: 'caller_number',
      header: 'Origen',
      render: (call: ActiveCall) => (
        <div>
          <span className="font-mono font-medium text-slate-900">
            {call.caller_number}
          </span>
          {call.account_id && (
            <span className="text-xs text-slate-500 ml-2">
              (Cuenta: {call.account_id})
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'callee_number',
      header: 'Destino',
      render: (call: ActiveCall) => (
        <div>
          <span className="font-mono text-slate-700">{call.callee_number}</span>
          {call.zone_name && (
            <span className="text-xs text-slate-500 block">{call.zone_name}</span>
          )}
        </div>
      ),
    },
    {
      key: 'direction',
      header: 'Dirección',
      render: (call: ActiveCall) => (
        <Badge
          variant={
            call.direction === 'outbound'
              ? 'info'
              : call.direction === 'inbound'
              ? 'success'
              : 'default'
          }
        >
          {call.direction === 'outbound'
            ? 'Saliente'
            : call.direction === 'inbound'
            ? 'Entrante'
            : 'Interna'}
        </Badge>
      ),
    },
    {
      key: 'start_time',
      header: 'Inicio',
      render: (call: ActiveCall) =>
        new Date(call.start_time).toLocaleTimeString('es-ES'),
    },
    {
      key: 'duration_seconds',
      header: 'Duración',
      render: (call: ActiveCall) => {
        // Calculate real-time duration from start_time
        const startTime = new Date(call.start_time).getTime()
        const now = Date.now()
        const durationSec = Math.max(0, Math.floor((now - startTime) / 1000))
        return (
          <span className="font-mono tabular-nums">{formatDuration(durationSec)}</span>
        )
      },
      className: 'text-right',
    },
    {
      key: 'status',
      header: 'Estado',
      render: (call: ActiveCall) => (
        <div className="flex items-center">
          {call.status === 'answered' ? (
            <Phone className="w-4 h-4 text-green-500 mr-2 animate-pulse" />
          ) : (
            <Phone className="w-4 h-4 text-yellow-500 mr-2" />
          )}
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
              ? 'Conectada'
              : call.status === 'ringing'
              ? 'Timbrando'
              : 'Marcando'}
          </Badge>
        </div>
      ),
    },
    {
      key: 'estimated_cost',
      header: 'Costo Est.',
      render: (call: ActiveCall) => {
        // Calculate real-time cost based on duration and rate
        const startTime = new Date(call.start_time).getTime()
        const now = Date.now()
        const durationSec = Math.max(0, Math.floor((now - startTime) / 1000))
        const rate = call.rate_per_minute || 0
        const cost = (durationSec / 60) * rate
        return (
          <span className="font-mono tabular-nums text-slate-700">
            {rate > 0 ? `S/${cost.toFixed(4)}` : '-'}
          </span>
        )
      },
      className: 'text-right',
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">Llamadas Activas</h1>
          <p className="text-slate-500">
            Monitoreo en tiempo real de llamadas en curso
          </p>
        </div>
        <div className="flex items-center space-x-4">
          <div className="flex items-center text-sm">
            {isConnected ? (
              <>
                <Wifi className="w-4 h-4 text-green-500 mr-2" />
                <span className="text-green-600">WebSocket Conectado</span>
              </>
            ) : (
              <>
                <WifiOff className="w-4 h-4 text-red-500 mr-2" />
                <span className="text-red-600">WebSocket Desconectado</span>
              </>
            )}
          </div>
          <div className="flex items-center bg-blue-100 text-blue-800 px-4 py-2 rounded-lg">
            <Phone className="w-5 h-5 mr-2" />
            <span className="font-bold text-lg">{calls.length}</span>
            <span className="ml-1">activas</span>
          </div>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <p className="text-sm text-slate-500">Salientes</p>
          <p className="text-2xl font-bold text-blue-600">
            {calls.filter((c) => c.direction === 'outbound').length}
          </p>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <p className="text-sm text-slate-500">Entrantes</p>
          <p className="text-2xl font-bold text-green-600">
            {calls.filter((c) => c.direction === 'inbound').length}
          </p>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <p className="text-sm text-slate-500">Internas</p>
          <p className="text-2xl font-bold text-slate-600">
            {calls.filter((c) => c.direction === 'internal').length}
          </p>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <p className="text-sm text-slate-500">Costo Total Est.</p>
          <p className="text-2xl font-bold tabular-nums text-purple-600">
            S/
            {calls
              .reduce((sum, c) => {
                const startTime = new Date(c.start_time).getTime()
                // eslint-disable-next-line react-hooks/purity
                const durationSec = Math.max(0, Math.floor((Date.now() - startTime) / 1000))
                const rate = c.rate_per_minute || 0
                return sum + (durationSec / 60) * rate
              }, 0)
              .toFixed(2)}
          </p>
        </div>
      </div>

      {/* Calls Table */}
      <DataTable
        columns={columns}
        data={calls}
        loading={isLoading}
        emptyMessage={
          <div className="flex flex-col items-center py-8">
            <PhoneOff className="w-12 h-12 text-slate-300 mb-4" />
            <p className="text-slate-500">No hay llamadas activas en este momento</p>
          </div>
        }
        searchable={true}
        searchPlaceholder="Buscar por número origen, destino o UUID..."
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
