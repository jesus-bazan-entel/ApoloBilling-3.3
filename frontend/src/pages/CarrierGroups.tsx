import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchCarrierGroups,
  createCarrierGroup,
  updateCarrierGroup,
  deleteCarrierGroup,
  addCarrierGroupMember,
  updateCarrierGroupMember,
  removeCarrierGroupMember,
  fetchSipCarriers,
} from '../api/client'
import type {
  CarrierGroupWithMembers,
  CreateCarrierGroupRequest,
  UpdateCarrierGroupRequestNew,
  AddCarrierGroupMemberRequest,
  CarrierGroupMember,
  SipCarrier,
} from '../types'

import * as Dialog from '@radix-ui/react-dialog'
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import {
  Plus,
  Pencil,
  Trash2,
  Layers,
  Check,
  X,
  AlertTriangle,
  Users,
  ChevronDown,
  ChevronRight,
  Server,
} from 'lucide-react'

const CarrierGroups = () => {
  const queryClient = useQueryClient()
  const [isModalOpen, setIsModalOpen] = useState(false)
  const [editingGroup, setEditingGroup] = useState<CarrierGroupWithMembers | null>(null)
  const [deleteGroup, setDeleteGroup] = useState<CarrierGroupWithMembers | null>(null)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set())
  const [addMemberGroupId, setAddMemberGroupId] = useState<string | null>(null)
  const [deleteMember, setDeleteMember] = useState<{
    groupId: string
    member: CarrierGroupMember
  } | null>(null)

  const [formData, setFormData] = useState<CreateCarrierGroupRequest>({
    name: '',
    description: '',
    strategy: 'sequential',
  })

  const [memberFormData, setMemberFormData] = useState<AddCarrierGroupMemberRequest>({
    carrier_id: '',
    priority: 1,
    weight: 100,
  })

  const { data, isLoading, error } = useQuery({
    queryKey: ['carrier-groups'],
    queryFn: fetchCarrierGroups,
  })

  const { data: carriersData } = useQuery({
    queryKey: ['sip-carriers'],
    queryFn: fetchSipCarriers,
  })

  const createMutation = useMutation({
    mutationFn: createCarrierGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
      closeModal()
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateCarrierGroupRequestNew }) =>
      updateCarrierGroup(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
      closeModal()
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteCarrierGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
      setDeleteGroup(null)
    },
  })

  const addMemberMutation = useMutation({
    mutationFn: ({ groupId, member }: { groupId: string; member: AddCarrierGroupMemberRequest }) =>
      addCarrierGroupMember(groupId, member),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
      setAddMemberGroupId(null)
      setMemberFormData({ carrier_id: '', priority: 1, weight: 100 })
    },
  })

  const updateMemberMutation = useMutation({
    mutationFn: ({
      groupId,
      memberId,
      data,
    }: {
      groupId: string
      memberId: string
      data: { priority?: number; weight?: number }
    }) => updateCarrierGroupMember(groupId, memberId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
    },
  })

  const removeMemberMutation = useMutation({
    mutationFn: ({ groupId, memberId }: { groupId: string; memberId: string }) =>
      removeCarrierGroupMember(groupId, memberId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['carrier-groups'] })
      setDeleteMember(null)
    },
  })

  const closeModal = () => {
    setIsModalOpen(false)
    setEditingGroup(null)
    setFormData({ name: '', description: '', strategy: 'sequential' })
  }

  const openCreateModal = () => {
    setEditingGroup(null)
    setFormData({ name: '', description: '', strategy: 'sequential' })
    setIsModalOpen(true)
  }

  const openEditModal = (group: CarrierGroupWithMembers) => {
    setEditingGroup(group)
    setFormData({
      name: group.name,
      description: group.description || '',
      strategy: group.strategy,
    })
    setIsModalOpen(true)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (editingGroup) {
      updateMutation.mutate({ id: editingGroup.id, data: formData })
    } else {
      createMutation.mutate(formData)
    }
  }

  const handleAddMember = (e: React.FormEvent) => {
    e.preventDefault()
    if (addMemberGroupId && memberFormData.carrier_id) {
      addMemberMutation.mutate({ groupId: addMemberGroupId, member: memberFormData })
    }
  }

  const toggleExpand = (groupId: string) => {
    const newExpanded = new Set(expandedGroups)
    if (newExpanded.has(groupId)) {
      newExpanded.delete(groupId)
    } else {
      newExpanded.add(groupId)
    }
    setExpandedGroups(newExpanded)
  }

  const getAvailableCarriers = (group: CarrierGroupWithMembers): SipCarrier[] => {
    const memberCarrierIds = new Set(group.members.map((m) => m.carrier_id))
    return (carriersData?.carriers || []).filter((c) => !memberCarrierIds.has(c.id))
  }

  const getSyncStatusBadge = (status: string) => {
    switch (status) {
      case 'synced':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-green-500/20 text-green-400">
            <Check className="w-3 h-3" /> Synced
          </span>
        )
      case 'pending':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-yellow-500/20 text-yellow-400">
            <AlertTriangle className="w-3 h-3" /> Pending
          </span>
        )
      case 'error':
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-red-500/20 text-red-400">
            <X className="w-3 h-3" /> Error
          </span>
        )
      default:
        return null
    }
  }

  const getStrategyLabel = (strategy: string) => {
    switch (strategy) {
      case 'sequential':
        return 'Secuencial'
      case 'round_robin':
        return 'Round Robin'
      case 'weighted':
        return 'Por Peso'
      default:
        return strategy
    }
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="w-8 h-8 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-4 text-red-400">
        Error loading carrier groups: {(error as Error).message}
      </div>
    )
  }

  const groups = data?.groups || []

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-3">
            <Layers className="w-7 h-7 text-cyan-400" />
            Carrier Groups
          </h1>
          <p className="text-slate-400 mt-1">
            Grupos de carriers para failover y balanceo de carga
          </p>
        </div>
        <button
          onClick={openCreateModal}
          className="flex items-center gap-2 px-4 py-2 bg-cyan-600 hover:bg-cyan-700 text-white rounded-lg transition-colors"
        >
          <Plus className="w-4 h-4" />
          Add Group
        </button>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-slate-800/50 border border-slate-700/50 rounded-lg p-4">
          <div className="text-2xl font-bold text-white">{data?.total || 0}</div>
          <div className="text-sm text-slate-400">Total Groups</div>
        </div>
        <div className="bg-slate-800/50 border border-slate-700/50 rounded-lg p-4">
          <div className="text-2xl font-bold text-white">
            {groups.reduce((sum, g) => sum + g.members.length, 0)}
          </div>
          <div className="text-sm text-slate-400">Total Members</div>
        </div>
        <div className="bg-slate-800/50 border border-slate-700/50 rounded-lg p-4">
          <div className="text-2xl font-bold text-white">
            {groups.filter((g) => g.sync_status === 'synced').length}
          </div>
          <div className="text-sm text-slate-400">Synced to Kamailio</div>
        </div>
      </div>

      {/* Groups List */}
      <div className="bg-slate-800/50 border border-slate-700/50 rounded-lg overflow-hidden">
        {groups.length === 0 ? (
          <div className="p-8 text-center text-slate-400">
            No carrier groups configured. Click "Add Group" to create one.
          </div>
        ) : (
          <div className="divide-y divide-slate-700/50">
            {groups.map((group) => (
              <div key={group.id} className="bg-slate-800/30">
                {/* Group Header */}
                <div
                  className="flex items-center justify-between p-4 cursor-pointer hover:bg-slate-700/30 transition-colors"
                  onClick={() => toggleExpand(group.id)}
                >
                  <div className="flex items-center gap-3">
                    {expandedGroups.has(group.id) ? (
                      <ChevronDown className="w-5 h-5 text-slate-400" />
                    ) : (
                      <ChevronRight className="w-5 h-5 text-slate-400" />
                    )}
                    <Layers className="w-5 h-5 text-cyan-400" />
                    <div>
                      <div className="font-medium text-white">{group.name}</div>
                      {group.description && (
                        <div className="text-sm text-slate-400">{group.description}</div>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-4">
                    <span className="text-sm text-slate-400">
                      {getStrategyLabel(group.strategy)}
                    </span>
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-slate-700 text-slate-300">
                      <Users className="w-3 h-3" /> {group.members.length} members
                    </span>
                    {getSyncStatusBadge(group.sync_status)}
                    {group.kamailio_group_id && (
                      <span className="text-xs text-slate-500">
                        Kam ID: {group.kamailio_group_id}
                      </span>
                    )}
                    <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                      <button
                        onClick={() => openEditModal(group)}
                        className="p-1.5 text-slate-400 hover:text-cyan-400 hover:bg-slate-700 rounded transition-colors"
                        title="Edit group"
                      >
                        <Pencil className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => setDeleteGroup(group)}
                        className="p-1.5 text-slate-400 hover:text-red-400 hover:bg-slate-700 rounded transition-colors"
                        title="Delete group"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>

                {/* Group Members (expanded) */}
                {expandedGroups.has(group.id) && (
                  <div className="px-4 pb-4 pl-12 space-y-2">
                    {group.members.length === 0 ? (
                      <div className="text-sm text-slate-500 italic py-2">
                        No carriers in this group
                      </div>
                    ) : (
                      <div className="bg-slate-900/50 rounded-lg overflow-hidden">
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="text-left text-slate-400 border-b border-slate-700/50">
                              <th className="px-3 py-2">Carrier</th>
                              <th className="px-3 py-2">Priority</th>
                              <th className="px-3 py-2">Weight</th>
                              <th className="px-3 py-2 w-20">Actions</th>
                            </tr>
                          </thead>
                          <tbody className="divide-y divide-slate-700/30">
                            {group.members
                              .sort((a, b) => a.priority - b.priority)
                              .map((member) => (
                                <tr key={member.id} className="text-slate-300">
                                  <td className="px-3 py-2">
                                    <div className="flex items-center gap-2">
                                      <Server className="w-4 h-4 text-slate-500" />
                                      {member.carrier_name}
                                    </div>
                                  </td>
                                  <td className="px-3 py-2">
                                    <input
                                      type="number"
                                      min="1"
                                      max="100"
                                      value={member.priority}
                                      onChange={(e) =>
                                        updateMemberMutation.mutate({
                                          groupId: group.id,
                                          memberId: member.id,
                                          data: { priority: e.target.value === '' ? 0 : parseInt(e.target.value) },
                                        })
                                      }
                                      className="w-16 px-2 py-1 bg-slate-800 border border-slate-600 rounded text-white text-center"
                                    />
                                  </td>
                                  <td className="px-3 py-2">
                                    <input
                                      type="number"
                                      min="1"
                                      max="100"
                                      value={member.weight}
                                      onChange={(e) =>
                                        updateMemberMutation.mutate({
                                          groupId: group.id,
                                          memberId: member.id,
                                          data: { weight: e.target.value === '' ? 0 : parseInt(e.target.value) },
                                        })
                                      }
                                      className="w-16 px-2 py-1 bg-slate-800 border border-slate-600 rounded text-white text-center"
                                    />
                                  </td>
                                  <td className="px-3 py-2">
                                    <button
                                      onClick={() =>
                                        setDeleteMember({ groupId: group.id, member })
                                      }
                                      className="p-1 text-slate-400 hover:text-red-400 hover:bg-slate-700 rounded transition-colors"
                                      title="Remove from group"
                                    >
                                      <Trash2 className="w-4 h-4" />
                                    </button>
                                  </td>
                                </tr>
                              ))}
                          </tbody>
                        </table>
                      </div>
                    )}

                    {/* Add Member Button/Form */}
                    {addMemberGroupId === group.id ? (
                      <form
                        onSubmit={handleAddMember}
                        className="flex items-center gap-2 p-3 bg-slate-900/50 rounded-lg"
                      >
                        <select
                          value={memberFormData.carrier_id}
                          onChange={(e) =>
                            setMemberFormData({ ...memberFormData, carrier_id: e.target.value })
                          }
                          className="flex-1 px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white"
                          required
                        >
                          <option value="">Select carrier...</option>
                          {getAvailableCarriers(group).map((carrier) => (
                            <option key={carrier.id} value={carrier.id}>
                              {carrier.name} ({carrier.host})
                            </option>
                          ))}
                        </select>
                        <input
                          type="number"
                          placeholder="Priority"
                          min="1"
                          max="100"
                          value={memberFormData.priority}
                          onChange={(e) =>
                            setMemberFormData({
                              ...memberFormData,
                              priority: e.target.value === '' ? 0 : parseInt(e.target.value),
                            })
                          }
                          className="w-20 px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white"
                        />
                        <input
                          type="number"
                          placeholder="Weight"
                          min="1"
                          max="100"
                          value={memberFormData.weight}
                          onChange={(e) =>
                            setMemberFormData({
                              ...memberFormData,
                              weight: e.target.value === '' ? 0 : parseInt(e.target.value),
                            })
                          }
                          className="w-20 px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white"
                        />
                        <button
                          type="submit"
                          disabled={addMemberMutation.isPending}
                          className="px-3 py-2 bg-cyan-600 hover:bg-cyan-700 text-white rounded-lg transition-colors disabled:opacity-50"
                        >
                          {addMemberMutation.isPending ? '...' : 'Add'}
                        </button>
                        <button
                          type="button"
                          onClick={() => setAddMemberGroupId(null)}
                          className="px-3 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
                        >
                          Cancel
                        </button>
                      </form>
                    ) : (
                      <button
                        onClick={() => {
                          setAddMemberGroupId(group.id)
                          setMemberFormData({ carrier_id: '', priority: 1, weight: 100 })
                        }}
                        disabled={getAvailableCarriers(group).length === 0}
                        className="flex items-center gap-2 px-3 py-2 text-sm text-cyan-400 hover:bg-slate-700/50 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        <Plus className="w-4 h-4" />
                        Add Carrier to Group
                      </button>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Create/Edit Modal */}
      <Dialog.Root open={isModalOpen} onOpenChange={setIsModalOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <Dialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-slate-800 border border-slate-700 rounded-xl p-6 w-full max-w-md shadow-xl">
            <Dialog.Title className="text-lg font-semibold text-white mb-4">
              {editingGroup ? 'Edit Carrier Group' : 'Create Carrier Group'}
            </Dialog.Title>

            <form onSubmit={handleSubmit} className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1">Name *</label>
                <input
                  type="text"
                  value={formData.name}
                  onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                  className="w-full px-3 py-2 bg-slate-900 border border-slate-600 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
                  placeholder="e.g., Primary Carriers"
                  required
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1">
                  Description
                </label>
                <input
                  type="text"
                  value={formData.description}
                  onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                  className="w-full px-3 py-2 bg-slate-900 border border-slate-600 rounded-lg text-white placeholder-slate-500 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
                  placeholder="Optional description"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1">
                  Failover Strategy
                </label>
                <select
                  value={formData.strategy}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      strategy: e.target.value as 'sequential' | 'round_robin' | 'weighted',
                    })
                  }
                  className="w-full px-3 py-2 bg-slate-900 border border-slate-600 rounded-lg text-white focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500"
                >
                  <option value="sequential">Sequential (by priority)</option>
                  <option value="round_robin">Round Robin</option>
                  <option value="weighted">Weighted</option>
                </select>
                <p className="text-xs text-slate-500 mt-1">
                  {formData.strategy === 'sequential' &&
                    'Try carriers in order of priority (lowest first)'}
                  {formData.strategy === 'round_robin' &&
                    'Distribute calls evenly across all carriers'}
                  {formData.strategy === 'weighted' && 'Distribute based on carrier weight'}
                </p>
              </div>

              <div className="flex justify-end gap-3 pt-4">
                <Dialog.Close asChild>
                  <button
                    type="button"
                    className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
                  >
                    Cancel
                  </button>
                </Dialog.Close>
                <button
                  type="submit"
                  disabled={createMutation.isPending || updateMutation.isPending}
                  className="px-4 py-2 bg-cyan-600 hover:bg-cyan-700 text-white rounded-lg transition-colors disabled:opacity-50"
                >
                  {createMutation.isPending || updateMutation.isPending
                    ? 'Saving...'
                    : editingGroup
                      ? 'Update'
                      : 'Create'}
                </button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Delete Group Confirmation */}
      <AlertDialog.Root open={!!deleteGroup} onOpenChange={() => setDeleteGroup(null)}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-slate-800 border border-slate-700 rounded-xl p-6 w-full max-w-md shadow-xl">
            <AlertDialog.Title className="text-lg font-semibold text-white">
              Delete Carrier Group
            </AlertDialog.Title>
            <AlertDialog.Description className="text-slate-400 mt-2">
              Are you sure you want to delete "{deleteGroup?.name}"? This will also remove all
              member associations. This action cannot be undone.
            </AlertDialog.Description>
            <div className="flex justify-end gap-3 mt-6">
              <AlertDialog.Cancel asChild>
                <button className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors">
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  onClick={() => deleteGroup && deleteMutation.mutate(deleteGroup.id)}
                  disabled={deleteMutation.isPending}
                  className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg transition-colors disabled:opacity-50"
                >
                  {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>

      {/* Delete Member Confirmation */}
      <AlertDialog.Root open={!!deleteMember} onOpenChange={() => setDeleteMember(null)}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-slate-800 border border-slate-700 rounded-xl p-6 w-full max-w-md shadow-xl">
            <AlertDialog.Title className="text-lg font-semibold text-white">
              Remove Carrier from Group
            </AlertDialog.Title>
            <AlertDialog.Description className="text-slate-400 mt-2">
              Are you sure you want to remove "{deleteMember?.member.carrier_name}" from this
              group?
            </AlertDialog.Description>
            <div className="flex justify-end gap-3 mt-6">
              <AlertDialog.Cancel asChild>
                <button className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors">
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  onClick={() =>
                    deleteMember &&
                    removeMemberMutation.mutate({
                      groupId: deleteMember.groupId,
                      memberId: deleteMember.member.id,
                    })
                  }
                  disabled={removeMemberMutation.isPending}
                  className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg transition-colors disabled:opacity-50"
                >
                  {removeMemberMutation.isPending ? 'Removing...' : 'Remove'}
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </div>
  )
}

export default CarrierGroups
