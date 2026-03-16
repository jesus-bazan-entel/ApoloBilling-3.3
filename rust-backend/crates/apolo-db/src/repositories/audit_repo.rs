//! Audit log repository implementation
//!
//! Provides PostgreSQL-backed storage for audit logs.

use apolo_core::{models::{AuditLog, AuditLogData}, AppError, AppResult};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use tracing::{debug, error, instrument};

/// PostgreSQL implementation of AuditLog repository
pub struct PgAuditLogRepository {
    pool: PgPool,
}

impl PgAuditLogRepository {
    /// Create a new audit log repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new audit log entry
    #[instrument(skip(self, data))]
    pub async fn create(&self, data: AuditLogData) -> AppResult<AuditLog> {
        debug!("Creating audit log: {} on {}", data.action, data.entity_type);

        let row = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                user_id, username, action, entity_type,
                entity_id, details, ip_address, user_agent
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id, user_id, username, action, entity_type,
                entity_id, details, ip_address, user_agent, created_at
            "#,
        )
        .bind(data.user_id)
        .bind(&data.username)
        .bind(&data.action)
        .bind(&data.entity_type)
        .bind(&data.entity_id)
        .bind(&data.details)
        .bind(&data.ip_address)
        .bind(&data.user_agent)
        .map(|row: sqlx::postgres::PgRow| AuditLog {
            id: row.get("id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            action: row.get("action"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            details: row.get("details"),
            ip_address: row.get("ip_address"),
            user_agent: row.get("user_agent"),
            created_at: row.get("created_at"),
        })
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating audit log: {}", e);
            AppError::Database(format!("Failed to create audit log: {}", e))
        })?;

        Ok(row)
    }

    /// Find audit logs with filters and pagination
    #[instrument(skip(self))]
    pub async fn find_with_filters(
        &self,
        username: Option<&str>,
        action: Option<&str>,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AuditLog>> {
        debug!("Finding audit logs with filters");

        let mut query = String::from(
            r#"
            SELECT
                id, user_id, username, action, entity_type,
                entity_id, details, ip_address, user_agent, created_at
            FROM audit_logs
            WHERE username != 'postgres'
              AND action NOT IN ('ddl_operation', 'ddl_drop_operation')
            "#,
        );

        let mut bind_count = 0;
        let mut bindings: Vec<Box<dyn std::fmt::Display + Send>> = Vec::new();

        if let Some(u) = username {
            bind_count += 1;
            query.push_str(&format!(" AND username = ${}", bind_count));
            bindings.push(Box::new(u.to_string()));
        }

        if let Some(a) = action {
            bind_count += 1;
            query.push_str(&format!(" AND action = ${}", bind_count));
            bindings.push(Box::new(a.to_string()));
        }

        if let Some(et) = entity_type {
            bind_count += 1;
            query.push_str(&format!(" AND entity_type = ${}", bind_count));
            bindings.push(Box::new(et.to_string()));
        }

        if let Some(eid) = entity_id {
            bind_count += 1;
            query.push_str(&format!(" AND entity_id = ${}", bind_count));
            bindings.push(Box::new(eid.to_string()));
        }

        if let Some(sd) = start_date {
            bind_count += 1;
            query.push_str(&format!(" AND created_at >= ${}", bind_count));
        }

        if let Some(ed) = end_date {
            bind_count += 1;
            query.push_str(&format!(" AND created_at <= ${}", bind_count));
        }

        query.push_str(" ORDER BY created_at DESC");
        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));
        bind_count += 1;
        query.push_str(&format!(" OFFSET ${}", bind_count));

        let mut q = sqlx::query(&query);

        if let Some(u) = username {
            q = q.bind(u);
        }
        if let Some(a) = action {
            q = q.bind(a);
        }
        if let Some(et) = entity_type {
            q = q.bind(et);
        }
        if let Some(eid) = entity_id {
            q = q.bind(eid);
        }
        if let Some(sd) = start_date {
            q = q.bind(sd);
        }
        if let Some(ed) = end_date {
            q = q.bind(ed);
        }

        q = q.bind(limit).bind(offset);

        let rows = q
            .map(|row: sqlx::postgres::PgRow| AuditLog {
                id: row.get("id"),
                user_id: row.get("user_id"),
                username: row.get("username"),
                action: row.get("action"),
                entity_type: row.get("entity_type"),
                entity_id: row.get("entity_id"),
                details: row.get("details"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                created_at: row.get("created_at"),
            })
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding audit logs: {}", e);
                AppError::Database(format!("Failed to fetch audit logs: {}", e))
            })?;

        Ok(rows)
    }

    /// Count audit logs with filters
    #[instrument(skip(self))]
    pub async fn count_with_filters(
        &self,
        username: Option<&str>,
        action: Option<&str>,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM audit_logs
            WHERE username != 'postgres'
              AND action NOT IN ('ddl_operation', 'ddl_drop_operation')
              AND ($1::TEXT IS NULL OR username = $1)
              AND ($2::TEXT IS NULL OR action = $2)
              AND ($3::TEXT IS NULL OR entity_type = $3)
              AND ($4::TEXT IS NULL OR entity_id = $4)
              AND ($5::TIMESTAMPTZ IS NULL OR created_at >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR created_at <= $6)
            "#,
        )
        .bind(username)
        .bind(action)
        .bind(entity_type)
        .bind(entity_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error counting audit logs: {}", e);
            AppError::Database(format!("Failed to count audit logs: {}", e))
        })?;

        Ok(count)
    }
}
