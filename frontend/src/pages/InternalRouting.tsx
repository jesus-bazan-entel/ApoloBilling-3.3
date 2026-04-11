import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import api from '../api/client'
import {
  ArrowRightLeft,
  Server,
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  ArrowRight,
  CheckCircle,
  XCircle,
  Clock,
  X,
} from 'lucide-react'

// Types
interface SystemEndpoint {
  id: string
  name: string
  endpoint_type: 'freeswitch' | 'kamailio'
  description?: string
  ip_address: string
  port: number
  transport: string
  fs_profile?: string
  fs_context?: string
  kam_gwid?: number
  kam_gw_type?: number
  enabled: boolean
  is_primary: boolean
  created_at: string
  updated_at: string
}

interface InternalRoute {
  id: string
  name: string
  description?: string
  route_type: 'fs_to_kamailio' | 'kamailio_to_fs'
  source_endpoint_id?: string
  source_name?: string
  source_type?: string
  source_ip?: string
  source_port?: number
  dest_endpoint_id?: string
  dest_name?: string
  dest_type?: string
  dest_ip?: string
  dest_port?: number
  bypass_media: boolean
  inherit_codec: boolean
  enable_100rel: boolean
  call_timeout: number
  prefix_pattern?: string
  sync_status: 'pending' | 'synced' | 'error'
  sync_error?: string
  last_sync?: string
  priority: number
  enabled: boolean
  created_at: string
  updated_at: string
}

// API functions
const fetchEndpoints = async (): Promise<SystemEndpoint[]> => {
  const response = await api.get('/internal-routing/endpoints')
  return response.data.data || []
}

const fetchRoutes = async (): Promise<InternalRoute[]> => {
  const response = await api.get('/internal-routing/routes')
  return response.data.data || []
}

const createEndpoint = async (data: Partial<SystemEndpoint>) => {
  const response = await api.post('/internal-routing/endpoints', data)
  return response.data
}

const updateEndpoint = async ({ id, ...data }: Partial<SystemEndpoint> & { id: string }) => {
  const response = await api.put(`/internal-routing/endpoints/${id}`, data)
  return response.data
}

const deleteEndpoint = async (id: string) => {
  const response = await api.delete(`/internal-routing/endpoints/${id}`)
  return response.data
}

const createRoute = async (data: Partial<InternalRoute>) => {
  const response = await api.post('/internal-routing/routes', data)
  return response.data
}

const updateRoute = async ({ id, ...data }: Partial<InternalRoute> & { id: string }) => {
  const response = await api.put(`/internal-routing/routes/${id}`, data)
  return response.data
}

const deleteRoute = async (id: string) => {
  const response = await api.delete(`/internal-routing/routes/${id}`)
  return response.data
}

const syncRoutes = async () => {
  const response = await api.post('/internal-routing/sync')
  return response.data
}

// Sync status badge
function SyncBadge({ status }: { status: string }) {
  const styles: Record<string, string> = {
    synced: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
    error: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
    pending: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
  }

  const icons: Record<string, React.ReactNode> = {
    synced: <CheckCircle className="h-3 w-3" />,
    error: <XCircle className="h-3 w-3" />,
    pending: <Clock className="h-3 w-3" />,
  }

  const labels: Record<string, string> = {
    synced: 'Sincronizado',
    error: 'Error',
    pending: 'Pendiente',
  }

  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs font-medium ${styles[status] || styles.pending}`}>
      {icons[status] || icons.pending}
      {labels[status] || 'Pendiente'}
    </span>
  )
}

export default function InternalRouting() {
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<'routes' | 'endpoints'>('routes')
  const [showEndpointModal, setShowEndpointModal] = useState(false)
  const [showRouteModal, setShowRouteModal] = useState(false)
  const [editingEndpoint, setEditingEndpoint] = useState<SystemEndpoint | null>(null)
  const [editingRoute, setEditingRoute] = useState<InternalRoute | null>(null)

  // Queries
  const { data: endpoints = [], isLoading: loadingEndpoints } = useQuery({
    queryKey: ['system-endpoints'],
    queryFn: fetchEndpoints,
  })

  const { data: routes = [], isLoading: loadingRoutes } = useQuery({
    queryKey: ['internal-routes'],
    queryFn: fetchRoutes,
  })

  // Mutations
  const syncMutation = useMutation({
    mutationFn: syncRoutes,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['internal-routes'] })
    },
  })

  const deleteEndpointMutation = useMutation({
    mutationFn: deleteEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['system-endpoints'] })
    },
  })

  const deleteRouteMutation = useMutation({
    mutationFn: deleteRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['internal-routes'] })
    },
  })

  const handleEditEndpoint = (endpoint: SystemEndpoint) => {
    setEditingEndpoint(endpoint)
    setShowEndpointModal(true)
  }

  const handleEditRoute = (route: InternalRoute) => {
    setEditingRoute(route)
    setShowRouteModal(true)
  }

  const handleDeleteEndpoint = (id: string) => {
    if (confirm('¿Eliminar este endpoint?')) {
      deleteEndpointMutation.mutate(id)
    }
  }

  const handleDeleteRoute = (id: string) => {
    if (confirm('¿Eliminar esta ruta?')) {
      deleteRouteMutation.mutate(id)
    }
  }

  const fsEndpoints = endpoints.filter((e) => e.endpoint_type === 'freeswitch')
  const kamEndpoints = endpoints.filter((e) => e.endpoint_type === 'kamailio')
  const outboundRoutes = routes.filter((r) => r.route_type === 'fs_to_kamailio')
  const inboundRoutes = routes.filter((r) => r.route_type === 'kamailio_to_fs')

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Rutas Internas</h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Gestiona la conexión entre FreeSWITCH y Kamailio
          </p>
        </div>
        <button
          onClick={() => syncMutation.mutate()}
          disabled={syncMutation.isPending}
          className="inline-flex items-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${syncMutation.isPending ? 'animate-spin' : ''}`} />
          Sincronizar
        </button>
      </div>

      {/* Tabs */}
      <div className="border-b border-gray-200 dark:border-gray-700">
        <nav className="-mb-px flex space-x-8">
          <button
            onClick={() => setActiveTab('routes')}
            className={`border-b-2 px-1 py-4 text-sm font-medium ${
              activeTab === 'routes'
                ? 'border-cyan-500 text-cyan-600'
                : 'border-transparent text-gray-500 hover:border-gray-300 hover:text-gray-700'
            }`}
          >
            Rutas ({routes.length})
          </button>
          <button
            onClick={() => setActiveTab('endpoints')}
            className={`border-b-2 px-1 py-4 text-sm font-medium ${
              activeTab === 'endpoints'
                ? 'border-cyan-500 text-cyan-600'
                : 'border-transparent text-gray-500 hover:border-gray-300 hover:text-gray-700'
            }`}
          >
            Endpoints ({endpoints.length})
          </button>
        </nav>
      </div>

      {/* Routes Tab */}
      {activeTab === 'routes' && (
        <div className="space-y-8">
          {/* Outbound Routes */}
          <div>
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                FreeSWITCH → Kamailio (Salientes)
              </h2>
              <button
                onClick={() => { setEditingRoute(null); setShowRouteModal(true); }}
                className="inline-flex items-center gap-2 rounded-lg bg-cyan-600 px-3 py-2 text-sm font-medium text-white hover:bg-cyan-700"
              >
                <Plus className="h-4 w-4" />
                Nueva Ruta
              </button>
            </div>
            {loadingRoutes ? (
              <div className="text-center py-8 text-gray-500">Cargando...</div>
            ) : outboundRoutes.length === 0 ? (
              <div className="rounded-lg border border-gray-200 bg-gray-50 p-8 text-center text-gray-500 dark:border-gray-700 dark:bg-gray-800">
                No hay rutas salientes configuradas
              </div>
            ) : (
              <div className="grid gap-4 md:grid-cols-2">
                {outboundRoutes.map((route) => (
                  <RouteCard
                    key={route.id}
                    route={route}
                    onEdit={() => handleEditRoute(route)}
                    onDelete={() => handleDeleteRoute(route.id)}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Inbound Routes */}
          <div>
            <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">
              Kamailio → FreeSWITCH (Entrantes)
            </h2>
            {loadingRoutes ? (
              <div className="text-center py-8 text-gray-500">Cargando...</div>
            ) : inboundRoutes.length === 0 ? (
              <div className="rounded-lg border border-gray-200 bg-gray-50 p-8 text-center text-gray-500 dark:border-gray-700 dark:bg-gray-800">
                No hay rutas entrantes configuradas
              </div>
            ) : (
              <div className="grid gap-4 md:grid-cols-2">
                {inboundRoutes.map((route) => (
                  <RouteCard
                    key={route.id}
                    route={route}
                    onEdit={() => handleEditRoute(route)}
                    onDelete={() => handleDeleteRoute(route.id)}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Endpoints Tab */}
      {activeTab === 'endpoints' && (
        <div className="space-y-8">
          {/* FreeSWITCH Endpoints */}
          <div>
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                Endpoints FreeSWITCH
              </h2>
              <button
                onClick={() => { setEditingEndpoint(null); setShowEndpointModal(true); }}
                className="inline-flex items-center gap-2 rounded-lg bg-cyan-600 px-3 py-2 text-sm font-medium text-white hover:bg-cyan-700"
              >
                <Plus className="h-4 w-4" />
                Nuevo Endpoint
              </button>
            </div>
            {loadingEndpoints ? (
              <div className="text-center py-8 text-gray-500">Cargando...</div>
            ) : fsEndpoints.length === 0 ? (
              <div className="rounded-lg border border-gray-200 bg-gray-50 p-8 text-center text-gray-500 dark:border-gray-700 dark:bg-gray-800">
                No hay endpoints FreeSWITCH configurados
              </div>
            ) : (
              <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                {fsEndpoints.map((endpoint) => (
                  <EndpointCard
                    key={endpoint.id}
                    endpoint={endpoint}
                    onEdit={() => handleEditEndpoint(endpoint)}
                    onDelete={() => handleDeleteEndpoint(endpoint.id)}
                  />
                ))}
              </div>
            )}
          </div>

          {/* Kamailio Endpoints */}
          <div>
            <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">
              Endpoints Kamailio
            </h2>
            {loadingEndpoints ? (
              <div className="text-center py-8 text-gray-500">Cargando...</div>
            ) : kamEndpoints.length === 0 ? (
              <div className="rounded-lg border border-gray-200 bg-gray-50 p-8 text-center text-gray-500 dark:border-gray-700 dark:bg-gray-800">
                No hay endpoints Kamailio configurados
              </div>
            ) : (
              <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                {kamEndpoints.map((endpoint) => (
                  <EndpointCard
                    key={endpoint.id}
                    endpoint={endpoint}
                    onEdit={() => handleEditEndpoint(endpoint)}
                    onDelete={() => handleDeleteEndpoint(endpoint.id)}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Endpoint Modal */}
      {showEndpointModal && (
        <EndpointModal
          endpoint={editingEndpoint}
          onClose={() => { setShowEndpointModal(false); setEditingEndpoint(null); }}
          onSuccess={() => {
            setShowEndpointModal(false)
            setEditingEndpoint(null)
            queryClient.invalidateQueries({ queryKey: ['system-endpoints'] })
          }}
        />
      )}

      {/* Route Modal */}
      {showRouteModal && (
        <RouteModal
          route={editingRoute}
          endpoints={endpoints}
          onClose={() => { setShowRouteModal(false); setEditingRoute(null); }}
          onSuccess={() => {
            setShowRouteModal(false)
            setEditingRoute(null)
            queryClient.invalidateQueries({ queryKey: ['internal-routes'] })
          }}
        />
      )}
    </div>
  )
}

// Endpoint Card
function EndpointCard({
  endpoint,
  onEdit,
  onDelete,
}: {
  endpoint: SystemEndpoint
  onEdit: () => void
  onDelete: () => void
}) {
  const isFS = endpoint.endpoint_type === 'freeswitch'

  return (
    <div className={`rounded-lg border bg-white p-4 shadow-sm dark:bg-gray-800 dark:border-gray-700 ${!endpoint.enabled ? 'opacity-60' : ''}`}>
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Server className={`h-5 w-5 ${isFS ? 'text-green-500' : 'text-blue-500'}`} />
          <span className="font-medium text-gray-900 dark:text-white">{endpoint.name}</span>
        </div>
        <div className="flex items-center gap-2">
          {endpoint.is_primary && (
            <span className="rounded bg-gray-100 px-2 py-1 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-300">
              Principal
            </span>
          )}
          <span className={`rounded px-2 py-1 text-xs ${isFS ? 'bg-green-100 text-green-700' : 'bg-blue-100 text-blue-700'}`}>
            {isFS ? 'FreeSWITCH' : 'Kamailio'}
          </span>
        </div>
      </div>

      {endpoint.description && (
        <p className="mb-3 text-sm text-gray-500 dark:text-gray-400">{endpoint.description}</p>
      )}

      <div className="mb-4 space-y-1 text-sm">
        <div className="flex justify-between">
          <span className="text-gray-500">Dirección:</span>
          <span className="font-mono text-gray-900 dark:text-white">{endpoint.ip_address}:{endpoint.port}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">Transporte:</span>
          <span className="uppercase text-gray-900 dark:text-white">{endpoint.transport}</span>
        </div>
        {isFS && endpoint.fs_profile && (
          <div className="flex justify-between">
            <span className="text-gray-500">Perfil:</span>
            <span className="text-gray-900 dark:text-white">{endpoint.fs_profile}</span>
          </div>
        )}
        {!isFS && endpoint.kam_gwid && (
          <div className="flex justify-between">
            <span className="text-gray-500">Gateway ID:</span>
            <span className="text-gray-900 dark:text-white">{endpoint.kam_gwid}</span>
          </div>
        )}
      </div>

      <div className="flex justify-end gap-2">
        <button
          onClick={onEdit}
          className="rounded p-2 text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700"
        >
          <Pencil className="h-4 w-4" />
        </button>
        <button
          onClick={onDelete}
          className="rounded p-2 text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  )
}

// Route Card
function RouteCard({
  route,
  onEdit,
  onDelete,
}: {
  route: InternalRoute
  onEdit: () => void
  onDelete: () => void
}) {
  const isOutbound = route.route_type === 'fs_to_kamailio'

  return (
    <div className={`rounded-lg border bg-white p-4 shadow-sm dark:bg-gray-800 dark:border-gray-700 ${!route.enabled ? 'opacity-60' : ''}`}>
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ArrowRightLeft className={`h-5 w-5 ${isOutbound ? 'text-orange-500' : 'text-purple-500'}`} />
          <span className="font-medium text-gray-900 dark:text-white">{route.name}</span>
        </div>
        <div className="flex items-center gap-2">
          <SyncBadge status={route.sync_status} />
          <span className={`rounded px-2 py-1 text-xs ${isOutbound ? 'bg-orange-100 text-orange-700' : 'bg-purple-100 text-purple-700'}`}>
            {isOutbound ? 'FS → Kam' : 'Kam → FS'}
          </span>
        </div>
      </div>

      {route.description && (
        <p className="mb-3 text-sm text-gray-500 dark:text-gray-400">{route.description}</p>
      )}

      {/* Flow diagram */}
      <div className="mb-4 flex items-center justify-center gap-2 rounded-lg bg-gray-50 p-3 dark:bg-gray-700">
        <div className="text-center">
          <div className="text-xs text-gray-500">Origen</div>
          <div className="font-mono text-sm text-gray-900 dark:text-white">
            {route.source_ip}:{route.source_port}
          </div>
          <div className="text-xs text-gray-500">{route.source_name}</div>
        </div>
        <ArrowRight className="h-5 w-5 text-gray-400" />
        <div className="text-center">
          <div className="text-xs text-gray-500">Destino</div>
          <div className="font-mono text-sm text-gray-900 dark:text-white">
            {route.dest_ip}:{route.dest_port}
          </div>
          <div className="text-xs text-gray-500">{route.dest_name}</div>
        </div>
      </div>

      {/* Options */}
      <div className="mb-4 grid grid-cols-2 gap-2 text-sm">
        <div className="flex justify-between">
          <span className="text-gray-500">Bypass Media:</span>
          <span className={route.bypass_media ? 'text-green-600' : 'text-gray-400'}>
            {route.bypass_media ? 'Sí' : 'No'}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">Timeout:</span>
          <span className="text-gray-900 dark:text-white">{route.call_timeout}s</span>
        </div>
      </div>

      {route.sync_error && (
        <div className="mb-4 rounded bg-red-50 p-2 text-xs text-red-600 dark:bg-red-900/20 dark:text-red-400">
          {route.sync_error}
        </div>
      )}

      <div className="flex justify-end gap-2">
        <button
          onClick={onEdit}
          className="rounded p-2 text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700"
        >
          <Pencil className="h-4 w-4" />
        </button>
        <button
          onClick={onDelete}
          className="rounded p-2 text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  )
}

// Endpoint Modal
function EndpointModal({
  endpoint,
  onClose,
  onSuccess,
}: {
  endpoint: SystemEndpoint | null
  onClose: () => void
  onSuccess: () => void
}) {
  const [formData, setFormData] = useState({
    name: endpoint?.name || '',
    endpoint_type: endpoint?.endpoint_type || 'freeswitch',
    description: endpoint?.description || '',
    ip_address: endpoint?.ip_address || '',
    port: endpoint?.port || 5060,
    transport: endpoint?.transport || 'udp',
    fs_profile: endpoint?.fs_profile || '',
    fs_context: endpoint?.fs_context || '',
    kam_gwid: endpoint?.kam_gwid,
    kam_gw_type: endpoint?.kam_gw_type || 9,
    enabled: endpoint?.enabled ?? true,
    is_primary: endpoint?.is_primary ?? false,
  })

  const mutation = useMutation({
    mutationFn: endpoint
      ? (data: typeof formData) => updateEndpoint({ id: endpoint.id, ...data })
      : createEndpoint,
    onSuccess,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    mutation.mutate(formData)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-md max-h-[90vh] overflow-y-auto rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {endpoint ? 'Editar Endpoint' : 'Nuevo Endpoint'}
          </h2>
          <button onClick={onClose} className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Nombre</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              required
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Tipo</label>
            <select
              value={formData.endpoint_type}
              onChange={(e) => setFormData({ ...formData, endpoint_type: e.target.value as 'freeswitch' | 'kamailio' })}
              disabled={!!endpoint}
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            >
              <option value="freeswitch">FreeSWITCH</option>
              <option value="kamailio">Kamailio</option>
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">IP</label>
              <input
                type="text"
                value={formData.ip_address}
                onChange={(e) => setFormData({ ...formData, ip_address: e.target.value })}
                required
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Puerto</label>
              <input
                type="number"
                value={formData.port}
                onChange={(e) => setFormData({ ...formData, port: parseInt(e.target.value) })}
                required
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              />
            </div>
          </div>

          {formData.endpoint_type === 'freeswitch' && (
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Perfil SIP</label>
                <input
                  type="text"
                  value={formData.fs_profile}
                  onChange={(e) => setFormData({ ...formData, fs_profile: e.target.value })}
                  placeholder="internal"
                  className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Contexto</label>
                <input
                  type="text"
                  value={formData.fs_context}
                  onChange={(e) => setFormData({ ...formData, fs_context: e.target.value })}
                  placeholder="from-pbx"
                  className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                />
              </div>
            </div>
          )}

          {formData.endpoint_type === 'kamailio' && (
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Gateway ID</label>
                <input
                  type="number"
                  value={formData.kam_gwid || ''}
                  onChange={(e) => setFormData({ ...formData, kam_gwid: parseInt(e.target.value) || undefined })}
                  className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Tipo GW</label>
                <select
                  value={formData.kam_gw_type}
                  onChange={(e) => setFormData({ ...formData, kam_gw_type: parseInt(e.target.value) })}
                  className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                >
                  <option value={8}>8 - Carrier</option>
                  <option value={9}>9 - PBX</option>
                </select>
              </div>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Descripción</label>
            <textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              rows={2}
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            />
          </div>

          <div className="flex items-center gap-6">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={formData.enabled}
                onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">Habilitado</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={formData.is_primary}
                onChange={(e) => setFormData({ ...formData, is_primary: e.target.checked })}
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">Principal</span>
            </label>
          </div>

          <div className="flex justify-end gap-3 pt-4 mt-4 border-t border-gray-200 dark:border-gray-700">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-700"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              className="rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-700 disabled:opacity-50"
            >
              {mutation.isPending ? 'Guardando...' : 'Guardar'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

// Route Modal
function RouteModal({
  route,
  endpoints,
  onClose,
  onSuccess,
}: {
  route: InternalRoute | null
  endpoints: SystemEndpoint[]
  onClose: () => void
  onSuccess: () => void
}) {
  const [formData, setFormData] = useState<{
    name: string
    description: string
    route_type: 'fs_to_kamailio' | 'kamailio_to_fs'
    source_endpoint_id: string | undefined
    dest_endpoint_id: string | undefined
    bypass_media: boolean
    inherit_codec: boolean
    enable_100rel: boolean
    call_timeout: number
    prefix_pattern: string
    priority: number
    enabled: boolean
  }>({
    name: route?.name || '',
    description: route?.description || '',
    route_type: route?.route_type || 'fs_to_kamailio',
    source_endpoint_id: route?.source_endpoint_id,
    dest_endpoint_id: route?.dest_endpoint_id,
    bypass_media: route?.bypass_media ?? true,
    inherit_codec: route?.inherit_codec ?? true,
    enable_100rel: route?.enable_100rel ?? true,
    call_timeout: route?.call_timeout ?? 60,
    prefix_pattern: route?.prefix_pattern || '.*',
    priority: route?.priority ?? 100,
    enabled: route?.enabled ?? true,
  })

  const mutation = useMutation({
    mutationFn: route
      ? (data: typeof formData) => updateRoute({ id: route.id, ...data })
      : createRoute,
    onSuccess,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    mutation.mutate(formData)
  }

  const fsEndpoints = endpoints.filter((e) => e.endpoint_type === 'freeswitch')
  const kamEndpoints = endpoints.filter((e) => e.endpoint_type === 'kamailio')
  const sourceEndpoints = formData.route_type === 'fs_to_kamailio' ? fsEndpoints : kamEndpoints
  const destEndpoints = formData.route_type === 'fs_to_kamailio' ? kamEndpoints : fsEndpoints

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-lg max-h-[90vh] overflow-y-auto rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {route ? 'Editar Ruta' : 'Nueva Ruta Interna'}
          </h2>
          <button onClick={onClose} className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
            <X className="h-5 w-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Nombre</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              required
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Dirección</label>
            <select
              value={formData.route_type}
              onChange={(e) => setFormData({ ...formData, route_type: e.target.value as 'fs_to_kamailio' | 'kamailio_to_fs', source_endpoint_id: undefined, dest_endpoint_id: undefined })}
              disabled={!!route}
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            >
              <option value="fs_to_kamailio">FreeSWITCH → Kamailio (Saliente)</option>
              <option value="kamailio_to_fs">Kamailio → FreeSWITCH (Entrante)</option>
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Origen</label>
              <select
                value={formData.source_endpoint_id || ''}
                onChange={(e) => setFormData({ ...formData, source_endpoint_id: e.target.value || undefined })}
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              >
                <option value="">Seleccionar...</option>
                {sourceEndpoints.map((ep) => (
                  <option key={ep.id} value={ep.id}>
                    {ep.name} ({ep.ip_address}:{ep.port})
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Destino</label>
              <select
                value={formData.dest_endpoint_id || ''}
                onChange={(e) => setFormData({ ...formData, dest_endpoint_id: e.target.value || undefined })}
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              >
                <option value="">Seleccionar...</option>
                {destEndpoints.map((ep) => (
                  <option key={ep.id} value={ep.id}>
                    {ep.name} ({ep.ip_address}:{ep.port})
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Timeout (seg)</label>
              <input
                type="number"
                value={formData.call_timeout}
                onChange={(e) => setFormData({ ...formData, call_timeout: parseInt(e.target.value) })}
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Prioridad</label>
              <input
                type="number"
                value={formData.priority}
                onChange={(e) => setFormData({ ...formData, priority: parseInt(e.target.value) })}
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
              />
            </div>
          </div>

          <div className="flex items-center gap-6">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={formData.bypass_media}
                onChange={(e) => setFormData({ ...formData, bypass_media: e.target.checked })}
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">Bypass Media</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={formData.inherit_codec}
                onChange={(e) => setFormData({ ...formData, inherit_codec: e.target.checked })}
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">Inherit Codec</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={formData.enable_100rel}
                onChange={(e) => setFormData({ ...formData, enable_100rel: e.target.checked })}
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">100rel</span>
            </label>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Descripción</label>
            <textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              rows={2}
              className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            />
          </div>

          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={formData.enabled}
              onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
              className="rounded border-gray-300"
            />
            <span className="text-sm text-gray-700 dark:text-gray-300">Habilitado</span>
          </label>

          <div className="flex justify-end gap-3 pt-4 mt-4 border-t border-gray-200 dark:border-gray-700">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-700"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              className="rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-700 disabled:opacity-50"
            >
              {mutation.isPending ? 'Guardando...' : 'Guardar'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
