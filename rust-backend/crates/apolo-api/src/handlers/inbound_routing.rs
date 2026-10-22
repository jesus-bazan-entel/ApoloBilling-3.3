//! Inbound Routing handlers
//!
//! HTTP handlers for simplified inbound routing management.
//! Routes are matched by pattern with auto-priority based on pattern length.

use crate::dto::inbound::{
    CreateInboundRouteRequest, InboundGatewayOption, InboundRouteListResponse,
    InboundRouteResponse, UpdateInboundRouteRequest,
};
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use sqlx::PgPool;
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// FreeSWITCH dialplan file path
const TO_KAMAILIO_XML: &str = "/etc/freeswitch/dialplan/to-kamailio.xml";

/// Build the FreeSWITCH bridge string for an inbound route destination.
///
/// * `endpoint` = `(profile, host, port)` of the `sip_endpoints` trunk (PBX)
///   destination -> bridged directly to `host:port`.
/// * neither -> raw fallback to `sofia/internal/<number>@<destination_host>`.
fn build_bridge(
    dest_type: &str,
    dial_var: &str,
    dest_host: &str,
    endpoint: Option<(&str, &str, i32)>,
) -> String {
    if dest_type != "gateway" {
        return format!("user/{}@{}", dial_var, dest_host);
    }
    if let Some((profile, host, port)) = endpoint {
        return format!("sofia/{}/{}@{}:{}", profile, dial_var, host, port);
    }
    format!("sofia/internal/{}@{}", dial_var, dest_host)
}

// ============================================================================
// Inbound Route Handlers
// ============================================================================

/// List all inbound routes
///
/// GET /api/v1/inbound-routing
#[instrument(skip(pool, user))]
pub async fn list_inbound_routes(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("inbound_routing") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing all inbound routes");

    let routes = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.pattern, r.source_ip,
               r.destination_type, r.sip_endpoint_id,
               se.name as "gateway_name?",
               se.host as "gateway_host?",
               se.port as "gateway_port?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media,
               r.enabled, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM inbound_routes r
        LEFT JOIN sip_endpoints se ON r.sip_endpoint_id = se.id
        ORDER BY LENGTH(r.pattern) DESC, r.pattern
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let response: Vec<InboundRouteResponse> = routes
        .into_iter()
        .map(|r| InboundRouteResponse {
            id: r.id,
            name: r.name,
            description: r.description,
            pattern: r.pattern.clone(),
            is_wildcard: r.pattern.ends_with('*'),
            source_ip: r.source_ip,
            destination_type: r.destination_type,
            sip_endpoint_id: r.sip_endpoint_id,
            gateway_name: r.gateway_name,
            gateway_host: r.gateway_host,
            gateway_port: r.gateway_port,
            destination_host: r
                .destination_host
                .unwrap_or_else(|| "10.10.22.4".to_string()),
            strip_digits: r.strip_digits.unwrap_or(4),
            prefix_to_add: r.prefix_to_add.unwrap_or_default(),
            call_timeout: r.call_timeout.unwrap_or(55),
            ims_mode: r.ims_mode.unwrap_or(false),
            early_media: r.early_media.unwrap_or(false),
            enabled: r.enabled.unwrap_or(true),
            sync_status: r.sync_status.unwrap_or_else(|| "pending".to_string()),
            sync_error: r.sync_error,
            created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
            updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
        })
        .collect();

    let total = response.len() as i64;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(InboundRouteListResponse {
            data: response,
            total,
        })),
    )
}

/// Get a single inbound route by ID
///
/// GET /api/v1/inbound-routing/{id}
#[instrument(skip(pool, user))]
pub async fn get_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("inbound_routing") {
        return Err(AppError::Forbidden);
    }

    let route_id = path.into_inner();
    debug!(id = %route_id, "Getting inbound route");

    let route = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.pattern, r.source_ip,
               r.destination_type, r.sip_endpoint_id,
               se.name as "gateway_name?",
               se.host as "gateway_host?",
               se.port as "gateway_port?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media,
               r.enabled, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM inbound_routes r
        LEFT JOIN sip_endpoints se ON r.sip_endpoint_id = se.id
        WHERE r.id = $1
        "#,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Inbound route {} not found", route_id)))?;

    let response = InboundRouteResponse {
        id: route.id,
        name: route.name,
        description: route.description,
        pattern: route.pattern.clone(),
        is_wildcard: route.pattern.ends_with('*'),
        source_ip: route.source_ip,
        destination_type: route.destination_type,
        sip_endpoint_id: route.sip_endpoint_id,
        gateway_name: route.gateway_name,
        gateway_host: route.gateway_host,
        gateway_port: route.gateway_port,
        destination_host: route
            .destination_host
            .unwrap_or_else(|| "10.10.22.4".to_string()),
        strip_digits: route.strip_digits.unwrap_or(4),
        prefix_to_add: route.prefix_to_add.unwrap_or_default(),
        call_timeout: route.call_timeout.unwrap_or(55),
        ims_mode: route.ims_mode.unwrap_or(false),
        early_media: route.early_media.unwrap_or(false),
        enabled: route.enabled.unwrap_or(true),
        sync_status: route.sync_status.unwrap_or_else(|| "pending".to_string()),
        sync_error: route.sync_error,
        created_at: route.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: route.updated_at.unwrap_or_else(chrono::Utc::now),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// List destination trunks (PBX) for the inbound route destination dropdown
///
/// Source: SIP devices with `endpoint_type = 'trunk'` (e.g. "PBX GC GROUP").
///
/// GET /api/v1/inbound-routing/destinations
///
/// # Errors
///
/// Returns `AppError::Forbidden` when the user lacks the `inbound_routing`
/// module, or a database error when the query fails.
#[instrument(skip(pool, user))]
pub async fn list_destinations(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("inbound_routing") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing inbound route destinations (SIP trunks)");

    let rows = sqlx::query!(
        r#"
        SELECT id, name, host, port, transport, fs_profile
        FROM sip_endpoints
        WHERE endpoint_type = 'trunk' AND enabled = true AND host IS NOT NULL
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let data: Vec<InboundGatewayOption> = rows
        .into_iter()
        .map(|r| InboundGatewayOption {
            id: r.id,
            name: r.name,
            host: r.host.unwrap_or_default(),
            port: r.port.unwrap_or(5060),
            transport: r.transport.unwrap_or_else(|| "udp".to_string()),
            fs_profile: r.fs_profile,
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(data)))
}

/// Create a new inbound route
///
/// POST /api/v1/inbound-routing
#[instrument(skip(pool, admin, req))]
pub async fn create_inbound_route(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreateInboundRouteRequest>,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, pattern = %req.pattern, "Creating inbound route");

    // Validate pattern
    validate_pattern(&req.pattern)?;

    // Check for duplicate pattern
    let existing = sqlx::query!(
        "SELECT id FROM inbound_routes WHERE pattern = $1",
        req.pattern
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "Route with pattern {} already exists",
            req.pattern
        )));
    }

    let route_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO inbound_routes (
            id, name, description, pattern, source_ip,
            destination_type, destination_host,
            strip_digits, prefix_to_add,
            call_timeout, ims_mode, early_media,
            enabled, sip_endpoint_id, sync_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'pending')
        "#,
        route_id,
        req.name,
        req.description,
        req.pattern,
        req.source_ip,
        req.destination_type,
        req.destination_host,
        req.strip_digits,
        req.prefix_to_add,
        req.call_timeout,
        req.ims_mode,
        req.early_media,
        req.enabled,
        req.sip_endpoint_id,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to FreeSWITCH
    if let Err(e) = sync_inbound_routes_to_freeswitch(&pool).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE inbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            route_id
        )
        .execute(pool.get_ref())
        .await;
        warn!(error = %error_msg, "Failed to sync inbound route to FreeSWITCH");
    } else {
        let _ = sqlx::query!(
            "UPDATE inbound_routes SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
            route_id
        )
        .execute(pool.get_ref())
        .await;
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_inbound_route")
        .entity_type("inbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({
            "name": req.name,
            "pattern": req.pattern,
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    // Return created route
    get_inbound_route(pool, web::Path::from(route_id), admin.0).await
}

/// Update an inbound route
///
/// PUT /api/v1/inbound-routing/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<UpdateInboundRouteRequest>,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(id = %route_id, "Updating inbound route");

    // Check exists
    let _ = sqlx::query!("SELECT id FROM inbound_routes WHERE id = $1", route_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Inbound route {} not found", route_id)))?;

    // Validate pattern if provided
    if let Some(ref pattern) = req.pattern {
        validate_pattern(pattern)?;

        // Check for duplicate pattern (excluding self)
        let dup = sqlx::query!(
            "SELECT id FROM inbound_routes WHERE pattern = $1 AND id != $2",
            pattern,
            route_id
        )
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if dup.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Route with pattern {} already exists",
                pattern
            )));
        }
    }

    // Destination reference (double option: omitted = unchanged, null = clear)
    let (update_endpoint, sip_endpoint_id) = resolve_destination_ref(&req);

    // Build dynamic update
    sqlx::query!(
        r#"
        UPDATE inbound_routes SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            pattern = COALESCE($4, pattern),
            source_ip = COALESCE($5, source_ip),
            destination_type = COALESCE($6, destination_type),
            sip_endpoint_id = CASE WHEN $7 THEN $8 ELSE sip_endpoint_id END,
            destination_host = COALESCE($9, destination_host),
            strip_digits = COALESCE($10, strip_digits),
            prefix_to_add = COALESCE($11, prefix_to_add),
            call_timeout = COALESCE($12, call_timeout),
            ims_mode = COALESCE($13, ims_mode),
            early_media = COALESCE($14, early_media),
            enabled = COALESCE($15, enabled),
            sync_status = 'pending',
            updated_at = NOW()
        WHERE id = $1
        "#,
        route_id,
        req.name,
        req.description,
        req.pattern,
        req.source_ip,
        req.destination_type,
        update_endpoint,
        sip_endpoint_id,
        req.destination_host,
        req.strip_digits,
        req.prefix_to_add,
        req.call_timeout,
        req.ims_mode,
        req.early_media,
        req.enabled,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to FreeSWITCH
    if let Err(e) = sync_inbound_routes_to_freeswitch(&pool).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE inbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            route_id
        )
        .execute(pool.get_ref())
        .await;
    } else {
        let _ = sqlx::query!(
            "UPDATE inbound_routes SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
            route_id
        )
        .execute(pool.get_ref())
        .await;
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_inbound_route")
        .entity_type("inbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_inbound_route(pool, web::Path::from(route_id), admin.0).await
}

/// Resolve the destination trunk reference of an update request.
///
/// Returns `(update, sip_endpoint_id)`:
/// * `(false, None)` - field omitted -> leave the stored value unchanged
/// * `(true, None)` - explicit `null` -> clear the stored value
/// * `(true, Some(id))` - store the given `sip_endpoints` trunk
fn resolve_destination_ref(req: &UpdateInboundRouteRequest) -> (bool, Option<Uuid>) {
    req.sip_endpoint_id
        .map_or((false, None), |value| (true, value))
}

/// Delete an inbound route
///
/// DELETE /api/v1/inbound-routing/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(id = %route_id, "Deleting inbound route");

    let result = sqlx::query!("DELETE FROM inbound_routes WHERE id = $1", route_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Inbound route {} not found",
            route_id
        )));
    }

    // Regenerate dialplan
    let _ = sync_inbound_routes_to_freeswitch(&pool).await;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_inbound_route")
        .entity_type("inbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Inbound route deleted successfully",
    )))
}

// ============================================================================
// Sync Functions
// ============================================================================

/// Sync all inbound routes to FreeSWITCH dialplan
///
/// POST /api/v1/inbound-routing/sync
#[instrument(skip(pool, _admin))]
pub async fn sync_inbound_routes(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Manual sync of inbound routes to FreeSWITCH");

    sync_inbound_routes_to_freeswitch(&pool).await?;

    // Update all sync statuses
    sqlx::query!(
        "UPDATE inbound_routes SET sync_status = 'synced', sync_error = NULL WHERE enabled = true"
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Inbound routes synced to FreeSWITCH",
    )))
}

/// Validate pattern format
fn validate_pattern(pattern: &str) -> Result<(), AppError> {
    // Pattern must not be empty
    if pattern.is_empty() {
        return Err(AppError::Validation("Pattern cannot be empty".to_string()));
    }

    // Check for wildcard at the end
    let is_wildcard = pattern.ends_with('*');
    let digits_part = if is_wildcard {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };

    // Digits part must be all numeric
    if !digits_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation(
            "Pattern must contain only digits (and optional * at the end)".to_string(),
        ));
    }

    // Must have at least one digit
    if digits_part.is_empty() {
        return Err(AppError::Validation(
            "Pattern must have at least one digit".to_string(),
        ));
    }

    Ok(())
}

/// Legs (host, port, nombre) para el dueno de un prefijo: los miembros
/// habilitados de su grupo en orden de prioridad, o la duena sola si no
/// esta agrupada. `owner_groups`: (endpoint, grupo). `mates`:
/// (grupo, endpoint, nombre, host, port, prioridad) — ya viene ordenado.
fn legs_for(
    owner: &Uuid,
    host: &str,
    port: i32,
    name: &str,
    owner_groups: &[(Uuid, Uuid)],
    mates: &[(Uuid, Uuid, String, String, i32, i32)],
) -> Vec<(String, i32, String)> {
    if let Some((_, group)) = owner_groups.iter().find(|(eid, _)| eid == owner) {
        let mut legs: Vec<(String, i32, String, i32)> = mates
            .iter()
            .filter(|(g, _, _, _, _, _)| g == group)
            .map(|(_, _, n, h, p, prio)| (h.clone(), *p, n.clone(), *prio))
            .collect();
        legs.sort_by_key(|l| l.3);
        if !legs.is_empty() {
            return legs.into_iter().map(|(h, p, n, _)| (h, p, n)).collect();
        }
    }
    vec![(host.to_string(), port, name.to_string())]
}

/// Parse (prefijo, pbx, host, port) desde did_ranges de las PBXs,
/// ordenado longest-match (prefijo mas largo primero, luego alfabetico).
/// Cada entrada de did_ranges puede ser un prefijo/bloque, un DID
/// individual de largo completo, o un rango numerico explicito
/// "START-END" — expand_did_ranges_lenient() las reduce todas a prefijos
/// planos (ignorando silenciosamente cualquier entrada mal formada).
fn collect_range_prefixes(
    pbxs: &[(String, String, i32, String)],
) -> Vec<(String, String, String, i32)> {
    let mut ranges: Vec<(String, String, String, i32)> = Vec::new();
    for (name, host, port, did_ranges) in pbxs {
        for p in crate::handlers::sip_endpoint::expand_did_ranges_lenient(did_ranges) {
            ranges.push((p, name.clone(), host.clone(), *port));
        }
    }
    ranges.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
    ranges
}

/// Read a system_settings key with default (missing table/key -> default).
async fn get_setting(pool: &PgPool, key: &str, default: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM system_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

/// Generate and write FreeSWITCH dialplan from inbound_routes table
pub(crate) async fn sync_inbound_routes_to_freeswitch(pool: &PgPool) -> Result<(), AppError> {
    info!("Generating FreeSWITCH dialplan from inbound routes");

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<!-- Auto-generated by ApoloBilling Inbound Routing System -->\n");
    xml.push_str("<!-- DO NOT EDIT MANUALLY - Changes will be overwritten -->\n");
    xml.push_str("<include>\n");
    xml.push_str("  <context name=\"to-kamailio\">\n\n");

    // Fetch all enabled routes ordered by pattern length DESC (auto-priority)
    let routes = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.pattern, r.source_ip,
               r.destination_type, r.sip_endpoint_id,
               se.name as "endpoint_name?",
               se.host as "endpoint_host?",
               se.port as "endpoint_port?",
               se.fs_profile as "endpoint_fs_profile?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media
        FROM inbound_routes r
        LEFT JOIN sip_endpoints se ON r.sip_endpoint_id = se.id
        WHERE r.enabled = true
        ORDER BY LENGTH(r.pattern) DESC, r.pattern
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut has_ims = false;

    // ETAPA 1/2 (two-stage): las wildcards RN* NO puentean: normalizan
    // (strip/prefix), aplican clase de carrier y transfieren a national-routing.
    // Solo las exactas puentean directo (overrides). OFF = legacy.
    let two_stage = get_setting(pool, "inbound.two_stage", "true")
        .await
        .trim()
        .eq_ignore_ascii_case("true");
    let default_timeout: i32 = get_setting(pool, "inbound.default_timeout", "55")
        .await
        .parse()
        .unwrap_or(55);

    for route in &routes {
        let is_wildcard = route.pattern.ends_with('*');
        let prefix = if is_wildcard {
            &route.pattern[..route.pattern.len() - 1]
        } else {
            &route.pattern
        };

        // Generate regex pattern
        let regex = if is_wildcard {
            format!("{}.+", prefix)
        } else {
            route.pattern.clone()
        };

        // Generate extension name
        let ext_name = if is_wildcard {
            format!("inbound-{}", route.name.to_lowercase().replace(' ', "-"))
        } else {
            format!("did-{}", route.pattern)
        };

        let strip = route.strip_digits.unwrap_or(4);
        let add_prefix = route.prefix_to_add.as_deref().unwrap_or("");

        if route.ims_mode.unwrap_or(false) {
            has_ims = true;
        }

        xml.push_str(&format!(
            "    <!-- Inbound: {} ({}) -->\n",
            route.name,
            if is_wildcard { "wildcard" } else { "exact" }
        ));
        xml.push_str(&format!("    <extension name=\"{}\">\n", ext_name));

        // Source IP filter if specified
        if let Some(ref source_ip) = route.source_ip {
            xml.push_str(&format!(
                "      <condition field=\"${{network_addr}}\" expression=\"{}\">\n",
                escape_xml(source_ip)
            ));
            xml.push_str(&format!(
                "        <condition field=\"destination_number\" expression=\"^({})$\">\n",
                regex
            ));
        } else {
            xml.push_str(&format!(
                "      <condition field=\"destination_number\" expression=\"^({})$\">\n",
                regex
            ));
        }

        let indent = if route.source_ip.is_some() {
            "          "
        } else {
            "        "
        };

        // Log
        xml.push_str(&format!(
            "{}<action application=\"log\" data=\"INFO Inbound {}: ${{caller_id_number}} -> ${{destination_number}}\"/>\n",
            indent, route.name
        ));

        // Number transformation
        if strip > 0 || !add_prefix.is_empty() {
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"original_destination=${{destination_number}}\"/>\n",
                indent
            ));
            if strip > 0 && !add_prefix.is_empty() {
                xml.push_str(&format!(
                    "{}<action application=\"set\" data=\"transformed_number={}${{destination_number:{}}}\"/>\n",
                    indent,
                    escape_xml(add_prefix),
                    strip
                ));
            } else if strip > 0 {
                xml.push_str(&format!(
                    "{}<action application=\"set\" data=\"transformed_number=${{destination_number:{}}}\"/>\n",
                    indent, strip
                ));
            } else {
                xml.push_str(&format!(
                    "{}<action application=\"set\" data=\"transformed_number={}${{destination_number}}\"/>\n",
                    indent,
                    escape_xml(add_prefix)
                ));
            }
        }

        // Dial string variable
        let dial_var = if strip > 0 || !add_prefix.is_empty() {
            "${transformed_number}"
        } else {
            "$1"
        };

        // Timeout
        let call_timeout = route.call_timeout.unwrap_or(55);
        xml.push_str(&format!(
            "{}<action application=\"set\" data=\"call_timeout={}\"/>\n",
            indent, call_timeout
        ));
        xml.push_str(&format!(
            "{}<action application=\"set\" data=\"originate_timeout={}\"/>\n",
            indent, call_timeout
        ));

        // IMS Mode
        let ims_mode = route.ims_mode.unwrap_or(false);
        if ims_mode {
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"progress_timeout={}\"/>\n",
                indent, call_timeout
            ));
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"sip_enable_100rel=true\"/>\n",
                indent
            ));
            xml.push_str(&format!(
                "{}<action application=\"export\" data=\"sip_enable_100rel=true\"/>\n",
                indent
            ));
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"bypass_media=false\"/>\n",
                indent
            ));
        }

        xml.push_str(&format!(
            "{}<action application=\"set\" data=\"inherit_codec=true\"/>\n",
            indent
        ));

        // Early Media
        let early_media = route.early_media.unwrap_or(false);
        if early_media {
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"ringback=${{us-ring}}\"/>\n",
                indent
            ));
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"instant_ringback=true\"/>\n",
                indent
            ));
            xml.push_str(&format!("{}<action application=\"pre_answer\"/>\n", indent));
        }

        // Continue on fail for IMS (solo cuando esta extension puentea)
        if ims_mode && !(two_stage && is_wildcard) {
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"hangup_after_bridge=false\"/>\n",
                indent
            ));
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"continue_on_fail=true\"/>\n",
                indent
            ));
        }

        // Bridge (exactas) o clasificar+transferir (wildcards en two-stage)
        if two_stage && is_wildcard {
            // ETAPA 1: normaliza y pasa a ETAPA 2. NO puentea aqui.
            let next = if strip > 0 || !add_prefix.is_empty() {
                "${transformed_number}"
            } else {
                "$1"
            };
            xml.push_str(&format!(
                "{}<action application=\"transfer\" data=\"{} XML national-routing\"/>\n",
                indent, next
            ));
        } else {
            let dest_type = route.destination_type.as_str();
            let dest_host = route.destination_host.as_deref().unwrap_or("10.10.22.4");
            let endpoint = match (&route.endpoint_host, route.endpoint_port) {
                (Some(host), Some(port)) => Some((
                    route.endpoint_fs_profile.as_deref().unwrap_or("internal"),
                    host.as_str(),
                    port,
                )),
                _ => None,
            };
            let bridge = build_bridge(dest_type, dial_var, dest_host, endpoint);

            xml.push_str(&format!(
                "{}<action application=\"bridge\" data=\"{}\"/>\n",
                indent, bridge
            ));

            // IMS failure handler
            if ims_mode {
                xml.push_str(&format!(
                    "{}<action application=\"transfer\" data=\"_handle_bridge_failure_\"/>\n",
                    indent
                ));
            }
        }

        if route.source_ip.is_some() {
            xml.push_str("        </condition>\n");
            xml.push_str("      </condition>\n");
        } else {
            xml.push_str("      </condition>\n");
        }
        xml.push_str("    </extension>\n\n");
    }

    // ETAPA 1b (two-stage): passthrough — lo que ya viene en formato nacional
    // entra directo a ETAPA 2. Va ULTIMO entre los clasificadores.
    if two_stage {
        xml.push_str("    <!-- ETAPA 1b: passthrough (formato ya nacional, sin RN) -->\n");
        xml.push_str("    <extension name=\"classify-passthrough\">\n");
        xml.push_str("      <condition field=\"destination_number\" expression=\"^(\\d+)$\">\n");
        xml.push_str("        <action application=\"log\" data=\"INFO ETAPA1-passthrough: formato nacional ${destination_number} -> national-routing\"/>\n");
        xml.push_str("        <action application=\"set\" data=\"original_destination=${destination_number}\"/>\n");
        xml.push_str("        <action application=\"set\" data=\"transformed_number=$1\"/>\n");
        xml.push_str(&format!(
            "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
            default_timeout
        ));
        xml.push_str(&format!(
            "        <action application=\"set\" data=\"originate_timeout={}\"/>\n",
            default_timeout
        ));
        xml.push_str(
            "        <action application=\"transfer\" data=\"$1 XML national-routing\"/>\n",
        );
        xml.push_str("      </condition>\n");
        xml.push_str("    </extension>\n\n");
    }

    // Add IMS failure handlers if any route uses IMS mode
    if has_ims {
        xml.push_str(&generate_ims_failure_handlers());
    }

    // ETAPA 1c: cierre de to-kamailio + apertura de national-routing.
    // En legacy todo vive en to-kamailio (sin segundo contexto).
    if two_stage {
        xml.push_str(
            "    <!-- ETAPA 1 seguridad: lo no clasificado -> 480 (no sale de to-kamailio) -->\n",
        );
        xml.push_str("    <extension name=\"fallback-reject\">\n");
        xml.push_str("      <condition field=\"destination_number\" expression=\"^(.+)$\">\n");
        xml.push_str("        <action application=\"log\" data=\"WARNING ETAPA1: sin clasificador para ${destination_number} desde ${network_addr}\"/>\n");
        xml.push_str("        <action application=\"set\" data=\"sip_reason=Q.850;cause=20\"/>\n");
        xml.push_str(
            "        <action application=\"respond\" data=\"480 Temporarily Unavailable\"/>\n",
        );
        xml.push_str("      </condition>\n");
        xml.push_str("    </extension>\n\n");
        xml.push_str("  </context>\n\n");
        xml.push_str("  <!-- ============================================================ -->\n");
        xml.push_str("  <!-- ETAPA 2 (national-routing): decide sobre el numero NACIONAL -->\n");
        xml.push_str("  <!-- L2 registrada -> L3 rango de su PBX -> L4 480.               -->\n");
        xml.push_str("  <!-- ============================================================ -->\n");
        xml.push_str("  <context name=\"national-routing\">\n\n");
    }

    // ============================================================
    // CARRIER-CLASS AUTO-ROUTING (L2/L3/L4)
    // L1 arriba = overrides explicitos (inbound_routes, por longitud
    // de patron). L2 = extensiones registradas, L3 = hunt sobre PBXs
    // conectadas (sip_endpoints trunk/ip), L4 = fallback configurable.
    // Cero-config por DID. Switches en system_settings: inbound.*.
    // ============================================================
    let auto_local = get_setting(pool, "inbound.auto_local_extensions", "true").await;
    let auto_pbx = get_setting(pool, "inbound.auto_pbx_hunt", "true").await;
    let local_timeout: i32 = get_setting(pool, "inbound.local_timeout", "30")
        .await
        .parse()
        .unwrap_or(30);
    let pbx_timeout: i32 = get_setting(pool, "inbound.pbx_hunt_timeout", "55")
        .await
        .parse()
        .unwrap_or(55);
    let default_action = get_setting(pool, "inbound.default_action", "480").await;
    let default_strip: i32 = get_setting(pool, "inbound.pbx_strip_digits", "0")
        .await
        .parse()
        .unwrap_or(0);
    let default_prefix = get_setting(pool, "inbound.pbx_prefix_to_add", "").await;

    // En ETAPA 2 el post-bridge deriva a los handlers de to-kamailio
    // (transfer cross-contexto); en legacy se cuelga directo.
    let fail_transfer = if two_stage {
        "_handle_bridge_failure_ XML to-kamailio"
    } else {
        "_handle_bridge_failure_"
    };

    // L2: extensiones locales registradas (lookup tipo HLR)
    if auto_local.trim().eq_ignore_ascii_case("true") {
        xml.push_str(
            "    <!-- L2 AUTO: extensiones locales registradas (cero-config por DID) -->\n",
        );
        xml.push_str("    <extension name=\"auto-local-extensions\">\n");
        xml.push_str("      <condition field=\"destination_number\" expression=\"^(\\d+)$\" break=\"never\">\n");
        xml.push_str("        <action application=\"set\" data=\"dialed_extension=$1\"/>\n");
        xml.push_str("      </condition>\n");
        xml.push_str("      <condition field=\"${sofia_contact(user/${dialed_extension}@10.10.22.4)}\" expression=\"^sofia/\">\n");
        xml.push_str("        <action application=\"log\" data=\"INFO AUTO-L2: extension local registrada ${dialed_extension} (${caller_id_number} -> ${destination_number})\"/>\n");
        xml.push_str("        <action application=\"set\" data=\"sip_enable_100rel=true\"/>\n");
        xml.push_str("        <action application=\"export\" data=\"sip_enable_100rel=true\"/>\n");
        xml.push_str(&format!(
            "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
            local_timeout
        ));
        if two_stage {
            xml.push_str(
                "        <action application=\"set\" data=\"hangup_after_bridge=false\"/>\n",
            );
            xml.push_str("        <action application=\"set\" data=\"continue_on_fail=true\"/>\n");
        } else {
            xml.push_str(
                "        <action application=\"set\" data=\"hangup_after_bridge=true\"/>\n",
            );
            xml.push_str("        <action application=\"set\" data=\"continue_on_fail=false\"/>\n");
        }
        xml.push_str("        <action application=\"set\" data=\"effective_caller_id_number=${caller_id_number}\"/>\n");
        xml.push_str("        <action application=\"set\" data=\"effective_caller_id_name=${caller_id_name}\"/>\n");
        xml.push_str("        <action application=\"bridge\" data=\"user/${dialed_extension}@10.10.22.4\"/>\n");
        if two_stage {
            xml.push_str("        <action application=\"log\" data=\"INFO AUTO-L2: bridge termino -> handlers IMS\"/>\n");
            xml.push_str(&format!(
                "        <action application=\"transfer\" data=\"{}\"/>\n",
                fail_transfer
            ));
        }
        xml.push_str("      </condition>\n");
        xml.push_str("    </extension>\n\n");
    }

    // L3 SELECTIVO estilo carrier (Vodafone/ATT): cada rango va SOLO a su PBX
    // duena (sip_endpoints.did_ranges, longest-match). Jamas broadcast a terceros.
    // Modo legacy hunt-all solo si inbound.pbx_hunt_mode='hunt-all' (single-tenant).
    if auto_pbx.trim().eq_ignore_ascii_case("true") {
        let hunt_mode = get_setting(pool, "inbound.pbx_hunt_mode", "ranges").await;
        if hunt_mode.trim().eq_ignore_ascii_case("hunt-all") {
            let pbx_ips: Vec<(String, String, i32)> = sqlx::query_as::<_, (String, String, i32)>(
                r#"SELECT name, host, port FROM sip_endpoints
                    WHERE enabled = true AND endpoint_type = 'trunk' AND auth_mode = 'ip'
                      AND host IS NOT NULL AND host <> ''
                    ORDER BY name"#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            if pbx_ips.is_empty() {
                xml.push_str(
                    "    <!-- L3 AUTO: sin PBXs conectadas (sin sip_endpoints trunk/ip) -->\n\n",
                );
            } else {
                xml.push_str(
                    "    <!-- L3 AUTO: hunt sobre PBXs conectadas (cero-config por DID) -->\n",
                );
                let dial_expr = if default_strip > 0 && !default_prefix.is_empty() {
                    format!(
                        "{}${{destination_number:{}}}",
                        escape_xml(&default_prefix),
                        default_strip
                    )
                } else if default_strip > 0 {
                    format!("${{destination_number:{}}}", default_strip)
                } else if !default_prefix.is_empty() {
                    format!("{}${{destination_number}}", escape_xml(&default_prefix))
                } else {
                    "${destination_number}".to_string()
                };
                let legs: Vec<String> = pbx_ips
                    .iter()
                    .map(|(_, host, port)| {
                        format!("sofia/internal/{}@{}:{}", dial_expr, host, port)
                    })
                    .collect();
                let names: Vec<&str> = pbx_ips.iter().map(|(n, _, _)| n.as_str()).collect();
                xml.push_str("    <extension name=\"auto-pbx-hunt\">\n");
                xml.push_str(
                    "      <condition field=\"destination_number\" expression=\"^(\\d+)$\">\n",
                );
                xml.push_str(&format!("        <action application=\"log\" data=\"INFO AUTO-L3: hunt PBXs [{}]: ${{caller_id_number}} -> ${{destination_number}}\"/>\n", escape_xml(&names.join(","))));
                xml.push_str(
                    "        <action application=\"set\" data=\"sip_enable_100rel=true\"/>\n",
                );
                xml.push_str(
                    "        <action application=\"export\" data=\"sip_enable_100rel=true\"/>\n",
                );
                xml.push_str(&format!(
                    "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
                    pbx_timeout
                ));
                xml.push_str(&format!(
                    "        <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                    pbx_timeout
                ));
                xml.push_str("        <action application=\"set\" data=\"bypass_media=false\"/>\n");
                xml.push_str("        <action application=\"set\" data=\"inherit_codec=true\"/>\n");
                xml.push_str(
                    "        <action application=\"set\" data=\"hangup_after_bridge=false\"/>\n",
                );
                xml.push_str(
                    "        <action application=\"set\" data=\"continue_on_fail=true\"/>\n",
                );
                xml.push_str(&format!(
                    "        <action application=\"bridge\" data=\"{}\"/>\n",
                    legs.join("|")
                ));
                if two_stage {
                    xml.push_str(&format!(
                        "        <action application=\"transfer\" data=\"{}\"/>\n",
                        fail_transfer
                    ));
                }
                xml.push_str("      </condition>\n");
                xml.push_str("    </extension>\n\n");
            } // end hunt-all con PBXs
        } else {
            // L3 SELECTIVO (default): los rangos DID viven EN LA PBX
            // (sip_endpoints.did_ranges, "174020,174021"). Una extension por
            // prefijo, longest-match, puenteando SOLO a la PBX duena.
            // Sin rango asignado = 480, nunca broadcast a terceros.
            // Transformacion global: inbound.pbx_strip_digits/prefix_to_add.
            let pbxs: Vec<(Uuid, String, String, i32, String, i32)> =
                sqlx::query_as::<_, (Uuid, String, String, i32, String, i32)>(
                    r#"SELECT id, name, host, port, COALESCE(did_ranges, ''), max_concurrent_calls
                    FROM sip_endpoints
                    WHERE enabled = true AND endpoint_type = 'trunk' AND auth_mode = 'ip'
                      AND host IS NOT NULL AND host <> ''
                    ORDER BY name"#,
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();
            // Grupo del dueno (maximo uno por PBX) y sus miembros habilitados.
            // (tabla ausente en instalaciones viejas -> solo la duena)
            let owner_groups: Vec<(Uuid, Uuid)> =
                sqlx::query_as(r#"SELECT sip_endpoint_id, group_id FROM pbx_group_members"#)
                    .fetch_all(pool)
                    .await
                    .unwrap_or_default();
            let mates: Vec<(Uuid, Uuid, String, String, i32, i32)> = sqlx::query_as(
                r#"SELECT m.group_id, m.sip_endpoint_id, se.name, se.host, se.port, m.priority
                   FROM pbx_group_members m JOIN sip_endpoints se ON se.id = m.sip_endpoint_id
                   WHERE se.enabled = true ORDER BY m.priority"#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            // (prefijo, legs[(host,port,nombre)], dueña, max_concurrent_calls) ordenado longest-match
            // did_ranges puede traer prefijos/bloques, DIDs individuales de
            // largo completo, o rangos "START-END" — se expanden todos a
            // prefijos planos antes de ordenar por longest-match.
            let mut ranges: Vec<(String, Vec<(String, i32, String)>, Uuid, i32)> = Vec::new();
            for (eid, name, host, port, did_ranges, max_calls) in &pbxs {
                for p in crate::handlers::sip_endpoint::expand_did_ranges_lenient(did_ranges) {
                    // legs: miembros del grupo (prioridad) o la duena sola
                    let legs = legs_for(eid, host, *port, name, &owner_groups, &mates);
                    ranges.push((p, legs, *eid, *max_calls));
                }
            }
            ranges.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
            if ranges.is_empty() {
                xml.push_str("    <!-- L3 SELECTIVO: ninguna PBX con did_ranges (todo lo no cubierto -> 480) -->\n\n");
            } else {
                xml.push_str(
                    "    <!-- L3 SELECTIVO: cada prefijo SOLO a su PBX (longest-match) -->\n",
                );
                for (prefix, legs, owner_id, max_calls) in &ranges {
                    let dial_expr = if default_strip > 0 && !default_prefix.is_empty() {
                        format!(
                            "{}${{destination_number:{}}}",
                            escape_xml(&default_prefix),
                            default_strip
                        )
                    } else if default_strip > 0 {
                        format!("${{destination_number:{}}}", default_strip)
                    } else if !default_prefix.is_empty() {
                        format!("{}${{destination_number}}", escape_xml(&default_prefix))
                    } else {
                        "${destination_number}".to_string()
                    };
                    // Serie dentro del grupo (mismo dueno): la 1a que responde gana
                    let hunt: Vec<String> = legs
                        .iter()
                        .map(|(h, p, _)| format!("sofia/internal/{}@{}:{}", dial_expr, h, p))
                        .collect();
                    let names: Vec<&str> = legs.iter().map(|(_, _, n)| n.as_str()).collect();
                    xml.push_str(&format!(
                        "    <extension name=\"pbx-range-{}\">\n",
                        escape_xml(prefix)
                    ));
                    // ".*" (no ".+"): si `prefix` ya es un DID de largo
                    // completo (rango de un solo numero), debe matchear
                    // exactamente ese numero y no solo numeros mas largos
                    // que lo extienden.
                    xml.push_str(&format!(
                        "      <condition field=\"destination_number\" expression=\"^({}.*)$\">\n",
                        escape_xml(prefix)
                    ));
                    xml.push_str(&format!("        <action application=\"log\" data=\"INFO AUTO-L3: rango {} -> [{}] (${{caller_id_number}} -> ${{destination_number}})\"/>\n", escape_xml(prefix), escape_xml(&names.join(","))));
                    // Límite de canales concurrentes de la PBX dueña (nativo,
                    // en memoria vía mod_hash): este bridge es directo
                    // FreeSWITCH->PBX, nunca pasa por Kamailio, así que
                    // CHECK_CALL_LIMIT (dr_gateways/dsip_call_settings) no
                    // aplica aquí. Resource = id del sip_endpoint (estable,
                    // independiente del host). Al exceder, transfiere a
                    // pbx-limit-exceeded (congestión + 503) sin llegar a
                    // intentar el bridge.
                    xml.push_str(&format!(
                        "        <action application=\"limit\" data=\"hash pbx_trunk {} {} _pbx_limit_exceeded_\"/>\n",
                        owner_id, max_calls
                    ));
                    xml.push_str(
                        "        <action application=\"set\" data=\"sip_enable_100rel=true\"/>\n",
                    );
                    xml.push_str("        <action application=\"export\" data=\"sip_enable_100rel=true\"/>\n");
                    xml.push_str(&format!(
                        "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
                        pbx_timeout
                    ));
                    xml.push_str(&format!(
                        "        <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                        pbx_timeout
                    ));
                    xml.push_str(
                        "        <action application=\"set\" data=\"bypass_media=false\"/>\n",
                    );
                    xml.push_str(
                        "        <action application=\"set\" data=\"inherit_codec=true\"/>\n",
                    );
                    if two_stage {
                        xml.push_str(
                            "        <action application=\"set\" data=\"hangup_after_bridge=false\"/>\n",
                        );
                        xml.push_str(
                            "        <action application=\"set\" data=\"continue_on_fail=true\"/>\n",
                        );
                    } else {
                        xml.push_str(
                            "        <action application=\"set\" data=\"hangup_after_bridge=true\"/>\n",
                        );
                        xml.push_str(
                            "        <action application=\"set\" data=\"continue_on_fail=false\"/>\n",
                        );
                    }
                    xml.push_str(&format!(
                        "        <action application=\"bridge\" data=\"{}\"/>\n",
                        hunt.join("|")
                    ));
                    if two_stage {
                        xml.push_str(&format!(
                            "        <action application=\"transfer\" data=\"{}\"/>\n",
                            fail_transfer
                        ));
                    }
                    xml.push_str("      </condition>\n");
                    xml.push_str("    </extension>\n\n");
                }

                // Destino común de `limit` al exceder max_concurrent_calls de
                // la PBX dueña (ver acción `limit` arriba): congestión, nunca
                // llega a intentar el bridge.
                xml.push_str("    <extension name=\"pbx-limit-exceeded\">\n");
                xml.push_str("      <condition field=\"destination_number\" expression=\"^_pbx_limit_exceeded_$\">\n");
                xml.push_str("        <action application=\"log\" data=\"WARNING AUTO-L3: límite de canales excedido, PBX ${limit_id} (max=${limit_max})\"/>\n");
                xml.push_str("        <action application=\"playback\" data=\"tone_stream://%(500,500,480,620);loops=3\"/>\n");
                xml.push_str(
                    "        <action application=\"set\" data=\"sip_reason=Q.850;cause=42\"/>\n",
                );
                xml.push_str(
                    "        <action application=\"respond\" data=\"503 Service Unavailable\"/>\n",
                );
                xml.push_str("      </condition>\n");
                xml.push_str("    </extension>\n\n");
            }
        }
    }

    // L4: fallback configurable (carrier-class: 480/503, nunca 403 para DID desconocido)
    let (sip_code, q850) = match default_action.trim() {
        "404" => ("404 Not Found", "1"),
        "503" => ("503 Service Unavailable", "42"),
        _ => ("480 Temporarily Unavailable", "20"),
    };
    xml.push_str(
        "    <!-- L4 FALLBACK carrier-class (system_settings inbound.default_action) -->\n",
    );
    xml.push_str("    <extension name=\"fallback-reject\">\n");
    xml.push_str("      <condition field=\"destination_number\" expression=\"^(.+)$\">\n");
    xml.push_str(&format!("        <action application=\"log\" data=\"WARNING AUTO-L4: sin destino para ${{destination_number}} desde ${{network_addr}} -> {}\"/>\n", sip_code));
    xml.push_str(&format!(
        "        <action application=\"set\" data=\"sip_reason=Q.850;cause={}\"/>\n",
        q850
    ));
    xml.push_str(&format!(
        "        <action application=\"respond\" data=\"{}\"/>\n",
        sip_code
    ));
    xml.push_str("      </condition>\n");
    xml.push_str("    </extension>\n\n");

    if two_stage {
        xml.push_str("  </context> <!-- national-routing -->\n");
    } else {
        xml.push_str("  </context>\n");
    }
    xml.push_str("</include>\n");

    // Backup existing file
    if Path::new(TO_KAMAILIO_XML).exists() {
        let backup_path = format!("{}.bak", TO_KAMAILIO_XML);
        let _ = fs::copy(TO_KAMAILIO_XML, &backup_path).await;
    }

    // Write new file
    fs::write(TO_KAMAILIO_XML, xml).await.map_err(|e| {
        error!(error = %e, "Failed to write FreeSWITCH dialplan");
        AppError::Internal(format!("Failed to write dialplan: {}", e))
    })?;

    // Reload FreeSWITCH
    reload_freeswitch().await?;

    info!("FreeSWITCH dialplan generated successfully");
    Ok(())
}

/// Generate IMS failure handler extensions
fn generate_ims_failure_handlers() -> String {
    r#"    <!-- IMS Failure Handlers -->
    <extension name="handle-bridge-success">
      <condition field="destination_number" expression="^_handle_bridge_failure_$"/>
      <condition field="${originate_disposition}" expression="^SUCCESS$">
        <action application="hangup" data="NORMAL_CLEARING"/>
      </condition>
    </extension>

    <extension name="handle-bridge-failure-dispatcher">
      <condition field="destination_number" expression="^_handle_bridge_failure_$"/>
      <condition field="${originate_disposition}" expression="^USER_BUSY$">
        <action application="transfer" data="_hangup_user_busy_"/>
        <anti-action application="transfer" data="_hangup_check_no_answer_"/>
      </condition>
    </extension>

    <extension name="handle-bridge-failure-no-answer">
      <condition field="destination_number" expression="^_hangup_check_no_answer_$"/>
      <condition field="${originate_disposition}" expression="^(NO_ANSWER|NO_USER_RESPONSE)$">
        <action application="transfer" data="_hangup_timeout_"/>
        <anti-action application="transfer" data="_hangup_check_cancel_"/>
      </condition>
    </extension>

    <extension name="handle-bridge-failure-cancel">
      <condition field="destination_number" expression="^_hangup_check_cancel_$"/>
      <condition field="${originate_disposition}" expression="^(ORIGINATOR_CANCEL|NORMAL_CLEARING|CANCEL)$">
        <action application="transfer" data="_hangup_timeout_"/>
        <anti-action application="transfer" data="_hangup_congestion_"/>
      </condition>
    </extension>

    <extension name="hangup-user-busy">
      <condition field="destination_number" expression="^_hangup_user_busy_$">
        <action application="playback" data="tone_stream://%(500,500,480,620);loops=4"/>
        <action application="respond" data="486 Busy Here"/>
      </condition>
    </extension>

    <extension name="hangup-timeout">
      <condition field="destination_number" expression="^_hangup_timeout_$">
        <action application="playback" data="tone_stream://%(500,500,480,620);loops=7"/>
        <action application="respond" data="486 Busy Here"/>
      </condition>
    </extension>

    <extension name="hangup-congestion">
      <condition field="destination_number" expression="^_hangup_congestion_$">
        <action application="playback" data="tone_stream://%(250,250,480,620);loops=6"/>
        <action application="respond" data="503 Service Unavailable"/>
      </condition>
    </extension>

"#
    .to_string()
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Reload FreeSWITCH dialplan
async fn reload_freeswitch() -> Result<String, AppError> {
    use tokio::process::Command;

    let output = Command::new("fs_cli")
        .args(["-x", "reloadxml"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute fs_cli: {}", e)))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure inbound routing routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/inbound-routing")
            .route("", web::get().to(list_inbound_routes))
            .route("", web::post().to(create_inbound_route))
            .route("/destinations", web::get().to(list_destinations))
            .route("/sync", web::post().to(sync_inbound_routes))
            .route("/{id}", web::get().to(get_inbound_route))
            .route("/{id}", web::put().to(update_inbound_route))
            .route("/{id}", web::delete().to(delete_inbound_route)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT_ID: &str = "67e81a07-67e8-46d1-b758-fe236461cf9b";

    #[test]
    fn bridge_to_sip_endpoint_trunk_is_direct_host_port() {
        let bridge = build_bridge(
            "gateway",
            "${transformed_number}",
            "10.40.41.124",
            Some(("internal", "10.40.41.124", 5160)),
        );
        assert_eq!(
            bridge,
            "sofia/internal/${transformed_number}@10.40.41.124:5160"
        );
    }

    #[test]
    fn bridge_without_destination_falls_back_to_raw_host() {
        let bridge = build_bridge("gateway", "$1", "10.40.41.124", None);
        assert_eq!(bridge, "sofia/internal/$1@10.40.41.124");
    }

    #[test]
    fn bridge_to_extension_uses_registered_user() {
        let bridge = build_bridge(
            "extension",
            "$1",
            "10.10.22.4",
            Some(("internal", "10.40.41.124", 5160)),
        );
        assert_eq!(bridge, "user/$1@10.10.22.4");
    }

    #[test]
    fn range_prefixes_are_longest_match_first() {
        let pbxs = vec![
            (
                "PBX Fibertel".to_string(),
                "10.30.0.26".to_string(),
                5160,
                "174020, 1740, abc, ".to_string(),
            ),
            (
                "PBX GC GROUP".to_string(),
                "10.40.41.124".to_string(),
                5160,
                "163751".to_string(),
            ),
        ];
        let ranges = collect_range_prefixes(&pbxs);
        // "abc" y vacios se descartan; orden: mas largo primero
        let prefixes: Vec<&str> = ranges.iter().map(|(p, _, _, _)| p.as_str()).collect();
        assert_eq!(prefixes, vec!["163751", "174020", "1740"]);
        assert!(ranges.iter().all(|(_, _, _, port)| *port == 5160));
    }

    #[test]
    fn destination_ref_omitted_field_leaves_value_unchanged() {
        let req: UpdateInboundRouteRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(resolve_destination_ref(&req), (false, None));
    }

    #[test]
    fn destination_ref_explicit_null_clears_reference() {
        let req: UpdateInboundRouteRequest =
            serde_json::from_str(r#"{"sip_endpoint_id":null}"#).unwrap();
        assert_eq!(resolve_destination_ref(&req), (true, None));
    }

    #[test]
    fn destination_ref_endpoint_id_is_stored() {
        let req: UpdateInboundRouteRequest =
            serde_json::from_str(&format!(r#"{{"sip_endpoint_id":"{}"}}"#, ENDPOINT_ID)).unwrap();
        assert_eq!(
            resolve_destination_ref(&req),
            (true, Some(Uuid::parse_str(ENDPOINT_ID).unwrap()))
        );
    }

    #[test]
    fn destination_ref_legacy_gateway_id_field_is_ignored() {
        let req: UpdateInboundRouteRequest = serde_json::from_str(
            r#"{"gateway_id":"56fdc636-d632-465f-89c6-aea8b47cc9e8","sip_endpoint_id":null}"#,
        )
        .unwrap();
        assert_eq!(resolve_destination_ref(&req), (true, None));
    }
}

#[cfg(test)]
mod legs_tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn legs_use_group_members_in_priority_order() {
        let owner = Uuid::new_v4();
        let backup = Uuid::new_v4();
        let g = Uuid::new_v4();
        let owner_groups = vec![(owner, g), (backup, g)];
        let mates = vec![
            (
                g,
                backup,
                "Sec".to_string(),
                "10.0.0.2".to_string(),
                5060,
                200,
            ),
            (
                g,
                owner,
                "Pri".to_string(),
                "10.0.0.1".to_string(),
                5060,
                100,
            ),
        ];
        let legs = legs_for(&owner, "10.0.0.1", 5060, "Pri", &owner_groups, &mates);
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].0, "10.0.0.1");
        assert_eq!(legs[1].0, "10.0.0.2");
    }

    #[test]
    fn legs_fall_back_to_owner_alone() {
        let owner = Uuid::new_v4();
        let legs = legs_for(&owner, "10.0.0.9", 5160, "Solo", &[], &[]);
        assert_eq!(
            legs,
            vec![("10.0.0.9".to_string(), 5160, "Solo".to_string())]
        );
    }
}
