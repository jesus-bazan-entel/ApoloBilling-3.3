import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchPlans,
  createPlan,
  updatePlan,
  deletePlan,
  getPlanBucketConfig,
  updatePlanBucketConfig,
  getRateCardGroups,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { FileText, Plus, Edit, X, Trash2, AlertCircle, Timer, Check } from 'lucide-react'
import type { Plan, AccountType, BucketConfig } from '../types'
import { DEFAULT_NEW_BUCKET_CONFIG } from '../lib/bucketDefaults'

export default function PlansPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingPlan, setEditingPlan] = useState<Plan | null>(null)
  const [deletingPlan, setDeletingPlan] = useState<Plan | null>(null)
  const [error, setError] = useState('')
  const queryClient = useQueryClient()

  const { data: plans = [], isLoading } = useQuery({
    queryKey: ['plans'],
    queryFn: fetchPlans,
  })

  const createMutation = useMutation({
    mutationFn: async ({ data, bucketConfig }: { data: Partial<Plan>; bucketConfig?: BucketConfig }) => {
      const newPlan = await createPlan(data)
      if (bucketConfig && bucketConfig.billing_model === 'minute_bucket') {
        await updatePlanBucketConfig(newPlan.id, bucketConfig)
      }
      return newPlan
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      queryClient.invalidateQueries({ queryKey: ['plan-bucket-config'] })
      setShowModal(false)
      setEditingPlan(null)
      setError('')
    },
    onError: (err: Error) => setError(err.message),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<Plan> }) =>
      updatePlan(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      setShowModal(false)
      setEditingPlan(null)
      setError('')
    },
    onError: (err: Error) => setError(err.message),
  })

  const deleteMutation = useMutation({
    mutationFn: deletePlan,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      setDeletingPlan(null)
    },
  })

  const handleEdit = (plan: Plan) => {
    setEditingPlan(plan)
    setShowModal(true)
    setError('')
  }

  const handleCreate = () => {
    setEditingPlan(null)
    setShowModal(true)
    setError('')
  }

  const handleDelete = () => {
    if (deletingPlan) {
      deleteMutation.mutate(deletingPlan.id)
    }
  }

  const columns = [
    {
      key: 'plan_code',
      header: 'Código',
      render: (plan: Plan) => (
        <span className="font-mono font-medium">{plan.plan_code}</span>
      ),
    },
    {
      key: 'plan_name',
      header: 'Nombre',
    },
    {
      key: 'account_type',
      header: 'Tipo',
      render: (plan: Plan) => {
        const isPrepaid = plan.account_type?.toLowerCase() === 'prepaid'
        return (
          <Badge variant={isPrepaid ? 'info' : 'warning'}>
            {isPrepaid ? 'Prepago' : 'Postpago'}
          </Badge>
        )
      },
    },
    {
      key: 'amount',
      header: 'Monto',
      render: (plan: Plan) => {
        const isPrepaid = plan.account_type?.toLowerCase() === 'prepaid'
        const amount = isPrepaid ? plan.initial_balance : plan.credit_limit
        return (
          <span className="font-mono">
            S/{Number(amount).toFixed(2)}
            <span className="text-xs text-[var(--color-text-tertiary)] ml-1">
              {isPrepaid ? 'inicial' : 'límite'}
            </span>
          </span>
        )
      },
    },
    {
      key: 'max_concurrent_calls',
      header: 'Llamadas',
      render: (plan: Plan) => <span>{plan.max_concurrent_calls}</span>,
    },
    {
      key: 'billing_model',
      header: 'Facturación',
      render: (plan: Plan) => <PlanBillingModelBadge planId={plan.id} />,
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (plan: Plan) => (
        <Badge variant={plan.enabled ? 'success' : 'error'}>
          {plan.enabled ? 'Activo' : 'Inactivo'}
        </Badge>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (plan: Plan) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => handleEdit(plan)}
            className="p-1 hover:bg-[var(--color-bg-secondary)] rounded"
            title="Editar"
          >
            <Edit className="w-4 h-4 text-[var(--color-text-secondary)]" />
          </button>
          <button
            onClick={() => setDeletingPlan(plan)}
            className="p-1 hover:bg-[var(--color-bg-secondary)] rounded"
            title="Eliminar"
          >
            <Trash2 className="w-4 h-4 text-red-600" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-[var(--color-text-primary)] flex items-center">
            <FileText className="w-8 h-8 mr-3 text-blue-600" />
            Planes
          </h1>
          <p className="text-[var(--color-text-secondary)] mt-1">
            Gestión de planes prepago y postpago
          </p>
        </div>
        <button
          onClick={handleCreate}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center"
        >
          <Plus className="w-4 h-4 mr-2" />
          Nuevo Plan
        </button>
      </div>

      <DataTable
        data={plans}
        columns={columns}
        loading={isLoading}
        emptyMessage="No hay planes configurados"
      />

      {showModal && (
        <PlanModal
          plan={editingPlan}
          error={error}
          onClose={() => {
            setShowModal(false)
            setEditingPlan(null)
            setError('')
          }}
          onSave={(data, bucketConfig) => {
            if (editingPlan) {
              updateMutation.mutate({ id: editingPlan.id, data })
            } else {
              createMutation.mutate({ data, bucketConfig })
            }
          }}
        />
      )}

      {deletingPlan && (
        <DeleteModal
          plan={deletingPlan}
          onClose={() => setDeletingPlan(null)}
          onConfirm={handleDelete}
        />
      )}
    </div>
  )
}

function PlanModal({
  plan,
  error,
  onClose,
  onSave,
}: {
  plan: Plan | null
  error: string
  onClose: () => void
  onSave: (data: Partial<Plan>, bucketConfig?: BucketConfig) => void
}) {
  const [formData, setFormData] = useState({
    plan_name: plan?.plan_name || '',
    plan_code: plan?.plan_code || '',
    // No hay planes prepago/postpago en el modelo de Fibertel — account_type
    // sigue existiendo en el esquema pero es inerte para planes de bolsa
    // (ver AuthorizationService::authorize, nunca lo lee en el camino
    // minute_bucket). Un plan nuevo siempre nace como Control/Abierto, así
    // que estos valores son un default fijo, no una elección del usuario.
    account_type: plan?.account_type || 'POSTPAID',
    initial_balance: plan?.initial_balance || 0,
    credit_limit: plan?.credit_limit || 0,
    max_concurrent_calls: plan?.max_concurrent_calls || 5,
    description: plan?.description || '',
    enabled: plan?.enabled ?? true,
  })
  // Only used when creating: edit mode configures/saves its bucket
  // independently via BucketConfigSection once the plan has an id.
  const [newBucketConfig, setNewBucketConfig] = useState<BucketConfig>(DEFAULT_NEW_BUCKET_CONFIG)
  const { data: cardGroups = ['standard'] } = useQuery({
    queryKey: ['rate-card-groups'],
    queryFn: getRateCardGroups,
    enabled: !plan,
  })

  const isPrepaid = formData.account_type.toUpperCase() === 'PREPAID'

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (plan) {
      onSave(formData)
    } else {
      // billing_model aquí es solo un hint para que el backend salte la
      // validación de saldo/crédito > 0 (ver PlanCreateRequest::validate_business_rules);
      // el billing_model real del plan se fija con el PUT bucket-config que sigue.
      onSave({ ...formData, billing_model: 'minute_bucket' }, newBucketConfig)
    }
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold">
            {plan ? 'Editar Plan' : 'Nuevo Plan'}
          </h2>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="mb-4 p-3 bg-red-50 border border-red-200 rounded flex items-start">
            <AlertCircle className="w-5 h-5 text-red-600 mr-2 flex-shrink-0 mt-0.5" />
            <span className="text-sm text-red-800">{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Nombre del Plan *
              </label>
              <input
                type="text"
                required
                value={formData.plan_name}
                onChange={(e) =>
                  setFormData({ ...formData, plan_name: e.target.value })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Código *
              </label>
              <input
                type="text"
                required
                disabled={!!plan}
                value={formData.plan_code}
                onChange={(e) =>
                  setFormData({ ...formData, plan_code: e.target.value })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-[var(--color-bg-secondary)]"
              />
            </div>
          </div>

          {plan && (
            <>
              <div>
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Tipo de Cuenta *
                </label>
                <select
                  required
                  disabled
                  value={formData.account_type}
                  onChange={(e) =>
                    setFormData({ ...formData, account_type: e.target.value as AccountType })
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-[var(--color-bg-secondary)]"
                >
                  <option value="PREPAID">Prepago (legacy)</option>
                  <option value="POSTPAID">Postpago (legacy)</option>
                </select>
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Campo legacy del modelo de saldo — los planes nuevos son Control/Abierto, sin prepago/postpago.
                </p>
              </div>

          {isPrepaid ? (
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Saldo Inicial * (debe ser &gt; 0)
              </label>
              <input
                type="number"
                required
                min="0.01"
                step="0.01"
                value={formData.initial_balance}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    initial_balance: Number(e.target.value),
                  })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                El saldo inicial no puede ser cero (sin planes ilimitados)
              </p>
            </div>
          ) : (
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Límite de Crédito * (debe ser &gt; 0)
              </label>
              <input
                type="number"
                required
                min="0.01"
                step="0.01"
                value={formData.credit_limit}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    credit_limit: Number(e.target.value),
                  })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                El límite de crédito no puede ser cero (sin planes ilimitados)
              </p>
            </div>
          )}
            </>
          )}

          {!plan && (
            <BucketConfigForm
              title="Tipo de Plan"
              initial={newBucketConfig}
              cardGroups={cardGroups}
              isSaving={false}
              saved={false}
              onChange={setNewBucketConfig}
              lockToBucket
            />
          )}

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Llamadas Concurrentes *
            </label>
            <input
              type="number"
              required
              min="1"
              max="100"
              value={formData.max_concurrent_calls}
              onChange={(e) =>
                setFormData({
                  ...formData,
                  max_concurrent_calls: Number(e.target.value),
                })
              }
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Descripción
            </label>
            <textarea
              value={formData.description}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
              rows={2}
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div className="flex items-center">
            <input
              type="checkbox"
              checked={formData.enabled}
              onChange={(e) =>
                setFormData({ ...formData, enabled: e.target.checked })
              }
              className="w-4 h-4 text-blue-600 rounded focus:ring-blue-500"
            />
            <label className="ml-2 text-sm font-medium text-[var(--color-text-secondary)]">
              Plan activo
            </label>
          </div>

          {plan && <BucketConfigSection planId={plan.id} />}

          <div className="flex justify-end space-x-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)]"
            >
              Cancelar
            </button>
            <button
              type="submit"
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
            >
              {plan ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function DeleteModal({
  plan,
  onClose,
  onConfirm,
}: {
  plan: Plan
  onClose: () => void
  onConfirm: () => void
}) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-bold mb-2">Eliminar Plan</h3>
        <p className="text-[var(--color-text-secondary)] mb-4">
          ¿Estás seguro de eliminar el plan <strong>{plan.plan_name}</strong>?
        </p>
        <p className="text-sm text-[var(--color-text-tertiary)] mb-4">
          Esta acción no se puede deshacer. Las cuentas existentes creadas con
          este plan no se verán afectadas.
        </p>
        <div className="flex justify-end space-x-3">
          <button
            onClick={onClose}
            className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)]"
          >
            Cancelar
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700"
          >
            Eliminar
          </button>
        </div>
      </div>
    </div>
  )
}

/** Compact badge showing a plan's billing model, fetched lazily per row. */
function PlanBillingModelBadge({ planId }: { planId: number }) {
  const { data } = useQuery({
    queryKey: ['plan-bucket-config', planId],
    queryFn: () => getPlanBucketConfig(planId),
    staleTime: 60000,
  })

  if (!data) return <span className="text-xs text-[var(--color-text-tertiary)]">…</span>

  if (data.billing_model !== 'minute_bucket') {
    return <Badge variant="default">Saldo</Badge>
  }

  return (
    <Badge variant="info">
      Bolsa ({data.bucket_type === 'control' ? 'Control' : 'Abierto'})
    </Badge>
  )
}

/**
 * Minute-bucket billing configuration for a plan. Lives on a separate
 * endpoint/table from the rest of the plan (`/plans/{id}/bucket-config`), so
 * it saves independently from the surrounding plan form instead of being
 * bundled into the same submit.
 */
function BucketConfigSection({ planId }: { planId: number }) {
  const queryClient = useQueryClient()
  const [saved, setSaved] = useState(false)

  const { data, isLoading } = useQuery({
    queryKey: ['plan-bucket-config', planId],
    queryFn: () => getPlanBucketConfig(planId),
  })

  const { data: cardGroups = ['standard'] } = useQuery({
    queryKey: ['rate-card-groups'],
    queryFn: getRateCardGroups,
  })

  const mutation = useMutation({
    mutationFn: (config: BucketConfig) => updatePlanBucketConfig(planId, config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plan-bucket-config', planId] })
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

  return (
    <BucketConfigForm
      title="Facturación: Bolsa de Minutos"
      initial={data}
      cardGroups={cardGroups}
      isSaving={mutation.isPending}
      saved={saved}
      onSave={(config) => mutation.mutate(config)}
    />
  )
}

/**
 * Shared bucket-config form used by both Plans (template) and Accounts
 * (override). Renders as a plain section (no nested <form>) so it can live
 * inside the parent modal's own <form> without invalid nested forms — the
 * save button is type="button" and calls onSave directly.
 */
export function BucketConfigForm({
  title,
  initial,
  cardGroups,
  isSaving,
  saved,
  onSave,
  onChange,
  extra,
  lockToBucket,
}: {
  title: string
  initial: BucketConfig
  cardGroups: string[]
  isSaving: boolean
  saved: boolean
  /** Immediate-save mode (edit screens): renders its own "Guardar" button. */
  onSave?: (config: BucketConfig) => void
  /**
   * Deferred mode (create screens, no id to save against yet): fires on every
   * field change so the parent can hold the config and submit it together
   * with the rest of the create form. No internal save button is rendered.
   */
  onChange?: (config: BucketConfig) => void
  extra?: React.ReactNode
  /**
   * There is no "Saldo" plan type anymore (Fibertel's model is Plan Control
   * / Plan Abierto only) — hides the billing_model dropdown entirely and
   * always renders the bucket fields, since `initial.billing_model` is
   * already forced to 'minute_bucket' by the caller.
   */
  lockToBucket?: boolean
}) {
  const [cfg, setCfg] = useState<BucketConfig>(initial)
  const updateCfg = (patch: Partial<BucketConfig>) => {
    setCfg((prev) => {
      const next = { ...prev, ...patch }
      onChange?.(next)
      return next
    })
  }
  const isBucket = lockToBucket || cfg.billing_model === 'minute_bucket'
  const usesFlatRate = cfg.overage_flat_rate_per_minute !== null && cfg.overage_flat_rate_per_minute !== undefined

  return (
    <div className="border-t border-[var(--color-border-primary)] pt-4 mt-2 space-y-4">
      <h3 className="text-sm font-bold text-[var(--color-text-primary)] flex items-center">
        <Timer className="w-4 h-4 mr-2 text-blue-600" />
        {title}
      </h3>

      {!lockToBucket && (
        <div>
          <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
            Modelo de Facturación
          </label>
          <select
            value={cfg.billing_model}
            onChange={(e) => {
              const billing_model = e.target.value as BucketConfig['billing_model']
              updateCfg({
                billing_model,
                // Default to "control" the first time so a bare model switch
                // is already valid against the backend (bucket_type is
                // required when billing_model = minute_bucket).
                bucket_type: billing_model === 'minute_bucket' ? cfg.bucket_type ?? 'control' : cfg.bucket_type,
              })
            }}
            className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
          >
            <option value="balance">Saldo (modelo actual)</option>
            <option value="minute_bucket">Bolsa de Minutos</option>
          </select>
        </div>
      )}

      {isBucket && (
        <>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              {lockToBucket ? 'Tipo de Plan' : 'Tipo de Bolsa'}
            </label>
            <div className="grid grid-cols-2 gap-3">
              <label
                className={`border rounded-lg p-3 cursor-pointer text-sm ${
                  cfg.bucket_type === 'control' ? 'border-blue-500 bg-blue-50' : 'border-[var(--color-border-primary)]'
                }`}
              >
                <input
                  type="radio"
                  name="bucket_type"
                  className="mr-2"
                  checked={cfg.bucket_type === 'control'}
                  onChange={() => updateCfg({ bucket_type: 'control' })}
                />
                <strong>Control</strong>
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Bloquea llamadas nuevas al agotar la bolsa.
                </p>
              </label>
              <label
                className={`border rounded-lg p-3 cursor-pointer text-sm ${
                  cfg.bucket_type === 'abierto' ? 'border-blue-500 bg-blue-50' : 'border-[var(--color-border-primary)]'
                }`}
              >
                <input
                  type="radio"
                  name="bucket_type"
                  className="mr-2"
                  checked={cfg.bucket_type === 'abierto'}
                  onChange={() => updateCfg({ bucket_type: 'abierto' })}
                />
                <strong>Abierto</strong>
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Nunca bloquea; el excedente se tarifica y reporta.
                </p>
              </label>
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Minutos Incluidos
            </label>
            <input
              type="number"
              min="0"
              value={cfg.included_minutes}
              onChange={(e) => updateCfg({ included_minutes: Number(e.target.value) })}
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Prorrateo del Primer Ciclo
            </label>
            <div className="flex gap-4 text-sm">
              <label className="flex items-center gap-1">
                <input
                  type="radio"
                  name="proration_mode"
                  checked={cfg.proration_mode === 'full'}
                  onChange={() => updateCfg({ proration_mode: 'full' })}
                />
                Bolsa completa desde el alta
              </label>
              <label className="flex items-center gap-1">
                <input
                  type="radio"
                  name="proration_mode"
                  checked={cfg.proration_mode === 'prorated'}
                  onChange={() => updateCfg({ proration_mode: 'prorated' })}
                />
                Prorratear por días restantes
              </label>
            </div>
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              El valor prorrateado se calcula al abrir el primer ciclo y queda editable desde el detalle de la cuenta.
            </p>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Tarifa de Excedente
            </label>
            <div className="flex gap-4 text-sm mb-2">
              <label className="flex items-center gap-1">
                <input
                  type="radio"
                  name="overage_mode"
                  checked={usesFlatRate}
                  onChange={() =>
                    updateCfg({ overage_flat_rate_per_minute: 0, overage_tariff_group: null })
                  }
                />
                Tarifa fija por minuto
              </label>
              <label className="flex items-center gap-1">
                <input
                  type="radio"
                  name="overage_mode"
                  checked={!usesFlatRate}
                  onChange={() =>
                    updateCfg({ overage_flat_rate_per_minute: null, overage_tariff_group: 'standard' })
                  }
                />
                Tarifario por destino (grupo)
              </label>
            </div>
            {usesFlatRate ? (
              <input
                type="number"
                min="0"
                step="0.0001"
                value={cfg.overage_flat_rate_per_minute ?? 0}
                onChange={(e) =>
                  updateCfg({ overage_flat_rate_per_minute: Number(e.target.value) })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                placeholder="Ej. 0.08"
              />
            ) : (
              <input
                type="text"
                list="overage-group-options"
                value={cfg.overage_tariff_group ?? 'standard'}
                onChange={(e) => updateCfg({ overage_tariff_group: e.target.value || 'standard' })}
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            )}
            <datalist id="overage-group-options">
              {cardGroups.map((g) => (
                <option key={g} value={g} />
              ))}
            </datalist>
          </div>
        </>
      )}

      {extra}

      {onSave && (
        <div className="flex items-center justify-end gap-3">
          {saved && (
            <span className="text-sm text-green-600 flex items-center">
              <Check className="w-4 h-4 mr-1" /> Guardado
            </span>
          )}
          <button
            type="button"
            disabled={isSaving}
            onClick={() => onSave(cfg)}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
          >
            {isSaving ? 'Guardando...' : 'Guardar Bolsa de Minutos'}
          </button>
        </div>
      )}
    </div>
  )
}
