import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchPlans, createPlan, updatePlan, deletePlan } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { FileText, Plus, Edit, X, Trash2, AlertCircle } from 'lucide-react'
import type { Plan, AccountType } from '../types'

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
    mutationFn: createPlan,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
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
            <span className="text-xs text-slate-500 ml-1">
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
            className="p-1 hover:bg-slate-100 rounded"
            title="Editar"
          >
            <Edit className="w-4 h-4 text-slate-600" />
          </button>
          <button
            onClick={() => setDeletingPlan(plan)}
            className="p-1 hover:bg-slate-100 rounded"
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
          <h1 className="text-2xl font-bold text-slate-900 flex items-center">
            <FileText className="w-8 h-8 mr-3 text-blue-600" />
            Planes
          </h1>
          <p className="text-slate-600 mt-1">
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
          onSave={(data) => {
            if (editingPlan) {
              updateMutation.mutate({ id: editingPlan.id, data })
            } else {
              createMutation.mutate(data)
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
  onSave: (data: Partial<Plan>) => void
}) {
  const [formData, setFormData] = useState({
    plan_name: plan?.plan_name || '',
    plan_code: plan?.plan_code || '',
    account_type: plan?.account_type || 'PREPAID',
    initial_balance: plan?.initial_balance || 10,
    credit_limit: plan?.credit_limit || 0,
    max_concurrent_calls: plan?.max_concurrent_calls || 5,
    description: plan?.description || '',
    enabled: plan?.enabled ?? true,
  })

  const isPrepaid = formData.account_type.toUpperCase() === 'PREPAID'

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave(formData)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold">
            {plan ? 'Editar Plan' : 'Nuevo Plan'}
          </h2>
          <button onClick={onClose} className="p-1 hover:bg-slate-100 rounded">
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
              <label className="block text-sm font-medium text-slate-700 mb-1">
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
              <label className="block text-sm font-medium text-slate-700 mb-1">
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
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-slate-100"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Tipo de Cuenta *
            </label>
            <select
              required
              disabled={!!plan}
              value={formData.account_type}
              onChange={(e) =>
                setFormData({ ...formData, account_type: e.target.value as AccountType })
              }
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-slate-100"
            >
              <option value="PREPAID">Prepago</option>
              <option value="POSTPAID">Postpago</option>
            </select>
          </div>

          {isPrepaid ? (
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
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
              <p className="text-xs text-slate-500 mt-1">
                El saldo inicial no puede ser cero (sin planes ilimitados)
              </p>
            </div>
          ) : (
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
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
              <p className="text-xs text-slate-500 mt-1">
                El límite de crédito no puede ser cero (sin planes ilimitados)
              </p>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
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
            <label className="block text-sm font-medium text-slate-700 mb-1">
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
            <label className="ml-2 text-sm font-medium text-slate-700">
              Plan activo
            </label>
          </div>

          <div className="flex justify-end space-x-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border rounded-lg hover:bg-slate-50"
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
      <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-bold mb-2">Eliminar Plan</h3>
        <p className="text-slate-600 mb-4">
          ¿Estás seguro de eliminar el plan <strong>{plan.plan_name}</strong>?
        </p>
        <p className="text-sm text-slate-500 mb-4">
          Esta acción no se puede deshacer. Las cuentas existentes creadas con
          este plan no se verán afectadas.
        </p>
        <div className="flex justify-end space-x-3">
          <button
            onClick={onClose}
            className="px-4 py-2 border rounded-lg hover:bg-slate-50"
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
