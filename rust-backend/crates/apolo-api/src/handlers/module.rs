//! Module catalog and per-user module grant handlers
//!
//! `GET /modules` exposes the catalog (filtered to what the caller's own
//! role can see). `GET/PUT /users/{id}/modules` (wired from `handlers::user`)
//! manage explicit grants for a single `Usuario` account.

use crate::dto::{ApiResponse, ModuleGrantRequest, ModuleResponse, UserModulesResponse};
use actix_web::{web, HttpResponse};
use apolo_auth::middleware::AuthenticatedUser;
use apolo_core::{
    models::{AuditLogBuilder, UserRole},
    traits::{Repository, UserRepository},
    AppError,
};
use apolo_db::{PgModuleRepository, PgUserRepository};
use sqlx::PgPool;
use std::collections::HashSet;
use tracing::debug;

/// List the module catalog visible to the caller's own role (Admin+).
///
/// Admin sees `usuario`/`admin` modules; Superadmin sees everything. This is
/// the source of truth the frontend uses both to render the menu and to
/// build the "which modules can I grant" checklist (client-side: any entry
/// with `min_role == "usuario"`).
pub async fn list_modules(
    pool: web::Data<PgPool>,
    admin: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !admin.is_admin() {
        return Err(AppError::Forbidden);
    }

    let module_repo = PgModuleRepository::new(pool.get_ref().clone());
    let catalog = module_repo.list_catalog().await?;

    let visible: Vec<ModuleResponse> = catalog
        .iter()
        .filter(|m| m.min_role.level() <= admin.user_role().level())
        .map(ModuleResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(visible)))
}

/// Get the modules explicitly granted to a `Usuario` account (Admin+).
pub async fn get_user_modules(
    pool: web::Data<PgPool>,
    admin: AuthenticatedUser,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    if !admin.is_admin() {
        return Err(AppError::Forbidden);
    }

    let target_id = path.into_inner();
    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let module_repo = PgModuleRepository::new(pool.get_ref().clone());

    let target = user_repo
        .find_by_id(target_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", target_id)))?;

    let granted = if target.role == UserRole::Usuario {
        module_repo.list_granted_keys(target_id).await?
    } else {
        Vec::new()
    };

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(UserModulesResponse {
            usuario_id: target_id,
            granted,
        })),
    )
}

/// Replace the full set of modules granted to a `Usuario` account (Admin+).
///
/// Rejects the request outright if the target account isn't `role = usuario`,
/// or if any requested key isn't a real, usuario-eligible catalog entry —
/// this is what makes the restriction real rather than cosmetic: the
/// `dialplan` / `legacy_routing` / `system_settings` modules can never be
/// granted here, no matter what the caller sends.
pub async fn put_user_modules(
    pool: web::Data<PgPool>,
    admin: AuthenticatedUser,
    path: web::Path<i32>,
    req: web::Json<ModuleGrantRequest>,
) -> Result<HttpResponse, AppError> {
    if !admin.is_admin() {
        return Err(AppError::Forbidden);
    }

    let target_id = path.into_inner();
    debug!(target_id, "Updating module grants");

    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let module_repo = PgModuleRepository::new(pool.get_ref().clone());

    let current_user = user_repo
        .find_by_username(&admin.username)
        .await?
        .ok_or_else(|| AppError::NotFound("Current user not found".to_string()))?;

    let target = user_repo
        .find_by_id(target_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", target_id)))?;

    if target.role != UserRole::Usuario {
        return Err(AppError::InvalidInput(
            "Module grants only apply to accounts with role 'usuario'".to_string(),
        ));
    }

    let catalog = module_repo.list_catalog().await?;
    let usuario_eligible: HashSet<&str> = catalog
        .iter()
        .filter(|m| m.activo && m.min_role == UserRole::Usuario)
        .map(|m| m.module_key.as_str())
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut valid_keys: Vec<String> = Vec::new();
    for key in &req.modules {
        if !usuario_eligible.contains(key.as_str()) {
            return Err(AppError::InvalidInput(format!(
                "Module '{}' does not exist or is not assignable to a usuario account",
                key
            )));
        }
        if seen.insert(key.clone()) {
            valid_keys.push(key.clone());
        }
    }

    let before = module_repo.list_granted_keys(target_id).await?;
    let after = module_repo
        .replace_granted(target_id, &valid_keys, Some(current_user.id))
        .await?;

    let added: Vec<&String> = after.iter().filter(|k| !before.contains(k)).collect();
    let removed: Vec<&String> = before.iter().filter(|k| !after.contains(k)).collect();

    if !added.is_empty() || !removed.is_empty() {
        if let Ok(audit_data) = AuditLogBuilder::default()
            .username(admin.username.clone())
            .action("module_grant_updated")
            .entity_type("user_modules")
            .entity_id(target_id.to_string())
            .details(serde_json::json!({
                "target_username": target.username,
                "added": added,
                "removed": removed,
            }))
            .build()
        {
            audit_data.insert(pool.get_ref()).await;
        }
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(UserModulesResponse {
            usuario_id: target_id,
            granted: after,
        })),
    )
}

/// Configure module catalog routes (`/users/{id}/modules` is wired from
/// `handlers::user::configure` since it lives under the `/users` scope).
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/modules").route(web::get().to(list_modules)));
}
