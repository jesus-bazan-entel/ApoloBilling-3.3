use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============ System Endpoints DTOs ============

#[derive(Debug, Deserialize)]
pub struct CreateEndpointRequest {
    pub name: String,
    pub endpoint_type: String,
    pub description: Option<String>,
    pub ip_address: String,
    pub port: i32,
    #[serde(default = "default_transport")]
    pub transport: String,
    // FreeSWITCH specific
    pub fs_profile: Option<String>,
    pub fs_context: Option<String>,
    // Kamailio specific
    pub kam_gwid: Option<i32>,
    pub kam_gw_type: Option<i32>,
    #[serde(default)]
    pub is_primary: bool,
}

fn default_transport() -> String {
    "udp".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateEndpointRequest {
    pub name: String,
    pub description: Option<String>,
    pub ip_address: String,
    pub port: i32,
    pub transport: String,
    pub fs_profile: Option<String>,
    pub fs_context: Option<String>,
    pub kam_gwid: Option<i32>,
    pub kam_gw_type: Option<i32>,
    pub enabled: bool,
    pub is_primary: bool,
}

// ============ Internal Routes DTOs ============

#[derive(Debug, Deserialize)]
pub struct CreateInternalRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub route_type: String,
    pub source_endpoint_id: Option<Uuid>,
    pub dest_endpoint_id: Option<Uuid>,
    #[serde(default = "default_true")]
    pub bypass_media: bool,
    #[serde(default = "default_true")]
    pub inherit_codec: bool,
    #[serde(default = "default_true")]
    pub enable_100rel: bool,
    #[serde(default = "default_timeout")]
    pub call_timeout: i32,
    pub prefix_pattern: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> i32 {
    60
}

fn default_priority() -> i32 {
    100
}

#[derive(Debug, Deserialize)]
pub struct UpdateInternalRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub source_endpoint_id: Option<Uuid>,
    pub dest_endpoint_id: Option<Uuid>,
    pub bypass_media: bool,
    pub inherit_codec: bool,
    pub enable_100rel: bool,
    pub call_timeout: i32,
    pub prefix_pattern: Option<String>,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub freeswitch_synced: bool,
    pub kamailio_synced: bool,
}
