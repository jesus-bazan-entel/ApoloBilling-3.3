//! Provisioning handlers
//!
//! Creates a billing account and its associated SIP device in a single
//! operation. Mirrors the logic of `account::create_account` (plan-derived
//! settings) and `sip_device::create_sip_device` (encrypted password + A1 hash),
//! but performs both together so a customer is provisioned end-to-end from one
//! request. Supports individual and bulk (per-row reporting) modes.

use crate::dto::provisioning::{
    BulkProvisionRequest, BulkProvisionResponse, BulkRowResult, BulkRowStatus, ProvisionRequest,
    ProvisionResult,
};
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::AdminUser;
use apolo_core::models::{Account, AccountStatus, AuditLogBuilder, SipDevice};
use apolo_core::traits::{AccountRepository, Repository, SipDeviceRepository};
use apolo_core::AppError;
use apolo_db::{PgAccountRepository, PgPlanRepository, PgSipDeviceRepository};
use apolo_services::SipDeviceService;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, instrument, warn};
use validator::Validate;

const DEFAULT_SIP_DOMAIN: &str = "app.fonowin.pe";
const DEFAULT_CODECS: &str = "PCMU,PCMA,G729,opus";
const DEFAULT_CONTEXT: &str = "from-pbx";

/// Outcome of provisioning one row, distinguishing duplicates (skippable)
/// from genuine errors.
enum ProvisionOutcome {
    Created(ProvisionResult),
    Skipped(String),
}

/// Provision a single account + SIP device.
///
/// Returns `Skipped` if the account number or SIP username already exists,
/// `Err` for validation / database failures. On SIP-device failure the freshly
/// created account is rolled back (compensating delete) to avoid orphans.
async fn provision_one(
    pool: &PgPool,
    sip_service: &SipDeviceService,
    req: &ProvisionRequest,
) -> Result<ProvisionOutcome, AppError> {
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let account_repo = PgAccountRepository::new(pool.clone());
    let sip_repo = PgSipDeviceRepository::new(pool.clone());
    let plan_repo = PgPlanRepository::new(pool.clone());

    let domain = req
        .domain
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SIP_DOMAIN.to_string());

    // Duplicate detection -> skip (idempotent bulk loads).
    if account_repo.find_by_number(&req.username).await?.is_some() {
        return Ok(ProvisionOutcome::Skipped(format!(
            "La cuenta {} ya existe",
            req.username
        )));
    }
    if sip_repo.username_exists(&req.username, &domain).await? {
        return Ok(ProvisionOutcome::Skipped(format!(
            "El dispositivo SIP {}@{} ya existe",
            req.username, domain
        )));
    }

    // Derive account settings from the plan.
    let plan = plan_repo
        .find_by_id(req.plan_id)
        .await?
        .ok_or_else(|| AppError::Validation(format!("Plan {} no encontrado", req.plan_id)))?;

    let customer_name = req
        .customer_name
        .clone()
        .filter(|n| !n.trim().is_empty());

    let account = Account {
        id: 0,
        account_number: req.username.clone(),
        account_name: customer_name,
        customer_phone: None,
        account_type: plan.account_type,
        balance: plan.initial_balance,
        credit_limit: plan.credit_limit,
        currency: "USD".to_string(),
        status: AccountStatus::Active,
        max_concurrent_calls: plan.max_concurrent_calls,
        plan_id: Some(plan.id),
        cod_ped: req.cod_ped.clone().filter(|s| !s.trim().is_empty()),
        departamento: req.departamento.clone().filter(|s| !s.trim().is_empty()),
        provincia: req.provincia.clone().filter(|s| !s.trim().is_empty()),
        distrito: req.distrito.clone().filter(|s| !s.trim().is_empty()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_account = account_repo.create(&account).await?;

    // Build + persist the SIP device. The order code (COD_PED) becomes the
    // device display_name (matching the existing provisioning convention).
    let password = req
        .password
        .clone()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(SipDeviceService::generate_password);

    let max_registrations = plan.max_concurrent_calls.clamp(1, 10);

    let mut device = SipDevice {
        id: 0,
        account_id: created_account.id,
        sip_username: req.username.clone(),
        sip_domain: domain.clone(),
        password_encrypted: Vec::new(),
        password_nonce: Vec::new(),
        a1_hash: String::new(),
        display_name: req.cod_ped.clone().filter(|s| !s.trim().is_empty()),
        description: None,
        context: DEFAULT_CONTEXT.to_string(),
        accountcode: None,
        codecs: DEFAULT_CODECS.to_string(),
        max_registrations,
        enabled: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    if let Err(e) = sip_service.prepare_device_with_password(&mut device, &password) {
        // Roll back the account so we don't leave an account without a device.
        let _ = account_repo.delete(created_account.id).await;
        return Err(e);
    }

    let created_device = match sip_repo.create(&device).await {
        Ok(d) => d,
        Err(e) => {
            let _ = account_repo.delete(created_account.id).await;
            return Err(e);
        }
    };

    info!(
        account_id = created_account.id,
        sip_device_id = created_device.id,
        username = %req.username,
        "Provisioned account + SIP device"
    );

    Ok(ProvisionOutcome::Created(ProvisionResult {
        account_id: created_account.id,
        account_number: created_account.account_number,
        sip_device_id: created_device.id,
        sip_username: created_device.sip_username,
        sip_domain: created_device.sip_domain,
        password,
        plan_id: plan.id,
    }))
}

/// Provision a single customer (account + SIP device).
///
/// POST /api/v1/provisioning
#[instrument(skip(pool, sip_service, admin, req))]
pub async fn provision(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipDeviceService>>,
    admin: AdminUser,
    req: web::Json<ProvisionRequest>,
) -> Result<HttpResponse, AppError> {
    let req = req.into_inner();

    match provision_one(pool.get_ref(), sip_service.get_ref(), &req).await? {
        ProvisionOutcome::Created(result) => {
            audit_provision(pool.get_ref(), &admin.username, &result).await;
            Ok(HttpResponse::Created().json(ApiResponse::with_message(
                result,
                "Cliente aprovisionado correctamente",
            )))
        }
        ProvisionOutcome::Skipped(reason) => Err(AppError::AlreadyExists(reason)),
    }
}

/// Provision many customers from an uploaded template, reporting per-row.
///
/// POST /api/v1/provisioning/bulk
#[instrument(skip(pool, sip_service, admin, req))]
pub async fn provision_bulk(
    pool: web::Data<PgPool>,
    sip_service: web::Data<Arc<SipDeviceService>>,
    admin: AdminUser,
    req: web::Json<BulkProvisionRequest>,
) -> Result<HttpResponse, AppError> {
    let rows = req.into_inner().rows;

    if rows.is_empty() {
        return Err(AppError::Validation(
            "El archivo no contiene filas".to_string(),
        ));
    }
    if rows.len() > 5000 {
        return Err(AppError::Validation(
            "El archivo excede el máximo de 5000 filas por carga".to_string(),
        ));
    }

    let mut results = Vec::with_capacity(rows.len());
    let (mut created, mut skipped, mut errors) = (0usize, 0usize, 0usize);

    for (idx, row) in rows.iter().enumerate() {
        let row_num = idx + 1;
        match provision_one(pool.get_ref(), sip_service.get_ref(), row).await {
            Ok(ProvisionOutcome::Created(result)) => {
                created += 1;
                audit_provision(pool.get_ref(), &admin.username, &result).await;
                results.push(BulkRowResult {
                    row: row_num,
                    username: row.username.clone(),
                    status: BulkRowStatus::Created,
                    message: None,
                    account_id: Some(result.account_id),
                    sip_device_id: Some(result.sip_device_id),
                    password: Some(result.password),
                });
            }
            Ok(ProvisionOutcome::Skipped(reason)) => {
                skipped += 1;
                results.push(BulkRowResult {
                    row: row_num,
                    username: row.username.clone(),
                    status: BulkRowStatus::Skipped,
                    message: Some(reason),
                    account_id: None,
                    sip_device_id: None,
                    password: None,
                });
            }
            Err(e) => {
                errors += 1;
                warn!(row = row_num, username = %row.username, error = %e, "Bulk provisioning row failed");
                results.push(BulkRowResult {
                    row: row_num,
                    username: row.username.clone(),
                    status: BulkRowStatus::Error,
                    message: Some(e.to_string()),
                    account_id: None,
                    sip_device_id: None,
                    password: None,
                });
            }
        }
    }

    let response = BulkProvisionResponse {
        total: rows.len(),
        created,
        skipped,
        errors,
        results,
    };

    info!(
        total = response.total,
        created, skipped, errors, "Bulk provisioning finished"
    );

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Carga masiva procesada",
    )))
}

/// Best-effort audit log for a successful provision.
async fn audit_provision(pool: &PgPool, username: &str, result: &ProvisionResult) {
    let details = serde_json::json!({
        "account_id": result.account_id,
        "account_number": result.account_number,
        "sip_device_id": result.sip_device_id,
        "sip_username": result.sip_username,
        "plan_id": result.plan_id,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(username.to_string())
        .action("provision_account")
        .entity_type("account")
        .entity_id(result.account_id.to_string())
        .details(details)
        .build()
    {
        audit_data.insert(pool).await;
    }
}

/// Configure provisioning routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/provisioning")
            .route("", web::post().to(provision))
            .route("/bulk", web::post().to(provision_bulk)),
    );
}
