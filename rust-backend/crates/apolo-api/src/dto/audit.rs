//! Audit log DTOs
//!
//! Data Transfer Objects for audit log endpoints.

use apolo_core::models::AuditLog;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Response containing audit log information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogResponse {
    pub id: i64,
    pub user_id: Option<i32>,
    pub username: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub details: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLog> for AuditLogResponse {
    fn from(log: AuditLog) -> Self {
        Self {
            id: log.id,
            user_id: log.user_id,
            username: log.username,
            action: log.action,
            entity_type: log.entity_type,
            entity_id: log.entity_id,
            details: log.details,
            ip_address: log.ip_address,
            user_agent: log.user_agent,
            created_at: log.created_at,
        }
    }
}

/// Query parameters for filtering audit logs
#[derive(Debug, Clone, Deserialize)]
pub struct AuditLogQueryParams {
    /// Filter by username
    pub username: Option<String>,

    /// Filter by action
    pub action: Option<String>,

    /// Filter by entity type
    pub entity_type: Option<String>,

    /// Filter by entity ID
    pub entity_id: Option<String>,

    /// Start date filter (ISO 8601)
    pub start_date: Option<String>,

    /// End date filter (ISO 8601)
    pub end_date: Option<String>,

    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: i64,

    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    50
}

/// Paginated list of audit logs
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogListResponse {
    pub logs: Vec<AuditLogResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}
