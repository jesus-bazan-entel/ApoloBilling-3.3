import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchSipCarriers,
  createSipCarrier,
  updateSipCarrier,
  deleteSipCarrier,
  checkSipCarrierStatus,
  syncAllSipCarriers,
} from '../api/client'
import type {
  SipCarrier,
  CreateSipCarrierRequest,
  UpdateSipCarrierRequest,
  TransportProtocol,
  CarrierSipCheckResult,
} from '../types'

import * as Dialog from '@radix-ui/react-dialog'
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import {
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  Activity,
  Server,
  Check,
  X,
  AlertTriangle,
  Clock,
  Wifi,
  WifiOff,
} from 'lucide-react'

const Carriers = () => {
  const queryClient = useQueryClient()
  const [isModalOpen, setIsModalOpen] = useState(false)
  const [editingCarrier, setEditingCarrier] = useState<SipCarrier | null>(null)
  const [deleteCarrier, setDeleteCarrier] = useState<SipCarrier | null>(null)
  const [sipCheckResult, setSipCheckResult] = useState<CarrierSipCheckResult | null>(null)
  const [sipCheckError, setSipCheckError] = useState<string | null>(null)
  const [formData, setFormData] = useState<CreateSipCarrierRequest>({
    name: '',
    host: '',
    port: 5060,
    transport: 'udp',
    enabled: true,
  })

  const { data, isLoading, error } = useQuery({
    queryKey: ['sip-carriers'],
    queryFn: fetchSipCarriers,
  })

  const createMutation = useMutation({
    mutationFn: createSipCarrier,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
      closeModal()
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateSipCarrierRequest }) =>
      updateSipCarrier(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
      closeModal()
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteSipCarrier,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
      setDeleteCarrier(null)
    },
  })

  const checkStatusMutation = useMutation({
    mutationFn: checkSipCarrierStatus,
    onSuccess: (result) => {
      setSipCheckError(null)
      setSipCheckResult(result)
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
    },
    onError: (error) => {
      const axiosError = error as { response?: { data?: { message?: string } }; message?: string }
      setSipCheckResult(null)
      setSipCheckError(
        axiosError.response?.data?.message ||
          axiosError.message ||
          'Failed to check SIP status'
      )
    },
  })

  const syncMutation = useMutation({
    mutationFn: syncAllSipCarriers,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
    },
  })

  const closeModal = () => {
    setIsModalOpen(false)
    setEditingCarrier(null)
    setFormData({
      name: '',
      host: '',
      port: 5060,
      transport: 'udp',
      enabled: true,
    })
  }

  const openCreateModal = () => {
    setEditingCarrier(null)
    setFormData({
      name: '',
      host: '',
      port: 5060,
      transport: 'udp',
      enabled: true,
    })
    setIsModalOpen(true)
  }

  const openEditModal = (carrier: SipCarrier) => {
    setEditingCarrier(carrier)
    setFormData({
      name: carrier.name,
      description: carrier.description,
      host: carrier.host,
      port: carrier.port,
      transport: carrier.transport,
      auth_username: carrier.auth_username,
      strip_digits: carrier.strip_digits,
      prefix_to_add: carrier.prefix_to_add,
      enabled: carrier.enabled,
    })
    setIsModalOpen(true)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (editingCarrier) {
      updateMutation.mutate({ id: editingCarrier.id, data: formData })
    } else {
      createMutation.mutate(formData)
    }
  }

  const getSipStatusIcon = (status: string) => {
    switch (status) {
      case 'reachable':
        return <Wifi className="w-4 h-4 text-green-500" />
      case 'unreachable':
        return <WifiOff className="w-4 h-4 text-red-500" />
      case 'checking':
        return <RefreshCw className="w-4 h-4 text-yellow-500 animate-spin" />
      default:
        return <Clock className="w-4 h-4 text-gray-400" />
    }
  }

  const getSyncStatusBadge = (status: string) => {
    switch (status) {
      case 'synced':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
            <Check className="w-3 h-3" />
            Synced
          </span>
        )
      case 'error':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400">
            <X className="w-3 h-3" />
            Error
          </span>
        )
      default:
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400">
            <AlertTriangle className="w-3 h-3" />
            Pending
          </span>
        )
    }
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <RefreshCw className="w-8 h-8 animate-spin text-blue-500" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="p-4 bg-red-100 text-red-700 rounded-lg dark:bg-red-900/30 dark:text-red-400">
        Error loading carriers: {(error as Error).message}
      </div>
    )
  }

  const carriers = data?.carriers || []

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            Carriers
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            External SIP providers (dr_gateways type=8)
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => syncMutation.mutate()}
            disabled={syncMutation.isPending}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-800 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-700 disabled:opacity-50"
          >
            <RefreshCw
              className={`w-4 h-4 ${syncMutation.isPending ? 'animate-spin' : ''}`}
            />
            Sync to Kamailio
          </button>
          <button
            onClick={openCreateModal}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700"
          >
            <Plus className="w-4 h-4" />
            Add Carrier
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Server className="w-8 h-8 text-blue-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.total || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Total Carriers
              </p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Wifi className="w-8 h-8 text-green-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.online_count || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">Online</p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <WifiOff className="w-8 h-8 text-red-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.offline_count || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Offline
              </p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Activity className="w-8 h-8 text-purple-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {carriers.filter((c) => c.enabled).length}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Enabled
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="bg-white rounded-lg border border-gray-200 overflow-hidden dark:bg-gray-800 dark:border-gray-700">
        <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead className="bg-gray-50 dark:bg-gray-900">
            <tr>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Carrier
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Host
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                SIP Status
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Sync
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Enabled
              </th>
              <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Actions
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
            {carriers.map((carrier) => (
              <tr
                key={carrier.id}
                className="hover:bg-gray-50 dark:hover:bg-gray-700/50"
              >
                <td className="px-6 py-4 whitespace-nowrap">
                  <div>
                    <div className="font-medium text-gray-900 dark:text-white">
                      {carrier.name}
                    </div>
                    {carrier.description && (
                      <div className="text-sm text-gray-500 dark:text-gray-400">
                        {carrier.description}
                      </div>
                    )}
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="text-sm text-gray-900 dark:text-white">
                    {carrier.host}:{carrier.port}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {carrier.transport.toUpperCase()}
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="flex items-center gap-2">
                    {getSipStatusIcon(carrier.sip_status)}
                    <span className="text-sm text-gray-700 dark:text-gray-300">
                      {carrier.sip_status}
                    </span>
                    {carrier.last_options_latency_ms && (
                      <span className="text-xs text-gray-500">
                        ({carrier.last_options_latency_ms}ms)
                      </span>
                    )}
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  {getSyncStatusBadge(carrier.sync_status)}
                  {carrier.kamailio_gwid && (
                    <span className="ml-2 text-xs text-gray-500">
                      gwid: {carrier.kamailio_gwid}
                    </span>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  {carrier.enabled ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
                      Yes
                    </span>
                  ) : (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-400">
                      No
                    </span>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap text-right">
                  <div className="flex items-center justify-end gap-2">
                    <button
                      onClick={() => checkStatusMutation.mutate(carrier.id)}
                      disabled={checkStatusMutation.isPending}
                      className="p-1 text-gray-400 hover:text-blue-600 dark:hover:text-blue-400"
                      title="Check SIP Status"
                    >
                      <Activity className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => openEditModal(carrier)}
                      className="p-1 text-gray-400 hover:text-blue-600 dark:hover:text-blue-400"
                      title="Edit"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => setDeleteCarrier(carrier)}
                      className="p-1 text-gray-400 hover:text-red-600 dark:hover:text-red-400"
                      title="Delete"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {carriers.length === 0 && (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-12 text-center text-gray-500 dark:text-gray-400"
                >
                  No carriers configured. Click "Add Carrier" to create one.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Create/Edit Modal */}
      <Dialog.Root open={isModalOpen} onOpenChange={setIsModalOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50" />
          <Dialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-md p-6">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {editingCarrier ? 'Edit Carrier' : 'Add Carrier'}
            </Dialog.Title>
            <form onSubmit={handleSubmit} className="mt-4 space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Name *
                </label>
                <input
                  type="text"
                  value={formData.name}
                  onChange={(e) =>
                    setFormData({ ...formData, name: e.target.value })
                  }
                  required
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Description
                </label>
                <input
                  type="text"
                  value={formData.description || ''}
                  onChange={(e) =>
                    setFormData({ ...formData, description: e.target.value })
                  }
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Host *
                  </label>
                  <input
                    type="text"
                    value={formData.host}
                    onChange={(e) =>
                      setFormData({ ...formData, host: e.target.value })
                    }
                    required
                    placeholder="10.0.0.1"
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Port
                  </label>
                  <input
                    type="number"
                    value={formData.port}
                    onChange={(e) =>
                      setFormData({ ...formData, port: parseInt(e.target.value) })
                    }
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Transport
                </label>
                <select
                  value={formData.transport}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      transport: e.target.value as TransportProtocol,
                    })
                  }
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                >
                  <option value="udp">UDP</option>
                  <option value="tcp">TCP</option>
                  <option value="tls">TLS</option>
                </select>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Auth Username
                  </label>
                  <input
                    type="text"
                    value={formData.auth_username || ''}
                    onChange={(e) =>
                      setFormData({ ...formData, auth_username: e.target.value })
                    }
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Auth Password
                  </label>
                  <input
                    type="password"
                    value={formData.auth_password || ''}
                    onChange={(e) =>
                      setFormData({ ...formData, auth_password: e.target.value })
                    }
                    placeholder={editingCarrier ? '(unchanged)' : ''}
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Strip Digits
                  </label>
                  <input
                    type="number"
                    value={formData.strip_digits || 0}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        strip_digits: parseInt(e.target.value),
                      })
                    }
                    min={0}
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Prefix to Add
                  </label>
                  <input
                    type="text"
                    value={formData.prefix_to_add || ''}
                    onChange={(e) =>
                      setFormData({ ...formData, prefix_to_add: e.target.value })
                    }
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  />
                </div>
              </div>
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="enabled"
                  checked={formData.enabled}
                  onChange={(e) =>
                    setFormData({ ...formData, enabled: e.target.checked })
                  }
                  className="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                />
                <label
                  htmlFor="enabled"
                  className="text-sm text-gray-700 dark:text-gray-300"
                >
                  Enabled
                </label>
              </div>
              <div className="flex justify-end gap-2 pt-4">
                <button
                  type="button"
                  onClick={closeModal}
                  className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-700 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-600"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={createMutation.isPending || updateMutation.isPending}
                  className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 disabled:opacity-50"
                >
                  {createMutation.isPending || updateMutation.isPending
                    ? 'Saving...'
                    : editingCarrier
                    ? 'Update'
                    : 'Create'}
                </button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* SIP Status Check Result Modal */}
      <Dialog.Root
        open={!!sipCheckResult || !!sipCheckError}
        onOpenChange={() => {
          setSipCheckResult(null)
          setSipCheckError(null)
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50" />
          <Dialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-md p-6">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              SIP Status Check
            </Dialog.Title>
            {sipCheckError ? (
              <div className="mt-4 p-3 rounded-lg bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 text-sm">
                {sipCheckError}
              </div>
            ) : sipCheckResult ? (
              <div className="mt-4 space-y-3">
                <div className="flex items-center gap-2">
                  {getSipStatusIcon(sipCheckResult.status)}
                  <span className="font-medium text-gray-900 dark:text-white">
                    {sipCheckResult.carrier_name}
                  </span>
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                  <dt className="text-gray-500 dark:text-gray-400">Status</dt>
                  <dd className="text-gray-900 dark:text-white">
                    {sipCheckResult.status}
                  </dd>
                  <dt className="text-gray-500 dark:text-gray-400">
                    Response Code
                  </dt>
                  <dd className="text-gray-900 dark:text-white">
                    {sipCheckResult.response_code ?? '—'}
                  </dd>
                  <dt className="text-gray-500 dark:text-gray-400">Latency</dt>
                  <dd className="text-gray-900 dark:text-white">
                    {sipCheckResult.latency_ms != null
                      ? `${sipCheckResult.latency_ms}ms`
                      : '—'}
                  </dd>
                  <dt className="text-gray-500 dark:text-gray-400">
                    Checked At
                  </dt>
                  <dd className="text-gray-900 dark:text-white">
                    {new Date(sipCheckResult.checked_at).toLocaleString()}
                  </dd>
                </dl>
                {sipCheckResult.error_message && (
                  <div className="p-3 rounded-lg bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 text-sm">
                    {sipCheckResult.error_message}
                  </div>
                )}
              </div>
            ) : null}
            <div className="flex justify-end pt-4">
              <button
                onClick={() => {
                  setSipCheckResult(null)
                  setSipCheckError(null)
                }}
                className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-700 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-600"
              >
                Close
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Delete Confirmation */}
      <AlertDialog.Root
        open={!!deleteCarrier}
        onOpenChange={() => setDeleteCarrier(null)}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/50" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-sm p-6">
            <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              Delete Carrier
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              Are you sure you want to delete "{deleteCarrier?.name}"? This
              action cannot be undone.
            </AlertDialog.Description>
            <div className="flex justify-end gap-2 mt-4">
              <AlertDialog.Cancel asChild>
                <button className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-700 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-600">
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  onClick={() =>
                    deleteCarrier && deleteMutation.mutate(deleteCarrier.id)
                  }
                  disabled={deleteMutation.isPending}
                  className="px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-lg hover:bg-red-700 disabled:opacity-50"
                >
                  {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </div>
  )
}

export default Carriers
