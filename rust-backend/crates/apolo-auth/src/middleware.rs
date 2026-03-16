//! Actix-web authentication middleware and request extractors
//!
//! Provides extractors for authenticated users with role-based access control.

use crate::jwt::JwtService;
use crate::Claims;
use actix_web::{dev::Payload, error::ErrorUnauthorized, web, FromRequest, HttpRequest};
use apolo_core::error::AppError;
use apolo_core::models::UserRole;
use futures::future::{ready, Ready};
use std::sync::Arc;
use tracing::{debug, warn};

/// Extract JWT token from request
///
/// Checks for token in the following order:
/// 1. Authorization header (Bearer token)
/// 2. Cookie named "token"
fn extract_token_from_request(req: &HttpRequest) -> Option<String> {
    // Try Authorization header first
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }

    // Try cookie
    if let Some(cookie) = req.cookie("token") {
        return Some(cookie.value().to_string());
    }

    None
}

/// Authenticated user extractor
///
/// Extracts and validates JWT token from request, providing access to user information.
/// Can be used as a request extractor in Actix-web handlers.
///
/// # Examples
///
/// ```no_run
/// use actix_web::{web, HttpResponse};
/// use apolo_auth::middleware::AuthenticatedUser;
///
/// async fn protected_handler(user: AuthenticatedUser) -> HttpResponse {
///     HttpResponse::Ok().json(serde_json::json!({
///         "username": user.username,
///         "role": user.role
///     }))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// Username of the authenticated user
    pub username: String,

    /// Role of the authenticated user
    pub role: String,

    /// Full claims from the JWT token
    pub claims: Claims,
}

impl AuthenticatedUser {
    /// Get the user's role as a UserRole enum
    pub fn user_role(&self) -> UserRole {
        self.claims.role
    }

    /// Check if user has admin privileges
    pub fn is_admin(&self) -> bool {
        self.claims.is_admin()
    }

    /// Check if user has superadmin privileges
    pub fn is_superadmin(&self) -> bool {
        self.claims.is_superadmin()
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // Extract JWT service from app data
        let jwt_service = match req.app_data::<web::Data<Arc<JwtService>>>() {
            Some(service) => service.get_ref().clone(),
            None => {
                warn!("JwtService not found in app data");
                return ready(Err(ErrorUnauthorized(AppError::Unauthorized(
                    "Authentication service not configured".to_string(),
                ))));
            }
        };

        // Extract token from request
        let token = match extract_token_from_request(req) {
            Some(t) => t,
            None => {
                debug!("No authentication token found in request");
                return ready(Err(ErrorUnauthorized(AppError::Unauthorized(
                    "No authentication token provided".to_string(),
                ))));
            }
        };

        // Validate token and extract claims
        match jwt_service.validate_token(&token) {
            Ok(claims) => {
                debug!(
                    username = %claims.sub,
                    role = ?claims.role,
                    "User authenticated successfully"
                );

                ready(Ok(AuthenticatedUser {
                    username: claims.sub.clone(),
                    role: claims.role.to_string(),
                    claims,
                }))
            }
            Err(e) => {
                warn!(error = %e, "Token validation failed");
                ready(Err(ErrorUnauthorized(e)))
            }
        }
    }
}

/// Admin user extractor
///
/// Requires the user to have admin or superadmin role.
/// Returns `Forbidden` error if the user doesn't have sufficient privileges.
///
/// # Examples
///
/// ```no_run
/// use actix_web::{web, HttpResponse};
/// use apolo_auth::middleware::AdminUser;
///
/// async fn admin_handler(admin: AdminUser) -> HttpResponse {
///     HttpResponse::Ok().json(serde_json::json!({
///         "message": "Admin access granted",
///         "username": admin.0.username
///     }))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthenticatedUser);

impl std::ops::Deref for AdminUser {
    type Target = AuthenticatedUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequest for AdminUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let auth_user = match AuthenticatedUser::from_request(req, payload).into_inner() {
            Ok(user) => user,
            Err(e) => return ready(Err(e)),
        };

        // Check if user has admin privileges
        if !auth_user.is_admin() {
            warn!(
                username = %auth_user.username,
                role = %auth_user.role,
                "User attempted admin access without privileges"
            );
            return ready(Err(ErrorUnauthorized(AppError::Forbidden)));
        }

        debug!(
            username = %auth_user.username,
            role = %auth_user.role,
            "Admin access granted"
        );

        ready(Ok(AdminUser(auth_user)))
    }
}

/// Superadmin user extractor
///
/// Requires the user to have superadmin role.
/// Returns `Forbidden` error if the user doesn't have superadmin privileges.
///
/// # Examples
///
/// ```no_run
/// use actix_web::{web, HttpResponse};
/// use apolo_auth::middleware::SuperadminUser;
///
/// async fn superadmin_handler(superadmin: SuperadminUser) -> HttpResponse {
///     HttpResponse::Ok().json(serde_json::json!({
///         "message": "Superadmin access granted",
///         "username": superadmin.0.username
///     }))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SuperadminUser(pub AuthenticatedUser);

impl std::ops::Deref for SuperadminUser {
    type Target = AuthenticatedUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequest for SuperadminUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let auth_user = match AuthenticatedUser::from_request(req, payload).into_inner() {
            Ok(user) => user,
            Err(e) => return ready(Err(e)),
        };

        // Check if user has superadmin privileges
        if !auth_user.is_superadmin() {
            warn!(
                username = %auth_user.username,
                role = %auth_user.role,
                "User attempted superadmin access without privileges"
            );
            return ready(Err(ErrorUnauthorized(AppError::Forbidden)));
        }

        debug!(
            username = %auth_user.username,
            role = %auth_user.role,
            "Superadmin access granted"
        );

        ready(Ok(SuperadminUser(auth_user)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    fn create_test_jwt_service() -> Arc<JwtService> {
        Arc::new(JwtService::new("test-secret-key-12345", 3600))
    }

    #[actix_web::test]
    async fn test_extract_token_from_authorization_header() {
        let jwt_service = create_test_jwt_service();
        let token = jwt_service
            .create_token_for_user("testuser", UserRole::Operator)
            .unwrap();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/test",
            web::get().to(|user: AuthenticatedUser| async move {
                assert_eq!(user.username, "testuser");
                "OK"
            }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_missing_token() {
        let jwt_service = create_test_jwt_service();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/test",
            web::get().to(|_user: AuthenticatedUser| async { "OK" }),
        ))
        .await;

        let req = test::TestRequest::get().uri("/test").to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_invalid_token() {
        let jwt_service = create_test_jwt_service();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/test",
            web::get().to(|_user: AuthenticatedUser| async { "OK" }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Authorization", "Bearer invalid.token.here"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_admin_user_with_admin_role() {
        let jwt_service = create_test_jwt_service();
        let token = jwt_service
            .create_token_for_user("admin", UserRole::Admin)
            .unwrap();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/admin",
            web::get().to(|admin: AdminUser| async move {
                assert_eq!(admin.username, "admin");
                "OK"
            }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/admin")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_admin_user_with_operator_role() {
        let jwt_service = create_test_jwt_service();
        let token = jwt_service
            .create_token_for_user("operator", UserRole::Operator)
            .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(jwt_service))
                .route("/admin", web::get().to(|_admin: AdminUser| async { "OK" })),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/admin")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401); // Forbidden
    }

    #[actix_web::test]
    async fn test_superadmin_user_with_superadmin_role() {
        let jwt_service = create_test_jwt_service();
        let token = jwt_service
            .create_token_for_user("superadmin", UserRole::Superadmin)
            .unwrap();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/superadmin",
            web::get().to(|superadmin: SuperadminUser| async move {
                assert_eq!(superadmin.username, "superadmin");
                "OK"
            }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/superadmin")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_superadmin_user_with_admin_role() {
        let jwt_service = create_test_jwt_service();
        let token = jwt_service
            .create_token_for_user("admin", UserRole::Admin)
            .unwrap();

        let app = test::init_service(App::new().app_data(web::Data::new(jwt_service)).route(
            "/superadmin",
            web::get().to(|_superadmin: SuperadminUser| async { "OK" }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/superadmin")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401); // Forbidden
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_authenticated_user_methods() {
        let claims = Claims::new("testuser", UserRole::Admin);
        let user = AuthenticatedUser {
            username: claims.sub.clone(),
            role: claims.role.to_string(),
            claims: claims.clone(),
        };

        assert_eq!(user.user_role(), UserRole::Admin);
        assert!(user.is_admin());
        assert!(!user.is_superadmin());
    }

    #[test]
    fn test_admin_user_deref() {
        let claims = Claims::new("admin", UserRole::Admin);
        let auth_user = AuthenticatedUser {
            username: claims.sub.clone(),
            role: claims.role.to_string(),
            claims,
        };
        let admin = AdminUser(auth_user);

        assert_eq!(admin.username, "admin");
        assert!(admin.is_admin());
    }

    #[test]
    fn test_superadmin_user_deref() {
        let claims = Claims::new("superadmin", UserRole::Superadmin);
        let auth_user = AuthenticatedUser {
            username: claims.sub.clone(),
            role: claims.role.to_string(),
            claims,
        };
        let superadmin = SuperadminUser(auth_user);

        assert_eq!(superadmin.username, "superadmin");
        assert!(superadmin.is_superadmin());
    }
}
