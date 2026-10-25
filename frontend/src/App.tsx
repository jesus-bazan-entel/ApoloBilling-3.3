import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import { ThemeProvider } from './contexts/ThemeContext'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import ActiveCalls from './pages/ActiveCalls'
import CDR from './pages/CDR'
import Accounts from './pages/Accounts'
import Users from './pages/Users'
import AuditLogs from './pages/AuditLogs'
import UnifiedRouting from './pages/UnifiedRouting'
import SipDevices from './pages/SipDevices'
import Provisioning from './pages/Provisioning'
import Login from './pages/Login'
import MonitoringDashboard from './pages/Monitoring/MonitoringDashboard'
import { getCurrentUser } from './api/client'
import type { UserRole } from './types'

// Create a client
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
      staleTime: 5000,
    },
  },
})

// Protected route wrapper with optional role check
interface ProtectedRouteProps {
  children: React.ReactNode
  requiredRole?: UserRole
}

function ProtectedRoute({ children, requiredRole }: ProtectedRouteProps) {
  const { data: user, isLoading } = useQuery({
    queryKey: ['currentUser'],
    queryFn: getCurrentUser,
    retry: false,
    staleTime: 60000,
  })

  if (isLoading) {
    return (
      <div className="min-h-screen bg-[#160d0a] flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <div className="w-8 h-8 border-2 border-[#ff5a0a] border-t-transparent rounded-full animate-spin" />
          <span className="text-[#b49a8b] text-sm">Cargando...</span>
        </div>
      </div>
    )
  }

  if (!user) {
    return <Navigate to="/login" replace />
  }

  // Check role permission if required
  if (requiredRole && user.role !== requiredRole) {
    return (
      <div className="min-h-screen bg-[#160d0a] flex items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-bold text-red-400 mb-2">Acceso Denegado</h2>
          <p className="text-[#b49a8b] mb-4">No tienes permisos para acceder a esta sección.</p>
          <button
            onClick={() => window.history.back()}
            className="px-4 py-2 bg-[#ff5a0a] text-white rounded hover:bg-[#d8450b]"
          >
            Volver
          </button>
        </div>
      </div>
    )
  }

  return <>{children}</>
}

function AppRoutes() {
  return (
    <Routes>
      {/* Public route - Login */}
      <Route path="/login" element={<Login />} />

      {/* Protected routes with Layout */}
      <Route
        path="/*"
        element={
          <ProtectedRoute>
            <Layout>
              <Routes>
                <Route path="/" element={<Dashboard />} />
                <Route path="/monitoring" element={<MonitoringDashboard />} />
                <Route path="/calls" element={<ActiveCalls />} />
                <Route path="/cdr" element={<CDR />} />
                <Route path="/accounts" element={<Accounts />} />
                {/* Superadmin only routes */}
                <Route
                  path="/users"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <Users />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/audit-logs"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <AuditLogs />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/routing"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <UnifiedRouting />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/sip-devices"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <SipDevices />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/provisioning"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <Provisioning />
                    </ProtectedRoute>
                  }
                />
                <Route path="*" element={<Navigate to="/" replace />} />
              </Routes>
            </Layout>
          </ProtectedRoute>
        }
      />
    </Routes>
  )
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <BrowserRouter>
          <AppRoutes />
        </BrowserRouter>
      </ThemeProvider>
    </QueryClientProvider>
  )
}

export default App
