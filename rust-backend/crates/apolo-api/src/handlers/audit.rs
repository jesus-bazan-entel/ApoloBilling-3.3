//! Audit log handlers
//!
//! HTTP handlers for audit log queries (superadmin only).

use crate::dto::{ApiResponse, AuditLogListResponse, AuditLogQueryParams, AuditLogResponse};
use actix_web::{web, HttpResponse};
use apolo_auth::middleware::AuthenticatedUser;
use apolo_core::AppError;
use apolo_db::PgAuditLogRepository;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::debug;

/// List audit logs with filters (superadmin only)
pub async fn list_audit_logs(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    query: web::Query<AuditLogQueryParams>,
) -> Result<HttpResponse, AppError> {
    // Only superadmin can view audit logs
    if !user.is_superadmin() {
        return Err(AppError::Forbidden);
    }

    debug!("Listing audit logs");

    let repo = PgAuditLogRepository::new(pool.get_ref().clone());

    let page = query.page.max(1);
    let per_page = query.per_page.min(100).max(1);
    let offset = (page - 1) * per_page;

    // Parse date filters
    let start_date = if let Some(ref sd) = query.start_date {
        Some(sd.parse::<DateTime<Utc>>().map_err(|_| {
            AppError::InvalidInput("Invalid start_date format. Use ISO 8601.".to_string())
        })?)
    } else {
        None
    };

    let end_date = if let Some(ref ed) = query.end_date {
        Some(ed.parse::<DateTime<Utc>>().map_err(|_| {
            AppError::InvalidInput("Invalid end_date format. Use ISO 8601.".to_string())
        })?)
    } else {
        None
    };

    // Get total count
    let total = repo
        .count_with_filters(
            query.username.as_deref(),
            query.action.as_deref(),
            query.entity_type.as_deref(),
            query.entity_id.as_deref(),
            start_date,
            end_date,
        )
        .await?;

    // Get logs
    let logs = repo
        .find_with_filters(
            query.username.as_deref(),
            query.action.as_deref(),
            query.entity_type.as_deref(),
            query.entity_id.as_deref(),
            start_date,
            end_date,
            per_page,
            offset,
        )
        .await?;

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    let response = AuditLogListResponse {
        logs: logs.into_iter().map(AuditLogResponse::from).collect(),
        total,
        page,
        per_page,
        total_pages,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Get audit statistics (superadmin only)
pub async fn get_audit_stats(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    // Only superadmin can view audit stats
    if !user.is_superadmin() {
        return Err(AppError::Forbidden);
    }

    debug!("Getting audit statistics");

    // Get top actions, top users, and recent activity (excluding postgres and DDL operations)
    let top_actions: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT action, COUNT(*) as count
        FROM audit_logs
        WHERE created_at >= NOW() - INTERVAL '30 days'
          AND username != 'postgres'
          AND action NOT IN ('ddl_operation', 'ddl_drop_operation')
        GROUP BY action
        ORDER BY count DESC
        LIMIT 10
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch top actions: {}", e)))?;

    let top_users: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT username, COUNT(*) as count
        FROM audit_logs
        WHERE created_at >= NOW() - INTERVAL '30 days'
          AND username != 'postgres'
          AND action NOT IN ('ddl_operation', 'ddl_drop_operation')
        GROUP BY username
        ORDER BY count DESC
        LIMIT 10
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch top users: {}", e)))?;

    let total_logs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE username != 'postgres' AND action NOT IN ('ddl_operation', 'ddl_drop_operation')"
    )
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to count logs: {}", e)))?;

    let logs_last_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE created_at >= NOW() - INTERVAL '24 hours' AND username != 'postgres' AND action NOT IN ('ddl_operation', 'ddl_drop_operation')",
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to count recent logs: {}", e)))?;

    let response = serde_json::json!({
        "total_logs": total_logs,
        "logs_last_24h": logs_last_24h,
        "top_actions": top_actions.into_iter().map(|(action, count)| {
            serde_json::json!({ "action": action, "count": count })
        }).collect::<Vec<_>>(),
        "top_users": top_users.into_iter().map(|(username, count)| {
            serde_json::json!({ "username": username, "count": count })
        }).collect::<Vec<_>>(),
    });

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Configure audit log routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/audit-logs")
            .route("", web::get().to(list_audit_logs))
            .route("/stats", web::get().to(get_audit_stats)),
    );
}
