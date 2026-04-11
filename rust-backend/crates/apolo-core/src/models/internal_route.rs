use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// System endpoint type (FreeSWITCH or Kamailio)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EndpointType {
    Freeswitch,
    Kamailio,
}

impl std::fmt::Display for EndpointType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointType::Freeswitch => write!(f, "freeswitch"),
            EndpointType::Kamailio => write!(f, "kamailio"),
        }
    }
}

impl std::str::FromStr for EndpointType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "freeswitch" => Ok(EndpointType::Freeswitch),
            "kamailio" => Ok(EndpointType::Kamailio),
            _ => Err(format!("Invalid endpoint type: {}", s)),
        }
    }
}

/// Internal route type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InternalRouteType {
    FsToKamailio,
    KamailioToFs,
}

impl std::fmt::Display for InternalRouteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalRouteType::FsToKamailio => write!(f, "fs_to_kamailio"),
            InternalRouteType::KamailioToFs => write!(f, "kamailio_to_fs"),
        }
    }
}

impl std::str::FromStr for InternalRouteType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fs_to_kamailio" => Ok(InternalRouteType::FsToKamailio),
            "kamailio_to_fs" => Ok(InternalRouteType::KamailioToFs),
            _ => Err(format!("Invalid route type: {}", s)),
        }
    }
}

/// System endpoint (FreeSWITCH or Kamailio connection point)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEndpoint {
    pub id: Uuid,
    pub name: String,
    pub endpoint_type: String,
    pub description: Option<String>,
    pub ip_address: String,
    pub port: i32,
    pub transport: String,
    // FreeSWITCH specific
    pub fs_profile: Option<String>,
    pub fs_context: Option<String>,
    // Kamailio specific
    pub kam_gwid: Option<i32>,
    pub kam_gw_type: Option<i32>,
    // Status
    pub enabled: bool,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal route between system endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalRoute {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub route_type: String,
    pub source_endpoint_id: Option<Uuid>,
    pub dest_endpoint_id: Option<Uuid>,
    // Routing options
    pub bypass_media: bool,
    pub inherit_codec: bool,
    pub enable_100rel: bool,
    pub call_timeout: i32,
    pub prefix_pattern: Option<String>,
    // Sync status
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
    // Status
    pub priority: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal route with endpoint details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalRouteWithEndpoints {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub route_type: String,
    // Source endpoint
    pub source_endpoint_id: Option<Uuid>,
    pub source_name: Option<String>,
    pub source_type: Option<String>,
    pub source_ip: Option<String>,
    pub source_port: Option<i32>,
    // Destination endpoint
    pub dest_endpoint_id: Option<Uuid>,
    pub dest_name: Option<String>,
    pub dest_type: Option<String>,
    pub dest_ip: Option<String>,
    pub dest_port: Option<i32>,
    // Routing options
    pub bypass_media: bool,
    pub inherit_codec: bool,
    pub enable_100rel: bool,
    pub call_timeout: i32,
    pub prefix_pattern: Option<String>,
    // Sync status
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
    // Status
    pub priority: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
