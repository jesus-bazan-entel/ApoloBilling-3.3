//! SIP Device handlers
//!
//! HTTP handlers for SIP device management endpoints.

use crate::dto::sip_device::{
    FreeswitchIpRequest, FreeswitchIpResponse, PasswordRegenerateResponse, SipDeviceCreateRequest,
    SipDeviceFilterParams, SipDeviceResponse, SipDeviceUpdateRequest, SipDeviceWithPasswordResponse,
    SipRegistrationStatusResponse,
};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::models::AuditLogBuilder;
use apolo_core::traits::{FreeswitchIpRepository, Repository, SipDeviceRepository};
use apolo_core::AppError;
use apolo_db::{PgFreeswitchIpRepository, PgSipDeviceRepository};
use apolo_services::SipDeviceService;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List SIP devices with pagination and filters
///
/// GET /api/v1/sip-devices
#[instrument(skip(pool, _user))]
pub async fn list_sip_devices(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    filters: web::Query<SipDeviceFilterParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        page = query.page,
        per_page = query.per_page,
        "Listing SIP devices"
    );

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());

    let (devices, total) = repo
        .list_filtered(
            filters.account_id,
            filters.enabled,
            query.limit(),
            query.offset(),
        )
        .await?;

    let response_data: Vec<SipDeviceResponse> = devices.into_iter().map(|d| d.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Create a new SIP device
///
/// POST /api/v1/sip-devices
#[instrument(skip(pool, sip_service, _user, req))]
pub async fn create_sip_device(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipDeviceService>>,
    _user: AuthenticatedUser,
    req: web::Json<SipDeviceCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("SIP device creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        sip_username = %req.sip_username,
        sip_domain = %req.sip_domain,
        "Creating SIP device"
    );

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());

    // Check if username already exists in domain
    if repo
        .username_exists(&req.sip_username, &req.sip_domain)
        .await?
    {
        warn!(
            sip_username = %req.sip_username,
            sip_domain = %req.sip_domain,
            "SIP device creation failed: duplicate username"
        );
        return Err(AppError::AlreadyExists(format!(
            "SIP device {}@{} already exists",
            req.sip_username, req.sip_domain
        )));
    }

    // Generate password if not provided
    let password = req
        .password
        .clone()
        .unwrap_or_else(SipDeviceService::generate_password);

    // Create device and prepare with encrypted password
    let mut device = req.to_device();
    sip_service.prepare_device_with_password(&mut device, &password)?;

    // Save to database
    let created = repo.create(&device).await?;

    info!(
        id = created.id,
        sip_username = %created.sip_username,
        "SIP device created successfully"
    );

    // Audit log
    let audit_details = serde_json::json!({
        "sip_username": created.sip_username,
        "sip_domain": created.sip_domain,
        "account_id": created.account_id,
        "context": created.context,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("create_sip_device")
        .entity_type("sip_device")
        .entity_id(created.id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Return response with password (only on creation)
    let response = SipDeviceWithPasswordResponse {
        device: SipDeviceResponse::from(created),
        password,
    };

    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "SIP device created successfully",
    )))
}

/// Get a single SIP device by ID
///
/// GET /api/v1/sip-devices/{id}
#[instrument(skip(pool, _user))]
pub async fn get_sip_device(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();
    debug!(id = device_id, "Getting SIP device");

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());
    let device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    let response = SipDeviceResponse::from(device);
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Update a SIP device
///
/// PUT /api/v1/sip-devices/{id}
#[instrument(skip(pool, _user, req))]
pub async fn update_sip_device(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
    req: web::Json<SipDeviceUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("SIP device update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let device_id = path.into_inner();
    debug!(id = device_id, "Updating SIP device");

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());

    // Get existing device
    let mut device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    // Build audit changes
    let mut changes = serde_json::json!({});

    // Apply updates
    if let Some(account_id) = req.account_id {
        changes["account_id"] = serde_json::json!({
            "old": device.account_id,
            "new": account_id
        });
        device.account_id = account_id;
    }

    if let Some(ref username) = req.sip_username {
        changes["sip_username"] = serde_json::json!({
            "old": device.sip_username,
            "new": username
        });
        device.sip_username = username.clone();
    }

    if let Some(ref domain) = req.sip_domain {
        changes["sip_domain"] = serde_json::json!({
            "old": device.sip_domain,
            "new": domain
        });
        device.sip_domain = domain.clone();
    }

    if let Some(ref display_name) = req.display_name {
        device.display_name = Some(display_name.clone());
    }

    if let Some(ref description) = req.description {
        device.description = Some(description.clone());
    }

    if let Some(ref context) = req.context {
        device.context = context.clone();
    }

    if let Some(ref accountcode) = req.accountcode {
        device.accountcode = Some(accountcode.clone());
    }

    if let Some(ref codecs) = req.codecs {
        device.codecs = codecs.clone();
    }

    if let Some(max_registrations) = req.max_registrations {
        device.max_registrations = max_registrations;
    }

    if let Some(enabled) = req.enabled {
        changes["enabled"] = serde_json::json!({
            "old": device.enabled,
            "new": enabled
        });
        device.enabled = enabled;
    }

    // Save updates
    let updated = repo.update(&device).await?;

    info!(
        id = updated.id,
        sip_username = %updated.sip_username,
        "SIP device updated successfully"
    );

    // Audit log
    let audit_details = serde_json::json!({
        "sip_username": updated.sip_username,
        "changes": changes
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("update_sip_device")
        .entity_type("sip_device")
        .entity_id(updated.id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = SipDeviceResponse::from(updated);
    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "SIP device updated successfully",
    )))
}

/// Delete a SIP device
///
/// DELETE /api/v1/sip-devices/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_sip_device(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();
    debug!(
        id = device_id,
        admin = %admin.username,
        "Deleting SIP device"
    );

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());

    // Verify device exists
    let device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    // Delete device
    let deleted = repo.delete(device_id).await?;

    if deleted {
        info!(
            id = device_id,
            admin = %admin.username,
            "SIP device deleted successfully"
        );

        // Audit log
        let audit_details = serde_json::json!({
            "sip_username": device.sip_username,
            "sip_domain": device.sip_domain,
            "account_id": device.account_id,
        });

        if let Ok(audit_data) = AuditLogBuilder::default()
            .username(admin.username.clone())
            .action("delete_sip_device")
            .entity_type("sip_device")
            .entity_id(device_id.to_string())
            .details(audit_details)
            .build()
        {
            audit_data.insert(pool.get_ref()).await;
        }

        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::Internal("Failed to delete SIP device".to_string()))
    }
}

/// Get SIP device password (admin only)
///
/// GET /api/v1/sip-devices/{id}/password
#[instrument(skip(pool, sip_service, admin))]
pub async fn get_sip_device_password(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipDeviceService>>,
    path: web::Path<i32>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();
    debug!(
        id = device_id,
        admin = %admin.username,
        "Getting SIP device password"
    );

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());
    let device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    // Decrypt password
    let password = sip_service.get_device_password(&device)?;

    // Audit log - password access is security-sensitive
    let audit_details = serde_json::json!({
        "sip_username": device.sip_username,
        "sip_domain": device.sip_domain,
        "action": "password_viewed"
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("view_sip_password")
        .entity_type("sip_device")
        .entity_id(device_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = SipDeviceWithPasswordResponse {
        device: SipDeviceResponse::from(device),
        password,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Regenerate SIP device password
///
/// POST /api/v1/sip-devices/{id}/regenerate-password
#[instrument(skip(pool, sip_service, _user))]
pub async fn regenerate_sip_device_password(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipDeviceService>>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();
    debug!(id = device_id, "Regenerating SIP device password");

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());
    let device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    // Generate new password
    let new_password = SipDeviceService::generate_password();

    // Encrypt and generate new A1 hash
    let (encrypted, nonce) = sip_service.encrypt_password(&new_password)?;
    let a1_hash =
        SipDeviceService::generate_a1_hash(&device.sip_username, &device.sip_domain, &new_password);

    // Update in database
    repo.update_password(device_id, &encrypted, &nonce, &a1_hash)
        .await?;

    info!(
        id = device_id,
        sip_username = %device.sip_username,
        "SIP device password regenerated"
    );

    // Audit log
    let audit_details = serde_json::json!({
        "sip_username": device.sip_username,
        "sip_domain": device.sip_domain,
        "action": "password_regenerated"
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("regenerate_sip_password")
        .entity_type("sip_device")
        .entity_id(device_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = PasswordRegenerateResponse {
        id: device_id,
        sip_username: device.sip_username,
        sip_domain: device.sip_domain,
        new_password,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Password regenerated successfully",
    )))
}

// ============== FreeSWITCH Allowed IPs ==============

/// List allowed IPs for FreeSWITCH
///
/// GET /api/v1/sip-devices/allowed-ips
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
/// POST /api/v1/sip-devices/allowed-ips
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

    info!(
        id = created.id,
        ip = %created.ip_address,
        admin = %admin.username,
        "FreeSWITCH IP added"
    );

    let response = FreeswitchIpResponse::from(created);
    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

/// Delete allowed IP
///
/// DELETE /api/v1/sip-devices/allowed-ips/{id}
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
        info!(
            id = ip_id,
            admin = %admin.username,
            "FreeSWITCH IP deleted"
        );
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound(format!("IP {} not found", ip_id)))
    }
}

/// Get SIP device registration status from FreeSWITCH
///
/// GET /api/v1/sip-devices/{id}/registration-status
#[instrument(skip(pool, _user))]
pub async fn get_sip_device_registration_status(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let device_id = path.into_inner();
    debug!(id = device_id, "Getting SIP device registration status");

    let repo = PgSipDeviceRepository::new(pool.get_ref().clone());
    let device = repo
        .find_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("SIP device {} not found", device_id)))?;

    // Query FreeSWITCH for registration status using fs_cli
    let reg_status = query_freeswitch_registration(&device.sip_username, &device.sip_domain).await;

    Ok(HttpResponse::Ok().json(ApiResponse::success(reg_status)))
}

/// Query FreeSWITCH for registration status of a specific user
async fn query_freeswitch_registration(username: &str, domain: &str) -> SipRegistrationStatusResponse {
    use std::process::Command;

    // Execute fs_cli command to get ALL registrations (passing AOR as argument doesn't work)
    let output = Command::new("fs_cli")
        .args(["-x", "sofia status profile internal reg"])
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            // Find the specific user in the output
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
fn parse_freeswitch_registration(output: &str, username: &str, domain: &str) -> SipRegistrationStatusResponse {
    // Check if no registrations exist
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

    // Build the User line pattern to find the correct registration block
    // Format: "User:       	username@domain"
    let user_pattern = format!("{}@{}", username, domain);

    // Find the start of the user's registration block
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

        // Check if this line contains our user
        if line.starts_with("User:") && line.contains(&user_pattern) {
            found_user = true;
            continue;
        }

        // If we haven't found our user yet, skip
        if !found_user {
            continue;
        }

        // If we hit a new Call-ID, we've passed our user's block
        if line.starts_with("Call-ID:") {
            break;
        }

        // Parse registration details for our user
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "Agent" => user_agent = Some(value.to_string()),
                "Contact" => contact = Some(value.to_string()),
                "Status" => {
                    status = Some(value.to_string());
                    // Extract EXPSECS from status line like "Registered(UDP)(unknown) EXP(2026-03-21 12:15:29) EXPSECS(360)"
                    if let Some(exp_start) = value.find("EXPSECS(") {
                        if let Some(exp_end) = value[exp_start..].find(')') {
                            let exp_str = &value[exp_start + 8..exp_start + exp_end];
                            expires_seconds = exp_str.parse().ok();
                        }
                    }
                    // Extract EXP datetime
                    if let Some(exp_start) = value.find("EXP(") {
                        if let Some(exp_end) = value[exp_start + 4..].find(')') {
                            expires_at = Some(value[exp_start + 4..exp_start + 4 + exp_end].to_string());
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

    // If we didn't find the user, return not registered
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
        registered: status.as_ref().map(|s| s.contains("Registered")).unwrap_or(false),
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

/// Configure SIP device routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/sip-devices")
            .route("", web::get().to(list_sip_devices))
            .route("", web::post().to(create_sip_device))
            .route("/allowed-ips", web::get().to(list_allowed_ips))
            .route("/allowed-ips", web::post().to(create_allowed_ip))
            .route("/allowed-ips/{id}", web::delete().to(delete_allowed_ip))
            .route("/{id}", web::get().to(get_sip_device))
            .route("/{id}", web::put().to(update_sip_device))
            .route("/{id}", web::delete().to(delete_sip_device))
            .route("/{id}/password", web::get().to(get_sip_device_password))
            .route(
                "/{id}/regenerate-password",
                web::post().to(regenerate_sip_device_password),
            )
            .route(
                "/{id}/registration-status",
                web::get().to(get_sip_device_registration_status),
            ),
    );
}
