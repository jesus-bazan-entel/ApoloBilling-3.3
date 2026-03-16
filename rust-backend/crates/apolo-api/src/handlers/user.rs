//! User management handlers
//!
//! HTTP handlers for user CRUD operations (superadmin only).

use crate::dto::{
    ApiResponse, PaginationParams, UserCreateRequest, UserListResponse, UserResponse,
    UserUpdateRequest,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{middleware::AuthenticatedUser, PasswordService};
use apolo_core::{
    models::UserRole,
    traits::{Repository, UserRepository},
    AppError,
};
use apolo_db::PgUserRepository;
use sqlx::PgPool;
use tracing::debug;
use validator::Validate;

/// List all users (superadmin only)
pub async fn list_users(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    // Only superadmin can list users
    if !user.is_superadmin() {
        return Err(AppError::Forbidden);
    }

    debug!("Listing users");

    let page = query.page.max(1);
    let per_page = query.per_page.min(100).max(1);
    let offset = (page - 1) * per_page;

    let repo = PgUserRepository::new(pool.get_ref().clone());

    // Get total count
    let total = repo.count().await?;

    // Get users
    let users = repo.find_all(per_page, offset).await?;

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    let response = UserListResponse {
        users: users.iter().map(UserResponse::from).collect(),
        total,
        page,
        per_page,
        total_pages,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Get user by ID (superadmin only, or own profile)
pub async fn get_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    let repo = PgUserRepository::new(pool.get_ref().clone());

    // Get current user's ID
    let current_user = repo
        .find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("Current user not found".to_string()))?;

    // Superadmin can view any user, others can only view their own profile
    if !user.is_superadmin() && current_user.id != user_id {
        return Err(AppError::Forbidden);
    }

    let found_user = repo
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(UserResponse::from(found_user))))
}

/// Create a new user (superadmin only)
pub async fn create_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    req: web::Json<UserCreateRequest>,
) -> Result<HttpResponse, AppError> {
    // Only superadmin can create users
    if !user.is_superadmin() {
        return Err(AppError::Forbidden);
    }

    req.validate()?;

    debug!("Creating user: {}", req.username);

    // Parse and validate role
    let role = UserRole::from_str(&req.role).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "Invalid role: {}. Must be one of: operator, admin, superadmin",
            req.role
        ))
    })?;

    // Hash password
    let password_service = PasswordService::new();
    let password_hash = password_service.hash_password(&req.password)?;

    let repo = PgUserRepository::new(pool.get_ref().clone());

    // Check if username already exists
    if repo.find_by_username(&req.username).await?.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "User {} already exists",
            req.username
        )));
    }

    // Check if email already exists (if provided)
    if let Some(ref email) = req.email {
        if repo.find_by_email(email).await?.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Email {} already in use",
                email
            )));
        }
    }

    // Create user
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

    Ok(HttpResponse::Created().json(ApiResponse::success(UserResponse::from(created))))
}

/// Update a user (superadmin only, or own profile with restrictions)
pub async fn update_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
    req: web::Json<UserUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    req.validate()?;

    let repo = PgUserRepository::new(pool.get_ref().clone());

    // Get current user's ID
    let current_user = repo
        .find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("Current user not found".to_string()))?;

    // Get existing user to update
    let mut existing_user = repo
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    // Check permissions
    let is_own_profile = current_user.id == user_id;
    let is_superadmin = user.is_superadmin();

    if !is_superadmin && !is_own_profile {
        return Err(AppError::Forbidden);
    }

    // Update fields
    if let Some(nombre) = &req.nombre {
        existing_user.nombre = Some(nombre.clone());
    }

    if let Some(apellido) = &req.apellido {
        existing_user.apellido = Some(apellido.clone());
    }

    if let Some(email) = &req.email {
        // Check if email is already in use by another user
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

    // Only superadmin can change role and active status
    if is_superadmin {
        if let Some(ref role_str) = req.role {
            let new_role = UserRole::from_str(role_str).ok_or_else(|| {
                AppError::InvalidInput(format!("Invalid role: {}", role_str))
            })?;
            existing_user.role = new_role;
        }

        if let Some(activo) = req.activo {
            // Prevent superadmin from deactivating themselves
            if user_id == current_user.id && !activo {
                return Err(AppError::InvalidInput(
                    "You cannot deactivate your own account".to_string(),
                ));
            }
            existing_user.activo = activo;
        }
    } else {
        // Non-superadmin cannot change role or active status
        if req.role.is_some() || req.activo.is_some() {
            return Err(AppError::Forbidden);
        }
    }

    existing_user.updated_at = chrono::Utc::now();

    let updated = repo.update(&existing_user).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(UserResponse::from(updated))))
}

/// Delete a user (superadmin only)
pub async fn delete_user(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    path: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let user_id = path.into_inner();

    // Only superadmin can delete users
    if !user.is_superadmin() {
        return Err(AppError::Forbidden);
    }

    let repo = PgUserRepository::new(pool.get_ref().clone());

    // Get current user's ID
    let current_user = repo
        .find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("Current user not found".to_string()))?;

    // Prevent superadmin from deleting themselves
    if user_id == current_user.id {
        return Err(AppError::InvalidInput(
            "You cannot delete your own account".to_string(),
        ));
    }

    let deleted = repo.delete(user_id).await?;

    if !deleted {
        return Err(AppError::NotFound(format!("User {} not found", user_id)));
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
            .route("/{id}", web::delete().to(delete_user)),
    );
}
