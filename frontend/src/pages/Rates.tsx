import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchRateCards,
  createRateCard,
  updateRateCard,
  deleteRateCard,
  lookupRate,
  fetchZones,
} from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import { Settings, Plus, Edit, Trash2, Search, X } from 'lucide-react'
import type { RateCard, Zone } from '../types'

export default function RatesPage() {
  const [showModal, setShowModal] = useState(false)
  const [editingRate, setEditingRate] = useState<RateCard | null>(null)
  const [deletingRate, setDeletingRate] = useState<RateCard | null>(null)
  const [showLookup, setShowLookup] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const queryClient = useQueryClient()

  const { data: rates = [], isLoading } = useQuery({
    queryKey: ['rates'],
    queryFn: fetchRateCards,
    refetchInterval: 30000,
  })

  const { data: zones = [] } = useQuery({
    queryKey: ['zones'],
    queryFn: fetchZones,
  })

  const createMutation = useMutation({
    mutationFn: createRateCard,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rates'] })
      setShowModal(false)
      setEditingRate(null)
      setError(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al crear la tarifa'
      setError(message)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: Partial<RateCard> }) =>
      updateRateCard(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rates'] })
      setShowModal(false)
      setEditingRate(null)
      setError(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al actualizar la tarifa'
      setError(message)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteRateCard,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['rates'] })
      setDeletingRate(null)
      setError(null)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      const message = err.response?.data?.message || err.message || 'Error al eliminar la tarifa'
      setError(message)
    },
  })

  const handleEdit = (rate: RateCard) => {
    setEditingRate(rate)
    setError(null)
    setShowModal(true)
  }

  const handleCreate = () => {
    setEditingRate(null)
    setError(null)
    setShowModal(true)
  }

  const columns = [
    {
      key: 'destination_prefix',
      header: 'Prefijo',
      render: (rate: RateCard) => (
        <span className="font-mono font-bold text-[var(--color-text-primary)]">
          {rate.destination_prefix}
        </span>
      ),
    },
    {
      key: 'destination_name',
      header: 'Destino',
      render: (rate: RateCard) => (
        <span className="font-medium text-[var(--color-text-primary)]">
          {rate.destination_name}
        </span>
      ),
    },
    {
      key: 'rate_per_minute',
      header: 'Tarifa/Min',
      render: (rate: RateCard) => (
        <span className="font-mono text-green-600 font-bold">
          S/{Number(rate.rate_per_minute).toFixed(4)}
        </span>
      ),
      className: 'text-right',
    },
    {
      key: 'billing_increment',
      header: 'Incremento (seg)',
      render: (rate: RateCard) => (
        <span className="font-mono text-[var(--color-text-secondary)]">{rate.billing_increment}</span>
      ),
      className: 'text-center',
    },
    {
      key: 'connection_fee',
      header: 'Cargo Conexión',
      render: (rate: RateCard) => (
        <span className="font-mono text-[var(--color-text-secondary)]">
          S/{Number(rate.connection_fee).toFixed(4)}
        </span>
      ),
      className: 'text-right',
    },
    {
      key: 'priority',
      header: 'Prioridad',
      render: (rate: RateCard) => (
        <Badge variant={rate.priority === 1 ? 'success' : 'default'}>
          {rate.priority}
        </Badge>
      ),
      className: 'text-center',
    },
    {
      key: 'effective_start',
      header: 'Inicio',
      render: (rate: RateCard) =>
        new Date(rate.effective_start).toLocaleDateString('es-PE'),
    },
    {
      key: 'effective_end',
      header: 'Fin',
      render: (rate: RateCard) =>
        rate.effective_end
          ? new Date(rate.effective_end).toLocaleDateString('es-PE')
          : <Badge variant="success">Vigente</Badge>,
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (rate: RateCard) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => handleEdit(rate)}
            className="p-1.5 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
            title="Editar"
          >
            <Edit className="w-4 h-4" />
          </button>
          <button
            onClick={() => setDeletingRate(rate)}
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
          <h1 className="text-3xl font-bold gradient-text tracking-tight">
            Gestión de Tarifas
          </h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Configura tarifas por destino con algoritmo LPM (Longest Prefix Match)
          </p>
        </div>
        <div className="flex items-center space-x-3">
          <button
            onClick={() => setShowLookup(true)}
            className="flex items-center px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)] transition-colors"
          >
            <Search className="w-5 h-5 mr-2" />
            Consultar Tarifa
          </button>
          <button
            onClick={handleCreate}
            className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            <Plus className="w-5 h-5 mr-2" />
            Nueva Tarifa
          </button>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Total Tarifas</p>
              <p className="text-2xl font-bold text-[var(--color-text-primary)]">{rates.length}</p>
            </div>
            <Settings className="w-8 h-8 text-blue-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Tarifa Promedio/Min</p>
              <p className="text-2xl font-bold text-green-600">
                S/
                {rates.length > 0
                  ? (
                      rates.reduce((sum, r) => sum + Number(r.rate_per_minute), 0) /
                      rates.length
                    ).toFixed(4)
                  : '0.0000'}
              </p>
            </div>
            <Settings className="w-8 h-8 text-green-500" />
          </div>
        </div>
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-[var(--color-text-tertiary)]">Vigentes</p>
              <p className="text-2xl font-bold text-blue-600">
                {rates.filter((r) => !r.effective_end).length}
              </p>
            </div>
            <Settings className="w-8 h-8 text-purple-500" />
          </div>
        </div>
      </div>

      {/* Info Box */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
        <p className="text-blue-800 text-sm">
          <strong>Algoritmo LPM (Longest Prefix Match):</strong> El sistema
          selecciona automáticamente la tarifa con el prefijo más largo que
          coincida con el número de destino. Por ejemplo, para marcar a
          "51987654321", el prefijo "51987" tiene prioridad sobre "519" o "51".
        </p>
      </div>

      {/* Rates Table */}
      <DataTable
        columns={columns}
        data={rates}
        loading={isLoading}
        emptyMessage="No hay tarifas registradas"
        searchable={true}
        searchPlaceholder="Buscar por prefijo o destino..."
      />

      {/* Create/Edit Modal */}
      {showModal && (
        <RateModal
          rate={editingRate}
          zones={zones}
          onClose={() => {
            setShowModal(false)
            setEditingRate(null)
            setError(null)
          }}
          onSubmit={(data) => {
            if (editingRate) {
              updateMutation.mutate({ id: editingRate.id, data })
            } else {
              createMutation.mutate(data)
            }
          }}
          isLoading={createMutation.isPending || updateMutation.isPending}
          error={error}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deletingRate && (
        <DeleteConfirmModal
          title="Eliminar Tarifa"
          message={`¿Estás seguro de que deseas eliminar la tarifa para "${deletingRate.destination_prefix} - ${deletingRate.destination_name}"? Esta acción no se puede deshacer.`}
          onClose={() => {
            setDeletingRate(null)
            setError(null)
          }}
          onConfirm={() => deleteMutation.mutate(deletingRate.id)}
          isLoading={deleteMutation.isPending}
          error={error}
        />
      )}

      {/* Lookup Modal */}
      {showLookup && <LookupModal onClose={() => setShowLookup(false)} />}
    </div>
  )
}

interface RateModalProps {
  rate: RateCard | null
  zones: Zone[]
  onClose: () => void
  onSubmit: (data: Partial<RateCard>) => void
  isLoading: boolean
  error: string | null
}

function RateModal({ rate, zones, onClose, onSubmit, isLoading, error }: RateModalProps) {
  // Find zone_id from destination_name if editing
  const findZoneId = () => {
    if (rate?.destination_name) {
      const zone = zones.find(z => z.zone_name === rate.destination_name)
      return zone?.id || ''
    }
    return ''
  }

  const [formData, setFormData] = useState<Partial<RateCard> & { zone_id?: number | string }>({
    destination_prefix: rate?.destination_prefix || '',
    destination_name: rate?.destination_name || '',
    zone_id: findZoneId(),
    rate_per_minute: rate?.rate_per_minute || 0,
    billing_increment: rate?.billing_increment || 60,
    connection_fee: rate?.connection_fee || 0,
    priority: rate?.priority || 1,
    effective_start: rate?.effective_start || new Date().toISOString().split('T')[0],
    effective_end: rate?.effective_end || null,
  })

  const handleZoneChange = (zoneId: string) => {
    const selectedZone = zones.find(z => z.id === parseInt(zoneId))
    setFormData({
      ...formData,
      zone_id: zoneId ? parseInt(zoneId) : '',
      destination_name: selectedZone?.zone_name || '',
    })
  }

  const [validationError, setValidationError] = useState<string | null>(null)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setValidationError(null)

    // Validate dates: effective_end must be >= effective_start
    if (formData.effective_end && formData.effective_start) {
      const startDate = new Date(formData.effective_start)
      const endDate = new Date(formData.effective_end)
      if (endDate < startDate) {
        setValidationError('La fecha de fin no puede ser anterior a la fecha de inicio')
        return
      }
    }

    // For update, only send fields the backend accepts
    if (rate) {
      const updateData: Partial<RateCard> = {
        destination_name: formData.destination_name,
        rate_per_minute: formData.rate_per_minute,
        billing_increment: formData.billing_increment,
        connection_fee: formData.connection_fee,
        priority: formData.priority,
        effective_end: formData.effective_end ? new Date(formData.effective_end).toISOString() : null,
      }
      onSubmit(updateData)
    } else {
      // For create, send all fields
      const createData = {
        ...formData,
        effective_start: formData.effective_start ? new Date(formData.effective_start).toISOString() : new Date().toISOString(),
        effective_end: formData.effective_end ? new Date(formData.effective_end).toISOString() : null,
      }
      onSubmit(createData)
    }
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">
            {rate ? 'Editar Tarifa' : 'Nueva Tarifa'}
          </h2>
          <button
            onClick={onClose}
            className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {(error || validationError) && (
            <div className="p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
              {validationError || error}
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Prefijo de Destino *
              </label>
              <input
                type="text"
                required
                value={formData.destination_prefix}
                onChange={(e) =>
                  setFormData({ ...formData, destination_prefix: e.target.value })
                }
                placeholder="Ej: 51, 519, 51987"
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Solo números, sin símbolos
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Zona de Destino *
              </label>
              <select
                required
                value={formData.zone_id || ''}
                onChange={(e) => handleZoneChange(e.target.value)}
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="">Seleccionar zona...</option>
                {zones.map((zone) => (
                  <option key={zone.id} value={zone.id}>
                    {zone.zone_name} {zone.description ? `- ${zone.description}` : ''}
                  </option>
                ))}
              </select>
              {zones.length === 0 && (
                <p className="text-xs text-amber-600 mt-1">
                  No hay zonas disponibles. Crea una zona primero.
                </p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Tarifa por Minuto (S/) *
              </label>
              <input
                type="number"
                step="0.0001"
                min="0"
                required
                value={formData.rate_per_minute}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    rate_per_minute: parseFloat(e.target.value) || 0,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Incremento de Facturación (seg) *
              </label>
              <input
                type="number"
                min="1"
                required
                value={formData.billing_increment}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    billing_increment: parseInt(e.target.value) || 60,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                Típico: 60 seg (1 min) o 6 seg
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Cargo de Conexión (S/)
              </label>
              <input
                type="number"
                step="0.0001"
                min="0"
                value={formData.connection_fee}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    connection_fee: parseFloat(e.target.value) || 0,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Prioridad
              </label>
              <input
                type="number"
                min="1"
                max="100"
                value={formData.priority}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    priority: parseInt(e.target.value) || 1,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
              <p className="text-xs text-[var(--color-text-tertiary)] mt-1">1 = más alta</p>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Fecha Inicio *
              </label>
              <input
                type="date"
                required
                value={formData.effective_start?.toString().split('T')[0]}
                onChange={(e) =>
                  setFormData({ ...formData, effective_start: e.target.value })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                Fecha Fin (opcional)
              </label>
              <input
                type="date"
                value={formData.effective_end?.toString().split('T')[0] || ''}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    effective_end: e.target.value || null,
                  })
                }
                className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
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
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {isLoading ? 'Guardando...' : rate ? 'Actualizar' : 'Crear'}
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
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-md w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">{title}</h2>
          <button
            onClick={onClose}
            className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
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

interface LookupModalProps {
  onClose: () => void
}

function LookupModal({ onClose }: LookupModalProps) {
  const [destination, setDestination] = useState('')
  const [result, setResult] = useState<RateCard | null | undefined>(undefined)
  const [loading, setLoading] = useState(false)

  const handleLookup = async () => {
    setLoading(true)
    try {
      const rate = await lookupRate(destination)
      setResult(rate)
    } catch {
      setResult(null)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-xl max-w-lg w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-[var(--color-border-primary)]">
          <h2 className="text-xl font-bold text-[var(--color-text-primary)]">
            Consultar Tarifa (LPM)
          </h2>
          <button
            onClick={onClose}
            className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="p-6 space-y-4">
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Número de Destino
            </label>
            <div className="flex space-x-2">
              <input
                type="text"
                value={destination}
                onChange={(e) => setDestination(e.target.value)}
                placeholder="Ej: 51987654321"
                className="flex-1 px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 font-mono"
              />
              <button
                onClick={handleLookup}
                disabled={loading || !destination}
                className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
              >
                {loading ? 'Buscando...' : 'Buscar'}
              </button>
            </div>
          </div>

          {result !== undefined && (
            <div className="mt-4">
              {result ? (
                <div className="bg-green-50 border border-green-200 rounded-lg p-4 space-y-2">
                  <p className="text-sm font-medium text-green-800">
                    Tarifa Encontrada (LPM)
                  </p>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span className="text-[var(--color-text-secondary)]">Prefijo:</span>
                      <span className="font-mono font-bold ml-2">
                        {result.destination_prefix}
                      </span>
                    </div>
                    <div>
                      <span className="text-[var(--color-text-secondary)]">Destino:</span>
                      <span className="font-medium ml-2">
                        {result.destination_name}
                      </span>
                    </div>
                    <div>
                      <span className="text-[var(--color-text-secondary)]">Tarifa/Min:</span>
                      <span className="font-mono font-bold text-green-600 ml-2">
                        S/{Number(result.rate_per_minute).toFixed(4)}
                      </span>
                    </div>
                    <div>
                      <span className="text-[var(--color-text-secondary)]">Incremento:</span>
                      <span className="font-mono ml-2">
                        {result.billing_increment}s
                      </span>
                    </div>
                    <div>
                      <span className="text-[var(--color-text-secondary)]">Cargo Conexión:</span>
                      <span className="font-mono ml-2">
                        S/{Number(result.connection_fee).toFixed(4)}
                      </span>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="bg-red-50 border border-red-200 rounded-lg p-4">
                  <p className="text-sm text-red-800">
                    No se encontró tarifa para este destino
                  </p>
                </div>
              )}
            </div>
          )}
        </div>

        <div className="p-6 border-t border-[var(--color-border-primary)]">
          <button
            onClick={onClose}
            className="w-full px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)]"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  )
}
