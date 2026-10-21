import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchPbxGroups,
  createPbxGroup,
  updatePbxGroup,
  deletePbxGroup,
  addPbxGroupMember,
  updatePbxGroupMember,
  removePbxGroupMember,
  fetchSipEndpoints,
} from '../api/client'
import type {
  PbxGroupWithMembers,
  CreatePbxGroupRequest,
  SipEndpoint,
} from '../types'
import {
  Plus,
  X,
  Trash2,
  Edit,
  Server,
  AlertCircle,
  ChevronDown,
  ChevronRight,
  Users,
  ArrowRight,
} from 'lucide-react'

export default function PBXGroupsPage() {
  const queryClient = useQueryClient()
  const [showModal, setShowModal] = useState(false)
  const [editing, setEditing] = useState<PbxGroupWithMembers | null>(null)
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [addTo, setAddTo] = useState<string | null>(null)
  const [memberEndpoint, setMemberEndpoint] = useState('')
  const [memberPriority, setMemberPriority] = useState(100)
  const [confirmDelete, setConfirmDelete] = useState<PbxGroupWithMembers | null>(null)
  const [error, setError] = useState<string | null>(null)

  const { data: groups = [], isLoading } = useQuery({
    queryKey: ['pbx-groups'],
    queryFn: fetchPbxGroups,
  })
  const { data: endpoints = [] } = useQuery({
    queryKey: ['sip-endpoints'],
    queryFn: () => fetchSipEndpoints(),
  })

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: ['pbx-groups'] })
    queryClient.invalidateQueries({ queryKey: ['inbound-routes'] })
  }
  const onError = (e: unknown) => {
    const ax = e as { response?: { data?: { message?: string } }; message?: string }
    setError(ax.response?.data?.message || ax.message || 'Error en la operación')
  }

  const createMutation = useMutation({
    mutationFn: createPbxGroup,
    onSuccess: () => { invalidate(); setShowModal(false); setEditing(null); setError(null) },
    onError,
  })
  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreatePbxGroupRequest }) => updatePbxGroup(id, data),
    onSuccess: () => { invalidate(); setShowModal(false); setEditing(null); setError(null) },
    onError,
  })
  const deleteMutation = useMutation({
    mutationFn: deletePbxGroup,
    onSuccess: () => { invalidate(); setConfirmDelete(null); setError(null) },
    onError,
  })
  const addMemberMutation = useMutation({
    mutationFn: ({ groupId, endpointId, priority }: { groupId: string; endpointId: string; priority: number }) =>
      addPbxGroupMember(groupId, { sip_endpoint_id: endpointId, priority }),
    onSuccess: () => { invalidate(); setAddTo(null); setMemberEndpoint(''); setError(null) },
    onError,
  })
  const updateMemberMutation = useMutation({
    mutationFn: ({ groupId, memberId, priority }: { groupId: string; memberId: string; priority: number }) =>
      updatePbxGroupMember(groupId, memberId, priority),
    onSuccess: () => { invalidate(); setError(null) },
    onError,
  })
  const removeMemberMutation = useMutation({
    mutationFn: ({ groupId, memberId }: { groupId: string; memberId: string }) =>
      removePbxGroupMember(groupId, memberId),
    onSuccess: () => { invalidate(); setError(null) },
    onError,
  })

  const trunks = (endpoints as SipEndpoint[]).filter(
    (e) => e.endpoint_type === 'trunk' && e.auth_mode === 'ip' && e.enabled
  )

  const toggle = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">Grupos PBX</h1>
          <p className="text-gray-500 dark:text-gray-400">
            Failover del mismo dueño: cada rango se enruta a los miembros en orden de prioridad
          </p>
        </div>
        <button
          onClick={() => { setEditing(null); setShowModal(true); setError(null) }}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
        >
          <Plus className="w-4 h-4" />
          Nuevo Grupo
        </button>
      </div>

      <div className="bg-violet-50 dark:bg-violet-900/20 border border-violet-200 dark:border-violet-800 rounded-lg p-4 text-sm text-violet-800 dark:text-violet-200">
        Un grupo = <strong>un solo dueño</strong> (una PBX pertenece máximo a un grupo). Los rangos
        DID (<code className="bg-violet-100 dark:bg-violet-800 px-1 rounded">did_ranges</code> de cada
        troncal) de los miembros se comparten: el L3 selectivo puentea cada prefijo a los miembros
        en orden de prioridad. Rangos que choquen con PBXs <strong>fuera</strong> del grupo se rechazan
        indicando el dueño actual.
      </div>

      {error && (
        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4 flex items-start gap-2">
          <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0" />
          <span className="text-sm text-red-800 dark:text-red-200">{error}</span>
          <button onClick={() => setError(null)} className="ml-auto text-red-500 hover:text-red-700">
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      {isLoading ? (
        <p className="text-gray-500">Cargando grupos...</p>
      ) : groups.length === 0 ? (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-8 text-center text-gray-500">
          Sin grupos PBX. Crea uno para dar failover primario/secundario a un cliente.
        </div>
      ) : (
        <div className="space-y-4">
          {groups.map((g: PbxGroupWithMembers) => {
            const sorted = [...g.members].sort((a, b) => a.priority - b.priority)
            return (
              <div key={g.id} className="bg-white dark:bg-gray-800 rounded-lg shadow">
                <div className="p-4 flex items-center gap-3">
                  <button onClick={() => toggle(g.id)} className="text-gray-500 hover:text-gray-700">
                    {expanded.has(g.id) ? <ChevronDown className="w-5 h-5" /> : <ChevronRight className="w-5 h-5" />}
                  </button>
                  <Users className="w-5 h-5 text-violet-500" />
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-gray-900 dark:text-gray-100">{g.name}</div>
                    {g.description && <div className="text-sm text-gray-500">{g.description}</div>}
                  </div>
                  <span className="text-xs text-gray-400">
                    {g.members.length} miembro{g.members.length === 1 ? '' : 's'} · {g.strategy}
                  </span>
                  <span className={`px-2 py-1 text-xs rounded-full ${g.enabled ? 'bg-green-100 text-green-800' : 'bg-gray-100 text-gray-600'}`}>
                    {g.enabled ? 'Activo' : 'Inactivo'}
                  </span>
                  <button
                    onClick={() => { setEditing(g); setShowModal(true); setError(null) }}
                    className="p-1 text-blue-600 hover:bg-blue-50 rounded"
                  >
                    <Edit className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => setConfirmDelete(g)}
                    className="p-1 text-red-600 hover:bg-red-50 rounded"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>

                {expanded.has(g.id) && (
                  <div className="px-4 pb-4 space-y-2 border-t border-gray-100 dark:border-gray-700 pt-3">
                    {sorted.map((m, i) => (
                      <div key={m.id} className="flex items-center gap-3 text-sm border border-gray-100 dark:border-gray-700 rounded-lg px-3 py-2">
                        <span className="text-xs font-mono bg-gray-100 dark:bg-gray-700 px-2 py-0.5 rounded">
                          P{i + 1} · prio {m.priority}
                        </span>
                        <Server className="w-4 h-4 text-blue-500" />
                        <span className="font-medium flex-1">{m.endpoint_name}</span>
                        <span className="text-xs text-gray-400 font-mono">{m.host}:{m.port}</span>
                        <input
                          type="number"
                          defaultValue={m.priority}
                          min={1}
                          max={10000}
                          className="w-20 px-2 py-1 border rounded text-sm"
                          onBlur={(e) => {
                            const v = Number(e.target.value)
                            if (v !== m.priority) updateMemberMutation.mutate({ groupId: g.id, memberId: m.id, priority: v })
                          }}
                        />
                        <button
                          onClick={() => removeMemberMutation.mutate({ groupId: g.id, memberId: m.id })}
                          className="p-1 text-red-600 hover:bg-red-50 rounded"
                          title="Quitar del grupo"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                    {addTo === g.id ? (
                      <div className="flex items-center gap-2 pt-1">
                        <select
                          value={memberEndpoint}
                          onChange={(e) => setMemberEndpoint(e.target.value)}
                          className="flex-1 px-3 py-2 border rounded-lg text-sm"
                        >
                          <option value="">Seleccionar troncal IP...</option>
                          {trunks.map((t) => (
                            <option key={t.id} value={t.id}>
                              {t.name} ({t.host}:{t.port}){t.did_ranges ? ` [${t.did_ranges}]` : ''}
                            </option>
                          ))}
                        </select>
                        <input
                          type="number"
                          value={memberPriority}
                          min={1}
                          onChange={(e) => setMemberPriority(Number(e.target.value))}
                          className="w-24 px-2 py-2 border rounded-lg text-sm"
                          title="Prioridad (menor = primero)"
                        />
                        <button
                          onClick={() => memberEndpoint && addMemberMutation.mutate({ groupId: g.id, endpointId: memberEndpoint, priority: memberPriority })}
                          className="px-3 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700"
                        >
                          Agregar
                        </button>
                        <button onClick={() => setAddTo(null)} className="p-2 text-gray-500">
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ) : (
                      <button
                        onClick={() => { setAddTo(g.id); setMemberEndpoint(''); setError(null) }}
                        className="flex items-center gap-1 text-sm text-blue-600 hover:text-blue-800"
                      >
                        <Plus className="w-4 h-4" /> Agregar PBX al grupo
                      </button>
                    )}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}

      {showModal && (
        <GroupModal
          group={editing}
          onClose={() => { setShowModal(false); setEditing(null) }}
          onSave={(data) => {
            if (editing) updateMutation.mutate({ id: editing.id, data })
            else createMutation.mutate(data)
          }}
        />
      )}

      {confirmDelete && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-sm w-full mx-4">
            <h3 className="font-semibold mb-2">Eliminar grupo “{confirmDelete.name}”</h3>
            <p className="text-sm text-gray-500 mb-4">
              Los miembros conservan sus rangos; los prefijos volverán a enrutar solo a su PBX dueña.
            </p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setConfirmDelete(null)} className="px-4 py-2 border rounded-lg text-sm">
                Cancelar
              </button>
              <button
                onClick={() => deleteMutation.mutate(confirmDelete.id)}
                className="px-4 py-2 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700 flex items-center gap-1"
              >
                Eliminar <ArrowRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function GroupModal({
  group,
  onClose,
  onSave,
}: {
  group: PbxGroupWithMembers | null
  onClose: () => void
  onSave: (data: CreatePbxGroupRequest) => void
}) {
  const [name, setName] = useState(group?.name || '')
  const [description, setDescription] = useState(group?.description || '')
  const [strategy, setStrategy] = useState(group?.strategy || 'sequential')
  const [enabled, setEnabled] = useState(group?.enabled ?? true)

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-semibold mb-4">{group ? 'Editar Grupo' : 'Nuevo Grupo PBX'}</h3>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1">Nombre *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Ej: Fibertel Primario/Secundario"
              className="w-full px-3 py-2 border rounded-lg"
              required
            />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1">Descripción</label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Cliente / sitio"
              className="w-full px-3 py-2 border rounded-lg"
            />
          </div>
          <div>
            <label className="block text-sm font-medium mb-1">Estrategia</label>
            <select
              value={strategy}
              onChange={(e) => setStrategy(e.target.value)}
              className="w-full px-3 py-2 border rounded-lg"
            >
              <option value="sequential">Secuencial (prioridad)</option>
              <option value="priority">Prioridad (primario/secundario)</option>
            </select>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
            Activo
          </label>
          <div className="flex justify-end gap-2">
            <button onClick={onClose} className="px-4 py-2 border rounded-lg text-sm">Cancelar</button>
            <button
              onClick={() => name.trim() && onSave({ name: name.trim(), description: description || undefined, strategy, enabled })}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm hover:bg-blue-700"
            >
              Guardar
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
