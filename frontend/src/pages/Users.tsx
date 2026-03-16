import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchUsers,
  createUser,
  updateUser,
  deleteUser,
  type UserCreateRequest,
  type UserUpdateRequest,
} from '../api/client'
import type { User } from '../types'
import DataTable from '../components/DataTable'
import { Plus, Pencil, Trash2, X, Shield, User as UserIcon, ShieldCheck } from 'lucide-react'

export default function Users() {
  const [showModal, setShowModal] = useState(false)
  const [editingUser, setEditingUser] = useState<User | null>(null)
  const [deletingUser, setDeletingUser] = useState<User | null>(null)
  const [page, setPage] = useState(1)
  const [perPage] = useState(50)

  const [formData, setFormData] = useState<UserCreateRequest>({
    username: '',
    password: '',
    nombre: '',
    apellido: '',
    email: '',
    role: 'operator',
  })

  const [formError, setFormError] = useState<string>('')

  const queryClient = useQueryClient()

  const { data, isLoading } = useQuery({
    queryKey: ['users', page, perPage],
    queryFn: () => fetchUsers(page, perPage),
  })

  const createMutation = useMutation({
    mutationFn: createUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      setShowModal(false)
      resetForm()
    },
    onError: (error: any) => {
      setFormError(error.response?.data?.message || 'Error al crear usuario')
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UserUpdateRequest }) => updateUser(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      setShowModal(false)
      setEditingUser(null)
      resetForm()
    },
    onError: (error: any) => {
      setFormError(error.response?.data?.message || 'Error al actualizar usuario')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: deleteUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      setDeletingUser(null)
    },
  })

  const resetForm = () => {
    setFormData({
      username: '',
      password: '',
      nombre: '',
      apellido: '',
      email: '',
      role: 'operator',
    })
    setFormError('')
  }

  const handleCreate = () => {
    setEditingUser(null)
    resetForm()
    setShowModal(true)
  }

  const handleEdit = (user: User) => {
    setEditingUser(user)
    setFormData({
      username: user.username,
      password: '', // No se cambia la contraseña al editar
      nombre: user.nombre || '',
      apellido: user.apellido || '',
      email: user.email || '',
      role: user.role,
    })
    setFormError('')
    setShowModal(true)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setFormError('')

    if (editingUser) {
      // Update
      const updateData: UserUpdateRequest = {
        nombre: formData.nombre || undefined,
        apellido: formData.apellido || undefined,
        email: formData.email || undefined,
        role: formData.role,
        activo: true, // Por ahora siempre activo al editar
      }
      updateMutation.mutate({ id: editingUser.id, data: updateData })
    } else {
      // Create
      if (!formData.username || !formData.password) {
        setFormError('Usuario y contraseña son obligatorios')
        return
      }
      createMutation.mutate(formData)
    }
  }

  const handleDelete = () => {
    if (deletingUser) {
      deleteMutation.mutate(deletingUser.id)
    }
  }

  const getRoleBadgeColor = (role: string) => {
    switch (role) {
      case 'superadmin':
        return 'bg-red-100 text-red-800 border-red-200'
      case 'admin':
        return 'bg-blue-100 text-blue-800 border-blue-200'
      case 'operator':
        return 'bg-gray-100 text-gray-800 border-gray-200'
      default:
        return 'bg-gray-100 text-gray-800 border-gray-200'
    }
  }

  const getRoleIcon = (role: string) => {
    switch (role) {
      case 'superadmin':
        return <ShieldCheck className="w-4 h-4" />
      case 'admin':
        return <Shield className="w-4 h-4" />
      default:
        return <UserIcon className="w-4 h-4" />
    }
  }

  const columns = [
    { key: 'id', header: 'ID', className: 'w-20' },
    { key: 'username', header: 'Usuario' },
    {
      key: 'nombre',
      header: 'Nombre Completo',
      render: (user: User) => {
        const nombre = [user.nombre, user.apellido].filter(Boolean).join(' ')
        return nombre || '-'
      },
    },
    { key: 'email', header: 'Email', render: (user: User) => user.email || '-' },
    {
      key: 'role',
      header: 'Rol',
      render: (user: User) => (
        <span
          className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium border ${getRoleBadgeColor(user.role)}`}
        >
          {getRoleIcon(user.role)}
          {user.role}
        </span>
      ),
    },
    {
      key: 'activo',
      header: 'Estado',
      render: (user: User) => (
        <span
          className={`px-2 py-1 rounded-full text-xs font-medium ${
            user.activo
              ? 'bg-green-100 text-green-800 border border-green-200'
              : 'bg-gray-100 text-gray-800 border border-gray-200'
          }`}
        >
          {user.activo ? 'Activo' : 'Inactivo'}
        </span>
      ),
    },
    {
      key: 'ultimo_login',
      header: 'Último Login',
      render: (user: User) =>
        user.ultimo_login ? new Date(user.ultimo_login).toLocaleString('es-PE') : 'Nunca',
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (user: User) => (
        <div className="flex items-center space-x-2">
          <button
            onClick={() => handleEdit(user)}
            className="p-1 text-blue-600 hover:text-blue-800 hover:bg-blue-50 rounded"
            title="Editar"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <button
            onClick={() => setDeletingUser(user)}
            className="p-1 text-red-600 hover:text-red-800 hover:bg-red-50 rounded"
            title="Eliminar"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
      className: 'w-24',
    },
  ]

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">Gestión de Usuarios</h1>
          <p className="text-sm text-slate-600">Administrar usuarios del sistema (solo superadmin)</p>
        </div>
        <button
          onClick={handleCreate}
          className="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <Plus className="w-4 h-4" />
          Nuevo Usuario
        </button>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <div className="flex items-center gap-3">
            <UserIcon className="w-8 h-8 text-slate-600" />
            <div>
              <p className="text-sm text-slate-600">Total Usuarios</p>
              <p className="text-2xl font-bold text-slate-900">{data?.total || 0}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200">
        <DataTable
          data={data?.users || []}
          columns={columns}
          loading={isLoading}
          pagination={{
            page: page,
            totalPages: data?.total_pages || 1,
            onPageChange: setPage,
          }}
        />
      </div>

      {/* Create/Edit Modal */}
      {showModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
          <div className="bg-white rounded-lg shadow-xl max-w-md w-full max-h-[90vh] overflow-y-auto">
            <div className="sticky top-0 bg-white border-b border-slate-200 px-6 py-4 flex items-center justify-between">
              <h3 className="text-lg font-semibold text-slate-900">
                {editingUser ? 'Editar Usuario' : 'Nuevo Usuario'}
              </h3>
              <button
                onClick={() => {
                  setShowModal(false)
                  setEditingUser(null)
                  resetForm()
                }}
                className="text-slate-400 hover:text-slate-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="p-6 space-y-4">
              {formError && (
                <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded text-sm">
                  {formError}
                </div>
              )}

              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">
                  Usuario *
                </label>
                <input
                  type="text"
                  value={formData.username}
                  onChange={(e) => setFormData({ ...formData, username: e.target.value })}
                  disabled={!!editingUser}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-slate-100"
                  required={!editingUser}
                />
              </div>

              {!editingUser && (
                <div>
                  <label className="block text-sm font-medium text-slate-700 mb-1">
                    Contraseña *
                  </label>
                  <input
                    type="password"
                    value={formData.password}
                    onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                    className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    required
                    minLength={6}
                  />
                  <p className="text-xs text-slate-500 mt-1">Mínimo 6 caracteres</p>
                </div>
              )}

              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Nombre</label>
                <input
                  type="text"
                  value={formData.nombre}
                  onChange={(e) => setFormData({ ...formData, nombre: e.target.value })}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Apellido</label>
                <input
                  type="text"
                  value={formData.apellido}
                  onChange={(e) => setFormData({ ...formData, apellido: e.target.value })}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Email</label>
                <input
                  type="email"
                  value={formData.email}
                  onChange={(e) => setFormData({ ...formData, email: e.target.value })}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-700 mb-1">Rol *</label>
                <select
                  value={formData.role}
                  onChange={(e) => setFormData({ ...formData, role: e.target.value })}
                  className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  required
                >
                  <option value="operator">Operador</option>
                  <option value="admin">Administrador</option>
                  <option value="superadmin">Superadministrador</option>
                </select>
              </div>

              <div className="flex gap-3 pt-4">
                <button
                  type="button"
                  onClick={() => {
                    setShowModal(false)
                    setEditingUser(null)
                    resetForm()
                  }}
                  className="flex-1 px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50 transition-colors"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={createMutation.isPending || updateMutation.isPending}
                  className="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {createMutation.isPending || updateMutation.isPending
                    ? 'Guardando...'
                    : editingUser
                      ? 'Actualizar'
                      : 'Crear'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deletingUser && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
          <div className="bg-white rounded-lg shadow-xl max-w-md w-full p-6">
            <h3 className="text-lg font-semibold text-slate-900 mb-4">Confirmar Eliminación</h3>
            <p className="text-slate-600 mb-6">
              ¿Estás seguro de eliminar al usuario <strong>{deletingUser.username}</strong>?
              <br />
              Esta acción no se puede deshacer.
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeletingUser(null)}
                className="flex-1 px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                onClick={handleDelete}
                disabled={deleteMutation.isPending}
                className="flex-1 px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50"
              >
                {deleteMutation.isPending ? 'Eliminando...' : 'Eliminar'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
