import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchDialplanRoutes, createDialplanRoute, updateDialplanRoute, deleteDialplanRoute, reloadDialplan } from '../api/client'
import DataTable from '../components/DataTable'
import { Network, Plus, X, Pencil, Trash2, RefreshCw } from 'lucide-react'
import type { DialplanRoute, BridgeDestination } from '../types'

export default function DialplanPage() {
  const [activeTab, setActiveTab] = useState<'from_pbx' | 'to_kamailio'>('from_pbx')
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [editingRoute, setEditingRoute] = useState<DialplanRoute | null>(null)
  const [deletingRoute, setDeletingRoute] = useState<DialplanRoute | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [successMessage, setSuccessMessage] = useState<string | null>(null)
  const queryClient = useQueryClient()

  const context = activeTab === 'from_pbx' ? 'from_pbx' : 'to_kamailio'

  const { data: routes = [], isLoading } = useQuery({
    queryKey: ['dialplan', context],
    queryFn: () => fetchDialplanRoutes(context),
    refetchInterval: 30000,
  })

  const createMutation = useMutation({
    mutationFn: (route: Partial<DialplanRoute>) => createDialplanRoute(context, route),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dialplan', context] })
      setShowCreateModal(false)
      setError(null)
      setSuccessMessage('Ruta creada exitosamente')
      setTimeout(() => setSuccessMessage(null), 3000)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al crear la ruta'
      setError(message)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<DialplanRoute> }) =>
      updateDialplanRoute(context, id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dialplan', context] })
      setEditingRoute(null)
      setError(null)
      setSuccessMessage('Ruta actualizada exitosamente')
      setTimeout(() => setSuccessMessage(null), 3000)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al actualizar la ruta'
      setError(message)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteDialplanRoute(context, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dialplan', context] })
      setDeletingRoute(null)
      setSuccessMessage('Ruta eliminada exitosamente')
      setTimeout(() => setSuccessMessage(null), 3000)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al eliminar la ruta'
      setError(message)
    },
  })

  const reloadMutation = useMutation({
    mutationFn: reloadDialplan,
    onSuccess: (data) => {
      setSuccessMessage(data.message || 'Dialplan recargado exitosamente')
      setTimeout(() => setSuccessMessage(null), 3000)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al recargar dialplan'
      setError(message)
      setTimeout(() => setError(null), 3000)
    },
  })

  const columns = [
    {
      key: 'priority',
      header: 'Prioridad',
      render: (route: DialplanRoute) => (
        <span className="font-semibold text-slate-900">{route.priority}</span>
      ),
    },
    {
      key: 'name',
      header: 'Nombre',
      render: (route: DialplanRoute) => (
        <span className="font-medium text-slate-900">{route.name}</span>
      ),
    },
    {
      key: 'prefix_pattern',
      header: 'Patrón',
      render: (route: DialplanRoute) => (
        <span className="font-mono text-sm text-slate-600">{route.prefix_pattern}</span>
      ),
    },
    {
      key: 'destination_ip',
      header: 'Destino (Bridge)',
      render: (route: DialplanRoute) => (
        <div className="space-y-0.5">
          <span className="font-mono text-sm text-slate-800">
            {route.destination_ip}:{route.destination_port}
          </span>
          {route.failover_destinations?.length > 0 && (
            <div className="flex flex-col gap-0.5">
              {route.failover_destinations.map((fo, i) => (
                <span key={i} className="font-mono text-xs text-amber-600">
                  &#x2BA1; {fo.ip}:{fo.port}
                </span>
              ))}
            </div>
          )}
        </div>
      ),
    },
    {
      key: 'sip_profile',
      header: 'Perfil SIP',
      render: (route: DialplanRoute) => (
        <span className="px-2 py-1 text-xs rounded-full bg-blue-100 text-blue-800">
          {route.sip_profile}
        </span>
      ),
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (route: DialplanRoute) => (
        <span className={`px-2 py-1 text-xs rounded-full ${
          route.enabled ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
        }`}>
          {route.enabled ? 'Activa' : 'Inactiva'}
        </span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (route: DialplanRoute) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setEditingRoute(route)}
            className="p-1.5 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
            title="Editar"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <button
            onClick={() => setDeletingRoute(route)}
            className="p-1.5 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
            title="Eliminar"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">
            Configuración Dialplan FreeSWITCH
          </h1>
          <p className="text-slate-500">
            Gestiona rutas de llamadas entrantes y salientes
          </p>
        </div>
        <button
          onClick={() => reloadMutation.mutate()}
          disabled={reloadMutation.isPending}
          className="flex items-center px-4 py-2 bg-amber-500 text-white rounded-lg hover:bg-amber-600 transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-5 h-5 mr-2 ${reloadMutation.isPending ? 'animate-spin' : ''}`} />
          Recargar XML
        </button>
      </div>

      {/* Success Message */}
      {successMessage && (
        <div className="p-3 bg-green-50 border border-green-200 rounded-lg text-green-700 text-sm">
          {successMessage}
        </div>
      )}

      {/* Tabs */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200">
        <div className="border-b border-slate-200">
          <nav className="flex -mb-px">
            <button
              onClick={() => setActiveTab('from_pbx')}
              className={`px-6 py-3 text-sm font-medium transition-colors ${
                activeTab === 'from_pbx'
                  ? 'border-b-2 border-blue-500 text-blue-600'
                  : 'text-slate-500 hover:text-slate-700'
              }`}
            >
              Rutas Salientes (from-pbx)
            </button>
            <button
              onClick={() => setActiveTab('to_kamailio')}
              className={`px-6 py-3 text-sm font-medium transition-colors ${
                activeTab === 'to_kamailio'
                  ? 'border-b-2 border-blue-500 text-blue-600'
                  : 'text-slate-500 hover:text-slate-700'
              }`}
            >
              Rutas Entrantes (to-kamailio)
            </button>
          </nav>
        </div>

        <div className="p-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center">
              <Network className="w-8 h-8 text-blue-500 mr-4" />
              <div>
                <p className="text-sm text-slate-500">Total de Rutas</p>
                <p className="text-3xl font-bold text-slate-900">{routes.length}</p>
              </div>
            </div>
            <button
              onClick={() => {
                setShowCreateModal(true)
                setError(null)
              }}
              className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              <Plus className="w-5 h-5 mr-2" />
              Nueva Ruta
            </button>
          </div>

          <DataTable
            columns={columns}
            data={routes}
            loading={isLoading}
            emptyMessage="No hay rutas configuradas"
            searchable={true}
            searchPlaceholder="Buscar por nombre, IP o patrón..."
          />
        </div>
      </div>

      {/* Create Modal */}
      {showCreateModal && (
        <RouteFormModal
          title="Nueva Ruta de Dialplan"
          context={activeTab}
          onClose={() => {
            setShowCreateModal(false)
            setError(null)
          }}
          onSubmit={(data) => createMutation.mutate(data)}
          isLoading={createMutation.isPending}
          error={error}
        />
      )}

      {/* Edit Modal */}
      {editingRoute && (
        <RouteFormModal
          title="Editar Ruta"
          route={editingRoute}
          context={activeTab}
          onClose={() => {
            setEditingRoute(null)
            setError(null)
          }}
          onSubmit={(data) => updateMutation.mutate({ id: editingRoute.id, data })}
          isLoading={updateMutation.isPending}
          error={error}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deletingRoute && (
        <DeleteConfirmModal
          title="Eliminar Ruta"
          message={`¿Estás seguro de que deseas eliminar la ruta "${deletingRoute.name}"? Esta acción no se puede deshacer.`}
          onClose={() => {
            setDeletingRoute(null)
            setError(null)
          }}
          onConfirm={() => deleteMutation.mutate(deletingRoute.id)}
          isLoading={deleteMutation.isPending}
          error={error}
        />
      )}
    </div>
  )
}

interface RouteFormModalProps {
  title: string
  route?: DialplanRoute
  context: 'from_pbx' | 'to_kamailio'
  onClose: () => void
  onSubmit: (data: Partial<DialplanRoute>) => void
  isLoading: boolean
  error: string | null
}

function RouteFormModal({ title, route, context, onClose, onSubmit, isLoading, error }: RouteFormModalProps) {
  const [formData, setFormData] = useState<Partial<DialplanRoute>>({
    name: route?.name || '',
    priority: route?.priority || 100,
    prefix_pattern: route?.prefix_pattern || '',
    destination_ip: route?.destination_ip || '',
    destination_port: route?.destination_port || 5060,
    sip_profile: route?.sip_profile || (context === 'from_pbx' ? 'external' : 'internal'),
    bypass_media: route?.bypass_media || false,
    inherit_codec: route?.inherit_codec || false,
    enable_100rel: route?.enable_100rel || false,
    ignore_early_media: route?.ignore_early_media || false,
    call_timeout: route?.call_timeout,
    source_ip_filter: route?.source_ip_filter || '',
    failover_destinations: route?.failover_destinations || [],
    enabled: route?.enabled !== false,
  })

  const addFailover = () => {
    setFormData({
      ...formData,
      failover_destinations: [...(formData.failover_destinations || []), { ip: '', port: 5060 }],
    })
  }

  const removeFailover = (index: number) => {
    const updated = [...(formData.failover_destinations || [])]
    updated.splice(index, 1)
    setFormData({ ...formData, failover_destinations: updated })
  }

  const updateFailover = (index: number, field: keyof BridgeDestination, value: string | number) => {
    const updated = [...(formData.failover_destinations || [])]
    updated[index] = { ...updated[index], [field]: value }
    setFormData({ ...formData, failover_destinations: updated })
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-slate-200">
          <h2 className="text-xl font-bold text-slate-900">{title}</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {error && (
            <div className="p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
              {error}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Nombre de Ruta *
            </label>
            <input
              type="text"
              required
              value={formData.name}
              onChange={(e) =>
                setFormData({ ...formData, name: e.target.value })
              }
              placeholder="Ej: Ruta Principal, Backup Carrier"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Prioridad *
              </label>
              <input
                type="number"
                required
                value={formData.priority}
                onChange={(e) =>
                  setFormData({ ...formData, priority: parseInt(e.target.value) || 100 })
                }
                placeholder="100"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Patrón de Destino *
              </label>
              <input
                type="text"
                required
                value={formData.prefix_pattern}
                onChange={(e) =>
                  setFormData({ ...formData, prefix_pattern: e.target.value })
                }
                placeholder="^(.+)$"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm"
              />
            </div>
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div className="col-span-2">
              <label className="block text-sm font-medium text-slate-700 mb-1">
                IP Destino *
              </label>
              <input
                type="text"
                required
                value={formData.destination_ip}
                onChange={(e) =>
                  setFormData({ ...formData, destination_ip: e.target.value })
                }
                placeholder="192.168.1.100"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Puerto *
              </label>
              <input
                type="number"
                required
                value={formData.destination_port}
                onChange={(e) =>
                  setFormData({ ...formData, destination_port: parseInt(e.target.value) || 5060 })
                }
                placeholder="5060"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
          </div>

          {/* Failover Destinations */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="block text-sm font-medium text-slate-700">
                Destinos Failover (pipe)
              </label>
              <button
                type="button"
                onClick={addFailover}
                className="flex items-center text-xs px-2 py-1 bg-amber-50 text-amber-700 border border-amber-200 rounded hover:bg-amber-100 transition-colors"
              >
                <Plus className="w-3 h-3 mr-1" />
                Agregar Failover
              </button>
            </div>
            {formData.failover_destinations && formData.failover_destinations.length > 0 ? (
              <div className="space-y-2">
                {formData.failover_destinations.map((fo, idx) => (
                  <div key={idx} className="flex items-center gap-2 p-2 bg-amber-50 border border-amber-200 rounded-lg">
                    <span className="text-xs text-amber-600 font-medium whitespace-nowrap">#{idx + 1}</span>
                    <input
                      type="text"
                      required
                      value={fo.ip}
                      onChange={(e) => updateFailover(idx, 'ip', e.target.value)}
                      placeholder="IP destino failover"
                      className="flex-1 px-2 py-1.5 border border-slate-300 rounded focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm"
                    />
                    <input
                      type="number"
                      required
                      value={fo.port}
                      onChange={(e) => updateFailover(idx, 'port', parseInt(e.target.value) || 5060)}
                      placeholder="5060"
                      className="w-24 px-2 py-1.5 border border-slate-300 rounded focus:ring-2 focus:ring-blue-500 focus:border-blue-500 text-sm"
                    />
                    <button
                      type="button"
                      onClick={() => removeFailover(idx)}
                      className="p-1 text-red-500 hover:bg-red-50 rounded transition-colors"
                      title="Eliminar"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-slate-400 italic">Sin failover — solo destino principal</p>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Perfil SIP *
            </label>
            <select
              value={formData.sip_profile}
              onChange={(e) =>
                setFormData({ ...formData, sip_profile: e.target.value })
              }
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            >
              <option value="internal">internal</option>
              <option value="external">external</option>
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="flex items-center space-x-4">
              <label className="flex items-center">
                <input
                  type="checkbox"
                  checked={formData.bypass_media}
                  onChange={(e) =>
                    setFormData({ ...formData, bypass_media: e.target.checked })
                  }
                  className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
                />
                <span className="ml-2 text-sm text-slate-700">Bypass Media</span>
              </label>

              <label className="flex items-center">
                <input
                  type="checkbox"
                  checked={formData.inherit_codec}
                  onChange={(e) =>
                    setFormData({ ...formData, inherit_codec: e.target.checked })
                  }
                  className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
                />
                <span className="ml-2 text-sm text-slate-700">Heredar Codec</span>
              </label>
            </div>

            <div className="flex items-center space-x-4">
              <label className="flex items-center">
                <input
                  type="checkbox"
                  checked={formData.enable_100rel}
                  onChange={(e) =>
                    setFormData({ ...formData, enable_100rel: e.target.checked })
                  }
                  className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
                />
                <span className="ml-2 text-sm text-slate-700">100rel</span>
              </label>

              <label className="flex items-center">
                <input
                  type="checkbox"
                  checked={formData.ignore_early_media}
                  onChange={(e) =>
                    setFormData({ ...formData, ignore_early_media: e.target.checked })
                  }
                  className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
                />
                <span className="ml-2 text-sm text-slate-700">Ignorar Early Media</span>
              </label>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Timeout (segundos)
              </label>
              <input
                type="number"
                value={formData.call_timeout || ''}
                onChange={(e) =>
                  setFormData({ ...formData, call_timeout: e.target.value ? parseInt(e.target.value) : undefined })
                }
                placeholder="120"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>

            {context === 'to_kamailio' && (
              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">
                  Filtro IP Origen
                </label>
                <input
                  type="text"
                  value={formData.source_ip_filter || ''}
                  onChange={(e) =>
                    setFormData({ ...formData, source_ip_filter: e.target.value })
                  }
                  placeholder="^10\.10\.22\.18$"
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono text-sm"
                />
              </div>
            )}
          </div>

          <div className="flex items-center">
            <input
              type="checkbox"
              id="enabled"
              checked={formData.enabled !== false}
              onChange={(e) =>
                setFormData({ ...formData, enabled: e.target.checked })
              }
              className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
            />
            <label htmlFor="enabled" className="ml-2 text-sm text-slate-700">
              Ruta habilitada
            </label>
          </div>

          <div className="flex justify-end space-x-3 pt-4 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={isLoading}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : 'Guardar'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

interface DeleteConfirmModalProps {
  title: string
  message: string
  onClose: () => void
  onConfirm: () => void
  isLoading: boolean
  error: string | null
}

function DeleteConfirmModal({ title, message, onClose, onConfirm, isLoading, error }: DeleteConfirmModalProps) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-slate-200">
          <h2 className="text-xl font-bold text-slate-900">{title}</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="p-6">
          {error && (
            <div className="p-3 mb-4 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
              {error}
            </div>
          )}

          <p className="text-slate-600">{message}</p>

          <div className="flex justify-end space-x-3 mt-6">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="button"
              onClick={onConfirm}
              disabled={isLoading}
              className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50"
            >
              {isLoading ? 'Eliminando...' : 'Eliminar'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
