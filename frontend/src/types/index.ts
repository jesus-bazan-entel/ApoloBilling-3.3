// Account types based on PRD
export type AccountType = 'prepaid' | 'postpaid' | 'PREPAID' | 'POSTPAID'
export type AccountStatus = 'active' | 'suspended' | 'closed' | 'ACTIVE' | 'SUSPENDED' | 'CLOSED'

// User types
export type UserRole = 'superadmin' | 'admin' | 'operator'

export interface User {
  id: number
  username: string
  nombre?: string
  apellido?: string
  email?: string
  role: UserRole
  activo: boolean
  ultimo_login?: string
  created_at: string
  updated_at: string
}

// Audit log types
export interface AuditLog {
  id: number
  user_id?: number
  username: string
  action: string
  entity_type: string
  entity_id?: string
  details?: any
  ip_address?: string
  user_agent?: string
  created_at: string
}

export interface Account {
  id: number
  account_number: string
  account_name?: string
  customer_phone?: string
  account_type: AccountType
  balance: number
  credit_limit: number
  currency?: string
  status: AccountStatus
  max_concurrent_calls: number
  available_balance?: number
  plan_id?: number              // Plan used to create this account
  consumed_credit?: number      // Postpago: crédito consumido (backend)
  utilization_percent?: number  // Postpago: % de utilización (backend)
  created_at: string
  updated_at: string
}

// Plan types
export interface Plan {
  id: number
  plan_name: string
  plan_code: string
  account_type: AccountType
  initial_balance: number
  credit_limit: number
  max_concurrent_calls: number
  description?: string
  enabled: boolean
  created_at: string
  updated_at: string
  created_by: string
}

// Account display information (for UI transformation)
export interface AccountDisplayInfo {
  currentBalance?: number       // Prepaid: saldo actual
  consumedCredit?: number       // Postpago: crédito consumido
  creditLimit?: number          // Postpago: límite total
  availableCredit?: number      // Postpago: disponible para gastar
  utilizationPercent?: number   // Postpago: 0-100%
  displayValue: string          // Texto formateado para mostrar
  isPrepaid: boolean
  isLowBalance: boolean         // Alerta de saldo bajo
  balanceColor: 'success' | 'warning' | 'error'  // Color según nivel
}

// Zone types
export interface Zone {
  id: number
  zone_name: string
  zone_code?: string
  zone_type?: string
  network_type?: string
  region_name?: string
  description?: string
  enabled?: boolean
  created_at?: string
  updated_at?: string
}

export interface RateCard {
  id: number
  rate_name?: string
  destination_prefix: string
  destination_name: string
  rate_per_minute: number
  billing_increment: number
  connection_fee: number
  effective_start: string
  effective_end: string | null
  priority: number
  is_effective?: boolean
  rate_per_second?: number
  created_at?: string
  updated_at?: string
}

// CDR (Call Detail Record) - based on PRD schema
export interface CDR {
  id: number
  call_uuid: string
  caller: string
  callee: string
  caller_number?: string // Alias for compatibility
  callee_number?: string // Alias for compatibility
  start_time: string
  answer_time?: string
  end_time?: string
  duration: number
  billsec: number
  total_cost?: number
  cost?: number
  rate?: number
  hangup_cause?: string
  direction: 'inbound' | 'outbound' | 'internal' | string
  destination?: string
  destination_prefix?: string
  rate_per_minute?: number
  zone_id?: number
  zone_name?: string
  account_id?: number
  answered?: boolean
}

// Active Call for real-time monitoring
export interface ActiveCall {
  id?: string | number
  call_uuid: string
  uuid?: string // Alias
  caller_number: string
  callee_number: string
  direction: string
  start_time: string
  answer_time?: string
  duration_seconds?: number
  duration?: number
  status: 'dialing' | 'answered' | 'ringing' | 'active'
  account_id?: number
  destination_prefix?: string
  rate_per_minute?: number
  reserved_amount?: number
  zone_name?: string
  estimated_cost?: number
  remaining_duration?: number
}

// Balance Reservation
export interface Reservation {
  id: string | number
  account_id: number
  call_uuid: string
  reserved_amount: number
  consumed_amount: number
  released_amount: number
  status: string
  reservation_type: string
  destination_prefix?: string
  rate_per_minute: number
  reserved_minutes?: number
  created_at: string
  updated_at?: string
  expires_at: string
}

// Balance Transaction for audit
export interface BalanceTransaction {
  id: number
  account_id: number
  transaction_type: 'recharge' | 'deduction' | 'adjustment' | 'refund'
  amount: number
  balance_before: number
  balance_after: number
  description: string
  created_at: string
  created_by: string
}

// Dashboard Statistics - matches Rust backend DashboardStats
export interface DashboardStats {
  total_accounts: number
  active_accounts: number
  total_balance: number
  active_calls: number
  active_reservations: number
  cdrs_today: number
  revenue_today: number
  minutes_today: number
  // Legacy fields for compatibility
  reserved_amount?: number
  calls_today?: number
  calls_this_month?: number
  revenue_this_month?: number
}

// Authorization Response from Rust engine
export interface AuthResponse {
  authorized: boolean
  reason: string
  uuid: string
  account_id?: number
  account_number?: string
  reservation_id?: string
  reserved_amount?: number
  max_duration_seconds?: number
  rate_per_minute?: number
}

// WebSocket Message Types
export interface WSMessage {
  type: 'active_calls' | 'call_start' | 'call_update' | 'call_end' | 'stats_update' | 'pong' | 'error'
  data: ActiveCall | ActiveCall[] | DashboardStats | { message: string }
}

// API Response wrapper - matches Rust backend ApiResponse
export interface ApiResponse<T> {
  success: boolean
  data?: T
  message?: string
  error?: string
}

// Pagination - matches Rust backend PaginatedResponse
export interface PaginatedResponse<T> {
  data: T[]
  items?: T[] // Alias for compatibility
  total: number
  page: number
  per_page: number
  total_pages: number
}

// Filter params for CDR
export interface CDRFilters {
  start_date?: string
  end_date?: string
  caller?: string
  callee?: string
  direction?: string
  destination_prefix?: string
  account_id?: number
  zone_id?: number
  min_cost?: number
  max_cost?: number
  min_duration?: number
  max_duration?: number
  hangup_cause?: string
}

// Dialplan Route for FreeSWITCH configuration
export interface BridgeDestination {
  ip: string
  port: number
}

export interface DialplanRoute {
  id: string
  name: string
  priority: number
  prefix_pattern: string
  destination_ip: string
  destination_port: number
  sip_profile: string
  bypass_media: boolean
  inherit_codec: boolean
  enable_100rel: boolean
  ignore_early_media: boolean
  call_timeout?: number
  source_ip_filter?: string
  failover_destinations: BridgeDestination[]
  enabled: boolean
}
