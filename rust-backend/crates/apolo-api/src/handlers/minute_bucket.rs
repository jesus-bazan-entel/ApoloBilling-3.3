//! Minute bucket handlers
//!
//! HTTP handlers for the "bolsa de minutos" billing model: plan/account
//! bucket configuration, live cycle status, manual top-ups, cycle history,
//! and the monthly consumption report.

use crate::dto::minute_bucket::{
    AccountBucketConfigRequest, BucketConfigRequest, MinuteBucketCycleEditRequest,
    MinuteBucketReportQuery, MinuteBucketStatusResponse, MinuteBucketTopupRequest,
};
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{AuditLogBuilder, MinuteBucketCycle};
use apolo_core::AppError;
use apolo_db::PgMinuteBucketRepository;
use chrono::{Datelike, NaiveDate, Utc};
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, info, instrument};
use validator::Validate;

fn overage_rate_source(billing_model: &str, flat: Option<rust_decimal::Decimal>, group: &Option<String>) -> String {
    if billing_model != "minute_bucket" {
        return "none".to_string();
    }
    if flat.is_some() {
        return "flat".to_string();
    }
    match group {
        Some(g) => format!("rate_card_group:{}", g),
        None => "standard".to_string(),
    }
}

async fn account_number(pool: &PgPool, account_id: i32) -> Result<String, AppError> {
    sqlx::query_scalar::<_, String>("SELECT account_number FROM accounts WHERE id = $1")
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to read account: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))
}

fn cycle_to_status(
    cycle: MinuteBucketCycle,
    account_id: i32,
    acc_number: String,
    billing_model: String,
    bucket_type: Option<String>,
    overage_rate_source_str: String,
) -> MinuteBucketStatusResponse {
    let total = rust_decimal::Decimal::from(cycle.included_minutes + cycle.additional_minutes);
    let available = total - cycle.consumed_minutes - cycle.reserved_minutes;
    let utilization = cycle.utilization_percent();
    let is_blocked = bucket_type.as_deref() == Some("control") && available <= rust_decimal::Decimal::ZERO;

    MinuteBucketStatusResponse {
        account_id,
        account_number: acc_number,
        billing_model,
        bucket_type,
        cycle_start: cycle.cycle_start,
        cycle_end: cycle.cycle_end,
        included_minutes: cycle.included_minutes,
        is_prorated: cycle.is_prorated,
        additional_minutes: cycle.additional_minutes,
        consumed_minutes: cycle.consumed_minutes,
        overage_minutes: cycle.overage_minutes,
        overage_cost: cycle.overage_cost,
        available_minutes: available,
        utilization_percent: utilization,
        is_blocked,
        overage_rate_source: overage_rate_source_str,
        status: cycle.status,
    }
}

// ============================================================
// Plan bucket config
// ============================================================

/// GET /api/v1/plans/{id}/bucket-config
#[instrument(skip(pool, user))]
pub async fn get_plan_bucket_config(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("plans") {
        return Err(AppError::Forbidden);
    }

    let plan_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let cfg = repo
        .get_plan_bucket_config(plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {} not found", plan_id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(cfg)))
}

/// PUT /api/v1/plans/{id}/bucket-config
#[instrument(skip(pool, admin, req))]
pub async fn update_plan_bucket_config(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
    req: web::Json<BucketConfigRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    req.validate_business().map_err(AppError::Validation)?;

    let plan_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let updated = repo
        .update_plan_bucket_config(plan_id, &req.to_config())
        .await?;

    info!(id = plan_id, "Plan bucket config updated");

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_plan_bucket_config")
        .entity_type("plan")
        .entity_id(plan_id.to_string())
        .details(json!({ "billing_model": req.billing_model, "bucket_type": req.bucket_type }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        updated,
        "Plan bucket config updated",
    )))
}

// ============================================================
// Account bucket config
// ============================================================

/// GET /api/v1/accounts/{id}/bucket-config
#[instrument(skip(pool, user))]
pub async fn get_account_bucket_config(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("minute_buckets") && !user.has_module("accounts") {
        return Err(AppError::Forbidden);
    }

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let cfg = repo
        .get_account_bucket_config(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(cfg)))
}

/// PUT /api/v1/accounts/{id}/bucket-config
#[instrument(skip(pool, admin, req))]
pub async fn update_account_bucket_config(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
    req: web::Json<AccountBucketConfigRequest>,
) -> Result<HttpResponse, AppError> {
    req.config
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    req.config.validate_business().map_err(AppError::Validation)?;

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let updated = repo
        .update_account_bucket_config(
            account_id,
            &req.config.to_config(),
            req.notification_email.as_deref(),
        )
        .await?;

    info!(id = account_id, "Account bucket config updated");

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_account_bucket_config")
        .entity_type("account")
        .entity_id(account_id.to_string())
        .details(json!({
            "billing_model": req.config.billing_model,
            "bucket_type": req.config.bucket_type,
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        updated,
        "Account bucket config updated",
    )))
}

// ============================================================
// Live cycle status / manual override / top-up / history
// ============================================================

/// GET /api/v1/accounts/{id}/minute-bucket
#[instrument(skip(pool, user))]
pub async fn get_minute_bucket_status(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("minute_buckets") && !user.has_module("accounts") {
        return Err(AppError::Forbidden);
    }

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());

    let cfg = repo
        .get_account_bucket_config(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    if !matches!(cfg.config.billing_model, apolo_core::models::BillingModel::MinuteBucket) {
        return Err(AppError::Validation(
            "Account is not on the minute_bucket billing model".to_string(),
        ));
    }

    let cycle = repo.ensure_open_cycle(account_id).await?;
    let acc_number = account_number(pool.get_ref(), account_id).await?;
    let source = overage_rate_source(
        "minute_bucket",
        cfg.config.overage_flat_rate_per_minute,
        &cfg.config.overage_tariff_group,
    );

    let status = cycle_to_status(
        cycle,
        account_id,
        acc_number,
        "minute_bucket".to_string(),
        cfg.config.bucket_type.map(|b| b.to_string()),
        source,
    );

    Ok(HttpResponse::Ok().json(ApiResponse::success(status)))
}

/// PUT /api/v1/accounts/{id}/minute-bucket — manual override of the open
/// cycle's included_minutes (e.g. adjusting a prorated suggestion by hand).
#[instrument(skip(pool, admin, req))]
pub async fn update_minute_bucket_cycle(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
    req: web::Json<MinuteBucketCycleEditRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());

    let cfg = repo
        .get_account_bucket_config(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    let cycle = repo
        .set_cycle_included_minutes(account_id, req.included_minutes)
        .await?;

    info!(
        id = account_id,
        included_minutes = req.included_minutes,
        "Bucket cycle manually overridden"
    );

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("override_bucket_cycle")
        .entity_type("account")
        .entity_id(account_id.to_string())
        .details(json!({
            "included_minutes": req.included_minutes,
            "reason": req.reason,
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let acc_number = account_number(pool.get_ref(), account_id).await?;
    let source = overage_rate_source(
        &cfg.config.billing_model.to_string(),
        cfg.config.overage_flat_rate_per_minute,
        &cfg.config.overage_tariff_group,
    );
    let status = cycle_to_status(
        cycle,
        account_id,
        acc_number,
        cfg.config.billing_model.to_string(),
        cfg.config.bucket_type.map(|b| b.to_string()),
        source,
    );

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        status,
        "Bucket cycle updated",
    )))
}

/// POST /api/v1/accounts/{id}/minute-bucket/topup
#[instrument(skip(pool, admin, req))]
pub async fn topup_minute_bucket(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
    req: web::Json<MinuteBucketTopupRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());

    let addition = repo
        .add_topup(
            account_id,
            req.minutes_added,
            req.reason.as_deref(),
            &admin.username,
        )
        .await?;

    info!(
        id = account_id,
        minutes = req.minutes_added,
        "Bolsa adicional registrada"
    );

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("topup_minute_bucket")
        .entity_type("account")
        .entity_id(account_id.to_string())
        .details(json!({ "minutes_added": req.minutes_added, "reason": req.reason }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        addition,
        "Bolsa adicional registrada — el trafico se reactiva en el proximo intento de llamada",
    )))
}

/// GET /api/v1/accounts/{id}/minute-bucket/history
#[instrument(skip(pool, user))]
pub async fn minute_bucket_history(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("minute_buckets") && !user.has_module("accounts") {
        return Err(AppError::Forbidden);
    }

    let account_id = path.into_inner();
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let cycles = repo.history(account_id).await?;

    #[derive(serde::Serialize)]
    struct CycleHistoryEntry {
        #[serde(flatten)]
        cycle: MinuteBucketCycle,
        additions: Vec<apolo_core::models::MinuteBucketAddition>,
    }

    let mut entries = Vec::with_capacity(cycles.len());
    for cycle in cycles {
        let additions = repo.additions_for_cycle(cycle.id).await?;
        entries.push(CycleHistoryEntry { cycle, additions });
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(entries)))
}

// ============================================================
// Monthly report
// ============================================================

fn parse_report_month(month: &Option<String>) -> Result<NaiveDate, AppError> {
    match month {
        None => Ok(Utc::now().date_naive()),
        Some(s) => {
            if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                Ok(d)
            } else if let Ok(d) = NaiveDate::parse_from_str(&format!("{}-01", s), "%Y-%m-%d") {
                Ok(d)
            } else {
                Err(AppError::Validation(format!(
                    "Invalid month '{}', expected YYYY-MM or YYYY-MM-DD",
                    s
                )))
            }
        }
    }
}

/// GET /api/v1/reports/minute-bucket
#[instrument(skip(pool, user))]
pub async fn minute_bucket_report(
    pool: web::Data<PgPool>,
    query: web::Query<MinuteBucketReportQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("minute_buckets") {
        return Err(AppError::Forbidden);
    }

    let month = parse_report_month(&query.month)?;
    debug!(month = %month, "Building minute bucket report");

    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let rows = repo
        .monthly_report(month, query.account_id, query.bucket_type.as_deref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(rows)))
}

/// GET /api/v1/reports/minute-bucket/export
#[instrument(skip(pool, user))]
pub async fn minute_bucket_report_export(
    pool: web::Data<PgPool>,
    query: web::Query<MinuteBucketReportQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("minute_buckets") {
        return Err(AppError::Forbidden);
    }

    let month = parse_report_month(&query.month)?;
    let repo = PgMinuteBucketRepository::new(pool.get_ref().clone());
    let rows = repo
        .monthly_report(month, query.account_id, query.bucket_type.as_deref())
        .await?;

    let mut csv = String::from(
        "account_number,account_name,bucket_type,included_minutes,is_prorated,additional_minutes,additions_count,consumed_minutes,overage_minutes,overage_cost,utilization_percent,cycle_status,is_blocked\n",
    );
    for r in &rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{:.2},{},{}\n",
            r.account_number,
            r.account_name.clone().unwrap_or_default().replace(',', " "),
            r.bucket_type.clone().unwrap_or_default(),
            r.included_minutes,
            r.is_prorated,
            r.additional_minutes,
            r.additions_count,
            r.consumed_minutes,
            r.overage_minutes,
            r.overage_cost,
            r.utilization_percent,
            r.cycle_status,
            r.is_blocked,
        ));
    }

    let filename = format!(
        "bolsas_minutos_{}{:02}.csv",
        month.year(),
        month.month()
    );

    Ok(HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        ))
        .body(csv))
}

/// Configure minute bucket routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/plans")
            .route("/{id}/bucket-config", web::get().to(get_plan_bucket_config))
            .route("/{id}/bucket-config", web::put().to(update_plan_bucket_config)),
    )
    .service(
        web::scope("/accounts")
            .route(
                "/{id}/bucket-config",
                web::get().to(get_account_bucket_config),
            )
            .route(
                "/{id}/bucket-config",
                web::put().to(update_account_bucket_config),
            )
            .route(
                "/{id}/minute-bucket",
                web::get().to(get_minute_bucket_status),
            )
            .route(
                "/{id}/minute-bucket",
                web::put().to(update_minute_bucket_cycle),
            )
            .route(
                "/{id}/minute-bucket/topup",
                web::post().to(topup_minute_bucket),
            )
            .route(
                "/{id}/minute-bucket/history",
                web::get().to(minute_bucket_history),
            ),
    )
    .service(
        web::scope("/reports").route(
            "/minute-bucket",
            web::get().to(minute_bucket_report),
        ),
    )
    .route(
        "/reports/minute-bucket/export",
        web::get().to(minute_bucket_report_export),
    );
}
