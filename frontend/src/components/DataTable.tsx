import { useState, useMemo } from 'react'
import { ChevronLeft, ChevronRight, ChevronUp, ChevronDown, Search } from 'lucide-react'
import { cn } from '../lib/utils'

interface Column<T> {
  key: keyof T | string
  header: string
  render?: (item: T) => React.ReactNode
  className?: string
  sortable?: boolean
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  loading?: boolean
  emptyMessage?: React.ReactNode
  pagination?: {
    page: number
    totalPages: number
    onPageChange: (page: number) => void
  }
  onRowClick?: (item: T) => void
  searchable?: boolean
  searchPlaceholder?: string
}

export default function DataTable<T extends { id?: number | string }>({
  columns,
  data,
  loading,
  emptyMessage = 'No hay datos disponibles',
  pagination,
  onRowClick,
  searchable = false,
  searchPlaceholder = 'Buscar...',
}: DataTableProps<T>) {
  const [sortColumn, setSortColumn] = useState<string | null>(null)
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc')
  const [searchTerm, setSearchTerm] = useState('')

  const handleSort = (columnKey: string) => {
    if (sortColumn === columnKey) {
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc')
    } else {
      setSortColumn(columnKey)
      setSortDirection('asc')
    }
  }

  const filteredAndSortedData = useMemo(() => {
    let result = [...data]

    // Filtrar por búsqueda
    if (searchable && searchTerm) {
      result = result.filter((item) => {
        return columns.some((col) => {
          const value = (item as Record<string, unknown>)[col.key as string]
          return String(value ?? '').toLowerCase().includes(searchTerm.toLowerCase())
        })
      })
    }

    // Ordenar
    if (sortColumn) {
      result.sort((a, b) => {
        const aValue = (a as Record<string, unknown>)[sortColumn]
        const bValue = (b as Record<string, unknown>)[sortColumn]

        if (aValue === null || aValue === undefined) return 1
        if (bValue === null || bValue === undefined) return -1

        if (typeof aValue === 'number' && typeof bValue === 'number') {
          return sortDirection === 'asc' ? aValue - bValue : bValue - aValue
        }

        const aStr = String(aValue)
        const bStr = String(bValue)
        return sortDirection === 'asc'
          ? aStr.localeCompare(bStr)
          : bStr.localeCompare(aStr)
      })
    }

    return result
  }, [data, searchTerm, sortColumn, sortDirection, columns, searchable])

  if (loading) {
    return (
      <div className="bg-white rounded-xl shadow-sm border border-slate-200 p-8">
        <div className="flex items-center justify-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
          <span className="ml-3 text-slate-500">Cargando...</span>
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {searchable && (
        <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-slate-400" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder={searchPlaceholder}
              className="w-full pl-10 pr-4 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
            />
          </div>
        </div>
      )}

      <div className="bg-white rounded-xl shadow-sm border border-slate-200 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="min-w-full divide-y divide-slate-200">
            <thead className="bg-slate-50">
              <tr>
                {columns.map((col) => (
                  <th
                    key={String(col.key)}
                    onClick={() => col.sortable !== false && handleSort(String(col.key))}
                    className={cn(
                      'px-6 py-3 text-left text-xs font-semibold text-slate-600 uppercase tracking-wider',
                      col.sortable !== false && 'cursor-pointer hover:bg-slate-100 transition-colors',
                      col.className
                    )}
                  >
                    <div className="flex items-center space-x-1">
                      <span>{col.header}</span>
                      {col.sortable !== false && sortColumn === String(col.key) && (
                        sortDirection === 'asc'
                          ? <ChevronUp className="w-4 h-4" />
                          : <ChevronDown className="w-4 h-4" />
                      )}
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-slate-200">
              {filteredAndSortedData.length === 0 ? (
                <tr>
                  <td
                    colSpan={columns.length}
                    className="px-6 py-12 text-center text-slate-500"
                  >
                    {emptyMessage}
                  </td>
                </tr>
              ) : (
                filteredAndSortedData.map((item, index) => (
                  <tr
                    key={item.id ?? index}
                    onClick={() => onRowClick?.(item)}
                    className={cn(
                      'hover:bg-slate-50 transition-colors',
                      onRowClick && 'cursor-pointer'
                    )}
                  >
                    {columns.map((col) => (
                      <td
                        key={String(col.key)}
                        className={cn(
                          'px-6 py-4 whitespace-nowrap text-sm',
                          col.className
                        )}
                      >
                        {col.render
                          ? col.render(item)
                          : String((item as Record<string, unknown>)[col.key as string] ?? '')}
                      </td>
                    ))}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {pagination && pagination.totalPages > 1 && (
          <div className="px-6 py-4 border-t border-slate-200 flex items-center justify-between bg-slate-50">
            <span className="text-sm text-slate-500">
              Página {pagination.page} de {pagination.totalPages}
            </span>
            <div className="flex space-x-2">
              <button
                onClick={() => pagination.onPageChange(pagination.page - 1)}
                disabled={pagination.page <= 1}
                className="p-2 rounded-lg border border-slate-300 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-white transition-colors"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
              <button
                onClick={() => pagination.onPageChange(pagination.page + 1)}
                disabled={pagination.page >= pagination.totalPages}
                className="p-2 rounded-lg border border-slate-300 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-white transition-colors"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
