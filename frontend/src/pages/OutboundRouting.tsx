import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchOutboundRoutes,
  createOutboundRoute,
  updateOutboundRoute,
  deleteOutboundRoute,
  fetchSipCarriers,
  fetchCarrierGroups,
  syncAllToKamailio,
} from '../api/client'
import type {
  OutboundRouteNew,
  CreateOutboundRouteRequestNew,
  UpdateOutboundRouteRequestNew,
} from '../types'

import * as Dialog from '@radix-ui/react-dialog'
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import {
  Plus,
  Pencil,
  Trash2,
  RefreshCw,
  Route,
  Check,
  X,
  AlertTriangle,
  Server,
  Layers,
} from 'lucide-react'

const apiErrorMessage = (error: unknown, fallback: string): string => {
  const axiosError = error as {
    response?: { data?: { message?: string; error?: string } }
    message?: string
  }
  return (
    axiosError.response?.data?.message ||
    axiosError.response?.data?.error ||
    axiosError.message ||
    fallback
  )
}

const OutboundRouting = () => {
  const queryClient = useQueryClient()
  const [isModalOpen, setIsModalOpen] = useState(false)
  const [editingRoute, setEditingRoute] = useState<OutboundRouteNew | null>(null)
  const [deleteRoute, setDeleteRoute] = useState<OutboundRouteNew | null>(null)
  const [destinationType, setDestinationType] = useState<'carrier' | 'group'>('carrier')
  const [formData, setFormData] = useState<CreateOutboundRouteRequestNew>({
    name: '',
    prefix: '',
    enabled: true,
  })

  const { data, isLoading, error } = useQuery({
    queryKey: ['outbound-routes'],
    queryFn: fetchOutboundRoutes,
  })

  const { data: carriersData } = useQuery({
    queryKey: ['sip-carriers'],
    queryFn: fetchSipCarriers,
  })

  const { data: groupsData } = useQuery({
    queryKey: ['carrier-groups'],
    queryFn: fetchCarrierGroups,
  })

  const createMutation = useMutation({
    mutationFn: createOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['outbound-routes'] })
      closeModal()
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({
      id,
      data,
    }: {
      id: string
      data: UpdateOutboundRouteRequestNew
    }) => updateOutboundRoute(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['outbound-routes'] })
      closeModal()
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteOutboundRoute,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['outbound-routes'] })
      setDeleteRoute(null)
    },
  })

  const syncAllMutation = useMutation({
    mutationFn: syncAllToKamailio,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['outbound-routes'] })
      queryClient.invalidateQueries({ queryKey: ['sip-carriers'] })
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
    },
  })

  const closeModal = () => {
    setIsModalOpen(false)
    setEditingRoute(null)
    setDestinationType('carrier')
    setFormData({
      name: '',
      prefix: '',
      enabled: true,
    })
  }

  const openCreateModal = () => {
    setEditingRoute(null)
    setDestinationType('carrier')
    setFormData({
      name: '',
      prefix: '',
      enabled: true,
    })
    createMutation.reset()
    updateMutation.reset()
    setIsModalOpen(true)
  }

  const openEditModal = (route: OutboundRouteNew) => {
    setEditingRoute(route)
    setDestinationType(route.destination_type === 'group' ? 'group' : 'carrier')
    setFormData({
      name: route.name,
      description: route.description,
      prefix: route.prefix,
      carrier_group_id: route.carrier_group_id || undefined,
      carrier_id: route.carrier_id || undefined,
      enabled: route.enabled,
    })
    createMutation.reset()
    updateMutation.reset()
    setIsModalOpen(true)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    const submitData: CreateOutboundRouteRequestNew = {
      name: formData.name,
      description: formData.description,
      prefix: formData.prefix,
      enabled: formData.enabled,
    }

    if (destinationType === 'group') {
      submitData.carrier_group_id = formData.carrier_group_id
      submitData.carrier_id = undefined
    } else {
      submitData.carrier_id = formData.carrier_id
      submitData.carrier_group_id = undefined
    }

    if (editingRoute) {
      updateMutation.mutate({ id: editingRoute.id, data: submitData })
    } else {
      createMutation.mutate(submitData)
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
        Error loading routes: {(error as Error).message}
      </div>
    )
  }

  const routes = data?.routes || []
  const carriers = carriersData?.carriers || []
  const groups = groupsData?.groups || []

  const syncResult = syncAllMutation.data
  const syncFailedCount = syncResult
    ? syncResult.carriers_failed + syncResult.groups_failed + syncResult.routes_failed
    : 0
  const syncOk = !!syncResult && syncFailedCount === 0 && syncResult.kamailio_reloaded

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            Outbound Routing
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Prefix-based routing with auto-priority (longer prefix = higher
            priority)
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => syncAllMutation.mutate()}
            disabled={syncAllMutation.isPending}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-800 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-700 disabled:opacity-50"
          >
            <RefreshCw
              className={`w-4 h-4 ${syncAllMutation.isPending ? 'animate-spin' : ''}`}
            />
            Sync All to Kamailio
          </button>
          <button
            onClick={openCreateModal}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700"
          >
            <Plus className="w-4 h-4" />
            Add Route
          </button>
        </div>
      </div>

      {/* Sync Result */}
      {syncResult && (
        <div
          className={`p-4 rounded-lg border ${
            syncOk
              ? 'bg-green-50 border-green-200 dark:bg-green-900/20 dark:border-green-800'
              : 'bg-red-50 border-red-200 dark:bg-red-900/20 dark:border-red-800'
          }`}
        >
          <h3
            className={`font-medium ${
              syncOk
                ? 'text-green-800 dark:text-green-300'
                : 'text-red-800 dark:text-red-300'
            }`}
          >
            {syncOk ? 'Sync Complete' : 'Sync finished with errors'}
          </h3>
          <div
            className={`mt-2 text-sm grid grid-cols-3 gap-4 ${
              syncOk
                ? 'text-green-700 dark:text-green-400'
                : 'text-red-700 dark:text-red-400'
            }`}
          >
            <div>
              Carriers: {syncResult.carriers_synced} synced,{' '}
              {syncResult.carriers_failed} failed
            </div>
            <div>
              Groups: {syncResult.groups_synced} synced,{' '}
              {syncResult.groups_failed} failed
            </div>
            <div>
              Routes: {syncResult.routes_synced} synced,{' '}
              {syncResult.routes_failed} failed
            </div>
          </div>
          {!syncResult.kamailio_reloaded && (
            <p className="mt-2 text-sm text-red-700 dark:text-red-400">
              Kamailio did not reload its routing tables, the changes are not
              active yet.
            </p>
          )}
          {syncResult.errors.length > 0 && (
            <ul className="mt-2 text-sm text-red-700 dark:text-red-400 list-disc pl-5 space-y-1">
              {syncResult.errors.map((message, index) => (
                <li key={index} className="break-words">
                  {message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {syncAllMutation.error && (
        <div className="p-4 bg-red-50 border border-red-200 rounded-lg text-sm text-red-700 dark:bg-red-900/20 dark:border-red-800 dark:text-red-400">
          Sync failed:{' '}
          {apiErrorMessage(syncAllMutation.error, 'unknown error')}
        </div>
      )}

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Route className="w-8 h-8 text-blue-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.total || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Total Routes
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
            <RefreshCw className="w-8 h-8 text-purple-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {data?.synced_count || 0}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Synced
              </p>
            </div>
          </div>
        </div>
        <div className="p-4 bg-white rounded-lg border border-gray-200 dark:bg-gray-800 dark:border-gray-700">
          <div className="flex items-center gap-3">
            <Layers className="w-8 h-8 text-orange-500" />
            <div>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">
                {groups.length}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Carrier Groups
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
                Route
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Prefix
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider dark:text-gray-400">
                Destination
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
            {routes.map((route) => (
              <tr
                key={route.id}
                className="hover:bg-gray-50 dark:hover:bg-gray-700/50"
              >
                <td className="px-6 py-4 whitespace-nowrap">
                  <div>
                    <div className="font-medium text-gray-900 dark:text-white">
                      {route.name}
                    </div>
                    {route.description && (
                      <div className="text-sm text-gray-500 dark:text-gray-400">
                        {route.description}
                      </div>
                    )}
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="flex items-center gap-2">
                    <code className="px-2 py-1 text-sm font-mono bg-gray-100 rounded dark:bg-gray-700">
                      {route.prefix}
                    </code>
                    <span className="text-xs text-gray-500">
                      (priority: {route.priority})
                    </span>
                  </div>
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="flex items-center gap-2">
                    {route.destination_type === 'group' ? (
                      <Layers className="w-4 h-4 text-orange-500" />
                    ) : (
                      <Server className="w-4 h-4 text-blue-500" />
                    )}
                    <span className="text-sm text-gray-900 dark:text-white">
                      {route.destination_name || 'Unknown'}
                    </span>
                    <span className="text-xs text-gray-500">
                      ({route.destination_type})
                    </span>
                  </div>
                </td>
                <td className="px-6 py-4 max-w-xs">
                  <div className="whitespace-nowrap">
                    {getSyncStatusBadge(route.sync_status)}
                    {route.kamailio_ruleid && (
                      <span className="ml-2 text-xs text-gray-500">
                        rule: {route.kamailio_ruleid}
                      </span>
                    )}
                  </div>
                  {route.sync_status === 'error' && route.sync_error && (
                    <p
                      className="mt-1 text-xs text-red-600 dark:text-red-400 break-words"
                      title={route.sync_error}
                    >
                      {route.sync_error}
                    </p>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  {route.enabled ? (
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
                      onClick={() => openEditModal(route)}
                      className="p-1 text-gray-400 hover:text-blue-600 dark:hover:text-blue-400"
                      title="Edit"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => setDeleteRoute(route)}
                      className="p-1 text-gray-400 hover:text-red-600 dark:hover:text-red-400"
                      title="Delete"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {routes.length === 0 && (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-12 text-center text-gray-500 dark:text-gray-400"
                >
                  No outbound routes configured. Click "Add Route" to create
                  one.
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
              {editingRoute ? 'Edit Route' : 'Add Outbound Route'}
            </Dialog.Title>
            <form onSubmit={handleSubmit} className="mt-4 space-y-4">
              {(createMutation.error || updateMutation.error) && (
                <div className="p-3 text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg dark:bg-red-900/20 dark:border-red-800 dark:text-red-400">
                  {apiErrorMessage(
                    createMutation.error || updateMutation.error,
                    'Could not save the route'
                  )}
                </div>
              )}
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
                  placeholder="e.g., Argentina Mobile"
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
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Prefix *
                </label>
                <input
                  type="text"
                  value={formData.prefix}
                  onChange={(e) =>
                    setFormData({ ...formData, prefix: e.target.value })
                  }
                  required
                  placeholder="e.g., 5411"
                  pattern="[0-9]+"
                  className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white font-mono"
                />
                <p className="mt-1 text-xs text-gray-500">
                  Digits only. Longer prefixes have higher priority.
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Destination Type
                </label>
                <div className="flex gap-4">
                  <label className="flex items-center gap-2">
                    <input
                      type="radio"
                      name="destType"
                      checked={destinationType === 'carrier'}
                      onChange={() => setDestinationType('carrier')}
                      className="text-blue-600 focus:ring-blue-500"
                    />
                    <Server className="w-4 h-4 text-blue-500" />
                    <span className="text-sm text-gray-700 dark:text-gray-300">
                      Single Carrier
                    </span>
                  </label>
                  <label className="flex items-center gap-2">
                    <input
                      type="radio"
                      name="destType"
                      checked={destinationType === 'group'}
                      onChange={() => setDestinationType('group')}
                      className="text-blue-600 focus:ring-blue-500"
                    />
                    <Layers className="w-4 h-4 text-orange-500" />
                    <span className="text-sm text-gray-700 dark:text-gray-300">
                      Carrier Group
                    </span>
                  </label>
                </div>
              </div>

              {destinationType === 'carrier' ? (
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Carrier *
                  </label>
                  <select
                    value={formData.carrier_id || ''}
                    onChange={(e) =>
                      setFormData({ ...formData, carrier_id: e.target.value })
                    }
                    required
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  >
                    <option value="">Select a carrier...</option>
                    {carriers
                      .filter((c) => c.enabled)
                      .map((carrier) => (
                        <option key={carrier.id} value={carrier.id}>
                          {carrier.name} ({carrier.host})
                        </option>
                      ))}
                  </select>
                </div>
              ) : (
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Carrier Group *
                  </label>
                  <select
                    value={formData.carrier_group_id || ''}
                    onChange={(e) =>
                      setFormData({
                        ...formData,
                        carrier_group_id: e.target.value,
                      })
                    }
                    required
                    className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
                  >
                    <option value="">Select a group...</option>
                    {groups.map((group) => (
                      <option key={group.id} value={group.id}>
                        {group.name} ({group.members.length} carriers)
                      </option>
                    ))}
                  </select>
                </div>
              )}

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
                    : editingRoute
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
        open={!!deleteRoute}
        onOpenChange={() => setDeleteRoute(null)}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/50" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-sm p-6">
            <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              Delete Route
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              Are you sure you want to delete "{deleteRoute?.name}"? This action
              cannot be undone.
            </AlertDialog.Description>
            {deleteMutation.error && (
              <p className="mt-2 text-sm text-red-600 dark:text-red-400">
                {apiErrorMessage(deleteMutation.error, 'Could not delete the route')}
              </p>
            )}
            <div className="flex justify-end gap-2 mt-4">
              <AlertDialog.Cancel asChild>
                <button className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 dark:bg-gray-700 dark:text-gray-200 dark:border-gray-600 dark:hover:bg-gray-600">
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  onClick={() =>
                    deleteRoute && deleteMutation.mutate(deleteRoute.id)
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

export default OutboundRouting
