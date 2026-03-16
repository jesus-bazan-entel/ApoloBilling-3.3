//! Management handlers
//!
//! HTTP handlers for zone, prefix, and tariff management endpoints.
//! These endpoints sync changes to the rate_cards table for billing.

use crate::dto::management::{
    PrefixCreateRequest, PrefixResponse, SyncResponse, TariffCreateRequest, TariffResponse,
    TariffUpdateRequest, ZoneCreateRequest, ZoneResponse, ZoneUpdateRequest,
};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{AuditLogBuilder, NetworkType, ZoneType};
use apolo_core::AppError;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use tracing::{debug, info, instrument, warn};
use validator::Validate;

// ============================================================================
// Zone Handlers
// ============================================================================

/// List all zones
///
/// GET /api/v1/zonas
#[instrument(skip(pool, _user))]
pub async fn list_zones(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(page = query.page, per_page = query.per_page, "Listing zones");

    let rows = sqlx::query(
        r#"
        SELECT z.*,
               (SELECT COUNT(*) FROM prefixes p WHERE p.zone_id = z.id) as prefix_count,
               (SELECT COUNT(*) FROM rate_zones rz WHERE rz.zone_id = z.id) as tariff_count
        FROM zones z
        ORDER BY z.zone_name
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(query.limit() as i64)
    .bind(query.offset() as i64)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to list zones: {}", e)))?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zones")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to count zones: {}", e)))?;

    let zones: Vec<ZoneResponse> = rows
        .iter()
        .map(|row| ZoneResponse {
            id: row.get("id"),
            zone_name: row.get("zone_name"),
            zone_code: row.get("zone_code"),
            description: row.get("description"),
            zone_type: row
                .get::<Option<String>, _>("zone_type")
                .unwrap_or_else(|| "GEOGRAPHIC".to_string()),
            region_name: row.get("region_name"),
            country_id: row.get("country_id"),
            enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
            prefix_count: row.get::<Option<i64>, _>("prefix_count"),
            tariff_count: row.get::<Option<i64>, _>("tariff_count"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(HttpResponse::Ok().json(query.paginate(zones, total)))
}

/// Create a new zone
///
/// POST /api/v1/zonas
#[instrument(skip(pool, _user, req))]
pub async fn create_zone(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<ZoneCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Zone creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(zone_name = %req.zone_name, "Creating zone");

    let zone = req.to_zone();
    let now = Utc::now();

    let result = sqlx::query(
        r#"
        INSERT INTO zones (zone_name, zone_code, description, zone_type, region_name, country_id, enabled, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(&zone.zone_name)
    .bind(&zone.zone_code)
    .bind(&zone.description)
    .bind(zone.zone_type.to_string())
    .bind(&zone.region_name)
    .bind(zone.country_id)
    .bind(zone.enabled)
    .bind(now)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Conflict(format!("Zone '{}' already exists", req.zone_name))
        } else {
            AppError::Database(format!("Failed to create zone: {}", e))
        }
    })?;

    let id: i32 = result.get("id");

    info!(id = id, zone_name = %req.zone_name, "Zone created successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "zone_name": req.zone_name,
        "zone_code": req.zone_code,
        "zone_type": req.zone_type,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("create_zone")
        .entity_type("zone")
        .entity_id(id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    let response = ZoneResponse {
        id,
        zone_name: zone.zone_name,
        zone_code: zone.zone_code,
        description: zone.description,
        zone_type: zone.zone_type.to_string(),
        region_name: zone.region_name,
        country_id: zone.country_id,
        enabled: zone.enabled,
        prefix_count: Some(0),
        tariff_count: Some(0),
        created_at: now,
        updated_at: now,
    };

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Zone created successfully",
    )))
}

/// Get a zone by ID
///
/// GET /api/v1/zonas/{id}
#[instrument(skip(pool, _user))]
pub async fn get_zone(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let zone_id = path.into_inner();
    debug!(id = zone_id, "Getting zone");

    let row = sqlx::query(
        r#"
        SELECT z.*,
               (SELECT COUNT(*) FROM prefixes p WHERE p.zone_id = z.id) as prefix_count,
               (SELECT COUNT(*) FROM rate_zones rz WHERE rz.zone_id = z.id) as tariff_count
        FROM zones z
        WHERE z.id = $1
        "#,
    )
    .bind(zone_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to get zone: {}", e)))?
    .ok_or_else(|| AppError::NotFound(format!("Zone {} not found", zone_id)))?;

    let response = ZoneResponse {
        id: row.get("id"),
        zone_name: row.get("zone_name"),
        zone_code: row.get("zone_code"),
        description: row.get("description"),
        zone_type: row
            .get::<Option<String>, _>("zone_type")
            .unwrap_or_else(|| "GEOGRAPHIC".to_string()),
        region_name: row.get("region_name"),
        country_id: row.get("country_id"),
        enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
        prefix_count: row.get::<Option<i64>, _>("prefix_count"),
        tariff_count: row.get::<Option<i64>, _>("tariff_count"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Update a zone
///
/// PUT /api/v1/zonas/{id}
#[instrument(skip(pool, _user, req))]
pub async fn update_zone(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
    req: web::Json<ZoneUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Zone update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let zone_id = path.into_inner();
    debug!(id = zone_id, "Updating zone");

    // Verify zone exists
    let existing = sqlx::query("SELECT id FROM zones WHERE id = $1")
        .bind(zone_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to check zone: {}", e)))?;

    if existing.is_none() {
        return Err(AppError::NotFound(format!("Zone {} not found", zone_id)));
    }

    let now = Utc::now();

    // Build dynamic update
    let mut updates = vec!["updated_at = $1".to_string()];
    let mut param_idx = 2;

    if req.zone_name.is_some() {
        updates.push(format!("zone_name = ${}", param_idx));
        param_idx += 1;
    }
    if req.zone_code.is_some() {
        updates.push(format!("zone_code = ${}", param_idx));
        param_idx += 1;
    }
    if req.description.is_some() {
        updates.push(format!("description = ${}", param_idx));
        param_idx += 1;
    }
    if req.zone_type.is_some() {
        updates.push(format!("zone_type = ${}", param_idx));
        param_idx += 1;
    }
    if req.region_name.is_some() {
        updates.push(format!("region_name = ${}", param_idx));
        param_idx += 1;
    }
    if req.enabled.is_some() {
        updates.push(format!("enabled = ${}", param_idx));
        param_idx += 1;
    }

    let query_str = format!(
        "UPDATE zones SET {} WHERE id = ${} RETURNING *",
        updates.join(", "),
        param_idx
    );

    let mut query = sqlx::query(&query_str).bind(now);

    if let Some(ref name) = req.zone_name {
        query = query.bind(name);
    }
    if let Some(ref code) = req.zone_code {
        query = query.bind(code);
    }
    if let Some(ref desc) = req.description {
        query = query.bind(desc);
    }
    if let Some(ref zt) = req.zone_type {
        query = query.bind(zt);
    }
    if let Some(ref region) = req.region_name {
        query = query.bind(region);
    }
    if let Some(enabled) = req.enabled {
        query = query.bind(enabled);
    }

    query = query.bind(zone_id);

    let row = query
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to update zone: {}", e)))?;

    info!(id = zone_id, "Zone updated successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "zone_name": req.zone_name,
        "enabled": req.enabled,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("update_zone")
        .entity_type("zone")
        .entity_id(zone_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    let response = ZoneResponse {
        id: row.get("id"),
        zone_name: row.get("zone_name"),
        zone_code: row.get("zone_code"),
        description: row.get("description"),
        zone_type: row
            .get::<Option<String>, _>("zone_type")
            .unwrap_or_else(|| "GEOGRAPHIC".to_string()),
        region_name: row.get("region_name"),
        country_id: row.get("country_id"),
        enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
        prefix_count: None,
        tariff_count: None,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Zone updated successfully",
    )))
}

/// Delete a zone
///
/// DELETE /api/v1/zonas/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_zone(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    let zone_id = path.into_inner();
    debug!(id = zone_id, admin = %admin.username, "Deleting zone");

    // Get zone info before deleting
    let zone_name: String = sqlx::query_scalar("SELECT zone_name FROM zones WHERE id = $1")
        .bind(zone_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to get zone: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Zone {} not found", zone_id)))?;

    // Check if zone has prefixes or tariffs
    let prefix_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prefixes WHERE zone_id = $1")
        .bind(zone_id)
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to check prefixes: {}", e)))?;

    if prefix_count > 0 {
        return Err(AppError::Validation(format!(
            "Cannot delete zone with {} prefixes. Delete prefixes first.",
            prefix_count
        )));
    }

    let tariff_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM rate_zones WHERE zone_id = $1")
            .bind(zone_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| AppError::Database(format!("Failed to check tariffs: {}", e)))?;

    if tariff_count > 0 {
        return Err(AppError::Validation(format!(
            "Cannot delete zone with {} tariffs. Delete tariffs first.",
            tariff_count
        )));
    }

    let result = sqlx::query("DELETE FROM zones WHERE id = $1")
        .bind(zone_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete zone: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Zone {} not found", zone_id)));
    }

    info!(id = zone_id, admin = %admin.username, "Zone deleted successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "zone_name": zone_name,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_zone")
        .entity_type("zone")
        .entity_id(zone_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    Ok(HttpResponse::NoContent().finish())
}

// ============================================================================
// Prefix Handlers
// ============================================================================

/// List prefixes for a zone
///
/// GET /api/v1/prefijos?zone_id=X
#[instrument(skip(pool, _user))]
pub async fn list_prefixes(
    pool: web::Data<PgPool>,
    query: web::Query<PrefixFilterParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!(zone_id = ?query.zone_id, "Listing prefixes");

    let rows = if let Some(zone_id) = query.zone_id {
        sqlx::query(
            r#"
            SELECT p.*, z.zone_name
            FROM prefixes p
            JOIN zones z ON z.id = p.zone_id
            WHERE p.zone_id = $1
            ORDER BY p.prefix
            "#,
        )
        .bind(zone_id)
        .fetch_all(pool.get_ref())
        .await
    } else {
        sqlx::query(
            r#"
            SELECT p.*, z.zone_name
            FROM prefixes p
            JOIN zones z ON z.id = p.zone_id
            ORDER BY p.prefix
            LIMIT 1000
            "#,
        )
        .fetch_all(pool.get_ref())
        .await
    }
    .map_err(|e| AppError::Database(format!("Failed to list prefixes: {}", e)))?;

    let prefixes: Vec<PrefixResponse> = rows
        .iter()
        .map(|row| PrefixResponse {
            id: row.get("id"),
            zone_id: row.get("zone_id"),
            zone_name: row.get("zone_name"),
            prefix: row.get("prefix"),
            prefix_length: row.get::<Option<i32>, _>("prefix_length").unwrap_or(0),
            operator_name: row.get("operator_name"),
            network_type: row
                .get::<Option<String>, _>("network_type")
                .unwrap_or_else(|| "FIXED".to_string()),
            enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(prefixes)))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrefixFilterParams {
    pub zone_id: Option<i32>,
}

/// Create a new prefix
///
/// POST /api/v1/prefijos
#[instrument(skip(pool, _user, req))]
pub async fn create_prefix(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<PrefixCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Prefix creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(prefix = %req.prefix, zone_id = req.zone_id, "Creating prefix");

    // Verify zone exists
    let zone_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zones WHERE id = $1)")
            .bind(req.zone_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| AppError::Database(format!("Failed to check zone: {}", e)))?;

    if !zone_exists {
        return Err(AppError::NotFound(format!(
            "Zone {} not found",
            req.zone_id
        )));
    }

    let prefix = req.to_prefix();
    let now = Utc::now();

    let result = sqlx::query(
        r#"
        INSERT INTO prefixes (zone_id, prefix, prefix_length, operator_name, network_type, enabled, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(prefix.zone_id)
    .bind(&prefix.prefix)
    .bind(prefix.prefix_length)
    .bind(&prefix.operator_name)
    .bind(prefix.network_type.to_string())
    .bind(prefix.enabled)
    .bind(now)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Conflict(format!("Prefix '{}' already exists", req.prefix))
        } else {
            AppError::Database(format!("Failed to create prefix: {}", e))
        }
    })?;

    let id: i32 = result.get("id");

    info!(id = id, prefix = %req.prefix, "Prefix created successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "prefix": req.prefix,
        "zone_id": req.zone_id,
        "network_type": req.network_type,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("create_prefix")
        .entity_type("prefix")
        .entity_id(id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    let response = PrefixResponse {
        id,
        zone_id: prefix.zone_id,
        zone_name: None,
        prefix: prefix.prefix,
        prefix_length: prefix.prefix_length,
        operator_name: prefix.operator_name,
        network_type: prefix.network_type.to_string(),
        enabled: prefix.enabled,
        created_at: now,
        updated_at: now,
    };

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Prefix created successfully",
    )))
}

/// Delete a prefix
///
/// DELETE /api/v1/prefijos/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_prefix(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    let prefix_id = path.into_inner();
    debug!(id = prefix_id, admin = %admin.username, "Deleting prefix");

    // Get prefix info before deleting
    let prefix: String = sqlx::query_scalar("SELECT prefix FROM prefixes WHERE id = $1")
        .bind(prefix_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to get prefix: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Prefix {} not found", prefix_id)))?;

    let result = sqlx::query("DELETE FROM prefixes WHERE id = $1")
        .bind(prefix_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete prefix: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Prefix {} not found",
            prefix_id
        )));
    }

    info!(id = prefix_id, admin = %admin.username, "Prefix deleted successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "prefix": prefix,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_prefix")
        .entity_type("prefix")
        .entity_id(prefix_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    Ok(HttpResponse::NoContent().finish())
}

// ============================================================================
// Tariff Handlers
// ============================================================================

/// List tariffs for a zone
///
/// GET /api/v1/tarifas?zone_id=X
#[instrument(skip(pool, _user))]
pub async fn list_tariffs(
    pool: web::Data<PgPool>,
    query: web::Query<TariffFilterParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!(zone_id = ?query.zone_id, "Listing tariffs");

    let rows = if let Some(zone_id) = query.zone_id {
        sqlx::query(
            r#"
            SELECT rz.*, z.zone_name
            FROM rate_zones rz
            JOIN zones z ON z.id = rz.zone_id
            WHERE rz.zone_id = $1
            ORDER BY rz.priority DESC, rz.rate_name
            "#,
        )
        .bind(zone_id)
        .fetch_all(pool.get_ref())
        .await
    } else {
        sqlx::query(
            r#"
            SELECT rz.*, z.zone_name
            FROM rate_zones rz
            JOIN zones z ON z.id = rz.zone_id
            ORDER BY rz.priority DESC, rz.rate_name
            LIMIT 1000
            "#,
        )
        .fetch_all(pool.get_ref())
        .await
    }
    .map_err(|e| AppError::Database(format!("Failed to list tariffs: {}", e)))?;

    let tariffs: Vec<TariffResponse> = rows
        .iter()
        .map(|row| {
            let rpm: Decimal = row.get("rate_per_minute");
            TariffResponse {
                id: row.get("id"),
                zone_id: row.get("zone_id"),
                zone_name: row.get("zone_name"),
                rate_name: row.get("rate_name"),
                rate_per_minute: rpm,
                rate_per_second: rpm / Decimal::from(60),
                rate_per_call: row
                    .get::<Option<Decimal>, _>("rate_per_call")
                    .unwrap_or(Decimal::ZERO),
                billing_increment: row.get::<Option<i32>, _>("billing_increment").unwrap_or(6),
                min_duration: row.get::<Option<i32>, _>("min_duration").unwrap_or(0),
                effective_from: row.get("effective_from"),
                currency: row
                    .get::<Option<String>, _>("currency")
                    .unwrap_or_else(|| "USD".to_string()),
                priority: row.get::<Option<i32>, _>("priority").unwrap_or(0),
                enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(tariffs)))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TariffFilterParams {
    pub zone_id: Option<i32>,
}

/// Create a new tariff
///
/// POST /api/v1/tarifas
#[instrument(skip(pool, _user, req))]
pub async fn create_tariff(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<TariffCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Tariff creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(zone_id = req.zone_id, rate = %req.rate_per_minute, "Creating tariff");

    // Verify zone exists
    let zone_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zones WHERE id = $1)")
            .bind(req.zone_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| AppError::Database(format!("Failed to check zone: {}", e)))?;

    if !zone_exists {
        return Err(AppError::NotFound(format!(
            "Zone {} not found",
            req.zone_id
        )));
    }

    let rate = req.to_rate_zone();
    let now = Utc::now();

    let result = sqlx::query(
        r#"
        INSERT INTO rate_zones (zone_id, rate_name, rate_per_minute, rate_per_call, billing_increment, min_duration, effective_from, currency, priority, enabled, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(rate.zone_id)
    .bind(&rate.rate_name)
    .bind(rate.rate_per_minute)
    .bind(rate.rate_per_call)
    .bind(rate.billing_increment)
    .bind(rate.min_duration)
    .bind(now)
    .bind(&rate.currency)
    .bind(rate.priority)
    .bind(rate.enabled)
    .bind(now)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to create tariff: {}", e)))?;

    let id: i32 = result.get("id");

    info!(id = id, zone_id = req.zone_id, "Tariff created successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "zone_id": req.zone_id,
        "rate_name": req.rate_name,
        "rate_per_minute": req.rate_per_minute,
        "billing_increment": req.billing_increment,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("create_tariff")
        .entity_type("tariff")
        .entity_id(id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    let rate_per_second = rate.rate_per_second();
    let response = TariffResponse {
        id,
        zone_id: rate.zone_id,
        zone_name: None,
        rate_name: rate.rate_name,
        rate_per_minute: rate.rate_per_minute,
        rate_per_second,
        rate_per_call: rate.rate_per_call,
        billing_increment: rate.billing_increment,
        min_duration: rate.min_duration,
        effective_from: now,
        currency: rate.currency,
        priority: rate.priority,
        enabled: rate.enabled,
        created_at: now,
        updated_at: now,
    };

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Tariff created successfully",
    )))
}

/// Update a tariff
///
/// PUT /api/v1/tarifas/{id}
#[instrument(skip(pool, _user, req))]
pub async fn update_tariff(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
    req: web::Json<TariffUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Tariff update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let tariff_id = path.into_inner();
    debug!(id = tariff_id, "Updating tariff");

    // Verify tariff exists
    let existing = sqlx::query("SELECT id FROM rate_zones WHERE id = $1")
        .bind(tariff_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to check tariff: {}", e)))?;

    if existing.is_none() {
        return Err(AppError::NotFound(format!(
            "Tariff {} not found",
            tariff_id
        )));
    }

    let now = Utc::now();

    // Build dynamic update
    let mut updates = vec!["updated_at = $1".to_string()];
    let mut param_idx = 2;

    if req.rate_name.is_some() {
        updates.push(format!("rate_name = ${}", param_idx));
        param_idx += 1;
    }
    if req.rate_per_minute.is_some() {
        updates.push(format!("rate_per_minute = ${}", param_idx));
        param_idx += 1;
    }
    if req.rate_per_call.is_some() {
        updates.push(format!("rate_per_call = ${}", param_idx));
        param_idx += 1;
    }
    if req.billing_increment.is_some() {
        updates.push(format!("billing_increment = ${}", param_idx));
        param_idx += 1;
    }
    if req.min_duration.is_some() {
        updates.push(format!("min_duration = ${}", param_idx));
        param_idx += 1;
    }
    if req.currency.is_some() {
        updates.push(format!("currency = ${}", param_idx));
        param_idx += 1;
    }
    if req.priority.is_some() {
        updates.push(format!("priority = ${}", param_idx));
        param_idx += 1;
    }
    if req.enabled.is_some() {
        updates.push(format!("enabled = ${}", param_idx));
        param_idx += 1;
    }

    let query_str = format!(
        "UPDATE rate_zones SET {} WHERE id = ${} RETURNING *",
        updates.join(", "),
        param_idx
    );

    let mut query = sqlx::query(&query_str).bind(now);

    if let Some(ref name) = req.rate_name {
        query = query.bind(name);
    }
    if let Some(rpm) = req.rate_per_minute {
        query = query.bind(rpm);
    }
    if let Some(rpc) = req.rate_per_call {
        query = query.bind(rpc);
    }
    if let Some(bi) = req.billing_increment {
        query = query.bind(bi);
    }
    if let Some(md) = req.min_duration {
        query = query.bind(md);
    }
    if let Some(ref curr) = req.currency {
        query = query.bind(curr);
    }
    if let Some(prio) = req.priority {
        query = query.bind(prio);
    }
    if let Some(enabled) = req.enabled {
        query = query.bind(enabled);
    }

    query = query.bind(tariff_id);

    let row = query
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to update tariff: {}", e)))?;

    info!(id = tariff_id, "Tariff updated successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "rate_name": req.rate_name,
        "rate_per_minute": req.rate_per_minute,
        "enabled": req.enabled,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("update_tariff")
        .entity_type("tariff")
        .entity_id(tariff_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    let rpm: Decimal = row.get("rate_per_minute");
    let response = TariffResponse {
        id: row.get("id"),
        zone_id: row.get("zone_id"),
        zone_name: None,
        rate_name: row.get("rate_name"),
        rate_per_minute: rpm,
        rate_per_second: rpm / Decimal::from(60),
        rate_per_call: row
            .get::<Option<Decimal>, _>("rate_per_call")
            .unwrap_or(Decimal::ZERO),
        billing_increment: row.get::<Option<i32>, _>("billing_increment").unwrap_or(6),
        min_duration: row.get::<Option<i32>, _>("min_duration").unwrap_or(0),
        effective_from: row.get("effective_from"),
        currency: row
            .get::<Option<String>, _>("currency")
            .unwrap_or_else(|| "USD".to_string()),
        priority: row.get::<Option<i32>, _>("priority").unwrap_or(0),
        enabled: row.get::<Option<bool>, _>("enabled").unwrap_or(true),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Tariff updated successfully",
    )))
}

/// Delete a tariff
///
/// DELETE /api/v1/tarifas/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_tariff(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    let tariff_id = path.into_inner();
    debug!(id = tariff_id, admin = %admin.username, "Deleting tariff");

    // Get tariff info before deleting
    let rate_name: String = sqlx::query_scalar("SELECT rate_name FROM rate_zones WHERE id = $1")
        .bind(tariff_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to get tariff: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Tariff {} not found", tariff_id)))?;

    let result = sqlx::query("DELETE FROM rate_zones WHERE id = $1")
        .bind(tariff_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete tariff: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Tariff {} not found",
            tariff_id
        )));
    }

    info!(id = tariff_id, admin = %admin.username, "Tariff deleted successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "rate_name": rate_name,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_tariff")
        .entity_type("tariff")
        .entity_id(tariff_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Sync to rate_cards
    let sync_result = sync_rate_cards(&pool).await;
    if let Err(e) = sync_result {
        warn!("Sync to rate_cards failed: {}", e);
    }

    Ok(HttpResponse::NoContent().finish())
}

// ============================================================================
// Sync Function
// ============================================================================

/// Sync zones, prefixes, and tariffs to rate_cards table
///
/// POST /api/v1/sync-rate-cards
#[instrument(skip(pool, admin))]
pub async fn sync_rate_cards_endpoint(
    pool: web::Data<PgPool>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    info!(admin = %admin.username, "Manual sync rate cards triggered");

    let result = sync_rate_cards(&pool).await?;

    // Audit log
    let audit_details = serde_json::json!({
        "rate_cards_synced": result.rate_cards_synced,
        "rate_cards_deleted": result.rate_cards_deleted,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("sync_rate_cards")
        .entity_type("rate_card")
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Internal sync function - rebuilds rate_cards from zones+prefixes+tariffs
async fn sync_rate_cards(pool: &PgPool) -> Result<SyncResponse, AppError> {
    let now = Utc::now();

    // Get all active zone+prefix+tariff combinations
    let rows = sqlx::query(
        r#"
        SELECT
            p.prefix as destination_prefix,
            z.zone_name as destination_name,
            rz.rate_per_minute,
            rz.billing_increment,
            rz.rate_per_call as connection_fee,
            rz.priority
        FROM zones z
        JOIN prefixes p ON p.zone_id = z.id
        JOIN rate_zones rz ON rz.zone_id = z.id
        WHERE z.enabled = true
          AND p.enabled = true
          AND rz.enabled = true
        ORDER BY p.prefix
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch zone data: {}", e)))?;

    let mut synced = 0;
    let mut deleted = 0;

    // Use upsert for each rate card
    for row in &rows {
        let prefix: String = row.get("destination_prefix");
        let name: String = row.get("destination_name");
        let rpm: Decimal = row.get("rate_per_minute");
        let bi: i32 = row.get::<Option<i32>, _>("billing_increment").unwrap_or(6);
        let cf: Decimal = row
            .get::<Option<Decimal>, _>("connection_fee")
            .unwrap_or(Decimal::ZERO);
        let prio: i32 = row.get::<Option<i32>, _>("priority").unwrap_or(0);

        let result = sqlx::query(
            r#"
            INSERT INTO rate_cards (destination_prefix, destination_name, rate_name, rate_per_minute, billing_increment, connection_fee, priority, effective_start, created_at, updated_at)
            VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $7, $7)
            ON CONFLICT (destination_prefix) DO UPDATE SET
                destination_name = $2,
                rate_name = $2,
                rate_per_minute = $3,
                billing_increment = $4,
                connection_fee = $5,
                priority = $6,
                updated_at = $7,
                effective_end = NULL
            "#,
        )
        .bind(&prefix)
        .bind(&name)
        .bind(rpm)
        .bind(bi)
        .bind(cf)
        .bind(prio)
        .bind(now)
        .execute(pool)
        .await;

        if result.is_ok() {
            synced += 1;
        }
    }

    // Optionally: soft-delete rate_cards that are no longer in the zone system
    // (set effective_end = now)
    let prefixes: Vec<String> = rows.iter().map(|r| r.get("destination_prefix")).collect();

    if !prefixes.is_empty() {
        // Mark as expired any rate_cards not in the current prefix list
        let del_result = sqlx::query(
            r#"
            UPDATE rate_cards
            SET effective_end = $1, updated_at = $1
            WHERE destination_prefix NOT IN (SELECT unnest($2::text[]))
              AND effective_end IS NULL
            "#,
        )
        .bind(now)
        .bind(&prefixes)
        .execute(pool)
        .await;

        if let Ok(r) = del_result {
            deleted = r.rows_affected() as i32;
        }
    }

    info!(
        synced = synced,
        deleted = deleted,
        "Rate cards sync completed"
    );

    Ok(SyncResponse {
        success: true,
        rate_cards_synced: synced,
        rate_cards_deleted: deleted,
        message: format!(
            "Synced {} rate cards, expired {} obsolete",
            synced, deleted
        ),
    })
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure management routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/zonas")
            .route("", web::get().to(list_zones))
            .route("", web::post().to(create_zone))
            .route("/{id}", web::get().to(get_zone))
            .route("/{id}", web::put().to(update_zone))
            .route("/{id}", web::delete().to(delete_zone)),
    )
    .service(
        web::scope("/prefijos")
            .route("", web::get().to(list_prefixes))
            .route("", web::post().to(create_prefix))
            .route("/{id}", web::delete().to(delete_prefix)),
    )
    .service(
        web::scope("/tarifas")
            .route("", web::get().to(list_tariffs))
            .route("", web::post().to(create_tariff))
            .route("/{id}", web::put().to(update_tariff))
            .route("/{id}", web::delete().to(delete_tariff)),
    )
    .route("/sync-rate-cards", web::post().to(sync_rate_cards_endpoint));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_response_serialization() {
        let response = SyncResponse {
            success: true,
            rate_cards_synced: 100,
            rate_cards_deleted: 5,
            message: "Sync completed".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
