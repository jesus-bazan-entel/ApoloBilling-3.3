//! SIP Endpoint handlers
//!
//! Unified CRUD for everything that talks SIP to this platform: a single
//! extension/softphone or a whole PBX/trunk, authenticated either by SIP
//! digest credentials (REGISTER via mod_xml_curl) or by trusted source IP
//! (Kamailio dr_gateways type=9). See `apolo_core::models::sip_endpoint` for
//! the `EndpointType`/`AuthMode` axes.

use crate::dto::sip_endpoint::{
    FreeswitchIpRequest, FreeswitchIpResponse, PasswordRegenerateResponse,
    SipEndpointCreateRequest, SipEndpointFilterParams, SipEndpointResponse,
    SipEndpointUpdateRequest, SipEndpointWithPasswordResponse, SipRegistrationStatusResponse,
};
use crate::dto::{ApiResponse, PaginationParams};
use crate::handlers::kamailio_gateway::{
    gateway_address, gateway_attrs, signalling_from_transport, GW_TYPE_PBX,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::models::{AuditLogBuilder, AuthMode, EndpointType};
use apolo_core::traits::{FreeswitchIpRepository, Repository, SipEndpointRepository};
use apolo_core::AppError;
use apolo_db::{PgFreeswitchIpRepository, PgSipEndpointRepository};
use apolo_services::SipEndpointService;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

const MODULE_KEY: &str = "sip_devices";

/// List SIP endpoints with pagination and filters
///
/// GET /api/v1/sip-endpoints
#[instrument(skip(pool, user))]
pub async fn list_sip_endpoints(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    filters: web::Query<SipEndpointFilterParams>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module(MODULE_KEY) {
        return Err(AppError::Forbidden);
    }

    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        page = query.page,
        per_page = query.per_page,
        "Listing SIP endpoints"
    );

    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());

    let (endpoints, total) = repo
        .list_filtered(
            filters.account_id,
            filters.enabled,
            query.limit(),
            query.offset(),
        )
        .await?;

    let response_data: Vec<SipEndpointResponse> = endpoints.into_iter().map(|e| e.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Get a single SIP endpoint by ID
///
/// GET /api/v1/sip-endpoints/{id}
#[instrument(skip(pool, user))]
pub async fn get_sip_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module(MODULE_KEY) {
        return Err(AppError::Forbidden);
    }

    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(SipEndpointResponse::from(endpoint))))
}

/// Create a new SIP endpoint
///
/// POST /api/v1/sip-endpoints
#[instrument(skip(pool, sip_service, admin, req))]
pub async fn create_sip_endpoint(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipEndpointService>>,
    admin: AdminUser,
    req: web::Json<SipEndpointCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("SIP endpoint creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    validate_type_auth_combo(req.endpoint_type, req.auth_mode)?;
    validate_channel_limit(req.endpoint_type, req.max_registrations)?;
    validate_call_limit(req.endpoint_type, req.max_concurrent_calls)?;

    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());

    let password = match req.auth_mode {
        AuthMode::Credentials => {
            let username = req
                .sip_username
                .clone()
                .ok_or_else(|| AppError::Validation("sip_username is required".to_string()))?;

            if repo.username_exists(&username, &req.sip_domain).await? {
                return Err(AppError::AlreadyExists(format!(
                    "SIP endpoint {}@{} already exists",
                    username, req.sip_domain
                )));
            }

            Some(
                req.password
                    .clone()
                    .unwrap_or_else(SipEndpointService::generate_password),
            )
        }
        AuthMode::Ip => {
            if req.host.as_deref().unwrap_or("").trim().is_empty() {
                return Err(AppError::Validation(
                    "host is required when auth_mode=ip".to_string(),
                ));
            }
            None
        }
    };

    let mut endpoint = req.to_endpoint();
    endpoint.did_ranges = normalize_did_ranges(req.did_ranges.as_deref().unwrap_or(""))?;
    if !endpoint.did_ranges.is_empty()
        && (req.endpoint_type != EndpointType::Trunk || req.auth_mode != AuthMode::Ip)
    {
        return Err(AppError::Validation(
            "did_ranges solo aplica a troncales IP (endpoint_type=trunk, auth_mode=ip)".to_string(),
        ));
    }
    check_did_range_conflicts(pool.get_ref(), None, &endpoint.did_ranges).await?;

    if let Some(ref pwd) = password {
        sip_service.prepare_device_with_password(&mut endpoint, pwd)?;
    }

    if endpoint.is_default {
        repo.clear_default_trunk(None).await?;
    }

    let created = repo.create(&endpoint).await?;

    // auth_mode = ip trunks get synced to Kamailio (dr_gateways type=9 + ACL
    // + call_settings limit) and reloaded so gateway and limit go live now.
    let created = if created.auth_mode == AuthMode::Ip {
        let synced = sync_trunk_to_kamailio(&repo, created).await;
        if let Err(e) = reload_kamailio_routing().await {
            warn!(error = %e, "Failed to reload Kamailio routing after create");
        }
        // El bridge entrante a esta PBX es directo FreeSWITCH->host:port (ver
        // build_bridge/L3 SELECTIVO en inbound_routing.rs): CHECK_CALL_LIMIT
        // de Kamailio no está en ese camino. Hay que regenerar el dialplan
        // para que el `limit` nativo (mod_hash) tome host/puerto/limite.
        if let Err(e) = crate::handlers::inbound_routing::sync_inbound_routes_to_freeswitch(
            pool.get_ref(),
        )
        .await
        {
            warn!(error = %e, "Failed to regenerate inbound dialplan after create");
        }
        synced
    } else {
        created
    };

    info!(id = %created.id, name = %created.name, "SIP endpoint created successfully");

    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_sip_endpoint")
        .entity_type("sip_endpoint")
        .entity_id(created.id.to_string())
        .details(serde_json::json!({
            "name": created.name,
            "endpoint_type": created.endpoint_type.to_string(),
            "auth_mode": created.auth_mode.to_string(),
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    let response = match password {
        Some(password) => serde_json::to_value(SipEndpointWithPasswordResponse {
            endpoint: SipEndpointResponse::from(created),
            password,
        }),
        None => serde_json::to_value(SipEndpointResponse::from(created)),
    }
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "SIP endpoint created successfully",
    )))
}

/// Update a SIP endpoint
///
/// PUT /api/v1/sip-endpoints/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_sip_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<SipEndpointUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("SIP endpoint update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());

    let mut endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    if let Some(ref name) = req.name {
        endpoint.name = name.clone();
    }
    if let Some(ref description) = req.description {
        endpoint.description = Some(description.clone());
    }
    // Allow setting account_id to None (null) by checking if the field is explicitly provided
    // We need to distinguish between "not provided" and "explicitly set to null"
    // Since Option<i32> doesn't distinguish, we check if the field was in the request
    // by seeing if the JSON value was null vs missing
    endpoint.account_id = req.account_id;
    if let Some(ref username) = req.sip_username {
        endpoint.sip_username = Some(username.clone());
    }
    if let Some(ref domain) = req.sip_domain {
        endpoint.sip_domain = Some(domain.clone());
    }
    if let Some(max_registrations) = req.max_registrations {
        validate_channel_limit(endpoint.endpoint_type, max_registrations)?;
        endpoint.max_registrations = max_registrations;
    }
    if let Some(max_concurrent_calls) = req.max_concurrent_calls {
        validate_call_limit(endpoint.endpoint_type, max_concurrent_calls)?;
        endpoint.max_concurrent_calls = max_concurrent_calls;
    }
    if let Some(ref host) = req.host {
        endpoint.host = Some(host.clone());
    }
    if let Some(port) = req.port {
        endpoint.port = port;
    }
    if let Some(ref transport) = req.transport {
        endpoint.transport = transport.to_string();
    }
    if let Some(ref display_name) = req.display_name {
        endpoint.display_name = Some(display_name.clone());
    }
    if let Some(ref accountcode) = req.accountcode {
        endpoint.accountcode = Some(accountcode.clone());
    }
    if let Some(ref codecs) = req.codecs {
        endpoint.codecs = codecs.clone();
    }
    if let Some(enabled) = req.enabled {
        endpoint.enabled = enabled;
    }
    if let Some(is_default) = req.is_default {
        endpoint.is_default = is_default;
        if is_default {
            repo.clear_default_trunk(Some(id)).await?;
        }
    }
    if let Some(ref did_ranges) = req.did_ranges {
        let canonical = normalize_did_ranges(did_ranges)?;
        if !canonical.is_empty()
            && (endpoint.endpoint_type != EndpointType::Trunk || endpoint.auth_mode != AuthMode::Ip)
        {
            return Err(AppError::Validation(
                "did_ranges solo aplica a troncales IP (endpoint_type=trunk, auth_mode=ip)"
                    .to_string(),
            ));
        }
        check_did_range_conflicts(pool.get_ref(), Some(id), &canonical).await?;
        endpoint.did_ranges = canonical;
    }

    let updated = repo.update(&endpoint).await?;

    // Re-sync + reload so a changed host/port/limit goes live now.
    let updated = if updated.auth_mode == AuthMode::Ip {
        let synced = sync_trunk_to_kamailio(&repo, updated).await;
        if let Err(e) = reload_kamailio_routing().await {
            warn!(error = %e, "Failed to reload Kamailio routing after update");
        }
        // Idem create: el `limit` nativo embebido en el dialplan entrante
        // necesita regenerarse para reflejar host/puerto/max_concurrent_calls
        // nuevos (ver nota en create_sip_endpoint).
        if let Err(e) = crate::handlers::inbound_routing::sync_inbound_routes_to_freeswitch(
            pool.get_ref(),
        )
        .await
        {
            warn!(error = %e, "Failed to regenerate inbound dialplan after update");
        }
        synced
    } else {
        updated
    };

    info!(id = %updated.id, name = %updated.name, "SIP endpoint updated successfully");

    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_sip_endpoint")
        .entity_type("sip_endpoint")
        .entity_id(updated.id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        SipEndpointResponse::from(updated),
        "SIP endpoint updated successfully",
    )))
}

/// Delete a SIP endpoint
///
/// DELETE /api/v1/sip-endpoints/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_sip_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());

    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    let deleted = repo.delete(id).await?;
    if !deleted {
        return Err(AppError::Internal(
            "Failed to delete SIP endpoint".to_string(),
        ));
    }

    if endpoint.auth_mode == AuthMode::Ip {
        if let Some(gwid) = endpoint.kamailio_gwid {
            if let Err(e) = delete_kamailio_gateway(gwid).await {
                warn!(gwid = gwid, error = %e, "Failed to delete endpoint from Kamailio");
            }
            if let Err(e) = reload_kamailio_routing().await {
                warn!(error = %e, "Failed to reload Kamailio routing after delete");
            }
        }
        // Sin esto, el dialplan generado conserva el bloque pbx-range-* del
        // endpoint borrado (bridge directo a un host que ya no es una
        // troncal valida).
        if let Err(e) = crate::handlers::inbound_routing::sync_inbound_routes_to_freeswitch(
            pool.get_ref(),
        )
        .await
        {
            warn!(error = %e, "Failed to regenerate inbound dialplan after delete");
        }
    }

    info!(id = %id, admin = %admin.username, "SIP endpoint deleted successfully");

    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_sip_endpoint")
        .entity_type("sip_endpoint")
        .entity_id(id.to_string())
        .details(serde_json::json!({"name": endpoint.name}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::NoContent().finish())
}

/// Get SIP endpoint password (admin only, auth_mode = credentials only)
///
/// GET /api/v1/sip-endpoints/{id}/password
#[instrument(skip(pool, sip_service, admin))]
pub async fn get_sip_endpoint_password(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipEndpointService>>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    if endpoint.auth_mode != AuthMode::Credentials {
        return Err(AppError::Validation(
            "This endpoint does not use credential authentication".to_string(),
        ));
    }

    let password = sip_service.get_device_password(&endpoint)?;

    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("view_sip_endpoint_password")
        .entity_type("sip_endpoint")
        .entity_id(id.to_string())
        .details(serde_json::json!({"name": endpoint.name}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(SipEndpointWithPasswordResponse {
            endpoint: SipEndpointResponse::from(endpoint),
            password,
        })),
    )
}

/// Regenerate SIP endpoint password
///
/// POST /api/v1/sip-endpoints/{id}/regenerate-password
#[instrument(skip(pool, sip_service, admin))]
pub async fn regenerate_sip_endpoint_password(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipEndpointService>>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    if endpoint.auth_mode != AuthMode::Credentials {
        return Err(AppError::Validation(
            "This endpoint does not use credential authentication".to_string(),
        ));
    }

    let username = endpoint
        .sip_username
        .clone()
        .ok_or_else(|| AppError::Internal("Endpoint missing sip_username".to_string()))?;
    let domain = endpoint
        .sip_domain
        .clone()
        .unwrap_or_else(|| "apolo.local".to_string());

    let new_password = SipEndpointService::generate_password();
    let (encrypted, nonce) = sip_service.encrypt_password(&new_password)?;
    let a1_hash = SipEndpointService::generate_a1_hash(&username, &domain, &new_password);

    repo.update_password(id, &encrypted, &nonce, &a1_hash)
        .await?;

    info!(id = %id, sip_username = %username, "SIP endpoint password regenerated");

    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("regenerate_sip_endpoint_password")
        .entity_type("sip_endpoint")
        .entity_id(id.to_string())
        .details(serde_json::json!({"sip_username": username}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        PasswordRegenerateResponse {
            id,
            sip_username: username,
            sip_domain: domain,
            new_password,
        },
        "Password regenerated successfully",
    )))
}

/// Manually re-sync a single trunk (auth_mode = ip) to Kamailio
///
/// POST /api/v1/sip-endpoints/{id}/sync
#[instrument(skip(pool, _admin))]
pub async fn sync_sip_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    if endpoint.auth_mode != AuthMode::Ip {
        return Err(AppError::Validation(
            "Only IP-authenticated trunks sync to Kamailio".to_string(),
        ));
    }

    let synced = sync_trunk_to_kamailio(&repo, endpoint).await;

    if let Err(e) = reload_kamailio_routing().await {
        warn!(error = %e, "Failed to reload Kamailio routing");
    }

    if let Err(e) =
        crate::handlers::inbound_routing::sync_inbound_routes_to_freeswitch(pool.get_ref()).await
    {
        warn!(error = %e, "Failed to regenerate inbound dialplan after sync");
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(SipEndpointResponse::from(synced))))
}

/// Re-sync all IP-authenticated trunks to Kamailio
///
/// POST /api/v1/sip-endpoints/sync-all
#[instrument(skip(pool, _admin))]
pub async fn sync_all_sip_endpoints(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let (trunks, _) = repo.list_filtered(None, Some(true), 10_000, 0).await?;

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for endpoint in trunks.into_iter().filter(|e| e.auth_mode == AuthMode::Ip) {
        let id = endpoint.id;
        let name = endpoint.name.clone();
        let synced = sync_trunk_to_kamailio(&repo, endpoint).await;
        if synced.sync_status == "synced" {
            succeeded += 1;
        } else {
            failed += 1;
        }
        results.push(serde_json::json!({
            "endpoint_id": id,
            "endpoint_name": name,
            "kamailio_gwid": synced.kamailio_gwid,
            "success": synced.sync_status == "synced",
            "error": synced.sync_error,
        }));
    }

    if let Err(e) = reload_kamailio_routing().await {
        warn!(error = %e, "Failed to reload Kamailio routing");
    }

    if let Err(e) =
        crate::handlers::inbound_routing::sync_inbound_routes_to_freeswitch(pool.get_ref()).await
    {
        warn!(error = %e, "Failed to regenerate inbound dialplan after sync-all");
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "total": succeeded + failed,
            "succeeded": succeeded,
            "failed": failed,
            "results": results
        }))),
    )
}

// ============== FreeSWITCH Allowed IPs ==============

/// List allowed IPs for FreeSWITCH
///
/// GET /api/v1/sip-endpoints/allowed-ips
#[instrument(skip(pool, _admin))]
pub async fn list_allowed_ips(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let repo = PgFreeswitchIpRepository::new(pool.get_ref().clone());
    let ips = repo.find_all(1000, 0).await?;

    let response: Vec<FreeswitchIpResponse> = ips.into_iter().map(|ip| ip.into()).collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Create allowed IP
///
/// POST /api/v1/sip-endpoints/allowed-ips
#[instrument(skip(pool, admin, req))]
pub async fn create_allowed_ip(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<FreeswitchIpRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("IP validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let repo = PgFreeswitchIpRepository::new(pool.get_ref().clone());
    let entity = req.to_entity();
    let created = repo.create(&entity).await?;

    info!(id = created.id, ip = %created.ip_address, admin = %admin.username, "FreeSWITCH IP added");

    Ok(HttpResponse::Created().json(ApiResponse::success(FreeswitchIpResponse::from(created))))
}

/// Delete allowed IP
///
/// DELETE /api/v1/sip-endpoints/allowed-ips/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_allowed_ip(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let ip_id = path.into_inner();

    let repo = PgFreeswitchIpRepository::new(pool.get_ref().clone());
    let deleted = repo.delete(ip_id).await?;

    if deleted {
        info!(id = ip_id, admin = %admin.username, "FreeSWITCH IP deleted");
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound(format!("IP {} not found", ip_id)))
    }
}

/// Get SIP endpoint registration status from FreeSWITCH (auth_mode = credentials only)
///
/// GET /api/v1/sip-endpoints/{id}/registration-status
#[instrument(skip(pool, user))]
pub async fn get_sip_endpoint_registration_status(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module(MODULE_KEY) {
        return Err(AppError::Forbidden);
    }

    let id = path.into_inner();
    let repo = PgSipEndpointRepository::new(pool.get_ref().clone());
    let endpoint = repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP endpoint {} not found", id)))?;

    if endpoint.auth_mode != AuthMode::Credentials {
        return Err(AppError::Validation(
            "Registration status only applies to credential-authenticated endpoints".to_string(),
        ));
    }

    let username = endpoint.sip_username.clone().unwrap_or_default();
    let domain = endpoint
        .sip_domain
        .clone()
        .unwrap_or_else(|| "apolo.local".to_string());

    let reg_status = query_freeswitch_registration(&username, &domain).await;

    Ok(HttpResponse::Ok().json(ApiResponse::success(reg_status)))
}

/// Query FreeSWITCH for registration status of a specific user
async fn query_freeswitch_registration(
    username: &str,
    domain: &str,
) -> SipRegistrationStatusResponse {
    use std::process::Command;

    let output = Command::new("fs_cli")
        .args(["-x", "sofia status profile internal reg"])
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            parse_freeswitch_registration(&stdout, username, domain)
        }
        Err(e) => {
            warn!("Failed to query FreeSWITCH: {}", e);
            SipRegistrationStatusResponse {
                registered: false,
                sip_username: username.to_string(),
                sip_domain: domain.to_string(),
                user_agent: None,
                contact: None,
                status: Some("Error querying FreeSWITCH".to_string()),
                ip: None,
                port: None,
                ping_status: None,
                expires_seconds: None,
                expires_at: None,
            }
        }
    }
}

/// Parse FreeSWITCH registration output
fn parse_freeswitch_registration(
    output: &str,
    username: &str,
    domain: &str,
) -> SipRegistrationStatusResponse {
    if output.contains("Total items returned: 0") || output.trim().is_empty() {
        return SipRegistrationStatusResponse {
            registered: false,
            sip_username: username.to_string(),
            sip_domain: domain.to_string(),
            user_agent: None,
            contact: None,
            status: Some("Not registered".to_string()),
            ip: None,
            port: None,
            ping_status: None,
            expires_seconds: None,
            expires_at: None,
        };
    }

    let user_pattern = format!("{}@{}", username, domain);

    let mut found_user = false;
    let mut user_agent = None;
    let mut contact = None;
    let mut status = None;
    let mut ip = None;
    let mut port = None;
    let mut ping_status = None;
    let mut expires_seconds = None;
    let mut expires_at = None;

    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("User:") && line.contains(&user_pattern) {
            found_user = true;
            continue;
        }

        if !found_user {
            continue;
        }

        if line.starts_with("Call-ID:") {
            break;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "Agent" => user_agent = Some(value.to_string()),
                "Contact" => contact = Some(value.to_string()),
                "Status" => {
                    status = Some(value.to_string());
                    if let Some(exp_start) = value.find("EXPSECS(") {
                        if let Some(exp_end) = value[exp_start..].find(')') {
                            let exp_str = &value[exp_start + 8..exp_start + exp_end];
                            expires_seconds = exp_str.parse().ok();
                        }
                    }
                    if let Some(exp_start) = value.find("EXP(") {
                        if let Some(exp_end) = value[exp_start + 4..].find(')') {
                            expires_at =
                                Some(value[exp_start + 4..exp_start + 4 + exp_end].to_string());
                        }
                    }
                }
                "IP" => ip = Some(value.to_string()),
                "Port" => port = value.parse().ok(),
                "Ping-Status" => ping_status = Some(value.to_string()),
                _ => {}
            }
        }
    }

    if !found_user {
        return SipRegistrationStatusResponse {
            registered: false,
            sip_username: username.to_string(),
            sip_domain: domain.to_string(),
            user_agent: None,
            contact: None,
            status: Some("Not registered".to_string()),
            ip: None,
            port: None,
            ping_status: None,
            expires_seconds: None,
            expires_at: None,
        };
    }

    SipRegistrationStatusResponse {
        registered: status
            .as_ref()
            .map(|s| s.contains("Registered"))
            .unwrap_or(false),
        sip_username: username.to_string(),
        sip_domain: domain.to_string(),
        user_agent,
        contact,
        status,
        ip,
        port,
        ping_status,
        expires_seconds,
        expires_at,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Reject endpoint_type/auth_mode combinations that aren't supported:
/// IP-trust only makes sense for a whole trunk/PBX, not a single extension.
fn validate_type_auth_combo(
    endpoint_type: EndpointType,
    auth_mode: AuthMode,
) -> Result<(), AppError> {
    if auth_mode == AuthMode::Ip && endpoint_type != EndpointType::Trunk {
        return Err(AppError::Validation(
            "auth_mode=ip is only valid for endpoint_type=trunk".to_string(),
        ));
    }
    Ok(())
}

/// Registration limit by endpoint type: trunk (PBX) 1-150, extension 1-20.
/// Solo tiene efecto real en endpoints auth_mode=credentials (los que hacen
/// REGISTER); las troncales IP no registran y el valor queda sin uso.
/// El DTO valida el outer bound 1-150; esta funcion aplica el limite por tipo.
/// En update se evalua contra el endpoint_type almacenado (no editable por API).
fn validate_channel_limit(
    endpoint_type: EndpointType,
    max_registrations: i32,
) -> Result<(), AppError> {
    let max = match endpoint_type {
        EndpointType::Trunk => 150,
        EndpointType::Extension => 20,
    };
    if !(1..=max).contains(&max_registrations) {
        return Err(AppError::Validation(format!(
            "max_registrations must be 1-{} for endpoint_type={}",
            max, endpoint_type
        )));
    }
    Ok(())
}

/// Concurrent-call limit by endpoint type: trunk 1-150, extension 1-20.
/// Enforcement real:
/// - auth_mode=credentials (extensiones y troncales por usuario/contraseña):
///   billing engine, contando active_calls (bypassean Kamailio routing).
/// - auth_mode=ip (troncales IP): Kamailio, via sync_trunk_call_settings.
fn validate_call_limit(
    endpoint_type: EndpointType,
    max_concurrent_calls: i32,
) -> Result<(), AppError> {
    let max = match endpoint_type {
        EndpointType::Trunk => 150,
        EndpointType::Extension => 20,
    };
    if !(1..=max).contains(&max_concurrent_calls) {
        return Err(AppError::Validation(format!(
            "max_concurrent_calls must be 1-{} for endpoint_type={}",
            max, endpoint_type
        )));
    }
    Ok(())
}

/// Normalize + validate DID ranges ("174020,174021"): digits and commas only.
/// Returns canonical "a,b,c" (trimmed, no empties, no self-overlap) or "".
fn normalize_did_ranges(raw: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    for p in &parts {
        if !p.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::Validation(format!(
                "did_ranges must be digit prefixes separated by commas (bad: '{}')",
                p
            )));
        }
    }
    // Self-overlap within the same value (e.g. "1740,174020" is ambiguous)
    for (i, a) in parts.iter().enumerate() {
        for b in parts.iter().skip(i + 1) {
            if a == b || a.starts_with(b) || b.starts_with(a) {
                return Err(AppError::Validation(format!(
                    "did_ranges '{}' and '{}' overlap each other",
                    a, b
                )));
            }
        }
    }
    Ok(parts.join(","))
}

/// Pure overlap check: which (owner, owner_prefix) pairs collide with `new`.
/// Collision = equal, prefix-of, or extension-of (ambiguous ownership).
fn find_range_conflicts<'a>(
    new: &[String],
    existing: &'a [(String, Vec<String>)],
) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for p in new {
        for (owner, prefixes) in existing {
            for q in prefixes {
                if p == q || p.starts_with(q.as_str()) || q.starts_with(p.as_str()) {
                    out.push((p.clone(), owner.clone(), q.clone()));
                }
            }
        }
    }
    out
}

/// Reject did_ranges that overlap ranges owned by OTHER endpoints,
/// naming the current owner in the message. `self_id=None` on create.
async fn check_did_range_conflicts(
    pool: &PgPool,
    self_id: Option<Uuid>,
    canonical: &str,
) -> Result<(), AppError> {
    let new: Vec<String> = if canonical.is_empty() {
        return Ok(());
    } else {
        canonical.split(',').map(|s| s.to_string()).collect()
    };
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"SELECT id, name, did_ranges FROM sip_endpoints WHERE COALESCE(did_ranges,'') <> ''
           -- del mismo grupo = mismo dueno: el solape interno es legitimo (failover)
           AND ($1::uuid IS NULL OR id NOT IN
                (SELECT sip_endpoint_id FROM pbx_group_members WHERE group_id IN
                 (SELECT group_id FROM pbx_group_members WHERE sip_endpoint_id = $1)))"#,
    )
    .bind(self_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to check DID ranges: {}", e)))?;
    let existing: Vec<(String, Vec<String>)> = rows
        .into_iter()
        .filter(|(id, _, _)| Some(*id) != self_id)
        .map(|(_, name, ranges)| {
            let prefixes = ranges
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect();
            (name, prefixes)
        })
        .collect();
    let conflicts = find_range_conflicts(&new, &existing);
    if conflicts.is_empty() {
        return Ok(());
    }
    let detail: Vec<String> = conflicts
        .iter()
        .map(|(p, owner, q)| format!("'{}' se solapa con '{}' de '{}'", p, q, owner))
        .collect();
    Err(AppError::Validation(format!(
        "Rango DID ya asignado: {}. Cada prefijo debe pertenecer a una sola PBX.",
        detail.join("; ")
    )))
}

/// Sync an IP-authenticated trunk to Kamailio's dr_gateways (type=9) + ACL.
/// Returns the endpoint with sync_status/kamailio_gwid refreshed from the DB;
/// on failure the sync_status/sync_error are persisted and returned as-is.
async fn sync_trunk_to_kamailio(
    repo: &PgSipEndpointRepository,
    endpoint: apolo_core::models::SipEndpoint,
) -> apolo_core::models::SipEndpoint {
    let id = endpoint.id;

    match try_sync_trunk_to_kamailio(&endpoint).await {
        Ok(gwid) => {
            let _ = repo.update_sync_status(id, gwid, "synced", None).await;
        }
        Err(e) => {
            let error_msg = e.to_string();
            warn!(error = %error_msg, "Failed to sync SIP endpoint to Kamailio");
            let _ = repo
                .update_sync_status(id, None, "error", Some(&error_msg))
                .await;
        }
    }

    repo.find_by_id(id).await.ok().flatten().unwrap_or(endpoint)
}

async fn try_sync_trunk_to_kamailio(
    endpoint: &apolo_core::models::SipEndpoint,
) -> Result<Option<i32>, AppError> {
    let host = endpoint
        .host
        .as_deref()
        .ok_or_else(|| AppError::Validation("Trunk has no host configured".to_string()))?;

    let address = gateway_address(host, Some(endpoint.port));
    let signalling = signalling_from_transport(Some(&endpoint.transport));

    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();
    let Some(kamailio_url) = kamailio_url else {
        warn!("KAMAILIO_DATABASE_URL not set, skipping Kamailio sync");
        return Ok(None);
    };

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url)
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB connection failed: {}", e)))?;

    let gwid: i32;

    if let Some(existing_gwid) = endpoint.kamailio_gwid {
        sqlx::query(
            r#"
            UPDATE dr_gateways SET address = ?, attrs = ?, description = ? WHERE gwid = ?
            "#,
        )
        .bind(&address)
        .bind(gateway_attrs(existing_gwid, GW_TYPE_PBX, signalling))
        .bind(&endpoint.name)
        .bind(existing_gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update Kamailio gateway: {}", e)))?;

        gwid = existing_gwid;

        sqlx::query("DELETE FROM address WHERE grp = 9 AND tag LIKE ?")
            .bind(format!("%gwgroup:{}%", existing_gwid))
            .execute(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to clear PBX ACL entry: {}", e)))?;
    } else {
        let result = sqlx::query(
            r#"
            INSERT INTO dr_gateways (type, address, strip, attrs, description)
            VALUES (?, ?, 0, '', ?)
            "#,
        )
        .bind(GW_TYPE_PBX)
        .bind(&address)
        .bind(&endpoint.name)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to insert Kamailio gateway: {}", e)))?;

        gwid = result.last_insert_id() as i32;

        sqlx::query("UPDATE dr_gateways SET attrs = ? WHERE gwid = ?")
            .bind(gateway_attrs(gwid, GW_TYPE_PBX, signalling))
            .bind(gwid)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| {
                AppError::Database(format!("Failed to set Kamailio gateway attrs: {}", e))
            })?;
    }

    // Grant the PBX's IP permission to route outbound (grp 9 = FLT_PBX in kamailio.cfg).
    sqlx::query("INSERT INTO address (grp, ip_addr, mask, port, tag) VALUES (9, ?, 32, 0, ?)")
        .bind(host)
        .bind(format!("name:{},gwgroup:{}", endpoint.name, gwid))
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to insert PBX ACL entry: {}", e)))?;

    // Enforcement real de canales: publica max_concurrent_calls (trunk: 1-150,
    // ver validate_call_limit) como limite de llamadas concurrentes de
    // origen para esta troncal. Sin estas filas, route[CHECK_CALL_LIMIT]
    // no tiene limite contra el que comparar y el valor solo se guarda.
    // (max_registrations queda reservado para troncales auth_mode=credentials,
    // que si hacen REGISTER; las IP no registran y lo ignoran.)
    sync_trunk_call_settings(&kamailio_pool, gwid, endpoint.max_concurrent_calls).await?;

    Ok(Some(gwid))
}

/// gwgroup dedicado de una troncal IP: 1:1 con su gwid (cf. INSTALL.md 13.1,
/// `gwid 100 -> gwgroup 100`). Cada PBX tiene asi su propio contador en el
/// htable `concurrent_calls` y su propio limite en `call_settings`.
fn trunk_gwgroupid(gwid: i32) -> i32 {
    gwid
}

/// Sync per-trunk concurrent-channel limit to Kamailio's call-limit engine.
///
/// Two rows make `route[CHECK_CALL_LIMIT]` enforce the trunk's
/// `max_concurrent_calls` (see `validate_call_limit`):
/// - `dsip_gw2gwgroup(gwid, gwgroupid)`: lets `SET_CALLSRC_INFO` /
///   `SET_CALLDST_INFO` resolve the trunk's gwgroup (`kamailio.cfg:3004,3035`).
/// - `dsip_call_settings(gwgroupid, limit, timeout)`: the actual limit, read
///   at runtime via the `call_settings` htable off the `dsip_call_settings_h`
///   view (`kamailio.cfg:922,3009-3012`). We write the base table, not the
///   view: `dsip_call_settings_h` casts every column to CHAR, which makes it
///   a non-updatable MySQL view (error 1471 on INSERT/DELETE).
/// A fresh `htable.reload` (see `reload_kamailio_routing`) is what makes the
/// new limit live; without it Kamailio keeps enforcing the previous value.
///
/// `timeout` is preserved when the row already exists (operator-tuned dialog
/// timeouts are not clobbered) and inserted as NULL (= no override) otherwise.
/// DELETE-first (in a transaction) keeps exactly one row per trunk whether or
/// not the tables carry unique keys.
async fn sync_trunk_call_settings(
    kamailio_pool: &sqlx::MySqlPool,
    gwid: i32,
    max_channels: i32,
) -> Result<(), AppError> {
    let gwgroupid = trunk_gwgroupid(gwid);

    let mut tx = kamailio_pool
        .begin()
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB transaction failed: {}", e)))?;

    sqlx::query("DELETE FROM dsip_gw2gwgroup WHERE gwid = ?")
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sync gw2gwgroup mapping: {}", e)))?;

    sqlx::query("INSERT INTO dsip_gw2gwgroup (gwid, gwgroupid) VALUES (?, ?)")
        .bind(gwid)
        .bind(gwgroupid)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sync gw2gwgroup mapping: {}", e)))?;

    let existing_timeout: Option<Option<i64>> =
        sqlx::query_scalar("SELECT timeout FROM dsip_call_settings WHERE gwgroupid = ?")
            .bind(gwgroupid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| AppError::Database(format!("Failed to read call_settings: {}", e)))?;
    let timeout: Option<i64> = existing_timeout.flatten();

    sqlx::query("DELETE FROM dsip_call_settings WHERE gwgroupid = ?")
        .bind(gwgroupid)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sync call_settings: {}", e)))?;

    sqlx::query("INSERT INTO dsip_call_settings (gwgroupid, `limit`, timeout) VALUES (?, ?, ?)")
        .bind(gwgroupid)
        .bind(max_channels)
        .bind(timeout)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sync call_settings: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB transaction failed: {}", e)))?;

    Ok(())
}

/// Delete a gateway from Kamailio
async fn delete_kamailio_gateway(gwid: i32) -> Result<(), AppError> {
    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();

    let Some(kamailio_url) = kamailio_url else {
        return Ok(());
    };

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url)
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB connection failed: {}", e)))?;

    sqlx::query("DELETE FROM dr_gateways WHERE gwid = ?")
        .bind(gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete Kamailio gateway: {}", e)))?;

    sqlx::query("DELETE FROM address WHERE grp = 9 AND tag LIKE ?")
        .bind(format!("%gwgroup:{}%", gwid))
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete PBX ACL entry: {}", e)))?;

    // Limpieza del enforcement de canales (gwgroup dedicado 1:1 con el gwid,
    // ver trunk_gwgroupid): sin esto el limite huerfano quedaria en los
    // htables tras el proximo reload.
    sqlx::query("DELETE FROM dsip_gw2gwgroup WHERE gwid = ?")
        .bind(gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete gw2gwgroup mapping: {}", e)))?;

    sqlx::query("DELETE FROM dsip_call_settings WHERE gwgroupid = ?")
        .bind(trunk_gwgroupid(gwid))
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete call_settings: {}", e)))?;

    Ok(())
}

/// Reload Kamailio routing tables + channel-limit htables.
///
/// `drouting.reload` / `permissions.addressReload` aplican gateways y ACL;
/// `htable.reload call_settings` / `htable.reload gw2gwgroup` aplican los
/// limites de canales (sin esto, `CHECK_CALL_LIMIT` sigue usando el valor
/// anterior aunque la DB ya tenga el nuevo).
async fn reload_kamailio_routing() -> Result<(), AppError> {
    use tokio::process::Command;

    let output = Command::new("kamcmd")
        .args(["drouting.reload"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute kamcmd: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(error = %stderr, "Failed to reload Kamailio drouting");
    }

    let acl_output = Command::new("kamcmd")
        .args(["permissions.addressReload"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute kamcmd: {}", e)))?;

    if !acl_output.status.success() {
        let stderr = String::from_utf8_lossy(&acl_output.stderr);
        tracing::error!(error = %stderr, "Failed to reload Kamailio permissions ACL");
    }

    // Channel-limit htables: sin estos reloads el nuevo `limit` / mapping
    // queda solo en DB y CHECK_CALL_LIMIT sigue enforceando el valor viejo.
    for table in ["call_settings", "gw2gwgroup"] {
        let ht_output = Command::new("kamcmd")
            .args(["htable.reload", table])
            .output()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to execute kamcmd: {}", e)))?;

        if !ht_output.status.success() {
            let stderr = String::from_utf8_lossy(&ht_output.stderr);
            tracing::error!(error = %stderr, table, "Failed to reload Kamailio htable");
        }
    }

    Ok(())
}

/// Configure SIP endpoint routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/sip-endpoints")
            .route("", web::get().to(list_sip_endpoints))
            .route("", web::post().to(create_sip_endpoint))
            .route("/sync-all", web::post().to(sync_all_sip_endpoints))
            .route("/allowed-ips", web::get().to(list_allowed_ips))
            .route("/allowed-ips", web::post().to(create_allowed_ip))
            .route("/allowed-ips/{id}", web::delete().to(delete_allowed_ip))
            .route("/{id}", web::get().to(get_sip_endpoint))
            .route("/{id}", web::put().to(update_sip_endpoint))
            .route("/{id}", web::delete().to(delete_sip_endpoint))
            .route("/{id}/password", web::get().to(get_sip_endpoint_password))
            .route(
                "/{id}/regenerate-password",
                web::post().to(regenerate_sip_endpoint_password),
            )
            .route(
                "/{id}/registration-status",
                web::get().to(get_sip_endpoint_registration_status),
            )
            .route("/{id}/sync", web::post().to(sync_sip_endpoint)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn existing() -> Vec<(String, Vec<String>)> {
        vec![("PBX Fibertel".to_string(), vec!["174020".to_string()])]
    }

    #[test]
    fn exact_duplicate_conflicts() {
        let c = find_range_conflicts(&["174020".to_string()], &existing());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].1, "PBX Fibertel");
    }

    #[test]
    fn narrower_prefix_over_owned_range_conflicts() {
        // "1740205" vive dentro de "174020" -> ambiguo aunque longest-match lo resolveria
        let c = find_range_conflicts(&["1740205".to_string()], &existing());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn wider_prefix_covering_owned_range_conflicts() {
        let c = find_range_conflicts(&["1740".to_string()], &existing());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn disjoint_prefix_has_no_conflict() {
        let c = find_range_conflicts(&["163751".to_string()], &existing());
        assert!(c.is_empty());
    }

    #[test]
    fn self_overlap_is_rejected() {
        assert!(normalize_did_ranges("1740,174020").is_err());
        assert!(normalize_did_ranges("174020,174020").is_err());
        assert_eq!(
            normalize_did_ranges(" 174020 , 163751 ").unwrap(),
            "174020,163751"
        );
    }

    #[test]
    fn trunk_allows_up_to_150_channels() {
        assert!(validate_channel_limit(EndpointType::Trunk, 1).is_ok());
        assert!(validate_channel_limit(EndpointType::Trunk, 10).is_ok());
        assert!(validate_channel_limit(EndpointType::Trunk, 150).is_ok());
        assert!(validate_channel_limit(EndpointType::Trunk, 0).is_err());
        assert!(validate_channel_limit(EndpointType::Trunk, 151).is_err());
    }

    #[test]
    fn extension_keeps_legacy_limit_of_20() {
        assert!(validate_channel_limit(EndpointType::Extension, 1).is_ok());
        assert!(validate_channel_limit(EndpointType::Extension, 20).is_ok());
        assert!(validate_channel_limit(EndpointType::Extension, 21).is_err());
        assert!(validate_channel_limit(EndpointType::Extension, 150).is_err());
    }

    #[test]
    fn trunk_gwgroup_is_one_to_one_with_gwid() {
        // Convencion INSTALL.md 13.1: cada troncal tiene gwgroup dedicado.
        assert_eq!(trunk_gwgroupid(103), 103);
        assert_eq!(trunk_gwgroupid(104), 104);
    }

    #[test]
    fn concurrent_call_limit_is_20_for_extensions_150_for_trunks() {
        assert!(validate_call_limit(EndpointType::Extension, 1).is_ok());
        assert!(validate_call_limit(EndpointType::Extension, 2).is_ok());
        assert!(validate_call_limit(EndpointType::Extension, 20).is_ok());
        assert!(validate_call_limit(EndpointType::Extension, 21).is_err());
        assert!(validate_call_limit(EndpointType::Trunk, 150).is_ok());
        assert!(validate_call_limit(EndpointType::Trunk, 151).is_err());
    }
}
