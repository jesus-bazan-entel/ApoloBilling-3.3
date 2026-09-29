//! JWT Claims structure
//!
//! Defines the claims structure used in JWT tokens for authentication.

use apolo_core::models::UserRole;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

/// JWT Claims
///
/// Standard claims used in JWT tokens for user authentication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// Subject (username)
    pub sub: String,

    /// User role
    pub role: UserRole,

    /// Effective module keys for this session (computed at login time from
    /// role + explicit grants — see `UserRole::effective_modules`). Empty on
    /// tokens issued before this field existed, thanks to `#[serde(default)]`.
    #[serde(default)]
    pub modules: Vec<String>,

    /// Issued at (Unix timestamp)
    pub iat: i64,

    /// Expiration time (Unix timestamp)
    pub exp: i64,
}

impl Claims {
    /// Create new claims with the specified username and role
    ///
    /// # Arguments
    ///
    /// * `username` - The username to include in the token
    /// * `role` - The user's role
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::Claims;
    /// use apolo_core::models::UserRole;
    ///
    /// let claims = Claims::new("admin", UserRole::Admin);
    /// assert_eq!(claims.sub, "admin");
    /// assert_eq!(claims.role, UserRole::Admin);
    /// ```
    pub fn new(username: &str, role: UserRole) -> Self {
        Self::new_with_modules(username, role, Vec::new())
    }

    /// Create new claims with an explicit effective-module list
    pub fn new_with_modules(username: &str, role: UserRole, modules: Vec<String>) -> Self {
        let now = Utc::now();

        Self {
            sub: username.to_string(),
            role,
            modules,
            iat: now.timestamp(),
            exp: 0, // Will be set by JwtService
        }
    }

    /// Create new claims with custom expiration duration
    ///
    /// # Arguments
    ///
    /// * `username` - The username to include in the token
    /// * `role` - The user's role
    /// * `expires_in_secs` - Token expiration time in seconds
    pub fn with_expiration(username: &str, role: UserRole, expires_in_secs: i64) -> Self {
        Self::with_expiration_and_modules(username, role, Vec::new(), expires_in_secs)
    }

    /// Create new claims with custom expiration duration and an explicit
    /// effective-module list
    pub fn with_expiration_and_modules(
        username: &str,
        role: UserRole,
        modules: Vec<String>,
        expires_in_secs: i64,
    ) -> Self {
        let now = Utc::now();
        let exp = now + Duration::seconds(expires_in_secs);

        Self {
            sub: username.to_string(),
            role,
            modules,
            iat: now.timestamp(),
            exp: exp.timestamp(),
        }
    }

    /// Check if the token is expired
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::Claims;
    /// use apolo_core::models::UserRole;
    ///
    /// let claims = Claims::with_expiration("user", UserRole::Usuario, 3600);
    /// assert!(!claims.is_expired());
    /// ```
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        self.exp <= now
    }

    /// Get the username from the claims
    pub fn username(&self) -> &str {
        &self.sub
    }

    /// Get the user role
    pub fn role(&self) -> UserRole {
        self.role
    }

    /// Check if user has admin privileges
    pub fn is_admin(&self) -> bool {
        self.role.is_admin()
    }

    /// Check if user has superadmin privileges
    pub fn is_superadmin(&self) -> bool {
        self.role.is_superadmin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_claims_creation() {
        let claims = Claims::new("testuser", UserRole::Usuario);
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.role, UserRole::Usuario);
        assert!(claims.iat > 0);
    }

    #[test]
    fn test_claims_with_expiration() {
        let claims = Claims::with_expiration("admin", UserRole::Admin, 3600);
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, UserRole::Admin);
        assert!(!claims.is_expired());

        let now = Utc::now().timestamp();
        assert!(claims.exp > now);
        assert!(claims.exp <= now + 3600);
    }

    #[test]
    fn test_expired_claims() {
        let mut claims = Claims::new("user", UserRole::Usuario);
        claims.exp = (Utc::now() - Duration::hours(1)).timestamp();
        assert!(claims.is_expired());
    }

    #[test]
    fn test_role_checks() {
        let operator_claims = Claims::new("operator", UserRole::Usuario);
        assert!(!operator_claims.is_admin());
        assert!(!operator_claims.is_superadmin());

        let admin_claims = Claims::new("admin", UserRole::Admin);
        assert!(admin_claims.is_admin());
        assert!(!admin_claims.is_superadmin());

        let superadmin_claims = Claims::new("superadmin", UserRole::Superadmin);
        assert!(superadmin_claims.is_admin());
        assert!(superadmin_claims.is_superadmin());
    }

    #[test]
    fn test_username_getter() {
        let claims = Claims::new("myuser", UserRole::Usuario);
        assert_eq!(claims.username(), "myuser");
    }

    #[test]
    fn test_role_getter() {
        let claims = Claims::new("user", UserRole::Admin);
        assert_eq!(claims.role(), UserRole::Admin);
    }
}
