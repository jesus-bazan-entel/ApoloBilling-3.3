import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchCDRs, exportCDRs } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { FileText, Download, Search, Filter, X } from 'lucide-react'
import type { CDR, CDRFilters } from '../types'

export default function CDRPage() {
  const [page, setPage] = useState(1)
  const [filters, setFilters] = useState<CDRFilters>({})
  const [showFilters, setShowFilters] = useState(false)

  const { data, isLoading } = useQuery({
    queryKey: ['cdrs', page, filters],
    queryFn: () => fetchCDRs(filters, page, 50),
  })

  const handleExport = async () => {
    try {
      const blob = await exportCDRs(filters)
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `cdr_export_${new Date().toISOString().split('T')[0]}.csv`
      a.click()
      window.URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Export failed:', error)
    }
  }

  const columns = [
    {
      key: 'start_time',
      header: 'Fecha/Hora',
      render: (cdr: CDR) => (
        <div>
          <div className="text-slate-900">
            {new Date(cdr.start_time).toLocaleDateString('es-PE')}
          </div>
          <div className="text-xs text-slate-500">
            {new Date(cdr.start_time).toLocaleTimeString('es-PE')}
          </div>
        </div>
      ),
    },
    {
      key: 'caller',
      header: 'Origen',
      render: (cdr: CDR) => (
        <span className="font-mono text-slate-900">{cdr.caller || cdr.caller_number || '-'}</span>
      ),
    },
    {
      key: 'callee',
      header: 'Destino',
      render: (cdr: CDR) => (
        <div>
          <span className="font-mono text-slate-700">{cdr.callee || cdr.callee_number || '-'}</span>
          {cdr.destination && (
            <span className="text-xs text-slate-500 block">{cdr.destination}</span>
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
      header: 'Facturable',
      render: (cdr: CDR) => formatDuration(cdr.billsec ?? 0),
      className: 'text-right',
    },
    {
      key: 'total_cost',
      header: 'Costo',
      render: (cdr: CDR) => {
        const cost = parseFloat(String(cdr.total_cost ?? cdr.cost ?? 0)) || 0
        return (
          <span className="font-mono font-medium text-slate-900">
            S/{cost.toFixed(4)}
          </span>
        )
      },
      className: 'text-right',
    },
    {
      key: 'hangup_cause',
      header: 'Estado',
      render: (cdr: CDR) => (
        <Badge
          variant={
            cdr.hangup_cause === 'NORMAL_CLEARING'
              ? 'success'
              : cdr.hangup_cause === 'NO_ANSWER'
              ? 'warning'
              : 'error'
          }
        >
          {cdr.hangup_cause === 'NORMAL_CLEARING'
            ? 'Completada'
            : cdr.hangup_cause === 'NO_ANSWER'
            ? 'Sin respuesta'
            : cdr.hangup_cause || 'Desconocido'}
        </Badge>
      ),
    },
  ]

  const clearFilters = () => {
    setFilters({})
    setPage(1)
  }

  const hasActiveFilters = Object.values(filters).some(
    (v) => v !== undefined && v !== ''
  )

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">
            Registros de Llamadas (CDR)
          </h1>
          <p className="text-slate-500">
            Historial detallado de todas las llamadas procesadas
          </p>
        </div>
        <div className="flex items-center space-x-3">
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center px-4 py-2 rounded-lg border transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-blue-50 border-blue-300 text-blue-700'
                : 'border-slate-300 text-slate-700 hover:bg-slate-50'
            }`}
          >
            <Filter className="w-4 h-4 mr-2" />
            Filtros
            {hasActiveFilters && (
              <span className="ml-2 bg-blue-600 text-white text-xs px-2 py-0.5 rounded-full">
                {Object.values(filters).filter((v) => v !== undefined && v !== '').length}
              </span>
            )}
          </button>
          <button
            onClick={handleExport}
            className="flex items-center px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors"
          >
            <Download className="w-4 h-4 mr-2" />
            Exportar CSV
          </button>
        </div>
      </div>

      {/* Filters Panel */}
      {showFilters && (
        <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="font-semibold text-slate-900">Filtros de Búsqueda</h3>
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
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Fecha Inicio
              </label>
              <input
                type="date"
                value={filters.start_date || ''}
                onChange={(e) =>
                  setFilters({ ...filters, start_date: e.target.value })
                }
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Fecha Fin
              </label>
              <input
                type="date"
                value={filters.end_date || ''}
                onChange={(e) =>
                  setFilters({ ...filters, end_date: e.target.value })
                }
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Número Origen
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-slate-400" />
                <input
                  type="text"
                  value={filters.caller || ''}
                  onChange={(e) =>
                    setFilters({ ...filters, caller: e.target.value })
                  }
                  placeholder="Ej: 1001"
                  className="w-full pl-10 pr-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Número Destino
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-slate-400" />
                <input
                  type="text"
                  value={filters.callee || ''}
                  onChange={(e) =>
                    setFilters({ ...filters, callee: e.target.value })
                  }
                  placeholder="Ej: 51987654321"
                  className="w-full pl-10 pr-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Dirección
              </label>
              <select
                value={filters.direction || ''}
                onChange={(e) =>
                  setFilters({ ...filters, direction: e.target.value || undefined })
                }
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="">Todas</option>
                <option value="outbound">Saliente</option>
                <option value="inbound">Entrante</option>
                <option value="internal">Interna</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Costo Mínimo
              </label>
              <input
                type="number"
                step="0.01"
                value={filters.min_cost || ''}
                onChange={(e) =>
                  setFilters({
                    ...filters,
                    min_cost: e.target.value ? parseFloat(e.target.value) : undefined,
                  })
                }
                placeholder="S/0.00"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Costo Máximo
              </label>
              <input
                type="number"
                step="0.01"
                value={filters.max_cost || ''}
                onChange={(e) =>
                  setFilters({
                    ...filters,
                    max_cost: e.target.value ? parseFloat(e.target.value) : undefined,
                  })
                }
                placeholder="S/100.00"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
          </div>
        </div>
      )}

      {/* Summary */}
      {data && (
        <div className="flex items-center justify-between bg-white rounded-lg shadow-sm border border-slate-200 p-4">
          <div className="flex items-center">
            <FileText className="w-5 h-5 text-slate-400 mr-2" />
            <span className="text-slate-600">
              Mostrando {(data.data ?? data.items ?? []).length} de {data.total} registros
            </span>
          </div>
          <div className="text-sm text-slate-500">
            Total facturado: S/
            {(data.data ?? data.items ?? []).reduce((sum, cdr) => sum + (parseFloat(String(cdr.total_cost ?? cdr.cost ?? 0)) || 0), 0).toFixed(2)}
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
        searchPlaceholder="Búsqueda rápida en resultados..."
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
