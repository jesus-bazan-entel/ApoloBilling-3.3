//! User DTOs
//!
//! Data Transfer Objects for user management endpoints.

use apolo_core::models::{User, UserInfo, UserRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request to create a new user
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UserCreateRequest {
    #[validate(length(min = 3, max = 50, message = "Username must be between 3 and 50 characters"))]
    pub username: String,

    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,

    pub nombre: Option<String>,
    pub apellido: Option<String>,

    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(length(min = 1, message = "Role is required"))]
    pub role: String,
}

/// Request to update a user
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UserUpdateRequest {
    pub nombre: Option<String>,
    pub apellido: Option<String>,

    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    pub role: Option<String>,
    pub activo: Option<bool>,
}

/// Response containing user information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub nombre: Option<String>,
    pub apellido: Option<String>,
    pub email: Option<String>,
    pub role: String,
    pub activo: bool,
    pub ultimo_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            nombre: user.nombre,
            apellido: user.apellido,
            email: user.email,
            role: user.role.to_string(),
            activo: user.activo,
            ultimo_login: user.ultimo_login,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            username: user.username.clone(),
            nombre: user.nombre.clone(),
            apellido: user.apellido.clone(),
            email: user.email.clone(),
            role: user.role.to_string(),
            activo: user.activo,
            ultimo_login: user.ultimo_login,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

/// Paginated list of users
#[derive(Debug, Clone, Serialize)]
pub struct UserListResponse {
    pub users: Vec<UserResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}
