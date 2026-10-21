import axios from 'axios'
import type {
  Account,
  Plan,
  Zone,
  RateCard,
  CDR,
  ActiveCall,
  DashboardStats,
  PaginatedResponse,
  CDRFilters,
  DialplanRoute,
  BucketConfig,
  PlanBucketConfig,
  AccountBucketConfig,
  MinuteBucketStatus,
  MinuteBucketAddition,
  CycleHistoryEntry,
  MinuteBucketReportRow,
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

// ============== MINUTE BUCKET BILLING (Plan Control / Plan Abierto) ==============

export const getPlanBucketConfig = async (planId: number): Promise<PlanBucketConfig> => {
  const { data } = await api.get(`/plans/${planId}/bucket-config`)
  return data.data || data
}

export const updatePlanBucketConfig = async (
  planId: number,
  config: BucketConfig
): Promise<PlanBucketConfig> => {
  const { data } = await api.put(`/plans/${planId}/bucket-config`, config)
  return data.data || data
}

export const getAccountBucketConfig = async (accountId: number): Promise<AccountBucketConfig> => {
  const { data } = await api.get(`/accounts/${accountId}/bucket-config`)
  return data.data || data
}

export const updateAccountBucketConfig = async (
  accountId: number,
  config: BucketConfig & { notification_email?: string | null }
): Promise<AccountBucketConfig> => {
  const { data } = await api.put(`/accounts/${accountId}/bucket-config`, config)
  return data.data || data
}

export const getMinuteBucketStatus = async (accountId: number): Promise<MinuteBucketStatus> => {
  const { data } = await api.get(`/accounts/${accountId}/minute-bucket`)
  return data.data || data
}

export const updateMinuteBucketCycle = async (
  accountId: number,
  includedMinutes: number,
  reason: string
): Promise<MinuteBucketStatus> => {
  const { data } = await api.put(`/accounts/${accountId}/minute-bucket`, {
    included_minutes: includedMinutes,
    reason,
  })
  return data.data || data
}

export const topupMinuteBucket = async (
  accountId: number,
  minutesAdded: number,
  reason?: string
): Promise<MinuteBucketAddition> => {
  const { data } = await api.post(`/accounts/${accountId}/minute-bucket/topup`, {
    minutes_added: minutesAdded,
    reason,
  })
  return data.data || data
}

export const getMinuteBucketHistory = async (accountId: number): Promise<CycleHistoryEntry[]> => {
  const { data } = await api.get(`/accounts/${accountId}/minute-bucket/history`)
  return data.data || data
}

export interface MinuteBucketReportFilters {
  month?: string // "YYYY-MM"
  account_id?: number
  bucket_type?: string
}

export const getMinuteBucketReport = async (
  filters?: MinuteBucketReportFilters
): Promise<MinuteBucketReportRow[]> => {
  const params = new URLSearchParams()
  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        params.append(key, value.toString())
      }
    })
  }
  const { data } = await api.get(`/reports/minute-bucket?${params.toString()}`)
  return data.data || data
}

export const exportMinuteBucketReport = async (
  filters?: MinuteBucketReportFilters
): Promise<Blob> => {
  const params = new URLSearchParams()
  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        params.append(key, value.toString())
      }
    })
  }
  const { data } = await api.get(`/reports/minute-bucket/export?${params.toString()}`, {
    responseType: 'blob',
  })
  return data
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

// Distinct tariff groups in use (e.g. "standard", "overage_fibertel") — used both
// by the Rate Cards page and the minute-bucket overage-tariff-group selector.
export const getRateCardGroups = async (): Promise<string[]> => {
  const { data } = await api.get('/rate-cards/card-groups')
  return data.data || data
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

// ============== AUTHENTICATION ==============

export interface LoginCredentials {
  username: string
  password: string
}

export interface AuthUser {
  id: number
  username: string
  role: string
  /** Effective module keys for this session — drives menu + route guards */
  modules: string[]
}

export const login = async (credentials: LoginCredentials): Promise<AuthUser> => {
  const { data } = await api.post('/auth/login', credentials)
  // API returns { data: { access_token, user: {...}, modules: [...], ... } }
  const payload = data.data || data
  const user = payload.user || payload
  return { ...user, modules: payload.modules || [] }
}

export const logout = async (): Promise<void> => {
  await api.post('/auth/logout')
}

export const getCurrentUser = async (): Promise<AuthUser | null> => {
  try {
    const { data } = await api.get('/auth/me')
    // API returns { data: { user: {...}, modules: [...], token_expires_at: ... } }
    const payload = data.data || data
    const user = payload.user || payload
    return { ...user, modules: payload.modules || [] }
  } catch {
    return null
  }
}

export interface ChangePasswordRequest {
  current_password: string
  new_password: string
}

export const changePassword = async (request: ChangePasswordRequest): Promise<{ message: string }> => {
  const { data } = await api.post('/auth/change-password', request)
  return data.data || data
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

export interface BucketTrendPoint {
  date: string
  day: number
  total_overage_cost: number
  total_minutes_consumed: number
  active_accounts: number
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

export const fetchBucketTrend = async (): Promise<BucketTrendPoint[]> => {
  const { data } = await api.get('/stats/bucket-trend')
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

// ============== USER MANAGEMENT (Admin+, hierarchy enforced server-side) ==============

import type { SystemModule, User, UserModulesResponse } from '../types'

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

// ============== MODULE VISIBILITY (Admin+) ==============
// Catalog of assignable modules, and per-user (role=usuario) grants.

export const fetchModuleCatalog = async (): Promise<SystemModule[]> => {
  const { data } = await api.get('/modules')
  return data.data || data
}

export const fetchUserModules = async (id: number): Promise<UserModulesResponse> => {
  const { data } = await api.get(`/users/${id}/modules`)
  return data.data || data
}

export const updateUserModules = async (
  id: number,
  modules: string[]
): Promise<UserModulesResponse> => {
  const { data } = await api.put(`/users/${id}/modules`, { modules })
  return data.data || data
}

// ============== AUDIT LOGS (Admin+) ==============

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

// ============== KAMAILIO DIALPLAN ==============

export interface KamailioCarrierGroup {
  id: number
  gwlist: string
  description: string | null
}

export interface KamailioCarrier {
  id?: number
  gwid: number
  gw_type: number
  address: string
  strip: number
  pri_prefix: string
  attrs: string
  description: string
}

export interface KamailioOutboundRoute {
  id?: number
  ruleid: number
  groupid: string
  prefix: string
  timerec: string
  priority: number
  routeid: string
  gwlist: string
  description: string
}

// Carrier Groups
export const fetchKamailioCarrierGroups = async (): Promise<KamailioCarrierGroup[]> => {
  try {
    const { data } = await api.get('/kamailio-dialplan/groups')
    const groups = data.data || []
    return groups
  } catch (error) {
    console.error('Error fetching carrier groups:', error)
    throw error
  }
}

export const fetchKamailioCarrierGroup = async (id: number): Promise<KamailioCarrierGroup> => {
  const { data } = await api.get(`/kamailio-dialplan/groups/${id}`)
  return data.data || data
}

export const createKamailioCarrierGroup = async (group: { description: string; gwlist?: string }): Promise<KamailioCarrierGroup> => {
  const { data } = await api.post('/kamailio-dialplan/groups', group)
  return data.data || data
}

export const updateKamailioCarrierGroup = async (id: number, group: { description: string; gwlist?: string }): Promise<KamailioCarrierGroup> => {
  const { data } = await api.put(`/kamailio-dialplan/groups/${id}`, group)
  return data.data || data
}

export const deleteKamailioCarrierGroup = async (id: number): Promise<void> => {
  await api.delete(`/kamailio-dialplan/groups/${id}`)
}

// Carriers (Gateways)
export const fetchKamailioCarriers = async (): Promise<KamailioCarrier[]> => {
  try {
    const { data } = await api.get('/kamailio-dialplan/carriers')
    const carriers = data.data || []
    // Add id field for DataTable compatibility
    return carriers.map((c: KamailioCarrier) => ({ ...c, id: c.gwid }))
  } catch (error) {
    console.error('Error fetching carriers:', error)
    throw error
  }
}

export const fetchKamailioCarrier = async (id: number): Promise<KamailioCarrier> => {
  const { data } = await api.get(`/kamailio-dialplan/carriers/${id}`)
  return data.data || data
}

export const createKamailioCarrier = async (carrier: Partial<KamailioCarrier>): Promise<KamailioCarrier> => {
  const { data } = await api.post('/kamailio-dialplan/carriers', carrier)
  return data.data || data
}

export const updateKamailioCarrier = async (id: number, carrier: Partial<KamailioCarrier>): Promise<KamailioCarrier> => {
  const { data } = await api.put(`/kamailio-dialplan/carriers/${id}`, carrier)
  return data.data || data
}

export const deleteKamailioCarrier = async (id: number): Promise<void> => {
  await api.delete(`/kamailio-dialplan/carriers/${id}`)
}

// PBX Groups (same-owner failover sets)
export const fetchPbxGroups = async (): Promise<PbxGroupWithMembers[]> => {
  const { data } = await api.get('/pbx-groups')
  return data.data || data
}

export const createPbxGroup = async (group: CreatePbxGroupRequest): Promise<PbxGroupWithMembers> => {
  const { data } = await api.post('/pbx-groups', group)
  return data.data || data
}

export const updatePbxGroup = async (id: string, group: UpdatePbxGroupRequest): Promise<PbxGroupWithMembers> => {
  const { data } = await api.put(`/pbx-groups/${id}`, group)
  return data.data || data
}

export const deletePbxGroup = async (id: string): Promise<void> => {
  await api.delete(`/pbx-groups/${id}`)
}

export const addPbxGroupMember = async (groupId: string, member: AddPbxGroupMemberRequest): Promise<void> => {
  await api.post(`/pbx-groups/${groupId}/members`, member)
}

export const updatePbxGroupMember = async (groupId: string, memberId: string, priority: number): Promise<void> => {
  await api.put(`/pbx-groups/${groupId}/members/${memberId}`, { priority })
}

export const removePbxGroupMember = async (groupId: string, memberId: string): Promise<void> => {
  await api.delete(`/pbx-groups/${groupId}/members/${memberId}`)
}

// Outbound Routes
export const fetchKamailioOutboundRoutes = async (): Promise<KamailioOutboundRoute[]> => {
  try {
    const { data } = await api.get('/kamailio-dialplan/routes')
    const routes = data.data || []
    // Add id field for DataTable compatibility
    return routes.map((r: KamailioOutboundRoute) => ({ ...r, id: r.ruleid }))
  } catch (error) {
    console.error('Error fetching outbound routes:', error)
    throw error
  }
}

export const fetchKamailioOutboundRoute = async (id: number): Promise<KamailioOutboundRoute> => {
  const { data } = await api.get(`/kamailio-dialplan/routes/${id}`)
  return data.data || data
}

export const createKamailioOutboundRoute = async (route: Partial<KamailioOutboundRoute>): Promise<KamailioOutboundRoute> => {
  const { data } = await api.post('/kamailio-dialplan/routes', route)
  return data.data || data
}

export const updateKamailioOutboundRoute = async (id: number, route: Partial<KamailioOutboundRoute>): Promise<KamailioOutboundRoute> => {
  const { data } = await api.put(`/kamailio-dialplan/routes/${id}`, route)
  return data.data || data
}

export const deleteKamailioOutboundRoute = async (id: number): Promise<void> => {
  await api.delete(`/kamailio-dialplan/routes/${id}`)
}

// Reload Kamailio configuration
export const reloadKamailio = async (): Promise<{ message: string }> => {
  const { data } = await api.post('/kamailio-dialplan/reload')
  return data.data || data
}

// ============== SIP ENDPOINTS (unified extensions + trunks) ==============

import type {
  SipEndpoint,
  SipEndpointWithPassword,
  SipEndpointCreateRequest,
  SipEndpointUpdateRequest,
  FreeswitchAllowedIp,
  PbxGroupWithMembers,
  CreatePbxGroupRequest,
  UpdatePbxGroupRequest,
  AddPbxGroupMemberRequest,
} from '../types'

export const fetchSipEndpoints = async (accountId?: number): Promise<SipEndpoint[]> => {
  const accountParam = accountId ? `&account_id=${accountId}` : ''
  const perPage = 1000 // backend max for per_page

  const all: SipEndpoint[] = []
  let page = 1
  let totalPages = 1

  do {
    const { data } = await api.get(
      `/sip-endpoints?page=${page}&per_page=${perPage}${accountParam}`
    )
    all.push(...(data.data || data))
    totalPages = data.pagination?.total_pages ?? 1
    page += 1
  } while (page <= totalPages)

  return all
}

export const fetchSipEndpoint = async (id: string): Promise<SipEndpoint> => {
  const { data } = await api.get(`/sip-endpoints/${id}`)
  return data.data || data
}

export const createSipEndpoint = async (
  endpoint: SipEndpointCreateRequest
): Promise<SipEndpointWithPassword | SipEndpoint> => {
  const { data } = await api.post('/sip-endpoints', endpoint)
  return data.data || data
}

export const updateSipEndpoint = async (
  id: string,
  endpoint: SipEndpointUpdateRequest
): Promise<SipEndpoint> => {
  const { data } = await api.put(`/sip-endpoints/${id}`, endpoint)
  return data.data || data
}

export const deleteSipEndpoint = async (id: string): Promise<void> => {
  await api.delete(`/sip-endpoints/${id}`)
}

export const getSipEndpointPassword = async (id: string): Promise<SipEndpointWithPassword> => {
  const { data } = await api.get(`/sip-endpoints/${id}/password`)
  return data.data || data
}

export const regenerateSipEndpointPassword = async (id: string): Promise<{
  id: string
  sip_username: string
  sip_domain: string
  new_password: string
}> => {
  const { data } = await api.post(`/sip-endpoints/${id}/regenerate-password`)
  return data.data || data
}

export const syncSipEndpoint = async (id: string): Promise<SipEndpoint> => {
  const { data } = await api.post(`/sip-endpoints/${id}/sync`)
  return data.data || data
}

export const syncAllSipEndpoints = async (): Promise<{
  total: number
  succeeded: number
  failed: number
  results: Array<{
    endpoint_id: string
    endpoint_name: string
    kamailio_gwid?: number
    success: boolean
    error?: string
  }>
}> => {
  const { data } = await api.post('/sip-endpoints/sync-all')
  return data.data || data
}

// SIP Registration Status (auth_mode = credentials only)
export interface SipRegistrationStatus {
  registered: boolean
  sip_username: string
  sip_domain: string
  user_agent?: string
  contact?: string
  status?: string
  ip?: string
  port?: number
  ping_status?: string
  expires_seconds?: number
  expires_at?: string
}

export const getSipEndpointRegistrationStatus = async (id: string): Promise<SipRegistrationStatus> => {
  const { data } = await api.get(`/sip-endpoints/${id}/registration-status`)
  return data.data || data
}

// FreeSWITCH Allowed IPs
export const fetchFreeswitchAllowedIps = async (): Promise<FreeswitchAllowedIp[]> => {
  const { data } = await api.get('/sip-endpoints/allowed-ips')
  return data.data || data
}

export const createFreeswitchAllowedIp = async (ip: {
  ip_address: string
  description?: string
  enabled?: boolean
}): Promise<FreeswitchAllowedIp> => {
  const { data } = await api.post('/sip-endpoints/allowed-ips', ip)
  return data.data || data
}

export const deleteFreeswitchAllowedIp = async (id: number): Promise<void> => {
  await api.delete(`/sip-endpoints/allowed-ips/${id}`)
}

// ============== UNIFIED ROUTING ==============

import type {
  RoutingTrunk,
  RoutingTrunkGroup,
  RoutingTrunkGroupWithMembers,
  RoutingOutboundRoute,
  RoutingInboundRoute,
  RoutingSyncStatus,
  RoutingReloadResult,
  RoutingMigrationResult,
  SyncToKamailioResult,
  CreateTrunkRequest,
  UpdateTrunkRequest,
  CreateTrunkGroupRequest,
  UpdateTrunkGroupRequest,
  CreateOutboundRouteRequest,
  UpdateOutboundRouteRequest,
  CreateRoutingInboundRouteRequest,
  UpdateRoutingInboundRouteRequest,
  SipStatusCheck,
  BulkSipStatusResult,
  SipStatusLogEntry,
} from '../types'

// Trunks
export const fetchRoutingTrunks = async (): Promise<RoutingTrunk[]> => {
  const { data } = await api.get('/routing/trunks')
  return data.data || data
}

export const fetchRoutingTrunk = async (id: string): Promise<RoutingTrunk> => {
  const { data } = await api.get(`/routing/trunks/${id}`)
  return data.data || data
}

export const createRoutingTrunk = async (trunk: CreateTrunkRequest): Promise<RoutingTrunk> => {
  const { data } = await api.post('/routing/trunks', trunk)
  return data.data || data
}

export const updateRoutingTrunk = async (id: string, trunk: UpdateTrunkRequest): Promise<RoutingTrunk> => {
  const { data } = await api.put(`/routing/trunks/${id}`, trunk)
  return data.data || data
}

export const deleteRoutingTrunk = async (id: string): Promise<void> => {
  await api.delete(`/routing/trunks/${id}`)
}

// Trunk Groups
export const fetchRoutingTrunkGroups = async (): Promise<RoutingTrunkGroup[]> => {
  const { data } = await api.get('/routing/trunk-groups')
  return data.data || data
}

export const fetchRoutingTrunkGroup = async (id: string): Promise<RoutingTrunkGroupWithMembers> => {
  const { data } = await api.get(`/routing/trunk-groups/${id}`)
  return data.data || data
}

export const createRoutingTrunkGroup = async (group: CreateTrunkGroupRequest): Promise<RoutingTrunkGroup> => {
  const { data } = await api.post('/routing/trunk-groups', group)
  return data.data || data
}

export const updateRoutingTrunkGroup = async (id: string, group: UpdateTrunkGroupRequest): Promise<RoutingTrunkGroup> => {
  const { data } = await api.put(`/routing/trunk-groups/${id}`, group)
  return data.data || data
}

export const deleteRoutingTrunkGroup = async (id: string): Promise<void> => {
  await api.delete(`/routing/trunk-groups/${id}`)
}

// Outbound Routes
export const fetchRoutingOutboundRoutes = async (): Promise<RoutingOutboundRoute[]> => {
  const { data } = await api.get('/routing/outbound')
  return data.data || data
}

export const fetchRoutingOutboundRoute = async (id: string): Promise<RoutingOutboundRoute> => {
  const { data } = await api.get(`/routing/outbound/${id}`)
  return data.data || data
}

export const createRoutingOutboundRoute = async (route: CreateOutboundRouteRequest): Promise<RoutingOutboundRoute> => {
  const { data } = await api.post('/routing/outbound', route)
  return data.data || data
}

export const updateRoutingOutboundRoute = async (id: string, route: UpdateOutboundRouteRequest): Promise<RoutingOutboundRoute> => {
  const { data } = await api.put(`/routing/outbound/${id}`, route)
  return data.data || data
}

export const deleteRoutingOutboundRoute = async (id: string): Promise<void> => {
  await api.delete(`/routing/outbound/${id}`)
}

// Inbound Routes
export const fetchRoutingInboundRoutes = async (): Promise<RoutingInboundRoute[]> => {
  const { data } = await api.get('/routing/inbound')
  return data.data || data
}

export const fetchRoutingInboundRoute = async (id: string): Promise<RoutingInboundRoute> => {
  const { data } = await api.get(`/routing/inbound/${id}`)
  return data.data || data
}

export const createRoutingInboundRoute = async (route: CreateRoutingInboundRouteRequest): Promise<RoutingInboundRoute> => {
  const { data } = await api.post('/routing/inbound', route)
  return data.data || data
}

export const updateRoutingInboundRoute = async (id: string, route: UpdateRoutingInboundRouteRequest): Promise<RoutingInboundRoute> => {
  const { data } = await api.put(`/routing/inbound/${id}`, route)
  return data.data || data
}

export const deleteRoutingInboundRoute = async (id: string): Promise<void> => {
  await api.delete(`/routing/inbound/${id}`)
}

// System operations
export const fetchRoutingSyncStatus = async (): Promise<RoutingSyncStatus> => {
  const { data } = await api.get('/routing/sync-status')
  return data.data || data
}

export const reloadRouting = async (): Promise<RoutingReloadResult> => {
  const { data } = await api.post('/routing/reload')
  return data.data || data
}

export const migrateRouting = async (): Promise<RoutingMigrationResult> => {
  const { data } = await api.post('/routing/migrate')
  return data.data || data
}

// Sync pending routes to Kamailio
export const syncToKamailio = async (): Promise<SyncToKamailioResult> => {
  const { data } = await api.post('/routing/sync-to-kamailio')
  return data.data || data
}

// Force re-sync all routes to Kamailio
export const forceSyncToKamailio = async (): Promise<SyncToKamailioResult> => {
  const { data } = await api.post('/routing/force-sync-kamailio')
  return data.data || data
}

// SIP Status Monitoring
export const checkAllTrunksSipStatus = async (): Promise<BulkSipStatusResult> => {
  const { data } = await api.get('/routing/sip-status')
  return data.data || data
}

export const checkTrunkSipStatus = async (id: string): Promise<SipStatusCheck> => {
  const { data } = await api.get(`/routing/sip-status/${id}`)
  return data.data || data
}

export const getTrunkSipHistory = async (
  id: string,
  limit = 20
): Promise<SipStatusLogEntry[]> => {
  const { data } = await api.get(`/routing/sip-status/${id}/history?limit=${limit}`)
  return data.data || data
}

// ============== INBOUND ROUTING (Simplified) ==============

import type {
  InboundRoute,
  InboundGatewayOption,
  CreateInboundRouteRequest,
  UpdateInboundRouteRequestNew,
  // Deprecated types kept for backward compatibility
  Carrier,
  CarrierSummary,
  DidOverride,
  CreateCarrierRequest,
  UpdateCarrierRequest,
  CreateDidOverrideRequest,
  UpdateDidOverrideRequest,
} from '../types'

// Inbound Routes (new simplified API)
export const fetchInboundRoutes = async (): Promise<InboundRoute[]> => {
  const { data } = await api.get('/inbound-routing')
  return data.data?.data || data.data || data
}

export const fetchInboundRoute = async (id: string): Promise<InboundRoute> => {
  const { data } = await api.get(`/inbound-routing/${id}`)
  return data.data || data
}

// Destination trunks (PBX) for the route destination dropdown:
// SIP devices with endpoint_type = 'trunk'
export const fetchInboundDestinations = async (): Promise<InboundGatewayOption[]> => {
  const { data } = await api.get('/inbound-routing/destinations')
  return data.data || data
}

export const createInboundRoute = async (
  route: CreateInboundRouteRequest
): Promise<InboundRoute> => {
  const { data } = await api.post('/inbound-routing', route)
  return data.data || data
}

export const updateInboundRoute = async (
  id: string,
  route: UpdateInboundRouteRequestNew
): Promise<InboundRoute> => {
  const { data } = await api.put(`/inbound-routing/${id}`, route)
  return data.data || data
}

export const deleteInboundRoute = async (id: string): Promise<void> => {
  await api.delete(`/inbound-routing/${id}`)
}

export const syncInboundRoutes = async (): Promise<{ message: string }> => {
  const { data } = await api.post('/inbound-routing/sync')
  return data.data || data
}

// DEPRECATED: Old Carrier API - use fetchInboundRoutes instead
export const fetchCarriers = async (): Promise<Carrier[]> => {
  const { data } = await api.get('/inbound-routing')
  return data.data?.data || data.data || data
}

export const fetchCarrierSummaries = async (): Promise<CarrierSummary[]> => {
  const { data } = await api.get('/inbound-routing/summary')
  return data.data || data
}

export const fetchCarrier = async (id: string): Promise<Carrier> => {
  const { data } = await api.get(`/inbound-routing/${id}`)
  return data.data || data
}

export const createCarrier = async (carrier: CreateCarrierRequest): Promise<Carrier> => {
  const { data } = await api.post('/inbound-routing', carrier)
  return data.data || data
}

export const updateCarrier = async (
  id: string,
  carrier: UpdateCarrierRequest
): Promise<Carrier> => {
  const { data } = await api.put(`/inbound-routing/${id}`, carrier)
  return data.data || data
}

export const deleteCarrier = async (id: string): Promise<void> => {
  await api.delete(`/inbound-routing/${id}`)
}

export const syncCarriers = async (): Promise<{ message: string }> => {
  const { data } = await api.post('/inbound-routing/sync')
  return data.data || data
}

// DEPRECATED: DID Overrides - use fetchInboundRoutes instead
export const fetchDidOverrides = async (): Promise<DidOverride[]> => {
  const { data } = await api.get('/inbound-routing')
  return data.data?.data || data.data || data
}

export const fetchCarrierDids = async (carrierId: string): Promise<DidOverride[]> => {
  const { data } = await api.get(`/inbound-routing/${carrierId}/dids`)
  return data.data || data
}

export const createDidOverride = async (
  didOverride: CreateDidOverrideRequest
): Promise<{ id: string }> => {
  const { data } = await api.post('/inbound-routing', didOverride)
  return data.data || data
}

export const updateDidOverride = async (
  id: string,
  didOverride: UpdateDidOverrideRequest
): Promise<void> => {
  await api.put(`/inbound-routing/${id}`, didOverride)
}

export const deleteDidOverride = async (id: string): Promise<void> => {
  await api.delete(`/inbound-routing/${id}`)
}

// ============== ENDPOINT MONITOR ==============

import type {
  EndpointStatusResponse,
  ProbeResultsResponse,
  ProbeResultsQuery,
  EvidenceReportQuery,
} from '../types'

// Get latest endpoint status (dashboard cards)
export const fetchEndpointStatus = async (): Promise<EndpointStatusResponse> => {
  const { data } = await api.get('/monitor/endpoints')
  return data.data || data
}

// Get probe results with downsampled series and raw rows
export const fetchProbeResults = async (
  query: ProbeResultsQuery
): Promise<ProbeResultsResponse> => {
  const params = new URLSearchParams()
  if (query.name) params.append('name', query.name)
  if (query.ip) params.append('ip', query.ip)
  if (query.frm) params.append('frm', query.frm.toString())
  if (query.to) params.append('to', query.to.toString())
  if (query.limit) params.append('limit', query.limit.toString())

  const { data } = await api.get(`/monitor/results?${params.toString()}`)
  return data.data || data
}

// Download evidence trace for a single probe
export const downloadEvidence = async (probeId: number): Promise<Blob> => {
  const { data } = await api.get(`/monitor/evidence?id=${probeId}`, {
    responseType: 'blob',
  })
  return data
}

// Download consolidated evidence report
export const downloadEvidenceReport = async (
  query: EvidenceReportQuery
): Promise<Blob> => {
  const params = new URLSearchParams()
  if (query.name) params.append('name', query.name)
  if (query.ip) params.append('ip', query.ip)
  if (query.frm) params.append('frm', query.frm.toString())
  if (query.to) params.append('to', query.to.toString())
  if (query.full) params.append('full', '1')

  const { data } = await api.get(`/monitor/evidence_report?${params.toString()}`, {
    responseType: 'blob',
  })
  return data
}

// ============== SIMPLIFIED OUTBOUND ROUTING (dSIPRouter model) ==============

import type {
  SipCarrier,
  SipCarrierListResponse,
  CreateSipCarrierRequest,
  UpdateSipCarrierRequest,
  CarrierSipCheckResult,
  BulkCarrierSyncResult,
  CarrierGroupWithMembers,
  CarrierGroupListResponse,
  CreateCarrierGroupRequest,
  UpdateCarrierGroupRequestNew,
  AddCarrierGroupMemberRequest,
  UpdateCarrierGroupMemberRequest,
  OutboundRouteNew,
  OutboundRouteListResponse,
  CreateOutboundRouteRequestNew,
  UpdateOutboundRouteRequestNew,
  OutboundSyncAllResult,
} from '../types'

// ============== CARRIERS ==============

export const fetchSipCarriers = async (): Promise<SipCarrierListResponse> => {
  const { data } = await api.get('/carriers')
  return data.data || data
}

export const fetchSipCarrier = async (id: string): Promise<SipCarrier> => {
  const { data } = await api.get(`/carriers/${id}`)
  return data.data || data
}

export const createSipCarrier = async (
  carrier: CreateSipCarrierRequest
): Promise<SipCarrier> => {
  const { data } = await api.post('/carriers', carrier)
  return data.data || data
}

export const updateSipCarrier = async (
  id: string,
  carrier: UpdateSipCarrierRequest
): Promise<SipCarrier> => {
  const { data } = await api.put(`/carriers/${id}`, carrier)
  return data.data || data
}

export const deleteSipCarrier = async (id: string): Promise<void> => {
  await api.delete(`/carriers/${id}`)
}

export const checkSipCarrierStatus = async (
  id: string
): Promise<CarrierSipCheckResult> => {
  const { data } = await api.post(`/carriers/${id}/check-status`)
  return data.data || data
}

export const syncAllSipCarriers = async (): Promise<BulkCarrierSyncResult> => {
  const { data } = await api.post('/carriers/sync')
  return data.data || data
}

// ============== CARRIER GROUPS ==============

export const fetchCarrierGroups = async (): Promise<CarrierGroupListResponse> => {
  const { data } = await api.get('/carrier-groups')
  return data.data || data
}

export const fetchCarrierGroup = async (
  id: string
): Promise<CarrierGroupWithMembers> => {
  const { data } = await api.get(`/carrier-groups/${id}`)
  return data.data || data
}

export const createCarrierGroup = async (
  group: CreateCarrierGroupRequest
): Promise<CarrierGroupWithMembers> => {
  const { data } = await api.post('/carrier-groups', group)
  return data.data || data
}

export const updateCarrierGroup = async (
  id: string,
  group: UpdateCarrierGroupRequestNew
): Promise<CarrierGroupWithMembers> => {
  const { data } = await api.put(`/carrier-groups/${id}`, group)
  return data.data || data
}

export const deleteCarrierGroup = async (id: string): Promise<void> => {
  await api.delete(`/carrier-groups/${id}`)
}

export const addCarrierGroupMember = async (
  groupId: string,
  member: AddCarrierGroupMemberRequest
): Promise<CarrierGroupWithMembers> => {
  const { data } = await api.post(`/carrier-groups/${groupId}/members`, member)
  return data.data || data
}

export const updateCarrierGroupMember = async (
  groupId: string,
  memberId: string,
  member: UpdateCarrierGroupMemberRequest
): Promise<CarrierGroupWithMembers> => {
  const { data } = await api.put(
    `/carrier-groups/${groupId}/members/${memberId}`,
    member
  )
  return data.data || data
}

export const removeCarrierGroupMember = async (
  groupId: string,
  memberId: string
): Promise<void> => {
  await api.delete(`/carrier-groups/${groupId}/members/${memberId}`)
}

// ============== OUTBOUND ROUTES (NEW) ==============

export const fetchOutboundRoutes = async (): Promise<OutboundRouteListResponse> => {
  const { data } = await api.get('/outbound-routing')
  return data.data || data
}

export const fetchOutboundRoute = async (id: string): Promise<OutboundRouteNew> => {
  const { data } = await api.get(`/outbound-routing/${id}`)
  return data.data || data
}

export const createOutboundRoute = async (
  route: CreateOutboundRouteRequestNew
): Promise<OutboundRouteNew> => {
  const { data } = await api.post('/outbound-routing', route)
  return data.data || data
}

export const updateOutboundRoute = async (
  id: string,
  route: UpdateOutboundRouteRequestNew
): Promise<OutboundRouteNew> => {
  const { data } = await api.put(`/outbound-routing/${id}`, route)
  return data.data || data
}

export const deleteOutboundRoute = async (id: string): Promise<void> => {
  await api.delete(`/outbound-routing/${id}`)
}

export const syncAllOutboundRoutes = async (): Promise<{ message: string }> => {
  const { data } = await api.post('/outbound-routing/sync')
  return data.data || data
}

// ============== KAMAILIO SYNC ==============

export const syncAllToKamailio = async (): Promise<OutboundSyncAllResult> => {
  const { data } = await api.post('/kamailio/sync-all')
  return data.data || data
}

export default api
