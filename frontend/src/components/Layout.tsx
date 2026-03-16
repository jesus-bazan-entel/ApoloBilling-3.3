import { Link, useLocation, useNavigate } from 'react-router-dom'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { checkHealth, getCurrentUser, logout } from '../api/client'
import { useState } from 'react'
import {
  LayoutDashboard,
  Users,
  FileText,
  Phone,
  DollarSign,
  Settings,
  Globe,
  Activity,
  LogOut,
  ChevronDown,
  Zap,
  Shield,
  UserCog,
  FileSearch,
  Network,
} from 'lucide-react'

const baseNavigation = [
  { name: 'Panel de Control', href: '/', icon: LayoutDashboard },
  { name: 'Llamadas Activas', href: '/calls', icon: Phone },
  { name: 'Registros CDR', href: '/cdr', icon: FileText },
  { name: 'Cuentas', href: '/accounts', icon: Users },
  { name: 'Planes', href: '/plans', icon: FileText },
  { name: 'Saldos', href: '/balance', icon: DollarSign },
  { name: 'Zonas', href: '/zones', icon: Globe },
  { name: 'Tarifas', href: '/rates', icon: Settings },
]

const superadminNavigation = [
  { name: 'Gestión de Usuarios', href: '/users', icon: UserCog },
  { name: 'Auditoría de Acciones', href: '/audit-logs', icon: FileSearch },
  { name: 'Dialplan FreeSWITCH', href: '/dialplan', icon: Network },
]

interface LayoutProps {
  children: React.ReactNode
}

export default function Layout({ children }: LayoutProps) {
  const location = useLocation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [userMenuOpen, setUserMenuOpen] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)

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

  // Build navigation based on user role
  const navigation = currentUser?.role === 'superadmin'
    ? [...baseNavigation, ...superadminNavigation]
    : baseNavigation

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
        return 'bg-slate-500/20 text-slate-400 border-slate-500/30'
    }
  }

  return (
    <div className="min-h-screen bg-[#0a0f1a]">
      {/* Ambient background effect */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-0 left-64 right-0 h-px bg-gradient-to-r from-cyan-500/50 via-transparent to-amber-500/50" />
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-cyan-900/10 via-transparent to-transparent" />
      </div>

      {/* Sidebar */}
      <aside className="fixed inset-y-0 left-0 w-64 bg-[#0d1321] border-r border-slate-800/50 z-20">
        {/* Logo */}
        <div className="flex items-center h-16 px-5 border-b border-slate-800/50">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded bg-gradient-to-br from-cyan-500 to-blue-600 flex items-center justify-center">
              <Zap className="w-4 h-4 text-white" />
            </div>
            <div className="flex flex-col">
              <span className="text-base font-bold tracking-tight text-white leading-none">
                APOLO
              </span>
              <span className="text-[10px] font-medium tracking-[0.2em] text-cyan-500 uppercase">
                Billing
              </span>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav className="p-3 space-y-0.5">
          {navigation.map((item) => {
            const isActive = location.pathname === item.href
            const Icon = item.icon

            return (
              <Link
                key={item.name}
                to={item.href}
                className={`
                  group flex items-center px-3 py-2.5 rounded-md text-sm font-medium
                  transition-all duration-150 relative overflow-hidden
                  ${isActive
                    ? 'bg-cyan-500/10 text-cyan-400'
                    : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                  }
                `}
              >
                {/* Active indicator */}
                {isActive && (
                  <div className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-5 bg-cyan-500 rounded-r" />
                )}
                <Icon className={`w-4 h-4 mr-3 transition-colors ${isActive ? 'text-cyan-400' : 'text-slate-500 group-hover:text-slate-300'}`} />
                <span>{item.name}</span>
                {isActive && (
                  <div className="ml-auto w-1.5 h-1.5 rounded-full bg-cyan-500 animate-pulse" />
                )}
              </Link>
            )
          })}
        </nav>

        {/* System Status */}
        <div className="absolute bottom-0 left-0 right-0 p-4 border-t border-slate-800/50 bg-[#0a0f1a]/80 backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Activity className="w-3.5 h-3.5 text-slate-500" />
              <span className="text-xs font-medium text-slate-500 uppercase tracking-wide">
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
        <header className="sticky top-0 z-10 h-14 bg-[#0d1321]/90 backdrop-blur-md border-b border-slate-800/50">
          <div className="flex items-center justify-between h-full px-6">
            {/* Left: Page context or breadcrumb could go here */}
            <div className="flex items-center gap-2">
              <div className="h-1 w-1 rounded-full bg-cyan-500/50" />
              <span className="text-xs font-mono text-slate-500 uppercase tracking-wider">
                {navigation.find(n => n.href === location.pathname)?.name || 'Dashboard'}
              </span>
            </div>

            {/* Right: User section */}
            <div className="flex items-center gap-4">
              {/* System time - adds industrial feel */}
              <div className="hidden sm:flex items-center gap-2 text-slate-500">
                <div className="h-4 w-px bg-slate-700" />
                <span className="text-xs font-mono tabular-nums">
                  {new Date().toLocaleTimeString('es-ES', { hour: '2-digit', minute: '2-digit' })}
                </span>
              </div>

              {/* User Menu */}
              {userLoading ? (
                <div className="flex items-center gap-2">
                  <div className="w-8 h-8 rounded bg-slate-800 animate-pulse" />
                  <div className="w-20 h-4 rounded bg-slate-800 animate-pulse" />
                </div>
              ) : currentUser ? (
                <div className="relative">
                  <button
                    onClick={() => setUserMenuOpen(!userMenuOpen)}
                    className="flex items-center gap-3 pl-3 pr-2 py-1.5 rounded-md bg-slate-800/50 border border-slate-700/50 hover:border-slate-600/50 transition-colors group"
                  >
                    {/* Avatar */}
                    <div className="w-7 h-7 rounded bg-gradient-to-br from-amber-500 to-orange-600 flex items-center justify-center text-xs font-bold text-white shadow-lg shadow-amber-500/20">
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

                    <ChevronDown className={`w-4 h-4 text-slate-500 transition-transform ${userMenuOpen ? 'rotate-180' : ''}`} />
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
                      <div className="absolute right-0 top-full mt-2 w-56 rounded-lg bg-[#151d2e] border border-slate-700/50 shadow-xl shadow-black/50 z-20 overflow-hidden">
                        {/* User header */}
                        <div className="px-4 py-3 border-b border-slate-700/50 bg-slate-800/30">
                          <div className="flex items-center gap-3">
                            <div className="w-10 h-10 rounded-md bg-gradient-to-br from-amber-500 to-orange-600 flex items-center justify-center text-sm font-bold text-white">
                              {getInitials(currentUser.username)}
                            </div>
                            <div>
                              <div className="text-sm font-medium text-white">
                                {currentUser.username}
                              </div>
                              <div className="flex items-center gap-1 mt-0.5">
                                <Shield className="w-3 h-3 text-amber-500" />
                                <span className="text-xs text-slate-400 capitalize">
                                  {currentUser.role}
                                </span>
                              </div>
                            </div>
                          </div>
                        </div>

                        {/* Menu items */}
                        <div className="p-2">
                          <button
                            onClick={handleLogout}
                            disabled={isLoggingOut}
                            className="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-sm text-slate-300 hover:text-white hover:bg-red-500/10 transition-colors group"
                          >
                            <LogOut className="w-4 h-4 text-slate-500 group-hover:text-red-400 transition-colors" />
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
                  className="flex items-center gap-2 px-4 py-2 rounded-md bg-cyan-500/10 border border-cyan-500/30 text-cyan-400 hover:bg-cyan-500/20 transition-colors text-sm font-medium"
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
          <div className="bg-slate-100 rounded-xl p-6 min-h-[calc(100vh-8rem)]">
            {children}
          </div>
        </main>
      </div>
    </div>
  )
}
