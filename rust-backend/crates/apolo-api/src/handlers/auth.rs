//! Authentication handlers
//!
//! HTTP handlers for authentication endpoints.

use crate::dto::auth::{
    ChangePasswordRequest, ChangePasswordResponse, LoginRequest, LoginResponse, LogoutResponse,
    MeResponse, RegisterRequest,
};
use crate::dto::ApiResponse;
use actix_web::{cookie::Cookie, web, HttpRequest, HttpResponse};
use apolo_auth::{AuthenticatedUser, JwtService, PasswordService};
use apolo_core::models::{AuditLogBuilder, User, UserInfo, UserRole};
use apolo_core::traits::{Repository, UserRepository};
use apolo_core::AppError;
use apolo_db::PgUserRepository;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use validator::Validate;

/// Login endpoint
///
/// POST /api/v1/auth/login
#[instrument(skip(pool, jwt_service, password_service, req))]
pub async fn login(
    pool: web::Data<PgPool>,
    jwt_service: web::Data<Arc<JwtService>>,
    password_service: web::Data<Arc<PasswordService>>,
    req: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    // Validate request
    req.validate().map_err(|e| {
        warn!("Login validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let username = req.username.trim();
    let password = &req.password;

    debug!(username = %username, "Processing login request");

    // Find user in database
    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let user = user_repo.find_by_username(username).await?.ok_or_else(|| {
        info!(username = %username, "Login failed: user not found");
        AppError::InvalidCredentials
    })?;

    // Check if user is active
    if !user.can_login() {
        warn!(username = %username, "Login failed: user is inactive");
        return Err(AppError::InvalidCredentials);
    }

    // Verify password
    let password_valid = password_service
        .verify_password(password, &user.password_hash)
        .map_err(|e| {
            error!("Password verification error: {}", e);
            AppError::Internal("Password verification failed".to_string())
        })?;

    if !password_valid {
        info!(username = %username, "Login failed: invalid password");
        return Err(AppError::InvalidCredentials);
    }

    // Update last login
    if let Err(e) = user_repo.update_last_login(user.id).await {
        warn!("Failed to update last login for user {}: {}", user.id, e);
    }

    // Generate JWT token
    let token = jwt_service.create_token_for_user(&user.username, user.role)?;
    let expires_in = jwt_service.expiration_secs();

    info!(username = %username, role = ?user.role, "Login successful");

    // Audit log
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("login")
        .entity_type("auth")
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Create response
    let user_info = UserInfo::from(&user);
    let response = LoginResponse::new(token.clone(), expires_in, user_info);

    // Set cookie with token
    let cookie = Cookie::build("token", token)
        .path("/")
        .http_only(true)
        .secure(false) // Set to true in production with HTTPS
        .max_age(actix_web::cookie::time::Duration::seconds(expires_in))
        .finish();

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .json(ApiResponse::success(response)))
}

/// Logout endpoint
///
/// POST /api/v1/auth/logout
#[instrument(skip(pool, _user))]
pub async fn logout(pool: web::Data<PgPool>, _user: AuthenticatedUser) -> HttpResponse {
    info!(username = %_user.username, "User logged out");

    // Audit log
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("logout")
        .entity_type("auth")
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    // Clear the token cookie
    let cookie = Cookie::build("token", "")
        .path("/")
        .http_only(true)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish();

    HttpResponse::Ok()
        .cookie(cookie)
        .json(ApiResponse::success(LogoutResponse::default()))
}

/// Get current user info
///
/// GET /api/v1/auth/me
#[instrument(skip(pool, user))]
pub async fn me(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!(username = %user.username, "Getting current user info");

    // Get fresh user data from database
    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let db_user = user_repo
        .find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let user_info = UserInfo::from(&db_user);
    let token_expires_at = Utc::now() + Duration::seconds(user.claims.exp - Utc::now().timestamp());

    let response = MeResponse {
        user: user_info,
        token_expires_at,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Register new user (admin only)
///
/// POST /api/v1/auth/register
#[instrument(skip(pool, password_service, admin, req))]
pub async fn register(
    pool: web::Data<PgPool>,
    password_service: web::Data<Arc<PasswordService>>,
    admin: apolo_auth::AdminUser,
    req: web::Json<RegisterRequest>,
) -> Result<HttpResponse, AppError> {
    // Validate request
    req.validate().map_err(|e| {
        warn!("Register validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        username = %req.username,
        admin = %admin.username,
        "Processing registration request"
    );

    // Parse role
    let role = UserRole::from_str(&req.role).unwrap_or(UserRole::Operator);

    // Check if admin can create this role
    if !admin.user_role().can_manage(&role) {
        warn!(
            admin = %admin.username,
            target_role = ?role,
            "Admin cannot create user with higher role"
        );
        return Err(AppError::Forbidden);
    }

    // Hash password
    let password_hash = password_service.hash_password(&req.password)?;

    // Create user
    let new_user = User {
        id: 0, // Will be set by database
        username: req.username.clone(),
        password_hash,
        nombre: req.nombre.clone(),
        apellido: req.apellido.clone(),
        email: req.email.clone(),
        role,
        activo: true,
        ultimo_login: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let created_user = user_repo.create(&new_user).await?;

    info!(
        username = %created_user.username,
        id = %created_user.id,
        admin = %admin.username,
        "User registered successfully"
    );

    let user_info = UserInfo::from(&created_user);
    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        user_info,
        "User created successfully",
    )))
}

/// Change password
///
/// POST /api/v1/auth/change-password
#[instrument(skip(pool, password_service, user, req))]
pub async fn change_password(
    pool: web::Data<PgPool>,
    password_service: web::Data<Arc<PasswordService>>,
    user: AuthenticatedUser,
    req: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, AppError> {
    // Validate request
    req.validate().map_err(|e| {
        warn!("Change password validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(username = %user.username, "Processing password change request");

    // Get current user from database
    let user_repo = PgUserRepository::new(pool.get_ref().clone());
    let mut db_user = user_repo
        .find_by_username(&user.username)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Verify current password
    let current_valid =
        password_service.verify_password(&req.current_password, &db_user.password_hash)?;

    if !current_valid {
        warn!(username = %user.username, "Change password failed: invalid current password");
        return Err(AppError::InvalidCredentials);
    }

    // Hash new password
    let new_hash = password_service.hash_password(&req.new_password)?;
    db_user.password_hash = new_hash;
    db_user.updated_at = Utc::now();

    // Update user
    user_repo.update(&db_user).await?;

    info!(username = %user.username, "Password changed successfully");

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(ChangePasswordResponse {
            message: "Password changed successfully".to_string(),
        })),
    )
}

/// Configure auth routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/me", web::get().to(me))
            .route("/register", web::post().to(register))
            .route("/change-password", web::post().to(change_password)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_validation() {
        let valid_req = LoginRequest {
            username: "admin".to_string(),
            password: "password".to_string(),
        };
        assert!(valid_req.validate().is_ok());

        let invalid_req = LoginRequest {
            username: "".to_string(),
            password: "".to_string(),
        };
        assert!(invalid_req.validate().is_err());
    }
}
