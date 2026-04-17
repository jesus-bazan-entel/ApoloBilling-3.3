import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchRoutingTrunks,
  fetchRoutingTrunkGroups,
  fetchRoutingOutboundRoutes,
  fetchRoutingInboundRoutes,
  fetchRoutingSyncStatus,
  createRoutingTrunk,
  updateRoutingTrunk,
  deleteRoutingTrunk,
  createRoutingTrunkGroup,
  updateRoutingTrunkGroup,
  createRoutingOutboundRoute,
  updateRoutingOutboundRoute,
  deleteRoutingOutboundRoute,
  createRoutingInboundRoute,
  updateRoutingInboundRoute,
  deleteRoutingInboundRoute,
  migrateRouting,
  syncToKamailio,
  checkTrunkSipStatus,
  checkAllTrunksSipStatus,
} from '../api/client'
import type {
  RoutingTrunk,
  RoutingTrunkGroup,
  RoutingTrunkGroupWithMembers,
  RoutingOutboundRoute,
  RoutingInboundRoute,
  CreateTrunkRequest,
  CreateTrunkGroupRequest,
  CreateOutboundRouteRequest,
  CreateInboundRouteRequest,
  TrunkGroupMemberInput,
  FailoverDestination,
  TrunkType,
  SipStatusCheck,
} from '../types'
import DataTable from '../components/DataTable'
import {
  Server,
  Route,
  Router,
  ArrowDownToLine,
  RefreshCw,
  Plus,
  X,
  AlertCircle,
  CheckCircle,
  Clock,
  Trash2,
  Database,
  Activity,
  Wifi,
  WifiOff,
  Building2,
  Globe,
  ArrowRight,
  ArrowLeft,
  MessageSquare,
  Eye,
  Terminal,
  Info,
} from 'lucide-react'

type TabType = 'trunks' | 'outbound' | 'inbound'

export default function UnifiedRouting() {
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<TabType>('trunks')

  // Modal states
  const [showTrunkModal, setShowTrunkModal] = useState(false)
  const [showGroupModal, setShowGroupModal] = useState(false)
  const [showOutboundModal, setShowOutboundModal] = useState(false)
  const [showInboundModal, setShowInboundModal] = useState(false)
  const [showMigrateModal, setShowMigrateModal] = useState(false)

  // Edit state
  const [editingTrunk, setEditingTrunk] = useState<RoutingTrunk | null>(null)
  const [editingGroup, setEditingGroup] = useState<RoutingTrunkGroupWithMembers | null>(null)
  const [editingOutbound, setEditingOutbound] = useState<RoutingOutboundRoute | null>(null)
  const [editingInbound, setEditingInbound] = useState<RoutingInboundRoute | null>(null)

  // Error state
  const [deleteError, setDeleteError] = useState<string | null>(null)

  // Queries
  const { data: trunks = [], isLoading: loadingTrunks } = useQuery({
    queryKey: ['routing-trunks'],
    queryFn: fetchRoutingTrunks,
  })

  const { data: trunkGroups = [] } = useQuery({
    queryKey: ['routing-trunk-groups'],
    queryFn: fetchRoutingTrunkGroups,
  })

  const { data: outboundRoutes = [], isLoading: loadingOutbound } = useQuery({
    queryKey: ['routing-outbound'],
    queryFn: fetchRoutingOutboundRoutes,
  })

  const { data: inboundRoutes = [], isLoading: loadingInbound } = useQuery({
    queryKey: ['routing-inbound'],
    queryFn: fetchRoutingInboundRoutes,
  })

  const { data: syncStatus } = useQuery({
    queryKey: ['routing-sync-status'],
    queryFn: fetchRoutingSyncStatus,
    refetchInterval: 30000,
  })

  // Mutations
  const migrateMutation = useMutation({
    mutationFn: migrateRouting,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
      queryClient.invalidateQueries({ queryKey: ['routing-trunk-groups'] })
      queryClient.invalidateQueries({ queryKey: ['routing-outbound'] })
      queryClient.invalidateQueries({ queryKey: ['routing-inbound'] })
      setShowMigrateModal(false)
    },
  })

  const syncMutation = useMutation({
    mutationFn: syncToKamailio,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
      queryClient.invalidateQueries({ queryKey: ['routing-trunk-groups'] })
      queryClient.invalidateQueries({ queryKey: ['routing-outbound'] })
      queryClient.invalidateQueries({ queryKey: ['routing-sync-status'] })
    },
  })

  // Trunk mutations
  const createTrunkMutation = useMutation({
    mutationFn: createRoutingTrunk,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
      setShowTrunkModal(false)
    },
  })

  const updateTrunkMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateTrunkRequest> }) =>
      updateRoutingTrunk(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
      setShowTrunkModal(false)
      setEditingTrunk(null)
    },
  })

  const deleteTrunkMutation = useMutation({
    mutationFn: deleteRoutingTrunk,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
      setDeleteError(null)
    },
    onError: (error: unknown) => {
      // Extract error message from axios error response
      // Backend returns: { error: "validation_error", message: "actual message", status: 400 }
      const axiosError = error as { response?: { data?: { error?: string; message?: string } }; message?: string }
      const errorMessage = axiosError.response?.data?.message
        || axiosError.response?.data?.error
        || axiosError.message
        || 'Error al eliminar la troncal'
      setDeleteError(errorMessage)
    },
  })

  // Group mutations
  const createGroupMutation = useMutation({
    mutationFn: createRoutingTrunkGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunk-groups'] })
      setShowGroupModal(false)
    },
  })

  const updateGroupMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateTrunkGroupRequest> }) =>
      updateRoutingTrunkGroup(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-trunk-groups'] })
      setShowGroupModal(false)
      setEditingGroup(null)
    },
  })

  // Outbound mutations
  const createOutboundMutation = useMutation({
    mutationFn: createRoutingOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-outbound'] })
      setShowOutboundModal(false)
    },
  })

  const updateOutboundMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateOutboundRouteRequest> }) =>
      updateRoutingOutboundRoute(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-outbound'] })
      setShowOutboundModal(false)
      setEditingOutbound(null)
    },
  })

  const deleteOutboundMutation = useMutation({
    mutationFn: deleteRoutingOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-outbound'] })
    },
  })

  // Inbound mutations
  const createInboundMutation = useMutation({
    mutationFn: createRoutingInboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-inbound'] })
      setShowInboundModal(false)
    },
  })

  const updateInboundMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateInboundRouteRequest> }) =>
      updateRoutingInboundRoute(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-inbound'] })
      setShowInboundModal(false)
      setEditingInbound(null)
    },
  })

  const deleteInboundMutation = useMutation({
    mutationFn: deleteRoutingInboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['routing-inbound'] })
    },
  })

  // Tabs
  const tabs = [
    { id: 'trunks' as TabType, label: 'Trunks', icon: Server, count: trunks.length },
    { id: 'outbound' as TabType, label: 'Rutas Salientes', icon: Route, count: outboundRoutes.length },
    { id: 'inbound' as TabType, label: 'Rutas Entrantes', icon: ArrowDownToLine, count: inboundRoutes.length },
  ]

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div className="flex items-center gap-4">
          <div className="p-3 bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-primary-hover)] rounded-xl shadow-lg shadow-[var(--color-primary)]/25">
            <Router className="w-7 h-7 text-white" />
          </div>
          <div>
            <h1 className="text-3xl font-bold gradient-text tracking-tight">Rutas Externas</h1>
            <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
              Configuración de trunks, rutas entrantes y salientes
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          {/* Sync Status */}
          {syncStatus && (
            <div className="flex items-center gap-2 px-3 py-2 bg-[var(--color-bg-card)] rounded-lg border border-[var(--color-border-primary)] shadow-sm">
              <div className={`w-2 h-2 rounded-full ${
                syncStatus.trunks_error > 0 || !(syncStatus.kamailio_connected && syncStatus.freeswitch_connected)
                  ? 'bg-red-500'
                  : syncStatus.trunks_pending > 0
                    ? 'bg-amber-500 animate-pulse'
                    : 'bg-emerald-500'
              }`} />
              <span className="text-sm text-[var(--color-text-secondary)]">
                {syncStatus.trunks_synced}/{syncStatus.trunks_total} sincronizados
              </span>
            </div>
          )}

          {/* Migrate Button */}
          <button
            onClick={() => setShowMigrateModal(true)}
            className="px-3 py-2 rounded-lg bg-[var(--color-bg-card)] border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:text-[var(--color-primary)] hover:border-[var(--color-primary)]/50 transition-colors flex items-center gap-2 cursor-pointer shadow-sm"
          >
            <Database className="w-4 h-4" />
            Migrar
          </button>

          {/* Sync to Kamailio Button */}
          <button
            onClick={() => syncMutation.mutate()}
            disabled={syncMutation.isPending}
            className="px-4 py-2 rounded-lg bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white hover:shadow-lg hover:shadow-[var(--color-primary)]/30 transition-all flex items-center gap-2 disabled:opacity-50 cursor-pointer font-medium"
            title="Sincronizar rutas a Kamailio y recargar"
          >
            <RefreshCw className={`w-4 h-4 ${syncMutation.isPending ? 'animate-spin' : ''}`} />
            Sincronizar
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1 p-1.5 bg-[var(--color-bg-card)] rounded-xl w-fit border border-[var(--color-border-primary)] shadow-sm">
        {tabs.map((tab) => {
          const Icon = tab.icon
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-all cursor-pointer ${
                activeTab === tab.id
                  ? 'bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white shadow-md shadow-[var(--color-primary)]/20'
                  : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)]'
              }`}
            >
              <Icon className="w-4 h-4" />
              {tab.label}
              <span className={`px-2 py-0.5 rounded-md text-xs font-semibold ${
                activeTab === tab.id
                  ? 'bg-white/20 text-white'
                  : 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-tertiary)]'
              }`}>
                {tab.count}
              </span>
            </button>
          )
        })}
      </div>

      {/* Error Message */}
      {deleteError && (
        <div className="mb-4 p-4 bg-red-500/10 border border-red-500/30 rounded-lg flex items-start gap-3">
          <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
          <div className="flex-1">
            <p className="text-red-600 dark:text-red-400 font-medium">Error al eliminar</p>
            <p className="text-red-500/80 text-sm mt-1">{deleteError}</p>
          </div>
          <button
            onClick={() => setDeleteError(null)}
            className="text-red-500/60 hover:text-red-500 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>
      )}

      {/* Tab Content */}
      {activeTab === 'trunks' && (
        <TrunksTab
          trunks={trunks}
          loading={loadingTrunks}
          onAddTrunk={() => { setEditingTrunk(null); setShowTrunkModal(true) }}
          onEditTrunk={(t) => { setEditingTrunk(t); setShowTrunkModal(true) }}
          onDeleteTrunk={(id) => { setDeleteError(null); deleteTrunkMutation.mutate(id) }}
        />
      )}

      {activeTab === 'outbound' && (
        <OutboundTab
          routes={outboundRoutes}
          loading={loadingOutbound}
          onAdd={() => { setEditingOutbound(null); setShowOutboundModal(true) }}
          onEdit={(r) => { setEditingOutbound(r); setShowOutboundModal(true) }}
          onDelete={(id) => deleteOutboundMutation.mutate(id)}
        />
      )}

      {activeTab === 'inbound' && (
        <InboundTab
          routes={inboundRoutes}
          loading={loadingInbound}
          onAdd={() => { setEditingInbound(null); setShowInboundModal(true) }}
          onEdit={(r) => { setEditingInbound(r); setShowInboundModal(true) }}
          onDelete={(id) => deleteInboundMutation.mutate(id)}
        />
      )}

      {/* Trunk Modal */}
      {showTrunkModal && (
        <TrunkModal
          trunk={editingTrunk}
          onClose={() => { setShowTrunkModal(false); setEditingTrunk(null) }}
          onSave={(data) => {
            if (editingTrunk) {
              updateTrunkMutation.mutate({ id: editingTrunk.id, data })
            } else {
              createTrunkMutation.mutate(data as CreateTrunkRequest)
            }
          }}
          isSaving={createTrunkMutation.isPending || updateTrunkMutation.isPending}
        />
      )}

      {/* Group Modal */}
      {showGroupModal && (
        <GroupModal
          group={editingGroup}
          trunks={trunks}
          onClose={() => { setShowGroupModal(false); setEditingGroup(null) }}
          onSave={(data) => {
            if (editingGroup) {
              updateGroupMutation.mutate({ id: editingGroup.id, data })
            } else {
              createGroupMutation.mutate(data as CreateTrunkGroupRequest)
            }
          }}
          isSaving={createGroupMutation.isPending || updateGroupMutation.isPending}
        />
      )}

      {/* Outbound Modal */}
      {showOutboundModal && (
        <OutboundModal
          route={editingOutbound}
          trunks={trunks}
          trunkGroups={trunkGroups}
          onClose={() => { setShowOutboundModal(false); setEditingOutbound(null) }}
          onSave={(data) => {
            if (editingOutbound) {
              updateOutboundMutation.mutate({ id: editingOutbound.id, data })
            } else {
              createOutboundMutation.mutate(data as CreateOutboundRouteRequest)
            }
          }}
          isSaving={createOutboundMutation.isPending || updateOutboundMutation.isPending}
        />
      )}

      {/* Inbound Modal */}
      {showInboundModal && (
        <InboundModal
          route={editingInbound}
          trunks={trunks.filter(t => t.trunk_type === 'private')}
          onClose={() => { setShowInboundModal(false); setEditingInbound(null) }}
          onSave={(data) => {
            if (editingInbound) {
              updateInboundMutation.mutate({ id: editingInbound.id, data })
            } else {
              createInboundMutation.mutate(data as CreateInboundRouteRequest)
            }
          }}
          isSaving={createInboundMutation.isPending || updateInboundMutation.isPending}
        />
      )}

      {/* Migrate Modal */}
      {showMigrateModal && (
        <MigrateModal
          onClose={() => setShowMigrateModal(false)}
          onMigrate={() => migrateMutation.mutate()}
          isMigrating={migrateMutation.isPending}
          result={migrateMutation.data}
        />
      )}
    </div>
  )
}

// ============== Trunks Tab ==============

interface TrunksTabProps {
  trunks: RoutingTrunk[]
  loading: boolean
  onAddTrunk: () => void
  onEditTrunk: (trunk: RoutingTrunk) => void
  onDeleteTrunk: (id: string) => void
}

function TrunksTab({ trunks, loading, onAddTrunk, onEditTrunk, onDeleteTrunk }: TrunksTabProps) {
  const [sipStatusModal, setSipStatusModal] = useState<SipStatusCheck | null>(null)
  const [checkingTrunkId, setCheckingTrunkId] = useState<string | null>(null)
  const [checkingAll, setCheckingAll] = useState(false)
  const queryClient = useQueryClient()

  const handleCheckSipStatus = async (trunkId: string) => {
    setCheckingTrunkId(trunkId)
    try {
      const result = await checkTrunkSipStatus(trunkId)
      setSipStatusModal(result)
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
    } catch (error) {
      console.error('Error checking SIP status:', error)
    } finally {
      setCheckingTrunkId(null)
    }
  }

  const handleCheckAllSipStatus = async () => {
    setCheckingAll(true)
    try {
      await checkAllTrunksSipStatus()
      queryClient.invalidateQueries({ queryKey: ['routing-trunks'] })
    } catch (error) {
      console.error('Error checking all SIP statuses:', error)
    } finally {
      setCheckingAll(false)
    }
  }

  const trunkColumns = [
    { key: 'name', header: 'Nombre' },
    {
      key: 'trunk_type',
      header: 'Tipo',
      render: (trunk: RoutingTrunk) => (
        <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-medium ${
          trunk.trunk_type === 'private'
            ? 'bg-violet-500/15 text-violet-600 dark:text-violet-400'
            : 'bg-sky-500/15 text-sky-600 dark:text-sky-400'
        }`}>
          {trunk.trunk_type === 'private' ? (
            <><Building2 className="w-3 h-3" /> Privada</>
          ) : (
            <><Globe className="w-3 h-3" /> Pública</>
          )}
        </span>
      ),
    },
    {
      key: 'host',
      header: 'Host:Puerto',
      render: (trunk: RoutingTrunk) => (
        <span className="font-mono text-sm text-[var(--color-text-secondary)]">
          {trunk.host}:{trunk.port ?? 5060}
        </span>
      ),
    },
    {
      key: 'transport',
      header: 'Transporte',
      render: (trunk: RoutingTrunk) => (
        <span className="px-2 py-0.5 rounded-md bg-[var(--color-bg-tertiary)] text-xs font-medium text-[var(--color-text-secondary)] uppercase">
          {trunk.transport}
        </span>
      ),
    },
    {
      key: 'sip_status',
      header: 'SIP',
      render: (trunk: RoutingTrunk) => (
        <SipStatusBadge
          status={trunk.sip_status}
          latencyMs={trunk.last_options_latency_ms}
          responseCode={trunk.last_options_response_code}
          isChecking={checkingTrunkId === trunk.id}
          onCheck={() => handleCheckSipStatus(trunk.id)}
        />
      ),
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (trunk: RoutingTrunk) => (
        <span className={`px-2 py-0.5 rounded-md text-xs font-medium ${
          trunk.enabled ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400' : 'bg-red-500/15 text-red-600 dark:text-red-400'
        }`}>
          {trunk.enabled ? 'Activo' : 'Inactivo'}
        </span>
      ),
    },
    {
      key: 'sync_status',
      header: 'Sync',
      render: (trunk: RoutingTrunk) => (
        <SyncBadgeWithError
          status={trunk.sync_status ?? 'pending'}
          error={trunk.sync_error}
          trunkType={trunk.trunk_type}
        />
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (trunk: RoutingTrunk) => (
        <div className="flex gap-1">
          <button
            onClick={(e) => { e.stopPropagation(); handleCheckSipStatus(trunk.id); }}
            className="p-1.5 hover:bg-emerald-500/10 rounded-md text-[var(--color-text-tertiary)] hover:text-emerald-500 transition-colors cursor-pointer"
            title="Monitorear conexión SIP (OPTIONS)"
          >
            <Eye className="w-4 h-4" />
          </button>
          <button onClick={(e) => { e.stopPropagation(); onEditTrunk(trunk); }} className="p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md text-[var(--color-text-tertiary)] hover:text-[var(--color-primary)] transition-colors cursor-pointer">
            <Server className="w-4 h-4" />
          </button>
          <button onClick={(e) => { e.stopPropagation(); onDeleteTrunk(trunk.id); }} className="p-1.5 hover:bg-red-500/10 rounded-md text-[var(--color-text-tertiary)] hover:text-red-500 transition-colors cursor-pointer">
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      {/* Trunks */}
      <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] overflow-hidden shadow-sm">
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)]">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-[var(--color-primary)]/10 rounded-lg">
              <Server className="w-4 h-4 text-[var(--color-primary)]" />
            </div>
            <h3 className="font-semibold text-[var(--color-text-primary)]">Trunks</h3>
            <span className="px-2 py-0.5 rounded-md bg-[var(--color-bg-tertiary)] text-xs font-medium text-[var(--color-text-tertiary)]">
              {trunks.length}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleCheckAllSipStatus}
              disabled={checkingAll}
              className="px-3 py-2 rounded-lg bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-500/20 transition-colors flex items-center gap-2 text-sm font-medium cursor-pointer disabled:opacity-50"
            >
              <Activity className={`w-4 h-4 ${checkingAll ? 'animate-pulse' : ''}`} />
              {checkingAll ? 'Verificando...' : 'Verificar SIP'}
            </button>
            <button
              onClick={onAddTrunk}
              className="px-3 py-2 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)] hover:bg-[var(--color-primary)]/20 transition-colors flex items-center gap-2 text-sm font-medium cursor-pointer"
            >
              <Plus className="w-4 h-4" />
              Nuevo Trunk
            </button>
          </div>
        </div>
        <DataTable
          data={trunks}
          columns={trunkColumns}
          loading={loading}
        />
      </div>

      {/* SIP Status Detail Modal */}
      {sipStatusModal && (
        <SipStatusDetailModal
          status={sipStatusModal}
          onClose={() => setSipStatusModal(null)}
        />
      )}
    </div>
  )
}

// ============== Outbound Tab ==============

interface OutboundTabProps {
  routes: RoutingOutboundRoute[]
  loading: boolean
  onAdd: () => void
  onEdit: (route: RoutingOutboundRoute) => void
  onDelete: (id: string) => void
}

function OutboundTab({ routes, loading, onAdd, onEdit, onDelete }: OutboundTabProps) {
  const columns = [
    {
      key: 'prefix_pattern',
      header: 'Prefijo',
      render: (route: RoutingOutboundRoute) => (
        <span className="font-mono text-sm font-medium text-[var(--color-text-primary)]">
          {route.prefix_pattern}
        </span>
      ),
    },
    { key: 'name', header: 'Nombre' },
    {
      key: 'destination',
      header: 'Destino',
      render: (route: RoutingOutboundRoute) => (
        <span className="text-sm text-[var(--color-text-secondary)]">
          {route.trunk_group_name || route.trunk_name || '-'}
        </span>
      ),
    },
    {
      key: 'priority',
      header: 'Prioridad',
      render: (route: RoutingOutboundRoute) => (
        <span className="px-2 py-0.5 rounded-md bg-[var(--color-bg-tertiary)] text-xs font-medium text-[var(--color-text-secondary)]">
          {route.priority}
        </span>
      ),
    },
    {
      key: 'time_schedule_enabled',
      header: 'Horario',
      render: (route: RoutingOutboundRoute) => route.time_schedule_enabled ? (
        <span className="px-2 py-0.5 rounded-md bg-amber-500/15 text-xs font-medium text-amber-600 dark:text-amber-400">
          {route.time_schedule}
        </span>
      ) : (
        <span className="text-[var(--color-text-muted)]">-</span>
      ),
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (route: RoutingOutboundRoute) => (
        <span className={`px-2 py-0.5 rounded-md text-xs font-medium ${
          route.enabled ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400' : 'bg-red-500/15 text-red-600 dark:text-red-400'
        }`}>
          {route.enabled ? 'Activo' : 'Inactivo'}
        </span>
      ),
    },
    {
      key: 'sync_status',
      header: 'Sync',
      render: (route: RoutingOutboundRoute) => <SyncBadge status={route.sync_status ?? 'pending'} />,
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (route: RoutingOutboundRoute) => (
        <div className="flex gap-1">
          <button onClick={(e) => { e.stopPropagation(); onEdit(route); }} className="p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md text-[var(--color-text-tertiary)] hover:text-[var(--color-primary)] transition-colors cursor-pointer">
            <Route className="w-4 h-4" />
          </button>
          <button onClick={(e) => { e.stopPropagation(); onDelete(route.id); }} className="p-1.5 hover:bg-red-500/10 rounded-md text-[var(--color-text-tertiary)] hover:text-red-500 transition-colors cursor-pointer">
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] overflow-hidden shadow-sm">
      <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)]">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-[var(--color-primary)]/10 rounded-lg">
            <Route className="w-4 h-4 text-[var(--color-primary)]" />
          </div>
          <h3 className="font-semibold text-[var(--color-text-primary)]">Rutas Salientes</h3>
          <span className="px-2 py-0.5 rounded-md bg-[var(--color-bg-tertiary)] text-xs font-medium text-[var(--color-text-tertiary)]">
            {routes.length}
          </span>
        </div>
        <button
          onClick={onAdd}
          className="px-3 py-2 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)] hover:bg-[var(--color-primary)]/20 transition-colors flex items-center gap-2 text-sm font-medium cursor-pointer"
        >
          <Plus className="w-4 h-4" />
          Nueva Ruta
        </button>
      </div>
      <DataTable
        data={routes}
        columns={columns}
        loading={loading}
      />
    </div>
  )
}

// ============== Inbound Tab ==============

interface InboundTabProps {
  routes: RoutingInboundRoute[]
  loading: boolean
  onAdd: () => void
  onEdit: (route: RoutingInboundRoute) => void
  onDelete: (id: string) => void
}

function InboundTab({ routes, loading, onAdd, onEdit, onDelete }: InboundTabProps) {
  const columns = [
    { key: 'did_pattern', header: 'DID' },
    { key: 'name', header: 'Nombre' },
    {
      key: 'destination',
      header: 'Destino',
      render: (route: RoutingInboundRoute) => (
        route.destination_trunk_name ? (
          <span className="flex items-center gap-1.5">
            <Server className="w-3.5 h-3.5 text-[var(--color-primary)]" />
            <span className="font-medium text-[var(--color-primary)]">{route.destination_trunk_name}</span>
          </span>
        ) : (
          <span className="text-[var(--color-text-secondary)]">{route.destination_host}:{route.destination_port ?? 5060}</span>
        )
      ),
    },
    { key: 'destination_profile', header: 'Perfil' },
    { key: 'priority', header: 'Prioridad' },
    {
      key: 'enabled',
      header: 'Estado',
      render: (route: RoutingInboundRoute) => (
        <span className={`px-2 py-0.5 rounded-md text-xs font-medium ${
          route.enabled ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400' : 'bg-red-500/15 text-red-600 dark:text-red-400'
        }`}>
          {route.enabled ? 'Activo' : 'Inactivo'}
        </span>
      ),
    },
    {
      key: 'sync_status',
      header: 'Sync',
      render: (route: RoutingInboundRoute) => <SyncBadge status={route.sync_status ?? 'pending'} />,
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (route: RoutingInboundRoute) => (
        <div className="flex gap-2">
          <button onClick={(e) => { e.stopPropagation(); onEdit(route); }} className="p-1.5 hover:bg-[var(--color-bg-tertiary)] rounded-md text-[var(--color-text-tertiary)] hover:text-[var(--color-primary)] transition-colors cursor-pointer">
            <ArrowDownToLine className="w-4 h-4" />
          </button>
          <button onClick={(e) => { e.stopPropagation(); onDelete(route.id); }} className="p-1.5 hover:bg-red-500/10 rounded-md text-[var(--color-text-tertiary)] hover:text-red-500 transition-colors cursor-pointer">
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] overflow-hidden shadow-sm">
      <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)]">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-[var(--color-primary)]/10 rounded-lg">
            <ArrowDownToLine className="w-4 h-4 text-[var(--color-primary)]" />
          </div>
          <h3 className="font-semibold text-[var(--color-text-primary)]">Rutas Entrantes</h3>
          <span className="px-2 py-0.5 rounded-md bg-[var(--color-bg-tertiary)] text-xs font-medium text-[var(--color-text-tertiary)]">
            {routes.length}
          </span>
        </div>
        <button
          onClick={onAdd}
          className="px-3 py-2 rounded-lg bg-[var(--color-primary)]/10 text-[var(--color-primary)] hover:bg-[var(--color-primary)]/20 transition-colors flex items-center gap-2 text-sm font-medium cursor-pointer"
        >
          <Plus className="w-4 h-4" />
          Nueva Ruta
        </button>
      </div>
      <DataTable
        data={routes}
        columns={columns}
        loading={loading}
      />
    </div>
  )
}

// ============== Sync Badge ==============

function SyncBadge({ status }: { status: string }) {
  const styles = {
    synced: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400',
    pending: 'bg-amber-500/15 text-amber-600 dark:text-amber-400',
    error: 'bg-red-500/15 text-red-600 dark:text-red-400',
  }
  const icons = {
    synced: <CheckCircle className="w-3.5 h-3.5" />,
    pending: <Clock className="w-3.5 h-3.5" />,
    error: <AlertCircle className="w-3.5 h-3.5" />,
  }
  const style = styles[status as keyof typeof styles] || styles.pending
  const icon = icons[status as keyof typeof icons] || icons.pending

  return (
    <span className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium ${style}`}>
      {icon}
      <span className="capitalize">{status === 'synced' ? 'Sync' : status}</span>
    </span>
  )
}

// ============== Trunk Modal ==============

interface TrunkModalProps {
  trunk: RoutingTrunk | null
  onClose: () => void
  onSave: (data: CreateTrunkRequest | Partial<CreateTrunkRequest>) => void
  isSaving: boolean
}

function TrunkModal({ trunk, onClose, onSave, isSaving }: TrunkModalProps) {
  const [name, setName] = useState(trunk?.name || '')
  const [host, setHost] = useState(trunk?.host || '')
  const [port, setPort] = useState(trunk?.port || 5060)
  const [transport, setTransport] = useState(trunk?.transport || 'udp')
  const [trunkType, setTrunkType] = useState<TrunkType>(trunk?.trunk_type || 'public')
  const [authUsername, setAuthUsername] = useState(trunk?.auth_username || '')
  const [authPassword, setAuthPassword] = useState('')
  const [stripDigits, setStripDigits] = useState(trunk?.strip_digits || 0)
  const [prefixToAdd, setPrefixToAdd] = useState(trunk?.prefix_to_add || '')
  const [enabled, setEnabled] = useState(trunk?.enabled ?? true)
  const [description, setDescription] = useState(trunk?.description || '')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave({
      name,
      host,
      port,
      transport,
      trunk_type: trunkType,
      auth_username: authUsername || undefined,
      auth_password: authPassword || undefined,
      strip_digits: stripDigits,
      prefix_to_add: prefixToAdd,
      enabled,
      description: description || undefined,
    })
  }

  return (
    <Modal title={trunk ? 'Editar Trunk' : 'Nuevo Trunk'} onClose={onClose}>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <FormField label="Nombre" required>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              required
            />
          </FormField>

          <FormField label="Host" required>
            <input
              type="text"
              value={host}
              onChange={(e) => setHost(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              placeholder="10.10.22.18"
              required
            />
          </FormField>

          <FormField label="Puerto">
            <input
              type="number"
              value={port}
              onChange={(e) => setPort(parseInt(e.target.value) || 5060)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="Transporte">
            <select
              value={transport}
              onChange={(e) => setTransport(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            >
              <option value="udp">UDP</option>
              <option value="tcp">TCP</option>
              <option value="tls">TLS</option>
            </select>
          </FormField>
        </div>

        {/* Trunk Type Selection */}
        <div className="p-4 bg-[var(--color-bg-secondary)] rounded-xl border border-[var(--color-border-primary)]">
          <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-3">
            Tipo de Troncal
          </label>
          <div className="grid grid-cols-2 gap-3">
            <button
              type="button"
              onClick={() => setTrunkType('private')}
              className={`p-4 rounded-xl border-2 transition-all cursor-pointer ${
                trunkType === 'private'
                  ? 'border-violet-500 bg-violet-500/10'
                  : 'border-[var(--color-border-primary)] hover:border-violet-500/50'
              }`}
            >
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-lg ${trunkType === 'private' ? 'bg-violet-500/20' : 'bg-[var(--color-bg-tertiary)]'}`}>
                  <Building2 className={`w-5 h-5 ${trunkType === 'private' ? 'text-violet-500' : 'text-[var(--color-text-tertiary)]'}`} />
                </div>
                <div className="text-left">
                  <div className={`font-semibold ${trunkType === 'private' ? 'text-violet-600 dark:text-violet-400' : 'text-[var(--color-text-primary)]'}`}>
                    Privada
                  </div>
                  <div className="text-xs text-[var(--color-text-tertiary)]">
                    Red interna → FreeSWITCH
                  </div>
                </div>
              </div>
            </button>

            <button
              type="button"
              onClick={() => setTrunkType('public')}
              className={`p-4 rounded-xl border-2 transition-all cursor-pointer ${
                trunkType === 'public'
                  ? 'border-sky-500 bg-sky-500/10'
                  : 'border-[var(--color-border-primary)] hover:border-sky-500/50'
              }`}
            >
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-lg ${trunkType === 'public' ? 'bg-sky-500/20' : 'bg-[var(--color-bg-tertiary)]'}`}>
                  <Globe className={`w-5 h-5 ${trunkType === 'public' ? 'text-sky-500' : 'text-[var(--color-text-tertiary)]'}`} />
                </div>
                <div className="text-left">
                  <div className={`font-semibold ${trunkType === 'public' ? 'text-sky-600 dark:text-sky-400' : 'text-[var(--color-text-primary)]'}`}>
                    Pública
                  </div>
                  <div className="text-xs text-[var(--color-text-tertiary)]">
                    Operadores externos → Kamailio
                  </div>
                </div>
              </div>
            </button>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <FormField label="Usuario Auth">
            <input
              type="text"
              value={authUsername}
              onChange={(e) => setAuthUsername(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="Password Auth">
            <input
              type="password"
              value={authPassword}
              onChange={(e) => setAuthPassword(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              placeholder={trunk ? '(sin cambios)' : ''}
            />
          </FormField>

          <FormField label="Strip Digits">
            <input
              type="number"
              value={stripDigits}
              onChange={(e) => setStripDigits(parseInt(e.target.value) || 0)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              min={0}
            />
          </FormField>

          <FormField label="Prefijo a Agregar">
            <input
              type="text"
              value={prefixToAdd}
              onChange={(e) => setPrefixToAdd(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>
        </div>

        <FormField label="Descripción">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            rows={2}
          />
        </FormField>

        <div className="flex items-center gap-2">
          <input
            type="checkbox"
            id="enabled"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
            className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
          />
          <label htmlFor="enabled" className="text-sm text-[var(--color-text-secondary)]">Habilitado</label>
        </div>

        <ModalFooter onClose={onClose} isSaving={isSaving} />
      </form>
    </Modal>
  )
}

// ============== Group Modal ==============

interface GroupModalProps {
  group: RoutingTrunkGroupWithMembers | null
  trunks: RoutingTrunk[]
  onClose: () => void
  onSave: (data: CreateTrunkGroupRequest | Partial<CreateTrunkGroupRequest>) => void
  isSaving: boolean
}

function GroupModal({ group, trunks, onClose, onSave, isSaving }: GroupModalProps) {
  const [name, setName] = useState(group?.name || '')
  const [description, setDescription] = useState(group?.description || '')
  const [strategy, setStrategy] = useState(group?.failover_strategy || 'sequential')
  const [members, setMembers] = useState<TrunkGroupMemberInput[]>(
    group?.members?.map(m => ({
      trunk_id: m.trunk_id,
      priority: m.priority,
      weight: m.weight,
      max_channels: m.max_channels,
    })) || []
  )

  const addMember = () => {
    const availableTrunk = trunks.find(t => !members.some(m => m.trunk_id === t.id))
    if (availableTrunk) {
      setMembers([...members, { trunk_id: availableTrunk.id, priority: members.length + 1, weight: 100 }])
    }
  }

  const removeMember = (idx: number) => {
    setMembers(members.filter((_, i) => i !== idx))
  }

  const updateMember = (idx: number, field: keyof TrunkGroupMemberInput, value: unknown) => {
    setMembers(members.map((m, i) => i === idx ? { ...m, [field]: value } : m))
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave({
      name,
      description: description || undefined,
      failover_strategy: strategy,
      members,
    })
  }

  return (
    <Modal title={group ? 'Editar Grupo' : 'Nuevo Grupo de Failover'} onClose={onClose} wide>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <FormField label="Nombre" required>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              required
            />
          </FormField>

          <FormField label="Estrategia">
            <select
              value={strategy}
              onChange={(e) => setStrategy(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            >
              <option value="sequential">Secuencial</option>
              <option value="round_robin">Round Robin</option>
              <option value="weighted">Por Peso</option>
              <option value="least_calls">Menos Ocupado</option>
            </select>
          </FormField>
        </div>

        <FormField label="Descripción">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            rows={2}
          />
        </FormField>

        {/* Members */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm font-medium text-[var(--color-text-secondary)]">Trunks del Grupo</label>
            <button
              type="button"
              onClick={addMember}
              disabled={members.length >= trunks.length}
              className="px-2.5 py-1.5 rounded-md bg-[var(--color-primary)]/10 text-[var(--color-primary)] hover:bg-[var(--color-primary)]/20 text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
            >
              <Plus className="w-3 h-3 inline mr-1" />
              Agregar
            </button>
          </div>

          {members.length === 0 ? (
            <p className="text-sm text-[var(--color-text-muted)] italic">Sin trunks asignados</p>
          ) : (
            <div className="space-y-2">
              {members.map((member, idx) => (
                <div key={idx} className="flex items-center gap-2 p-2 bg-[var(--color-bg-secondary)] rounded-lg border border-[var(--color-border-primary)]">
                  <select
                    value={member.trunk_id}
                    onChange={(e) => updateMember(idx, 'trunk_id', e.target.value)}
                    className="flex-1 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-md text-[var(--color-text-primary)] text-sm focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                  >
                    {trunks.map(t => (
                      <option key={t.id} value={t.id} disabled={members.some((m, i) => i !== idx && m.trunk_id === t.id)}>
                        {t.name} ({t.host}:{t.port})
                      </option>
                    ))}
                  </select>

                  {(strategy === 'sequential' || strategy === 'least_calls') && (
                    <input
                      type="number"
                      value={member.priority}
                      onChange={(e) => updateMember(idx, 'priority', parseInt(e.target.value) || 1)}
                      className="w-20 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-md text-[var(--color-text-primary)] text-sm focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                      placeholder="Prio"
                      title="Prioridad"
                    />
                  )}

                  {strategy === 'weighted' && (
                    <input
                      type="number"
                      value={member.weight}
                      onChange={(e) => updateMember(idx, 'weight', parseInt(e.target.value) || 100)}
                      className="w-20 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-md text-[var(--color-text-primary)] text-sm focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                      placeholder="Peso"
                      title="Peso %"
                    />
                  )}

                  <button
                    type="button"
                    onClick={() => removeMember(idx)}
                    className="p-1.5 text-red-500/70 hover:text-red-500 hover:bg-red-500/10 rounded-md transition-colors cursor-pointer"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <ModalFooter onClose={onClose} isSaving={isSaving} />
      </form>
    </Modal>
  )
}

// ============== Outbound Modal ==============

interface OutboundModalProps {
  route: RoutingOutboundRoute | null
  trunks: RoutingTrunk[]
  trunkGroups: RoutingTrunkGroup[]
  onClose: () => void
  onSave: (data: CreateOutboundRouteRequest | Partial<CreateOutboundRouteRequest>) => void
  isSaving: boolean
}

function OutboundModal({ route, trunks, trunkGroups, onClose, onSave, isSaving }: OutboundModalProps) {
  const [name, setName] = useState(route?.name || '')
  const [prefix, setPrefix] = useState(route?.prefix_pattern || '')
  const [priority, setPriority] = useState(route?.priority || 100)
  const [destinationType, setDestinationType] = useState<'trunk' | 'group'>(
    route?.trunk_group_id ? 'group' : 'trunk'
  )
  const [trunkId, setTrunkId] = useState(route?.trunk_id || (trunks[0]?.id || ''))
  const [groupId, setGroupId] = useState(route?.trunk_group_id || (trunkGroups[0]?.id || ''))
  const [timeEnabled, setTimeEnabled] = useState(route?.time_schedule_enabled || false)
  const [timeSchedule, setTimeSchedule] = useState(route?.time_schedule || '')
  const [enabled, setEnabled] = useState(route?.enabled ?? true)
  const [description, setDescription] = useState(route?.description || '')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave({
      name,
      prefix_pattern: prefix,
      priority,
      trunk_id: destinationType === 'trunk' ? trunkId : undefined,
      trunk_group_id: destinationType === 'group' ? groupId : undefined,
      time_schedule: timeEnabled ? timeSchedule : undefined,
      time_schedule_enabled: timeEnabled,
      enabled,
      description: description || undefined,
    })
  }

  return (
    <Modal title={route ? 'Editar Ruta Saliente' : 'Nueva Ruta Saliente'} onClose={onClose}>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <FormField label="Nombre" required>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              required
            />
          </FormField>

          <FormField label="Prefijo" required>
            <input
              type="text"
              value={prefix}
              onChange={(e) => setPrefix(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              placeholder="54, 1, 0800"
              required
            />
          </FormField>

          <FormField label="Prioridad">
            <input
              type="number"
              value={priority}
              onChange={(e) => setPriority(parseInt(e.target.value) || 100)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="Tipo de Destino">
            <select
              value={destinationType}
              onChange={(e) => setDestinationType(e.target.value as 'trunk' | 'group')}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            >
              <option value="trunk">Trunk Individual</option>
              <option value="group">Grupo de Failover</option>
            </select>
          </FormField>

          {destinationType === 'trunk' && (
            <FormField label="Trunk">
              <select
                value={trunkId}
                onChange={(e) => setTrunkId(e.target.value)}
                className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              >
                {trunks.map(t => (
                  <option key={t.id} value={t.id}>{t.name}</option>
                ))}
              </select>
            </FormField>
          )}

          {destinationType === 'group' && (
            <FormField label="Grupo">
              <select
                value={groupId}
                onChange={(e) => setGroupId(e.target.value)}
                className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              >
                {trunkGroups.map(g => (
                  <option key={g.id} value={g.id}>{g.name}</option>
                ))}
              </select>
            </FormField>
          )}
        </div>

        <FormField label="Descripción">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            rows={2}
          />
        </FormField>

        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="timeEnabled"
              checked={timeEnabled}
              onChange={(e) => setTimeEnabled(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="timeEnabled" className="text-sm text-[var(--color-text-secondary)]">Aplicar solo en horario</label>
          </div>

          {timeEnabled && (
            <FormField label="Horario (formato timerec)">
              <input
                type="text"
                value={timeSchedule}
                onChange={(e) => setTimeSchedule(e.target.value)}
                className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                placeholder="* * * * 1-5 09:00-18:00"
              />
              <p className="text-xs text-[var(--color-text-muted)] mt-1">Ej: * * * * 1-5 09:00-18:00 (Lun-Vie 9am-6pm)</p>
            </FormField>
          )}
        </div>

        <div className="flex items-center gap-2">
          <input
            type="checkbox"
            id="enabled"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
            className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
          />
          <label htmlFor="enabled" className="text-sm text-[var(--color-text-secondary)]">Habilitado</label>
        </div>

        <ModalFooter onClose={onClose} isSaving={isSaving} />
      </form>
    </Modal>
  )
}

// ============== Inbound Modal ==============

interface InboundModalProps {
  route: RoutingInboundRoute | null
  trunks: RoutingTrunk[]
  onClose: () => void
  onSave: (data: CreateInboundRouteRequest | Partial<CreateInboundRouteRequest>) => void
  isSaving: boolean
}

function InboundModal({ route, trunks, onClose, onSave, isSaving }: InboundModalProps) {
  const [name, setName] = useState(route?.name || '')
  const [didPattern, setDidPattern] = useState(route?.did_pattern || '^(.+)$')
  const [sourceIp, setSourceIp] = useState(route?.source_ip_pattern || '')
  const [priority, setPriority] = useState(route?.priority || 100)
  const [destHost, setDestHost] = useState(route?.destination_host || '')
  const [destPort, setDestPort] = useState(route?.destination_port || 5060)
  const [destProfile, setDestProfile] = useState(route?.destination_profile || 'internal')
  const [callTimeout, setCallTimeout] = useState(route?.call_timeout || 120)
  const [inheritCodec, setInheritCodec] = useState(route?.inherit_codec ?? true)
  const [ignoreEarlyMedia, setIgnoreEarlyMedia] = useState(route?.ignore_early_media || false)
  const [bypassMedia, setBypassMedia] = useState(route?.bypass_media || false)
  const [stripDigits, setStripDigits] = useState(route?.strip_digits || 0)
  const [prefixToAdd, setPrefixToAdd] = useState(route?.prefix_to_add || '')
  const [destinationTrunkId, setDestinationTrunkId] = useState<string | null>(route?.destination_trunk_id || null)
  const [sendEarlyMedia, setSendEarlyMedia] = useState(route?.send_early_media || false)
  const [enabled, setEnabled] = useState(route?.enabled ?? true)
  const [description, setDescription] = useState(route?.description || '')
  const [failover, setFailover] = useState<FailoverDestination[]>(route?.failover_destinations || [])

  const addFailover = () => {
    setFailover([...failover, { host: '', port: 5060 }])
  }

  const removeFailover = (idx: number) => {
    setFailover(failover.filter((_, i) => i !== idx))
  }

  const updateFailover = (idx: number, field: keyof FailoverDestination, value: unknown) => {
    setFailover(failover.map((f, i) => i === idx ? { ...f, [field]: value } : f))
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave({
      name,
      did_pattern: didPattern,
      source_ip_pattern: sourceIp || undefined,
      priority,
      destination_host: destHost,
      destination_port: destPort,
      destination_profile: destProfile,
      call_timeout: callTimeout,
      inherit_codec: inheritCodec,
      ignore_early_media: ignoreEarlyMedia,
      bypass_media: bypassMedia,
      strip_digits: stripDigits,
      prefix_to_add: prefixToAdd || undefined,
      destination_trunk_id: destinationTrunkId || undefined,
      send_early_media: sendEarlyMedia,
      failover_destinations: failover.filter(f => f.host),
      enabled,
      description: description || undefined,
    })
  }

  return (
    <Modal title={route ? 'Editar Ruta Entrante' : 'Nueva Ruta Entrante'} onClose={onClose} wide>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <FormField label="Nombre" required>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              required
            />
          </FormField>

          <FormField label="DID Pattern" required>
            <input
              type="text"
              value={didPattern}
              onChange={(e) => setDidPattern(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors font-mono"
              required
            />
          </FormField>

          <FormField label="Source IP Filter">
            <input
              type="text"
              value={sourceIp}
              onChange={(e) => setSourceIp(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors font-mono"
              placeholder="^10\.10\.22\.18$"
            />
          </FormField>

          <FormField label="Prioridad">
            <input
              type="number"
              value={priority}
              onChange={(e) => setPriority(parseInt(e.target.value) || 100)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="Destination Host" required>
            <input
              type="text"
              value={destHost}
              onChange={(e) => setDestHost(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              required
            />
          </FormField>

          <FormField label="Destination Port">
            <input
              type="number"
              value={destPort}
              onChange={(e) => setDestPort(parseInt(e.target.value) || 5060)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="SIP Profile">
            <select
              value={destProfile}
              onChange={(e) => setDestProfile(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            >
              <option value="internal">internal</option>
              <option value="external">external</option>
            </select>
          </FormField>

          <FormField label="Destino Trunk (PBX)" hint="Si se selecciona, usa gateway en vez de IP">
            <select
              value={destinationTrunkId || ''}
              onChange={(e) => setDestinationTrunkId(e.target.value || null)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            >
              <option value="">Softphone / IP directo</option>
              {trunks.map(t => (
                <option key={t.id} value={t.id}>
                  {t.name} ({t.host}:{t.port})
                </option>
              ))}
            </select>
          </FormField>

          <FormField label="Call Timeout (s)">
            <input
              type="number"
              value={callTimeout}
              onChange={(e) => setCallTimeout(parseInt(e.target.value) || 120)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            />
          </FormField>

          <FormField label="Strip Digits" hint="Dígitos a eliminar del inicio">
            <input
              type="number"
              min="0"
              value={stripDigits}
              onChange={(e) => setStripDigits(parseInt(e.target.value) || 0)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
              placeholder="0"
            />
          </FormField>

          <FormField label="Prefix to Add" hint="Prefijo a agregar después de strip">
            <input
              type="text"
              value={prefixToAdd}
              onChange={(e) => setPrefixToAdd(e.target.value)}
              className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors font-mono"
              placeholder=""
            />
          </FormField>
        </div>

        <FormField label="Descripción">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full px-3 py-2.5 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-lg text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
            rows={2}
          />
        </FormField>

        {/* Failover Destinations */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm font-medium text-[var(--color-text-secondary)]">Failover Destinations</label>
            <button
              type="button"
              onClick={addFailover}
              className="px-2.5 py-1.5 rounded-md bg-[var(--color-primary)]/10 text-[var(--color-primary)] hover:bg-[var(--color-primary)]/20 text-xs font-medium transition-colors cursor-pointer"
            >
              <Plus className="w-3 h-3 inline mr-1" />
              Agregar
            </button>
          </div>

          {failover.length === 0 ? (
            <p className="text-sm text-[var(--color-text-muted)] italic">Sin destinos de failover</p>
          ) : (
            <div className="space-y-2">
              {failover.map((fo, idx) => (
                <div key={idx} className="flex items-center gap-2">
                  <input
                    type="text"
                    value={fo.host}
                    onChange={(e) => updateFailover(idx, 'host', e.target.value)}
                    className="flex-1 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-md text-[var(--color-text-primary)] text-sm focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                    placeholder="Host"
                  />
                  <input
                    type="number"
                    value={fo.port}
                    onChange={(e) => updateFailover(idx, 'port', parseInt(e.target.value) || 5060)}
                    className="w-24 px-3 py-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-primary)] rounded-md text-[var(--color-text-primary)] text-sm focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/20 focus:outline-none transition-colors"
                    placeholder="Puerto"
                  />
                  <button
                    type="button"
                    onClick={() => removeFailover(idx)}
                    className="p-1.5 text-red-500/70 hover:text-red-500 hover:bg-red-500/10 rounded-md transition-colors cursor-pointer"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex flex-wrap gap-4">
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="inheritCodec"
              checked={inheritCodec}
              onChange={(e) => setInheritCodec(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="inheritCodec" className="text-sm text-[var(--color-text-secondary)]">Inherit Codec</label>
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="ignoreEarlyMedia"
              checked={ignoreEarlyMedia}
              onChange={(e) => setIgnoreEarlyMedia(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="ignoreEarlyMedia" className="text-sm text-[var(--color-text-secondary)]">Ignore Early Media</label>
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="bypassMedia"
              checked={bypassMedia}
              onChange={(e) => setBypassMedia(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="bypassMedia" className="text-sm text-[var(--color-text-secondary)]">Bypass Media</label>
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="sendEarlyMedia"
              checked={sendEarlyMedia}
              onChange={(e) => setSendEarlyMedia(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="sendEarlyMedia" className="text-sm text-[var(--color-text-secondary)]">Send Early Media (183)</label>
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="enabled"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="rounded border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
            />
            <label htmlFor="enabled" className="text-sm text-[var(--color-text-secondary)]">Habilitado</label>
          </div>
        </div>

        <ModalFooter onClose={onClose} isSaving={isSaving} />
      </form>
    </Modal>
  )
}

// ============== Migrate Modal ==============

interface MigrateModalProps {
  onClose: () => void
  onMigrate: () => void
  isMigrating: boolean
  result?: {
    trunks_imported: number
    trunk_groups_imported: number
    outbound_routes_imported: number
    inbound_routes_imported: number
    warnings: string[]
    errors: string[]
  }
}

function MigrateModal({ onClose, onMigrate, isMigrating, result }: MigrateModalProps) {
  return (
    <Modal title="Migrar Rutas Existentes" onClose={onClose}>
      <div className="space-y-4">
        {!result ? (
          <>
            <p className="text-[var(--color-text-secondary)]">
              Esta operación importará las rutas existentes desde la configuración
              actual del sistema de telefonía.
            </p>

            <div className="p-4 bg-amber-500/10 border border-amber-500/30 rounded-xl">
              <div className="flex items-start gap-3">
                <AlertCircle className="w-5 h-5 text-amber-500 flex-shrink-0 mt-0.5" />
                <div>
                  <p className="text-amber-600 dark:text-amber-400 font-medium">Importante</p>
                  <p className="text-sm text-amber-600/80 dark:text-amber-400/80 mt-1">
                    Se importarán los gateways y extensiones configurados actualmente.
                    Los elementos existentes no serán duplicados.
                  </p>
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-3 pt-2">
              <button
                onClick={onClose}
                className="px-4 py-2.5 rounded-lg text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors cursor-pointer font-medium"
              >
                Cancelar
              </button>
              <button
                onClick={onMigrate}
                disabled={isMigrating}
                className="px-5 py-2.5 rounded-lg bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white hover:shadow-lg hover:shadow-[var(--color-primary)]/30 transition-all disabled:opacity-50 flex items-center gap-2 cursor-pointer font-medium"
              >
                {isMigrating && <RefreshCw className="w-4 h-4 animate-spin" />}
                {isMigrating ? 'Migrando...' : 'Iniciar Migración'}
              </button>
            </div>
          </>
        ) : (
          <>
            <div className="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-xl">
              <div className="flex items-center gap-2 text-emerald-600 dark:text-emerald-400 font-semibold mb-4">
                <CheckCircle className="w-5 h-5" />
                Migración Completada
              </div>
              <div className="grid grid-cols-2 gap-3 text-sm">
                <div className="text-[var(--color-text-secondary)]">Trunks importados:</div>
                <div className="text-[var(--color-text-primary)] font-semibold">{result.trunks_imported}</div>
                <div className="text-[var(--color-text-secondary)]">Grupos importados:</div>
                <div className="text-[var(--color-text-primary)] font-semibold">{result.trunk_groups_imported}</div>
                <div className="text-[var(--color-text-secondary)]">Rutas salientes:</div>
                <div className="text-[var(--color-text-primary)] font-semibold">{result.outbound_routes_imported}</div>
                <div className="text-[var(--color-text-secondary)]">Rutas entrantes:</div>
                <div className="text-[var(--color-text-primary)] font-semibold">{result.inbound_routes_imported}</div>
              </div>
            </div>

            {result.warnings.length > 0 && (
              <div className="p-4 bg-amber-500/10 border border-amber-500/30 rounded-xl">
                <p className="text-amber-600 dark:text-amber-400 font-medium text-sm mb-2">Advertencias:</p>
                <ul className="text-xs text-amber-600/80 dark:text-amber-400/80 list-disc list-inside space-y-1">
                  {result.warnings.map((w, i) => <li key={i}>{w}</li>)}
                </ul>
              </div>
            )}

            {result.errors.length > 0 && (
              <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-xl">
                <p className="text-red-600 dark:text-red-400 font-medium text-sm mb-2">Errores:</p>
                <ul className="text-xs text-red-600/80 dark:text-red-400/80 list-disc list-inside space-y-1">
                  {result.errors.map((e, i) => <li key={i}>{e}</li>)}
                </ul>
              </div>
            )}

            <div className="flex justify-end pt-2">
              <button
                onClick={onClose}
                className="px-5 py-2.5 rounded-lg bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white hover:shadow-lg hover:shadow-[var(--color-primary)]/30 transition-all cursor-pointer font-medium"
              >
                Cerrar
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  )
}

// ============== Shared Components ==============

interface ModalProps {
  title: string
  children: React.ReactNode
  onClose: () => void
  wide?: boolean
}

function Modal({ title, children, onClose, wide }: ModalProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" onClick={onClose} />
      <div className={`relative w-full bg-[var(--color-bg-card)] border border-[var(--color-border-primary)] rounded-2xl shadow-2xl ${wide ? 'max-w-2xl' : 'max-w-lg'}`}>
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--color-border-primary)]">
          <h2 className="text-lg font-semibold text-[var(--color-text-primary)]">{title}</h2>
          <button onClick={onClose} className="p-2 rounded-lg text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors cursor-pointer">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="px-6 py-5 max-h-[70vh] overflow-y-auto">
          {children}
        </div>
      </div>
    </div>
  )
}

interface FormFieldProps {
  label: string
  required?: boolean
  hint?: string
  children: React.ReactNode
}

function FormField({ label, required, hint, children }: FormFieldProps) {
  return (
    <div>
      <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1.5">
        {label}
        {required && <span className="text-red-500 ml-1">*</span>}
        {hint && <span className="text-xs text-[var(--color-text-muted)] ml-2 font-normal">({hint})</span>}
      </label>
      {children}
    </div>
  )
}

interface ModalFooterProps {
  onClose: () => void
  isSaving: boolean
}

function ModalFooter({ onClose, isSaving }: ModalFooterProps) {
  return (
    <div className="flex justify-end gap-3 pt-5 mt-2 border-t border-[var(--color-border-primary)]">
      <button
        type="button"
        onClick={onClose}
        className="px-4 py-2.5 rounded-lg text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] transition-colors cursor-pointer font-medium"
      >
        Cancelar
      </button>
      <button
        type="submit"
        disabled={isSaving}
        className="px-5 py-2.5 rounded-lg bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white hover:shadow-lg hover:shadow-[var(--color-primary)]/30 transition-all disabled:opacity-50 flex items-center gap-2 cursor-pointer font-medium"
      >
        {isSaving && <RefreshCw className="w-4 h-4 animate-spin" />}
        {isSaving ? 'Guardando...' : 'Guardar'}
      </button>
    </div>
  )
}

// ============== Sync Badge With Error ==============

interface SyncBadgeWithErrorProps {
  status: string
  error?: string | null
  trunkType?: string
}

function SyncBadgeWithError({ status, error, trunkType }: SyncBadgeWithErrorProps) {
  const [showError, setShowError] = useState(false)

  const styles = {
    synced: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400',
    pending: 'bg-amber-500/15 text-amber-600 dark:text-amber-400',
    error: 'bg-red-500/15 text-red-600 dark:text-red-400',
  }
  const icons = {
    synced: <CheckCircle className="w-3.5 h-3.5" />,
    pending: <Clock className="w-3.5 h-3.5" />,
    error: <AlertCircle className="w-3.5 h-3.5" />,
  }
  const style = styles[status as keyof typeof styles] || styles.pending
  const icon = icons[status as keyof typeof icons] || icons.pending

  const systemLabel = trunkType === 'private' ? 'FreeSWITCH' : 'Kamailio'

  return (
    <div className="relative">
      <button
        onClick={(e) => { e.stopPropagation(); if (error) setShowError(!showError); }}
        className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium ${style} ${error ? 'cursor-pointer hover:opacity-80' : ''}`}
        title={error || `Sincronizado con ${systemLabel}`}
      >
        {icon}
        <span className="capitalize">{status === 'synced' ? systemLabel : status}</span>
        {error && <Info className="w-3 h-3 ml-0.5" />}
      </button>

      {/* Error Popup */}
      {showError && error && (
        <div className="absolute z-50 top-full left-0 mt-1 w-72 p-3 bg-[var(--color-bg-card)] border border-red-500/30 rounded-lg shadow-xl">
          <div className="flex items-start gap-2">
            <AlertCircle className="w-4 h-4 text-red-500 flex-shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="text-xs font-medium text-red-600 dark:text-red-400 mb-1">Error de sincronización</p>
              <p className="text-xs text-[var(--color-text-secondary)] break-words">{error}</p>
              <p className="text-[10px] text-[var(--color-text-muted)] mt-2">
                Sistema: {systemLabel}
              </p>
            </div>
            <button
              onClick={(e) => { e.stopPropagation(); setShowError(false); }}
              className="p-1 hover:bg-[var(--color-bg-tertiary)] rounded text-[var(--color-text-tertiary)]"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

// ============== SIP Status Badge ==============

interface SipStatusBadgeProps {
  status?: string
  latencyMs?: number
  responseCode?: number
  isChecking: boolean
  onCheck: () => void
}

function SipStatusBadge({ status, latencyMs, responseCode, isChecking, onCheck }: SipStatusBadgeProps) {
  const getStatusConfig = () => {
    switch (status) {
      case 'reachable':
        return {
          icon: <Wifi className="w-3.5 h-3.5" />,
          bg: 'bg-emerald-500/15',
          text: 'text-emerald-600 dark:text-emerald-400',
          label: 'Conectado',
        }
      case 'unreachable':
        return {
          icon: <WifiOff className="w-3.5 h-3.5" />,
          bg: 'bg-red-500/15',
          text: 'text-red-600 dark:text-red-400',
          label: 'Sin conexión',
        }
      case 'checking':
        return {
          icon: <RefreshCw className="w-3.5 h-3.5 animate-spin" />,
          bg: 'bg-amber-500/15',
          text: 'text-amber-600 dark:text-amber-400',
          label: 'Verificando',
        }
      default:
        return {
          icon: <Activity className="w-3.5 h-3.5" />,
          bg: 'bg-[var(--color-bg-tertiary)]',
          text: 'text-[var(--color-text-tertiary)]',
          label: 'Sin verificar',
        }
    }
  }

  const config = getStatusConfig()

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={(e) => { e.stopPropagation(); onCheck(); }}
        disabled={isChecking}
        className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium ${config.bg} ${config.text} hover:opacity-80 transition-opacity cursor-pointer disabled:cursor-not-allowed`}
        title={`${config.label}${latencyMs ? ` (${latencyMs}ms)` : ''}${responseCode ? ` - ${responseCode}` : ''}`}
      >
        {isChecking ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : config.icon}
        <span>{config.label}</span>
        {latencyMs && status === 'reachable' && (
          <span className="text-[10px] opacity-70">{latencyMs}ms</span>
        )}
      </button>
    </div>
  )
}

// ============== SIP Status Detail Modal ==============

interface SipStatusDetailModalProps {
  status: SipStatusCheck
  onClose: () => void
}

function SipStatusDetailModal({ status, onClose }: SipStatusDetailModalProps) {
  const [activeView, setActiveView] = useState<'visual' | 'terminal'>('visual')

  // Generate command that was executed
  const getCommand = () => {
    if (status.trunk_type === 'private' || status.check_source === 'freeswitch') {
      const gwName = status.gateway_name || status.trunk_name.toLowerCase().replace(/[^a-z0-9]/g, '-')
      return `fs_cli -x "sofia status gateway ${gwName}"`
    } else {
      return `sipsak -s sip:${status.host}:${status.port} -v`
    }
  }

  return (
    <Modal title="Monitoreo SIP en Tiempo Real" onClose={onClose} wide>
      <div className="space-y-5">
        {/* Header Info */}
        <div className="flex items-center justify-between p-4 bg-[var(--color-bg-secondary)] rounded-xl border border-[var(--color-border-primary)]">
          <div className="flex items-center gap-4">
            <div className={`p-3 rounded-xl ${
              status.status === 'reachable' ? 'bg-emerald-500/15' : 'bg-red-500/15'
            }`}>
              {status.status === 'reachable' ? (
                <Wifi className="w-6 h-6 text-emerald-500" />
              ) : (
                <WifiOff className="w-6 h-6 text-red-500" />
              )}
            </div>
            <div>
              <h3 className="font-semibold text-[var(--color-text-primary)]">{status.trunk_name}</h3>
              <div className="flex items-center gap-2 mt-0.5">
                <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-medium ${
                  status.trunk_type === 'private'
                    ? 'bg-violet-500/15 text-violet-600 dark:text-violet-400'
                    : 'bg-sky-500/15 text-sky-600 dark:text-sky-400'
                }`}>
                  {status.trunk_type === 'private' ? <Building2 className="w-3 h-3" /> : <Globe className="w-3 h-3" />}
                  {status.trunk_type === 'private' ? 'Privada' : 'Pública'}
                </span>
                <span className="text-xs text-[var(--color-text-tertiary)]">
                  vía {status.trunk_type === 'private' || status.check_source === 'freeswitch' ? 'FreeSWITCH' : 'Kamailio'}
                </span>
              </div>
            </div>
          </div>

          <div className="text-right">
            <div className={`text-xl font-bold ${
              status.status === 'reachable' ? 'text-emerald-500' : 'text-red-500'
            }`}>
              {status.response_code || '---'}
            </div>
            {status.latency_ms && (
              <div className="text-sm text-[var(--color-text-tertiary)]">{status.latency_ms}ms</div>
            )}
          </div>
        </div>

        {/* Status Message */}
        <div className={`p-3 rounded-lg ${
          status.status === 'reachable'
            ? 'bg-emerald-500/10 border border-emerald-500/20'
            : 'bg-red-500/10 border border-red-500/20'
        }`}>
          <p className={`text-sm font-medium ${
            status.status === 'reachable' ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'
          }`}>
            {status.status_message}
          </p>
        </div>

        {/* View Toggle */}
        <div className="flex items-center gap-1 p-1 bg-[var(--color-bg-tertiary)] rounded-lg w-fit">
          <button
            onClick={() => setActiveView('visual')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-all cursor-pointer ${
              activeView === 'visual'
                ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)]'
            }`}
          >
            <MessageSquare className="w-4 h-4 inline mr-1.5" />
            Visual
          </button>
          <button
            onClick={() => setActiveView('terminal')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-all cursor-pointer ${
              activeView === 'terminal'
                ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)]'
            }`}
          >
            <Terminal className="w-4 h-4 inline mr-1.5" />
            Terminal
          </button>
        </div>

        {/* Terminal View */}
        {activeView === 'terminal' && (
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-[var(--color-text-secondary)]">
              Comando ejecutado en Apolo SBC:
            </h4>
            <div className="bg-slate-900 rounded-xl p-4 font-mono text-sm overflow-x-auto">
              <div className="flex items-center gap-2 text-slate-400 mb-3">
                <span className="text-emerald-400">apolo@sbc</span>
                <span>:</span>
                <span className="text-sky-400">~</span>
                <span>$</span>
              </div>
              <div className="text-emerald-300 mb-4">{getCommand()}</div>

              {status.status === 'reachable' ? (
                <div className="space-y-1 text-slate-300">
                  <div className="text-sky-400">--- Enviando OPTIONS ---</div>
                  <div>OPTIONS sip:{status.sip_exchange?.remote_endpoint} SIP/2.0</div>
                  <div>Via: SIP/2.0/UDP {status.sip_exchange?.local_endpoint};branch=z9hG4bK-check</div>
                  <div>From: &lt;sip:apolo-sbc@local&gt;;tag=check123</div>
                  <div>To: &lt;sip:{status.sip_exchange?.remote_endpoint}&gt;</div>
                  <div>Call-ID: options-{Date.now()}@apolo-sbc</div>
                  <div>CSeq: 1 OPTIONS</div>
                  <div>User-Agent: Apolo-SBC/1.0</div>
                  <div>Max-Forwards: 70</div>
                  <div>Content-Length: 0</div>
                  <div className="h-2" />
                  <div className="text-emerald-400">--- Respuesta recibida ({status.latency_ms}ms) ---</div>
                  <div>SIP/2.0 {status.response_code} {status.response_code === 200 ? 'OK' : 'Response'}</div>
                  <div>Via: SIP/2.0/UDP {status.sip_exchange?.local_endpoint};branch=z9hG4bK-check</div>
                  <div>From: &lt;sip:apolo-sbc@local&gt;;tag=check123</div>
                  <div>To: &lt;sip:{status.sip_exchange?.remote_endpoint}&gt;;tag=resp456</div>
                  <div>Allow: INVITE, ACK, BYE, CANCEL, OPTIONS, PRACK, REFER</div>
                  <div>Supported: timer, 100rel</div>
                  <div>Content-Length: 0</div>
                </div>
              ) : (
                <div className="space-y-1">
                  <div className="text-sky-400">--- Enviando OPTIONS ---</div>
                  <div className="text-slate-300">OPTIONS sip:{status.sip_exchange?.remote_endpoint} SIP/2.0</div>
                  <div className="text-slate-300">...</div>
                  <div className="h-2" />
                  <div className="text-red-400">--- Error ---</div>
                  <div className="text-red-300">{status.status_message}</div>
                  {status.response_code && (
                    <div className="text-amber-400">Código de respuesta: {status.response_code}</div>
                  )}
                </div>
              )}
            </div>

            <div className="p-3 bg-[var(--color-bg-tertiary)] rounded-lg">
              <p className="text-xs text-[var(--color-text-muted)]">
                <strong>Nota:</strong> Este es el comando que Apolo SBC ejecuta para verificar la conectividad SIP con el peer remoto.
                {status.trunk_type === 'private'
                  ? ' FreeSWITCH usa el comando sofia para verificar el estado del gateway.'
                  : ' Kamailio usa sipsak para enviar un mensaje OPTIONS directamente al peer.'}
              </p>
            </div>
          </div>
        )}

        {/* SIP Exchange Visualization - Visual View */}
        {activeView === 'visual' && status.sip_exchange && (
          <div className="space-y-4">
            <h4 className="font-semibold text-[var(--color-text-primary)] flex items-center gap-2">
              <MessageSquare className="w-4 h-4 text-[var(--color-primary)]" />
              Intercambio SIP (OPTIONS)
            </h4>

            {/* Endpoints */}
            <div className="flex items-center justify-between px-4 py-3 bg-[var(--color-bg-secondary)] rounded-lg">
              <div className="text-center">
                <div className="text-xs text-[var(--color-text-tertiary)] mb-1">Local</div>
                <div className="font-mono text-sm text-[var(--color-text-primary)]">
                  {status.sip_exchange.local_endpoint}
                </div>
              </div>
              <div className="flex-1 flex items-center justify-center px-4">
                <div className="flex-1 h-px bg-[var(--color-border-primary)]" />
                <ArrowRight className="w-5 h-5 mx-2 text-[var(--color-primary)]" />
                <div className="flex-1 h-px bg-[var(--color-border-primary)]" />
              </div>
              <div className="text-center">
                <div className="text-xs text-[var(--color-text-tertiary)] mb-1">Remoto</div>
                <div className="font-mono text-sm text-[var(--color-text-primary)]">
                  {status.sip_exchange.remote_endpoint}
                </div>
              </div>
            </div>

            {/* Timeline */}
            {status.sip_exchange.timeline && status.sip_exchange.timeline.length > 0 && (
              <div className="space-y-2">
                <h5 className="text-sm font-medium text-[var(--color-text-secondary)]">Cronología</h5>
                <div className="relative pl-6 space-y-3">
                  <div className="absolute left-2.5 top-2 bottom-2 w-px bg-[var(--color-border-primary)]" />
                  {status.sip_exchange.timeline.map((step, idx) => (
                    <div key={idx} className="relative flex items-start gap-3">
                      <div className={`absolute -left-3.5 w-3 h-3 rounded-full border-2 ${
                        step.direction === 'sent'
                          ? 'bg-sky-500 border-sky-500/30'
                          : 'bg-emerald-500 border-emerald-500/30'
                      }`} />
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          {step.direction === 'sent' ? (
                            <ArrowRight className="w-3.5 h-3.5 text-sky-500" />
                          ) : (
                            <ArrowLeft className="w-3.5 h-3.5 text-emerald-500" />
                          )}
                          <span className={`text-xs font-medium ${
                            step.direction === 'sent' ? 'text-sky-600 dark:text-sky-400' : 'text-emerald-600 dark:text-emerald-400'
                          }`}>
                            {step.message_type}
                          </span>
                          <span className="text-xs text-[var(--color-text-muted)]">{step.timestamp}</span>
                        </div>
                        <p className="text-sm text-[var(--color-text-secondary)] mt-0.5">{step.summary}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Request/Response Details */}
            <div className="grid grid-cols-2 gap-4">
              {/* Request */}
              <div className="space-y-2">
                <h5 className="text-sm font-medium text-[var(--color-text-secondary)] flex items-center gap-2">
                  <ArrowRight className="w-4 h-4 text-sky-500" />
                  Request OPTIONS
                </h5>
                <div className="p-3 bg-sky-500/5 rounded-lg border border-sky-500/20 font-mono text-xs space-y-1">
                  <div className="text-sky-600 dark:text-sky-400 font-semibold">
                    {status.sip_exchange.request_summary.method_or_status}
                  </div>
                  {status.sip_exchange.request_summary.from && (
                    <div><span className="text-[var(--color-text-tertiary)]">From:</span> {status.sip_exchange.request_summary.from}</div>
                  )}
                  {status.sip_exchange.request_summary.to && (
                    <div><span className="text-[var(--color-text-tertiary)]">To:</span> {status.sip_exchange.request_summary.to}</div>
                  )}
                  {status.sip_exchange.request_summary.call_id && (
                    <div><span className="text-[var(--color-text-tertiary)]">Call-ID:</span> {status.sip_exchange.request_summary.call_id}</div>
                  )}
                </div>
              </div>

              {/* Response */}
              {status.sip_exchange.response_summary && (
                <div className="space-y-2">
                  <h5 className="text-sm font-medium text-[var(--color-text-secondary)] flex items-center gap-2">
                    <ArrowLeft className="w-4 h-4 text-emerald-500" />
                    Response
                  </h5>
                  <div className="p-3 bg-emerald-500/5 rounded-lg border border-emerald-500/20 font-mono text-xs space-y-1">
                    <div className={`font-semibold ${
                      status.sip_exchange.response_summary.method_or_status.startsWith('2')
                        ? 'text-emerald-600 dark:text-emerald-400'
                        : 'text-red-600 dark:text-red-400'
                    }`}>
                      {status.sip_exchange.response_summary.method_or_status}
                    </div>
                    {status.sip_exchange.response_summary.user_agent && (
                      <div><span className="text-[var(--color-text-tertiary)]">User-Agent:</span> {status.sip_exchange.response_summary.user_agent}</div>
                    )}
                    {status.sip_exchange.response_summary.allow && (
                      <div><span className="text-[var(--color-text-tertiary)]">Allow:</span> {status.sip_exchange.response_summary.allow}</div>
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Checked At */}
        <div className="text-xs text-[var(--color-text-muted)] text-center pt-2 border-t border-[var(--color-border-primary)]">
          Verificado: {new Date(status.checked_at).toLocaleString('es-PE')}
        </div>

        {/* Close Button */}
        <div className="flex justify-end">
          <button
            onClick={onClose}
            className="px-5 py-2.5 rounded-lg bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white hover:shadow-lg hover:shadow-[var(--color-primary)]/30 transition-all cursor-pointer font-medium"
          >
            Cerrar
          </button>
        </div>
      </div>
    </Modal>
  )
}
