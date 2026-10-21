export type AccountStatus = 'active' | 'suspended' | 'closed' | 'ACTIVE' | 'SUSPENDED' | 'CLOSED'

// User types
export type UserRole = 'superadmin' | 'admin' | 'usuario'

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

// Module catalog (visibility system) types
export interface SystemModule {
  module_key: string
  nombre: string
  seccion: string
  min_role: UserRole
  always_visible: boolean
  orden: number
}

export interface UserModulesResponse {
  usuario_id: number
  granted: string[]
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
  status: AccountStatus
  max_concurrent_calls: number
  plan_id?: number              // Plan used to create this account
  // Minute-bucket billing (Plan Control / Plan Abierto) — every account has this
  bucket_type: BucketType
  included_minutes: number
  proration_mode: ProrationMode
  overage_flat_rate_per_minute?: number | null
  overage_tariff_group?: string | null
  notification_email?: string | null
  created_at: string
  updated_at: string
}

// ============== Minute Bucket Billing (Plan Control / Plan Abierto) ==============

export type BucketType = 'control' | 'abierto'
export type ProrationMode = 'full' | 'prorated'

export interface BucketConfig {
  bucket_type: BucketType
  included_minutes: number
  proration_mode: ProrationMode
  overage_flat_rate_per_minute: number | null
  overage_tariff_group: string | null
}

export interface PlanBucketConfig extends BucketConfig {
  plan_id: number
}

export interface AccountBucketConfig extends BucketConfig {
  account_id: number
  notification_email: string | null
}

export interface MinuteBucketStatus {
  account_id: number
  account_number: string
  bucket_type: BucketType
  cycle_start: string
  cycle_end: string
  included_minutes: number
  is_prorated: boolean
  additional_minutes: number
  consumed_minutes: number
  overage_minutes: number
  overage_cost: number
  available_minutes: number
  utilization_percent: number
  is_blocked: boolean
  overage_rate_source: string
  status: 'open' | 'closed'
}

export interface MinuteBucketAddition {
  id: number
  account_id: number
  cycle_id: number
  minutes_added: number
  reason: string | null
  created_by: string
  created_at: string
}

export interface CycleHistoryEntry {
  id: number
  account_id: number
  cycle_start: string
  cycle_end: string
  included_minutes: number
  is_prorated: boolean
  proration_days: number | null
  additional_minutes: number
  reserved_minutes: number
  consumed_minutes: number
  overage_minutes: number
  overage_cost: number
  status: 'open' | 'closed'
  created_at: string
  updated_at: string
  additions: MinuteBucketAddition[]
}

export interface MinuteBucketReportRow {
  account_id: number
  account_number: string
  account_name: string | null
  bucket_type: string
  included_minutes: number
  is_prorated: boolean
  additional_minutes: number
  additions_count: number
  consumed_minutes: number
  overage_minutes: number
  overage_cost: number
  utilization_percent: number
  cycle_status: string
  is_blocked: boolean
}

// Plan types
export interface Plan {
  id: number
  plan_name: string
  plan_code: string
  max_concurrent_calls: number
  description?: string
  enabled: boolean
  // Minute-bucket billing template (Plan Control / Plan Abierto)
  bucket_type: BucketType
  included_minutes: number
  proration_mode: ProrationMode
  overage_flat_rate_per_minute?: number | null
  overage_tariff_group?: string | null
  created_at: string
  updated_at: string
  created_by: string
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
  card_group: string
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

  // Minute-bucket billing (Plan Control / Plan Abierto)
  bucket_cycle_id?: number
  in_bucket_minutes?: number
  overage_minutes?: number
  overage_cost?: number
  overage_rate_applied?: number
  source_ip?: string
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
// Dashboard Statistics - matches Rust backend DashboardStats
export interface DashboardStats {
  total_accounts: number
  active_accounts: number
  total_overage_cost_month: number
  active_calls: number
  blocked_accounts: number
  cdrs_today: number
  revenue_today: number
  minutes_today: number
  // Legacy fields for compatibility
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

// SIP Device types
// Unified SIP Endpoint model: an "extension" (single softphone/device) or a
// "trunk" (PBX carrying many calls), authenticated either by SIP credentials
// (REGISTER) or by trusted source IP (Kamailio ACL, trunk only).
export type EndpointType = 'extension' | 'trunk'
export type EndpointAuthMode = 'credentials' | 'ip'

export interface SipEndpoint {
  id: string
  name: string
  description?: string
  endpoint_type: EndpointType
  auth_mode: EndpointAuthMode
  account_id?: number

  // auth_mode = credentials
  sip_username?: string
  sip_domain?: string
  aor?: string
  max_registrations: number
  // Llamadas simultaneas (enforceada por billing engine/ESL en extensiones)
  max_concurrent_calls: number

  // auth_mode = ip
  host?: string
  port: number
  transport: TransportProtocol
  kamailio_gwid?: number
  sync_status: KamailioSyncStatus
  sync_error?: string
  is_default: boolean
  did_ranges: string
  default_caller_id?: string

  display_name?: string
  accountcode?: string
  codecs: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface SipEndpointWithPassword extends SipEndpoint {
  password: string
}

export interface SipEndpointCreateRequest {
  name: string
  description?: string
  endpoint_type: EndpointType
  auth_mode: EndpointAuthMode
  account_id?: number

  sip_username?: string
  sip_domain?: string
  password?: string
  max_registrations?: number
  max_concurrent_calls?: number

  host?: string
  port?: number
  transport?: TransportProtocol
  is_default?: boolean
  did_ranges?: string
  default_caller_id?: string

  display_name?: string
  accountcode?: string
  codecs?: string
  enabled?: boolean
}

export interface SipEndpointUpdateRequest {
  name?: string
  description?: string
  account_id?: number
  sip_username?: string
  sip_domain?: string
  max_registrations?: number
  max_concurrent_calls?: number
  host?: string
  port?: number
  transport?: TransportProtocol
  is_default?: boolean
  did_ranges?: string
  default_caller_id?: string
  display_name?: string
  accountcode?: string
  codecs?: string
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
  enabled?: boolean
}

// DEPRECATED: Old routing inbound route request - use CreateInboundRouteRequest from simplified model
export interface CreateRoutingInboundRouteRequest {
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
  failover_destinations?: FailoverDestination[]
  enabled?: boolean
}

// DEPRECATED: Old routing inbound route update - use UpdateInboundRouteRequestNew from simplified model
export interface UpdateRoutingInboundRouteRequest {
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

// ============== INBOUND ROUTING (Simplified) ==============

// Inbound Route represents a simple routing rule with auto-priority
// Priority is determined by pattern length - longer patterns win
export interface InboundRoute {
  id: string
  name: string
  description?: string
  pattern: string           // e.g. "672017402000" (exact) or "6720*" (wildcard)
  is_wildcard: boolean      // computed from pattern
  source_ip?: string
  destination_type: 'extension' | 'gateway'
  sip_endpoint_id?: string
  gateway_name?: string
  gateway_host?: string
  gateway_port?: number
  destination_host: string
  strip_digits: number
  prefix_to_add: string
  call_timeout: number
  ims_mode: boolean
  early_media: boolean
  enabled: boolean
  sync_status: string
  sync_error?: string
  created_at: string
  updated_at: string
}

// Request type for creating inbound route
// sip_endpoint_id (SIP device trunk, endpoint_type = 'trunk') selects the
// destination trunk; null = no trunk selected
export interface CreateInboundRouteRequest {
  name: string
  description?: string
  pattern: string
  source_ip?: string
  destination_type?: string
  sip_endpoint_id?: string | null
  destination_host?: string
  strip_digits?: number
  prefix_to_add?: string
  call_timeout?: number
  ims_mode?: boolean
  early_media?: boolean
  enabled?: boolean
}

// Request type for updating inbound route
// sip_endpoint_id: omit = keep current value, null = clear
export interface UpdateInboundRouteRequestNew {
  name?: string
  description?: string
  pattern?: string
  source_ip?: string
  destination_type?: string
  sip_endpoint_id?: string | null
  destination_host?: string
  strip_digits?: number
  prefix_to_add?: string
  call_timeout?: number
  ims_mode?: boolean
  early_media?: boolean
  enabled?: boolean
}

// Destination trunk (PBX) option for the inbound route gateway dropdown.
// Source: SIP devices with endpoint_type = 'trunk' (GET /inbound-routing/destinations)
export interface InboundGatewayOption {
  id: string
  name: string
  host: string
  port: number
  transport: string
  fs_profile: string
}

// DEPRECATED: Kept for backward compatibility
export interface Carrier {
  id: string
  name: string
  description?: string
  prefix: string
  source_ip?: string
  default_destination_type: 'extension' | 'gateway'
  default_gateway_id?: string
  default_gateway_name?: string
  default_destination_host: string
  default_destination_profile: string
  strip_digits: number
  prefix_to_add: string
  ims_mode: boolean
  send_early_media: boolean
  call_timeout: number
  enabled: boolean
  sync_status: string
  sync_error?: string
  did_count: number
  created_at: string
  updated_at: string
}

// DEPRECATED: Carrier summary for dropdowns
export interface CarrierSummary {
  id: string
  name: string
  prefix: string
}

// DEPRECATED: DID Override represents a routing exception for a specific DID
export interface DidOverride {
  id: string
  carrier_id: string
  carrier_name: string
  carrier_prefix: string
  did: string
  name?: string
  description?: string
  destination_type: 'extension' | 'gateway'
  gateway_id?: string
  gateway_name?: string
  destination_extension?: string
  destination_host?: string
  destination_profile: string
  strip_digits?: number
  prefix_to_add?: string
  ims_mode?: boolean
  send_early_media?: boolean
  call_timeout?: number
  enabled: boolean
  sync_status: string
  sync_error?: string
  created_at: string
  updated_at: string
}

// DEPRECATED: Request types for Carrier
export interface CreateCarrierRequest {
  name: string
  description?: string
  prefix: string
  source_ip?: string
  default_destination_type?: string
  default_gateway_id?: string
  default_destination_host?: string
  default_destination_profile?: string
  strip_digits?: number
  prefix_to_add?: string
  ims_mode?: boolean
  send_early_media?: boolean
  call_timeout?: number
  enabled?: boolean
}

export interface UpdateCarrierRequest {
  name?: string
  description?: string
  prefix?: string
  source_ip?: string
  default_destination_type?: string
  default_gateway_id?: string
  default_destination_host?: string
  default_destination_profile?: string
  strip_digits?: number
  prefix_to_add?: string
  ims_mode?: boolean
  send_early_media?: boolean
  call_timeout?: number
  enabled?: boolean
}

// DEPRECATED: Request types for DID Override
export interface CreateDidOverrideRequest {
  carrier_id: string
  did: string
  name?: string
  description?: string
  destination_type: string
  gateway_id?: string
  destination_extension?: string
  destination_host?: string
  destination_profile?: string
  strip_digits?: number
  prefix_to_add?: string
  ims_mode?: boolean
  send_early_media?: boolean
  call_timeout?: number
  enabled?: boolean
}

export interface UpdateDidOverrideRequest {
  carrier_id?: string
  did?: string
  name?: string
  description?: string
  destination_type?: string
  gateway_id?: string
  destination_extension?: string
  destination_host?: string
  destination_profile?: string
  strip_digits?: number
  prefix_to_add?: string
  ims_mode?: boolean
  send_early_media?: boolean
  call_timeout?: number
  enabled?: boolean
}

// ============== ENDPOINT MONITOR ==============

// Current status of a single endpoint (from latest probe cycle)
export interface EndpointStatus {
  trunk_id: string
  name: string
  ip: string
  port: number
  trunk_type: string
  status: 'up' | 'down'
  resp_code: number
  rtt_ms: string | null
  last_ts: number
}

// Response for GET /monitor/endpoints
export interface EndpointStatusResponse {
  endpoints: EndpointStatus[]
  generated_at: number
}

// A single point in the time series for charting
export interface SeriesPoint {
  t: number
  rtt: string | null
  status: 'up' | 'down'
  downs: number
  n: number
}

// Time series data for a single endpoint
export interface EndpointSeries {
  trunk_id: string
  name: string
  ip: string
  points: SeriesPoint[]
}

// A row in the probes table
export interface ProbeRow {
  id: number
  ts: number
  trunk_id: string
  name: string
  ip: string
  trunk_type: string
  port: number
  status: 'up' | 'down'
  resp_code: number
  rtt_ms: string | null
}

// Response for GET /monitor/results
export interface ProbeResultsResponse {
  frm: number
  to: number
  bucket_seconds: number
  generated_at: number
  series: EndpointSeries[]
  rows: ProbeRow[]
}

// Query parameters for probe results
export interface ProbeResultsQuery {
  name?: string
  ip?: string
  frm?: number
  to?: number
  limit?: number
}

// Query parameters for evidence report
export interface EvidenceReportQuery {
  name?: string
  ip?: string
  frm?: number
  to?: number
  full?: boolean
}

// ============== SIMPLIFIED ROUTING (dSIPRouter model) ==============

// Transport protocol
export type TransportProtocol = 'udp' | 'tcp' | 'tls'

// Sync status with Kamailio
export type KamailioSyncStatus = 'pending' | 'synced' | 'error'

// SIP OPTIONS status
export type SipOptionsStatus = 'unknown' | 'reachable' | 'unreachable' | 'checking'

// Failover strategy for carrier groups
export type CarrierFailoverStrategy = 'sequential' | 'round_robin' | 'weighted'

// ============== CARRIERS (dr_gateways type=8) ==============

export interface SipCarrier {
  id: string
  name: string
  description?: string
  host: string
  port: number
  transport: TransportProtocol
  auth_username?: string
  strip_digits: number
  prefix_to_add: string
  kamailio_gwid?: number
  sync_status: KamailioSyncStatus
  sync_error?: string
  sip_status: SipOptionsStatus
  last_options_check?: string
  last_options_latency_ms?: number
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface CreateSipCarrierRequest {
  name: string
  description?: string
  host: string
  port?: number
  transport?: TransportProtocol
  auth_username?: string
  auth_password?: string
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
}

export interface UpdateSipCarrierRequest {
  name?: string
  description?: string
  host?: string
  port?: number
  transport?: TransportProtocol
  auth_username?: string
  auth_password?: string
  strip_digits?: number
  prefix_to_add?: string
  enabled?: boolean
}

export interface SipCarrierListResponse {
  carriers: SipCarrier[]
  total: number
  online_count: number
  offline_count: number
}

export interface CarrierSipCheckResult {
  carrier_id: string
  carrier_name: string
  status: SipOptionsStatus
  latency_ms?: number
  response_code?: number
  error_message?: string
  checked_at: string
}

export interface CarrierSyncResult {
  carrier_id: string
  carrier_name: string
  kamailio_gwid?: number
  success: boolean
  error?: string
}

export interface BulkCarrierSyncResult {
  total: number
  succeeded: number
  failed: number
  results: CarrierSyncResult[]
}

// ============== CARRIER GROUPS ==============

export interface CarrierGroupMember {
  id: string
  group_id: string
  carrier_id: string
  carrier_name: string
  carrier_host: string
  carrier_enabled: boolean
  priority: number
  weight: number
}

export interface CarrierGroupBase {
  id: string
  name: string
  description?: string
  strategy: CarrierFailoverStrategy
  kamailio_group_id?: number
  sync_status: KamailioSyncStatus
  sync_error?: string
  created_at: string
  updated_at: string
}

export interface CarrierGroupWithMembers extends CarrierGroupBase {
  members: CarrierGroupMember[]
}

export interface CreateCarrierGroupRequest {
  name: string
  description?: string
  strategy?: CarrierFailoverStrategy
}

export interface UpdateCarrierGroupRequestNew {
  name?: string
  description?: string
  strategy?: CarrierFailoverStrategy
}

export interface AddCarrierGroupMemberRequest {
  carrier_id: string
  priority?: number
  weight?: number
}

export interface UpdateCarrierGroupMemberRequest {
  priority?: number
  weight?: number
}

export interface CarrierGroupListResponse {
  groups: CarrierGroupWithMembers[]
  total: number
}

// ============== OUTBOUND ROUTES ==============

export interface OutboundRouteNew {
  id: string
  name: string
  description?: string
  prefix: string
  carrier_group_id?: string
  carrier_id?: string
  destination_name?: string
  destination_type: 'group' | 'carrier'
  priority: number
  kamailio_ruleid?: number
  sync_status: KamailioSyncStatus
  sync_error?: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface CreateOutboundRouteRequestNew {
  name: string
  description?: string
  prefix: string
  carrier_group_id?: string
  carrier_id?: string
  enabled?: boolean
}

export interface UpdateOutboundRouteRequestNew {
  name?: string
  description?: string
  prefix?: string
  carrier_group_id?: string
  carrier_id?: string
  enabled?: boolean
}

export interface OutboundRouteListResponse {
  routes: OutboundRouteNew[]
  total: number
  enabled_count: number
  synced_count: number
}

export interface OutboundRouteSyncResult {
  route_id: string
  route_name: string
  kamailio_ruleid?: number
  success: boolean
  error?: string
}

// ============== KAMAILIO SYNC ==============

export interface OutboundSyncAllResult {
  carriers_synced: number
  carriers_failed: number
  groups_synced: number
  groups_failed: number
  routes_synced: number
  routes_failed: number
  kamailio_reloaded: boolean
  errors: string[]
}

export interface PbxGroupMember {
  id: string
  sip_endpoint_id: string
  endpoint_name: string
  host: string
  port: number
  priority: number
}

export interface PbxGroupWithMembers {
  id: string
  name: string
  description?: string
  strategy: string
  enabled: boolean
  created_at: string
  updated_at: string
  members: PbxGroupMember[]
}

export interface CreatePbxGroupRequest {
  name: string
  description?: string
  strategy?: string
  enabled?: boolean
}

export interface UpdatePbxGroupRequest {
  name?: string
  description?: string
  strategy?: string
  enabled?: boolean
}

export interface AddPbxGroupMemberRequest {
  sip_endpoint_id: string
  priority?: number
}
