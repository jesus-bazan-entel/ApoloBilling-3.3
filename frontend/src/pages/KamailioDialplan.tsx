import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchKamailioCarrierGroups,
  createKamailioCarrierGroup,
  deleteKamailioCarrierGroup,
  fetchKamailioCarriers,
  createKamailioCarrier,
  deleteKamailioCarrier,
  fetchKamailioOutboundRoutes,
  createKamailioOutboundRoute,
  deleteKamailioOutboundRoute,
  reloadKamailio,
  type KamailioCarrierGroup,
  type KamailioCarrier,
  type KamailioOutboundRoute,
} from '../api/client'
import DataTable from '../components/DataTable'
import {
  Server,
  Plus,
  X,
  Trash2,
  RefreshCw,
  Radio,
  Route,
  Layers,
  AlertCircle,
} from 'lucide-react'

type TabType = 'carrier-groups' | 'carriers' | 'outbound-routes'

export default function KamailioDialplanPage() {
  const [activeTab, setActiveTab] = useState<TabType>('carrier-groups')
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [deletingItem, setDeletingItem] = useState<{ type: TabType; item: KamailioCarrierGroup | KamailioCarrier | KamailioOutboundRoute } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [successMessage, setSuccessMessage] = useState<string | null>(null)
  const queryClient = useQueryClient()

  // Queries
  const { data: carrierGroups = [], isLoading: loadingGroups, error: groupsError } = useQuery({
    queryKey: ['kamailio-carrier-groups'],
    queryFn: fetchKamailioCarrierGroups,
    retry: 1,
  })

  const { data: carriers = [], isLoading: loadingCarriers, error: carriersError } = useQuery({
    queryKey: ['kamailio-carriers'],
    queryFn: fetchKamailioCarriers,
    retry: 1,
  })

  const { data: outboundRoutes = [], isLoading: loadingRoutes, error: routesError } = useQuery({
    queryKey: ['kamailio-outbound-routes'],
    queryFn: fetchKamailioOutboundRoutes,
    retry: 1,
  })

  // Mutations
  const createGroupMutation = useMutation({
    mutationFn: createKamailioCarrierGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-carrier-groups'] })
      setShowCreateModal(false)
      showSuccess('Grupo de carriers creado exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const deleteGroupMutation = useMutation({
    mutationFn: deleteKamailioCarrierGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-carrier-groups'] })
      setDeletingItem(null)
      showSuccess('Grupo eliminado exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const createCarrierMutation = useMutation({
    mutationFn: createKamailioCarrier,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-carriers'] })
      setShowCreateModal(false)
      showSuccess('Carrier creado exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const deleteCarrierMutation = useMutation({
    mutationFn: deleteKamailioCarrier,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-carriers'] })
      setDeletingItem(null)
      showSuccess('Carrier eliminado exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const createRouteMutation = useMutation({
    mutationFn: createKamailioOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-outbound-routes'] })
      setShowCreateModal(false)
      showSuccess('Ruta creada exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const deleteRouteMutation = useMutation({
    mutationFn: deleteKamailioOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kamailio-outbound-routes'] })
      setDeletingItem(null)
      showSuccess('Ruta eliminada exitosamente')
    },
    onError: (err: Error) => setError(err.message),
  })

  const reloadMutation = useMutation({
    mutationFn: reloadKamailio,
    onSuccess: (data) => showSuccess(data.message || 'Kamailio recargado exitosamente'),
    onError: (err: Error) => setError(err.message),
  })

  const showSuccess = (message: string) => {
    setSuccessMessage(message)
    setError(null)
    setTimeout(() => setSuccessMessage(null), 3000)
  }

  const handleDelete = () => {
    if (!deletingItem) return

    switch (deletingItem.type) {
      case 'carrier-groups':
        deleteGroupMutation.mutate((deletingItem.item as KamailioCarrierGroup).id)
        break
      case 'carriers':
        deleteCarrierMutation.mutate((deletingItem.item as KamailioCarrier).gwid)
        break
      case 'outbound-routes':
        deleteRouteMutation.mutate((deletingItem.item as KamailioOutboundRoute).ruleid)
        break
    }
  }

  // Check for connection errors
  const hasConnectionError = groupsError || carriersError || routesError
  const connectionErrorMessage = hasConnectionError
    ? 'No se pudo conectar con la base de datos de Kamailio. Verifica que la variable de entorno KAMAILIO_DATABASE_URL este configurada correctamente.'
    : null

  // Columns for each tab
  const groupColumns = [
    {
      key: 'id',
      header: 'ID',
      render: (group: KamailioCarrierGroup) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{group.id}</span>
      ),
    },
    {
      key: 'description',
      header: 'Descripcion',
      render: (group: KamailioCarrierGroup) => (
        <span className="font-semibold text-[var(--color-text-primary)]">{group.description || '-'}</span>
      ),
    },
    {
      key: 'gwlist',
      header: 'Gateways',
      render: (group: KamailioCarrierGroup) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">
          {group.gwlist || '-'}
        </span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (group: KamailioCarrierGroup) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setDeletingItem({ type: 'carrier-groups', item: group })}
            className="p-1.5 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
            title="Eliminar"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  const carrierColumns = [
    {
      key: 'gwid',
      header: 'ID',
      render: (carrier: KamailioCarrier) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{carrier.gwid}</span>
      ),
    },
    {
      key: 'description',
      header: 'Descripcion',
      render: (carrier: KamailioCarrier) => (
        <span className="font-semibold text-[var(--color-text-primary)]">{carrier.description || '-'}</span>
      ),
    },
    {
      key: 'address',
      header: 'Direccion SIP',
      render: (carrier: KamailioCarrier) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{carrier.address}</span>
      ),
    },
    {
      key: 'strip',
      header: 'Strip',
      render: (carrier: KamailioCarrier) => (
        <span className="text-sm text-[var(--color-text-secondary)]">{carrier.strip}</span>
      ),
    },
    {
      key: 'pri_prefix',
      header: 'Prefijo',
      render: (carrier: KamailioCarrier) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{carrier.pri_prefix || '-'}</span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (carrier: KamailioCarrier) => (
        <button
          onClick={() => setDeletingItem({ type: 'carriers', item: carrier })}
          className="p-1.5 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
          title="Eliminar"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      ),
    },
  ]

  const routeColumns = [
    {
      key: 'ruleid',
      header: 'ID',
      render: (route: KamailioOutboundRoute) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{route.ruleid}</span>
      ),
    },
    {
      key: 'prefix',
      header: 'Prefijo',
      render: (route: KamailioOutboundRoute) => (
        <span className="font-mono text-sm font-semibold text-[var(--color-text-primary)]">{route.prefix || '*'}</span>
      ),
    },
    {
      key: 'description',
      header: 'Descripcion',
      render: (route: KamailioOutboundRoute) => (
        <span className="text-[var(--color-text-secondary)]">{route.description || '-'}</span>
      ),
    },
    {
      key: 'gwlist',
      header: 'Gateway/Grupo',
      render: (route: KamailioOutboundRoute) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">{route.gwlist}</span>
      ),
    },
    {
      key: 'priority',
      header: 'Prioridad',
      render: (route: KamailioOutboundRoute) => (
        <span className="px-2 py-1 text-xs rounded-full bg-blue-100 text-blue-800">
          {route.priority}
        </span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (route: KamailioOutboundRoute) => (
        <button
          onClick={() => setDeletingItem({ type: 'outbound-routes', item: route })}
          className="p-1.5 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
          title="Eliminar"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      ),
    },
  ]

  const tabs = [
    { id: 'carrier-groups' as const, label: 'Grupos de Carriers', icon: Layers, count: carrierGroups.length },
    { id: 'carriers' as const, label: 'Carriers/Gateways', icon: Server, count: carriers.length },
    { id: 'outbound-routes' as const, label: 'Rutas Salientes', icon: Route, count: outboundRoutes.length },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-[var(--color-text-primary)] flex items-center gap-2">
            <Radio className="w-7 h-7 text-purple-500" />
            Dialplan Kamailio
          </h1>
          <p className="text-[var(--color-text-tertiary)]">
            Gestiona carriers y rutas salientes de Kamailio
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => reloadMutation.mutate()}
            disabled={reloadMutation.isPending}
            className="flex items-center px-4 py-2 bg-purple-500 text-white rounded-lg hover:bg-purple-600 transition-colors disabled:opacity-50"
          >
            <RefreshCw className={`w-5 h-5 mr-2 ${reloadMutation.isPending ? 'animate-spin' : ''}`} />
            Recargar Kamailio
          </button>
        </div>
      </div>

      {/* Connection Error */}
      {connectionErrorMessage && (
        <div className="p-4 bg-amber-50 border border-amber-200 rounded-lg flex items-start gap-3">
          <AlertCircle className="w-5 h-5 text-amber-500 flex-shrink-0 mt-0.5" />
          <div>
            <p className="text-amber-800 font-medium">Advertencia de conexion</p>
            <p className="text-amber-700 text-sm">{connectionErrorMessage}</p>
          </div>
        </div>
      )}

      {/* Success Message */}
      {successMessage && (
        <div className="p-3 bg-green-50 border border-green-200 rounded-lg text-green-700 text-sm">
          {successMessage}
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div className="p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
          {error}
        </div>
      )}

      {/* Tabs */}
      <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)]">
        <div className="border-b border-[var(--color-border-primary)]">
          <nav className="flex -mb-px">
            {tabs.map((tab) => {
              const Icon = tab.icon
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center px-6 py-3 text-sm font-medium transition-colors ${
                    activeTab === tab.id
                      ? 'border-b-2 border-purple-500 text-purple-600'
                      : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)]'
                  }`}
                >
                  <Icon className="w-4 h-4 mr-2" />
                  {tab.label}
                  <span className={`ml-2 px-2 py-0.5 text-xs rounded-full ${
                    activeTab === tab.id ? 'bg-purple-100 text-purple-700' : 'bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)]'
                  }`}>
                    {tab.count}
                  </span>
                </button>
              )
            })}
          </nav>
        </div>

        <div className="p-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center">
              {activeTab === 'carrier-groups' && <Layers className="w-8 h-8 text-purple-500 mr-4" />}
              {activeTab === 'carriers' && <Server className="w-8 h-8 text-purple-500 mr-4" />}
              {activeTab === 'outbound-routes' && <Route className="w-8 h-8 text-purple-500 mr-4" />}
              <div>
                <p className="text-sm text-[var(--color-text-tertiary)]">
                  {activeTab === 'carrier-groups' && 'Total de Grupos'}
                  {activeTab === 'carriers' && 'Total de Carriers'}
                  {activeTab === 'outbound-routes' && 'Total de Rutas'}
                </p>
                <p className="text-3xl font-bold text-[var(--color-text-primary)]">
                  {activeTab === 'carrier-groups' && carrierGroups.length}
                  {activeTab === 'carriers' && carriers.length}
                  {activeTab === 'outbound-routes' && outboundRoutes.length}
                </p>
              </div>
            </div>
            <button
              onClick={() => {
                setShowCreateModal(true)
                setError(null)
              }}
              className="flex items-center px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors"
            >
              <Plus className="w-5 h-5 mr-2" />
              {activeTab === 'carrier-groups' && 'Nuevo Grupo'}
              {activeTab === 'carriers' && 'Nuevo Carrier'}
              {activeTab === 'outbound-routes' && 'Nueva Ruta'}
            </button>
          </div>

          {/* Data Tables */}
          {activeTab === 'carrier-groups' && (
            <DataTable
              columns={groupColumns}
              data={carrierGroups}
              loading={loadingGroups}
              emptyMessage="No hay grupos de carriers configurados"
              searchable={true}
              searchPlaceholder="Buscar grupos..."
            />
          )}

          {activeTab === 'carriers' && (
            <DataTable
              columns={carrierColumns}
              data={carriers}
              loading={loadingCarriers}
              emptyMessage="No hay carriers configurados"
              searchable={true}
              searchPlaceholder="Buscar carriers..."
            />
          )}

          {activeTab === 'outbound-routes' && (
            <DataTable
              columns={routeColumns}
              data={outboundRoutes}
              loading={loadingRoutes}
              emptyMessage="No hay rutas salientes configuradas"
              searchable={true}
              searchPlaceholder="Buscar rutas..."
            />
          )}
        </div>
      </div>

      {/* Create Modals */}
      {showCreateModal && activeTab === 'carrier-groups' && (
        <CarrierGroupModal
          onClose={() => setShowCreateModal(false)}
          onSubmit={(data) => createGroupMutation.mutate(data)}
          isLoading={createGroupMutation.isPending}
          error={error}
        />
      )}

      {showCreateModal && activeTab === 'carriers' && (
        <CarrierModal
          onClose={() => setShowCreateModal(false)}
          onSubmit={(data) => createCarrierMutation.mutate(data)}
          isLoading={createCarrierMutation.isPending}
          error={error}
        />
      )}

      {showCreateModal && activeTab === 'outbound-routes' && (
        <OutboundRouteModal
          carrierGroups={carrierGroups}
          onClose={() => setShowCreateModal(false)}
          onSubmit={(data) => createRouteMutation.mutate(data)}
          isLoading={createRouteMutation.isPending}
          error={error}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deletingItem && (
        <DeleteConfirmModal
          title={
            deletingItem.type === 'carrier-groups' ? 'Eliminar Grupo' :
            deletingItem.type === 'carriers' ? 'Eliminar Carrier' :
            'Eliminar Ruta'
          }
          message={`¿Estas seguro de que deseas eliminar este elemento? Esta accion no se puede deshacer.`}
          onClose={() => setDeletingItem(null)}
          onConfirm={handleDelete}
          isLoading={deleteGroupMutation.isPending || deleteCarrierMutation.isPending || deleteRouteMutation.isPending}
        />
      )}
    </div>
  )
}

// Modal Components
interface CarrierGroupModalProps {
  onClose: () => void
  onSubmit: (data: { description: string; gwlist?: string }) => void
  isLoading: boolean
  error: string | null
}

function CarrierGroupModal({ onClose, onSubmit, isLoading, error }: CarrierGroupModalProps) {
  const [description, setDescription] = useState('')
  const [gwlist, setGwlist] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit({ description, gwlist: gwlist || undefined })
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">Nuevo Grupo de Carriers</h2>
          <button onClick={onClose} className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]">
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
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Descripcion *
            </label>
            <input
              type="text"
              required
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Ej: Carrier Principal"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Lista de Gateways (IDs separados por coma)
            </label>
            <input
              type="text"
              value={gwlist}
              onChange={(e) => setGwlist(e.target.value)}
              placeholder="1,2,3"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500 font-mono"
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              IDs de gateways separados por coma, ej: 1,2,3
            </p>
          </div>

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
              className="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : 'Crear Grupo'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

interface CarrierModalProps {
  onClose: () => void
  onSubmit: (data: Partial<KamailioCarrier>) => void
  isLoading: boolean
  error: string | null
}

function CarrierModal({ onClose, onSubmit, isLoading, error }: CarrierModalProps) {
  const [description, setDescription] = useState('')
  const [address, setAddress] = useState('')
  const [strip, setStrip] = useState(0)
  const [priPrefix, setPriPrefix] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit({ description, address, strip, pri_prefix: priPrefix })
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">Nuevo Carrier</h2>
          <button onClick={onClose} className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]">
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
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Descripcion
            </label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Ej: Gateway Claro"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Direccion SIP *
            </label>
            <input
              type="text"
              required
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="sip:carrier.example.com:5060"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500 font-mono text-sm"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Strip (digitos)
              </label>
              <input
                type="number"
                value={strip}
                onChange={(e) => setStrip(parseInt(e.target.value) || 0)}
                min="0"
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Prefijo
              </label>
              <input
                type="text"
                value={priPrefix}
                onChange={(e) => setPriPrefix(e.target.value)}
                placeholder="011"
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500 font-mono"
              />
            </div>
          </div>

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
              className="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : 'Crear Carrier'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

interface OutboundRouteModalProps {
  carrierGroups: KamailioCarrierGroup[]
  onClose: () => void
  onSubmit: (data: Partial<KamailioOutboundRoute>) => void
  isLoading: boolean
  error: string | null
}

function OutboundRouteModal({ carrierGroups, onClose, onSubmit, isLoading, error }: OutboundRouteModalProps) {
  const [prefix, setPrefix] = useState('')
  const [description, setDescription] = useState('')
  const [gwgroupid, setGwgroupid] = useState<number | undefined>()
  const [priority, setPriority] = useState(1)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit({
      prefix,
      description,
      gwlist: gwgroupid ? `#${gwgroupid}` : '',
      priority,
      groupid: '8000', // FLT_OUTBOUND default
      timerec: '',
      routeid: '',
    })
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">Nueva Ruta Saliente</h2>
          <button onClick={onClose} className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]">
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
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Prefijo de Destino
            </label>
            <input
              type="text"
              value={prefix}
              onChange={(e) => setPrefix(e.target.value)}
              placeholder="54 (vacio = todas las llamadas)"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500 font-mono"
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              Deja vacio para que coincida con todas las llamadas
            </p>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Descripcion
            </label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Ruta para llamadas nacionales"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Grupo de Carriers *
            </label>
            <select
              required
              value={gwgroupid || ''}
              onChange={(e) => setGwgroupid(parseInt(e.target.value))}
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
            >
              <option value="">Selecciona un grupo</option>
              {carrierGroups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.description || `Grupo ${group.id}`}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Prioridad
            </label>
            <input
              type="number"
              value={priority}
              onChange={(e) => setPriority(parseInt(e.target.value) || 1)}
              min="1"
              className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-purple-500"
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              Menor numero = mayor prioridad
            </p>
          </div>

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
              className="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : 'Crear Ruta'}
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
}

function DeleteConfirmModal({ title, message, onClose, onConfirm, isLoading }: DeleteConfirmModalProps) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">{title}</h2>
          <button onClick={onClose} className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]">
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="p-6">
          <p className="text-[var(--color-text-secondary)]">{message}</p>

          <div className="flex justify-end space-x-3 mt-6">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)]"
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
