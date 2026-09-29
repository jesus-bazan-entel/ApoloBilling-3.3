//! User model
//!
//! Represents system users for authentication and authorization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::module::SystemModule;

/// User role enumeration
///
/// Three roles with real business meaning:
/// - `Usuario`: staff from other areas of the client company. Sees only the
///   modules explicitly granted to them (see [`UserRole::effective_modules`]).
/// - `Admin`: the client company's administrator. Full access to all
///   operational/business modules; creates and manages `Usuario` accounts.
/// - `Superadmin`: factory support (the vendor). Full access, including
///   infrastructure-core / advanced-system modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Staff from other areas of the client, with module visibility
    /// controlled by Admin/Superadmin.
    #[default]
    Usuario,
    /// Administrator with full access to operational/business modules.
    Admin,
    /// Super administrator (factory support) with system-wide access.
    Superadmin,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::Usuario => write!(f, "usuario"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Superadmin => write!(f, "superadmin"),
        }
    }
}

impl UserRole {
    /// Parse from string (case-insensitive)
    ///
    /// Accepts the legacy `"operator"` value as a deprecated alias for
    /// `Usuario`, kept for one release while old JWTs/clients roll over.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "usuario" | "operator" => Some(UserRole::Usuario),
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
            UserRole::Usuario => 1,
            UserRole::Admin => 2,
            UserRole::Superadmin => 3,
        }
    }

    /// Check if this role can manage another role
    pub fn can_manage(&self, other: &UserRole) -> bool {
        self.level() > other.level()
    }

    /// Roles this role is allowed to *assign* when creating a new account.
    ///
    /// This is intentionally distinct from [`UserRole::can_manage`]: a
    /// Superadmin can create another Superadmin (factory support may need a
    /// backup account), which `can_manage` (a strict `>` on level) does not
    /// allow. An Admin may only ever create `Usuario` accounts.
    pub fn assignable_roles(&self) -> &'static [UserRole] {
        match self {
            UserRole::Superadmin => &[UserRole::Usuario, UserRole::Admin, UserRole::Superadmin],
            UserRole::Admin => &[UserRole::Usuario],
            UserRole::Usuario => &[],
        }
    }

    /// Whether this role can assign the given role to a (new or existing) account.
    pub fn can_assign(&self, role: &UserRole) -> bool {
        self.assignable_roles().contains(role)
    }

    /// Compute the set of module keys this session should actually see.
    ///
    /// - `Admin`/`Superadmin`: every active module in the catalog whose
    ///   `min_role` is at or below this role (Superadmin also sees
    ///   `min_role = superadmin` modules).
    /// - `Usuario`: modules marked `always_visible` (e.g. the dashboard),
    ///   plus any `min_role = usuario` module explicitly present in
    ///   `granted` (grants for modules above `usuario` are ignored
    ///   defensively, in case the catalog changed after the grant).
    pub fn effective_modules(&self, granted: &[String], catalog: &[SystemModule]) -> Vec<String> {
        use std::collections::BTreeSet;

        let mut result: BTreeSet<String> = BTreeSet::new();

        match self {
            UserRole::Admin | UserRole::Superadmin => {
                for module in catalog.iter().filter(|m| m.activo) {
                    if module.min_role.level() <= self.level() {
                        result.insert(module.module_key.clone());
                    }
                }
            }
            UserRole::Usuario => {
                for module in catalog.iter().filter(|m| m.activo && m.always_visible) {
                    result.insert(module.module_key.clone());
                }

                let usuario_eligible: BTreeSet<&str> = catalog
                    .iter()
                    .filter(|m| m.activo && m.min_role == UserRole::Usuario)
                    .map(|m| m.module_key.as_str())
                    .collect();

                for key in granted {
                    if usuario_eligible.contains(key.as_str()) {
                        result.insert(key.clone());
                    }
                }
            }
        }

        result.into_iter().collect()
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
            role: UserRole::Usuario,
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
    use crate::models::module::SystemModule;

    fn catalog() -> Vec<SystemModule> {
        vec![
            SystemModule {
                id: 1,
                module_key: "dashboard".to_string(),
                nombre: "Panel de Control".to_string(),
                seccion: "Principal".to_string(),
                min_role: UserRole::Usuario,
                always_visible: true,
                orden: 10,
                activo: true,
            },
            SystemModule {
                id: 2,
                module_key: "cdr".to_string(),
                nombre: "Registros CDR".to_string(),
                seccion: "Principal".to_string(),
                min_role: UserRole::Usuario,
                always_visible: false,
                orden: 20,
                activo: true,
            },
            SystemModule {
                id: 3,
                module_key: "users".to_string(),
                nombre: "Usuarios y Permisos".to_string(),
                seccion: "Sistema".to_string(),
                min_role: UserRole::Admin,
                always_visible: false,
                orden: 30,
                activo: true,
            },
            SystemModule {
                id: 4,
                module_key: "dialplan".to_string(),
                nombre: "Dialplan".to_string(),
                seccion: "Sistema Avanzado".to_string(),
                min_role: UserRole::Superadmin,
                always_visible: false,
                orden: 40,
                activo: true,
            },
        ]
    }

    #[test]
    fn test_role_hierarchy() {
        let usuario = UserRole::Usuario;
        let admin = UserRole::Admin;
        let superadmin = UserRole::Superadmin;

        assert!(admin.can_manage(&usuario));
        assert!(superadmin.can_manage(&usuario));
        assert!(superadmin.can_manage(&admin));
        assert!(!usuario.can_manage(&admin));
        assert!(!admin.can_manage(&superadmin));
    }

    #[test]
    fn test_role_permissions() {
        assert!(!UserRole::Usuario.is_admin());
        assert!(UserRole::Admin.is_admin());
        assert!(UserRole::Superadmin.is_admin());

        assert!(!UserRole::Usuario.is_superadmin());
        assert!(!UserRole::Admin.is_superadmin());
        assert!(UserRole::Superadmin.is_superadmin());
    }

    #[test]
    fn test_operator_alias_parses_as_usuario() {
        assert_eq!(UserRole::from_str("operator"), Some(UserRole::Usuario));
        assert_eq!(UserRole::from_str("usuario"), Some(UserRole::Usuario));
        assert_eq!(UserRole::from_str("USUARIO"), Some(UserRole::Usuario));
    }

    #[test]
    fn test_assignable_roles() {
        assert!(UserRole::Superadmin.can_assign(&UserRole::Superadmin));
        assert!(UserRole::Superadmin.can_assign(&UserRole::Admin));
        assert!(UserRole::Superadmin.can_assign(&UserRole::Usuario));

        assert!(UserRole::Admin.can_assign(&UserRole::Usuario));
        assert!(!UserRole::Admin.can_assign(&UserRole::Admin));
        assert!(!UserRole::Admin.can_assign(&UserRole::Superadmin));

        assert!(!UserRole::Usuario.can_assign(&UserRole::Usuario));
    }

    #[test]
    fn test_effective_modules_admin_sees_usuario_and_admin_modules_only() {
        let modules = UserRole::Admin.effective_modules(&[], &catalog());
        assert!(modules.contains(&"dashboard".to_string()));
        assert!(modules.contains(&"cdr".to_string()));
        assert!(modules.contains(&"users".to_string()));
        assert!(!modules.contains(&"dialplan".to_string()));
    }

    #[test]
    fn test_effective_modules_superadmin_sees_everything() {
        let modules = UserRole::Superadmin.effective_modules(&[], &catalog());
        assert_eq!(modules.len(), 4);
        assert!(modules.contains(&"dialplan".to_string()));
    }

    #[test]
    fn test_effective_modules_usuario_needs_explicit_grant() {
        let no_grants = UserRole::Usuario.effective_modules(&[], &catalog());
        assert_eq!(no_grants, vec!["dashboard".to_string()]);

        let with_grant = UserRole::Usuario.effective_modules(&["cdr".to_string()], &catalog());
        assert!(with_grant.contains(&"dashboard".to_string()));
        assert!(with_grant.contains(&"cdr".to_string()));
    }

    #[test]
    fn test_effective_modules_usuario_cannot_be_granted_admin_or_superadmin_modules() {
        let modules = UserRole::Usuario
            .effective_modules(&["users".to_string(), "dialplan".to_string()], &catalog());
        assert_eq!(modules, vec!["dashboard".to_string()]);
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
