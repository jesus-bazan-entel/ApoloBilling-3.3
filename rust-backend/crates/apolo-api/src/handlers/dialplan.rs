//! Dialplan handlers
//!
//! HTTP handlers for FreeSWITCH dialplan XML file management.

use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::SuperadminUser;
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{error, info, instrument, warn};

// File paths (hardcoded for security)
const FROM_PBX_XML: &str = "/etc/freeswitch/dialplan/from-pbx.xml";
const TO_KAMAILIO_XML: &str = "/etc/freeswitch/dialplan/from-kamailio.xml";

/// Default values for DTO fields
fn default_port() -> u16 {
    5060
}

fn default_profile() -> String {
    "external".to_string()
}

fn default_true() -> bool {
    true
}

/// A failover bridge destination (IP + port)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDestination {
    pub ip: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

/// Dialplan route representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialplanRoute {
    pub id: String,                     // Extension name (slug)
    pub name: String,                   // Display name
    pub priority: i32,                  // Order (lower = first)
    pub prefix_pattern: String,         // Regex for destination_number
    pub destination_ip: String,         // Bridge target IP
    pub destination_port: u16,          // Port (default 5060)
    pub sip_profile: String,            // "internal" or "external"
    pub bypass_media: bool,
    pub inherit_codec: bool,
    pub enable_100rel: bool,
    pub ignore_early_media: bool,
    pub call_timeout: Option<i32>,
    pub source_ip_filter: Option<String>, // Only inbound: network_addr filter
    #[serde(default)]
    pub failover_destinations: Vec<BridgeDestination>, // Failover targets (pipe-separated)
    pub enabled: bool,
}

/// Request to create/update a dialplan route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialplanRouteRequest {
    pub name: String,
    pub priority: i32,
    pub prefix_pattern: String,
    pub destination_ip: String,
    #[serde(default = "default_port")]
    pub destination_port: u16,
    #[serde(default = "default_profile")]
    pub sip_profile: String,
    #[serde(default)]
    pub bypass_media: bool,
    #[serde(default = "default_true")]
    pub inherit_codec: bool,
    #[serde(default)]
    pub enable_100rel: bool,
    #[serde(default)]
    pub ignore_early_media: bool,
    pub call_timeout: Option<i32>,
    pub source_ip_filter: Option<String>,
    #[serde(default)]
    pub failover_destinations: Vec<BridgeDestination>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Convert context parameter to file path
fn context_to_path(context: &str) -> Result<&'static str, AppError> {
    match context {
        "from_pbx" => Ok(FROM_PBX_XML),
        "to_kamailio" => Ok(TO_KAMAILIO_XML),
        _ => Err(AppError::Validation(format!(
            "Invalid context '{}'. Must be 'from_pbx' or 'to_kamailio'",
            context
        ))),
    }
}

/// Convert context parameter to context name in XML
fn context_to_name(context: &str) -> Result<&'static str, AppError> {
    match context {
        "from_pbx" => Ok("from-pbx"),
        "to_kamailio" => Ok("to-kamailio"),
        _ => Err(AppError::Validation(format!(
            "Invalid context '{}'",
            context
        ))),
    }
}

/// List routes for a context
///
/// GET /api/v1/dialplan/{context}
#[instrument(skip(pool, _admin))]
pub async fn list_routes(
    pool: web::Data<PgPool>,
    context: web::Path<String>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let context_str = context.as_str();
    let file_path = context_to_path(context_str)?;

    info!(context = %context_str, path = %file_path, "Listing dialplan routes");

    // Read and parse XML file
    let routes = read_routes_from_file(file_path, context_str).await?;

    // Log audit
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_admin.username.clone())
        .action("list_dialplan_routes")
        .entity_type("dialplan")
        .entity_id(context_str.to_string())
        .details(serde_json::json!({
            "context": context_str,
            "route_count": routes.len()
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Create a new route
///
/// POST /api/v1/dialplan/{context}
#[instrument(skip(pool, admin))]
pub async fn create_route(
    pool: web::Data<PgPool>,
    context: web::Path<String>,
    req: web::Json<DialplanRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let context_str = context.as_str();
    let file_path = context_to_path(context_str)?;

    // Validate request
    validate_route_request(&req)?;

    // Generate ID from name (slug)
    let route_id = slugify(&req.name);

    info!(
        context = %context_str,
        route_id = %route_id,
        name = %req.name,
        "Creating dialplan route"
    );

    // Read existing routes
    let mut routes = read_routes_from_file(file_path, context_str).await?;

    // Check for duplicate ID
    if routes.iter().any(|r| r.id == route_id) {
        return Err(AppError::Conflict(format!(
            "Route with ID '{}' already exists",
            route_id
        )));
    }

    // Create new route
    let new_route = DialplanRoute {
        id: route_id.clone(),
        name: req.name.clone(),
        priority: req.priority,
        prefix_pattern: req.prefix_pattern.clone(),
        destination_ip: req.destination_ip.clone(),
        destination_port: req.destination_port,
        sip_profile: req.sip_profile.clone(),
        bypass_media: req.bypass_media,
        inherit_codec: req.inherit_codec,
        enable_100rel: req.enable_100rel,
        ignore_early_media: req.ignore_early_media,
        call_timeout: req.call_timeout,
        source_ip_filter: req.source_ip_filter.clone(),
        failover_destinations: req.failover_destinations.clone(),
        enabled: req.enabled,
    };

    routes.push(new_route.clone());

    // Backup, write, and reload
    backup_and_write_routes(file_path, &routes, context_str).await?;
    reload_freeswitch_dialplan().await?;

    // Log audit
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_dialplan_route")
        .entity_type("dialplan")
        .entity_id(route_id.clone())
        .details(serde_json::json!({
            "context": context_str,
            "route": new_route
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        new_route,
        "Route created and FreeSWITCH dialplan reloaded",
    )))
}

/// Update an existing route
///
/// PUT /api/v1/dialplan/{context}/{id}
#[instrument(skip(pool, admin))]
pub async fn update_route(
    pool: web::Data<PgPool>,
    path: web::Path<(String, String)>,
    req: web::Json<DialplanRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let (context_str, route_id) = path.into_inner();
    let file_path = context_to_path(&context_str)?;

    // Validate request
    validate_route_request(&req)?;

    info!(
        context = %context_str,
        route_id = %route_id,
        "Updating dialplan route"
    );

    // Read existing routes
    let mut routes = read_routes_from_file(file_path, &context_str).await?;

    // Find and update the route
    let route = routes
        .iter_mut()
        .find(|r| r.id == route_id)
        .ok_or_else(|| AppError::NotFound(format!("Route '{}' not found", route_id)))?;

    // Update fields
    route.name = req.name.clone();
    route.priority = req.priority;
    route.prefix_pattern = req.prefix_pattern.clone();
    route.destination_ip = req.destination_ip.clone();
    route.destination_port = req.destination_port;
    route.sip_profile = req.sip_profile.clone();
    route.bypass_media = req.bypass_media;
    route.inherit_codec = req.inherit_codec;
    route.enable_100rel = req.enable_100rel;
    route.ignore_early_media = req.ignore_early_media;
    route.call_timeout = req.call_timeout;
    route.source_ip_filter = req.source_ip_filter.clone();
    route.failover_destinations = req.failover_destinations.clone();
    route.enabled = req.enabled;

    let updated_route = route.clone();

    // Backup, write, and reload
    backup_and_write_routes(file_path, &routes, &context_str).await?;
    reload_freeswitch_dialplan().await?;

    // Log audit
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_dialplan_route")
        .entity_type("dialplan")
        .entity_id(route_id.clone())
        .details(serde_json::json!({
            "context": context_str,
            "route": updated_route
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        updated_route,
        "Route updated and FreeSWITCH dialplan reloaded",
    )))
}

/// Delete a route
///
/// DELETE /api/v1/dialplan/{context}/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_route(
    pool: web::Data<PgPool>,
    path: web::Path<(String, String)>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let (context_str, route_id) = path.into_inner();
    let file_path = context_to_path(&context_str)?;

    info!(
        context = %context_str,
        route_id = %route_id,
        "Deleting dialplan route"
    );

    // Read existing routes
    let mut routes = read_routes_from_file(file_path, &context_str).await?;

    // Find the route to delete
    let initial_len = routes.len();
    routes.retain(|r| r.id != route_id);

    if routes.len() == initial_len {
        return Err(AppError::NotFound(format!("Route '{}' not found", route_id)));
    }

    // Backup, write, and reload
    backup_and_write_routes(file_path, &routes, &context_str).await?;
    reload_freeswitch_dialplan().await?;

    // Log audit
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_dialplan_route")
        .entity_type("dialplan")
        .entity_id(route_id.clone())
        .details(serde_json::json!({
            "context": context_str,
            "route_id": route_id
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "deleted": route_id }),
        "Route deleted and FreeSWITCH dialplan reloaded",
    )))
}

/// Force reload FreeSWITCH dialplan
///
/// POST /api/v1/dialplan/reload
#[instrument(skip(pool, admin))]
pub async fn reload_dialplan(
    pool: web::Data<PgPool>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Manual dialplan reload requested");

    reload_freeswitch_dialplan().await?;

    // Log audit
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("reload_dialplan")
        .entity_type("dialplan")
        .entity_id("manual".to_string())
        .details(serde_json::json!({
            "manual_reload": true
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "reloaded": true }),
        "FreeSWITCH dialplan reloaded successfully",
    )))
}

/// Configure routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/dialplan")
            .route("/reload", web::post().to(reload_dialplan))
            .route("/{context}", web::get().to(list_routes))
            .route("/{context}", web::post().to(create_route))
            .route("/{context}/{id}", web::put().to(update_route))
            .route("/{context}/{id}", web::delete().to(delete_route)),
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validate route request
fn validate_route_request(req: &DialplanRouteRequest) -> Result<(), AppError> {
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Route name cannot be empty".to_string()));
    }

    if req.prefix_pattern.trim().is_empty() {
        return Err(AppError::Validation(
            "Prefix pattern cannot be empty".to_string(),
        ));
    }

    if req.destination_ip.trim().is_empty() {
        return Err(AppError::Validation(
            "Destination IP cannot be empty".to_string(),
        ));
    }

    if req.destination_port == 0 {
        return Err(AppError::Validation(
            "Destination port must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

/// Convert name to slug (lowercase, alphanumeric + hyphens)
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

/// Read routes from XML file
async fn read_routes_from_file(
    file_path: &str,
    context: &str,
) -> Result<Vec<DialplanRoute>, AppError> {
    // Check if file exists
    if !Path::new(file_path).exists() {
        warn!(path = %file_path, "Dialplan file does not exist, returning empty list");
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(file_path).await.map_err(|e| {
        error!(path = %file_path, error = %e, "Failed to read dialplan file");
        AppError::Internal(format!("Failed to read dialplan file: {}", e))
    })?;

    parse_routes_from_xml(&content, context)
}

/// Parse routes from XML content using simple string parsing
fn parse_routes_from_xml(xml: &str, context: &str) -> Result<Vec<DialplanRoute>, AppError> {
    let mut routes = Vec::new();

    // Split by <extension> tags
    let parts: Vec<&str> = xml.split("<extension").collect();

    for part in parts.iter().skip(1) {
        // Skip first empty part
        if let Some(end_idx) = part.find("</extension>") {
            let extension_content = &part[..end_idx];

            // Skip fallback-reject extension
            if extension_content.contains("name=\"fallback-reject\"") {
                continue;
            }

            if let Some(route) = parse_single_extension(extension_content, context) {
                routes.push(route);
            }
        }
    }

    Ok(routes)
}

/// Parse a single extension into a DialplanRoute
fn parse_single_extension(content: &str, context: &str) -> Option<DialplanRoute> {
    // Extract extension name
    let name = extract_attribute(content, "name")?;
    let id = name.clone();

    // Extract conditions and actions
    let prefix_pattern = extract_destination_pattern(content)?;
    let source_ip_filter = extract_source_ip_filter(content);

    // Parse bridge action to get destination(s)
    let (destination_ip, destination_port, sip_profile, failover_destinations) =
        parse_bridge_action(content)?;

    // Extract boolean flags from set actions
    let bypass_media = contains_set_action(content, "bypass_media=true");
    let inherit_codec = contains_set_action(content, "inherit_codec=true");
    let enable_100rel = contains_set_action(content, "sip_enable_100rel=true");
    let ignore_early_media = contains_set_action(content, "ignore_early_media=true");

    // Extract call_timeout
    let call_timeout = extract_call_timeout(content);

    // Assign default priority based on order (we'll sort properly when writing)
    let priority = 100;

    Some(DialplanRoute {
        id,
        name,
        priority,
        prefix_pattern,
        destination_ip,
        destination_port,
        sip_profile,
        bypass_media,
        inherit_codec,
        enable_100rel,
        ignore_early_media,
        call_timeout,
        source_ip_filter,
        failover_destinations,
        enabled: true,
    })
}

/// Extract attribute value from XML
fn extract_attribute(content: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr_name);
    if let Some(start_idx) = content.find(&pattern) {
        let value_start = start_idx + pattern.len();
        if let Some(end_idx) = content[value_start..].find('"') {
            return Some(content[value_start..value_start + end_idx].to_string());
        }
    }
    None
}

/// Extract destination_number pattern
fn extract_destination_pattern(content: &str) -> Option<String> {
    // Look for: field="destination_number" expression="PATTERN"
    if let Some(field_idx) = content.find("field=\"destination_number\"") {
        let search_area = &content[field_idx..];
        if let Some(expr_idx) = search_area.find("expression=\"") {
            let value_start = expr_idx + "expression=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                return Some(search_area[value_start..value_start + end_idx].to_string());
            }
        }
    }
    None
}

/// Extract source IP filter from network_addr condition
fn extract_source_ip_filter(content: &str) -> Option<String> {
    // Look for: field="${network_addr}" expression="PATTERN"
    if let Some(field_idx) = content.find("field=\"${network_addr}\"") {
        let search_area = &content[field_idx..];
        if let Some(expr_idx) = search_area.find("expression=\"") {
            let value_start = expr_idx + "expression=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                return Some(search_area[value_start..value_start + end_idx].to_string());
            }
        }
    }
    None
}

/// Parse a single sofia URI segment into (IP, port, profile)
fn parse_sofia_segment(segment: &str) -> Option<(String, u16, String)> {
    let sofia_start = segment.find("sofia/")?;
    let sofia_part = &segment[sofia_start + "sofia/".len()..];

    // Extract profile
    let slash_idx = sofia_part.find('/')?;
    let profile = &sofia_part[..slash_idx];

    // Extract IP and port from @IP:PORT
    let at_idx = sofia_part.find('@')?;
    let after_at = &sofia_part[at_idx + 1..];

    if let Some(colon_idx) = after_at.find(':') {
        let ip = after_at[..colon_idx].to_string();
        let port_str = after_at[colon_idx + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        if let Ok(port) = port_str.parse::<u16>() {
            return Some((ip, port, profile.to_string()));
        }
    } else {
        let ip = after_at.to_string();
        return Some((ip, 5060, profile.to_string()));
    }
    None
}

/// Parse bridge action to extract primary destination + failover destinations
fn parse_bridge_action(content: &str) -> Option<(String, u16, String, Vec<BridgeDestination>)> {
    // Look for: <action application="bridge" data="...sofia/PROFILE/...@IP:PORT|sofia/..."/>
    if let Some(bridge_idx) = content.find("application=\"bridge\"") {
        let search_area = &content[bridge_idx..];
        if let Some(data_idx) = search_area.find("data=\"") {
            let value_start = data_idx + "data=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                let bridge_data = &search_area[value_start..value_start + end_idx];

                // Split on pipe for failover destinations
                let segments: Vec<&str> = bridge_data.split('|').collect();

                // Parse primary destination (first segment)
                let (ip, port, profile) = parse_sofia_segment(segments[0])?;

                // Parse failover destinations (remaining segments)
                let mut failover = Vec::new();
                for segment in segments.iter().skip(1) {
                    if let Some((fo_ip, fo_port, _)) = parse_sofia_segment(segment) {
                        failover.push(BridgeDestination { ip: fo_ip, port: fo_port });
                    }
                }

                return Some((ip, port, profile, failover));
            }
        }
    }
    None
}

/// Check if content contains a specific set action
fn contains_set_action(content: &str, setting: &str) -> bool {
    let pattern = format!("data=\"{}\"", setting);
    content.contains(&pattern)
}

/// Extract call_timeout value
fn extract_call_timeout(content: &str) -> Option<i32> {
    if let Some(idx) = content.find("data=\"call_timeout=") {
        let value_start = idx + "data=\"call_timeout=".len();
        if let Some(end_idx) = content[value_start..].find('"') {
            let timeout_str = &content[value_start..value_start + end_idx];
            return timeout_str.parse::<i32>().ok();
        }
    }
    None
}

/// Backup and write routes to file
async fn backup_and_write_routes(
    file_path: &str,
    routes: &[DialplanRoute],
    context: &str,
) -> Result<(), AppError> {
    // Create backup
    if Path::new(file_path).exists() {
        let backup_path = format!("{}.bak", file_path);
        fs::copy(file_path, &backup_path).await.map_err(|e| {
            error!(path = %file_path, error = %e, "Failed to create backup");
            AppError::Internal(format!("Failed to create backup: {}", e))
        })?;
        info!(backup = %backup_path, "Created backup of dialplan file");
    }

    // Generate XML
    let xml_content = generate_xml(routes, context)?;

    // Write to file
    fs::write(file_path, xml_content).await.map_err(|e| {
        error!(path = %file_path, error = %e, "Failed to write dialplan file");
        AppError::Internal(format!("Failed to write dialplan file: {}", e))
    })?;

    info!(path = %file_path, route_count = routes.len(), "Wrote dialplan file");

    Ok(())
}

/// Generate XML content from routes
fn generate_xml(routes: &[DialplanRoute], context: &str) -> Result<String, AppError> {
    let context_name = context_to_name(context)?;

    // Sort routes by priority (ascending)
    let mut sorted_routes = routes.to_vec();
    sorted_routes.sort_by_key(|r| r.priority);

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<include>\n");
    xml.push_str(&format!("  <context name=\"{}\">\n", context_name));

    // Generate extensions
    for route in sorted_routes.iter().filter(|r| r.enabled) {
        if context == "from_pbx" {
            xml.push_str(&generate_from_pbx_extension(route));
        } else {
            xml.push_str(&generate_to_kamailio_extension(route));
        }
    }

    // Add fallback-reject for to-kamailio context
    if context == "to_kamailio" {
        xml.push_str("    <extension name=\"fallback-reject\">\n");
        xml.push_str("      <condition field=\"destination_number\" expression=\"^(.+)$\">\n");
        xml.push_str("        <action application=\"log\" data=\"WARNING Rejecting call from unknown source: ${network_addr}\"/>\n");
        xml.push_str("        <action application=\"respond\" data=\"403 Forbidden\"/>\n");
        xml.push_str("      </condition>\n");
        xml.push_str("    </extension>\n");
    }

    xml.push_str("  </context>\n");
    xml.push_str("</include>\n");

    Ok(xml)
}

/// Generate extension XML for from-pbx context
fn generate_from_pbx_extension(route: &DialplanRoute) -> String {
    let mut ext = String::new();

    ext.push_str(&format!("    <extension name=\"{}\">\n", route.id));
    ext.push_str(&format!(
        "      <condition field=\"destination_number\" expression=\"{}\">\n",
        escape_xml(&route.prefix_pattern)
    ));
    ext.push_str(&format!(
        "        <action application=\"log\" data=\"INFO FS: Ruta {} desde ${{network_ip}} → $1\"/>\n",
        route.name
    ));

    // Set actions
    ext.push_str(&format!(
        "        <action application=\"set\" data=\"bypass_media={}\"/>\n",
        route.bypass_media
    ));
    ext.push_str(&format!(
        "        <action application=\"set\" data=\"inherit_codec={}\"/>\n",
        route.inherit_codec
    ));

    if route.enable_100rel {
        ext.push_str("        <action application=\"set\" data=\"sip_enable_100rel=true\"/>\n");
        ext.push_str("        <action application=\"export\" data=\"sip_enable_100rel=true\"/>\n");
    }

    if route.ignore_early_media {
        ext.push_str("        <action application=\"set\" data=\"ignore_early_media=true\"/>\n");
    }

    if let Some(timeout) = route.call_timeout {
        ext.push_str(&format!(
            "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
            timeout
        ));
    }

    ext.push_str("        <action application=\"export\" data=\"sip_h_P-Asserted-Identity=${sip_h_P-Asserted-Identity}\"/>\n");

    // Bridge action with failover
    let bridge_prefix = if route.bypass_media {
        "{bypass_media=true}"
    } else {
        ""
    };
    let mut bridge_data = format!(
        "{}sofia/{}/$1@{}:{}",
        bridge_prefix, route.sip_profile, route.destination_ip, route.destination_port
    );
    for fo in &route.failover_destinations {
        bridge_data.push_str(&format!(
            "|sofia/{}/$1@{}:{}",
            route.sip_profile, fo.ip, fo.port
        ));
    }
    ext.push_str(&format!(
        "        <action application=\"bridge\" data=\"{}\"/>\n",
        bridge_data
    ));

    ext.push_str("      </condition>\n");
    ext.push_str("    </extension>\n");

    ext
}

/// Generate extension XML for to-kamailio context
fn generate_to_kamailio_extension(route: &DialplanRoute) -> String {
    let mut ext = String::new();

    ext.push_str(&format!("    <extension name=\"{}\">\n", route.id));

    // Network address filter if specified
    if let Some(ref source_filter) = route.source_ip_filter {
        ext.push_str(&format!(
            "      <condition field=\"${{network_addr}}\" expression=\"{}\">\n",
            escape_xml(source_filter)
        ));
        ext.push_str(&format!(
            "        <condition field=\"destination_number\" expression=\"{}\">\n",
            escape_xml(&route.prefix_pattern)
        ));
        ext.push_str(&format!(
            "          <action application=\"log\" data=\"INFO Inbound {}: ${{caller_id_number}} -> ${{destination_number}}\"/>\n",
            route.name
        ));

        // Set actions
        if let Some(timeout) = route.call_timeout {
            ext.push_str(&format!(
                "          <action application=\"set\" data=\"call_timeout={}\"/>\n",
                timeout
            ));
            ext.push_str(&format!(
                "          <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                timeout
            ));
        }

        if route.ignore_early_media {
            ext.push_str("          <action application=\"set\" data=\"ignore_early_media=true\"/>\n");
        }

        if route.inherit_codec {
            ext.push_str("          <action application=\"set\" data=\"inherit_codec=true\"/>\n");
        }

        // Bridge with failover
        let mut bridge_data = format!(
            "sofia/{}/$1@{}:{}",
            route.sip_profile, route.destination_ip, route.destination_port
        );
        for fo in &route.failover_destinations {
            bridge_data.push_str(&format!(
                "|sofia/{}/$1@{}:{}",
                route.sip_profile, fo.ip, fo.port
            ));
        }
        ext.push_str(&format!(
            "          <action application=\"bridge\" data=\"{}\"/>\n",
            bridge_data
        ));

        ext.push_str("        </condition>\n");
        ext.push_str("      </condition>\n");
    } else {
        // No source filter
        ext.push_str(&format!(
            "      <condition field=\"destination_number\" expression=\"{}\">\n",
            escape_xml(&route.prefix_pattern)
        ));
        ext.push_str(&format!(
            "        <action application=\"log\" data=\"INFO Inbound {}: ${{caller_id_number}} -> ${{destination_number}}\"/>\n",
            route.name
        ));

        // Set actions
        if let Some(timeout) = route.call_timeout {
            ext.push_str(&format!(
                "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
                timeout
            ));
            ext.push_str(&format!(
                "        <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                timeout
            ));
        }

        if route.ignore_early_media {
            ext.push_str("        <action application=\"set\" data=\"ignore_early_media=true\"/>\n");
        }

        if route.inherit_codec {
            ext.push_str("        <action application=\"set\" data=\"inherit_codec=true\"/>\n");
        }

        // Bridge with failover
        let mut bridge_data = format!(
            "sofia/{}/$1@{}:{}",
            route.sip_profile, route.destination_ip, route.destination_port
        );
        for fo in &route.failover_destinations {
            bridge_data.push_str(&format!(
                "|sofia/{}/$1@{}:{}",
                route.sip_profile, fo.ip, fo.port
            ));
        }
        ext.push_str(&format!(
            "        <action application=\"bridge\" data=\"{}\"/>\n",
            bridge_data
        ));

        ext.push_str("      </condition>\n");
    }

    ext.push_str("    </extension>\n");

    ext
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Reload FreeSWITCH dialplan via fs_cli
async fn reload_freeswitch_dialplan() -> Result<(), AppError> {
    use tokio::process::Command;

    info!("Executing fs_cli -x reloadxml");

    let output = Command::new("fs_cli")
        .args(["-x", "reloadxml"])
        .output()
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to execute fs_cli");
            AppError::Internal(format!("Failed to execute fs_cli: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(stderr = %stderr, "fs_cli returned non-zero status, but continuing");
        // Don't fail - XML was written successfully
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        info!(stdout = %stdout, "FreeSWITCH dialplan reloaded successfully");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Outbound to Kamailio"), "outbound-to-kamailio");
        assert_eq!(slugify("Test Route 123"), "test-route-123");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Special!@#$%Characters"), "special-characters");
    }

    #[test]
    fn test_context_to_path() {
        assert_eq!(context_to_path("from_pbx").unwrap(), FROM_PBX_XML);
        assert_eq!(context_to_path("to_kamailio").unwrap(), TO_KAMAILIO_XML);
        assert!(context_to_path("invalid").is_err());
    }

    #[test]
    fn test_context_to_name() {
        assert_eq!(context_to_name("from_pbx").unwrap(), "from-pbx");
        assert_eq!(context_to_name("to_kamailio").unwrap(), "to-kamailio");
        assert!(context_to_name("invalid").is_err());
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("normal text"), "normal text");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("A & B"), "A &amp; B");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_validate_route_request() {
        let valid_req = DialplanRouteRequest {
            name: "Test Route".to_string(),
            priority: 100,
            prefix_pattern: "^(.+)$".to_string(),
            destination_ip: "10.10.22.18".to_string(),
            destination_port: 5060,
            sip_profile: "external".to_string(),
            bypass_media: true,
            inherit_codec: true,
            enable_100rel: false,
            ignore_early_media: false,
            call_timeout: None,
            source_ip_filter: None,
            failover_destinations: vec![],
            enabled: true,
        };

        assert!(validate_route_request(&valid_req).is_ok());

        let invalid_name = DialplanRouteRequest {
            name: "".to_string(),
            ..valid_req.clone()
        };
        assert!(validate_route_request(&invalid_name).is_err());

        let invalid_pattern = DialplanRouteRequest {
            prefix_pattern: "".to_string(),
            ..valid_req.clone()
        };
        assert!(validate_route_request(&invalid_pattern).is_err());

        let invalid_ip = DialplanRouteRequest {
            destination_ip: "".to_string(),
            ..valid_req.clone()
        };
        assert!(validate_route_request(&invalid_ip).is_err());

        let invalid_port = DialplanRouteRequest {
            destination_port: 0,
            ..valid_req
        };
        assert!(validate_route_request(&invalid_port).is_err());
    }

    #[test]
    fn test_parse_single_extension_from_pbx() {
        let xml = r#"name="outbound-to-kamailio">
      <condition field="destination_number" expression="^(.+)$">
        <action application="log" data="INFO FS: Llamada desde ${network_ip} → $1"/>
        <action application="set" data="bypass_media=true"/>
        <action application="set" data="inherit_codec=true"/>
        <action application="set" data="sip_enable_100rel=true"/>
        <action application="export" data="sip_enable_100rel=true"/>
        <action application="bridge" data="{bypass_media=true}sofia/external/$1@10.10.22.18:5060"/>
      </condition>"#;

        let route = parse_single_extension(xml, "from_pbx").unwrap();

        assert_eq!(route.id, "outbound-to-kamailio");
        assert_eq!(route.prefix_pattern, "^(.+)$");
        assert_eq!(route.destination_ip, "10.10.22.18");
        assert_eq!(route.destination_port, 5060);
        assert_eq!(route.sip_profile, "external");
        assert!(route.bypass_media);
        assert!(route.inherit_codec);
        assert!(route.enable_100rel);
        assert!(!route.ignore_early_media);
        assert!(route.failover_destinations.is_empty());
    }

    #[test]
    fn test_parse_bridge_with_failover() {
        let xml = r#"name="outbound-failover">
      <condition field="destination_number" expression="^(.+)$">
        <action application="set" data="bypass_media=true"/>
        <action application="bridge" data="{bypass_media=true}sofia/external/$1@10.10.22.18:5060|sofia/external/$1@10.10.22.100:5060|sofia/external/$1@10.10.22.200:5070"/>
      </condition>"#;

        let route = parse_single_extension(xml, "from_pbx").unwrap();

        assert_eq!(route.destination_ip, "10.10.22.18");
        assert_eq!(route.destination_port, 5060);
        assert_eq!(route.failover_destinations.len(), 2);
        assert_eq!(route.failover_destinations[0].ip, "10.10.22.100");
        assert_eq!(route.failover_destinations[0].port, 5060);
        assert_eq!(route.failover_destinations[1].ip, "10.10.22.200");
        assert_eq!(route.failover_destinations[1].port, 5070);
    }

    #[test]
    fn test_parse_single_extension_to_kamailio() {
        let xml = r#"name="inbound-from-kamailio">
      <condition field="${network_addr}" expression="^10\.10\.22\.18$">
        <condition field="destination_number" expression="^(.+)$">
          <action application="log" data="INFO Inbound from Kamailio: ${caller_id_number} -> ${destination_number}"/>
          <action application="set" data="call_timeout=120"/>
          <action application="set" data="ignore_early_media=true"/>
          <action application="set" data="inherit_codec=true"/>
          <action application="bridge" data="sofia/internal/${destination_number}@10.10.22.7:5060"/>
        </condition>
      </condition>"#;

        let route = parse_single_extension(xml, "to_kamailio").unwrap();

        assert_eq!(route.id, "inbound-from-kamailio");
        assert_eq!(route.prefix_pattern, "^(.+)$");
        assert_eq!(route.destination_ip, "10.10.22.7");
        assert_eq!(route.destination_port, 5060);
        assert_eq!(route.sip_profile, "internal");
        assert_eq!(route.source_ip_filter, Some("^10\\.10\\.22\\.18$".to_string()));
        assert_eq!(route.call_timeout, Some(120));
        assert!(route.ignore_early_media);
        assert!(route.inherit_codec);
        assert!(route.failover_destinations.is_empty());
    }

    #[test]
    fn test_generate_xml_from_pbx() {
        let routes = vec![DialplanRoute {
            id: "test-route".to_string(),
            name: "Test Route".to_string(),
            priority: 100,
            prefix_pattern: "^(.+)$".to_string(),
            destination_ip: "10.10.22.18".to_string(),
            destination_port: 5060,
            sip_profile: "external".to_string(),
            bypass_media: true,
            inherit_codec: true,
            enable_100rel: true,
            ignore_early_media: false,
            call_timeout: None,
            source_ip_filter: None,
            failover_destinations: vec![],
            enabled: true,
        }];

        let xml = generate_xml(&routes, "from_pbx").unwrap();

        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("<context name=\"from-pbx\">"));
        assert!(xml.contains("<extension name=\"test-route\">"));
        assert!(xml.contains("bypass_media=true"));
        assert!(xml.contains("inherit_codec=true"));
        assert!(xml.contains("sip_enable_100rel=true"));
        assert!(xml.contains("sofia/external/$1@10.10.22.18:5060"));
    }

    #[test]
    fn test_generate_xml_from_pbx_with_failover() {
        let routes = vec![DialplanRoute {
            id: "test-failover".to_string(),
            name: "Test Failover".to_string(),
            priority: 100,
            prefix_pattern: "^(.+)$".to_string(),
            destination_ip: "10.10.22.18".to_string(),
            destination_port: 5060,
            sip_profile: "external".to_string(),
            bypass_media: true,
            inherit_codec: true,
            enable_100rel: false,
            ignore_early_media: false,
            call_timeout: None,
            source_ip_filter: None,
            failover_destinations: vec![
                BridgeDestination { ip: "10.10.22.100".to_string(), port: 5060 },
                BridgeDestination { ip: "10.10.22.200".to_string(), port: 5070 },
            ],
            enabled: true,
        }];

        let xml = generate_xml(&routes, "from_pbx").unwrap();

        assert!(xml.contains("{bypass_media=true}sofia/external/$1@10.10.22.18:5060|sofia/external/$1@10.10.22.100:5060|sofia/external/$1@10.10.22.200:5070"));
    }

    #[test]
    fn test_generate_xml_to_kamailio() {
        let routes = vec![DialplanRoute {
            id: "inbound-test".to_string(),
            name: "Inbound Test".to_string(),
            priority: 100,
            prefix_pattern: "^(.+)$".to_string(),
            destination_ip: "10.10.22.7".to_string(),
            destination_port: 5060,
            sip_profile: "internal".to_string(),
            bypass_media: false,
            inherit_codec: true,
            enable_100rel: false,
            ignore_early_media: true,
            call_timeout: Some(120),
            source_ip_filter: Some("^10\\.10\\.22\\.18$".to_string()),
            failover_destinations: vec![],
            enabled: true,
        }];

        let xml = generate_xml(&routes, "to_kamailio").unwrap();

        assert!(xml.contains("<context name=\"to-kamailio\">"));
        assert!(xml.contains("<extension name=\"inbound-test\">"));
        assert!(xml.contains("${network_addr}"));
        assert!(xml.contains("^10\\.10\\.22\\.18$"));
        assert!(xml.contains("call_timeout=120"));
        assert!(xml.contains("ignore_early_media=true"));
        assert!(xml.contains("sofia/internal/$1@10.10.22.7:5060"));
        assert!(xml.contains("fallback-reject"));
    }

    #[test]
    fn test_generate_xml_to_kamailio_with_failover() {
        let routes = vec![DialplanRoute {
            id: "inbound-fo".to_string(),
            name: "Inbound Failover".to_string(),
            priority: 100,
            prefix_pattern: "^(.+)$".to_string(),
            destination_ip: "10.10.22.7".to_string(),
            destination_port: 5060,
            sip_profile: "internal".to_string(),
            bypass_media: false,
            inherit_codec: true,
            enable_100rel: false,
            ignore_early_media: false,
            call_timeout: None,
            source_ip_filter: Some("^10\\.10\\.22\\.18$".to_string()),
            failover_destinations: vec![
                BridgeDestination { ip: "10.10.22.100".to_string(), port: 5060 },
            ],
            enabled: true,
        }];

        let xml = generate_xml(&routes, "to_kamailio").unwrap();

        assert!(xml.contains("sofia/internal/$1@10.10.22.7:5060|sofia/internal/$1@10.10.22.100:5060"));
    }
}
