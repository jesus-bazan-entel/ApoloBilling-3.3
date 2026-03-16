import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchZones, createZone, updateZone, deleteZone } from '../api/client'
import DataTable from '../components/DataTable'
import { Globe, Plus, X, Pencil, Trash2 } from 'lucide-react'
import type { Zone } from '../types'

export default function ZonesPage() {
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [editingZone, setEditingZone] = useState<Zone | null>(null)
  const [deletingZone, setDeletingZone] = useState<Zone | null>(null)
  const [error, setError] = useState<string | null>(null)
  const queryClient = useQueryClient()

  const { data: zones = [], isLoading } = useQuery({
    queryKey: ['zones'],
    queryFn: fetchZones,
    refetchInterval: 30000,
  })

  const createMutation = useMutation({
    mutationFn: createZone,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['zones'] })
      setShowCreateModal(false)
      setError(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al crear la zona'
      setError(message)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<Zone> }) => updateZone(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['zones'] })
      setEditingZone(null)
      setError(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al actualizar la zona'
      setError(message)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteZone,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['zones'] })
      setDeletingZone(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al eliminar la zona'
      setError(message)
    },
  })

  const columns = [
    {
      key: 'id',
      header: 'ID',
      render: (zone: Zone) => (
        <span className="font-mono text-slate-600">{zone.id}</span>
      ),
    },
    {
      key: 'zone_name',
      header: 'Nombre de Zona',
      render: (zone: Zone) => (
        <span className="font-medium text-slate-900">{zone.zone_name}</span>
      ),
    },
    {
      key: 'zone_code',
      header: 'Código',
      render: (zone: Zone) => (
        <span className="font-mono text-slate-600">{zone.zone_code || '-'}</span>
      ),
    },
    {
      key: 'description',
      header: 'Descripción',
      render: (zone: Zone) => (
        <span className="text-slate-600">{zone.description || '-'}</span>
      ),
    },
    {
      key: 'zone_type',
      header: 'Tipo',
      render: (zone: Zone) => (
        <span className="px-2 py-1 text-xs rounded-full bg-blue-100 text-blue-800">
          {zone.zone_type || 'GEOGRAPHIC'}
        </span>
      ),
    },
    {
      key: 'enabled',
      header: 'Estado',
      render: (zone: Zone) => (
        <span className={`px-2 py-1 text-xs rounded-full ${
          zone.enabled !== false ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
        }`}>
          {zone.enabled !== false ? 'Activa' : 'Inactiva'}
        </span>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (zone: Zone) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setEditingZone(zone)}
            className="p-1.5 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
            title="Editar"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <button
            onClick={() => setDeletingZone(zone)}
            className="p-1.5 text-red-600 hover:bg-red-50 rounded-lg transition-colors"
            title="Eliminar"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">
            Gestión de Zonas Geográficas
          </h1>
          <p className="text-slate-500">
            Define zonas para organizar tarifas por destino
          </p>
        </div>
        <button
          onClick={() => setShowCreateModal(true)}
          className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <Plus className="w-5 h-5 mr-2" />
          Nueva Zona
        </button>
      </div>

      {/* Summary Card */}
      <div className="bg-white rounded-lg shadow-sm p-6 border border-slate-200">
        <div className="flex items-center">
          <Globe className="w-8 h-8 text-blue-500 mr-4" />
          <div>
            <p className="text-sm text-slate-500">Total de Zonas</p>
            <p className="text-3xl font-bold text-slate-900">{zones.length}</p>
          </div>
        </div>
      </div>

      {/* Zones Table */}
      <DataTable
        columns={columns}
        data={zones}
        loading={isLoading}
        emptyMessage="No hay zonas geográficas registradas"
        searchable={true}
        searchPlaceholder="Buscar por nombre o descripción..."
      />

      {/* Create Modal */}
      {showCreateModal && (
        <ZoneFormModal
          title="Nueva Zona Geográfica"
          onClose={() => {
            setShowCreateModal(false)
            setError(null)
          }}
          onSubmit={(data) => createMutation.mutate(data)}
          isLoading={createMutation.isPending}
          error={error}
        />
      )}

      {/* Edit Modal */}
      {editingZone && (
        <ZoneFormModal
          title="Editar Zona"
          zone={editingZone}
          onClose={() => {
            setEditingZone(null)
            setError(null)
          }}
          onSubmit={(data) => updateMutation.mutate({ id: editingZone.id, data })}
          isLoading={updateMutation.isPending}
          error={error}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deletingZone && (
        <DeleteConfirmModal
          title="Eliminar Zona"
          message={`¿Estás seguro de que deseas eliminar la zona "${deletingZone.zone_name}"? Esta acción no se puede deshacer.`}
          onClose={() => {
            setDeletingZone(null)
            setError(null)
          }}
          onConfirm={() => deleteMutation.mutate(deletingZone.id)}
          isLoading={deleteMutation.isPending}
          error={error}
        />
      )}
    </div>
  )
}

interface ZoneFormModalProps {
  title: string
  zone?: Zone
  onClose: () => void
  onSubmit: (data: Partial<Zone>) => void
  isLoading: boolean
  error: string | null
}

function ZoneFormModal({ title, zone, onClose, onSubmit, isLoading, error }: ZoneFormModalProps) {
  const [formData, setFormData] = useState<Partial<Zone>>({
    zone_name: zone?.zone_name || '',
    zone_code: zone?.zone_code || '',
    description: zone?.description || '',
    zone_type: zone?.zone_type || 'GEOGRAPHIC',
    region_name: zone?.region_name || '',
    enabled: zone?.enabled !== false,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl max-w-lg w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-slate-200">
          <h2 className="text-xl font-bold text-slate-900">{title}</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600"
          >
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
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Nombre de Zona *
            </label>
            <input
              type="text"
              required
              value={formData.zone_name}
              onChange={(e) =>
                setFormData({ ...formData, zone_name: e.target.value })
              }
              placeholder="Ej: Peru_Lima, USA_NewYork"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Código de Zona
            </label>
            <input
              type="text"
              value={formData.zone_code || ''}
              onChange={(e) =>
                setFormData({ ...formData, zone_code: e.target.value })
              }
              placeholder="Ej: PE-LIM, US-NYC"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Descripción
            </label>
            <textarea
              rows={2}
              value={formData.description || ''}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
              placeholder="Descripción de la zona"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Tipo de Zona
              </label>
              <select
                value={formData.zone_type || 'GEOGRAPHIC'}
                onChange={(e) =>
                  setFormData({ ...formData, zone_type: e.target.value })
                }
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="GEOGRAPHIC">Geográfica</option>
                <option value="MOBILE">Móvil</option>
                <option value="SPECIAL">Especial</option>
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-700 mb-1">
                Región
              </label>
              <input
                type="text"
                value={formData.region_name || ''}
                onChange={(e) =>
                  setFormData({ ...formData, region_name: e.target.value })
                }
                placeholder="Ej: Sudamérica"
                className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
          </div>

          <div className="flex items-center">
            <input
              type="checkbox"
              id="enabled"
              checked={formData.enabled !== false}
              onChange={(e) =>
                setFormData({ ...formData, enabled: e.target.checked })
              }
              className="w-4 h-4 text-blue-600 border-slate-300 rounded focus:ring-blue-500"
            />
            <label htmlFor="enabled" className="ml-2 text-sm text-slate-700">
              Zona activa
            </label>
          </div>

          <div className="flex justify-end space-x-3 pt-4 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={isLoading}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : 'Guardar'}
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
  error: string | null
}

function DeleteConfirmModal({ title, message, onClose, onConfirm, isLoading, error }: DeleteConfirmModalProps) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-slate-200">
          <h2 className="text-xl font-bold text-slate-900">{title}</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="p-6">
          {error && (
            <div className="p-3 mb-4 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
              {error}
            </div>
          )}

          <p className="text-slate-600">{message}</p>

          <div className="flex justify-end space-x-3 mt-6">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
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
