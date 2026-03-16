//! Authentication DTOs
//!
//! Request and response types for authentication endpoints.

use apolo_core::models::UserInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Login request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct LoginRequest {
    /// Username
    #[validate(length(min = 1, max = 100, message = "Username is required"))]
    pub username: String,

    /// Password
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

/// Login response
#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    /// Access token (JWT)
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiration time in seconds
    pub expires_in: i64,

    /// User information
    pub user: UserInfo,
}

impl LoginResponse {
    /// Create a new login response
    pub fn new(access_token: String, expires_in: i64, user: UserInfo) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in,
            user,
        }
    }
}

/// User registration request (admin only)
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RegisterRequest {
    /// Username
    #[validate(length(
        min = 3,
        max = 100,
        message = "Username must be between 3 and 100 characters"
    ))]
    pub username: String,

    /// Password
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,

    /// Email
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    /// First name
    pub nombre: Option<String>,

    /// Last name
    pub apellido: Option<String>,

    /// Role (admin, operator, superadmin)
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "operator".to_string()
}

/// Current user response
#[derive(Debug, Clone, Serialize)]
pub struct MeResponse {
    /// User information
    pub user: UserInfo,

    /// Token expiration timestamp
    pub token_expires_at: DateTime<Utc>,
}

/// Logout response
#[derive(Debug, Clone, Serialize)]
pub struct LogoutResponse {
    /// Success message
    pub message: String,
}

impl Default for LogoutResponse {
    fn default() -> Self {
        Self {
            message: "Logged out successfully".to_string(),
        }
    }
}

/// Password change request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    /// Current password
    #[validate(length(min = 1, message = "Current password is required"))]
    pub current_password: String,

    /// New password
    #[validate(length(min = 6, message = "New password must be at least 6 characters"))]
    pub new_password: String,
}

/// Password change response
#[derive(Debug, Clone, Serialize)]
pub struct ChangePasswordResponse {
    /// Success message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_validation() {
        let valid_request = LoginRequest {
            username: "admin".to_string(),
            password: "password123".to_string(),
        };
        assert!(valid_request.validate().is_ok());

        let invalid_request = LoginRequest {
            username: "".to_string(),
            password: "pass".to_string(),
        };
        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_register_request_validation() {
        let valid_request = RegisterRequest {
            username: "newuser".to_string(),
            password: "password123".to_string(),
            email: Some("user@example.com".to_string()),
            nombre: Some("John".to_string()),
            apellido: Some("Doe".to_string()),
            role: "operator".to_string(),
        };
        assert!(valid_request.validate().is_ok());

        let invalid_request = RegisterRequest {
            username: "ab".to_string(),               // Too short
            password: "12345".to_string(),            // Too short
            email: Some("invalid-email".to_string()), // Invalid email
            nombre: None,
            apellido: None,
            role: "operator".to_string(),
        };
        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_login_response() {
        let user_info = UserInfo {
            id: 1,
            username: "admin".to_string(),
            nombre: Some("Admin".to_string()),
            apellido: None,
            email: Some("admin@example.com".to_string()),
            role: "admin".to_string(),
            activo: true,
            ultimo_login: None,
        };

        let response = LoginResponse::new("jwt_token".to_string(), 3600, user_info);
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert_eq!(response.user.username, "admin");
    }
}
