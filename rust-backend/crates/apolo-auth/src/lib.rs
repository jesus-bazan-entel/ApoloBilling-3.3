//! Authentication and authorization for ApoloBilling
//!
//! This crate provides JWT-based authentication, password hashing with Argon2,
//! and Actix-web middleware for role-based access control.
//!
//! # Features
//!
//! - JWT token creation and validation
//! - Argon2 password hashing and verification
//! - Request extractors for authenticated users
//! - Role-based access control (RBAC)
//!
//! # Examples
//!
//! ## Creating a JWT token
//!
//! ```no_run
//! use apolo_auth::{JwtService, Claims};
//! use apolo_core::models::UserRole;
//!
//! let jwt_service = JwtService::new("your-secret-key", 3600);
//! let claims = Claims::new("admin", UserRole::Admin);
//! let token = jwt_service.create_token(&claims)?;
//! # Ok::<(), apolo_core::error::AppError>(())
//! ```
//!
//! ## Password hashing
//!
//! ```no_run
//! use apolo_auth::PasswordService;
//!
//! let password_service = PasswordService::new();
//! let hash = password_service.hash_password("secure_password")?;
//! let is_valid = password_service.verify_password("secure_password", &hash)?;
//! assert!(is_valid);
//! # Ok::<(), apolo_core::error::AppError>(())
//! ```
//!
//! ## Using extractors in Actix-web
//!
//! ```no_run
//! use actix_web::{web, HttpResponse};
//! use apolo_auth::middleware::{AuthenticatedUser, AdminUser};
//!
//! async fn protected_route(user: AuthenticatedUser) -> HttpResponse {
//!     HttpResponse::Ok().json(serde_json::json!({
//!         "username": user.username,
//!         "role": user.role
//!     }))
//! }
//!
//! async fn admin_route(admin: AdminUser) -> HttpResponse {
//!     HttpResponse::Ok().json(serde_json::json!({
//!         "message": "Admin access granted"
//!     }))
//! }
//! ```

pub mod claims;
pub mod jwt;
pub mod middleware;
pub mod password;

pub use claims::Claims;
pub use jwt::JwtService;
pub use middleware::{AdminUser, AuthenticatedUser, SuperadminUser};
pub use password::PasswordService;

#[cfg(test)]
mod tests {
    use super::*;
    use apolo_core::models::UserRole;

    #[test]
    fn test_integration_jwt_and_password() {
        let password_service = PasswordService::new();
        let jwt_service = JwtService::new("test-secret-key-12345", 3600);

        // Test password hashing
        let password = "my_secure_password";
        let hash = password_service.hash_password(password).unwrap();
        assert!(password_service.verify_password(password, &hash).unwrap());
        assert!(!password_service
            .verify_password("wrong_password", &hash)
            .unwrap());

        // Test JWT creation and validation
        let claims = Claims::new("testuser", UserRole::Admin);
        let token = jwt_service.create_token(&claims).unwrap();
        let decoded_claims = jwt_service.validate_token(&token).unwrap();

        assert_eq!(decoded_claims.sub, "testuser");
        assert_eq!(decoded_claims.role, UserRole::Admin);
    }
}
