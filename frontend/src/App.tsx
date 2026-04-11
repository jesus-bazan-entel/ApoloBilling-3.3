import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import { ThemeProvider } from './contexts/ThemeContext'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import ActiveCalls from './pages/ActiveCalls'
import CDR from './pages/CDR'
import Accounts from './pages/Accounts'
import Balance from './pages/Balance'
import Zones from './pages/Zones'
import Rates from './pages/Rates'
import Plans from './pages/Plans'
import Users from './pages/Users'
import AuditLogs from './pages/AuditLogs'
import UnifiedRouting from './pages/UnifiedRouting'
import InternalRouting from './pages/InternalRouting'
import SipDevices from './pages/SipDevices'
import Login from './pages/Login'
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
      <div className="min-h-screen bg-[#0a0f1a] flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <div className="w-8 h-8 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-500 text-sm">Cargando...</span>
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
      <div className="min-h-screen bg-[#0a0f1a] flex items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-bold text-red-400 mb-2">Acceso Denegado</h2>
          <p className="text-slate-500 mb-4">No tienes permisos para acceder a esta sección.</p>
          <button
            onClick={() => window.history.back()}
            className="px-4 py-2 bg-cyan-500 text-white rounded hover:bg-cyan-600"
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
                <Route path="/calls" element={<ActiveCalls />} />
                <Route path="/cdr" element={<CDR />} />
                <Route path="/accounts" element={<Accounts />} />
                <Route path="/plans" element={<Plans />} />
                <Route path="/balance" element={<Balance />} />
                <Route path="/zones" element={<Zones />} />
                <Route path="/rates" element={<Rates />} />
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
                  path="/internal-routing"
                  element={
                    <ProtectedRoute requiredRole="superadmin">
                      <InternalRouting />
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
