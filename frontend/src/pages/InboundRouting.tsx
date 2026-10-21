import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchInboundRoutes,
  createInboundRoute,
  updateInboundRoute,
  deleteInboundRoute,
  syncInboundRoutes,
  fetchInboundDestinations,
  fetchSipEndpoints,
  fetchSettings,
} from '../api/client'
import type {
  InboundRoute,
  InboundGatewayOption,
  CreateInboundRouteRequest,
  UpdateInboundRouteRequestNew,
} from '../types'
import type { SipEndpoint } from '../types'
import type { SystemSetting } from '../api/client'
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
  Search,
  Asterisk,
  ArrowRight,
  Layers,
  Network,
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

  // Destination trunks (PBX) for the route destination dropdown:
  // SIP devices with endpoint_type = 'trunk'
  const { data: destinations = [] } = useQuery({
    queryKey: ['inbound-destinations'],
    queryFn: fetchInboundDestinations,
  })

  // ETAPA 2 data: PBX trunks with their owned DID ranges + system settings
  const { data: endpoints = [] } = useQuery({
    queryKey: ['sip-endpoints'],
    queryFn: () => fetchSipEndpoints(),
  })
  const { data: settings = [] } = useQuery({
    queryKey: ['system-settings'],
    queryFn: fetchSettings,
  })
  const settingValue = (key: string, fallback: string) =>
    settings.find((s: SystemSetting) => s.key === key)?.value ?? fallback

  // ETAPA 1 split: exact overrides vs RN classifiers (wildcards)
  const exactRoutes = useMemo(() => routes.filter((r) => !r.is_wildcard), [routes])
  const classifierRoutes = useMemo(() => routes.filter((r) => r.is_wildcard), [routes])

  // ETAPA 2: owned ranges per PBX trunk (longest-match order)
  const pbxRanges = useMemo(() => {
    const out: { pbx: string; host: string; port: number; prefix: string }[] = []
    for (const ep of endpoints as SipEndpoint[]) {
      if (ep.endpoint_type !== 'trunk' || ep.auth_mode !== 'ip' || !ep.enabled) continue
      for (const p of (ep.did_ranges || '').split(',').map((s) => s.trim()).filter(Boolean)) {
        out.push({ pbx: ep.name, host: ep.host || '', port: ep.port, prefix: p })
      }
    }
    out.sort((a, b) => b.prefix.length - a.prefix.length || a.prefix.localeCompare(b.prefix))
    return out
  }, [endpoints])

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
  // `className` goes to both <th> and <td>: fixed-width columns shrink to
  // their content while `name` wraps and absorbs the available width, so the
  // table always fits the viewport. Secondary columns are hidden on small
  // screens to keep the name column readable.
  const columns = [
    {
      key: 'pattern',
      header: 'Patron',
      className: 'w-px whitespace-nowrap align-top',
      render: (route: InboundRoute) => (
        <div className="flex items-center gap-2">
          <code className="bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded text-sm font-mono">
            {route.pattern}
          </code>
          {route.is_wildcard ? (
            <span
              title="ETAPA 1: clasifica por RN, normaliza y transfiere a national-routing. No puentea."
              className="flex items-center gap-1 px-2 py-0.5 text-xs bg-violet-100 text-violet-800 dark:bg-violet-900 dark:text-violet-200 rounded"
            >
              <Asterisk className="w-3 h-3" />
              Clasificador RN · E1
            </span>
          ) : (
            <span
              title="Override con destino fijo: puentea directo sin pasar por ETAPA 2"
              className="px-2 py-0.5 text-xs bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 rounded"
            >
              Exacta · override
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'name',
      header: 'Nombre',
      className: 'w-auto min-w-0 whitespace-normal break-words align-top',
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
      className: 'w-auto min-w-0 max-w-[16rem] whitespace-normal break-words align-top',
      render: (route: InboundRoute) => (
        <div className="flex items-center gap-2">
          {route.is_wildcard ? (
            <span
              title="Los clasificadores no puentean: transfieren el numero nacional a la ETAPA 2"
              className="text-xs text-violet-700 dark:text-violet-300 italic"
            >
              → national-routing (transfiere, no puentea)
            </span>
          ) : route.destination_type === 'gateway' ? (
            <>
              <Server className="w-4 h-4 text-blue-500" />
              <span>
                {route.gateway_name || 'Sin troncal'}
                {route.gateway_host && (
                  <span className="text-xs text-gray-400 ml-1">
                    {route.gateway_host}
                    {route.gateway_port ? `:${route.gateway_port}` : ''}
                  </span>
                )}
              </span>
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
      className: 'hidden md:table-cell w-px whitespace-nowrap align-top',
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
      className: 'hidden lg:table-cell w-px whitespace-nowrap align-top',
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
      className: 'hidden sm:table-cell w-px whitespace-nowrap align-top',
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
      className: 'w-px whitespace-nowrap align-top',
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
      className: 'w-px whitespace-nowrap align-top',
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

      {/* Two-stage flow banner */}
      <div className="bg-violet-50 dark:bg-violet-900/20 border border-violet-200 dark:border-violet-800 rounded-lg p-4">
        <div className="flex items-center gap-2 flex-wrap text-sm">
          <span className="flex items-center gap-1.5 px-2.5 py-1 bg-violet-600 text-white rounded-full text-xs font-medium">
            <Layers className="w-3.5 h-3.5" />
            ETAPA 1 · Clasificar
          </span>
          <span className="text-gray-600 dark:text-gray-300 text-xs">
            {exactRoutes.length} exacta{exactRoutes.length === 1 ? '' : 's'} (override directo)
            {' + '}
            {classifierRoutes.length} clasificador{classifierRoutes.length === 1 ? '' : 'es'} RN
            (normalizan a nacional)
          </span>
          <ArrowRight className="w-4 h-4 text-violet-500" />
          <span className="flex items-center gap-1.5 px-2.5 py-1 bg-green-600 text-white rounded-full text-xs font-medium">
            <Network className="w-3.5 h-3.5" />
            ETAPA 2 · Decidir
          </span>
          <span className="text-gray-600 dark:text-gray-300 text-xs">
            Ext. registrada → {pbxRanges.length} rango{pbxRanges.length === 1 ? '' : 's'} PBX →{' '}
            {settingValue('inbound.default_action', '480')}
          </span>
        </div>
        <div className="text-xs text-violet-800 dark:text-violet-200 mt-2">
          Un DID nuevo no necesita fila: cae en el clasificador de su RN y la ETAPA 2 lo
          entrega a la extensión registrada o a la PBX dueña del rango. Solo los servicios
          especiales llevan ruta exacta.
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

      {/* ETAPA 2 panel: owned ranges per PBX (from Dispositivos SIP) */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            ETAPA 2 · Rangos por PBX ({pbxRanges.length})
          </h2>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
            Se configuran en la ficha de cada troncal IP (Dispositivos SIP → Rangos DID propios).
            Cada prefijo se enruta SOLO a su PBX (longest-match). Sin dueño →{' '}
            {settingValue('inbound.default_action', '480')}.
          </p>
        </div>
        <div className="p-4">
          {pbxRanges.length === 0 ? (
            <p className="text-sm text-gray-500">
              Ninguna PBX tiene rangos asignados: lo no cubierto por L1/L2 responde{' '}
              {settingValue('inbound.default_action', '480')}.
            </p>
          ) : (
            <div className="flex flex-col gap-2">
              {pbxRanges.map((r) => (
                <div
                  key={`${r.pbx}-${r.prefix}`}
                  className="flex items-center gap-3 text-sm border border-gray-100 dark:border-gray-700 rounded-lg px-3 py-2"
                >
                  <code className="bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded font-mono">
                    {r.prefix}*
                  </code>
                  <ArrowRight className="w-4 h-4 text-gray-400" />
                  <Server className="w-4 h-4 text-blue-500" />
                  <span className="font-medium">{r.pbx}</span>
                  <span className="text-xs text-gray-400 font-mono">
                    {r.host}:{r.port}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Route Modal */}
      {showModal && (
        <InboundRouteModal
          route={editingRoute}
          destinations={destinations}
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
  destinations,
  onClose,
  onSave,
  isLoading,
}: {
  route: InboundRoute | null
  destinations: InboundGatewayOption[]
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
    sip_endpoint_id: route?.sip_endpoint_id || null,
    destination_host: route?.destination_host || '10.10.22.4',
    strip_digits: route?.strip_digits ?? 4,
    prefix_to_add: route?.prefix_to_add || '',
    call_timeout: route?.call_timeout ?? 55,
    ims_mode: route?.ims_mode ?? false,
    early_media: route?.early_media ?? false,
    enabled: route?.enabled ?? true,
  })

  // The select encodes the reference type: `se:` = sip_endpoints trunk (PBX).
  const destinationValue = formData.sip_endpoint_id
    ? `se:${formData.sip_endpoint_id}`
    : ''

  const handleDestinationChange = (value: string) => {
    setFormData({
      ...formData,
      sip_endpoint_id: value.startsWith('se:') ? value.slice(3) : null,
    })
  }

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
              ) : formData.pattern.endsWith('*') ? (
                <p className="text-xs text-violet-600 mt-1">
                  Clasificador RN (ETAPA 1): normaliza a nacional y transfiere a national-routing.
                  No puentea: el destino de abajo se ignora.
                </p>
              ) : (
                <p className="text-xs text-gray-500 mt-1">
                  Ruta exacta (override): puentea directo sin pasar por ETAPA 2. Úsala solo para
                  servicios especiales.
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
              {formData.pattern.endsWith('*') && (
                <span className="ml-2 text-xs font-normal text-violet-600">
                  (ignorado en clasificadores: ver nota del patrón)
                </span>
              )}
            </h4>
            {!formData.pattern.endsWith('*') &&
              formData.destination_type === 'gateway' &&
              !formData.sip_endpoint_id &&
              (!formData.destination_host ||
                ['10.10.22.4', '127.0.0.1', 'localhost'].includes(
                  formData.destination_host.trim()
                )) && (
                <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg flex items-start gap-2">
                  <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0" />
                  <p className="text-xs text-red-800 dark:text-red-200">
                    <strong>Riesgo de hairpin (500 No rules):</strong> una troncal sin PBX
                    seleccionada contra el propio SBC reinyecta la llamada a Kamailio. Elige una
                    troncal (PBX) en el desplegable o usa tipo Extensión.
                  </p>
                </div>
              )}
            <div className="flex gap-6">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="destination_type"
                  checked={formData.destination_type === 'gateway'}
                  onChange={() => setFormData({ ...formData, destination_type: 'gateway' })}
                  className="w-4 h-4 text-blue-600"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">
                  Troncal (PBX)
                </span>
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
                    Troncal (PBX)
                  </label>
                  <select
                    value={destinationValue}
                    onChange={(e) => handleDestinationChange(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100"
                  >
                    <option value="">Seleccionar troncal...</option>
                    {destinations.map((dest) => (
                      <option key={`se:${dest.id}`} value={`se:${dest.id}`}>
                        {dest.name} ({dest.host}:{dest.port})
                      </option>
                    ))}
                  </select>
                  {destinationValue.startsWith('se:') &&
                    (() => {
                      const dest = destinations.find(
                        (d) => `se:${d.id}` === destinationValue
                      )
                      return dest ? (
                        <p className="text-xs text-gray-500 mt-1">
                          Puente: sofia/{dest.fs_profile}/$&#123;numero&#125;@{dest.host}:
                          {dest.port}
                        </p>
                      ) : null
                    })()}
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
                    setFormData({ ...formData, strip_digits: e.target.value === '' ? 0 : parseInt(e.target.value) })
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
                    setFormData({ ...formData, call_timeout: e.target.value === '' ? 0 : parseInt(e.target.value) })
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
