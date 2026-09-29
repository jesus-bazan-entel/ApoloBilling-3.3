//! Inbound Routing handlers
//!
//! HTTP handlers for simplified inbound routing management.
//! Routes are matched by pattern with auto-priority based on pattern length.

use crate::dto::inbound::{
    CreateInboundRouteRequest, InboundRouteListResponse, InboundRouteResponse,
    UpdateInboundRouteRequest,
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

/// Normalize gateway name for FreeSWITCH compatibility
/// Converts spaces to hyphens and lowercases the name
fn normalize_gateway_name(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .replace('_', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect()
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
               r.destination_type, r.gateway_id, t.name as "gateway_name?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media,
               r.enabled, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM inbound_routes r
        LEFT JOIN routing_trunks t ON r.gateway_id = t.id
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
            gateway_id: r.gateway_id,
            gateway_name: r.gateway_name,
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
               r.destination_type, r.gateway_id, t.name as "gateway_name?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media,
               r.enabled, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM inbound_routes r
        LEFT JOIN routing_trunks t ON r.gateway_id = t.id
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
        gateway_id: route.gateway_id,
        gateway_name: route.gateway_name,
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
            destination_type, gateway_id, destination_host,
            strip_digits, prefix_to_add,
            call_timeout, ims_mode, early_media,
            enabled, sync_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'pending')
        "#,
        route_id,
        req.name,
        req.description,
        req.pattern,
        req.source_ip,
        req.destination_type,
        req.gateway_id,
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

    // Build dynamic update
    sqlx::query!(
        r#"
        UPDATE inbound_routes SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            pattern = COALESCE($4, pattern),
            source_ip = COALESCE($5, source_ip),
            destination_type = COALESCE($6, destination_type),
            gateway_id = COALESCE($7, gateway_id),
            destination_host = COALESCE($8, destination_host),
            strip_digits = COALESCE($9, strip_digits),
            prefix_to_add = COALESCE($10, prefix_to_add),
            call_timeout = COALESCE($11, call_timeout),
            ims_mode = COALESCE($12, ims_mode),
            early_media = COALESCE($13, early_media),
            enabled = COALESCE($14, enabled),
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
        req.gateway_id,
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

/// Generate and write FreeSWITCH dialplan from inbound_routes table
async fn sync_inbound_routes_to_freeswitch(pool: &PgPool) -> Result<(), AppError> {
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
               r.destination_type, r.gateway_id, t.name as "gateway_name?",
               r.destination_host, r.strip_digits, r.prefix_to_add,
               r.call_timeout, r.ims_mode, r.early_media
        FROM inbound_routes r
        LEFT JOIN routing_trunks t ON r.gateway_id = t.id
        WHERE r.enabled = true
        ORDER BY LENGTH(r.pattern) DESC, r.pattern
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut has_ims = false;

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

        // Continue on fail for IMS
        if ims_mode {
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"hangup_after_bridge=false\"/>\n",
                indent
            ));
            xml.push_str(&format!(
                "{}<action application=\"set\" data=\"continue_on_fail=true\"/>\n",
                indent
            ));
        }

        // Bridge
        let dest_type = route.destination_type.as_str();
        let dest_host = route.destination_host.as_deref().unwrap_or("10.10.22.4");
        let bridge = if dest_type == "gateway" {
            if let Some(ref gw_name) = route.gateway_name {
                let normalized_gw = normalize_gateway_name(gw_name);
                format!("sofia/gateway/{}/{}", normalized_gw, dial_var)
            } else {
                format!("sofia/internal/{}@{}", dial_var, dest_host)
            }
        } else {
            format!("user/{}@{}", dial_var, dest_host)
        };

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

        if route.source_ip.is_some() {
            xml.push_str("        </condition>\n");
            xml.push_str("      </condition>\n");
        } else {
            xml.push_str("      </condition>\n");
        }
        xml.push_str("    </extension>\n\n");
    }

    // Add IMS failure handlers if any route uses IMS mode
    if has_ims {
        xml.push_str(&generate_ims_failure_handlers());
    }

    // Fallback reject
    xml.push_str("    <!-- Fallback: Reject unknown calls -->\n");
    xml.push_str("    <extension name=\"fallback-reject\">\n");
    xml.push_str("      <condition field=\"destination_number\" expression=\"^(.+)$\">\n");
    xml.push_str("        <action application=\"log\" data=\"WARNING Rejecting call to unknown destination: ${destination_number}\"/>\n");
    xml.push_str("        <action application=\"respond\" data=\"404 Not Found\"/>\n");
    xml.push_str("      </condition>\n");
    xml.push_str("    </extension>\n\n");

    xml.push_str("  </context>\n");
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
            .route("/sync", web::post().to(sync_inbound_routes))
            .route("/{id}", web::get().to(get_inbound_route))
            .route("/{id}", web::put().to(update_inbound_route))
            .route("/{id}", web::delete().to(delete_inbound_route)),
    );
}
