import axios from 'axios'
import type {
  Account,
  Plan,
  Zone,
  RateCard,
  CDR,
  ActiveCall,
  Reservation,
  DashboardStats,
  PaginatedResponse,
  CDRFilters,
  DialplanRoute,
} from '../types'

const api = axios.create({
  baseURL: '/api/v1',
  headers: {
    'Content-Type': 'application/json',
  },
  withCredentials: true, // For JWT cookie authentication
})

// Response interceptor to handle token expiration
api.interceptors.response.use(
  (response) => response,
  (error) => {
    // Check if error is 401 Unauthorized (token expired or invalid)
    if (error.response?.status === 401) {
      // Avoid infinite loop - don't redirect if already on login page
      const currentPath = window.location.pathname
      if (!currentPath.includes('/login') && !currentPath.includes('/auth/login')) {
        console.warn('Session expired. Redirecting to login...')

        // Clear any stored state if needed
        localStorage.removeItem('user')

        // Force redirect to login page
        window.location.href = '/login'
      }
    }
    return Promise.reject(error)
  }
)

// Health check
export const checkHealth = async (): Promise<{ status: string; version: string }> => {
  const { data } = await api.get('/health')
  return data
}

// Dashboard Stats
export const fetchStats = async (): Promise<DashboardStats> => {
  const { data } = await api.get('/stats')
  return data
}

// ============== ACCOUNTS ==============

export const fetchAccounts = async (): Promise<Account[]> => {
  const { data } = await api.get('/accounts')
  // Handle paginated response from Rust backend
  return data.data || data
}

export const fetchAccount = async (id: number): Promise<Account> => {
  const { data } = await api.get(`/accounts/${id}`)
  return data.data || data
}

export const createAccount = async (account: Partial<Account>): Promise<Account> => {
  const { data } = await api.post('/accounts', account)
  return data.data || data
}

export const updateAccount = async (id: number, account: Partial<Account>): Promise<Account> => {
  const { data } = await api.put(`/accounts/${id}`, account)
  return data.data || data
}

export const deleteAccount = async (id: number): Promise<void> => {
  await api.delete(`/accounts/${id}`)
}

export const topupAccount = async (
  id: number,
  amount: number,
  reason?: string
): Promise<{ previous_balance: number; amount: number; new_balance: number }> => {
  const { data } = await api.post(`/accounts/${id}/topup`, { amount, reason })
  return data.data || data
}

// ============== PLANS ==============

export const fetchPlans = async (): Promise<Plan[]> => {
  const { data } = await api.get('/plans')
  return data.data || data
}

export const fetchActivePlans = async (): Promise<Plan[]> => {
  const { data } = await api.get('/plans/active')
  return data.data || data
}

export const fetchPlan = async (id: number): Promise<Plan> => {
  const { data } = await api.get(`/plans/${id}`)
  return data.data || data
}

export const createPlan = async (plan: Partial<Plan>): Promise<Plan> => {
  const { data } = await api.post('/plans', plan)
  return data.data || data
}

export const updatePlan = async (id: number, plan: Partial<Plan>): Promise<Plan> => {
  const { data} = await api.put(`/plans/${id}`, plan)
  return data.data || data
}

export const deletePlan = async (id: number): Promise<void> => {
  await api.delete(`/plans/${id}`)
}

// ============== ZONES (Management) ==============

export const fetchZones = async (): Promise<Zone[]> => {
  const { data } = await api.get('/zonas')
  return data.data || data
}

export const createZone = async (zone: Partial<Zone>): Promise<Zone> => {
  const { data } = await api.post('/zonas', zone)
  return data.data || data
}

export const updateZone = async (id: number, zone: Partial<Zone>): Promise<Zone> => {
  const { data } = await api.put(`/zonas/${id}`, zone)
  return data.data || data
}

export const deleteZone = async (id: number): Promise<void> => {
  await api.delete(`/zonas/${id}`)
}

// ============== RATE CARDS ==============

export const fetchRateCards = async (): Promise<RateCard[]> => {
  const { data } = await api.get('/rate-cards')
  return data.data || data
}

export const fetchRateCard = async (id: number): Promise<RateCard> => {
  const { data } = await api.get(`/rate-cards/${id}`)
  return data.data || data
}

export const createRateCard = async (rate: Partial<RateCard>): Promise<RateCard> => {
  const { data } = await api.post('/rate-cards', rate)
  return data.data || data
}

export const updateRateCard = async (id: number, rate: Partial<RateCard>): Promise<RateCard> => {
  const { data } = await api.put(`/rate-cards/${id}`, rate)
  return data.data || data
}

export const deleteRateCard = async (id: number): Promise<void> => {
  await api.delete(`/rate-cards/${id}`)
}

// Rate lookup (LPM - Longest Prefix Match)
export const lookupRate = async (destination: string): Promise<RateCard | null> => {
  try {
    const { data } = await api.get(`/rate-cards/search/${destination}`)
    return data.data || data
  } catch {
    return null
  }
}

// ============== CDRs (Call Detail Records) ==============

export const fetchCDRs = async (
  filters?: CDRFilters,
  page = 1,
  perPage = 50
): Promise<PaginatedResponse<CDR>> => {
  const params = new URLSearchParams()
  params.append('page', page.toString())
  params.append('per_page', perPage.toString())

  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        params.append(key, value.toString())
      }
    })
  }

  const { data } = await api.get(`/cdrs?${params.toString()}`)

  // Transform API response format to expected format
  // API returns: { data: [...], pagination: { total, page, per_page, total_pages } }
  // Frontend expects: { data: [...], total, page, per_page, total_pages }
  if (data.pagination) {
    return {
      data: data.data,
      total: data.pagination.total,
      page: data.pagination.page,
      per_page: data.pagination.per_page,
      total_pages: data.pagination.total_pages,
    }
  }
  return data
}

export const fetchCDR = async (id: number): Promise<CDR> => {
  const { data } = await api.get(`/cdrs/${id}`)
  return data.data || data
}

export const exportCDRs = async (filters?: CDRFilters, format: 'csv' | 'json' = 'csv'): Promise<Blob> => {
  const params = new URLSearchParams()
  params.append('format', format)

  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        params.append(key, value.toString())
      }
    })
  }

  const { data } = await api.get(`/cdrs/export?${params.toString()}`, {
    responseType: 'blob',
  })
  return data
}

export const fetchCDRStats = async (filters?: CDRFilters): Promise<{
  total_cdrs: number
  total_minutes: number
  total_cost: number
  avg_duration: number
}> => {
  const params = new URLSearchParams()
  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        params.append(key, value.toString())
      }
    })
  }

  const { data } = await api.get(`/cdrs/stats?${params.toString()}`)
  return data
}

// ============== ACTIVE CALLS ==============

export const fetchActiveCalls = async (): Promise<ActiveCall[]> => {
  const { data } = await api.get('/active-calls')
  return data.data || data
}

export const createActiveCall = async (call: Partial<ActiveCall>): Promise<ActiveCall> => {
  const { data } = await api.post('/active-calls', call)
  return data.data || data
}

export const deleteActiveCall = async (callId: string): Promise<void> => {
  await api.delete(`/active-calls/${callId}`)
}

// ============== RESERVATIONS ==============

export const fetchReservations = async (status?: string): Promise<Reservation[]> => {
  const params = status ? `?status=${status}` : ''
  const { data } = await api.get(`/reservations${params}`)
  return data
}

export const fetchActiveReservations = async (): Promise<Reservation[]> => {
  const { data } = await api.get('/reservations/active')
  return data
}

// ============== AUTHENTICATION ==============

export interface LoginCredentials {
  username: string
  password: string
}

export interface AuthUser {
  id: number
  username: string
  role: string
}

export const login = async (credentials: LoginCredentials): Promise<AuthUser> => {
  const { data } = await api.post('/auth/login', credentials)
  // API returns { data: { access_token, user: {...}, ... } }
  // Return just the user object to match getCurrentUser format
  return data.data?.user || data.user || data.data || data
}

export const logout = async (): Promise<void> => {
  await api.post('/auth/logout')
}

export const getCurrentUser = async (): Promise<AuthUser | null> => {
  try {
    const { data } = await api.get('/auth/me')
    // API returns { data: { user: {...}, token_expires_at: ... } }
    return data.data?.user || data.user || data.data || data
  } catch {
    return null
  }
}

// ============== MANAGEMENT (Prefixes, Tariffs) ==============

export interface Prefix {
  id: number
  zone_id: number
  prefix: string
  description?: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface Tariff {
  id: number
  zone_id: number
  rate_name?: string
  rate_per_minute: number
  rate_per_call: number
  billing_increment: number
  minimum_seconds: number
  priority: number
  effective_from: string
  effective_until?: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export const fetchPrefixes = async (zoneId?: number): Promise<Prefix[]> => {
  const params = zoneId ? `?zone_id=${zoneId}` : ''
  const { data } = await api.get(`/prefijos${params}`)
  return data.data || data
}

export const createPrefix = async (prefix: Partial<Prefix>): Promise<Prefix> => {
  const { data } = await api.post('/prefijos', prefix)
  return data.data || data
}

export const deletePrefix = async (id: number): Promise<void> => {
  await api.delete(`/prefijos/${id}`)
}

export const fetchTariffs = async (zoneId?: number): Promise<Tariff[]> => {
  const params = zoneId ? `?zone_id=${zoneId}` : ''
  const { data } = await api.get(`/tarifas${params}`)
  return data.data || data
}

export const createTariff = async (tariff: Partial<Tariff>): Promise<Tariff> => {
  const { data } = await api.post('/tarifas', tariff)
  return data.data || data
}

export const updateTariff = async (id: number, tariff: Partial<Tariff>): Promise<Tariff> => {
  const { data } = await api.put(`/tarifas/${id}`, tariff)
  return data.data || data
}

export const deleteTariff = async (id: number): Promise<void> => {
  await api.delete(`/tarifas/${id}`)
}

// Sync rate cards from zones/prefixes/tariffs
export const syncRateCards = async (): Promise<{ synced_count: number }> => {
  const { data } = await api.post('/sync-rate-cards')
  return data.data || data
}

// ============== STATISTICS ==============

export interface HourlyCallStats {
  hour: number
  hour_label: string
  call_count: number
  total_duration: number
  total_revenue: number
}

export interface DailyRevenueStats {
  date: string
  day_of_week: number
  day_label: string
  call_count: number
  revenue: number
  total_minutes: number
}

export interface BalanceTrendPoint {
  date: string
  day: number
  total_balance: number
  active_accounts: number
  average_balance: number
}

export const fetchCallsByHour = async (): Promise<HourlyCallStats[]> => {
  const { data } = await api.get('/stats/calls-by-hour')
  // Backend returns { data: { data: [...] } }
  return data.data?.data || data.data || data
}

export const fetchRevenueByDay = async (): Promise<DailyRevenueStats[]> => {
  const { data } = await api.get('/stats/revenue-by-day')
  // Backend returns { data: { data: [...] } }
  return data.data?.data || data.data || data
}

export const fetchBalanceTrend = async (): Promise<BalanceTrendPoint[]> => {
  const { data } = await api.get('/stats/balance-trend')
  // Backend returns { data: { data: [...] } }
  return data.data?.data || data.data || data
}

export interface CallTypeStats {
  call_type: string
  label: string
  call_count: number
  total_duration: number
  total_cost: number
  percentage: number
}

export interface ZoneStats {
  zone_id?: number
  zone_name: string
  call_count: number
  total_duration: number
  total_cost: number
  percentage: number
}

export const fetchCallsByType = async (): Promise<CallTypeStats[]> => {
  const { data } = await api.get('/stats/calls-by-type')
  // Backend returns { data: { data: [...] } }
  return data.data?.data || data.data || data
}

export const fetchCallsByZone = async (): Promise<ZoneStats[]> => {
  const { data } = await api.get('/stats/calls-by-zone')
  // Backend returns { data: { data: [...] } }
  return data.data?.data || data.data || data
}

export interface TrafficStats {
  direction: string
  label: string
  total_calls: number
  total_minutes: number
  total_revenue: number
  avg_duration: number
}

export interface TrafficByDirection {
  inbound: TrafficStats
  outbound: TrafficStats
}

export const fetchTrafficByDirection = async (): Promise<TrafficByDirection> => {
  const { data } = await api.get('/stats/traffic-by-direction')
  // Backend returns { data: { inbound: {...}, outbound: {...} } }
  return data.data || data
}

// ============== USER MANAGEMENT (Superadmin only) ==============

import type { User } from '../types'

export interface UserCreateRequest {
  username: string
  password: string
  nombre?: string
  apellido?: string
  email?: string
  role: string
}

export interface UserUpdateRequest {
  nombre?: string
  apellido?: string
  email?: string
  role?: string
  activo?: boolean
}

export const fetchUsers = async (page = 1, per_page = 50) => {
  const { data } = await api.get(`/users?page=${page}&per_page=${per_page}`)
  return data.data || data
}

export const fetchUser = async (id: number): Promise<User> => {
  const { data } = await api.get(`/users/${id}`)
  return data.data || data
}

export const createUser = async (userData: UserCreateRequest): Promise<User> => {
  const { data } = await api.post('/users', userData)
  return data.data || data
}

export const updateUser = async (id: number, userData: UserUpdateRequest): Promise<User> => {
  const { data } = await api.put(`/users/${id}`, userData)
  return data.data || data
}

export const deleteUser = async (id: number): Promise<void> => {
  await api.delete(`/users/${id}`)
}

// ============== AUDIT LOGS (Superadmin only) ==============

export interface AuditLogFilters {
  username?: string
  action?: string
  entity_type?: string
  entity_id?: string
  start_date?: string
  end_date?: string
  page?: number
  per_page?: number
}

export const fetchAuditLogs = async (filters: AuditLogFilters) => {
  const params = new URLSearchParams()
  Object.entries(filters).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== '') {
      params.append(key, String(value))
    }
  })
  const { data } = await api.get(`/audit-logs?${params.toString()}`)
  return data.data || data
}

export const fetchAuditStats = async () => {
  const { data } = await api.get('/audit-logs/stats')
  return data.data || data
}

// ============== DIALPLAN (FreeSWITCH Configuration) ==============

export const fetchDialplanRoutes = async (context: string): Promise<DialplanRoute[]> => {
  const { data } = await api.get(`/dialplan/${context}`)
  return data.data || data
}

export const createDialplanRoute = async (context: string, route: Partial<DialplanRoute>): Promise<DialplanRoute> => {
  const { data } = await api.post(`/dialplan/${context}`, route)
  return data.data || data
}

export const updateDialplanRoute = async (context: string, id: string, route: Partial<DialplanRoute>): Promise<DialplanRoute> => {
  const { data } = await api.put(`/dialplan/${context}/${id}`, route)
  return data.data || data
}

export const deleteDialplanRoute = async (context: string, id: string): Promise<void> => {
  await api.delete(`/dialplan/${context}/${id}`)
}

export const reloadDialplan = async (): Promise<{ message: string }> => {
  const { data } = await api.post('/dialplan/reload')
  return data.data || data
}

// ============== SYSTEM SETTINGS ==============

export interface SystemSetting {
  key: string
  value: string
  description: string | null
  updated_at: string | null
}

export const fetchSettings = async (): Promise<SystemSetting[]> => {
  const { data } = await api.get('/settings')
  return data.data || data
}

export const updateSetting = async (key: string, value: string): Promise<SystemSetting> => {
  const { data } = await api.put(`/settings/${key}`, { value })
  return data.data || data
}

export default api
