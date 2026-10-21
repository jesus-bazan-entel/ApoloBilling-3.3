//! PBX Group DTOs
//!
//! Failover sets of same-owner IP trunks (mirror of Carrier Groups, but for
//! customer PBXs). L3 routes each owned prefix to the group members in
//! priority order — legitimate same-owner failover, never cross-customer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// PBX group with its members (ordered by priority)
#[derive(Debug, Clone, Serialize)]
pub struct PbxGroupWithMembers {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub strategy: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub members: Vec<PbxGroupMember>,
}

/// One group member (an IP trunk) with its endpoint coordinates
#[derive(Debug, Clone, Serialize)]
pub struct PbxGroupMember {
    pub id: Uuid,
    pub sip_endpoint_id: Uuid,
    pub endpoint_name: String,
    pub host: String,
    pub port: i32,
    pub priority: i32,
}

/// Create group request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreatePbxGroupRequest {
    #[validate(length(min = 1, max = 100, message = "Name must be 1-100 characters"))]
    pub name: String,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

/// Update group request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdatePbxGroupRequest {
    #[validate(length(min = 1, max = 100, message = "Name must be 1-100 characters"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

/// Add member request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AddPbxGroupMemberRequest {
    pub sip_endpoint_id: Uuid,
    pub priority: Option<i32>,
}

/// Update member (priority) request
#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePbxGroupMemberRequest {
    pub priority: Option<i32>,
}
