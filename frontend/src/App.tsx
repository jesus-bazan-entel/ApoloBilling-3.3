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
import MinuteBuckets from './pages/MinuteBuckets'
import Users from './pages/Users'
import AuditLogs from './pages/AuditLogs'
import UnifiedRouting from './pages/UnifiedRouting'
import InternalRouting from './pages/InternalRouting'
import SipDevices from './pages/SipDevices'
import InboundRoutingPage from './pages/InboundRouting'
import EndpointMonitor from './pages/EndpointMonitor'
// New simplified routing pages
import Carriers from './pages/Carriers'
import Endpoints from './pages/Endpoints'
import OutboundRouting from './pages/OutboundRouting'
import CarrierGroups from './pages/CarrierGroups'
import Login from './pages/Login'
import { getCurrentUser } from './api/client'
import { MODULES, hasModule, type ModuleKey } from './constants/modules'

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

// Protected route wrapper with an optional module gate. `requiredModule`
// mirrors the server's `has_module` check: Admin/Superadmin always pass,
// a Usuario session needs the module explicitly granted. This is UX only —
// the real enforcement happens on the backend for every request.
interface ProtectedRouteProps {
  children: React.ReactNode
  requiredModule?: ModuleKey
}

function ProtectedRoute({ children, requiredModule }: ProtectedRouteProps) {
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

  if (requiredModule && !hasModule(user, requiredModule)) {
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
                {/* Always visible to any authenticated user (dashboard = always_visible module) */}
                <Route path="/" element={<Dashboard />} />
                <Route
                  path="/calls"
                  element={
                    <ProtectedRoute requiredModule={MODULES.ACTIVE_CALLS}>
                      <ActiveCalls />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/cdr"
                  element={
                    <ProtectedRoute requiredModule={MODULES.CDR}>
                      <CDR />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/accounts"
                  element={
                    <ProtectedRoute requiredModule={MODULES.ACCOUNTS}>
                      <Accounts />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/plans"
                  element={
                    <ProtectedRoute requiredModule={MODULES.PLANS}>
                      <Plans />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/balance"
                  element={
                    <ProtectedRoute requiredModule={MODULES.BALANCE}>
                      <Balance />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/minute-buckets"
                  element={
                    <ProtectedRoute requiredModule={MODULES.MINUTE_BUCKETS}>
                      <MinuteBuckets />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/zones"
                  element={
                    <ProtectedRoute requiredModule={MODULES.ZONES}>
                      <Zones />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/rates"
                  element={
                    <ProtectedRoute requiredModule={MODULES.RATES}>
                      <Rates />
                    </ProtectedRoute>
                  }
                />
                {/* Sistema - Admin+ only, never assignable to a Usuario */}
                <Route
                  path="/users"
                  element={
                    <ProtectedRoute requiredModule={MODULES.USERS}>
                      <Users />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/audit-logs"
                  element={
                    <ProtectedRoute requiredModule={MODULES.AUDIT_LOGS}>
                      <AuditLogs />
                    </ProtectedRoute>
                  }
                />
                {/* Legacy routing - exclusive to factory support (Superadmin) */}
                <Route
                  path="/routing"
                  element={
                    <ProtectedRoute requiredModule={MODULES.LEGACY_ROUTING}>
                      <UnifiedRouting />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/internal-routing"
                  element={
                    <ProtectedRoute requiredModule={MODULES.LEGACY_ROUTING}>
                      <InternalRouting />
                    </ProtectedRoute>
                  }
                />
                {/* Operación - full access for Admin/Superadmin, grantable to a Usuario */}
                <Route
                  path="/sip-devices"
                  element={
                    <ProtectedRoute requiredModule={MODULES.SIP_DEVICES}>
                      <SipDevices />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/inbound-routing"
                  element={
                    <ProtectedRoute requiredModule={MODULES.INBOUND_ROUTING}>
                      <InboundRoutingPage />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/endpoint-monitor"
                  element={
                    <ProtectedRoute requiredModule={MODULES.ENDPOINT_MONITOR}>
                      <EndpointMonitor />
                    </ProtectedRoute>
                  }
                />
                {/* New simplified routing pages */}
                <Route
                  path="/carriers"
                  element={
                    <ProtectedRoute requiredModule={MODULES.CARRIERS}>
                      <Carriers />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/endpoints"
                  element={
                    <ProtectedRoute requiredModule={MODULES.ENDPOINTS}>
                      <Endpoints />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/outbound-routing"
                  element={
                    <ProtectedRoute requiredModule={MODULES.OUTBOUND_ROUTING}>
                      <OutboundRouting />
                    </ProtectedRoute>
                  }
                />
                <Route
                  path="/carrier-groups"
                  element={
                    <ProtectedRoute requiredModule={MODULES.CARRIER_GROUPS}>
                      <CarrierGroups />
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
