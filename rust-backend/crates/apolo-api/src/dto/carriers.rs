use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport protocol for SIP
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Udp,
    Tcp,
    Tls,
}

impl Default for Transport {
    fn default() -> Self {
        Transport::Udp
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

impl std::str::FromStr for Transport {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "udp" => Ok(Transport::Udp),
            "tcp" => Ok(Transport::Tcp),
            "tls" => Ok(Transport::Tls),
            _ => Err(format!("Invalid transport: {}", s)),
        }
    }
}

/// Sync status with Kamailio
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Pending,
    Synced,
    Error,
}

impl Default for SyncStatus {
    fn default() -> Self {
        SyncStatus::Pending
    }
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncStatus::Pending => write!(f, "pending"),
            SyncStatus::Synced => write!(f, "synced"),
            SyncStatus::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for SyncStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(SyncStatus::Pending),
            "synced" => Ok(SyncStatus::Synced),
            "error" => Ok(SyncStatus::Error),
            _ => Err(format!("Invalid sync status: {}", s)),
        }
    }
}

/// SIP OPTIONS health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SipStatus {
    Unknown,
    Reachable,
    Unreachable,
    Checking,
}

impl Default for SipStatus {
    fn default() -> Self {
        SipStatus::Unknown
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

impl std::str::FromStr for SipStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unknown" => Ok(SipStatus::Unknown),
            "reachable" => Ok(SipStatus::Reachable),
            "unreachable" => Ok(SipStatus::Unreachable),
            "checking" => Ok(SipStatus::Checking),
            _ => Err(format!("Invalid SIP status: {}", s)),
        }
    }
}

/// Carrier - External SIP provider (dr_gateways type=8)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Carrier {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,

    // SIP connection
    pub host: String,
    pub port: i32,
    pub transport: Transport,

    // Authentication (optional)
    pub auth_username: Option<String>,
    #[serde(skip_serializing)]
    pub auth_password: Option<String>,

    // Number transformation
    pub strip_digits: i32,
    pub prefix_to_add: String,

    // Kamailio sync
    pub kamailio_gwid: Option<i32>,
    pub sync_status: SyncStatus,
    pub sync_error: Option<String>,

    // SIP OPTIONS monitoring
    pub sip_status: SipStatus,
    pub last_options_check: Option<DateTime<Utc>>,
    pub last_options_latency_ms: Option<i32>,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new carrier
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCarrierRequest {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    pub enabled: Option<bool>,
}

/// Request to update a carrier
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCarrierRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub strip_digits: Option<i32>,
    pub prefix_to_add: Option<String>,
    pub enabled: Option<bool>,
}

/// Carrier list response with summary stats
#[derive(Debug, Clone, Serialize)]
pub struct CarrierListResponse {
    pub carriers: Vec<Carrier>,
    pub total: i64,
    pub online_count: i64,
    pub offline_count: i64,
}

/// Result of SIP OPTIONS check
#[derive(Debug, Clone, Serialize)]
pub struct CarrierSipCheckResult {
    pub carrier_id: Uuid,
    pub carrier_name: String,
    pub status: SipStatus,
    pub latency_ms: Option<i32>,
    pub response_code: Option<i32>,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

/// Result of Kamailio sync operation
#[derive(Debug, Clone, Serialize)]
pub struct CarrierSyncResult {
    pub carrier_id: Uuid,
    pub carrier_name: String,
    pub kamailio_gwid: Option<i32>,
    pub success: bool,
    pub error: Option<String>,
}

/// Bulk sync result
#[derive(Debug, Clone, Serialize)]
pub struct BulkSyncResult {
    pub total: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub results: Vec<CarrierSyncResult>,
}
