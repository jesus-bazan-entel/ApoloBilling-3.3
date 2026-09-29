import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  getMinuteBucketReport,
  exportMinuteBucketReport,
  getMinuteBucketHistory,
  fetchAccounts,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import CreditUtilizationBar from '../components/CreditUtilizationBar'
import { Timer, Download, X, PhoneOff, PlusCircle } from 'lucide-react'
import type { MinuteBucketReportRow, CycleHistoryEntry } from '../types'

function currentMonth(): string {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`
}

function monthLabel(month: string): string {
  const [y, m] = month.split('-').map(Number)
  return new Date(y, m - 1, 1).toLocaleDateString('es-PE', { month: 'long', year: 'numeric' })
}

export default function MinuteBucketsPage() {
  const [month, setMonth] = useState(currentMonth())
  const [bucketTypeFilter, setBucketTypeFilter] = useState<string>('')
  const [accountIdFilter, setAccountIdFilter] = useState<number | undefined>(undefined)
  const [detailRow, setDetailRow] = useState<MinuteBucketReportRow | null>(null)

  const isCurrentMonth = month === currentMonth()

  const { data: rows = [], isLoading } = useQuery({
    queryKey: ['minute-bucket-report', month, bucketTypeFilter, accountIdFilter],
    queryFn: () =>
      getMinuteBucketReport({
        month,
        bucket_type: bucketTypeFilter || undefined,
        account_id: accountIdFilter,
      }),
    // Live status view (current month) refreshes on its own; historical
    // months are static once the cycle closes.
    refetchInterval: isCurrentMonth ? 15000 : false,
  })

  const { data: accounts = [] } = useQuery({
    queryKey: ['accounts'],
    queryFn: fetchAccounts,
  })

  const handleExport = async () => {
    try {
      const blob = await exportMinuteBucketReport({
        month,
        bucket_type: bucketTypeFilter || undefined,
        account_id: accountIdFilter,
      })
      const url = window.URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `bolsas_minutos_${month}.csv`
      a.click()
      window.URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Export failed:', error)
    }
  }

  // DataTable requires rows to carry an `id` — the report's natural key is
  // account_id, so alias it rather than changing the shared API type.
  const rowsWithId = rows.map((r) => ({ ...r, id: r.account_id }))

  const blockedCount = rows.filter((r) => r.is_blocked).length
  const totalOverageCost = rows.reduce((sum, r) => sum + Number(r.overage_cost || 0), 0)
  const nearLimitCount = rows.filter((r) => !r.is_blocked && r.utilization_percent >= 80).length

  const columns = [
    {
      key: 'account_number',
      header: 'Cuenta',
      render: (r: MinuteBucketReportRow) => (
        <div>
          <div className="font-mono font-medium text-[var(--color-text-primary)]">
            {r.account_number}
          </div>
          {r.account_name && (
            <div className="text-xs text-[var(--color-text-tertiary)]">{r.account_name}</div>
          )}
        </div>
      ),
    },
    {
      key: 'bucket_type',
      header: 'Plan',
      render: (r: MinuteBucketReportRow) => (
        <Badge variant={r.bucket_type === 'control' ? 'warning' : 'info'}>
          {r.bucket_type === 'control' ? 'Control' : 'Abierto'}
        </Badge>
      ),
    },
    {
      key: 'included_minutes',
      header: 'Bolsa',
      render: (r: MinuteBucketReportRow) => (
        <div className="text-sm">
          <span className="font-mono">{r.included_minutes} min</span>
          {r.is_prorated && (
            <span className="ml-1 text-xs text-[var(--color-text-tertiary)]">(prorrateado)</span>
          )}
          {r.additional_minutes > 0 && (
            <div className="text-xs text-[var(--color-text-tertiary)]">
              +{r.additional_minutes} min ({r.additions_count} adición
              {r.additions_count === 1 ? '' : 'es'})
            </div>
          )}
        </div>
      ),
    },
    {
      key: 'consumed_minutes',
      header: 'Consumo',
      render: (r: MinuteBucketReportRow) => (
        <div className="min-w-[160px]">
          <div className="text-sm font-mono">
            {Number(r.consumed_minutes).toFixed(0)} / {r.included_minutes + r.additional_minutes} min
          </div>
          <CreditUtilizationBar utilizationPercent={r.utilization_percent} showLabel={false} />
        </div>
      ),
    },
    {
      key: 'overage_minutes',
      header: 'Excedente',
      render: (r: MinuteBucketReportRow) =>
        Number(r.overage_minutes) > 0 ? (
          <div className="text-right">
            <div className="font-mono text-amber-600">{Number(r.overage_minutes).toFixed(1)} min</div>
            <div className="text-xs text-[var(--color-text-tertiary)]">
              ${Number(r.overage_cost).toFixed(2)}
            </div>
          </div>
        ) : (
          <span className="text-[var(--color-text-tertiary)]">—</span>
        ),
      className: 'text-right',
    },
    {
      key: 'status',
      header: 'Estado',
      render: (r: MinuteBucketReportRow) =>
        r.is_blocked ? (
          <Badge variant="error">
            <PhoneOff className="w-3 h-3 mr-1 inline" /> Bloqueado
          </Badge>
        ) : r.utilization_percent >= 80 ? (
          <Badge variant="warning">Cerca del límite</Badge>
        ) : (
          <Badge variant="success">Activo</Badge>
        ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight flex items-center">
            <Timer className="w-8 h-8 mr-3 text-blue-600" />
            Bolsas de Minutos
          </h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Estado del ciclo actual y reporte mensual de consumo (Plan Control / Plan Abierto)
          </p>
        </div>
        <button
          onClick={handleExport}
          className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <Download className="w-4 h-4 mr-2" />
          Exportar CSV
        </button>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-end gap-4 bg-[var(--color-bg-card)] rounded-lg border border-[var(--color-border-primary)] p-4">
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
            Mes
          </label>
          <input
            type="month"
            value={month}
            onChange={(e) => setMonth(e.target.value)}
            className="px-3 py-2 border rounded-lg text-sm"
          />
          <p className="text-xs text-[var(--color-text-tertiary)] mt-1 capitalize">
            {monthLabel(month)}
            {isCurrentMonth && ' · ciclo en curso (se actualiza solo)'}
          </p>
        </div>
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
            Tipo de plan
          </label>
          <select
            value={bucketTypeFilter}
            onChange={(e) => setBucketTypeFilter(e.target.value)}
            className="px-3 py-2 border rounded-lg text-sm"
          >
            <option value="">Todos</option>
            <option value="control">Control</option>
            <option value="abierto">Abierto</option>
          </select>
        </div>
        <div>
          <label className="block text-xs font-medium text-[var(--color-text-secondary)] mb-1">
            Cuenta
          </label>
          <select
            value={accountIdFilter ?? ''}
            onChange={(e) => setAccountIdFilter(e.target.value ? Number(e.target.value) : undefined)}
            className="px-3 py-2 border rounded-lg text-sm min-w-[180px]"
          >
            <option value="">Todas</option>
            {accounts
              .filter((a) => a.billing_model === 'minute_bucket')
              .map((a) => (
                <option key={a.id} value={a.id}>
                  {a.account_number}
                </option>
              ))}
          </select>
        </div>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <p className="text-sm text-[var(--color-text-tertiary)]">Cuentas con bolsa</p>
          <p className="text-2xl font-bold text-[var(--color-text-primary)]">{rows.length}</p>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <p className="text-sm text-[var(--color-text-tertiary)]">Bloqueadas (Control)</p>
          <p className="text-2xl font-bold text-red-600">{blockedCount}</p>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <p className="text-sm text-[var(--color-text-tertiary)]">Cerca del límite (≥80%)</p>
          <p className="text-2xl font-bold text-amber-600">{nearLimitCount}</p>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <p className="text-sm text-[var(--color-text-tertiary)]">Costo de excedente (mes)</p>
          <p className="text-2xl font-bold text-[var(--color-text-primary)]">
            ${totalOverageCost.toFixed(2)}
          </p>
        </div>
      </div>

      <DataTable
        columns={columns}
        data={rowsWithId}
        loading={isLoading}
        emptyMessage="No hay cuentas en modo bolsa de minutos para este período"
        searchable
        searchPlaceholder="Buscar por número de cuenta..."
        onRowClick={(r) => setDetailRow(r)}
      />

      {detailRow && (
        <BucketDetailModal row={detailRow} onClose={() => setDetailRow(null)} />
      )}
    </div>
  )
}

function BucketDetailModal({
  row,
  onClose,
}: {
  row: MinuteBucketReportRow
  onClose: () => void
}) {
  const { data: history = [], isLoading } = useQuery({
    queryKey: ['minute-bucket-history', row.account_id],
    queryFn: () => getMinuteBucketHistory(row.account_id),
  })

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[85vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-xl font-bold">{row.account_number}</h2>
            <p className="text-sm text-[var(--color-text-tertiary)]">
              {row.account_name || 'Historial de ciclos de bolsa de minutos'}
            </p>
          </div>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>

        {isLoading ? (
          <p className="text-sm text-[var(--color-text-tertiary)]">Cargando historial...</p>
        ) : history.length === 0 ? (
          <p className="text-sm text-[var(--color-text-tertiary)]">Sin ciclos registrados aún.</p>
        ) : (
          <div className="space-y-4">
            {history.map((cycle: CycleHistoryEntry) => (
              <div
                key={cycle.id}
                className="border border-[var(--color-border-primary)] rounded-lg p-4"
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="font-medium">
                    {cycle.cycle_start} → {cycle.cycle_end}
                  </span>
                  <Badge variant={cycle.status === 'open' ? 'success' : 'default'}>
                    {cycle.status === 'open' ? 'Ciclo actual' : 'Cerrado'}
                  </Badge>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-sm">
                  <div>
                    <span className="text-[var(--color-text-tertiary)]">Incluidos: </span>
                    <span className="font-mono">
                      {cycle.included_minutes}
                      {cycle.is_prorated ? ' (prorrateado)' : ''}
                    </span>
                  </div>
                  <div>
                    <span className="text-[var(--color-text-tertiary)]">Adicional: </span>
                    <span className="font-mono">{cycle.additional_minutes}</span>
                  </div>
                  <div>
                    <span className="text-[var(--color-text-tertiary)]">Consumido: </span>
                    <span className="font-mono">{Number(cycle.consumed_minutes).toFixed(0)}</span>
                  </div>
                  <div>
                    <span className="text-[var(--color-text-tertiary)]">Excedente: </span>
                    <span className="font-mono">
                      {Number(cycle.overage_minutes).toFixed(1)} (${Number(cycle.overage_cost).toFixed(2)})
                    </span>
                  </div>
                </div>

                {cycle.additions.length > 0 && (
                  <div className="mt-3 pt-3 border-t border-[var(--color-border-primary)]">
                    <p className="text-xs font-medium text-[var(--color-text-tertiary)] mb-1 flex items-center">
                      <PlusCircle className="w-3 h-3 mr-1" /> Bolsas adicionales
                    </p>
                    <ul className="space-y-1">
                      {cycle.additions.map((add) => (
                        <li key={add.id} className="text-xs text-[var(--color-text-secondary)]">
                          +{add.minutes_added} min — {new Date(add.created_at).toLocaleDateString('es-PE')}
                          {' '}por {add.created_by}
                          {add.reason && <span className="italic"> ({add.reason})</span>}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
