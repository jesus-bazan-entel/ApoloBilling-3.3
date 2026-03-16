//! User model
//!
//! Represents system users for authentication and authorization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// User role enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Standard operator with limited access
    #[default]
    Operator,
    /// Administrator with full access to most features
    Admin,
    /// Super administrator with system-wide access
    Superadmin,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::Operator => write!(f, "operator"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Superadmin => write!(f, "superadmin"),
        }
    }
}

impl UserRole {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "operator" => Some(UserRole::Operator),
            "admin" => Some(UserRole::Admin),
            "superadmin" => Some(UserRole::Superadmin),
            _ => None,
        }
    }

    /// Check if role has admin privileges
    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Superadmin)
    }

    /// Check if role has superadmin privileges
    pub fn is_superadmin(&self) -> bool {
        matches!(self, UserRole::Superadmin)
    }

    /// Get role hierarchy level (higher = more privileges)
    pub fn level(&self) -> u8 {
        match self {
            UserRole::Operator => 1,
            UserRole::Admin => 2,
            UserRole::Superadmin => 3,
        }
    }

    /// Check if this role can manage another role
    pub fn can_manage(&self, other: &UserRole) -> bool {
        self.level() > other.level()
    }
}

/// User entity
///
/// Represents a system user for authentication and authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique identifier
    pub id: i32,

    /// Username (unique, for login)
    pub username: String,

    /// Password hash (never expose in API responses)
    #[serde(skip_serializing)]
    pub password_hash: String,

    /// First name
    pub nombre: Option<String>,

    /// Last name
    pub apellido: Option<String>,

    /// Email address
    pub email: Option<String>,

    /// User role
    pub role: UserRole,

    /// Whether user is active
    pub activo: bool,

    /// Last login timestamp
    pub ultimo_login: Option<DateTime<Utc>>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Get full name
    pub fn full_name(&self) -> String {
        match (&self.nombre, &self.apellido) {
            (Some(nombre), Some(apellido)) => format!("{} {}", nombre, apellido),
            (Some(nombre), None) => nombre.clone(),
            (None, Some(apellido)) => apellido.clone(),
            (None, None) => self.username.clone(),
        }
    }

    /// Check if user can perform admin actions
    pub fn can_admin(&self) -> bool {
        self.activo && self.role.is_admin()
    }

    /// Check if user can perform superadmin actions
    pub fn can_superadmin(&self) -> bool {
        self.activo && self.role.is_superadmin()
    }

    /// Check if user is active and can login
    pub fn can_login(&self) -> bool {
        self.activo
    }
}

impl Default for User {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            username: String::new(),
            password_hash: String::new(),
            nombre: None,
            apellido: None,
            email: None,
            role: UserRole::Operator,
            activo: true,
            ultimo_login: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// User info for API responses (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i32,
    pub username: String,
    pub nombre: Option<String>,
    pub apellido: Option<String>,
    pub email: Option<String>,
    pub role: String,
    pub activo: bool,
    pub ultimo_login: Option<DateTime<Utc>>,
}

impl From<User> for UserInfo {
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
        }
    }
}

impl From<&User> for UserInfo {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        let operator = UserRole::Operator;
        let admin = UserRole::Admin;
        let superadmin = UserRole::Superadmin;

        assert!(admin.can_manage(&operator));
        assert!(superadmin.can_manage(&operator));
        assert!(superadmin.can_manage(&admin));
        assert!(!operator.can_manage(&admin));
        assert!(!admin.can_manage(&superadmin));
    }

    #[test]
    fn test_role_permissions() {
        assert!(!UserRole::Operator.is_admin());
        assert!(UserRole::Admin.is_admin());
        assert!(UserRole::Superadmin.is_admin());

        assert!(!UserRole::Operator.is_superadmin());
        assert!(!UserRole::Admin.is_superadmin());
        assert!(UserRole::Superadmin.is_superadmin());
    }

    #[test]
    fn test_user_full_name() {
        let user = User {
            nombre: Some("Juan".to_string()),
            apellido: Some("Perez".to_string()),
            username: "jperez".to_string(),
            ..Default::default()
        };
        assert_eq!(user.full_name(), "Juan Perez");

        let user2 = User {
            nombre: Some("Maria".to_string()),
            apellido: None,
            username: "maria".to_string(),
            ..Default::default()
        };
        assert_eq!(user2.full_name(), "Maria");

        let user3 = User {
            nombre: None,
            apellido: None,
            username: "admin".to_string(),
            ..Default::default()
        };
        assert_eq!(user3.full_name(), "admin");
    }

    #[test]
    fn test_user_can_login() {
        let active_user = User {
            activo: true,
            ..Default::default()
        };
        assert!(active_user.can_login());

        let inactive_user = User {
            activo: false,
            ..Default::default()
        };
        assert!(!inactive_user.can_login());
    }
}
