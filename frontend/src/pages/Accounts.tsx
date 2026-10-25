import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient, keepPreviousData } from '@tanstack/react-query'
import { fetchAccountsPaginated, fetchAccountStats, createAccount, updateAccount, deleteAccount } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { Users, Plus, Edit, X, Trash2, Power, AlertTriangle, Search, UserX } from 'lucide-react'
import type { Account, AccountStatus } from '../types'

const PER_PAGE = 25

const STATUS_LABEL: Record<string, string> = { active: 'Activo', suspended: 'Suspendido', closed: 'Cerrado' }

/**
 * Customers (subscribers). On this SBC an account is who a SIP device belongs
 * to: any status other than active blocks its devices' REGISTER.
 */
export default function AccountsPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingAccount, setEditingAccount] = useState<Account | null>(null)
  const [deleteConfirm, setDeleteConfirm] = useState<Account | null>(null)
  const [page, setPage] = useState(1)
  const [searchInput, setSearchInput] = useState('')
  const [search, setSearch] = useState('')
  const queryClient = useQueryClient()

  // Debounce the search box, and reset to page 1 whenever the term changes.
  useEffect(() => {
    const t = setTimeout(() => {
      setSearch(searchInput)
      setPage(1)
    }, 400)
    return () => clearTimeout(t)
  }, [searchInput])

  const { data: accountsPage, isLoading } = useQuery({
    queryKey: ['accounts', page, search],
    queryFn: () => fetchAccountsPaginated(page, PER_PAGE, search),
    refetchInterval: 10000,
    placeholderData: keepPreviousData,
  })

  const accounts = accountsPage?.data ?? []
  const totalPages = accountsPage?.total_pages ?? 1

  // Global aggregates for the summary cards (across all customers, not just this page).
  const { data: stats } = useQuery({
    queryKey: ['accountStats'],
    queryFn: fetchAccountStats,
    refetchInterval: 10000,
  })

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: ['accounts'] })
    queryClient.invalidateQueries({ queryKey: ['accountStats'] })
  }

  const createMutation = useMutation({
    mutationFn: createAccount,
    onSuccess: () => {
      invalidate()
      setShowModal(false)
      setEditingAccount(null)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<Account> }) => updateAccount(id, data),
    onSuccess: () => {
      invalidate()
      setShowModal(false)
      setEditingAccount(null)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteAccount,
    onSuccess: () => {
      invalidate()
      setDeleteConfirm(null)
    },
  })

  const toggleStatusMutation = useMutation({
    mutationFn: ({ id, currentStatus }: { id: number; currentStatus: string }) => {
      const newStatus = currentStatus?.toLowerCase() === 'active' ? 'suspended' : 'active'
      return updateAccount(id, { status: newStatus })
    },
    onSuccess: invalidate,
  })

  const columns = [
    {
      key: 'account_number',
      header: 'Número',
      render: (acc: Account) => (
        <span className="font-mono font-medium text-[var(--color-text-primary)]">{acc.account_number}</span>
      ),
    },
    {
      key: 'account_name',
      header: 'Cliente',
      render: (acc: Account) => (
        <div className="min-w-0">
          <div className="text-sm text-[var(--color-text-primary)] truncate">
            {acc.account_name || <span className="text-[var(--color-text-muted)] italic">Sin nombre</span>}
          </div>
          {acc.customer_phone && (
            <div className="text-xs font-mono text-[var(--color-text-tertiary)]">{acc.customer_phone}</div>
          )}
        </div>
      ),
    },
    {
      key: 'cod_ped',
      header: 'COD_PED',
      render: (acc: Account) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{acc.cod_ped || '—'}</span>
      ),
    },
    {
      key: 'location',
      header: 'Ubicación',
      render: (acc: Account) => {
        const parts = [acc.distrito, acc.provincia, acc.departamento].filter(Boolean)
        return (
          <span className="text-sm text-[var(--color-text-secondary)]">{parts.length ? parts.join(', ') : '—'}</span>
        )
      },
    },
    {
      key: 'status',
      header: 'Estado',
      render: (acc: Account) => {
        const status = acc.status?.toLowerCase()
        return (
          <Badge variant={status === 'active' ? 'success' : status === 'suspended' ? 'warning' : 'error'}>
            {STATUS_LABEL[status] ?? acc.status}
          </Badge>
        )
      },
    },
    {
      key: 'created_at',
      header: 'Alta',
      render: (acc: Account) => new Date(acc.created_at).toLocaleDateString('es-PE'),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (acc: Account) => {
        const isActive = acc.status?.toLowerCase() === 'active'
        return (
          <div className="flex items-center gap-1">
            <button
              onClick={() => {
                setEditingAccount(acc)
                setShowModal(true)
              }}
              className="p-1.5 text-[var(--color-text-tertiary)] hover:text-[#ff5a0a] hover:bg-[#fff3ed] rounded transition-colors"
              title="Editar cliente"
            >
              <Edit className="w-4 h-4" />
            </button>
            <button
              onClick={() => toggleStatusMutation.mutate({ id: acc.id, currentStatus: acc.status })}
              disabled={toggleStatusMutation.isPending}
              className={`p-1.5 rounded transition-colors ${
                isActive
                  ? 'text-[var(--color-text-tertiary)] hover:text-amber-600 hover:bg-amber-50'
                  : 'text-amber-600 hover:text-green-600 hover:bg-green-50'
              }`}
              title={isActive ? 'Suspender (bloquea el registro de sus dispositivos SIP)' : 'Activar cliente'}
            >
              <Power className="w-4 h-4" />
            </button>
            <button
              onClick={() => setDeleteConfirm(acc)}
              className="p-1.5 text-[var(--color-text-tertiary)] hover:text-red-600 hover:bg-red-50 rounded transition-colors"
              title="Eliminar cliente"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        )
      },
    },
  ]

  const totalAccounts = stats?.total_accounts ?? 0
  const activeAccounts = stats?.active_accounts ?? 0
  const blockedAccounts = Math.max(0, totalAccounts - activeAccounts)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">Clientes</h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Abonados del SBC · un cliente suspendido o cerrado no puede registrar sus dispositivos SIP
          </p>
        </div>
        <button
          onClick={() => {
            setEditingAccount(null)
            setShowModal(true)
          }}
          className="flex items-center px-4 py-2 bg-[#ff5a0a] text-white rounded-lg hover:bg-[#d8450b] transition-colors"
        >
          <Plus className="w-5 h-5 mr-2" />
          Nuevo Cliente
        </button>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Total clientes</p>
              <p className="text-2xl font-bold text-[var(--color-text-primary)]">{totalAccounts.toLocaleString('es-PE')}</p>
            </div>
            <Users className="w-8 h-8 text-[#ff5a0a]" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Activos</p>
              <p className="text-2xl font-bold text-green-600">{activeAccounts.toLocaleString('es-PE')}</p>
            </div>
            <Users className="w-8 h-8 text-green-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Suspendidos o cerrados</p>
              <p className="text-2xl font-bold text-amber-600">{blockedAccounts.toLocaleString('es-PE')}</p>
            </div>
            <UserX className="w-8 h-8 text-amber-500" />
          </div>
        </div>
      </div>

      {/* Search box (server-side, searches across all pages) */}
      <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)] p-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-[var(--color-text-tertiary)]" />
          <input
            type="text"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder="Buscar por número, nombre o teléfono..."
            className="w-full pl-10 pr-4 py-2 bg-[var(--color-bg-primary)] border border-[var(--color-border-primary)] text-[var(--color-text-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a] transition-colors placeholder:text-[var(--color-text-muted)]"
          />
        </div>
      </div>

      <DataTable
        columns={columns}
        data={accounts}
        loading={isLoading}
        emptyMessage="No hay clientes registrados"
        pagination={{ page, totalPages, onPageChange: setPage }}
      />

      {showModal && (
        <AccountModal
          account={editingAccount}
          onClose={() => {
            setShowModal(false)
            setEditingAccount(null)
          }}
          onSubmit={(data) => {
            if (editingAccount) {
              updateMutation.mutate({ id: editingAccount.id, data })
            } else {
              createMutation.mutate(data)
            }
          }}
          isLoading={createMutation.isPending || updateMutation.isPending}
        />
      )}

      {deleteConfirm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
            <div className="p-6">
              <div className="flex items-center gap-4 mb-4">
                <div className="w-12 h-12 rounded-full bg-red-100 flex items-center justify-center">
                  <AlertTriangle className="w-6 h-6 text-red-600" />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-[var(--color-text-primary)]">Eliminar cliente</h3>
                  <p className="text-sm text-[var(--color-text-tertiary)]">Esta acción no se puede deshacer</p>
                </div>
              </div>

              <div className="bg-[var(--color-bg-secondary)] rounded-lg p-4 mb-4">
                <p className="text-sm text-[var(--color-text-secondary)]">
                  ¿Eliminar el cliente{' '}
                  <span className="font-mono font-bold text-[var(--color-text-primary)]">{deleteConfirm.account_number}</span>?
                  Sus dispositivos SIP dejarán de poder registrarse.
                </p>
              </div>

              <div className="flex justify-end gap-3">
                <button
                  onClick={() => setDeleteConfirm(null)}
                  className="px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)]"
                >
                  Cancelar
                </button>
                <button
                  onClick={() => deleteMutation.mutate(deleteConfirm.id)}
                  disabled={deleteMutation.isPending}
                  className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 flex items-center gap-2"
                >
                  {deleteMutation.isPending ? (
                    'Eliminando...'
                  ) : (
                    <>
                      <Trash2 className="w-4 h-4" />
                      Eliminar
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

interface AccountModalProps {
  account: Account | null
  onClose: () => void
  onSubmit: (data: Partial<Account>) => void
  isLoading: boolean
}

function AccountModal({ account, onClose, onSubmit, isLoading }: AccountModalProps) {
  const isEditing = !!account
  const [formData, setFormData] = useState({
    account_number: account?.account_number || '',
    customer_phone: account?.customer_phone || '',
    status: account?.status?.toLowerCase() || 'active',
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (isEditing) {
      // "" (not undefined) so a cleared phone actually persists
      onSubmit({ customer_phone: formData.customer_phone, status: formData.status as AccountStatus })
    } else {
      onSubmit({ account_number: formData.account_number, customer_phone: formData.customer_phone || undefined })
    }
  }

  const field =
    'w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a] disabled:bg-[var(--color-bg-secondary)]'

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-lg w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">
            {isEditing ? 'Editar cliente' : 'Nuevo cliente'}
          </h2>
          <button onClick={onClose} className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]">
            <X className="w-6 h-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Número de cliente *</label>
            <input
              type="text"
              required
              value={formData.account_number}
              onChange={(e) => setFormData({ ...formData, account_number: e.target.value })}
              disabled={isEditing}
              className={field}
            />
            {!isEditing && (
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Para crear cliente y dispositivo SIP juntos usa Aprovisionamiento.
              </p>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Teléfono de contacto</label>
            <input
              type="text"
              value={formData.customer_phone}
              onChange={(e) => setFormData({ ...formData, customer_phone: e.target.value })}
              className={field}
            />
          </div>

          {isEditing && (
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Estado</label>
              <select
                value={formData.status}
                onChange={(e) => setFormData({ ...formData, status: e.target.value as AccountStatus })}
                className={field}
              >
                <option value="active">Activo</option>
                <option value="suspended">Suspendido</option>
                <option value="closed">Cerrado</option>
              </select>
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Suspender o cerrar desregistra sus dispositivos SIP y bloquea nuevos registros.
              </p>
            </div>
          )}

          <div className="flex justify-end space-x-3 pt-4 border-t border-[var(--color-border-primary)]">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)]"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={isLoading}
              className="px-4 py-2 bg-[#ff5a0a] text-white rounded-lg hover:bg-[#d8450b] disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : isEditing ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
