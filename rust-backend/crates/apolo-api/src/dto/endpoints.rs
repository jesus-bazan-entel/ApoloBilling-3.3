use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::carriers::{SyncStatus, Transport};

/// Endpoint - Internal PBX connection (dr_gateways type=9)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,

    // SIP connection
    pub host: String,
    pub port: i32,
    pub transport: Transport,

    // FreeSWITCH config
    pub fs_profile: String,

    // Kamailio sync
    pub kamailio_gwid: Option<i32>,
    pub sync_status: SyncStatus,
    pub sync_error: Option<String>,

    pub enabled: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new SIP endpoint (for simplified routing)
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSipEndpointRequest {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub fs_profile: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

/// Request to update a SIP endpoint (for simplified routing)
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSipEndpointRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub fs_profile: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

/// Endpoint list response with summary stats
#[derive(Debug, Clone, Serialize)]
pub struct EndpointListResponse {
    pub endpoints: Vec<Endpoint>,
    pub total: i64,
    pub enabled_count: i64,
}

/// Result of Kamailio sync operation for endpoint
#[derive(Debug, Clone, Serialize)]
pub struct EndpointSyncResult {
    pub endpoint_id: Uuid,
    pub endpoint_name: String,
    pub kamailio_gwid: Option<i32>,
    pub success: bool,
    pub error: Option<String>,
}
