import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchSipDevices,
  createSipDevice,
  updateSipDevice,
  deleteSipDevice,
  getSipDevicePassword,
  regenerateSipDevicePassword,
  fetchAccounts,
  getSipDeviceRegistrationStatus,
  type SipRegistrationStatus,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
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
} from 'lucide-react'
import type { SipDevice, SipDeviceWithPassword, Account } from '../types'

export default function SipDevicesPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingDevice, setEditingDevice] = useState<SipDevice | null>(null)
  const [deletingDevice, setDeletingDevice] = useState<SipDevice | null>(null)
  const [viewingPassword, setViewingPassword] = useState<SipDeviceWithPassword | null>(null)
  const [viewingStatus, setViewingStatus] = useState<{ device: SipDevice; status: SipRegistrationStatus } | null>(null)
  const [error, setError] = useState('')
  const [createdDevice, setCreatedDevice] = useState<SipDeviceWithPassword | null>(null)
  const queryClient = useQueryClient()

  const { data: devices = [], isLoading } = useQuery({
    queryKey: ['sip-devices'],
    queryFn: () => fetchSipDevices(),
  })

  const { data: accounts = [] } = useQuery({
    queryKey: ['accounts'],
    queryFn: fetchAccounts,
  })

  const createMutation = useMutation({
    mutationFn: createSipDevice,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['sip-devices'] })
      setShowModal(false)
      setEditingDevice(null)
      setError('')
      setCreatedDevice(data)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      setError(err.response?.data?.message || err.message)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<SipDevice> }) =>
      updateSipDevice(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-devices'] })
      setShowModal(false)
      setEditingDevice(null)
      setError('')
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      setError(err.response?.data?.message || err.message)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteSipDevice,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-devices'] })
      setDeletingDevice(null)
    },
  })

  const getPasswordMutation = useMutation({
    mutationFn: getSipDevicePassword,
    onSuccess: (data) => {
      setViewingPassword(data)
    },
  })

  const regeneratePasswordMutation = useMutation({
    mutationFn: regenerateSipDevicePassword,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['sip-devices'] })
      setViewingPassword({
        ...viewingPassword!,
        password: data.new_password,
      })
    },
  })

  const getStatusMutation = useMutation({
    mutationFn: (device: SipDevice) => getSipDeviceRegistrationStatus(device.id),
    onSuccess: (status, device) => {
      setViewingStatus({ device, status })
    },
  })

  const handleEdit = (device: SipDevice) => {
    setEditingDevice(device)
    setShowModal(true)
    setError('')
  }

  const handleCreate = () => {
    setEditingDevice(null)
    setShowModal(true)
    setError('')
  }

  const handleDelete = () => {
    if (deletingDevice) {
      deleteMutation.mutate(deletingDevice.id)
    }
  }

  const handleViewPassword = (device: SipDevice) => {
    getPasswordMutation.mutate(device.id)
  }

  const handleViewStatus = (device: SipDevice) => {
    getStatusMutation.mutate(device)
  }

  const getAccountName = (accountId: number) => {
    const account = accounts.find((a: Account) => a.id === accountId)
    return account?.account_number || `ID: ${accountId}`
  }

  const columns = [
    {
      key: 'sip_username',
      header: 'Usuario SIP',
      render: (device: SipDevice) => (
        <div>
          <span className="font-mono font-medium">{device.sip_username}</span>
          <span className="text-[var(--color-text-muted)]">@{device.sip_domain}</span>
        </div>
      ),
    },
    {
      key: 'display_name',
      header: 'Nombre',
      render: (device: SipDevice) => (
        <span>{device.display_name || '-'}</span>
      ),
    },
    {
      key: 'account_id',
      header: 'Cuenta',
      render: (device: SipDevice) => (
        <span className="text-sm">{getAccountName(device.account_id)}</span>
      ),
    },
    {
      key: 'context',
      header: 'Contexto',
      render: (device: SipDevice) => (
        <span className="font-mono text-xs bg-[var(--color-bg-secondary)] px-2 py-1 rounded">
          {device.context}
        </span>
      ),
    },
    {
      key: 'max_registrations',
      header: 'Registros',
      render: (device: SipDevice) => <span>{device.max_registrations}</span>,
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (device: SipDevice) => (
        <Badge variant={device.enabled ? 'success' : 'error'}>
          {device.enabled ? 'Activo' : 'Inactivo'}
        </Badge>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (device: SipDevice) => (
        <div className="flex items-center space-x-1">
          <button
            onClick={() => handleViewStatus(device)}
            className="p-1.5 hover:bg-blue-50 rounded"
            title="Ver estado de registro"
          >
            <Activity className="w-4 h-4 text-blue-600" />
          </button>
          <button
            onClick={() => handleViewPassword(device)}
            className="p-1.5 hover:bg-[var(--color-bg-secondary)] rounded"
            title="Ver contraseña"
          >
            <Eye className="w-4 h-4 text-[var(--color-text-secondary)]" />
          </button>
          <button
            onClick={() => handleEdit(device)}
            className="p-1.5 hover:bg-[var(--color-bg-secondary)] rounded"
            title="Editar"
          >
            <Edit className="w-4 h-4 text-[var(--color-text-secondary)]" />
          </button>
          <button
            onClick={() => setDeletingDevice(device)}
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
            Gestión de teléfonos IP y softphones
          </p>
        </div>
        <button
          onClick={handleCreate}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center"
        >
          <Plus className="w-4 h-4 mr-2" />
          Nuevo Dispositivo
        </button>
      </div>

      <DataTable
        data={devices}
        columns={columns}
        loading={isLoading}
        emptyMessage="No hay dispositivos SIP configurados"
      />

      {showModal && (
        <DeviceModal
          device={editingDevice}
          accounts={accounts}
          error={error}
          onClose={() => {
            setShowModal(false)
            setEditingDevice(null)
            setError('')
          }}
          onSave={(data) => {
            if (editingDevice) {
              updateMutation.mutate({ id: editingDevice.id, data })
            } else {
              createMutation.mutate(data)
            }
          }}
        />
      )}

      {deletingDevice && (
        <DeleteModal
          device={deletingDevice}
          onClose={() => setDeletingDevice(null)}
          onConfirm={handleDelete}
        />
      )}

      {viewingPassword && (
        <PasswordModal
          device={viewingPassword}
          onClose={() => setViewingPassword(null)}
          onRegenerate={() => regeneratePasswordMutation.mutate(viewingPassword.id)}
          isRegenerating={regeneratePasswordMutation.isPending}
        />
      )}

      {createdDevice && (
        <CreatedDeviceModal
          device={createdDevice}
          onClose={() => setCreatedDevice(null)}
        />
      )}

      {viewingStatus && (
        <RegistrationStatusModal
          device={viewingStatus.device}
          status={viewingStatus.status}
          onClose={() => setViewingStatus(null)}
          onRefresh={() => getStatusMutation.mutate(viewingStatus.device)}
          isRefreshing={getStatusMutation.isPending}
        />
      )}
    </div>
  )
}

function DeviceModal({
  device,
  accounts,
  error,
  onClose,
  onSave,
}: {
  device: SipDevice | null
  accounts: Account[]
  error: string
  onClose: () => void
  onSave: (data: any) => void
}) {
  const [formData, setFormData] = useState({
    account_id: device?.account_id || (accounts[0]?.id || 0),
    sip_username: device?.sip_username || '',
    sip_domain: device?.sip_domain || '10.10.22.4',
    display_name: device?.display_name || '',
    description: device?.description || '',
    context: device?.context || 'from-pbx',
    accountcode: device?.accountcode || '',
    codecs: device?.codecs || 'PCMU,PCMA,G729,opus',
    max_registrations: device?.max_registrations || 3,
    enabled: device?.enabled ?? true,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSave(formData)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold">
            {device ? 'Editar Dispositivo SIP' : 'Nuevo Dispositivo SIP'}
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
                Usuario SIP *
              </label>
              <input
                type="text"
                required
                disabled={!!device}
                value={formData.sip_username}
                onChange={(e) =>
                  setFormData({ ...formData, sip_username: e.target.value })
                }
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
                disabled={!!device}
                value={formData.sip_domain}
                onChange={(e) =>
                  setFormData({ ...formData, sip_domain: e.target.value })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 disabled:bg-[var(--color-bg-secondary)]"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Puerto SIP: 5080 (configurar en softphone)
              </p>
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Cuenta de Facturación *
            </label>
            <select
              required
              value={formData.account_id}
              onChange={(e) =>
                setFormData({ ...formData, account_id: Number(e.target.value) })
              }
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            >
              {accounts.map((account: Account) => (
                <option key={account.id} value={account.id}>
                  {account.account_number} - {account.customer_phone || 'Sin teléfono'}
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
                onChange={(e) =>
                  setFormData({ ...formData, display_name: e.target.value })
                }
                placeholder="Juan Pérez"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Contexto
              </label>
              <select
                value={formData.context}
                onChange={(e) =>
                  setFormData({ ...formData, context: e.target.value })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              >
                <option value="from-pbx">from-pbx (Saliente)</option>
                <option value="from-internal">from-internal (Interno)</option>
                <option value="public">public (Público)</option>
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Código de Cuenta (CDR)
              </label>
              <input
                type="text"
                value={formData.accountcode}
                onChange={(e) =>
                  setFormData({ ...formData, accountcode: e.target.value })
                }
                placeholder="Ej: ACC-001"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Usado para vincular CDRs con facturación
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Registros Simultáneos
              </label>
              <input
                type="number"
                min="1"
                max="10"
                value={formData.max_registrations}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    max_registrations: Number(e.target.value),
                  })
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Codecs
            </label>
            <input
              type="text"
              value={formData.codecs}
              onChange={(e) =>
                setFormData({ ...formData, codecs: e.target.value })
              }
              className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
              Lista separada por comas: PCMU,PCMA,G729,opus
            </p>
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
              placeholder="Teléfono de escritorio, oficina principal..."
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
            <button
              type="submit"
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
            >
              {device ? 'Actualizar' : 'Crear'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function DeleteModal({
  device,
  onClose,
  onConfirm,
}: {
  device: SipDevice
  onClose: () => void
  onConfirm: () => void
}) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-bold mb-2">Eliminar Dispositivo SIP</h3>
        <p className="text-[var(--color-text-secondary)] mb-4">
          ¿Estás seguro de eliminar el dispositivo{' '}
          <strong>{device.sip_username}@{device.sip_domain}</strong>?
        </p>
        <p className="text-sm text-[var(--color-text-tertiary)] mb-4">
          El dispositivo ya no podrá registrarse ni realizar llamadas.
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

function PasswordModal({
  device,
  onClose,
  onRegenerate,
  isRegenerating,
}: {
  device: SipDeviceWithPassword
  onClose: () => void
  onRegenerate: () => void
  isRegenerating: boolean
}) {
  const [showPassword, setShowPassword] = useState(false)
  const [copied, setCopied] = useState(false)

  const copyToClipboard = () => {
    navigator.clipboard.writeText(device.password)
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
              {device.sip_username}@{device.sip_domain}:5080
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
                  value={device.password}
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

function CreatedDeviceModal({
  device,
  onClose,
}: {
  device: SipDeviceWithPassword
  onClose: () => void
}) {
  const [copied, setCopied] = useState(false)
  // Puerto SIP para FreeSWITCH internal profile
  const sipPort = 5080

  const copyCredentials = () => {
    const text = `Usuario: ${device.sip_username}\nServidor: ${device.sip_domain}\nPuerto: ${sipPort}\nContraseña: ${device.password}`
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
            <p className="font-mono font-medium">{device.sip_username}</p>
          </div>
          <div>
            <label className="block text-xs font-medium text-[var(--color-text-tertiary)]">Servidor SIP</label>
            <p className="font-mono">{device.sip_domain}:{sipPort}</p>
          </div>
          <div>
            <label className="block text-xs font-medium text-[var(--color-text-tertiary)]">Contraseña</label>
            <p className="font-mono font-medium text-lg">{device.password}</p>
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
  device,
  status,
  onClose,
  onRefresh,
  isRefreshing,
}: {
  device: SipDevice
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
                {device.sip_username}@{device.sip_domain}
              </p>
            </div>
          </div>
          <button onClick={onClose} className="p-1 hover:bg-[var(--color-bg-secondary)] rounded">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Status badge */}
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

        {/* Registration details */}
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

        {/* Actions */}
        <div className="flex justify-end space-x-3 mt-4 pt-4 border-t">
          <button
            onClick={onRefresh}
            disabled={isRefreshing}
            className="px-4 py-2 border rounded-lg hover:bg-[var(--color-bg-secondary)] flex items-center disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
            Actualizar
          </button>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  )
}
