// Customer (account) status: anything but active blocks the customer's SIP REGISTER
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
  details?: Record<string, unknown>
  ip_address?: string
  user_agent?: string
  created_at: string
}

/** Customer (subscriber) a SIP device belongs to */
export interface Account {
  id: number
  account_number: string
  account_name?: string
  customer_phone?: string
  status: AccountStatus
  cod_ped?: string
  departamento?: string
  provincia?: string
  distrito?: string
  created_at: string
  updated_at: string
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
  hangup_cause?: string
  direction: 'inbound' | 'outbound' | 'internal' | string
  destination?: string
  destination_prefix?: string
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
}

// Stats pushed over the WebSocket
export interface WsStats {
  active_calls: number
  calls_today: number
}

// WebSocket Message Types
export interface WSMessage {
  type: 'active_calls' | 'call_start' | 'call_update' | 'call_end' | 'stats_update' | 'pong' | 'error'
  data: ActiveCall | ActiveCall[] | WsStats | { message: string }
}

// API Response wrapper - matches Rust backend ApiResponse
export interface ApiResponse<T> {
  success: boolean
  data?: T
  message?: string
  error?: string
}

// Global account statistics (matches backend AccountStats)
export interface AccountStats {
  total_accounts: number
  active_accounts: number
}

// Provisioning (account + SIP device, individual & bulk)
export interface ProvisionRequest {
  customer_name?: string
  username: string
  password?: string
  domain?: string
  /** Simultaneous registrations for the SIP device (1–10, server default 2) */
  max_registrations?: number
  cod_ped?: string
  departamento?: string
  provincia?: string
  distrito?: string
}

export interface ProvisionResult {
  account_id: number
  account_number: string
  sip_device_id: number
  sip_username: string
  sip_domain: string
  password: string
  max_registrations: number
}

export type BulkRowStatus = 'created' | 'skipped' | 'error'

export interface BulkRowResult {
  row: number
  username: string
  status: BulkRowStatus
  message?: string
  account_id?: number
  sip_device_id?: number
  password?: string
}

export interface BulkProvisionResponse {
  total: number
  created: number
  skipped: number
  errors: number
  results: BulkRowResult[]
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

// SIP Device types
export interface SipDevice {
  id: number
  account_id: number
  sip_username: string
  sip_domain: string
  display_name?: string
  description?: string
  context: string
  accountcode?: string
  codecs: string
  max_registrations: number
  enabled: boolean
  aor: string
  created_at: string
  updated_at: string
}

export interface SipDeviceWithPassword extends SipDevice {
  password: string
}

export interface SipDeviceCreateRequest {
  account_id: number
  sip_username: string
  sip_domain?: string
  password?: string
  display_name?: string
  description?: string
  context?: string
  accountcode?: string
  codecs?: string
  max_registrations?: number
  enabled?: boolean
}

export interface SipDeviceUpdateRequest {
  account_id?: number
  sip_username?: string
  sip_domain?: string
  display_name?: string
  description?: string
  context?: string
  accountcode?: string
  codecs?: string
  max_registrations?: number
  enabled?: boolean
}

export interface FreeswitchAllowedIp {
  id: number
  ip_address: string
  description?: string
  enabled: boolean
  created_at: string
  updated_at: string
}

// ============== UNIFIED ROUTING ==============

// Failover strategy for trunk groups
export type FailoverStrategy = 'sequential' | 'round_robin' | 'weighted' | 'least_calls'

// SIP transport protocol
export type Transport = 'udp' | 'tcp' | 'tls'

// Sync status
export type SyncStatus = 'pending' | 'synced' | 'error'

// Trunk type: private (FreeSWITCH) or public (Kamailio)
export type TrunkType = 'private' | 'public'

// SIP connection status
export type SipStatus = 'unknown' | 'reachable' | 'unreachable' | 'checking'

// Trunk (carrier/gateway)
export interface RoutingTrunk {
  id: string
  name: string
  description?: string
  host: string
  port: number
  transport: string
  auth_username?: string
  strip_digits: number
  prefix_to_add: string
  enabled: boolean
  // Trunk type and system routing
  trunk_type?: TrunkType
  freeswitch_gateway_name?: string
  // Kamailio sync
  kamailio_gwid?: number
  sync_status: string
  sync_error?: string
  // SIP status monitoring
  sip_status?: SipStatus
  sip_status_message?: string
  last_options_check?: string
  last_options_latency_ms?: number
  last_options_response_code?: number
  // Timestamps
  created_at: string
  updated_at: string
}

// Trunk group (failover group)
export interface RoutingTrunkGroup {
  id: string
  name: string
  description?: string
  failover_strategy: string
  kamailio_group_id?: number
  sync_status: string
  sync_error?: string
  created_at: string
  updated_at: string
}

// Trunk group member
export interface RoutingTrunkGroupMember {
  id: string
  trunk_id: string
  trunk_name: string
  trunk_host: string
  trunk_port: number
  trunk_enabled: boolean
  priority: number
  weight: number
  max_channels?: number
}

// Trunk group with members
export interface RoutingTrunkGroupWithMembers extends RoutingTrunkGroup {
  members: RoutingTrunkGroupMember[]
}

// Outbound route
export interface RoutingOutboundRoute {
  id: string
  name: string
  description?: string
  prefix_pattern: string
  priority: number
  trunk_group_id?: string
  trunk_group_name?: string
  trunk_id?: string
  trunk_name?: string
  time_schedule?: string
  time_schedule_enabled: boolean
  strip_digits: number
  prefix_to_add: string
  enabled: boolean
  kamailio_ruleid?: number
  sync_status: string
  sync_error?: string
  created_at: string
  updated_at: string
}

// Failover destination for inbound routes
export interface FailoverDestination {
  host: string
  port: number
}

// Inbound route
export interface RoutingInboundRoute {
  id: string
  name: string
  description?: string
  did_pattern: string
  source_ip_pattern?: string
  priority: number
  destination_host: string
  destination_port: number
  destination_profile: string
  call_timeout: number
  inherit_codec: boolean
  ignore_early_media: boolean
  bypass_media: boolean
  strip_digits: number
  prefix_to_add: string
  destination_trunk_id?: string
  destination_trunk_name?: string
  send_early_media?: boolean
  parallel_trunk_id?: string
  parallel_trunk_name?: string
  direct_routing?: boolean
  fs_numbers?: string[]
  failover_destinations: FailoverDestination[]
  enabled: boolean
  freeswitch_extension_id?: string
  sync_status: string
  sync_error?: string
  created_at: string
  updated_at: string
}

// Sync status summary
export interface RoutingSyncStatus {
  trunks_total: number
  trunks_synced: number
  trunks_pending: number
  trunks_error: number
  trunk_groups_total: number
  trunk_groups_synced: number
  outbound_routes_total: number
  outbound_routes_synced: number
  inbound_routes_total: number
  inbound_routes_synced: number
  kamailio_connected: boolean
  freeswitch_connected: boolean
  last_sync?: string
}

// Reload result
export interface RoutingReloadResult {
  kamailio_reloaded: boolean
  freeswitch_reloaded: boolean
  kamailio_message?: string
  freeswitch_message?: string
}

// Migration result
export interface RoutingMigrationResult {
  trunks_imported: number
  trunk_groups_imported: number
  outbound_routes_imported: number
  inbound_routes_imported: number
  warnings: string[]
  errors: string[]
}

// Sync to Kamailio result
export interface SyncToKamailioResult {
  trunks_synced: number
  trunks_failed: number
  trunk_groups_synced: number
  trunk_groups_failed: number
  outbound_routes_synced: number
  outbound_routes_failed: number
  kamailio_reloaded: boolean
  errors: string[]
}

// Request types
export interface CreateTrunkRequest {
  name: string
  description?: string
  host: string
  port?: number
  transport?: string
  auth_username?: string
  auth_password?: string
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
  trunk_type?: TrunkType
}

export interface UpdateTrunkRequest {
  name?: string
  description?: string
  host?: string
  port?: number
  transport?: string
  auth_username?: string
  auth_password?: string
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
  trunk_type?: TrunkType
}

export interface TrunkGroupMemberInput {
  trunk_id: string
  priority?: number
  weight?: number
  max_channels?: number
}

export interface CreateTrunkGroupRequest {
  name: string
  description?: string
  failover_strategy?: string
  members?: TrunkGroupMemberInput[]
}

export interface UpdateTrunkGroupRequest {
  name?: string
  description?: string
  failover_strategy?: string
  members?: TrunkGroupMemberInput[]
}

export interface CreateOutboundRouteRequest {
  name: string
  description?: string
  prefix_pattern: string
  priority?: number
  trunk_group_id?: string
  trunk_id?: string
  time_schedule?: string
  time_schedule_enabled?: boolean
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
}

export interface UpdateOutboundRouteRequest {
  name?: string
  description?: string
  prefix_pattern?: string
  priority?: number
  trunk_group_id?: string
  trunk_id?: string
  time_schedule?: string
  time_schedule_enabled?: boolean
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
}

export interface CreateInboundRouteRequest {
  name: string
  description?: string
  did_pattern: string
  source_ip_pattern?: string
  priority?: number
  destination_host: string
  destination_port?: number
  destination_profile?: string
  call_timeout?: number
  inherit_codec?: boolean
  ignore_early_media?: boolean
  bypass_media?: boolean
  strip_digits?: number
  prefix_to_add?: string
  destination_trunk_id?: string
  send_early_media?: boolean
  parallel_trunk_id?: string
  direct_routing?: boolean
  fs_numbers?: string[]
  failover_destinations?: FailoverDestination[]
  enabled?: boolean
}

export interface UpdateInboundRouteRequest {
  name?: string
  description?: string
  did_pattern?: string
  source_ip_pattern?: string
  priority?: number
  destination_host?: string
  destination_port?: number
  destination_profile?: string
  call_timeout?: number
  inherit_codec?: boolean
  ignore_early_media?: boolean
  bypass_media?: boolean
  strip_digits?: number
  prefix_to_add?: string
  destination_trunk_id?: string | null
  send_early_media?: boolean
  parallel_trunk_id?: string
  direct_routing?: boolean
  fs_numbers?: string[]
  failover_destinations?: FailoverDestination[]
  enabled?: boolean
}

// ============== SIP STATUS MONITORING ==============

// SIP message summary (header highlights)
export interface SipMessageSummary {
  method_or_status: string
  from?: string
  to?: string
  call_id?: string
  cseq?: string
  via?: string
  user_agent?: string
  allow?: string
}

// Timeline step in the SIP exchange
export interface SipExchangeStep {
  timestamp: string
  direction: 'sent' | 'received'
  message_type: string
  summary: string
}

// Full SIP exchange details
export interface SipExchange {
  local_endpoint: string
  remote_endpoint: string
  request_summary: SipMessageSummary
  response_summary?: SipMessageSummary
  timeline: SipExchangeStep[]
}

// SIP status check result for a single trunk
export interface SipStatusCheck {
  trunk_id: string
  trunk_name: string
  trunk_type: string
  host: string
  port: number
  gateway_name?: string
  status: SipStatus
  status_message: string
  check_source: string
  response_code?: number
  latency_ms?: number
  checked_at: string
  sip_exchange?: SipExchange
}

// Bulk SIP status result for all trunks
export interface BulkSipStatusResult {
  total_checked: number
  reachable: number
  unreachable: number
  errors: number
  results: SipStatusCheck[]
}

// SIP status history entry
export interface SipStatusLogEntry {
  id: number
  trunk_id: string
  check_timestamp: string
  check_source: string
  status: string
  response_code?: number
  latency_ms?: number
  request_sent?: string
  response_received?: string
  error_message?: string
  peer_user_agent?: string
  peer_allow_methods?: string
}

// ============================================================================
// NOC overview (GET /stats/noc)
// ============================================================================

export type NocCompare = 'yesterday' | 'week'

export interface NocTotals {
  attempts: number
  answered: number
  billsec: number
  network_failures: number
}

export interface NocSlotValues {
  attempts: number
  answered: number
  concurrent: number
}

export interface NocSlot {
  slot: number
  label: string
  today: NocSlotValues | null
  compare: NocSlotValues
}

export interface NocCause {
  cause: string
  category: 'network' | 'user' | 'cancel' | 'other'
  today: number
  compare: number
  /** Of today, CDRs imported from Kamailio (direct routing, real SIP code) */
  today_direct: number
}

export interface NocRange {
  prefix: string
  route: 'freeswitch' | 'direct'
  attempts: number
  answered: number
  billsec: number
  compare_attempts: number
  /** Attempts per local hour of today (24 values) */
  hourly_attempts: number[]
}

export interface NocThresholds {
  asr_warn: number
  asr_crit: number
  ner_warn: number
  ner_crit: number
  min_range_attempts: number
}

export interface NocCapacity {
  max_channels: number | null
  max_cps: number | null
  peak_cps_last_hour: number
  avg_cps_last_hour: number
}

export interface NocQuality {
  window_start: string | null
  as_of: string | null
  /** 15 = live 1-minute samples, 60 = hourly rollup fallback */
  window_minutes: number
  /** Sessions past ringing with media missing in one or both directions */
  one_sided_sessions: number | null
  /** Sessions whose media direction was checked (latest sample) */
  sessions_checked: number | null
  active_sessions: number | null
}

export type NocEventKind = 'incident' | 'campaign' | 'maintenance' | 'migration' | 'other'

export interface NocEvent {
  id: number
  kind: NocEventKind
  title: string
  starts_at: string
  ends_at: string | null
  created_by: string | null
}

export interface NocOverview {
  timezone: string
  compare: NocCompare
  day_start: string
  compare_day_start: string
  generated_at: string
  current_slot: number
  slot_minutes: number
  kpis: {
    today: NocTotals
    compare_same_time: NocTotals
    compare_full_day: NocTotals
  }
  series: NocSlot[]
  causes: NocCause[]
  ranges: NocRange[]
  thresholds: NocThresholds
  capacity: NocCapacity
  quality: NocQuality
  /** First direct-routing (Kamailio) CDR since the comparison day */
  direct_cdrs_since: string | null
  events: NocEvent[]
}
