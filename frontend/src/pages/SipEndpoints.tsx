import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchSipEndpoints,
  createSipEndpoint,
  updateSipEndpoint,
  deleteSipEndpoint,
  getSipEndpointPassword,
  regenerateSipEndpointPassword,
  getSipEndpointRegistrationStatus,
  syncSipEndpoint,
  syncAllSipEndpoints,
  fetchAccounts,
  type SipRegistrationStatus,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import Tooltip from '../components/Tooltip'
import {
  Phone,
  Plus,
  Edit,
  X,
  Trash2,
  AlertCircle,
  Eye,
  EyeOff,
  RefreshCw,
  Copy,
  CheckCircle,
  Wifi,
  WifiOff,
  Activity,
  Clock,
  Globe,
  Monitor,
  Server,
  KeyRound,
  ChevronDown,
  ChevronUp,
  Filter,
  Search,
  Info,
} from 'lucide-react'
import type {
  SipEndpoint,
  SipEndpointWithPassword,
  SipEndpointCreateRequest,
  Account,
  EndpointType,
  EndpointAuthMode,
  TransportProtocol,
  KamailioSyncStatus,
} from '../types'

function isWithPassword(
  endpoint: SipEndpoint | SipEndpointWithPassword
): endpoint is SipEndpointWithPassword {
  return 'password' in endpoint
}

const PAGE_SIZE = 20

interface EndpointFilters {
  search: string
  type: 'all' | EndpointType
  authMode: 'all' | EndpointAuthMode
  status: 'all' | 'enabled' | 'disabled'
  transport: 'all' | TransportProtocol
  syncStatus: 'all' | KamailioSyncStatus
  accountId: 'all' | 'none' | number
}

const DEFAULT_FILTERS: EndpointFilters = {
  search: '',
  type: 'all',
  authMode: 'all',
  status: 'all',
  transport: 'all',
  syncStatus: 'all',
  accountId: 'all',
}

export default function SipEndpointsPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingEndpoint, setEditingEndpoint] = useState<SipEndpoint | null>(null)
  const [deletingEndpoint, setDeletingEndpoint] = useState<SipEndpoint | null>(null)
  const [viewingPassword, setViewingPassword] = useState<SipEndpointWithPassword | null>(null)
  const [viewingStatus, setViewingStatus] = useState<{ endpoint: SipEndpoint; status: SipRegistrationStatus } | null>(null)
  const [error, setError] = useState('')
  const [createdEndpoint, setCreatedEndpoint] = useState<SipEndpointWithPassword | null>(null)
  const [page, setPage] = useState(1)
  const [showFilters, setShowFilters] = useState(false)
  const [filters, setFilters] = useState<EndpointFilters>(DEFAULT_FILTERS)
  const queryClient = useQueryClient()

  const { data: endpoints = [], isLoading } = useQuery({
    queryKey: ['sip-endpoints'],
    queryFn: () => fetchSipEndpoints(),
  })

  const { data: accounts = [] } = useQuery({
    queryKey: ['accounts'],
    queryFn: fetchAccounts,
  })

  const hasActiveFilters = Object.entries(filters).some(
    ([key, value]) => value !== DEFAULT_FILTERS[key as keyof EndpointFilters]
  )

  const updateFilters = (partial: Partial<EndpointFilters>) => {
    setFilters((prev) => ({ ...prev, ...partial }))
    setPage(1)
  }

  const clearFilters = () => {
    setFilters(DEFAULT_FILTERS)
    setPage(1)
  }

  const filteredEndpoints = useMemo(() => {
    const term = filters.search.trim().toLowerCase()

    return endpoints.filter((e) => {
      if (filters.type !== 'all' && e.endpoint_type !== filters.type) return false
      if (filters.authMode !== 'all' && e.auth_mode !== filters.authMode) return false
      if (filters.status !== 'all') {
        const wantEnabled = filters.status === 'enabled'
        if (e.enabled !== wantEnabled) return false
      }
      if (filters.transport !== 'all' && e.transport !== filters.transport) return false
      if (filters.syncStatus !== 'all' && e.sync_status !== filters.syncStatus) return false
      if (filters.accountId !== 'all') {
        if (filters.accountId === 'none') {
          if (e.account_id) return false
        } else if (e.account_id !== filters.accountId) {
          return false
        }
      }
      if (term) {
        const haystack = [
          e.name,
          e.description,
          e.display_name,
          e.accountcode,
          e.sip_username,
          e.sip_domain,
          e.host,
          e.port ? String(e.port) : '',
          e.did_ranges,
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
        if (!haystack.includes(term)) return false
      }
      return true
    })
  }, [endpoints, filters])

  const totalPages = Math.max(1, Math.ceil(filteredEndpoints.length / PAGE_SIZE))
  const currentPage = Math.min(page, totalPages)
  const paginatedEndpoints = filteredEndpoints.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE
  )

  const createMutation = useMutation({
    mutationFn: createSipEndpoint,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      setShowModal(false)
      setEditingEndpoint(null)
      setError('')
      if (isWithPassword(data)) {
        setCreatedEndpoint(data)
      }
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      setError(err.response?.data?.message || err.message)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<SipEndpointCreateRequest> }) =>
      updateSipEndpoint(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      setShowModal(false)
      setEditingEndpoint(null)
      setError('')
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      setError(err.response?.data?.message || err.message)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteSipEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      setDeletingEndpoint(null)
    },
  })

  const getPasswordMutation = useMutation({
    mutationFn: getSipEndpointPassword,
    onSuccess: (data) => setViewingPassword(data),
  })

  const regeneratePasswordMutation = useMutation({
    mutationFn: regenerateSipEndpointPassword,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      setViewingPassword({ ...viewingPassword!, password: data.new_password })
    },
  })

  const getStatusMutation = useMutation({
    mutationFn: (endpoint: SipEndpoint) => getSipEndpointRegistrationStatus(endpoint.id),
    onSuccess: (status, endpoint) => setViewingStatus({ endpoint, status }),
  })

  const syncOneMutation = useMutation({
    mutationFn: syncSipEndpoint,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] }),
  })

  const syncAllMutation = useMutation({
    mutationFn: syncAllSipEndpoints,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] }),
  })

  const handleEdit = (endpoint: SipEndpoint) => {
    setEditingEndpoint(endpoint)
    setShowModal(true)
    setError('')
  }

  const handleCreate = () => {
    setEditingEndpoint(null)
    setShowModal(true)
    setError('')
  }

  const handleDelete = () => {
    if (deletingEndpoint) {
      deleteMutation.mutate(deletingEndpoint.id)
    }
  }

  const getAccountName = (accountId?: number) => {
    if (!accountId) return '-'
    const account = accounts.find((a: Account) => a.id === accountId)
    return account?.account_number || `ID: ${accountId}`
  }

  const hasIpTrunks = endpoints.some((e) => e.auth_mode === 'ip')

  const columns = [
    {
      key: 'name',
      header: 'Nombre',
      render: (endpoint: SipEndpoint) => (
        <div>
          <span className="font-medium">{endpoint.name}</span>
          {endpoint.description && (
            <p className="text-xs text-[var(--color-text-muted)]">{endpoint.description}</p>
          )}
        </div>
      ),
    },
    {
      key: 'endpoint_type',
      header: 'Tipo',
      render: (endpoint: SipEndpoint) => (
        <Badge variant={endpoint.endpoint_type === 'trunk' ? 'info' : 'default'}>
          {endpoint.endpoint_type === 'trunk' ? 'Troncal' : 'Extensión'}
        </Badge>
      ),
    },
    {
      key: 'auth_mode',
      header: 'Autenticación',
      render: (endpoint: SipEndpoint) =>
        endpoint.auth_mode === 'credentials' ? (
          <span className="inline-flex items-center text-sm">
            <KeyRound className="w-3.5 h-3.5 mr-1 text-[var(--color-text-muted)]" />
            Usuario y contraseña
          </span>
        ) : (
          <span className="inline-flex items-center text-sm">
            <Globe className="w-3.5 h-3.5 mr-1 text-[var(--color-text-muted)]" />
            IP de origen
          </span>
        ),
    },
    {
      key: 'identifier',
      header: 'Identificador',
      render: (endpoint: SipEndpoint) =>
        endpoint.auth_mode === 'credentials' ? (
          <span className="font-mono text-sm">
            {endpoint.sip_username}
            <span className="text-[var(--color-text-muted)]">@{endpoint.sip_domain}</span>
          </span>
        ) : (
          <span className="font-mono text-sm">
            {endpoint.host}:{endpoint.port}
            <span className="text-[var(--color-text-muted)] ml-1">
              ({endpoint.transport.toUpperCase()})
            </span>
            {endpoint.did_ranges && (
              <span className="block text-xs text-[var(--color-text-secondary)]">
                DIDs: {endpoint.did_ranges}
              </span>
            )}
            {endpoint.default_caller_id && (
              <span className="block text-xs text-[var(--color-text-secondary)]">
                Cabecera: {endpoint.default_caller_id}
              </span>
            )}
          </span>
        ),
    },
    {
      key: 'account_id',
      header: 'Suscripción',
      render: (endpoint: SipEndpoint) => (
        <span className="text-sm">{getAccountName(endpoint.account_id)}</span>
      ),
    },
    {
      key: 'status',
      header: 'Estado',
      render: (endpoint: SipEndpoint) => (
        <div className="flex items-center gap-2">
          <Badge variant={endpoint.enabled ? 'success' : 'error'}>
            {endpoint.enabled ? 'Activo' : 'Inactivo'}
          </Badge>
          {endpoint.auth_mode === 'ip' && (
            <Badge
              variant={
                endpoint.sync_status === 'synced'
                  ? 'success'
                  : endpoint.sync_status === 'error'
                  ? 'error'
                  : 'warning'
              }
            >
              {endpoint.sync_status === 'synced'
                ? 'Sincronizado'
                : endpoint.sync_status === 'error'
                ? 'Error de sync'
                : 'Pendiente'}
            </Badge>
          )}
        </div>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (endpoint: SipEndpoint) => (
        <div className="flex items-center space-x-1">
          {endpoint.auth_mode === 'credentials' && (
            <>
              <button
                onClick={() => getStatusMutation.mutate(endpoint)}
                className="p-1.5 hover:bg-blue-50 rounded"
                title="Ver estado de registro"
              >
                <Activity className="w-4 h-4 text-blue-600" />
              </button>
              <button
                onClick={() => getPasswordMutation.mutate(endpoint.id)}
                className="p-1.5 hover:bg-[var(--color-bg-secondary)] rounded"
                title="Ver contraseña"
              >
                <Eye className="w-4 h-4 text-[var(--color-text-secondary)]" />
              </button>
            </>
          )}
          {endpoint.auth_mode === 'ip' && (
            <button
              onClick={() => syncOneMutation.mutate(endpoint.id)}
              disabled={syncOneMutation.isPending}
              className="p-1.5 hover:bg-[var(--color-bg-secondary)] rounded disabled:opacity-50"
              title="Re-sincronizar con Kamailio"
            >
              <RefreshCw
                className={`w-4 h-4 text-[var(--color-text-secondary)] ${
                  syncOneMutation.isPending ? 'animate-spin' : ''
                }`}
              />
            </button>
          )}
          <button
            onClick={() => handleEdit(endpoint)}
            className="p-1.5 hover:bg-[var(--color-bg-secondary)] rounded"
            title="Editar"
          >
            <Edit className="w-4 h-4 text-[var(--color-text-secondary)]" />
          </button>
          <button
            onClick={() => setDeletingEndpoint(endpoint)}
            className="p-1.5 hover:bg-red-50 rounded"
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
            <Phone className="w-8 h-8 mr-3 text-blue-600" />
            Dispositivos SIP
          </h1>
          <p className="text-[var(--color-text-secondary)] mt-1">
            Extensiones (softphones) y troncales (PBX) de la plataforma
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center px-4 py-2 rounded-lg border transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-blue-50 border-blue-300 text-blue-700'
                : 'border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]'
            }`}
          >
            <Filter className="w-4 h-4 mr-2" />
            Filtros
            {hasActiveFilters && (
              <span className="ml-2 bg-blue-600 text-white text-xs px-2 py-0.5 rounded-full">
                {
                  Object.entries(filters).filter(
                    ([key, value]) => value !== DEFAULT_FILTERS[key as keyof EndpointFilters]
                  ).length
                }
              </span>
            )}
          </button>
          {hasIpTrunks && (
            <button
              onClick={() => syncAllMutation.mutate()}
              disabled={syncAllMutation.isPending}
              className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)] flex items-center disabled:opacity-50"
            >
              <RefreshCw
                className={`w-4 h-4 mr-2 ${syncAllMutation.isPending ? 'animate-spin' : ''}`}
              />
              Sincronizar troncales
            </button>
          )}
          <button
            onClick={handleCreate}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center"
          >
            <Plus className="w-4 h-4 mr-2" />
            Nuevo Dispositivo
          </button>
        </div>
      </div>

      {showFilters && (
        <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="font-semibold text-[var(--color-text-primary)]">Filtros de Búsqueda</h3>
            {hasActiveFilters && (
              <button
                onClick={clearFilters}
                className="text-sm text-red-600 hover:text-red-700 flex items-center"
              >
                <X className="w-4 h-4 mr-1" />
                Limpiar filtros
              </button>
            )}
          </div>
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            <div className="md:col-span-2">
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Búsqueda general
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-[var(--color-text-muted)]" />
                <input
                  type="text"
                  value={filters.search}
                  onChange={(e) => updateFilters({ search: e.target.value })}
                  placeholder="Nombre, usuario SIP, host, DID..."
                  className="w-full pl-10 pr-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Tipo
              </label>
              <select
                value={filters.type}
                onChange={(e) =>
                  updateFilters({ type: e.target.value as EndpointFilters['type'] })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todos</option>
                <option value="extension">Extensión</option>
                <option value="trunk">Troncal</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Autenticación
              </label>
              <select
                value={filters.authMode}
                onChange={(e) =>
                  updateFilters({ authMode: e.target.value as EndpointFilters['authMode'] })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todas</option>
                <option value="credentials">Usuario y contraseña</option>
                <option value="ip">IP de origen</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Estado
              </label>
              <select
                value={filters.status}
                onChange={(e) =>
                  updateFilters({ status: e.target.value as EndpointFilters['status'] })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todos</option>
                <option value="enabled">Activo</option>
                <option value="disabled">Inactivo</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Transporte
              </label>
              <select
                value={filters.transport}
                onChange={(e) =>
                  updateFilters({ transport: e.target.value as EndpointFilters['transport'] })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todos</option>
                <option value="udp">UDP</option>
                <option value="tcp">TCP</option>
                <option value="tls">TLS</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Sincronización
              </label>
              <select
                value={filters.syncStatus}
                onChange={(e) =>
                  updateFilters({ syncStatus: e.target.value as EndpointFilters['syncStatus'] })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todas</option>
                <option value="synced">Sincronizado</option>
                <option value="error">Error de sync</option>
                <option value="pending">Pendiente</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Suscripción
              </label>
              <select
                value={filters.accountId}
                onChange={(e) =>
                  updateFilters({
                    accountId:
                      e.target.value === 'all' || e.target.value === 'none'
                        ? (e.target.value as 'all' | 'none')
                        : Number(e.target.value),
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="all">Todas</option>
                <option value="none">Sin suscripción</option>
                {accounts.map((account: Account) => (
                  <option key={account.id} value={account.id}>
                    {account.account_number}
                    {account.account_name && ` - ${account.account_name}`}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </div>
      )}

      <p className="text-sm text-[var(--color-text-tertiary)]">
        {filteredEndpoints.length === endpoints.length
          ? `${endpoints.length} dispositivo${endpoints.length === 1 ? '' : 's'}`
          : `${filteredEndpoints.length} de ${endpoints.length} dispositivos`}
      </p>

      <DataTable
        data={paginatedEndpoints}
        columns={columns}
        loading={isLoading}
        emptyMessage="No hay dispositivos SIP que coincidan con los filtros"
        pagination={{
          page: currentPage,
          totalPages,
          onPageChange: setPage,
        }}
      />

      {showModal && (
        <EndpointModal
          endpoint={editingEndpoint}
          accounts={accounts}
          error={error}
          onClose={() => {
            setShowModal(false)
            setEditingEndpoint(null)
            setError('')
          }}
          onSave={(data) => {
            if (editingEndpoint) {
              updateMutation.mutate({ id: editingEndpoint.id, data })
            } else {
              createMutation.mutate(data)
            }
          }}
        />
      )}

      {deletingEndpoint && (
        <DeleteModal
          endpoint={deletingEndpoint}
          onClose={() => setDeletingEndpoint(null)}
          onConfirm={handleDelete}
        />
      )}

      {viewingPassword && (
        <PasswordModal
          endpoint={viewingPassword}
          onClose={() => setViewingPassword(null)}
          onRegenerate={() => regeneratePasswordMutation.mutate(viewingPassword.id)}
          isRegenerating={regeneratePasswordMutation.isPending}
        />
      )}

      {createdEndpoint && (
        <CreatedEndpointModal endpoint={createdEndpoint} onClose={() => setCreatedEndpoint(null)} />
      )}

      {viewingStatus && (
        <RegistrationStatusModal
          endpoint={viewingStatus.endpoint}
          status={viewingStatus.status}
          onClose={() => setViewingStatus(null)}
          onRefresh={() => getStatusMutation.mutate(viewingStatus.endpoint)}
          isRefreshing={getStatusMutation.isPending}
        />
      )}
    </div>
  )
}

function EndpointModal({
  endpoint,
  accounts,
  error,
  onClose,
  onSave,
}: {
  endpoint: SipEndpoint | null
  accounts: Account[]
  error: string
  onClose: () => void
  onSave: (data: SipEndpointCreateRequest) => void
}) {
  const [endpointType, setEndpointType] = useState<EndpointType>(endpoint?.endpoint_type || 'extension')
  const [authMode, setAuthMode] = useState<EndpointAuthMode>(endpoint?.auth_mode || 'credentials')
  const [showAdvanced, setShowAdvanced] = useState(false)

  const [formData, setFormData] = useState({
    name: endpoint?.name || '',
    description: endpoint?.description || '',
    account_id: endpoint?.account_id,
    sip_username: endpoint?.sip_username || '',
    sip_domain: endpoint?.sip_domain || '10.10.22.4',
    password: '',
    max_registrations: endpoint?.max_registrations || 3,
    max_concurrent_calls: endpoint?.max_concurrent_calls ?? 2,
    host: endpoint?.host || '',
    port: endpoint?.port || 5060,
    transport: endpoint?.transport || ('udp' as TransportProtocol),
    is_default: endpoint?.is_default || false,
    did_ranges: endpoint?.did_ranges || '',
    default_caller_id: endpoint?.default_caller_id || '',
    display_name: endpoint?.display_name || '',
    accountcode: endpoint?.accountcode || '',
    codecs: endpoint?.codecs || 'PCMU,PCMA,G729,opus',
    enabled: endpoint?.enabled ?? true,
  })

  // Extensión only supports credential auth - keep state coherent if the
  // user flips the type selector away from Troncal.
  const handleTypeChange = (type: EndpointType) => {
    setEndpointType(type)
    if (type === 'extension') {
      setAuthMode('credentials')
    }
    // Clamp channel/call limits when switching to extension (1-20) so a
    // trunk value like 100 doesn't linger out of range.
    setFormData((prev) => ({
      ...prev,
      max_registrations:
        type === 'extension' ? Math.min(prev.max_registrations, 20) : prev.max_registrations,
      max_concurrent_calls:
        type === 'extension'
          ? Math.min(prev.max_concurrent_calls, 20)
          : prev.max_concurrent_calls,
    }))
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave({
      name: formData.name,
      description: formData.description || undefined,
      endpoint_type: endpointType,
      auth_mode: authMode,
      account_id: formData.account_id || undefined,
      sip_username: authMode === 'credentials' ? formData.sip_username : undefined,
      sip_domain: authMode === 'credentials' ? formData.sip_domain : undefined,
      password: authMode === 'credentials' && formData.password ? formData.password : undefined,
      max_registrations: formData.max_registrations,
      max_concurrent_calls: formData.max_concurrent_calls,
      host: authMode === 'ip' ? formData.host : undefined,
      port: authMode === 'ip' ? formData.port : undefined,
      transport: authMode === 'ip' ? formData.transport : undefined,
      is_default: authMode === 'ip' ? formData.is_default : undefined,
      did_ranges: authMode === 'ip' ? formData.did_ranges : undefined,
      default_caller_id: authMode === 'ip' ? formData.default_caller_id : undefined,
      display_name: formData.display_name || undefined,
      accountcode: formData.accountcode || undefined,
      codecs: formData.codecs,
      enabled: formData.enabled,
    })
  }

  // max_registrations: solo tiene efecto si el endpoint hace REGISTER
  // (auth_mode=credentials). Las troncales IP no registran y lo ignoran.
  const registrationsMax = endpointType === 'trunk' ? 150 : 20
  const registrationsHelp =
    endpointType === 'trunk'
      ? 'Límite de registros simultáneos de esta troncal (1-150). No aplica a troncales por IP.'
      : 'Límite de registros simultáneos de este softphone (1-20).'

  // max_concurrent_calls: límite real de llamadas activas a la vez. Se
  // enforcea en el billing engine (credentials) o en Kamailio (troncal IP).
  const callsLabel = endpointType === 'trunk' ? 'Canales concurrentes' : 'Llamadas simultáneas'
  const callsMax = endpointType === 'trunk' ? 150 : 20
  const callsHelp =
    endpointType === 'trunk'
      ? authMode === 'ip'
        ? 'Límite de canales simultáneos de la troncal, enforceado en Kamailio (1-150).'
        : 'Límite de llamadas simultáneas de la troncal, enforceado por el billing engine (1-150).'
      : 'Máximo de llamadas activas a la vez (entrantes + salientes). La llamada que exceda escucha ocupado y se corta (1-20).'

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold">
            {endpoint ? 'Editar Dispositivo SIP' : 'Nuevo Dispositivo SIP'}
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
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Nombre *
            </label>
            <input
              type="text"
              required
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              placeholder="Ej: Extensión Recepción, PBX Sucursal Norte"
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>

          {/* Tipo: Extensión / Troncal */}
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-2">
              Tipo *
            </label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                disabled={!!endpoint}
                onClick={() => handleTypeChange('extension')}
                className={`flex items-center justify-center gap-2 px-3 py-2.5 border rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed ${
                  endpointType === 'extension'
                    ? 'border-blue-600 bg-blue-50 text-blue-700'
                    : 'hover:bg-[var(--color-bg-secondary)]'
                }`}
              >
                <Phone className="w-4 h-4" />
                Extensión
              </button>
              <button
                type="button"
                disabled={!!endpoint}
                onClick={() => handleTypeChange('trunk')}
                className={`flex items-center justify-center gap-2 px-3 py-2.5 border rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed ${
                  endpointType === 'trunk'
                    ? 'border-blue-600 bg-blue-50 text-blue-700'
                    : 'hover:bg-[var(--color-bg-secondary)]'
                }`}
              >
                <Server className="w-4 h-4" />
                Troncal (PBX)
              </button>
            </div>
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              {endpointType === 'extension'
                ? 'Un softphone o teléfono IP individual.'
                : 'Una central telefónica (PBX) completa detrás, con varios canales simultáneos.'}
            </p>
          </div>

          {/* Autenticación: solo elegible para troncales */}
          {endpointType === 'trunk' && (
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-2">
                Autenticación *
              </label>
              <div className="grid grid-cols-2 gap-3">
                <button
                  type="button"
                  disabled={!!endpoint}
                  onClick={() => setAuthMode('credentials')}
                  className={`flex items-center justify-center gap-2 px-3 py-2.5 border rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed ${
                    authMode === 'credentials'
                      ? 'border-blue-600 bg-blue-50 text-blue-700'
                      : 'hover:bg-[var(--color-bg-secondary)]'
                  }`}
                >
                  <KeyRound className="w-4 h-4" />
                  Usuario y contraseña
                </button>
                <button
                  type="button"
                  disabled={!!endpoint}
                  onClick={() => setAuthMode('ip')}
                  className={`flex items-center justify-center gap-2 px-3 py-2.5 border rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed ${
                    authMode === 'ip'
                      ? 'border-blue-600 bg-blue-50 text-blue-700'
                      : 'hover:bg-[var(--color-bg-secondary)]'
                  }`}
                >
                  <Globe className="w-4 h-4" />
                  IP de origen
                </button>
              </div>
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                {authMode === 'credentials'
                  ? 'La PBX remota se registra con usuario y contraseña (útil si no tiene IP fija).'
                  : 'Se confía en la IP de origen sin necesidad de registro (requiere IP pública fija).'}
              </p>
            </div>
          )}

          {authMode === 'credentials' ? (
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Usuario SIP *
                </label>
                <input
                  type="text"
                  required
                  disabled={!!endpoint}
                  value={formData.sip_username}
                  onChange={(e) => setFormData({ ...formData, sip_username: e.target.value })}
                  placeholder="1001"
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-[var(--color-bg-secondary)]"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Dominio SIP *
                </label>
                <input
                  type="text"
                  required
                  disabled={!!endpoint}
                  value={formData.sip_domain}
                  onChange={(e) => setFormData({ ...formData, sip_domain: e.target.value })}
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-[var(--color-bg-secondary)]"
                />
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Puerto SIP: 5080 (configurar en el dispositivo)
                </p>
              </div>
              {!endpoint && (
                <div className="col-span-2">
                  <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                    Contraseña SIP
                  </label>
                  <input
                    type="text"
                    value={formData.password}
                    onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                    placeholder="Dejar vacío para generar una automáticamente"
                    className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 font-mono"
                  />
                  <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                    Si la dejás vacía, se genera una contraseña segura y se te muestra al crear el dispositivo.
                    Para cambiarla después, usá "Regenerar contraseña" desde la lista.
                  </p>
                </div>
              )}
            </div>
          ) : (
            <div className="grid grid-cols-3 gap-4">
              <div className="col-span-2">
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Host / IP *
                </label>
                <input
                  type="text"
                  required
                  value={formData.host}
                  onChange={(e) => setFormData({ ...formData, host: e.target.value })}
                  placeholder="10.0.0.5"
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Puerto
                </label>
                <input
                  type="number"
                  value={formData.port}
                  onChange={(e) => setFormData({ ...formData, port: Number(e.target.value) })}
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div className="col-span-3">
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Transporte
                </label>
                <select
                  value={formData.transport}
                  onChange={(e) =>
                    setFormData({ ...formData, transport: e.target.value as TransportProtocol })
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                >
                  <option value="udp">UDP</option>
                  <option value="tcp">TCP</option>
                  <option value="tls">TLS</option>
                </select>
              </div>
              <div className="col-span-3">
                <label className="flex items-center gap-1.5 text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Rangos DID propios
                  <Tooltip
                    content={
                      <div className="space-y-2">
                        <p className="font-semibold">Qué números de esta PBX enrutar automáticamente</p>
                        <p>Separá las entradas con coma. Admite 3 formatos, combinables en el mismo campo:</p>
                        <ul className="space-y-1">
                          <li>
                            <code className="bg-black/10 px-1 rounded">174020</code> → bloque completo
                            (equivale a 17402000-17402099, 100 números)
                          </li>
                          <li>
                            <code className="bg-black/10 px-1 rounded">17402001</code> → un solo DID
                            individual
                          </li>
                          <li>
                            <code className="bg-black/10 px-1 rounded">17402010-17402099</code> → rango
                            explícito (cualquier cantidad de números, no solo decenas/centenas)
                          </li>
                        </ul>
                        <p className="font-semibold">Ejemplo real: subdividir un bloque entre 4 clientes</p>
                        <p className="font-mono bg-black/10 rounded px-2 py-1">
                          17402000,17402001,17402002,17402010-17402099
                        </p>
                        <p>
                          → <code>17402000</code>, <code>17402001</code> y <code>17402002</code> van cada
                          uno a su propia PBX; el resto del bloque (<code>17402010</code> a{' '}
                          <code>17402099</code>) va a una cuarta. Si dos PBX distintas tienen entradas que se
                          superponen, gana la más específica (más dígitos) — así se reparte un bloque sin
                          pisarse. Vacío = esta PBX no recibe DIDs por este mecanismo.
                        </p>
                      </div>
                    }
                  >
                    <Info className="w-3.5 h-3.5 text-[var(--color-text-tertiary)]" />
                  </Tooltip>
                </label>
                <input
                  type="text"
                  value={formData.did_ranges}
                  onChange={(e) => setFormData({ ...formData, did_ranges: e.target.value })}
                  placeholder="174020, 17402001, 17402010-17402099"
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Prefijo (bloque), DID individual, o rango "inicio-fin" — separados por coma. Pasá el
                  mouse sobre <Info className="inline w-3 h-3" /> para ver ejemplos. Vacío = sin rangos.
                </p>
              </div>
              <div className="col-span-3">
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Número principal / cabecera
                </label>
                <input
                  type="text"
                  value={formData.default_caller_id}
                  onChange={(e) =>
                    setFormData({ ...formData, default_caller_id: e.target.value })
                  }
                  placeholder="17402000"
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Si la PBX manda caller ID "Anonymous" o vacío en sus salientes, Kamailio lo
                  reemplaza por este número antes de enviarlo al carrier. Vacío = no reemplazar.
                </p>
              </div>
              <div className="col-span-3 flex items-center">
                <input
                  type="checkbox"
                  checked={formData.is_default}
                  onChange={(e) => setFormData({ ...formData, is_default: e.target.checked })}
                  className="w-4 h-4 text-blue-600 rounded focus:ring-blue-500"
                />
                <label className="ml-2 text-sm font-medium text-[var(--color-text-secondary)]">
                  Troncal por defecto
                </label>
              </div>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Suscripción de Facturación
            </label>
            <select
              value={formData.account_id ?? ''}
              onChange={(e) =>
                setFormData({
                  ...formData,
                  account_id: e.target.value ? Number(e.target.value) : undefined,
                })
              }
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            >
              <option value="">Sin suscripción vinculada</option>
              {accounts.map((account: Account) => (
                <option key={account.id} value={account.id}>
                  {account.account_number}
                  {account.account_name && ` - ${account.account_name}`}
                  {account.customer_phone && ` (${account.customer_phone})`}
                </option>
              ))}
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Nombre para Mostrar
              </label>
              <input
                type="text"
                value={formData.display_name}
                onChange={(e) => setFormData({ ...formData, display_name: e.target.value })}
                placeholder="Juan Pérez"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                {callsLabel}
              </label>
              <input
                type="number"
                min="1"
                max={callsMax}
                value={formData.max_concurrent_calls}
                onChange={(e) =>
                  setFormData({ ...formData, max_concurrent_calls: Number(e.target.value) })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">{callsHelp}</p>
            </div>
          </div>

          {authMode === 'credentials' && (
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Registros simultáneos
              </label>
              <input
                type="number"
                min="1"
                max={registrationsMax}
                value={formData.max_registrations}
                onChange={(e) =>
                  setFormData({ ...formData, max_registrations: Number(e.target.value) })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">{registrationsHelp}</p>
            </div>
          )}

          <button
            type="button"
            onClick={() => setShowAdvanced(!showAdvanced)}
            className="flex items-center text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
          >
            {showAdvanced ? <ChevronUp className="w-4 h-4 mr-1" /> : <ChevronDown className="w-4 h-4 mr-1" />}
            Opciones avanzadas
          </button>

          {showAdvanced && (
            <div className="space-y-4 pl-1 border-l-2 border-[var(--color-border-primary)] ml-1">
              <div className="pl-3">
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Código de Cuenta (CDR)
                </label>
                <input
                  type="text"
                  value={formData.accountcode}
                  onChange={(e) => setFormData({ ...formData, accountcode: e.target.value })}
                  placeholder="Ej: ACC-001"
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div className="pl-3">
                <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                  Codecs
                </label>
                <input
                  type="text"
                  value={formData.codecs}
                  onChange={(e) => setFormData({ ...formData, codecs: e.target.value })}
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                />
                <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                  Lista separada por comas: PCMU,PCMA,G729,opus
                </p>
              </div>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Descripción
            </label>
            <textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              rows={2}
              placeholder="Teléfono de escritorio, oficina principal..."
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div className="flex items-center">
            <input
              type="checkbox"
              checked={formData.enabled}
              onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
              className="w-4 h-4 text-blue-600 rounded focus:ring-blue-500"
            />
            <label className="ml-2 text-sm font-medium text-[var(--color-text-secondary)]">
              Dispositivo activo
            </label>
          </div>

          <div className="flex justify-end space-x-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)]"
            >
              Cancelar
            </button>
            <button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
              {endpoint ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function DeleteModal({
  endpoint,
  onClose,
  onConfirm,
}: {
  endpoint: SipEndpoint
  onClose: () => void
  onConfirm: () => void
}) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-bold mb-2">Eliminar Dispositivo SIP</h3>
        <p className="text-[var(--color-text-secondary)] mb-4">
          ¿Estás seguro de eliminar <strong>{endpoint.name}</strong>?
        </p>
        <p className="text-sm text-[var(--color-text-tertiary)] mb-4">
          {endpoint.auth_mode === 'credentials'
            ? 'El dispositivo ya no podrá registrarse ni realizar llamadas.'
            : 'La troncal dejará de estar autorizada en Kamailio y ya no podrá cursar llamadas.'}
        </p>
        <div className="flex justify-end space-x-3">
          <button onClick={onClose} className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)]">
            Cancelar
          </button>
          <button onClick={onConfirm} className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700">
            Eliminar
          </button>
        </div>
      </div>
    </div>
  )
}

function PasswordModal({
  endpoint,
  onClose,
  onRegenerate,
  isRegenerating,
}: {
  endpoint: SipEndpointWithPassword
  onClose: () => void
  onRegenerate: () => void
  isRegenerating: boolean
}) {
  const [showPassword, setShowPassword] = useState(false)
  const [copied, setCopied] = useState(false)

  const copyToClipboard = () => {
    navigator.clipboard.writeText(endpoint.password)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-md w-full mx-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-bold">Contraseña SIP</h3>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-tertiary)] mb-1">
              Dispositivo
            </label>
            <p className="font-mono text-lg">
              {endpoint.sip_username}@{endpoint.sip_domain}:5080
            </p>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-tertiary)] mb-1">
              Contraseña
            </label>
            <div className="flex items-center space-x-2">
              <div className="flex-1 relative">
                <input
                  type={showPassword ? 'text' : 'password'}
                  value={endpoint.password}
                  readOnly
                  className="w-full px-3 py-2 pr-20 font-mono border rounded-lg bg-[var(--color-bg-secondary)]"
                />
                <button
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 p-1 hover:bg-[var(--color-bg-tertiary)] rounded"
                >
                  {showPassword ? (
                    <EyeOff className="w-4 h-4 text-[var(--color-text-tertiary)]" />
                  ) : (
                    <Eye className="w-4 h-4 text-[var(--color-text-tertiary)]" />
                  )}
                </button>
              </div>
              <button
                onClick={copyToClipboard}
                className="p-2 hover:bg-[var(--color-bg-secondary)] rounded border"
                title="Copiar"
              >
                {copied ? (
                  <CheckCircle className="w-5 h-5 text-green-600" />
                ) : (
                  <Copy className="w-5 h-5 text-[var(--color-text-tertiary)]" />
                )}
              </button>
            </div>
          </div>

          <div className="pt-4 border-t">
            <button
              onClick={onRegenerate}
              disabled={isRegenerating}
              className="w-full px-4 py-2 bg-amber-500 text-white rounded-lg hover:bg-amber-600 flex items-center justify-center disabled:opacity-50"
            >
              <RefreshCw className={`w-4 h-4 mr-2 ${isRegenerating ? 'animate-spin' : ''}`} />
              {isRegenerating ? 'Regenerando...' : 'Regenerar Contraseña'}
            </button>
            <p className="text-xs text-[var(--color-text-tertiary)] mt-2 text-center">
              El dispositivo deberá registrarse nuevamente con la nueva contraseña
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}

function CreatedEndpointModal({
  endpoint,
  onClose,
}: {
  endpoint: SipEndpointWithPassword
  onClose: () => void
}) {
  const [copied, setCopied] = useState(false)
  const sipPort = 5080

  const copyCredentials = () => {
    const text = `Usuario: ${endpoint.sip_username}\nServidor: ${endpoint.sip_domain}\nPuerto: ${sipPort}\nContraseña: ${endpoint.password}`
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-md w-full mx-4">
        <div className="text-center mb-4">
          <div className="w-12 h-12 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-3">
            <CheckCircle className="w-6 h-6 text-green-600" />
          </div>
          <h3 className="text-lg font-bold">Dispositivo Creado</h3>
          <p className="text-[var(--color-text-secondary)] text-sm mt-1">
            Guarda estas credenciales, la contraseña no se mostrará de nuevo por seguridad.
          </p>
        </div>

        <div className="bg-[var(--color-bg-secondary)] rounded-lg p-4 space-y-3">
          <div>
            <label className="block text-xs font-medium text-[var(--color-text-tertiary)]">Usuario SIP</label>
            <p className="font-mono font-medium">{endpoint.sip_username}</p>
          </div>
          <div>
            <label className="block text-xs font-medium text-[var(--color-text-tertiary)]">Servidor SIP</label>
            <p className="font-mono">
              {endpoint.sip_domain}:{sipPort}
            </p>
          </div>
          <div>
            <label className="block text-xs font-medium text-[var(--color-text-tertiary)]">Contraseña</label>
            <p className="font-mono font-medium text-lg">{endpoint.password}</p>
          </div>
        </div>

        <div className="flex space-x-3 mt-4">
          <button
            onClick={copyCredentials}
            className="flex-1 px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)] flex items-center justify-center"
          >
            {copied ? (
              <>
                <CheckCircle className="w-4 h-4 mr-2 text-green-600" />
                Copiado
              </>
            ) : (
              <>
                <Copy className="w-4 h-4 mr-2" />
                Copiar
              </>
            )}
          </button>
          <button
            onClick={onClose}
            className="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Entendido
          </button>
        </div>
      </div>
    </div>
  )
}

function RegistrationStatusModal({
  endpoint,
  status,
  onClose,
  onRefresh,
  isRefreshing,
}: {
  endpoint: SipEndpoint
  status: SipRegistrationStatus
  onClose: () => void
  onRefresh: () => void
  isRefreshing: boolean
}) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-lg w-full mx-4">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-3">
            {status.registered ? (
              <div className="w-10 h-10 bg-green-100 rounded-full flex items-center justify-center">
                <Wifi className="w-5 h-5 text-green-600" />
              </div>
            ) : (
              <div className="w-10 h-10 bg-[var(--color-bg-secondary)] rounded-full flex items-center justify-center">
                <WifiOff className="w-5 h-5 text-[var(--color-text-muted)]" />
              </div>
            )}
            <div>
              <h3 className="text-lg font-bold">Estado de Registro</h3>
              <p className="text-sm text-[var(--color-text-tertiary)]">
                {endpoint.sip_username}@{endpoint.sip_domain}
              </p>
            </div>
          </div>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="mb-4">
          {status.registered ? (
            <span className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-green-100 text-green-800">
              <span className="w-2 h-2 bg-green-500 rounded-full mr-2 animate-pulse" />
              Registrado
            </span>
          ) : (
            <span className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)]">
              <span className="w-2 h-2 bg-[var(--color-text-muted)] rounded-full mr-2" />
              No registrado
            </span>
          )}
        </div>

        {status.registered && (
          <div className="bg-[var(--color-bg-secondary)] rounded-lg p-4 space-y-3">
            {status.contact && (
              <div className="flex items-start space-x-3">
                <Phone className="w-4 h-4 text-[var(--color-text-muted)] mt-0.5" />
                <div>
                  <p className="text-xs font-medium text-[var(--color-text-tertiary)]">Contact</p>
                  <p className="font-mono text-sm break-all">{status.contact}</p>
                </div>
              </div>
            )}

            {status.ip && (
              <div className="flex items-start space-x-3">
                <Globe className="w-4 h-4 text-[var(--color-text-muted)] mt-0.5" />
                <div>
                  <p className="text-xs font-medium text-[var(--color-text-tertiary)]">Dirección IP</p>
                  <p className="font-mono text-sm">
                    {status.ip}
                    {status.port && <span className="text-[var(--color-text-muted)]">:{status.port}</span>}
                  </p>
                </div>
              </div>
            )}

            {status.user_agent && (
              <div className="flex items-start space-x-3">
                <Monitor className="w-4 h-4 text-[var(--color-text-muted)] mt-0.5" />
                <div>
                  <p className="text-xs font-medium text-[var(--color-text-tertiary)]">Dispositivo</p>
                  <p className="text-sm">{status.user_agent}</p>
                </div>
              </div>
            )}

            {status.ping_status && (
              <div className="flex items-start space-x-3">
                <Activity className="w-4 h-4 text-[var(--color-text-muted)] mt-0.5" />
                <div>
                  <p className="text-xs font-medium text-[var(--color-text-tertiary)]">Ping</p>
                  <p className="text-sm">
                    <span
                      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                        status.ping_status === 'Reachable'
                          ? 'bg-green-100 text-green-700'
                          : 'bg-red-100 text-red-700'
                      }`}
                    >
                      {status.ping_status}
                    </span>
                  </p>
                </div>
              </div>
            )}

            {(status.expires_at || status.expires_seconds) && (
              <div className="flex items-start space-x-3">
                <Clock className="w-4 h-4 text-[var(--color-text-muted)] mt-0.5" />
                <div>
                  <p className="text-xs font-medium text-[var(--color-text-tertiary)]">Expiración</p>
                  <p className="text-sm">
                    {status.expires_at && <span>{status.expires_at}</span>}
                    {status.expires_seconds && (
                      <span className="text-[var(--color-text-tertiary)] ml-1">
                        ({status.expires_seconds} segundos)
                      </span>
                    )}
                  </p>
                </div>
              </div>
            )}
          </div>
        )}

        {!status.registered && (
          <div className="bg-[var(--color-bg-secondary)] rounded-lg p-4 text-center">
            <WifiOff className="w-12 h-12 text-[var(--color-text-muted)] mx-auto mb-2" />
            <p className="text-[var(--color-text-tertiary)] text-sm">
              El dispositivo no está registrado actualmente en el servidor SIP.
            </p>
            <p className="text-[var(--color-text-muted)] text-xs mt-1">
              Verifica que el dispositivo esté encendido y configurado correctamente.
            </p>
          </div>
        )}

        <div className="flex justify-end space-x-3 mt-4 pt-4 border-t">
          <button
            onClick={onRefresh}
            disabled={isRefreshing}
            className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)] flex items-center disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
            Actualizar
          </button>
          <button onClick={onClose} className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
            Cerrar
          </button>
        </div>
      </div>
    </div>
  )
}
