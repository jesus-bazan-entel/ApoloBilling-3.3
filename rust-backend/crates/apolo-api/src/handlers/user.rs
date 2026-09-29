//! User management handlers
//!
//! HTTP handlers for user CRUD operations. Accessible to Admin and Superadmin,
//! with hierarchy enforced per request:
//! - Superadmin can create/edit/delete anyone (Usuario, Admin, Superadmin).
//! - Admin can only create/edit/delete Usuario accounts, and can only ever
//!   assign the Usuario role. Admin accounts and Superadmin accounts are
//!   invisible to (and untouchable by) other Admins.
//! - Anyone can view/edit their own basic profile fields (name/email), but
//!   never their own role or active status.

use crate::dto::{
    ApiResponse, PaginationParams, UserCreateRequest, UserListResponse, UserResponse,
    UserUpdateRequest,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{middleware::AuthenticatedUser, PasswordService};
use apolo_core::{
    models::{AuditLogBuilder, UserRole},
    traits::{Repository, UserRepository},
    AppError,
};
use apolo_db::PgUserRepository;
use sqlx::PgPool;
use tracing::debug;
use validator::Validate;

/// Load the DB record for the currently authenticated session.
async fn load_current_user(
    repo: &PgUserRepository,
    user: &AuthenticatedUser,
) -> Result<apolo_core::models::User, AppError> {
    repo.find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("Current user not found".to_string()))
}

/// List users (Admin+). Admins never see Admin/Superadmin peers' accounts —
/// only their own profile row and Usuario accounts.
pub async fn list_users(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden);
    }

    debug!("Listing users");

    let page = query.page.max(1);
    let per_page = query.per_page.min(100).max(1);

    let repo = PgUserRepository::new(pool.get_ref().clone());

    let (visible, total) = if user.is_superadmin() {
        let offset = (page - 1) * per_page;
        let total = repo.count().await?;
        let users = repo.find_all(per_page, offset).await?;
        (users, total)
    } else {
        // Small deployment scale: fetch everything and filter/paginate in
        // memory so the reported `total` matches what Admin can actually see.
        let all = repo.find_all(10_000, 0).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|u| u.role != UserRole::Superadmin)
            .collect();
        let total = filtered.len() as i64;
        let offset = ((page - 1) * per_page) as usize;
        let page_slice = filtered
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();
        (page_slice, total)
    };

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    let response = UserListResponse {
        users: visible.iter().map(UserResponse::from).collect(),
        total,
        page,
        per_page,
        total_pages,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Get user by ID (Admin+, or own profile). Admins cannot view Superadmin profiles.
pub async fn get_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    let repo = PgUserRepository::new(pool.get_ref().clone());
    let current_user = load_current_user(&repo, &user).await?;

    let found_user = repo
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    let is_own_profile = current_user.id == user_id;
    let can_view = user.is_superadmin()
        || is_own_profile
        || (user.is_admin() && found_user.role != UserRole::Superadmin);

    if !can_view {
        return Err(AppError::Forbidden);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(UserResponse::from(found_user))))
}

/// Create a new user (Admin+). The requested role must be assignable by the
/// creator: Superadmin can assign any role; Admin can only assign Usuario.
pub async fn create_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    req: web::Json<UserCreateRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden);
    }

    req.validate()?;

    debug!("Creating user: {}", req.username);

    let role = UserRole::from_str(&req.role).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "Invalid role: {}. Must be one of: usuario, admin, superadmin",
            req.role
        ))
    })?;

    if !user.user_role().can_assign(&role) {
        return Err(AppError::Forbidden);
    }

    let password_service = PasswordService::new();
    let password_hash = password_service.hash_password(&req.password)?;

    let repo = PgUserRepository::new(pool.get_ref().clone());

    if repo.find_by_username(&req.username).await?.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "User {} already exists",
            req.username
        )));
    }

    if let Some(ref email) = req.email {
        if repo.find_by_email(email).await?.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Email {} already in use",
                email
            )));
        }
    }

    let new_user = apolo_core::models::User {
        id: 0,
        username: req.username.clone(),
        password_hash,
        nombre: req.nombre.clone(),
        apellido: req.apellido.clone(),
        email: req.email.clone(),
        role,
        activo: true,
        ultimo_login: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created = repo.create(&new_user).await?;

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("user_created")
        .entity_type("user")
        .entity_id(created.id.to_string())
        .details(serde_json::json!({
            "created_username": created.username,
            "role": role.to_string(),
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(UserResponse::from(created))))
}

/// Update a user (Admin+ for accounts they can manage, or own basic profile fields).
pub async fn update_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
    req: web::Json<UserUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    req.validate()?;

    let repo = PgUserRepository::new(pool.get_ref().clone());
    let current_user = load_current_user(&repo, &user).await?;

    let mut existing_user = repo
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    let is_own_profile = current_user.id == user_id;
    // Superadmin can touch any account (including another Superadmin, for
    // maintenance purposes). Admin can only touch accounts strictly below
    // its own level, i.e. Usuario accounts.
    let can_touch = user.is_superadmin() || user.user_role().can_manage(&existing_user.role);

    if !can_touch && !is_own_profile {
        return Err(AppError::Forbidden);
    }

    if let Some(nombre) = &req.nombre {
        existing_user.nombre = Some(nombre.clone());
    }

    if let Some(apellido) = &req.apellido {
        existing_user.apellido = Some(apellido.clone());
    }

    if let Some(email) = &req.email {
        if let Some(email_user) = repo.find_by_email(email).await? {
            if email_user.id != user_id {
                return Err(AppError::AlreadyExists(format!(
                    "Email {} already in use",
                    email
                )));
            }
        }
        existing_user.email = Some(email.clone());
    }

    let mut role_changed = false;

    // Only someone who can actually "touch" this account may change its
    // role or active status — never just via self-edit.
    if can_touch {
        if let Some(ref role_str) = req.role {
            let new_role = UserRole::from_str(role_str)
                .ok_or_else(|| AppError::InvalidInput(format!("Invalid role: {}", role_str)))?;

            if !user.user_role().can_assign(&new_role) {
                return Err(AppError::Forbidden);
            }

            if new_role != existing_user.role {
                role_changed = true;
            }
            existing_user.role = new_role;
        }

        if let Some(activo) = req.activo {
            if user_id == current_user.id && !activo {
                return Err(AppError::InvalidInput(
                    "You cannot deactivate your own account".to_string(),
                ));
            }
            existing_user.activo = activo;
        }
    } else if req.role.is_some() || req.activo.is_some() {
        return Err(AppError::Forbidden);
    }

    existing_user.updated_at = chrono::Utc::now();

    let updated = repo.update(&existing_user).await?;

    if role_changed {
        if let Ok(audit_data) = AuditLogBuilder::default()
            .username(user.username.clone())
            .action("user_role_changed")
            .entity_type("user")
            .entity_id(updated.id.to_string())
            .details(serde_json::json!({
                "target_username": updated.username,
                "new_role": updated.role.to_string(),
            }))
            .build()
        {
            audit_data.insert(pool.get_ref()).await;
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(UserResponse::from(updated))))
}

/// Delete a user (Admin+, only for accounts they can manage).
pub async fn delete_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    if !user.is_admin() {
        return Err(AppError::Forbidden);
    }

    let repo = PgUserRepository::new(pool.get_ref().clone());
    let current_user = load_current_user(&repo, &user).await?;

    if user_id == current_user.id {
        return Err(AppError::InvalidInput(
            "You cannot delete your own account".to_string(),
        ));
    }

    let existing_user = repo
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    let can_touch = user.is_superadmin() || user.user_role().can_manage(&existing_user.role);
    if !can_touch {
        return Err(AppError::Forbidden);
    }

    let deleted = repo.delete(user_id).await?;

    if !deleted {
        return Err(AppError::NotFound(format!("User {} not found", user_id)));
    }

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("user_deleted")
        .entity_type("user")
        .entity_id(user_id.to_string())
        .details(serde_json::json!({ "deleted_username": existing_user.username }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success("User deleted successfully")))
}

/// Configure user management routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/{id}", web::get().to(get_user))
            .route("/{id}", web::put().to(update_user))
            .route("/{id}", web::delete().to(delete_user))
            .route(
                "/{id}/modules",
                web::get().to(crate::handlers::module::get_user_modules),
            )
            .route(
                "/{id}/modules",
                web::put().to(crate::handlers::module::put_user_modules),
            ),
    );
}
