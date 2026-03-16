import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchAuditLogs, fetchAuditStats, type AuditLogFilters } from '../api/client'
import type { AuditLog } from '../types'
import DataTable from '../components/DataTable'
import { FileSearch, Filter, X, BarChart3, TrendingUp } from 'lucide-react'

export default function AuditLogs() {
  const [page, setPage] = useState(1)
  const [perPage] = useState(50)
  const [showFilters, setShowFilters] = useState(false)
  const [selectedLog, setSelectedLog] = useState<AuditLog | null>(null)

  const [filters, setFilters] = useState<AuditLogFilters>({
    username: '',
    action: '',
    entity_type: '',
    entity_id: '',
    start_date: '',
    end_date: '',
    page,
    per_page: perPage,
  })

  const { data: logs, isLoading } = useQuery({
    queryKey: ['auditLogs', filters],
    queryFn: () => fetchAuditLogs({ ...filters, page, per_page: perPage }),
  })

  const { data: stats } = useQuery({
    queryKey: ['auditStats'],
    queryFn: fetchAuditStats,
    refetchInterval: 60000, // Refresh every minute
  })

  const handleFilterChange = (key: string, value: string) => {
    setFilters((prev) => ({ ...prev, [key]: value }))
    setPage(1) // Reset to first page when filtering
  }

  const clearFilters = () => {
    setFilters({
      username: '',
      action: '',
      entity_type: '',
      entity_id: '',
      start_date: '',
      end_date: '',
      page: 1,
      per_page: perPage,
    })
    setPage(1)
  }

  const getActionColor = (action: string) => {
    if (action.includes('create')) return 'bg-green-100 text-green-800 border-green-200'
    if (action.includes('update')) return 'bg-blue-100 text-blue-800 border-blue-200'
    if (action.includes('delete')) return 'bg-red-100 text-red-800 border-red-200'
    if (action.includes('login')) return 'bg-purple-100 text-purple-800 border-purple-200'
    return 'bg-gray-100 text-gray-800 border-gray-200'
  }

  const columns = [
    { key: 'id', header: 'ID', className: 'w-16' },
    {
      key: 'created_at',
      header: 'Fecha/Hora',
      render: (log: AuditLog) => (
        <div className="text-sm">
          <div className="font-medium text-slate-900">
            {new Date(log.created_at).toLocaleDateString('es-PE')}
          </div>
          <div className="text-slate-500">{new Date(log.created_at).toLocaleTimeString('es-PE')}</div>
        </div>
      ),
      className: 'w-36',
    },
    { key: 'username', header: 'Usuario', className: 'w-32' },
    {
      key: 'action',
      header: 'Acción',
      render: (log: AuditLog) => (
        <span
          className={`px-2 py-1 rounded-full text-xs font-medium border ${getActionColor(log.action)}`}
        >
          {log.action}
        </span>
      ),
      className: 'w-40',
    },
    {
      key: 'entity_type',
      header: 'Tipo',
      render: (log: AuditLog) => (
        <span className="px-2 py-1 bg-slate-100 text-slate-800 rounded text-xs font-mono">
          {log.entity_type}
        </span>
      ),
      className: 'w-28',
    },
    { key: 'entity_id', header: 'ID Entidad', className: 'w-24' },
    {
      key: 'ip_address',
      header: 'IP',
      render: (log: AuditLog) => (
        <span className="font-mono text-xs text-slate-600">{log.ip_address || '-'}</span>
      ),
      className: 'w-32',
    },
    {
      key: 'details',
      header: 'Detalles',
      render: (log: AuditLog) => (
        <button
          onClick={() => setSelectedLog(log)}
          className="text-blue-600 hover:text-blue-800 text-sm underline"
        >
          Ver detalles
        </button>
      ),
      className: 'w-24',
    },
  ]

  const hasActiveFilters =
    filters.username ||
    filters.action ||
    filters.entity_type ||
    filters.entity_id ||
    filters.start_date ||
    filters.end_date

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">Auditoría de Acciones</h1>
          <p className="text-sm text-slate-600">
            Registro completo de acciones en el sistema (solo superadmin)
          </p>
        </div>
        <button
          onClick={() => setShowFilters(!showFilters)}
          className={`inline-flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
            showFilters || hasActiveFilters
              ? 'bg-blue-600 text-white hover:bg-blue-700'
              : 'bg-white border border-slate-300 text-slate-700 hover:bg-slate-50'
          }`}
        >
          <Filter className="w-4 h-4" />
          Filtros {hasActiveFilters && `(${Object.values(filters).filter(Boolean).length - 2})`}
        </button>
      </div>

      {/* Stats */}
      {stats && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
            <div className="flex items-center gap-3">
              <FileSearch className="w-8 h-8 text-blue-600" />
              <div>
                <p className="text-sm text-slate-600">Total Logs</p>
                <p className="text-2xl font-bold text-slate-900">{stats.total_logs?.toLocaleString()}</p>
              </div>
            </div>
          </div>
          <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
            <div className="flex items-center gap-3">
              <TrendingUp className="w-8 h-8 text-green-600" />
              <div>
                <p className="text-sm text-slate-600">Últimas 24h</p>
                <p className="text-2xl font-bold text-slate-900">{stats.logs_last_24h?.toLocaleString()}</p>
              </div>
            </div>
          </div>
          <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200 col-span-2">
            <div className="flex items-center gap-2 mb-2">
              <BarChart3 className="w-5 h-5 text-slate-600" />
              <p className="text-sm font-medium text-slate-700">Acciones Más Frecuentes (30 días)</p>
            </div>
            <div className="flex flex-wrap gap-2">
              {stats.top_actions?.slice(0, 5).map((item: any) => (
                <span
                  key={item.action}
                  className="px-2 py-1 bg-slate-100 text-slate-700 rounded text-xs border border-slate-200"
                >
                  {item.action}: <strong>{item.count}</strong>
                </span>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Filters Panel */}
      {showFilters && (
        <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">Usuario</label>
              <input
                type="text"
                value={filters.username}
                onChange={(e) => handleFilterChange('username', e.target.value)}
                placeholder="Filtrar por usuario..."
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">Acción</label>
              <input
                type="text"
                value={filters.action}
                onChange={(e) => handleFilterChange('action', e.target.value)}
                placeholder="ej: create_account"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">Tipo de Entidad</label>
              <input
                type="text"
                value={filters.entity_type}
                onChange={(e) => handleFilterChange('entity_type', e.target.value)}
                placeholder="ej: account, user"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">ID Entidad</label>
              <input
                type="text"
                value={filters.entity_id}
                onChange={(e) => handleFilterChange('entity_id', e.target.value)}
                placeholder="ID específico..."
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">Fecha Inicio</label>
              <input
                type="datetime-local"
                value={filters.start_date}
                onChange={(e) => handleFilterChange('start_date', e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">Fecha Fin</label>
              <input
                type="datetime-local"
                value={filters.end_date}
                onChange={(e) => handleFilterChange('end_date', e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
          </div>
          {hasActiveFilters && (
            <div className="mt-4 flex justify-end">
              <button
                onClick={clearFilters}
                className="inline-flex items-center gap-2 px-3 py-1.5 text-sm text-slate-600 hover:text-slate-900 hover:bg-slate-100 rounded-lg transition-colors"
              >
                <X className="w-4 h-4" />
                Limpiar Filtros
              </button>
            </div>
          )}
        </div>
      )}

      {/* Table */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200">
        <DataTable
          data={logs?.logs || []}
          columns={columns}
          loading={isLoading}
          pagination={{
            page: page,
            totalPages: logs?.total_pages || 1,
            onPageChange: setPage,
          }}
        />
      </div>

      {/* Details Modal */}
      {selectedLog && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
          <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
            <div className="sticky top-0 bg-white border-b border-slate-200 px-6 py-4 flex items-center justify-between">
              <h3 className="text-lg font-semibold text-slate-900">Detalles del Log</h3>
              <button
                onClick={() => setSelectedLog(null)}
                className="text-slate-400 hover:text-slate-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm font-medium text-slate-500">ID</p>
                  <p className="text-sm text-slate-900">{selectedLog.id}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">Usuario</p>
                  <p className="text-sm text-slate-900">{selectedLog.username}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">Acción</p>
                  <p className="text-sm">
                    <span className={`px-2 py-1 rounded-full text-xs font-medium border ${getActionColor(selectedLog.action)}`}>
                      {selectedLog.action}
                    </span>
                  </p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">Tipo de Entidad</p>
                  <p className="text-sm text-slate-900">{selectedLog.entity_type}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">ID Entidad</p>
                  <p className="text-sm text-slate-900">{selectedLog.entity_id || '-'}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">Fecha/Hora</p>
                  <p className="text-sm text-slate-900">
                    {new Date(selectedLog.created_at).toLocaleString('es-PE')}
                  </p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">IP Address</p>
                  <p className="text-sm text-slate-900 font-mono">{selectedLog.ip_address || '-'}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-slate-500">User ID</p>
                  <p className="text-sm text-slate-900">{selectedLog.user_id || '-'}</p>
                </div>
              </div>

              {selectedLog.details && (
                <div>
                  <p className="text-sm font-medium text-slate-500 mb-2">Detalles Adicionales</p>
                  <pre className="bg-slate-50 border border-slate-200 rounded-lg p-4 text-xs overflow-x-auto">
                    {JSON.stringify(selectedLog.details, null, 2)}
                  </pre>
                </div>
              )}

              {selectedLog.user_agent && (
                <div>
                  <p className="text-sm font-medium text-slate-500 mb-1">User Agent</p>
                  <p className="text-xs text-slate-600 font-mono break-all">{selectedLog.user_agent}</p>
                </div>
              )}
            </div>

            <div className="sticky bottom-0 bg-slate-50 border-t border-slate-200 px-6 py-4">
              <button
                onClick={() => setSelectedLog(null)}
                className="w-full px-4 py-2 bg-slate-600 text-white rounded-lg hover:bg-slate-700 transition-colors"
              >
                Cerrar
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
