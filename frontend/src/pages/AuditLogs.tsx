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
    return 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border-[var(--color-border-primary)]'
  }

  const columns = [
    { key: 'id', header: 'ID', className: 'w-16' },
    {
      key: 'created_at',
      header: 'Fecha/Hora',
      render: (log: AuditLog) => (
        <div className="text-sm">
          <div className="font-medium text-[var(--color-text-primary)]">
            {new Date(log.created_at).toLocaleDateString('es-PE')}
          </div>
          <div className="text-[var(--color-text-tertiary)]">{new Date(log.created_at).toLocaleTimeString('es-PE')}</div>
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
        <span className="px-2 py-1 bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] rounded text-xs font-mono">
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
        <span className="font-mono text-xs text-[var(--color-text-secondary)]">{log.ip_address || '-'}</span>
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
          <h1 className="text-2xl font-bold text-[var(--color-text-primary)]">Auditoría de Acciones</h1>
          <p className="text-sm text-[var(--color-text-secondary)]">
            Registro completo de acciones en el sistema (solo superadmin)
          </p>
        </div>
        <button
          onClick={() => setShowFilters(!showFilters)}
          className={`inline-flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
            showFilters || hasActiveFilters
              ? 'bg-blue-600 text-white hover:bg-blue-700'
              : 'bg-[var(--color-bg-card)] border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]'
          }`}
        >
          <Filter className="w-4 h-4" />
          Filtros {hasActiveFilters && `(${Object.values(filters).filter(Boolean).length - 2})`}
        </button>
      </div>

      {/* Stats */}
      {stats && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
            <div className="flex items-center gap-3">
              <FileSearch className="w-8 h-8 text-blue-600" />
              <div>
                <p className="text-sm text-[var(--color-text-secondary)]">Total Logs</p>
                <p className="text-2xl font-bold text-[var(--color-text-primary)]">{stats.total_logs?.toLocaleString()}</p>
              </div>
            </div>
          </div>
          <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
            <div className="flex items-center gap-3">
              <TrendingUp className="w-8 h-8 text-green-600" />
              <div>
                <p className="text-sm text-[var(--color-text-secondary)]">Últimas 24h</p>
                <p className="text-2xl font-bold text-[var(--color-text-primary)]">{stats.logs_last_24h?.toLocaleString()}</p>
              </div>
            </div>
          </div>
          <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)] col-span-2">
            <div className="flex items-center gap-2 mb-2">
              <BarChart3 className="w-5 h-5 text-[var(--color-text-secondary)]" />
              <p className="text-sm font-medium text-[var(--color-text-secondary)]">Acciones Más Frecuentes (30 días)</p>
            </div>
            <div className="flex flex-wrap gap-2">
              {stats.top_actions?.slice(0, 5).map((item: any) => (
                <span
                  key={item.action}
                  className="px-2 py-1 bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)] rounded text-xs border border-[var(--color-border-primary)]"
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
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)] p-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Usuario</label>
              <input
                type="text"
                value={filters.username}
                onChange={(e) => handleFilterChange('username', e.target.value)}
                placeholder="Filtrar por usuario..."
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Acción</label>
              <input
                type="text"
                value={filters.action}
                onChange={(e) => handleFilterChange('action', e.target.value)}
                placeholder="ej: create_account"
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Tipo de Entidad</label>
              <input
                type="text"
                value={filters.entity_type}
                onChange={(e) => handleFilterChange('entity_type', e.target.value)}
                placeholder="ej: account, user"
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">ID Entidad</label>
              <input
                type="text"
                value={filters.entity_id}
                onChange={(e) => handleFilterChange('entity_id', e.target.value)}
                placeholder="ID específico..."
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Fecha Inicio</label>
              <input
                type="datetime-local"
                value={filters.start_date}
                onChange={(e) => handleFilterChange('start_date', e.target.value)}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Fecha Fin</label>
              <input
                type="datetime-local"
                value={filters.end_date}
                onChange={(e) => handleFilterChange('end_date', e.target.value)}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              />
            </div>
          </div>
          {hasActiveFilters && (
            <div className="mt-4 flex justify-end">
              <button
                onClick={clearFilters}
                className="inline-flex items-center gap-2 px-3 py-1.5 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-secondary)] rounded-lg transition-colors"
              >
                <X className="w-4 h-4" />
                Limpiar Filtros
              </button>
            </div>
          )}
        </div>
      )}

      {/* Table */}
      <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)]">
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
          <div className="bg-[var(--color-bg-card)] rounded-lg shadow-xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
            <div className="sticky top-0 bg-[var(--color-bg-card)] border-b border-[var(--color-border-primary)] px-6 py-4 flex items-center justify-between">
              <h3 className="text-lg font-semibold text-[var(--color-text-primary)]">Detalles del Log</h3>
              <button
                onClick={() => setSelectedLog(null)}
                className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">ID</p>
                  <p className="text-sm text-[var(--color-text-primary)]">{selectedLog.id}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">Usuario</p>
                  <p className="text-sm text-[var(--color-text-primary)]">{selectedLog.username}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">Acción</p>
                  <p className="text-sm">
                    <span className={`px-2 py-1 rounded-full text-xs font-medium border ${getActionColor(selectedLog.action)}`}>
                      {selectedLog.action}
                    </span>
                  </p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">Tipo de Entidad</p>
                  <p className="text-sm text-[var(--color-text-primary)]">{selectedLog.entity_type}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">ID Entidad</p>
                  <p className="text-sm text-[var(--color-text-primary)]">{selectedLog.entity_id || '-'}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">Fecha/Hora</p>
                  <p className="text-sm text-[var(--color-text-primary)]">
                    {new Date(selectedLog.created_at).toLocaleString('es-PE')}
                  </p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">IP Address</p>
                  <p className="text-sm text-[var(--color-text-primary)] font-mono">{selectedLog.ip_address || '-'}</p>
                </div>
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)]">User ID</p>
                  <p className="text-sm text-[var(--color-text-primary)]">{selectedLog.user_id || '-'}</p>
                </div>
              </div>

              {selectedLog.details && (
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)] mb-2">Detalles Adicionales</p>
                  <pre className="bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg p-4 text-xs overflow-x-auto">
                    {JSON.stringify(selectedLog.details, null, 2)}
                  </pre>
                </div>
              )}

              {selectedLog.user_agent && (
                <div>
                  <p className="text-sm font-medium text-[var(--color-text-tertiary)] mb-1">User Agent</p>
                  <p className="text-xs text-[var(--color-text-secondary)] font-mono break-all">{selectedLog.user_agent}</p>
                </div>
              )}
            </div>

            <div className="sticky bottom-0 bg-[var(--color-bg-secondary)] border-t border-[var(--color-border-primary)] px-6 py-4">
              <button
                onClick={() => setSelectedLog(null)}
                className="w-full px-4 py-2 bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] rounded-lg hover:bg-[var(--color-bg-hover)] transition-colors"
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
