use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::carriers::{Carrier, SyncStatus};

/// Failover strategy for carrier groups
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FailoverStrategy {
    Sequential,
    RoundRobin,
    Weighted,
}

impl Default for FailoverStrategy {
    fn default() -> Self {
        FailoverStrategy::Sequential
    }
}

impl std::fmt::Display for FailoverStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailoverStrategy::Sequential => write!(f, "sequential"),
            FailoverStrategy::RoundRobin => write!(f, "round_robin"),
            FailoverStrategy::Weighted => write!(f, "weighted"),
        }
    }
}

impl std::str::FromStr for FailoverStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sequential" => Ok(FailoverStrategy::Sequential),
            "round_robin" => Ok(FailoverStrategy::RoundRobin),
            "weighted" => Ok(FailoverStrategy::Weighted),
            _ => Err(format!("Invalid failover strategy: {}", s)),
        }
    }
}

/// Carrier group for failover routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub strategy: FailoverStrategy,
    pub kamailio_group_id: Option<i32>,
    pub sync_status: SyncStatus,
    pub sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Carrier group member with priority and weight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierGroupMember {
    pub id: Uuid,
    pub group_id: Uuid,
    pub carrier_id: Uuid,
    pub carrier_name: String,
    pub carrier_host: String,
    pub carrier_enabled: bool,
    pub priority: i32,
    pub weight: i32,
}

/// Carrier group with its members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierGroupWithMembers {
    #[serde(flatten)]
    pub group: CarrierGroup,
    pub members: Vec<CarrierGroupMember>,
}

/// Request to create a carrier group
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCarrierGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub strategy: Option<FailoverStrategy>,
}

/// Request to update a carrier group
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCarrierGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub strategy: Option<FailoverStrategy>,
}

/// Request to add a member to a carrier group
#[derive(Debug, Clone, Deserialize)]
pub struct AddCarrierGroupMemberRequest {
    pub carrier_id: Uuid,
    pub priority: Option<i32>,
    pub weight: Option<i32>,
}

/// Request to update a member in a carrier group
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCarrierGroupMemberRequest {
    pub priority: Option<i32>,
    pub weight: Option<i32>,
}

/// Outbound route - prefix-based routing to carrier/group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundRoute {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub prefix: String,

    // Destination (one of these)
    pub carrier_group_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,

    // Resolved destination info
    pub destination_name: Option<String>,
    pub destination_type: String, // "group" or "carrier"

    /// Kamailio drouting priority, derived from the prefix length
    pub priority: i32,

    pub kamailio_ruleid: Option<i32>,
    pub sync_status: SyncStatus,
    pub sync_error: Option<String>,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create an outbound route
#[derive(Debug, Clone, Deserialize)]
pub struct CreateOutboundRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub prefix: String,
    pub carrier_group_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,
    pub enabled: Option<bool>,
}

/// Request to update an outbound route
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateOutboundRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prefix: Option<String>,
    pub carrier_group_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,
    pub enabled: Option<bool>,
}

/// Outbound route list response with stats
#[derive(Debug, Clone, Serialize)]
pub struct OutboundRouteListResponse {
    pub routes: Vec<OutboundRoute>,
    pub total: i64,
    pub enabled_count: i64,
    pub synced_count: i64,
}

/// Result of Kamailio sync for outbound routes
#[derive(Debug, Clone, Serialize)]
pub struct OutboundRouteSyncResult {
    pub route_id: Uuid,
    pub route_name: String,
    pub kamailio_ruleid: Option<i32>,
    pub success: bool,
    pub error: Option<String>,
}

/// Carrier group list response
#[derive(Debug, Clone, Serialize)]
pub struct CarrierGroupListResponse {
    pub groups: Vec<CarrierGroupWithMembers>,
    pub total: i64,
}

/// Result of Kamailio sync for carrier groups
#[derive(Debug, Clone, Serialize)]
pub struct CarrierGroupSyncResult {
    pub group_id: Uuid,
    pub group_name: String,
    pub kamailio_group_id: Option<i32>,
    pub success: bool,
    pub error: Option<String>,
}

/// Combined sync result for all outbound routing
#[derive(Debug, Clone, Serialize)]
pub struct OutboundSyncAllResult {
    pub carriers_synced: i32,
    pub carriers_failed: i32,
    pub groups_synced: i32,
    pub groups_failed: i32,
    pub routes_synced: i32,
    pub routes_failed: i32,
    pub kamailio_reloaded: bool,
    pub errors: Vec<String>,
}
