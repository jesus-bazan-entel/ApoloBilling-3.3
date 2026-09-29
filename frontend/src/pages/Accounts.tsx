import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchAccounts,
  createAccount,
  updateAccount,
  deleteAccount,
  fetchPlans,
  fetchSettings,
  updateSetting,
  getAccountBucketConfig,
  updateAccountBucketConfig,
  getPlanBucketConfig,
  getMinuteBucketStatus,
  topupMinuteBucket,
  getRateCardGroups,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import CreditUtilizationBar from '../components/CreditUtilizationBar'
import { BucketConfigForm } from './Plans'
import { DEFAULT_NEW_BUCKET_CONFIG } from '../lib/bucketDefaults'
import {
  Users,
  Plus,
  Edit,
  X,
  DollarSign,
  Trash2,
  Power,
  AlertTriangle,
  Shield,
  ShieldOff,
  PhoneOff,
  PlusCircle,
} from 'lucide-react'
import type { Account, AccountType, AccountStatus, Plan, BucketConfig } from '../types'
import { getAccountDisplayInfo } from '../lib/accountHelpers'

export default function AccountsPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingAccount, setEditingAccount] = useState<Account | null>(null)
  const [deleteConfirm, setDeleteConfirm] = useState<Account | null>(null)
  const queryClient = useQueryClient()

  const { data: accounts = [], isLoading } = useQuery({
    queryKey: ['accounts'],
    queryFn: fetchAccounts,
    refetchInterval: 10000,
  })

  // Cargar planes para mostrar nombre del plan en la tabla
  const { data: plans = [] } = useQuery({
    queryKey: ['plans'],
    queryFn: fetchPlans,
  })

  const { data: settings = [] } = useQuery({
    queryKey: ['settings'],
    queryFn: fetchSettings,
  })

  const updateSettingMutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      updateSetting(key, value),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] })
    },
  })

  const authRequired = settings.find(s => s.key === 'require_outbound_authorization')?.value !== 'false'

  // Helper para obtener nombre del plan
  const getPlanName = (planId?: number) => {
    if (!planId) return '-'
    const plan = plans.find(p => p.id === planId)
    return plan?.plan_name || '-'
  }

  const createMutation = useMutation({
    mutationFn: async ({ data, bucketConfig }: { data: Partial<Account>; bucketConfig?: BucketConfig }) => {
      const newAccount = await createAccount(data)
      if (bucketConfig && bucketConfig.billing_model === 'minute_bucket') {
        await updateAccountBucketConfig(newAccount.id, bucketConfig)
      }
      return newAccount
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts'] })
      queryClient.invalidateQueries({ queryKey: ['account-bucket-config'] })
      setShowModal(false)
      setEditingAccount(null)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<Account> }) =>
      updateAccount(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts'] })
      setShowModal(false)
      setEditingAccount(null)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteAccount,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts'] })
      setDeleteConfirm(null)
    },
  })

  const toggleStatusMutation = useMutation({
    mutationFn: ({ id, currentStatus }: { id: number; currentStatus: string }) => {
      const newStatus = currentStatus?.toLowerCase() === 'active' ? 'suspended' : 'active'
      return updateAccount(id, { status: newStatus })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts'] })
    },
  })

  const handleEdit = (account: Account) => {
    setEditingAccount(account)
    setShowModal(true)
  }

  const handleCreate = () => {
    setEditingAccount(null)
    setShowModal(true)
  }

  const columns = [
    {
      key: 'account_number',
      header: 'Número de Cuenta',
      render: (acc: Account) => (
        <span className="font-mono font-medium text-[var(--color-text-primary)]">
          {acc.account_number}
        </span>
      ),
    },
    {
      key: 'account_type',
      header: 'Tipo',
      render: (acc: Account) => {
        const isPrepaid = acc.account_type?.toLowerCase() === 'prepaid'
        return (
          <Badge variant={isPrepaid ? 'info' : 'warning'}>
            {isPrepaid ? 'Prepago' : 'Postpago'}
          </Badge>
        )
      },
    },
    {
      key: 'plan',
      header: 'Plan',
      render: (acc: Account) => {
        const planName = getPlanName(acc.plan_id)
        return (
          <span className={`text-sm ${planName === '-' ? 'text-[var(--color-text-muted)] italic' : 'text-[var(--color-text-secondary)]'}`}>
            {planName}
          </span>
        )
      },
    },
    {
      key: 'status',
      header: 'Estado',
      render: (acc: Account) => {
        const status = acc.status?.toLowerCase()
        return (
          <Badge
            variant={
              status === 'active'
                ? 'success'
                : status === 'suspended'
                ? 'warning'
                : 'error'
            }
          >
            {status === 'active'
              ? 'Activa'
              : status === 'suspended'
              ? 'Suspendida'
              : 'Cerrada'}
          </Badge>
        )
      },
    },
    {
      key: 'balance',
      header: 'Estado Financiero',
      render: (acc: Account) => <AccountFinancialCell account={acc} />,
      className: 'text-right min-w-[200px]',
    },
    {
      key: 'created_at',
      header: 'Creada',
      render: (acc: Account) =>
        new Date(acc.created_at).toLocaleDateString('es-PE'),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (acc: Account) => {
        const isActive = acc.status?.toLowerCase() === 'active'
        const isToggling = toggleStatusMutation.isPending
        return (
          <div className="flex items-center gap-1">
            {/* Editar */}
            <button
              onClick={() => handleEdit(acc)}
              className="p-1.5 text-[var(--color-text-tertiary)] hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
              title="Editar cuenta"
            >
              <Edit className="w-4 h-4" />
            </button>

            {/* Activar/Desactivar */}
            <button
              onClick={() => toggleStatusMutation.mutate({ id: acc.id, currentStatus: acc.status })}
              disabled={isToggling}
              className={`p-1.5 rounded transition-colors ${
                isActive
                  ? 'text-[var(--color-text-tertiary)] hover:text-amber-600 hover:bg-amber-50'
                  : 'text-amber-600 hover:text-green-600 hover:bg-green-50'
              }`}
              title={isActive ? 'Suspender cuenta' : 'Activar cuenta'}
            >
              <Power className="w-4 h-4" />
            </button>

            {/* Eliminar */}
            <button
              onClick={() => setDeleteConfirm(acc)}
              className="p-1.5 text-[var(--color-text-tertiary)] hover:text-red-600 hover:bg-red-50 rounded transition-colors"
              title="Eliminar cuenta"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        )
      },
    },
  ]

  // Summary stats
  const totalBalance = accounts.reduce((sum, acc) => sum + (Number(acc.balance) || 0), 0)
  const activeAccounts = accounts.filter((acc) => acc.status?.toLowerCase() === 'active').length
  const prepaidAccounts = accounts.filter((acc) => acc.account_type?.toLowerCase() === 'prepaid').length
  const postpaidAccounts = accounts.filter((acc) => acc.account_type?.toLowerCase() === 'postpaid').length

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">
            Gestión de Cuentas
          </h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Administra cuentas prepago y postpago
          </p>
        </div>
        <button
          onClick={handleCreate}
          className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <Plus className="w-5 h-5 mr-2" />
          Nueva Cuenta
        </button>
      </div>

      {/* Authorization Setting Banner */}
      <div className={`rounded-lg border p-4 flex items-center justify-between ${
        authRequired
          ? 'bg-blue-50 border-blue-200'
          : 'bg-amber-50 border-amber-200'
      }`}>
        <div className="flex items-center gap-3">
          {authRequired ? (
            <Shield className="w-5 h-5 text-blue-600" />
          ) : (
            <ShieldOff className="w-5 h-5 text-amber-600" />
          )}
          <div>
            <p className={`text-sm font-medium ${authRequired ? 'text-blue-900' : 'text-amber-900'}`}>
              {authRequired
                ? 'Control de autorización activo'
                : 'Control de autorización desactivado'}
            </p>
            <p className={`text-xs ${authRequired ? 'text-blue-700' : 'text-amber-700'}`}>
              {authRequired
                ? 'Las llamadas salientes requieren cuenta activa con saldo para ser autorizadas'
                : 'Todas las llamadas salientes se permiten sin verificación de cuenta ni saldo'}
            </p>
          </div>
        </div>
        <button
          onClick={() => updateSettingMutation.mutate({
            key: 'require_outbound_authorization',
            value: authRequired ? 'false' : 'true'
          })}
          disabled={updateSettingMutation.isPending}
          className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
            authRequired
              ? 'bg-amber-100 text-amber-800 hover:bg-amber-200 border border-amber-300'
              : 'bg-blue-100 text-blue-800 hover:bg-blue-200 border border-blue-300'
          } disabled:opacity-50`}
        >
          {updateSettingMutation.isPending
            ? 'Cambiando...'
            : authRequired
              ? 'Desactivar Control'
              : 'Activar Control'}
        </button>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Total Cuentas</p>
              <p className="text-2xl font-bold text-[var(--color-text-primary)]">{accounts.length}</p>
            </div>
            <Users className="w-8 h-8 text-blue-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Activas</p>
              <p className="text-2xl font-bold text-green-600">{activeAccounts}</p>
            </div>
            <Users className="w-8 h-8 text-green-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Prepago / Postpago</p>
              <p className="text-2xl font-bold text-[var(--color-text-primary)]">
                {prepaidAccounts} / {postpaidAccounts}
              </p>
            </div>
            <DollarSign className="w-8 h-8 text-purple-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Saldo Total</p>
              <p className="text-2xl font-bold text-green-600">
                S/{totalBalance.toFixed(2)}
              </p>
            </div>
            <DollarSign className="w-8 h-8 text-green-500" />
          </div>
        </div>
      </div>

      {/* Accounts Table */}
      <DataTable
        columns={columns}
        data={accounts}
        loading={isLoading}
        emptyMessage="No hay cuentas registradas"
        searchable={true}
        searchPlaceholder="Buscar por número de cuenta..."
      />

      {/* Create/Edit Modal */}
      {showModal && (
        <AccountModal
          account={editingAccount}
          onClose={() => {
            setShowModal(false)
            setEditingAccount(null)
          }}
          onSubmit={(data, bucketConfig) => {
            if (editingAccount) {
              updateMutation.mutate({ id: editingAccount.id, data })
            } else {
              createMutation.mutate({ data, bucketConfig })
            }
          }}
          isLoading={createMutation.isPending || updateMutation.isPending}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deleteConfirm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
            <div className="p-6">
              <div className="flex items-center gap-4 mb-4">
                <div className="w-12 h-12 rounded-full bg-red-100 flex items-center justify-center">
                  <AlertTriangle className="w-6 h-6 text-red-600" />
                </div>
                <div>
                  <h3 className="text-lg font-bold text-[var(--color-text-primary)]">Eliminar Cuenta</h3>
                  <p className="text-sm text-[var(--color-text-tertiary)]">Esta acción no se puede deshacer</p>
                </div>
              </div>

              <div className="bg-[var(--color-bg-secondary)] rounded-lg p-4 mb-4">
                <p className="text-sm text-[var(--color-text-secondary)]">
                  ¿Estás seguro de eliminar la cuenta{' '}
                  <span className="font-mono font-bold text-[var(--color-text-primary)]">
                    {deleteConfirm.account_number}
                  </span>
                  ?
                </p>
                {Number(deleteConfirm.balance) > 0 && (
                  <p className="text-sm text-amber-600 mt-2">
                    <strong>Advertencia:</strong> Esta cuenta tiene un saldo de $
                    {Number(deleteConfirm.balance).toFixed(2)}
                  </p>
                )}
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
  onSubmit: (data: Partial<Account>, bucketConfig?: BucketConfig) => void
  isLoading: boolean
}

function AccountModal({ account, onClose, onSubmit, isLoading }: AccountModalProps) {
  const isEditing = !!account
  const [selectedPlan, setSelectedPlan] = useState<Plan | null>(null)
  const [formData, setFormData] = useState({
    account_number: account?.account_number || '',
    // No hay cuentas prepago/postpago en el modelo de Fibertel — este campo
    // sigue existiendo en el esquema pero es inerte para cuentas de bolsa
    // (AuthorizationService nunca lo lee en el camino minute_bucket). Una
    // cuenta nueva siempre nace Control/Abierto, así que 'postpaid'/0/0 es
    // un default fijo, no una elección del usuario.
    account_type: account?.account_type?.toLowerCase() || 'postpaid',
    initial_balance: 0, // Solo para crear - backend usa initial_balance
    credit_limit: Number(account?.credit_limit) || 0,
    status: account?.status?.toLowerCase() || 'active',
    max_concurrent_calls: account?.max_concurrent_calls || 5,
    plan_id: account?.plan_id,
  })
  // Solo se usa al crear: en edición la bolsa se configura/guarda aparte
  // vía AccountBucketConfigSection, una vez que la cuenta ya tiene id.
  const [newBucketConfig, setNewBucketConfig] = useState<BucketConfig>(DEFAULT_NEW_BUCKET_CONFIG)

  // Cargar planes disponibles (solo para crear nuevas cuentas)
  const { data: plans = [] } = useQuery({
    queryKey: ['plans'],
    queryFn: fetchPlans,
    enabled: !isEditing, // Solo cargar cuando creamos una cuenta
  })

  const { data: cardGroups = ['standard'] } = useQuery({
    queryKey: ['rate-card-groups'],
    queryFn: getRateCardGroups,
    enabled: !isEditing,
  })

  // Handler para cambio de plan (auto-poblar campos, incluida la bolsa si el plan es minute_bucket).
  // El fetch de la bolsa del plan se resuelve ANTES de tocar el estado, para
  // que selectedPlan/formData/newBucketConfig se actualicen juntos en un
  // solo render (BucketConfigForm se remonta por `key={selectedPlan.id}` y
  // debe encontrar ya el valor heredado correcto, no el default previo).
  const handlePlanChange = async (planId: string) => {
    if (!planId) {
      setSelectedPlan(null)
      setNewBucketConfig(DEFAULT_NEW_BUCKET_CONFIG)
      return
    }

    const plan = plans.find(p => p.id === Number(planId))
    if (!plan) return

    let bucketConfig = DEFAULT_NEW_BUCKET_CONFIG
    try {
      const planBucket = await getPlanBucketConfig(plan.id)
      if (planBucket.billing_model === 'minute_bucket') {
        bucketConfig = {
          billing_model: planBucket.billing_model,
          bucket_type: planBucket.bucket_type,
          included_minutes: planBucket.included_minutes,
          proration_mode: planBucket.proration_mode,
          overage_flat_rate_per_minute: planBucket.overage_flat_rate_per_minute,
          overage_tariff_group: planBucket.overage_tariff_group,
        }
      }
    } catch {
      // Sin config de bolsa para este plan: se mantiene el default (saldo)
    }

    setSelectedPlan(plan)

    // Auto-poblar valores del plan
    const isPrepaid = plan.account_type.toLowerCase() === 'prepaid'
    setFormData({
      ...formData,
      plan_id: plan.id,
      account_type: plan.account_type.toLowerCase(),
      initial_balance: isPrepaid ? Number(plan.initial_balance) : 0,
      credit_limit: !isPrepaid ? Number(plan.credit_limit) : 0,
      max_concurrent_calls: plan.max_concurrent_calls,
    })
    setNewBucketConfig(bucketConfig)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (isEditing) {
      // Solo enviar campos que se pueden editar
      onSubmit({
        status: formData.status as AccountStatus,
        credit_limit: formData.credit_limit,
        max_concurrent_calls: formData.max_concurrent_calls,
      })
    } else {
      // Crear con initial_balance y plan_id (si aplica)
      const payload: Partial<Account> & { initial_balance?: number } = {
        account_number: formData.account_number,
        account_type: formData.account_type as AccountType,
        initial_balance: formData.initial_balance,
        credit_limit: formData.credit_limit,
        max_concurrent_calls: formData.max_concurrent_calls,
      }

      // Solo incluir plan_id si hay un plan seleccionado
      if (formData.plan_id) {
        payload.plan_id = formData.plan_id
      }

      onSubmit(payload as Partial<Account>, newBucketConfig)
    }
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">
            {account ? 'Editar Cuenta' : 'Nueva Cuenta'}
          </h2>
          <button
            onClick={onClose}
            className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {!isEditing && (
            <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-4">
              <label className="block text-sm font-medium text-blue-900 mb-2">
                Seleccionar Plan (Opcional)
              </label>
              <select
                value={selectedPlan?.id || ''}
                onChange={(e) => handlePlanChange(e.target.value)}
                className="w-full px-3 py-2 border border-blue-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 bg-[var(--color-bg-card)]"
              >
                <option value="">Sin plan (configuración manual)</option>
                {plans
                  .filter(p => p.enabled)
                  .map((plan) => (
                    <option key={plan.id} value={plan.id}>
                      {plan.plan_name}
                    </option>
                  ))}
              </select>
              {selectedPlan && (
                <p className="text-xs text-blue-700 mt-2">
                  ℹ️ Los valores del plan se han aplicado automáticamente y están bloqueados
                </p>
              )}
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Número de Cuenta *
              </label>
              <input
                type="text"
                required
                value={formData.account_number}
                onChange={(e) =>
                  setFormData({ ...formData, account_number: e.target.value })
                }
                disabled={!!account}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-[var(--color-bg-secondary)]"
              />
            </div>

            {isEditing && (
              <>
                <div>
                  <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                    Tipo de Cuenta *
                  </label>
                  <select
                    value={formData.account_type?.toLowerCase()}
                    disabled
                    className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-[var(--color-bg-secondary)]"
                  >
                    <option value="prepaid">Prepago (legacy)</option>
                    <option value="postpaid">Postpago (legacy)</option>
                  </select>
                  <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                    Campo legacy del modelo de saldo — las cuentas nuevas son Control/Abierto.
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                    Saldo Actual
                  </label>
                  <div className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)] font-mono">
                    S/{Number(account?.balance || 0).toFixed(2)}
                  </div>
                  <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                    Para modificar el saldo, usa la opción "Recargar" en Gestión de Saldos
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                    {formData.account_type === 'postpaid' ? 'Límite de Crédito *' : 'Límite de Crédito'}
                  </label>
                  <input
                    type="number"
                    step="0.01"
                    min={formData.account_type === 'postpaid' ? '0.01' : '0'}
                    required={formData.account_type === 'postpaid'}
                    value={formData.credit_limit}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        credit_limit: parseFloat(e.target.value) || 0,
                      })
                    }
                    className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  />
                  {formData.account_type === 'postpaid' && (
                    <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                      Postpago requiere límite de crédito {'>'} 0
                    </p>
                  )}
                </div>
              </>
            )}

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Estado
              </label>
              <select
                value={formData.status?.toLowerCase()}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    status: e.target.value as AccountStatus,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="active">Activa</option>
                <option value="suspended">Suspendida</option>
                <option value="closed">Cerrada</option>
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Llamadas Concurrentes Máx. *
              </label>
              <input
                type="number"
                min="1"
                max="100"
                required
                value={formData.max_concurrent_calls}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    max_concurrent_calls: parseInt(e.target.value) || 5,
                  })
                }
                disabled={!!selectedPlan && !isEditing}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:bg-[var(--color-bg-secondary)]"
              />
              {selectedPlan && !isEditing && (
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">🔒 Bloqueado por plan</p>
              )}
            </div>
          </div>

          {isEditing && account ? (
            <AccountBucketConfigSection accountId={account.id} />
          ) : (
            <div>
              <BucketConfigForm
                key={selectedPlan?.id ?? 'manual'}
                title="Tipo de Plan"
                initial={newBucketConfig}
                cardGroups={cardGroups}
                isSaving={false}
                saved={false}
                onChange={setNewBucketConfig}
                lockToBucket
              />
              {selectedPlan && (
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  ℹ️ Precargado desde el plan "{selectedPlan.plan_name}" — podés ajustarlo antes de crear.
                </p>
              )}
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
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : account ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

/**
 * Financial status cell for the accounts table. Balance-model accounts show
 * the existing prepaid/postpaid widgets unchanged; minute-bucket accounts
 * show a bucket meter instead, fetched lazily per row.
 */
function AccountFinancialCell({ account: acc }: { account: Account }) {
  const { data: bucketConfig } = useQuery({
    queryKey: ['account-bucket-config', acc.id],
    queryFn: () => getAccountBucketConfig(acc.id),
    staleTime: 30000,
  })

  if (bucketConfig?.billing_model === 'minute_bucket') {
    return <BucketMeterCell accountId={acc.id} bucketType={bucketConfig.bucket_type} />
  }

  const info = getAccountDisplayInfo(acc)

  if (info.isPrepaid) {
    return (
      <div className="text-right">
        <div
          className={`font-mono font-bold ${
            info.balanceColor === 'success'
              ? 'text-green-600'
              : info.balanceColor === 'warning'
              ? 'text-yellow-600'
              : 'text-red-600'
          }`}
        >
          S/{info.currentBalance?.toFixed(2)}
        </div>
        <div className="text-xs text-[var(--color-text-tertiary)]">Saldo Disponible</div>
      </div>
    )
  }

  return (
    <div className="text-right space-y-1 min-w-[200px]">
      <div className="text-sm font-medium text-[var(--color-text-secondary)]">
        Consumido: S/{info.consumedCredit?.toFixed(2)}
      </div>
      <CreditUtilizationBar utilizationPercent={info.utilizationPercent || 0} showLabel={false} />
      <div className="text-xs text-[var(--color-text-tertiary)]">
        Disponible: S/{info.availableCredit?.toFixed(2)} de S/{info.creditLimit?.toFixed(2)}
      </div>
    </div>
  )
}

function BucketMeterCell({
  accountId,
  bucketType,
}: {
  accountId: number
  bucketType: 'control' | 'abierto' | null
}) {
  const [showTopup, setShowTopup] = useState(false)
  const queryClient = useQueryClient()

  const { data: status, isLoading } = useQuery({
    queryKey: ['minute-bucket-status', accountId],
    queryFn: () => getMinuteBucketStatus(accountId),
    refetchInterval: 15000,
  })

  if (isLoading || !status) {
    return <span className="text-xs text-[var(--color-text-tertiary)]">Cargando bolsa...</span>
  }

  return (
    <div className="text-right space-y-1 min-w-[200px]">
      <div className="flex items-center justify-end gap-2">
        {status.is_blocked ? (
          <Badge variant="error">
            <PhoneOff className="w-3 h-3 mr-1 inline" /> Bloqueado
          </Badge>
        ) : (
          <Badge variant="success">Activo</Badge>
        )}
        <Badge variant={bucketType === 'control' ? 'warning' : 'info'}>
          {bucketType === 'control' ? 'Control' : 'Abierto'}
        </Badge>
      </div>
      <div className="text-sm font-mono">
        {Number(status.consumed_minutes).toFixed(0)} / {status.included_minutes + status.additional_minutes} min
      </div>
      <CreditUtilizationBar utilizationPercent={status.utilization_percent} showLabel={false} />
      <div className="text-xs text-[var(--color-text-tertiary)]">
        Disponible: {Number(status.available_minutes).toFixed(0)} min
        {Number(status.overage_minutes) > 0 && ` · Excedente: ${Number(status.overage_minutes).toFixed(1)} min`}
      </div>
      <button
        type="button"
        onClick={() => setShowTopup(true)}
        className="text-xs text-blue-600 hover:underline flex items-center justify-end gap-1 ml-auto"
      >
        <PlusCircle className="w-3 h-3" /> Agregar bolsa
      </button>

      {showTopup && (
        <TopupBucketModal
          accountId={accountId}
          onClose={() => setShowTopup(false)}
          onSaved={() => {
            queryClient.invalidateQueries({ queryKey: ['minute-bucket-status', accountId] })
            setShowTopup(false)
          }}
        />
      )}
    </div>
  )
}

function TopupBucketModal({
  accountId,
  onClose,
  onSaved,
}: {
  accountId: number
  onClose: () => void
  onSaved: () => void
}) {
  const [minutes, setMinutes] = useState(100)
  const [reason, setReason] = useState('')

  const mutation = useMutation({
    mutationFn: () => topupMinuteBucket(accountId, minutes, reason || undefined),
    onSuccess: onSaved,
  })

  return (
    <div
      className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-sm w-full mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-bold">Agregar Bolsa Adicional</h3>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="space-y-3">
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Minutos a agregar
            </label>
            <input
              type="number"
              min="1"
              value={minutes}
              onChange={(e) => setMinutes(Number(e.target.value))}
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Motivo (opcional)
            </label>
            <input
              type="text"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Ej. Orden #4521"
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>
          {mutation.isError && (
            <p className="text-sm text-red-600">No se pudo agregar la bolsa. Intenta de nuevo.</p>
          )}
          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)]"
            >
              Cancelar
            </button>
            <button
              type="button"
              disabled={mutation.isPending || minutes <= 0}
              onClick={() => mutation.mutate()}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {mutation.isPending ? 'Agregando...' : 'Agregar'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

/**
 * Account-level minute-bucket configuration override, used inside the
 * account edit modal. Saves independently to `/accounts/{id}/bucket-config`
 * (separate endpoint/table from the rest of the account), same pattern as
 * `BucketConfigSection` in Plans.tsx — reuses `BucketConfigForm` and adds the
 * account-specific `notification_email` field via the `extra` slot.
 */
function AccountBucketConfigSection({ accountId }: { accountId: number }) {
  const queryClient = useQueryClient()
  const [saved, setSaved] = useState(false)
  const [notificationEmail, setNotificationEmail] = useState<string | null>(null)

  const { data, isLoading } = useQuery({
    queryKey: ['account-bucket-config', accountId],
    queryFn: () => getAccountBucketConfig(accountId),
  })

  const { data: cardGroups = ['standard'] } = useQuery({
    queryKey: ['rate-card-groups'],
    queryFn: getRateCardGroups,
  })

  const mutation = useMutation({
    mutationFn: (config: BucketConfig) =>
      updateAccountBucketConfig(accountId, {
        ...config,
        notification_email: notificationEmail,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['account-bucket-config', accountId] })
      queryClient.invalidateQueries({ queryKey: ['minute-bucket-status', accountId] })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    },
  })

  if (isLoading || !data) {
    return (
      <div className="border-t border-[var(--color-border-primary)] pt-4 mt-2">
        <p className="text-sm text-[var(--color-text-tertiary)]">Cargando configuración de bolsa...</p>
      </div>
    )
  }

  const email = notificationEmail !== null ? notificationEmail : data.notification_email || ''

  return (
    <BucketConfigForm
      title="Facturación: Bolsa de Minutos (override de cuenta)"
      initial={data}
      cardGroups={cardGroups}
      isSaving={mutation.isPending}
      saved={saved}
      onSave={(config) => mutation.mutate(config)}
      extra={
        data.billing_model === 'minute_bucket' ? (
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Email de notificación (alertas 80% / 100%)
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setNotificationEmail(e.target.value)}
              placeholder="facturacion@cliente.com"
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>
        ) : undefined
      }
    />
  )
}
