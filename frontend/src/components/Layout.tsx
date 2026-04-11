import { Link, useLocation, useNavigate } from 'react-router-dom'
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query'
import { checkHealth, getCurrentUser, logout, changePassword } from '../api/client'
import { useState } from 'react'
import { useTheme } from '../contexts/ThemeContext'
import {
  ArrowRightLeft,
  LayoutDashboard,
  Users,
  FileText,
  Phone,
  DollarSign,
  Globe,
  Activity,
  LogOut,
  ChevronDown,
  Shield,
  UserCog,
  FileSearch,
  Key,
  X,
  Eye,
  EyeOff,
  AlertCircle,
  CheckCircle,
  CreditCard,
  Headphones,
  BarChart3,
  Router,
  Sun,
  Moon,
  Monitor,
} from 'lucide-react'

// Navigation items organized by sections
interface NavSection {
  title: string
  items: NavItem[]
  adminOnly?: boolean
}

interface NavItem {
  name: string
  href: string
  icon: React.ComponentType<{ className?: string }>
}

// Main operations - visible to all authenticated users
const mainSection: NavSection = {
  title: 'Principal',
  items: [
    { name: 'Panel de Control', href: '/', icon: LayoutDashboard },
    { name: 'Llamadas Activas', href: '/calls', icon: Phone },
    { name: 'Registros CDR', href: '/cdr', icon: FileText },
  ],
}

// Accounts & Billing
const billingSection: NavSection = {
  title: 'Facturación',
  items: [
    { name: 'Cuentas', href: '/accounts', icon: Users },
    { name: 'Planes', href: '/plans', icon: CreditCard },
    { name: 'Saldos', href: '/balance', icon: DollarSign },
  ],
}

// Rates management
const ratesSection: NavSection = {
  title: 'Tarifas',
  items: [
    { name: 'Zonas', href: '/zones', icon: Globe },
    { name: 'Tarifas', href: '/rates', icon: BarChart3 },
  ],
}

// Administration - superadmin only
const adminSection: NavSection = {
  title: 'Administración',
  adminOnly: true,
  items: [
    { name: 'Dispositivos SIP', href: '/sip-devices', icon: Headphones },
    { name: 'Rutas Externas', href: '/routing', icon: Router },
    { name: 'Rutas Internas', href: '/internal-routing', icon: ArrowRightLeft },
  ],
}

// System - superadmin only
const systemSection: NavSection = {
  title: 'Sistema',
  adminOnly: true,
  items: [
    { name: 'Usuarios', href: '/users', icon: UserCog },
    { name: 'Auditoría', href: '/audit-logs', icon: FileSearch },
  ],
}

const allSections = [mainSection, billingSection, ratesSection, adminSection, systemSection]

interface LayoutProps {
  children: React.ReactNode
}

export default function Layout({ children }: LayoutProps) {
  const location = useLocation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const { theme, setTheme } = useTheme()
  const [userMenuOpen, setUserMenuOpen] = useState(false)
  const [themeMenuOpen, setThemeMenuOpen] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)
  const [showPasswordModal, setShowPasswordModal] = useState(false)
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showCurrentPassword, setShowCurrentPassword] = useState(false)
  const [showNewPassword, setShowNewPassword] = useState(false)
  const [passwordError, setPasswordError] = useState('')
  const [passwordSuccess, setPasswordSuccess] = useState('')

  const changePasswordMutation = useMutation({
    mutationFn: changePassword,
    onSuccess: () => {
      setPasswordSuccess('Contraseña cambiada exitosamente')
      setPasswordError('')
      setCurrentPassword('')
      setNewPassword('')
      setConfirmPassword('')
      setTimeout(() => {
        setShowPasswordModal(false)
        setPasswordSuccess('')
      }, 2000)
    },
    onError: (error: Error & { response?: { data?: { message?: string } } }) => {
      const message = error.response?.data?.message || error.message || 'Error al cambiar la contraseña'
      setPasswordError(message)
      setPasswordSuccess('')
    },
  })

  const handleChangePassword = () => {
    setPasswordError('')
    setPasswordSuccess('')

    if (!currentPassword || !newPassword || !confirmPassword) {
      setPasswordError('Todos los campos son obligatorios')
      return
    }

    if (newPassword.length < 6) {
      setPasswordError('La nueva contraseña debe tener al menos 6 caracteres')
      return
    }

    if (newPassword !== confirmPassword) {
      setPasswordError('Las contraseñas no coinciden')
      return
    }

    changePasswordMutation.mutate({
      current_password: currentPassword,
      new_password: newPassword,
    })
  }

  const openPasswordModal = () => {
    setUserMenuOpen(false)
    setShowPasswordModal(true)
    setCurrentPassword('')
    setNewPassword('')
    setConfirmPassword('')
    setPasswordError('')
    setPasswordSuccess('')
  }

  const { data: health } = useQuery({
    queryKey: ['health'],
    queryFn: checkHealth,
    refetchInterval: 5000,
    retry: false,
  })

  const { data: currentUser, isLoading: userLoading } = useQuery({
    queryKey: ['currentUser'],
    queryFn: getCurrentUser,
    retry: false,
    staleTime: 60000,
  })

  const isOnline = health?.status === 'ok' || health?.status === 'healthy'

  // Filter sections based on user role
  const visibleSections = allSections.filter(
    section => !section.adminOnly || currentUser?.role === 'superadmin'
  )

  const handleLogout = async () => {
    setIsLoggingOut(true)
    try {
      await logout()
      queryClient.clear()
      navigate('/login')
    } catch (error) {
      console.error('Error al cerrar sesión:', error)
      // Force redirect even on error
      navigate('/login')
    } finally {
      setIsLoggingOut(false)
    }
  }

  // Get initials for avatar
  const getInitials = (username: string) => {
    return username.slice(0, 2).toUpperCase()
  }

  // Get role color
  const getRoleStyle = (role: string) => {
    switch (role) {
      case 'superadmin':
        return 'bg-red-500/20 text-red-400 border-red-500/30'
      case 'admin':
        return 'bg-amber-500/20 text-amber-400 border-amber-500/30'
      case 'operator':
        return 'bg-cyan-500/20 text-cyan-400 border-cyan-500/30'
      default:
        return 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)] border-[var(--color-border-primary)]'
    }
  }

  return (
    <div className="min-h-screen bg-[#0A0F1A]">
      {/* Ambient background effect */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-0 left-64 right-0 h-px bg-gradient-to-r from-[#0099D9]/50 via-transparent to-[#6B7280]/50" />
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-[#0099D9]/10 via-transparent to-transparent" />
      </div>

      {/* Sidebar */}
      <aside className="fixed inset-y-0 left-0 w-64 bg-[#0D1B2A] border-r border-[#1B3A4B] z-20">
        {/* Logo */}
        <div className="flex items-center h-16 px-5 border-b border-[#1B3A4B]">
          <div className="flex items-center gap-3">
            <img
              src="/logo.png"
              alt="Fibertel"
              className="h-9 w-auto"
            />
            <div className="flex flex-col">
              <span className="text-[10px] font-medium tracking-[0.15em] text-[#0099D9] uppercase">
                Sistema de
              </span>
              <span className="text-sm font-bold tracking-tight text-white leading-none">
                Facturación
              </span>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav className="p-3 space-y-4 overflow-y-auto max-h-[calc(100vh-8rem)]">
          {visibleSections.map((section, sectionIdx) => (
            <div key={section.title}>
              {/* Section header */}
              {sectionIdx > 0 && (
                <div className="flex items-center gap-2 px-3 mb-2">
                  <span className="text-[10px] font-semibold text-[#64748B] uppercase tracking-wider">
                    {section.title}
                  </span>
                  <div className="flex-1 h-px bg-[#1B3A4B]" />
                </div>
              )}
              {sectionIdx === 0 && <div className="mb-1" />}

              {/* Section items */}
              <div className="space-y-0.5">
                {section.items.map((item) => {
                  const isActive = location.pathname === item.href
                  const Icon = item.icon

                  return (
                    <Link
                      key={item.name}
                      to={item.href}
                      className={`
                        group flex items-center px-3 py-2 rounded-md text-sm font-medium
                        transition-all duration-150 relative overflow-hidden
                        ${isActive
                          ? 'bg-[#0099D9]/10 text-[#38BDF8]'
                          : 'text-[#94A3B8] hover:text-white hover:bg-[#1B3A4B]'
                        }
                      `}
                    >
                      {/* Active indicator */}
                      {isActive && (
                        <div className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-5 bg-[#0099D9] rounded-r" />
                      )}
                      <Icon className={`w-4 h-4 mr-3 transition-colors ${isActive ? 'text-[#38BDF8]' : 'text-[#64748B] group-hover:text-[#94A3B8]'}`} />
                      <span>{item.name}</span>
                      {isActive && (
                        <div className="ml-auto w-1.5 h-1.5 rounded-full bg-[#0099D9] animate-pulse" />
                      )}
                    </Link>
                  )
                })}
              </div>
            </div>
          ))}
        </nav>

        {/* System Status */}
        <div className="absolute bottom-0 left-0 right-0 p-4 border-t border-[#1B3A4B] bg-[#0D1B2A]/90 backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Activity className="w-3.5 h-3.5 text-[#64748B]" />
              <span className="text-xs font-medium text-[#64748B] uppercase tracking-wide">
                Engine
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              <div
                className={`w-1.5 h-1.5 rounded-full ${
                  isOnline
                    ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]'
                    : 'bg-red-500 shadow-[0_0_6px_rgba(239,68,68,0.5)]'
                }`}
              />
              <span className={`text-xs font-mono ${isOnline ? 'text-emerald-400' : 'text-red-400'}`}>
                {isOnline ? 'ONLINE' : 'OFFLINE'}
              </span>
            </div>
          </div>
        </div>
      </aside>

      {/* Main content area */}
      <div className="ml-64 min-h-screen relative">
        {/* Top Header Bar */}
        <header className="sticky top-0 z-10 h-14 bg-[#0D1B2A]/90 backdrop-blur-md border-b border-[#1B3A4B]">
          <div className="flex items-center justify-between h-full px-6">
            {/* Left: Page context or breadcrumb could go here */}
            <div className="flex items-center gap-2">
              <div className="h-1 w-1 rounded-full bg-[#0099D9]/50" />
              <span className="text-xs font-mono text-[#64748B] uppercase tracking-wider">
                {allSections.flatMap(s => s.items).find(n => n.href === location.pathname)?.name || 'Dashboard'}
              </span>
            </div>

            {/* Right: User section */}
            <div className="flex items-center gap-4">
              {/* System time - adds industrial feel */}
              <div className="hidden sm:flex items-center gap-2 text-[#64748B]">
                <div className="h-4 w-px bg-[#334155]" />
                <span className="text-xs font-mono tabular-nums">
                  {new Date().toLocaleTimeString('es-ES', { hour: '2-digit', minute: '2-digit' })}
                </span>
              </div>

              {/* Theme Toggle */}
              <div className="relative">
                <button
                  onClick={() => setThemeMenuOpen(!themeMenuOpen)}
                  className="p-2 rounded-md bg-[#1B3A4B]/50 border border-[#334155] hover:border-[#0099D9]/50 transition-colors text-[#94A3B8] hover:text-white"
                  aria-label="Toggle theme"
                >
                  {theme === 'light' && <Sun className="w-4 h-4" />}
                  {theme === 'dark' && <Moon className="w-4 h-4" />}
                  {theme === 'system' && <Monitor className="w-4 h-4" />}
                </button>

                {/* Theme Dropdown */}
                {themeMenuOpen && (
                  <>
                    <div
                      className="fixed inset-0 z-10"
                      onClick={() => setThemeMenuOpen(false)}
                    />
                    <div className="absolute right-0 top-full mt-2 w-40 rounded-lg bg-[#1B3A4B] border border-[#334155] shadow-xl shadow-black/50 z-20 overflow-hidden">
                      <div className="p-1">
                        <button
                          onClick={() => {
                            setTheme('light')
                            setThemeMenuOpen(false)
                          }}
                          className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                            theme === 'light'
                              ? 'bg-[#0099D9]/10 text-[#38BDF8]'
                              : 'text-[#CBD5E1] hover:text-white hover:bg-[#334155]'
                          }`}
                        >
                          <Sun className="w-4 h-4" />
                          <span>Claro</span>
                        </button>
                        <button
                          onClick={() => {
                            setTheme('dark')
                            setThemeMenuOpen(false)
                          }}
                          className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                            theme === 'dark'
                              ? 'bg-[#0099D9]/10 text-[#38BDF8]'
                              : 'text-[#CBD5E1] hover:text-white hover:bg-[#334155]'
                          }`}
                        >
                          <Moon className="w-4 h-4" />
                          <span>Oscuro</span>
                        </button>
                        <button
                          onClick={() => {
                            setTheme('system')
                            setThemeMenuOpen(false)
                          }}
                          className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                            theme === 'system'
                              ? 'bg-[#0099D9]/10 text-[#38BDF8]'
                              : 'text-[#CBD5E1] hover:text-white hover:bg-[#334155]'
                          }`}
                        >
                          <Monitor className="w-4 h-4" />
                          <span>Sistema</span>
                        </button>
                      </div>
                    </div>
                  </>
                )}
              </div>

              {/* User Menu */}
              {userLoading ? (
                <div className="flex items-center gap-2">
                  <div className="w-8 h-8 rounded bg-[#1B3A4B] animate-pulse" />
                  <div className="w-20 h-4 rounded bg-[#1B3A4B] animate-pulse" />
                </div>
              ) : currentUser ? (
                <div className="relative">
                  <button
                    onClick={() => setUserMenuOpen(!userMenuOpen)}
                    className="flex items-center gap-3 pl-3 pr-2 py-1.5 rounded-md bg-[#1B3A4B]/50 border border-[#334155] hover:border-[#0099D9]/50 transition-colors group"
                  >
                    {/* Avatar */}
                    <div className="w-7 h-7 rounded bg-gradient-to-br from-[#0099D9] to-[#007BB5] flex items-center justify-center text-xs font-bold text-white shadow-lg shadow-[#0099D9]/20">
                      {getInitials(currentUser.username)}
                    </div>

                    {/* User info */}
                    <div className="flex flex-col items-start">
                      <span className="text-sm font-medium text-white leading-none">
                        {currentUser.username}
                      </span>
                      <span className={`text-[10px] font-mono uppercase tracking-wide mt-0.5 px-1.5 py-0.5 rounded border ${getRoleStyle(currentUser.role)}`}>
                        {currentUser.role}
                      </span>
                    </div>

                    <ChevronDown className={`w-4 h-4 text-[#64748B] transition-transform ${userMenuOpen ? 'rotate-180' : ''}`} />
                  </button>

                  {/* Dropdown Menu */}
                  {userMenuOpen && (
                    <>
                      {/* Backdrop */}
                      <div
                        className="fixed inset-0 z-10"
                        onClick={() => setUserMenuOpen(false)}
                      />

                      {/* Menu */}
                      <div className="absolute right-0 top-full mt-2 w-56 rounded-lg bg-[#1B3A4B] border border-[#334155] shadow-xl shadow-black/50 z-20 overflow-hidden">
                        {/* User header */}
                        <div className="px-4 py-3 border-b border-[#334155] bg-[#0D1B2A]/50">
                          <div className="flex items-center gap-3">
                            <div className="w-10 h-10 rounded-md bg-gradient-to-br from-[#0099D9] to-[#007BB5] flex items-center justify-center text-sm font-bold text-white">
                              {getInitials(currentUser.username)}
                            </div>
                            <div>
                              <div className="text-sm font-medium text-white">
                                {currentUser.username}
                              </div>
                              <div className="flex items-center gap-1 mt-0.5">
                                <Shield className="w-3 h-3 text-[#0099D9]" />
                                <span className="text-xs text-[#94A3B8] capitalize">
                                  {currentUser.role}
                                </span>
                              </div>
                            </div>
                          </div>
                        </div>

                        {/* Menu items */}
                        <div className="p-2 space-y-1">
                          <button
                            onClick={openPasswordModal}
                            className="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-sm text-[#CBD5E1] hover:text-white hover:bg-[#334155] transition-colors group"
                          >
                            <Key className="w-4 h-4 text-[#64748B] group-hover:text-[#0099D9] transition-colors" />
                            <span className="group-hover:text-[#38BDF8] transition-colors">
                              Cambiar Contraseña
                            </span>
                          </button>
                          <button
                            onClick={handleLogout}
                            disabled={isLoggingOut}
                            className="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-sm text-[#CBD5E1] hover:text-white hover:bg-red-500/10 transition-colors group"
                          >
                            <LogOut className="w-4 h-4 text-[#64748B] group-hover:text-red-400 transition-colors" />
                            <span className="group-hover:text-red-400 transition-colors">
                              {isLoggingOut ? 'Cerrando...' : 'Cerrar Sesión'}
                            </span>
                          </button>
                        </div>
                      </div>
                    </>
                  )}
                </div>
              ) : (
                /* Not logged in state */
                <Link
                  to="/login"
                  className="flex items-center gap-2 px-4 py-2 rounded-md bg-[#0099D9]/10 border border-[#0099D9]/30 text-[#38BDF8] hover:bg-[#0099D9]/20 transition-colors text-sm font-medium"
                >
                  <Shield className="w-4 h-4" />
                  <span>Iniciar Sesión</span>
                </Link>
              )}
            </div>
          </div>
        </header>

        {/* Page Content */}
        <main className="p-6">
          <div className="bg-[var(--color-bg-primary)] rounded-2xl p-6 min-h-[calc(100vh-8rem)] transition-colors shadow-sm border border-[var(--color-border-primary)]">
            {children}
          </div>
        </main>
      </div>

      {/* Change Password Modal */}
      {showPasswordModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          {/* Backdrop */}
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setShowPasswordModal(false)}
          />

          {/* Modal */}
          <div className="relative w-full max-w-md mx-4 bg-[#1B3A4B] border border-[#334155] rounded-xl shadow-2xl">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-[#334155]">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-lg bg-[#0099D9]/10 flex items-center justify-center">
                  <Key className="w-5 h-5 text-[#38BDF8]" />
                </div>
                <div>
                  <h2 className="text-lg font-semibold text-white">Cambiar Contraseña</h2>
                  <p className="text-xs text-[#94A3B8]">Actualiza tu contraseña de acceso</p>
                </div>
              </div>
              <button
                onClick={() => setShowPasswordModal(false)}
                className="p-2 rounded-lg text-[#94A3B8] hover:text-white hover:bg-[#334155] transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Body */}
            <div className="p-6 space-y-4">
              {/* Error message */}
              {passwordError && (
                <div className="flex items-center gap-2 px-4 py-3 rounded-lg bg-red-500/10 border border-red-500/30">
                  <AlertCircle className="w-5 h-5 text-red-400 flex-shrink-0" />
                  <span className="text-sm text-red-400">{passwordError}</span>
                </div>
              )}

              {/* Success message */}
              {passwordSuccess && (
                <div className="flex items-center gap-2 px-4 py-3 rounded-lg bg-emerald-500/10 border border-emerald-500/30">
                  <CheckCircle className="w-5 h-5 text-emerald-400 flex-shrink-0" />
                  <span className="text-sm text-emerald-400">{passwordSuccess}</span>
                </div>
              )}

              {/* Current Password */}
              <div>
                <label className="block text-sm font-medium text-[#CBD5E1] mb-2">
                  Contraseña Actual
                </label>
                <div className="relative">
                  <input
                    type={showCurrentPassword ? 'text' : 'password'}
                    value={currentPassword}
                    onChange={(e) => setCurrentPassword(e.target.value)}
                    className="w-full px-4 py-3 pr-12 bg-[#0D1B2A] border border-[#334155] rounded-lg text-white placeholder-[#64748B] focus:outline-none focus:border-[#0099D9]/50 focus:ring-1 focus:ring-[#0099D9]/50 transition-colors"
                    placeholder="Ingresa tu contraseña actual"
                  />
                  <button
                    type="button"
                    onClick={() => setShowCurrentPassword(!showCurrentPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-[#64748B] hover:text-[#94A3B8] transition-colors"
                  >
                    {showCurrentPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                  </button>
                </div>
              </div>

              {/* New Password */}
              <div>
                <label className="block text-sm font-medium text-[#CBD5E1] mb-2">
                  Nueva Contraseña
                </label>
                <div className="relative">
                  <input
                    type={showNewPassword ? 'text' : 'password'}
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    className="w-full px-4 py-3 pr-12 bg-[#0D1B2A] border border-[#334155] rounded-lg text-white placeholder-[#64748B] focus:outline-none focus:border-[#0099D9]/50 focus:ring-1 focus:ring-[#0099D9]/50 transition-colors"
                    placeholder="Mínimo 6 caracteres"
                  />
                  <button
                    type="button"
                    onClick={() => setShowNewPassword(!showNewPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-[#64748B] hover:text-[#94A3B8] transition-colors"
                  >
                    {showNewPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                  </button>
                </div>
              </div>

              {/* Confirm Password */}
              <div>
                <label className="block text-sm font-medium text-[#CBD5E1] mb-2">
                  Confirmar Nueva Contraseña
                </label>
                <input
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  className="w-full px-4 py-3 bg-[#0D1B2A] border border-[#334155] rounded-lg text-white placeholder-[#64748B] focus:outline-none focus:border-[#0099D9]/50 focus:ring-1 focus:ring-[#0099D9]/50 transition-colors"
                  placeholder="Repite la nueva contraseña"
                />
              </div>
            </div>

            {/* Footer */}
            <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-[#334155] bg-[#0D1B2A]/50 rounded-b-xl">
              <button
                onClick={() => setShowPasswordModal(false)}
                className="px-4 py-2.5 rounded-lg text-sm font-medium text-[#CBD5E1] hover:text-white hover:bg-[#334155] transition-colors"
              >
                Cancelar
              </button>
              <button
                onClick={handleChangePassword}
                disabled={changePasswordMutation.isPending}
                className="px-4 py-2.5 rounded-lg text-sm font-medium bg-[#0099D9] hover:bg-[#007BB5] text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                {changePasswordMutation.isPending ? (
                  <>
                    <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                    Guardando...
                  </>
                ) : (
                  'Guardar Cambios'
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
