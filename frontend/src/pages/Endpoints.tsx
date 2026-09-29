import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchSipEndpoints,
  createSipEndpoint,
  updateSipEndpoint,
  deleteSipEndpoint,
  syncAllSipEndpoints,
} from '../api/client'
import type {
  SipEndpoint,
  CreateSipEndpointRequest,
  UpdateSipEndpointRequest,
  TransportProtocol,
} from '../types'

import * as Dialog from '@radix-ui/react-dialog'
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import {
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  Server,
  Check,
  X,
  AlertTriangle,
  Star,
} from 'lucide-react'

const Endpoints = () => {
  const queryClient = useQueryClient()
  const [isModalOpen, setIsModalOpen] = useState(false)
  const [editingEndpoint, setEditingEndpoint] = useState<SipEndpoint | null>(null)
  const [deleteEndpoint, setDeleteEndpoint] = useState<SipEndpoint | null>(null)
  const [formData, setFormData] = useState<CreateSipEndpointRequest>({
    name: '',
    host: '',
    port: 5060,
    transport: 'udp',
    fs_profile: 'internal',
    enabled: true,
    is_default: false,
  })

  const { data, isLoading, error } = useQuery({
    queryKey: ['sip-endpoints'],
    queryFn: fetchSipEndpoints,
  })

  const createMutation = useMutation({
    mutationFn: createSipEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      closeModal()
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateSipEndpointRequest }) =>
      updateSipEndpoint(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      closeModal()
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteSipEndpoint,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
      setDeleteEndpoint(null)
    },
  })

  const syncMutation = useMutation({
    mutationFn: syncAllSipEndpoints,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['sip-endpoints'] })
    },
  })

  const closeModal = () => {
    setIsModalOpen(false)
    setEditingEndpoint(null)
    setFormData({
      name: '',
      host: '',
      port: 5060,
      transport: 'udp',
      fs_profile: 'internal',
      enabled: true,
      is_default: false,
    })
  }

  const openCreateModal = () => {
    setEditingEndpoint(null)
    setFormData({
      name: '',
      host: '',
      port: 5060,
      transport: 'udp',
      fs_profile: 'internal',
      enabled: true,
      is_default: false,
    })
    setIsModalOpen(true)
  }

  const openEditModal = (endpoint: SipEndpoint) => {
    setEditingEndpoint(endpoint)
    setFormData({
      name: endpoint.name,
      description: endpoint.description,
      host: endpoint.host,
      port: endpoint.port,
      transport: endpoint.transport,
      fs_profile: endpoint.fs_profile,
      enabled: endpoint.enabled,
      is_default: endpoint.is_default,
    })
    setIsModalOpen(true)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (editingEndpoint) {
      updateMutation.mutate({ id: editingEndpoint.id, data: formData })
    } else {
      createMutation.mutate(formData)
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
        Error loading endpoints: {(error as Error).message}
      </div>
    )
  }

  const endpoints = data?.endpoints || []

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            Endpoints
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Internal PBX connections (dr_gateways type=9)
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
            Add Endpoint
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Server className="w-8 h-8 text-blue-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.total || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Total Endpoints
              </p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Check className="w-8 h-8 text-green-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.enabled_count || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Enabled
              </p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Star className="w-8 h-8 text-yellow-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {endpoints.filter((e) => e.is_default).length}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Default
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
                Endpoint
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Host
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                FS Profile
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Sync
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Status
              </th>
              <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Actions
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
            {endpoints.map((endpoint) => (
              <tr
                key={endpoint.id}
                className="hover:bg-gray-50 dark:hover:bg-gray-700/50"
              >
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="flex items-center gap-2">
                    {endpoint.is_default && (
                      <Star className="w-4 h-4 text-yellow-500" />
                    )}
                    <div>
                      <div className="font-medium text-gray-900 dark:text-white">
                        {endpoint.name}
                      </div>
                      {endpoint.description && (
                        <div className="text-sm text-gray-500 dark:text-gray-400">
                          {endpoint.description}
                        </div>
                      )}
                    </div>
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="text-sm text-gray-900 dark:text-white">
                    {endpoint.host}:{endpoint.port}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {endpoint.transport.toUpperCase()}
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400">
                    {endpoint.fs_profile}
                  </span>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  {getSyncStatusBadge(endpoint.sync_status)}
                  {endpoint.kamailio_gwid && (
                    <span className="ml-2 text-xs text-gray-500">
                      gwid: {endpoint.kamailio_gwid}
                    </span>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  {endpoint.enabled ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
                      Enabled
                    </span>
                  ) : (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-400">
                      Disabled
                    </span>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap text-right">
                  <div className="flex items-center justify-end gap-2">
                    <button
                      onClick={() => openEditModal(endpoint)}
                      className="p-1 text-gray-400 hover:text-blue-600 dark:hover:text-blue-400"
                      title="Edit"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => setDeleteEndpoint(endpoint)}
                      className="p-1 text-gray-400 hover:text-red-600 dark:hover:text-red-400"
                      title="Delete"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {endpoints.length === 0 && (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-12 text-center text-gray-500 dark:text-gray-400"
                >
                  No endpoints configured. Click "Add Endpoint" to create one.
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
              {editingEndpoint ? 'Edit Endpoint' : 'Add Endpoint'}
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
                    placeholder="127.0.0.1"
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
              <div className="grid grid-cols-2 gap-4">
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
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    FS Profile
                  </label>
                  <select
                    value={formData.fs_profile}
                    onChange={(e) =>
                      setFormData({ ...formData, fs_profile: e.target.value })
                    }
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  >
                    <option value="internal">internal</option>
                    <option value="external">external</option>
                    <option value="inbound">inbound</option>
                  </select>
                </div>
              </div>
              <div className="flex items-center gap-4">
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
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="is_default"
                    checked={formData.is_default}
                    onChange={(e) =>
                      setFormData({ ...formData, is_default: e.target.checked })
                    }
                    className="rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                  />
                  <label
                    htmlFor="is_default"
                    className="text-sm text-gray-700 dark:text-gray-300"
                  >
                    Default Endpoint
                  </label>
                </div>
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
                    : editingEndpoint
                    ? 'Update'
                    : 'Create'}
                </button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Delete Confirmation */}
      <AlertDialog.Root
        open={!!deleteEndpoint}
        onOpenChange={() => setDeleteEndpoint(null)}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/50" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-sm p-6">
            <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              Delete Endpoint
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              Are you sure you want to delete "{deleteEndpoint?.name}"? This
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
                    deleteEndpoint && deleteMutation.mutate(deleteEndpoint.id)
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

export default Endpoints
