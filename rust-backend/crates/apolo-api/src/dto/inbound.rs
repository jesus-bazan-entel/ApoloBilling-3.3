//! Data Transfer Objects for Inbound Routing System
//!
//! DTOs for simplified inbound routes with auto-priority.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Inbound Route DTOs
// ============================================================================

/// Inbound Route response DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRouteResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub pattern: String,
    pub is_wildcard: bool,
    pub source_ip: Option<String>,
    pub destination_type: String,
    pub sip_endpoint_id: Option<Uuid>,
    pub gateway_name: Option<String>,
    pub gateway_host: Option<String>,
    pub gateway_port: Option<i32>,
    pub destination_host: String,
    pub strip_digits: i32,
    pub prefix_to_add: String,
    pub call_timeout: i32,
    pub ims_mode: bool,
    pub early_media: bool,
    pub enabled: bool,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create inbound route request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInboundRouteRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub pattern: String,
    #[serde(default)]
    pub source_ip: Option<String>,
    #[serde(default = "default_destination_type")]
    pub destination_type: String,
    /// Destination trunk (PBX) as a SIP device: `sip_endpoints.id`
    /// where `endpoint_type = 'trunk'`.
    #[serde(default)]
    pub sip_endpoint_id: Option<Uuid>,
    #[serde(default = "default_destination_host")]
    pub destination_host: String,
    #[serde(default = "default_strip_digits")]
    pub strip_digits: i32,
    #[serde(default)]
    pub prefix_to_add: String,
    #[serde(default = "default_call_timeout")]
    pub call_timeout: i32,
    #[serde(default)]
    pub ims_mode: bool,
    #[serde(default)]
    pub early_media: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_destination_type() -> String {
    "extension".to_string()
}

fn default_destination_host() -> String {
    "10.10.22.4".to_string()
}

fn default_strip_digits() -> i32 {
    4
}

fn default_call_timeout() -> i32 {
    55
}

fn default_true() -> bool {
    true
}

/// Deserialize a field that must distinguish "absent" from "null".
///
/// * key missing -> `None` (leave the stored value unchanged)
/// * `null`      -> `Some(None)` (clear the stored value)
/// * `"uuid"`    -> `Some(Some(id))` (store the given value)
#[allow(clippy::option_option)]
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Update inbound route request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInboundRouteRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub source_ip: Option<String>,
    #[serde(default)]
    pub destination_type: Option<String>,
    /// Destination trunk (PBX) as a SIP device: `sip_endpoints.id`
    /// where `endpoint_type = 'trunk'`.
    ///
    /// Double option so the client can distinguish "leave unchanged" (field
    /// omitted) from "clear" (`null`), which plain `Option` cannot do.
    #[serde(default, deserialize_with = "double_option")]
    pub sip_endpoint_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub destination_host: Option<String>,
    #[serde(default)]
    pub strip_digits: Option<i32>,
    #[serde(default)]
    pub prefix_to_add: Option<String>,
    #[serde(default)]
    pub call_timeout: Option<i32>,
    #[serde(default)]
    pub ims_mode: Option<bool>,
    #[serde(default)]
    pub early_media: Option<bool>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Paginated inbound route list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRouteListResponse {
    pub data: Vec<InboundRouteResponse>,
    pub total: i64,
}

/// Destination option for the inbound route "Troncal (PBX)" dropdown.
///
/// Source: `sip_endpoints WHERE endpoint_type = 'trunk'` (SIP devices that are
/// trunks/PBX, e.g. "PBX GC GROUP").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundGatewayOption {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub transport: String,
    pub fs_profile: String,
}

// ============================================================================
// Deprecated types for backward compatibility
// These will be removed after frontend migration
// ============================================================================

/// DEPRECATED: Use InboundRouteResponse
pub type CarrierResponse = InboundRouteResponse;

/// DEPRECATED: Use InboundRouteResponse
pub type DidOverrideResponse = InboundRouteResponse;

/// DEPRECATED: Use CreateInboundRouteRequest
pub type CreateCarrierRequest = CreateInboundRouteRequest;

/// DEPRECATED: Use CreateInboundRouteRequest
pub type CreateDidOverrideRequest = CreateInboundRouteRequest;

/// DEPRECATED: Use UpdateInboundRouteRequest
pub type UpdateCarrierRequest = UpdateInboundRouteRequest;

/// DEPRECATED: Use UpdateInboundRouteRequest
pub type UpdateDidOverrideRequest = UpdateInboundRouteRequest;

/// DEPRECATED: Use InboundRouteListResponse
pub type CarrierListResponse = InboundRouteListResponse;

/// DEPRECATED: Use InboundRouteListResponse
pub type DidOverrideListResponse = InboundRouteListResponse;

/// Carrier summary for dropdown (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierSummary {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
}

/// Destination type enum for DTOs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum CarrierDestinationType {
    Extension,
    Gateway,
}

impl Default for CarrierDestinationType {
    fn default() -> Self {
        Self::Extension
    }
}

impl std::fmt::Display for CarrierDestinationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extension => write!(f, "extension"),
            Self::Gateway => write!(f, "gateway"),
        }
    }
}
