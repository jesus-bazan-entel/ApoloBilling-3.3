//! Audit log model
//!
//! Tracks all user actions for security and compliance purposes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Audit log entry
///
/// Records user actions for security auditing and compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    /// Unique identifier
    pub id: i64,

    /// User ID (if available)
    pub user_id: Option<i32>,

    /// Username
    pub username: String,

    /// Action performed
    pub action: String,

    /// Entity type affected (e.g., "account", "rate_card", "user")
    pub entity_type: String,

    /// Entity ID (if applicable)
    pub entity_id: Option<String>,

    /// Additional details (JSON)
    pub details: Option<JsonValue>,

    /// IP address of the request
    pub ip_address: Option<String>,

    /// User agent string
    pub user_agent: Option<String>,

    /// Timestamp of the action
    pub created_at: DateTime<Utc>,
}

impl AuditLog {
    /// Create a new audit log builder
    pub fn builder() -> AuditLogBuilder {
        AuditLogBuilder::default()
    }
}

/// Builder for creating audit log entries
#[derive(Debug, Default)]
pub struct AuditLogBuilder {
    user_id: Option<i32>,
    username: Option<String>,
    action: Option<String>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    details: Option<JsonValue>,
    ip_address: Option<String>,
    user_agent: Option<String>,
}

impl AuditLogBuilder {
    pub fn user_id(mut self, user_id: i32) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn entity_type(mut self, entity_type: impl Into<String>) -> Self {
        self.entity_type = Some(entity_type.into());
        self
    }

    pub fn entity_id(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_id = Some(entity_id.into());
        self
    }

    pub fn details(mut self, details: JsonValue) -> Self {
        self.details = Some(details);
        self
    }

    pub fn ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Build the audit log entry (returns data for insertion, not the final entity)
    pub fn build(self) -> Result<AuditLogData, &'static str> {
        Ok(AuditLogData {
            user_id: self.user_id,
            username: self.username.ok_or("username is required")?,
            action: self.action.ok_or("action is required")?,
            entity_type: self.entity_type.ok_or("entity_type is required")?,
            entity_id: self.entity_id,
            details: self.details,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
        })
    }
}

/// Data for creating an audit log entry
#[derive(Debug, Clone)]
pub struct AuditLogData {
    pub user_id: Option<i32>,
    pub username: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub details: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditLogData {
    /// Insert this audit log entry into the database
    ///
    /// This is a convenience method for handlers to quickly log actions.
    /// It swallows errors to avoid breaking the main request flow.
    pub async fn insert(self, pool: &sqlx::PgPool) {
        use sqlx::Row;
        use tracing::warn;

        let result = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                user_id, username, action, entity_type,
                entity_id, details, ip_address, user_agent
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(self.user_id)
        .bind(&self.username)
        .bind(&self.action)
        .bind(&self.entity_type)
        .bind(&self.entity_id)
        .bind(&self.details)
        .bind(&self.ip_address)
        .bind(&self.user_agent)
        .execute(pool)
        .await;

        if let Err(e) = result {
            warn!("Failed to insert audit log: {}", e);
        }
    }
}
