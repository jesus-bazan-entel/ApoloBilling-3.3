import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { isAxiosError } from 'axios'
import {
  fetchUsers,
  createUser,
  updateUser,
  deleteUser,
  fetchModuleCatalog,
  fetchUserModules,
  updateUserModules,
  getCurrentUser,
  type UserCreateRequest,
  type UserUpdateRequest,
} from '../api/client'
import type { User, UserRole } from '../types'
import DataTable from '../components/DataTable'
import {
  Plus,
  Pencil,
  Trash2,
  X,
  Shield,
  User as UserIcon,
  ShieldCheck,
  LayoutGrid,
  LayoutDashboard,
  Phone,
  FileText,
  Users as UsersIcon,
  CreditCard,
  DollarSign,
  Globe,
  BarChart3,
  Headphones,
  Building2,
  Route,
  Server,
  Layers,
  Activity,
} from 'lucide-react'

// Mirrors the icon set used in the sidebar (Layout.tsx) so a module in this
// checklist is instantly recognizable as "that item in the menu".
const MODULE_ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  dashboard: LayoutDashboard,
  active_calls: Phone,
  cdr: FileText,
  accounts: UsersIcon,
  plans: CreditCard,
  balance: DollarSign,
  zones: Globe,
  rates: BarChart3,
  sip_devices: Headphones,
  inbound_routing: Building2,
  outbound_routing: Route,
  carriers: Server,
  carrier_groups: Layers,
  endpoints: Shield,
  endpoint_monitor: Activity,
}

const ROLE_LABELS: Record<UserRole, string> = {
  usuario: 'Usuario',
  admin: 'Administrador',
  superadmin: 'Superadministrador',
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (isAxiosError(error)) {
    return error.response?.data?.message || fallback
  }
  return fallback
}

export default function Users() {
  const [showModal, setShowModal] = useState(false)
  const [editingUser, setEditingUser] = useState<User | null>(null)
  const [deletingUser, setDeletingUser] = useState<User | null>(null)
  const [page, setPage] = useState(1)
  const [perPage] = useState(50)
  const [selectedModules, setSelectedModules] = useState<string[]>([])

  const [formData, setFormData] = useState<UserCreateRequest>({
    username: '',
    password: '',
    nombre: '',
    apellido: '',
    email: '',
    role: 'usuario',
  })

  const [formError, setFormError] = useState<string>('')

  const queryClient = useQueryClient()

  const { data: currentUser } = useQuery({
    queryKey: ['currentUser'],
    queryFn: getCurrentUser,
    staleTime: 60000,
  })

  const isSuperadmin = currentUser?.role === 'superadmin'
  // Mirrors UserRole::assignable_roles on the backend
  const assignableRoles: UserRole[] = isSuperadmin ? ['usuario', 'admin', 'superadmin'] : ['usuario']

  const { data, isLoading } = useQuery({
    queryKey: ['users', page, perPage],
    queryFn: () => fetchUsers(page, perPage),
  })

  const { data: moduleCatalog = [] } = useQuery({
    queryKey: ['moduleCatalog'],
    queryFn: fetchModuleCatalog,
    staleTime: 5 * 60 * 1000,
  })

  const { data: userModules } = useQuery({
    queryKey: ['userModules', editingUser?.id],
    queryFn: () => fetchUserModules(editingUser!.id),
    enabled: !!editingUser && editingUser.role === 'usuario',
  })

  // Sync the checklist to freshly-fetched grants for the user being edited.
  // Adjusting state during render (rather than in a useEffect) per React's
  // guidance for "resetting state when a prop changes".
  const [syncedForUserId, setSyncedForUserId] = useState<number | null>(null)
  if (editingUser && userModules && syncedForUserId !== editingUser.id) {
    setSyncedForUserId(editingUser.id)
    setSelectedModules(userModules.granted)
  }

  const createMutation = useMutation({
    mutationFn: createUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] })
      setShowModal(false)
      resetForm()
    },
    onError: (error: unknown) => {
      setFormError(getErrorMessage(error, 'Error al crear usuario'))
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UserUpdateRequest }) => updateUser(id, data),
    onError: (error: unknown) => {
      setFormError(getErrorMessage(error, 'Error al actualizar usuario'))
    },
  })

  const modulesMutation = useMutation({
    mutationFn: ({ id, modules }: { id: number; modules: string[] }) => updateUserModules(id, modules),
    onError: (error: unknown) => {
      setFormError(getErrorMessage(error, 'Error al guardar los módulos'))
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
      role: 'usuario',
    })
    setSelectedModules([])
    setSyncedForUserId(null)
    setFormError('')
  }

  const handleCreate = () => {
    setEditingUser(null)
    resetForm()
    setFormData(prev => ({ ...prev, role: assignableRoles[0] }))
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

  // Can the current session touch (edit/delete) this row? Mirrors the
  // backend's can_manage() hierarchy: Superadmin manages everyone; Admin
  // only manages Usuario accounts.
  const canManageRow = (user: User) => {
    if (!currentUser) return false
    if (currentUser.role === 'superadmin') return true
    if (currentUser.role === 'admin') return user.role === 'usuario'
    return false
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setFormError('')

    if (editingUser) {
      const updateData: UserUpdateRequest = {
        nombre: formData.nombre || undefined,
        apellido: formData.apellido || undefined,
        email: formData.email || undefined,
        role: formData.role,
        activo: true, // Por ahora siempre activo al editar
      }

      try {
        await updateMutation.mutateAsync({ id: editingUser.id, data: updateData })

        if (formData.role === 'usuario') {
          await modulesMutation.mutateAsync({ id: editingUser.id, modules: selectedModules })
        }

        queryClient.invalidateQueries({ queryKey: ['users'] })
        queryClient.invalidateQueries({ queryKey: ['userModules', editingUser.id] })
        setShowModal(false)
        setEditingUser(null)
        resetForm()
      } catch {
        // Error message already set by the mutation's onError handler
      }
    } else {
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

  const toggleModule = (key: string) => {
    setSelectedModules(prev =>
      prev.includes(key) ? prev.filter(k => k !== key) : [...prev, key]
    )
  }

  const getRoleBadgeColor = (role: string) => {
    switch (role) {
      case 'superadmin':
        return 'bg-red-100 text-red-800 border-red-200'
      case 'admin':
        return 'bg-blue-100 text-blue-800 border-blue-200'
      case 'usuario':
        return 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border-[var(--color-border-primary)]'
      default:
        return 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border-[var(--color-border-primary)]'
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
          {ROLE_LABELS[user.role] || user.role}
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
              : 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border border-[var(--color-border-primary)]'
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
      render: (user: User) =>
        canManageRow(user) ? (
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
        ) : (
          <span className="text-xs text-[var(--color-text-tertiary)]">—</span>
        ),
      className: 'w-24',
    },
  ]

  // Only modules with min_role = usuario can ever be granted — grouped by
  // section to match the sidebar menu structure.
  const assignableModules = moduleCatalog.filter(m => m.min_role === 'usuario')
  const modulesBySection = assignableModules.reduce<Record<string, typeof assignableModules>>(
    (acc, m) => {
      acc[m.seccion] = acc[m.seccion] || []
      acc[m.seccion].push(m)
      return acc
    },
    {}
  )

  const showModulesPanel = !!editingUser && formData.role === 'usuario'

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold gradient-text tracking-tight">Gestión de Usuarios</h1>
          <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
            Administrar usuarios y visibilidad de módulos (Admin y Superadmin)
          </p>
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
        <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm p-4 border border-[var(--color-border-primary)]">
          <div className="flex items-center gap-3">
            <UserIcon className="w-8 h-8 text-[var(--color-text-secondary)]" />
            <div>
              <p className="text-sm text-[var(--color-text-secondary)]">Total Usuarios</p>
              <p className="text-2xl font-bold text-[var(--color-text-primary)]">{data?.total || 0}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="bg-[var(--color-bg-card)] rounded-lg shadow-sm border border-[var(--color-border-primary)]">
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
          <div
            className={`bg-[var(--color-bg-card)] rounded-lg shadow-xl w-full max-h-[90vh] overflow-y-auto transition-[max-width] ${
              showModulesPanel ? 'max-w-3xl' : 'max-w-md'
            }`}
          >
            <div className="sticky top-0 z-10 bg-[var(--color-bg-card)] border-b border-[var(--color-border-primary)] px-6 py-4 flex items-center justify-between">
              <h3 className="text-lg font-semibold text-[var(--color-text-primary)]">
                {editingUser ? 'Editar Usuario' : 'Nuevo Usuario'}
              </h3>
              <button
                onClick={() => {
                  setShowModal(false)
                  setEditingUser(null)
                  resetForm()
                }}
                className="text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="p-6">
              {formError && (
                <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded text-sm mb-4">
                  {formError}
                </div>
              )}

              <div className={showModulesPanel ? 'grid grid-cols-1 lg:grid-cols-2 gap-6' : ''}>
                {/* Basic fields */}
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                      Usuario *
                    </label>
                    <input
                      type="text"
                      value={formData.username}
                      onChange={(e) => setFormData({ ...formData, username: e.target.value })}
                      disabled={!!editingUser}
                      className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-[var(--color-bg-secondary)]"
                      required={!editingUser}
                    />
                  </div>

                  {!editingUser && (
                    <div>
                      <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                        Contraseña *
                      </label>
                      <input
                        type="password"
                        value={formData.password}
                        onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                        className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                        required
                        minLength={6}
                      />
                      <p className="text-xs text-[var(--color-text-tertiary)] mt-1">Mínimo 6 caracteres</p>
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                        Nombre
                      </label>
                      <input
                        type="text"
                        value={formData.nombre}
                        onChange={(e) => setFormData({ ...formData, nombre: e.target.value })}
                        className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      />
                    </div>
                    <div>
                      <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                        Apellido
                      </label>
                      <input
                        type="text"
                        value={formData.apellido}
                        onChange={(e) => setFormData({ ...formData, apellido: e.target.value })}
                        className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                      Email
                    </label>
                    <input
                      type="email"
                      value={formData.email}
                      onChange={(e) => setFormData({ ...formData, email: e.target.value })}
                      className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
                      Rol *
                    </label>
                    <select
                      value={formData.role}
                      onChange={(e) => setFormData({ ...formData, role: e.target.value })}
                      disabled={assignableRoles.length === 1}
                      className="w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-[var(--color-bg-secondary)]"
                      required
                    >
                      {assignableRoles.map(role => (
                        <option key={role} value={role}>
                          {ROLE_LABELS[role]}
                        </option>
                      ))}
                    </select>
                    {!isSuperadmin && (
                      <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                        Como Admin solo puedes crear/editar usuarios con rol Usuario.
                      </p>
                    )}
                    {isSuperadmin && formData.role !== 'usuario' && (
                      <p className="text-xs text-[var(--color-text-tertiary)] mt-1">
                        Admin y Superadmin ven todos los módulos automáticamente — la visibilidad por módulo solo
                        aplica al rol Usuario.
                      </p>
                    )}
                  </div>
                </div>

                {/* Module visibility panel — only meaningful for role = usuario */}
                {showModulesPanel && (
                  <div className="flex flex-col min-h-0 lg:border-l lg:border-[var(--color-border-primary)] lg:pl-6">
                    <div className="flex items-center justify-between gap-2 mb-1">
                      <div className="flex items-center gap-1.5">
                        <LayoutGrid className="w-4 h-4 text-blue-500" />
                        <span className="text-sm font-semibold text-[var(--color-text-primary)]">
                          Módulos visibles
                        </span>
                      </div>
                      <span className="text-xs font-semibold px-2 py-0.5 rounded-full bg-blue-50 text-blue-700 dark:bg-blue-500/10 dark:text-blue-400 whitespace-nowrap">
                        {selectedModules.length} / {assignableModules.length}
                      </span>
                    </div>
                    <p className="text-xs text-[var(--color-text-tertiary)] mb-2">
                      Elegí qué secciones del menú puede ver. El Panel de Control siempre es visible.
                    </p>
                    <div className="flex items-center gap-3 mb-3">
                      <button
                        type="button"
                        onClick={() => setSelectedModules(assignableModules.map(m => m.module_key))}
                        className="text-xs font-medium text-blue-600 hover:text-blue-700 dark:text-blue-400"
                      >
                        Seleccionar todos
                      </button>
                      <span className="text-[var(--color-border-primary)]">·</span>
                      <button
                        type="button"
                        onClick={() => setSelectedModules([])}
                        className="text-xs font-medium text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
                      >
                        Ninguno
                      </button>
                    </div>

                    <div className="space-y-2.5 max-h-[360px] overflow-y-auto pr-1 rounded-lg border border-[var(--color-border-primary)] bg-[var(--color-bg-secondary)] p-2.5">
                      {Object.entries(modulesBySection).map(([seccion, modules]) => {
                        const sectionSelected = modules.filter(m => selectedModules.includes(m.module_key)).length
                        return (
                          <div
                            key={seccion}
                            className="bg-[var(--color-bg-card)] rounded-lg border border-[var(--color-border-primary)] p-2.5"
                          >
                            <div className="flex items-center justify-between mb-1.5 px-0.5">
                              <p className="text-[10px] font-bold uppercase tracking-wide text-[var(--color-text-tertiary)]">
                                {seccion}
                              </p>
                              <span className="text-[10px] text-[var(--color-text-tertiary)] font-medium">
                                {sectionSelected}/{modules.length}
                              </span>
                            </div>
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
                              {modules.map(m => {
                                const Icon = MODULE_ICONS[m.module_key] || LayoutGrid
                                const checked = selectedModules.includes(m.module_key)
                                return (
                                  <label
                                    key={m.module_key}
                                    className={`flex items-center gap-2 px-2 py-1.5 rounded-md border cursor-pointer text-sm transition-colors ${
                                      checked
                                        ? 'bg-blue-50 border-blue-200 text-blue-800 dark:bg-blue-500/10 dark:border-blue-500/30 dark:text-blue-300'
                                        : 'bg-transparent border-transparent hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]'
                                    }`}
                                  >
                                    <input
                                      type="checkbox"
                                      checked={checked}
                                      onChange={() => toggleModule(m.module_key)}
                                      className="rounded border-[var(--color-border-primary)] text-blue-600 focus:ring-blue-500 flex-shrink-0"
                                    />
                                    <Icon className="w-3.5 h-3.5 flex-shrink-0" />
                                    <span className="truncate">{m.nombre}</span>
                                  </label>
                                )
                              })}
                            </div>
                          </div>
                        )
                      })}
                      {assignableModules.length === 0 && (
                        <p className="text-xs text-[var(--color-text-tertiary)] text-center py-4">
                          No hay módulos disponibles para asignar.
                        </p>
                      )}
                    </div>
                  </div>
                )}
              </div>

              <div className="flex gap-3 pt-6 mt-6 border-t border-[var(--color-border-primary)]">
                <button
                  type="button"
                  onClick={() => {
                    setShowModal(false)
                    setEditingUser(null)
                    resetForm()
                  }}
                  className="flex-1 px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)] transition-colors"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={createMutation.isPending || updateMutation.isPending || modulesMutation.isPending}
                  className="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {createMutation.isPending || updateMutation.isPending || modulesMutation.isPending
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
          <div className="bg-[var(--color-bg-card)] rounded-lg shadow-xl max-w-md w-full p-6">
            <h3 className="text-lg font-semibold text-[var(--color-text-primary)] mb-4">Confirmar Eliminación</h3>
            <p className="text-[var(--color-text-secondary)] mb-6">
              ¿Estás seguro de eliminar al usuario <strong>{deletingUser.username}</strong>?
              <br />
              Esta acción no se puede deshacer.
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeletingUser(null)}
                className="flex-1 px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-secondary)]"
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
