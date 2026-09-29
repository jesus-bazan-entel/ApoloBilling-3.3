import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchInboundRoutes,
  createInboundRoute,
  updateInboundRoute,
  deleteInboundRoute,
  syncInboundRoutes,
  fetchRoutingTrunks,
} from '../api/client'
import type {
  InboundRoute,
  CreateInboundRouteRequest,
  UpdateInboundRouteRequestNew,
  RoutingTrunk,
} from '../types'
import DataTable from '../components/DataTable'
import {
  Phone,
  Plus,
  X,
  RefreshCw,
  CheckCircle,
  AlertCircle,
  Clock,
  Trash2,
  Edit,
  Server,
  Info,
  Search,
  Asterisk,
} from 'lucide-react'

export default function InboundRoutingPage() {
  const queryClient = useQueryClient()

  // Modal state
  const [showModal, setShowModal] = useState(false)
  const [editingRoute, setEditingRoute] = useState<InboundRoute | null>(null)

  // Search state
  const [searchTerm, setSearchTerm] = useState('')

  // Error state
  const [deleteError, setDeleteError] = useState<string | null>(null)

  // Queries
  const { data: routes = [], isLoading } = useQuery({
    queryKey: ['inbound-routes'],
    queryFn: fetchInboundRoutes,
  })

  const { data: trunks = [] } = useQuery({
    queryKey: ['routing-trunks'],
    queryFn: fetchRoutingTrunks,
  })

  // Filtered routes based on search
  const filteredRoutes = useMemo(() => {
    if (!searchTerm) return routes
    const term = searchTerm.toLowerCase()
    return routes.filter(
      (route) =>
        route.name.toLowerCase().includes(term) ||
        route.pattern.toLowerCase().includes(term) ||
        route.description?.toLowerCase().includes(term)
    )
  }, [routes, searchTerm])

  // Mutations
  const syncMutation = useMutation({
    mutationFn: syncInboundRoutes,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['inbound-routes'] })
    },
  })

  const createMutation = useMutation({
    mutationFn: createInboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['inbound-routes'] })
      setShowModal(false)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateInboundRouteRequestNew }) =>
      updateInboundRoute(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['inbound-routes'] })
      setShowModal(false)
      setEditingRoute(null)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteInboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['inbound-routes'] })
      setDeleteError(null)
    },
    onError: (error: unknown) => {
      const axiosError = error as { response?: { data?: { message?: string } }; message?: string }
      const errorMessage =
        axiosError.response?.data?.message || axiosError.message || 'Error deleting route'
      setDeleteError(errorMessage)
    },
  })

  // Table columns
  const columns = [
    {
      key: 'pattern',
      header: 'Patron',
      render: (route: InboundRoute) => (
        <div className="flex items-center gap-2">
          <code className="bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded text-sm font-mono">
            {route.pattern}
          </code>
          {route.is_wildcard && (
            <span className="flex items-center gap-1 px-2 py-0.5 text-xs bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200 rounded">
              <Asterisk className="w-3 h-3" />
              Wildcard
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'name',
      header: 'Nombre',
      render: (route: InboundRoute) => (
        <div>
          <div className="font-medium text-gray-900 dark:text-gray-100">{route.name}</div>
          {route.description && (
            <div className="text-sm text-gray-500">{route.description}</div>
          )}
        </div>
      ),
    },
    {
      key: 'destination',
      header: 'Destino',
      render: (route: InboundRoute) => (
        <div className="flex items-center gap-2">
          {route.destination_type === 'gateway' ? (
            <>
              <Server className="w-4 h-4 text-blue-500" />
              <span>{route.gateway_name || 'Gateway'}</span>
            </>
          ) : (
            <>
              <Phone className="w-4 h-4 text-green-500" />
              <span>{route.destination_host}</span>
            </>
          )}
        </div>
      ),
    },
    {
      key: 'transform',
      header: 'Transformacion',
      render: (route: InboundRoute) => {
        // Show transformation example
        const exampleInput = route.is_wildcard
          ? route.pattern.replace('*', '12345678')
          : route.pattern
        const exampleOutput =
          route.prefix_to_add + exampleInput.slice(route.strip_digits)

        return (
          <div className="text-sm">
            <div className="flex items-center gap-2">
              <span className="text-gray-500">-{route.strip_digits}</span>
              {route.prefix_to_add && (
                <span className="text-green-600">+{route.prefix_to_add}</span>
              )}
            </div>
            <div className="text-xs text-gray-400">
              {exampleInput} → {exampleOutput}
            </div>
          </div>
        )
      },
    },
    {
      key: 'options',
      header: 'Opciones',
      render: (route: InboundRoute) => (
        <div className="flex gap-2">
          {route.ims_mode && (
            <span className="px-2 py-0.5 text-xs bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200 rounded">
              IMS
            </span>
          )}
          {route.early_media && (
            <span className="px-2 py-0.5 text-xs bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 rounded">
              Early Media
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'sync_status',
      header: 'Estado',
      render: (route: InboundRoute) => (
        <div className="flex items-center gap-2">
          {route.sync_status === 'synced' ? (
            <CheckCircle className="w-4 h-4 text-green-500" />
          ) : route.sync_status === 'error' ? (
            <AlertCircle className="w-4 h-4 text-red-500" />
          ) : (
            <Clock className="w-4 h-4 text-yellow-500" />
          )}
          <span
            className={
              route.sync_status === 'synced'
                ? 'text-green-600'
                : route.sync_status === 'error'
                  ? 'text-red-600'
                  : 'text-yellow-600'
            }
          >
            {route.sync_status}
          </span>
        </div>
      ),
    },
    {
      key: 'enabled',
      header: 'Activo',
      render: (route: InboundRoute) => (
        <span
          className={`px-2 py-1 text-xs rounded-full ${
            route.enabled
              ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
              : 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300'
          }`}
        >
          {route.enabled ? 'Si' : 'No'}
        </span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (route: InboundRoute) => (
        <div className="flex gap-2">
          <button
            onClick={() => {
              setEditingRoute(route)
              setShowModal(true)
            }}
            className="p-1 text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded"
          >
            <Edit className="w-4 h-4" />
          </button>
          <button
            onClick={() => deleteMutation.mutate(route.id)}
            className="p-1 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
            Rutas Entrantes
          </h1>
          <p className="text-gray-500 dark:text-gray-400">
            Gestion de rutas para llamadas entrantes
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => syncMutation.mutate()}
            disabled={syncMutation.isPending}
            className="flex items-center gap-2 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 ${syncMutation.isPending ? 'animate-spin' : ''}`} />
            Sincronizar
          </button>
          <button
            onClick={() => {
              setEditingRoute(null)
              setShowModal(true)
            }}
            className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            <Plus className="w-4 h-4" />
            Nueva Ruta
          </button>
        </div>
      </div>

      {/* Info Banner */}
      <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-blue-500 mt-0.5" />
          <div className="text-sm text-blue-800 dark:text-blue-200">
            <strong>Auto-prioridad:</strong> Los patrones mas largos (mas especificos) se procesan
            primero. Por ejemplo, <code className="bg-blue-100 dark:bg-blue-800 px-1 rounded">672017402000</code> tiene
            mayor prioridad que <code className="bg-blue-100 dark:bg-blue-800 px-1 rounded">6720*</code>.
            No es necesario configurar prioridades manualmente.
          </div>
        </div>
      </div>

      {/* Delete Error */}
      {deleteError && (
        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-5 h-5 text-red-500" />
            <span className="text-red-800 dark:text-red-200">{deleteError}</span>
            <button
              onClick={() => setDeleteError(null)}
              className="ml-auto text-red-500 hover:text-red-700"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Routes Table */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Rutas ({filteredRoutes.length})
          </h2>
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Buscar patron o nombre..."
              className="pl-10 pr-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 text-sm w-64"
            />
          </div>
        </div>
        <DataTable
          columns={columns}
          data={filteredRoutes}
          loading={isLoading}
          emptyMessage="No hay rutas entrantes configuradas"
        />
      </div>

      {/* Route Modal */}
      {showModal && (
        <InboundRouteModal
          route={editingRoute}
          trunks={trunks}
          onClose={() => {
            setShowModal(false)
            setEditingRoute(null)
          }}
          onSave={(data) => {
            if (editingRoute) {
              updateMutation.mutate({ id: editingRoute.id, data })
            } else {
              createMutation.mutate(data as CreateInboundRouteRequest)
            }
          }}
          isLoading={createMutation.isPending || updateMutation.isPending}
        />
      )}
    </div>
  )
}

// Inbound Route Modal Component
function InboundRouteModal({
  route,
  trunks,
  onClose,
  onSave,
  isLoading,
}: {
  route: InboundRoute | null
  trunks: RoutingTrunk[]
  onClose: () => void
  onSave: (data: CreateInboundRouteRequest | UpdateInboundRouteRequestNew) => void
  isLoading: boolean
}) {
  const [formData, setFormData] = useState<CreateInboundRouteRequest>({
    name: route?.name || '',
    description: route?.description || '',
    pattern: route?.pattern || '',
    source_ip: route?.source_ip || '',
    destination_type: route?.destination_type || 'extension',
    gateway_id: route?.gateway_id || undefined,
    destination_host: route?.destination_host || '10.10.22.4',
    strip_digits: route?.strip_digits ?? 4,
    prefix_to_add: route?.prefix_to_add || '',
    call_timeout: route?.call_timeout ?? 55,
    ims_mode: route?.ims_mode ?? false,
    early_media: route?.early_media ?? false,
    enabled: route?.enabled ?? true,
  })

  // Pattern validation
  const patternError = useMemo(() => {
    if (!formData.pattern) return null
    // Valid patterns: digits only, or digits followed by *
    if (!/^\d+\*?$/.test(formData.pattern)) {
      return 'Solo digitos, opcionalmente terminando en * para wildcard'
    }
    return null
  }, [formData.pattern])

  // Transformation preview
  const transformPreview = useMemo(() => {
    if (!formData.pattern) return null
    const isWildcard = formData.pattern.endsWith('*')
    const exampleInput = isWildcard
      ? formData.pattern.replace('*', '12345678')
      : formData.pattern
    const exampleOutput = formData.prefix_to_add + exampleInput.slice(formData.strip_digits)
    return { input: exampleInput, output: exampleOutput }
  }, [formData.pattern, formData.strip_digits, formData.prefix_to_add])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (patternError) return
    onSave(formData)
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl max-h-[90vh] overflow-y-auto">
        <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            {route ? 'Editar Ruta' : 'Nueva Ruta Entrante'}
          </h3>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-6">
          {/* Basic Info */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Nombre *
              </label>
              <input
                type="text"
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                placeholder="Ej: Entel Principal"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Patron *
              </label>
              <input
                type="text"
                value={formData.pattern}
                onChange={(e) => setFormData({ ...formData, pattern: e.target.value })}
                placeholder="672017402000 o 6720*"
                className={`w-full px-3 py-2 border rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 font-mono ${
                  patternError
                    ? 'border-red-500'
                    : 'border-gray-300 dark:border-gray-600'
                }`}
                required
              />
              {patternError ? (
                <p className="text-xs text-red-500 mt-1">{patternError}</p>
              ) : (
                <p className="text-xs text-gray-500 mt-1">
                  Usa * al final para wildcard (ej: 6720* coincide con 672012345)
                </p>
              )}
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Descripcion
            </label>
            <input
              type="text"
              value={formData.description || ''}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              placeholder="Descripcion opcional"
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
            />
          </div>

          {/* Destination */}
          <div className="space-y-4">
            <h4 className="font-medium text-gray-900 dark:text-gray-100 border-b border-gray-200 dark:border-gray-700 pb-2">
              Destino
            </h4>
            <div className="flex gap-6">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="destination_type"
                  checked={formData.destination_type === 'gateway'}
                  onChange={() => setFormData({ ...formData, destination_type: 'gateway' })}
                  className="w-4 h-4 text-blue-600"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">Gateway</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="destination_type"
                  checked={formData.destination_type === 'extension'}
                  onChange={() => setFormData({ ...formData, destination_type: 'extension' })}
                  className="w-4 h-4 text-blue-600"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">Extension</span>
              </label>
            </div>
            <div className="grid grid-cols-2 gap-4">
              {formData.destination_type === 'gateway' ? (
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Gateway
                  </label>
                  <select
                    value={formData.gateway_id || ''}
                    onChange={(e) =>
                      setFormData({ ...formData, gateway_id: e.target.value || undefined })
                    }
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                  >
                    <option value="">Seleccionar gateway...</option>
                    {trunks.map((trunk) => (
                      <option key={trunk.id} value={trunk.id}>
                        {trunk.name}
                      </option>
                    ))}
                  </select>
                </div>
              ) : (
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Host Destino
                  </label>
                  <input
                    type="text"
                    value={formData.destination_host}
                    onChange={(e) => setFormData({ ...formData, destination_host: e.target.value })}
                    placeholder="10.10.22.4"
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                  />
                </div>
              )}
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  IP Origen (opcional)
                </label>
                <input
                  type="text"
                  value={formData.source_ip || ''}
                  onChange={(e) => setFormData({ ...formData, source_ip: e.target.value })}
                  placeholder="10.35.13.100"
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                />
              </div>
            </div>
          </div>

          {/* Transformation */}
          <div className="space-y-4">
            <h4 className="font-medium text-gray-900 dark:text-gray-100 border-b border-gray-200 dark:border-gray-700 pb-2">
              Transformacion de Numero
            </h4>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Quitar Digitos
                </label>
                <input
                  type="number"
                  value={formData.strip_digits}
                  onChange={(e) =>
                    setFormData({ ...formData, strip_digits: parseInt(e.target.value) || 0 })
                  }
                  min={0}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Agregar Prefijo
                </label>
                <input
                  type="text"
                  value={formData.prefix_to_add}
                  onChange={(e) => setFormData({ ...formData, prefix_to_add: e.target.value })}
                  placeholder="Ej: 9"
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                />
              </div>
            </div>
            {transformPreview && (
              <div className="bg-gray-50 dark:bg-gray-700 rounded-lg p-3 text-sm">
                <span className="text-gray-500">Resultado: </span>
                <code className="text-gray-900 dark:text-gray-100">
                  {transformPreview.input}
                </code>
                <span className="text-gray-500 mx-2">→</span>
                <code className="text-green-600 dark:text-green-400">
                  {transformPreview.output}
                </code>
              </div>
            )}
          </div>

          {/* Options */}
          <div className="space-y-4">
            <h4 className="font-medium text-gray-900 dark:text-gray-100 border-b border-gray-200 dark:border-gray-700 pb-2">
              Opciones
            </h4>
            <div className="grid grid-cols-3 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Timeout (seg)
                </label>
                <input
                  type="number"
                  value={formData.call_timeout}
                  onChange={(e) =>
                    setFormData({ ...formData, call_timeout: parseInt(e.target.value) || 55 })
                  }
                  min={1}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                />
              </div>
            </div>
            <div className="flex gap-6">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.ims_mode}
                  onChange={(e) => setFormData({ ...formData, ims_mode: e.target.checked })}
                  className="w-4 h-4 text-blue-600 rounded"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">
                  Modo IMS (100rel/PRACK)
                </span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.early_media}
                  onChange={(e) => setFormData({ ...formData, early_media: e.target.checked })}
                  className="w-4 h-4 text-blue-600 rounded"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">Early Media</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.enabled}
                  onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
                  className="w-4 h-4 text-blue-600 rounded"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">Habilitado</span>
              </label>
            </div>
          </div>

          {/* Actions */}
          <div className="flex justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={isLoading || !!patternError}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : route ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
