//! Data Transfer Objects for Unified Routing System
//!
//! DTOs for trunks, trunk groups, outbound routes, inbound routes,
//! and synchronization status.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Enums
// ============================================================================

/// Transport protocol for SIP trunks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Udp,
    Tcp,
    Tls,
}

impl Default for Transport {
    fn default() -> Self {
        Self::Udp
    }
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transport::Udp => write!(f, "udp"),
            Transport::Tcp => write!(f, "tcp"),
            Transport::Tls => write!(f, "tls"),
        }
    }
}

/// Failover strategy for trunk groups
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum FailoverStrategy {
    Sequential,
    RoundRobin,
    Weighted,
    LeastCalls,
}

impl Default for FailoverStrategy {
    fn default() -> Self {
        Self::Sequential
    }
}

impl std::fmt::Display for FailoverStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailoverStrategy::Sequential => write!(f, "sequential"),
            FailoverStrategy::RoundRobin => write!(f, "round_robin"),
            FailoverStrategy::Weighted => write!(f, "weighted"),
            FailoverStrategy::LeastCalls => write!(f, "least_calls"),
        }
    }
}

/// SIP profile for FreeSWITCH bridges
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum SipProfile {
    Internal,
    External,
}

impl Default for SipProfile {
    fn default() -> Self {
        Self::Internal
    }
}

impl std::fmt::Display for SipProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SipProfile::Internal => write!(f, "internal"),
            SipProfile::External => write!(f, "external"),
        }
    }
}

/// Synchronization status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Pending,
    Synced,
    Error,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Trunk type: private (FreeSWITCH) or public (Kamailio)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum TrunkType {
    Private,
    Public,
}

impl Default for TrunkType {
    fn default() -> Self {
        Self::Public
    }
}

impl std::fmt::Display for TrunkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrunkType::Private => write!(f, "private"),
            TrunkType::Public => write!(f, "public"),
        }
    }
}

/// SIP connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum SipStatus {
    Unknown,
    Reachable,
    Unreachable,
    Checking,
}

impl Default for SipStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl std::fmt::Display for SipStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SipStatus::Unknown => write!(f, "unknown"),
            SipStatus::Reachable => write!(f, "reachable"),
            SipStatus::Unreachable => write!(f, "unreachable"),
            SipStatus::Checking => write!(f, "checking"),
        }
    }
}

// ============================================================================
// Trunk DTOs
// ============================================================================

/// Trunk/Gateway representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trunk {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub port: Option<i32>,
    pub transport: Option<String>,
    pub auth_username: Option<String>,
    #[serde(skip_serializing)] // Never expose password
    pub auth_password_encrypted: Option<Vec<u8>>,
    #[serde(skip_serializing)] // Never expose nonce
    pub auth_password_nonce: Option<Vec<u8>>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    pub enabled: Option<bool>,
    /// Trunk type: private (FreeSWITCH) or public (Kamailio)
    pub trunk_type: Option<String>,
    /// FreeSWITCH gateway name (for private trunks)
    pub freeswitch_gateway_name: Option<String>,
    /// Kamailio gateway ID (for public trunks)
    pub kamailio_gwid: Option<i32>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    /// SIP connection status
    pub sip_status: Option<String>,
    pub sip_status_message: Option<String>,
    pub last_options_check: Option<DateTime<Utc>>,
    pub last_options_latency_ms: Option<i32>,
    pub last_options_response_code: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Request to create a new trunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrunkRequest {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default)]
    pub transport: String,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    #[serde(default)]
    pub strip_digits: i32,
    #[serde(default)]
    pub prefix_to_add: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Trunk type: "private" (FreeSWITCH) or "public" (Kamailio)
    #[serde(default = "default_trunk_type")]
    pub trunk_type: String,
}

/// Request to update a trunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrunkRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub transport: Option<String>,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    pub enabled: Option<bool>,
    pub trunk_type: Option<String>,
}

// ============================================================================
// Trunk Group DTOs
// ============================================================================

/// Trunk group representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub failover_strategy: Option<String>,
    pub kamailio_group_id: Option<i32>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Trunk group with its member trunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkGroupWithMembers {
    #[serde(flatten)]
    pub group: TrunkGroup,
    pub members: Vec<TrunkGroupMember>,
}

/// Trunk group member (trunk with priority/weight)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkGroupMember {
    pub id: Uuid,
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub trunk_host: String,
    pub trunk_port: Option<i32>,
    pub trunk_enabled: Option<bool>,
    pub priority: Option<i32>,
    pub weight: Option<i32>,
    pub max_channels: Option<i32>,
}

/// Request to create a trunk group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrunkGroupRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_failover_strategy")]
    pub failover_strategy: String,
    #[serde(default)]
    pub members: Vec<TrunkGroupMemberInput>,
}

/// Request to update a trunk group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrunkGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub failover_strategy: Option<String>,
    pub members: Option<Vec<TrunkGroupMemberInput>>,
}

/// Input for trunk group member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkGroupMemberInput {
    pub trunk_id: Uuid,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_weight")]
    pub weight: i32,
    pub max_channels: Option<i32>,
}

// ============================================================================
// Outbound Route DTOs
// ============================================================================

/// Outbound route representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundRoute {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub prefix_pattern: String,
    pub priority: Option<i32>,
    pub trunk_group_id: Option<Uuid>,
    pub trunk_group_name: Option<String>,
    pub trunk_id: Option<Uuid>,
    pub trunk_name: Option<String>,
    pub time_schedule: Option<String>,
    pub time_schedule_enabled: Option<bool>,
    pub enabled: Option<bool>,
    pub kamailio_ruleid: Option<i32>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Request to create an outbound route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOutboundRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub prefix_pattern: String,
    #[serde(default = "default_route_priority")]
    pub priority: i32,
    pub trunk_group_id: Option<Uuid>,
    pub trunk_id: Option<Uuid>,
    pub time_schedule: Option<String>,
    #[serde(default)]
    pub time_schedule_enabled: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request to update an outbound route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOutboundRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prefix_pattern: Option<String>,
    pub priority: Option<i32>,
    pub trunk_group_id: Option<Uuid>,
    pub trunk_id: Option<Uuid>,
    pub time_schedule: Option<String>,
    pub time_schedule_enabled: Option<bool>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Inbound Route DTOs
// ============================================================================

/// Failover destination for inbound routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverDestination {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
}

/// Inbound route representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRoute {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub did_pattern: String,
    pub source_ip_pattern: Option<String>,
    pub priority: Option<i32>,
    pub destination_host: String,
    pub destination_port: Option<i32>,
    pub destination_profile: Option<String>,
    pub call_timeout: Option<i32>,
    pub inherit_codec: Option<bool>,
    pub ignore_early_media: Option<bool>,
    pub bypass_media: Option<bool>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    pub destination_trunk_id: Option<Uuid>,
    pub destination_trunk_name: Option<String>,
    /// Send 183 Session Progress with ringback tone before bridging
    pub send_early_media: Option<bool>,
    pub failover_destinations: Vec<FailoverDestination>,
    pub enabled: Option<bool>,
    pub freeswitch_extension_id: Option<String>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Request to create an inbound route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInboundRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub did_pattern: String,
    pub source_ip_pattern: Option<String>,
    #[serde(default = "default_route_priority")]
    pub priority: i32,
    pub destination_host: String,
    #[serde(default = "default_port")]
    pub destination_port: i32,
    #[serde(default = "default_profile")]
    pub destination_profile: String,
    #[serde(default = "default_call_timeout")]
    pub call_timeout: i32,
    #[serde(default = "default_true")]
    pub inherit_codec: bool,
    #[serde(default)]
    pub ignore_early_media: bool,
    #[serde(default)]
    pub bypass_media: bool,
    #[serde(default)]
    pub strip_digits: i32,
    #[serde(default)]
    pub prefix_to_add: String,
    /// If set, route via this trunk's gateway instead of destination_host:port
    pub destination_trunk_id: Option<Uuid>,
    /// Send 183 Session Progress with ringback tone before bridging
    #[serde(default)]
    pub send_early_media: bool,
    #[serde(default)]
    pub failover_destinations: Vec<FailoverDestination>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request to update an inbound route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInboundRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub did_pattern: Option<String>,
    pub source_ip_pattern: Option<String>,
    pub priority: Option<i32>,
    pub destination_host: Option<String>,
    pub destination_port: Option<i32>,
    pub destination_profile: Option<String>,
    pub call_timeout: Option<i32>,
    pub inherit_codec: Option<bool>,
    pub ignore_early_media: Option<bool>,
    pub bypass_media: Option<bool>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    /// If set, route via this trunk's gateway instead of destination_host:port
    pub destination_trunk_id: Option<Uuid>,
    /// Send 183 Session Progress with ringback tone before bridging
    pub send_early_media: Option<bool>,
    pub failover_destinations: Option<Vec<FailoverDestination>>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Sync Status DTOs
// ============================================================================

/// Overall sync status for the routing system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSyncStatus {
    pub trunks_total: i64,
    pub trunks_synced: i64,
    pub trunks_pending: i64,
    pub trunks_error: i64,
    pub trunk_groups_total: i64,
    pub trunk_groups_synced: i64,
    pub outbound_routes_total: i64,
    pub outbound_routes_synced: i64,
    pub inbound_routes_total: i64,
    pub inbound_routes_synced: i64,
    pub kamailio_connected: bool,
    pub freeswitch_connected: bool,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Sync log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogEntry {
    pub id: i64,
    pub operation: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub target_system: String,
    pub status: String,
    pub error_message: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Migration DTOs
// ============================================================================

/// Migration result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub trunks_imported: i64,
    pub trunk_groups_imported: i64,
    pub outbound_routes_imported: i64,
    pub inbound_routes_imported: i64,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Reload result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadResult {
    pub kamailio_reloaded: bool,
    pub freeswitch_reloaded: bool,
    pub kamailio_message: Option<String>,
    pub freeswitch_message: Option<String>,
}

/// Sync to Kamailio result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToKamailioResult {
    pub trunks_synced: i64,
    pub trunks_failed: i64,
    pub trunk_groups_synced: i64,
    pub trunk_groups_failed: i64,
    pub outbound_routes_synced: i64,
    pub outbound_routes_failed: i64,
    pub kamailio_reloaded: bool,
    pub errors: Vec<String>,
}

// ============================================================================
// Default value functions
// ============================================================================

fn default_port() -> i32 {
    5060
}

fn default_true() -> bool {
    true
}

fn default_failover_strategy() -> String {
    "sequential".to_string()
}

fn default_priority() -> i32 {
    1
}

fn default_weight() -> i32 {
    100
}

fn default_route_priority() -> i32 {
    100
}

fn default_profile() -> String {
    "internal".to_string()
}

fn default_call_timeout() -> i32 {
    120
}

fn default_trunk_type() -> String {
    "public".to_string()
}

// ============================================================================
// SIP Status DTOs
// ============================================================================

/// SIP OPTIONS check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipStatusCheck {
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub trunk_type: String,
    pub host: String,
    pub port: i32,
    /// FreeSWITCH gateway name (for private trunks)
    pub gateway_name: Option<String>,
    pub status: String,
    pub status_message: String,
    pub response_code: Option<i32>,
    pub latency_ms: Option<i32>,
    pub checked_at: DateTime<Utc>,
    /// Which system performed the check
    pub check_source: String,
    /// Formatted SIP exchange for display
    pub sip_exchange: Option<SipExchange>,
}

/// SIP message exchange details (for didactic display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipExchange {
    /// Our SIP endpoint info
    pub local_endpoint: String,
    /// Remote SIP endpoint info
    pub remote_endpoint: String,
    /// The OPTIONS request we sent (simplified)
    pub request_summary: SipMessageSummary,
    /// The response we received (simplified)
    pub response_summary: Option<SipMessageSummary>,
    /// Timeline of the exchange
    pub timeline: Vec<SipExchangeStep>,
}

/// Simplified SIP message for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipMessageSummary {
    pub method_or_status: String,
    pub from: String,
    pub to: String,
    pub call_id: Option<String>,
    pub user_agent: Option<String>,
    pub allow_methods: Option<String>,
}

/// Step in the SIP exchange timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipExchangeStep {
    pub timestamp: DateTime<Utc>,
    pub direction: String,  // "outbound" or "inbound"
    pub message_type: String,
    pub description: String,
    pub status: String,  // "success", "pending", "error"
}

/// SIP status log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipStatusLogEntry {
    pub id: i64,
    pub trunk_id: Uuid,
    pub check_timestamp: Option<DateTime<Utc>>,
    pub check_source: String,
    pub status: String,
    pub response_code: Option<i32>,
    pub latency_ms: Option<i32>,
    pub error_message: Option<String>,
    pub peer_user_agent: Option<String>,
}

/// Bulk SIP status check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSipStatusResult {
    pub total_checked: i32,
    pub reachable: i32,
    pub unreachable: i32,
    pub errors: i32,
    pub results: Vec<SipStatusCheck>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_display() {
        assert_eq!(Transport::Udp.to_string(), "udp");
        assert_eq!(Transport::Tcp.to_string(), "tcp");
        assert_eq!(Transport::Tls.to_string(), "tls");
    }

    #[test]
    fn test_failover_strategy_display() {
        assert_eq!(FailoverStrategy::Sequential.to_string(), "sequential");
        assert_eq!(FailoverStrategy::RoundRobin.to_string(), "round_robin");
        assert_eq!(FailoverStrategy::Weighted.to_string(), "weighted");
        assert_eq!(FailoverStrategy::LeastCalls.to_string(), "least_calls");
    }

    #[test]
    fn test_create_trunk_request_defaults() {
        let json = r#"{"name": "Test", "host": "10.0.0.1"}"#;
        let req: CreateTrunkRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.port, 5060);
        assert!(req.enabled);
        assert_eq!(req.strip_digits, 0);
    }

    #[test]
    fn test_create_inbound_route_defaults() {
        let json = r#"{"name": "Test", "did_pattern": "^(.+)$", "destination_host": "10.0.0.1"}"#;
        let req: CreateInboundRouteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.destination_port, 5060);
        assert_eq!(req.destination_profile, "internal");
        assert_eq!(req.call_timeout, 120);
        assert!(req.inherit_codec);
        assert!(req.enabled);
    }
}
