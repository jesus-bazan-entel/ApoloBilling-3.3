//! JWT token creation and validation service
//!
//! Provides secure JWT token generation and validation using the jsonwebtoken crate.

use crate::claims::Claims;
use apolo_core::error::AppError;
use apolo_core::models::UserRole;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use tracing::{debug, error, warn};

/// JWT Service for token creation and validation
///
/// Handles JWT token lifecycle including creation, validation, and expiration checks.
#[derive(Clone)]
pub struct JwtService {
    /// Secret key for signing tokens (kept for potential future use)
    #[allow(dead_code)]
    secret: String,

    /// Default token expiration time in seconds
    expiration_secs: i64,

    /// Encoding key (cached)
    encoding_key: EncodingKey,

    /// Decoding key (cached)
    decoding_key: DecodingKey,
}

impl JwtService {
    /// Create a new JWT service
    ///
    /// # Arguments
    ///
    /// * `secret` - The secret key used to sign tokens
    /// * `expiration_secs` - Default token expiration time in seconds
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::JwtService;
    ///
    /// let jwt_service = JwtService::new("my-secret-key", 3600);
    /// ```
    pub fn new(secret: &str, expiration_secs: i64) -> Self {
        Self {
            secret: secret.to_string(),
            expiration_secs,
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    /// Create a JWT token from claims
    ///
    /// # Arguments
    ///
    /// * `claims` - The claims to encode in the token
    ///
    /// # Errors
    ///
    /// Returns `AppError::InvalidToken` if token creation fails
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::{JwtService, Claims};
    /// use apolo_core::models::UserRole;
    ///
    /// let jwt_service = JwtService::new("secret", 3600);
    /// let claims = Claims::new("admin", UserRole::Admin);
    /// let token = jwt_service.create_token(&claims)?;
    /// # Ok::<(), apolo_core::error::AppError>(())
    /// ```
    pub fn create_token(&self, claims: &Claims) -> Result<String, AppError> {
        // Create mutable claims to set expiration
        let mut token_claims = claims.clone();

        // Set expiration if not already set
        if token_claims.exp == 0 {
            let exp = Utc::now() + Duration::seconds(self.expiration_secs);
            token_claims.exp = exp.timestamp();
        }

        debug!(
            username = %token_claims.sub,
            role = ?token_claims.role,
            exp = %token_claims.exp,
            "Creating JWT token"
        );

        encode(&Header::default(), &token_claims, &self.encoding_key).map_err(|e| {
            error!(error = %e, "Failed to create JWT token");
            AppError::InvalidToken(format!("Token creation failed: {}", e))
        })
    }

    /// Create a token for a user with username and role
    ///
    /// # Arguments
    ///
    /// * `username` - The username to include in the token
    /// * `role` - The user's role
    ///
    /// # Errors
    ///
    /// Returns `AppError::InvalidToken` if token creation fails
    pub fn create_token_for_user(
        &self,
        username: &str,
        role: UserRole,
    ) -> Result<String, AppError> {
        let claims = Claims::new(username, role);
        self.create_token(&claims)
    }

    /// Validate a JWT token and extract claims
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token to validate
    ///
    /// # Errors
    ///
    /// Returns:
    /// - `AppError::TokenExpired` if the token has expired
    /// - `AppError::InvalidToken` if the token is invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::{JwtService, Claims};
    /// use apolo_core::models::UserRole;
    ///
    /// let jwt_service = JwtService::new("secret", 3600);
    /// let claims = Claims::new("user", UserRole::Operator);
    /// let token = jwt_service.create_token(&claims)?;
    /// let decoded = jwt_service.validate_token(&token)?;
    /// assert_eq!(decoded.sub, "user");
    /// # Ok::<(), apolo_core::error::AppError>(())
    /// ```
    pub fn validate_token(&self, token: &str) -> Result<Claims, AppError> {
        let validation = Validation::default();

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            // Check if error is due to expiration
            if e.to_string().contains("ExpiredSignature") {
                warn!("Token expired");
                return AppError::TokenExpired;
            }

            warn!(error = %e, "Invalid token");
            AppError::InvalidToken(format!("Token validation failed: {}", e))
        })?;

        let claims = token_data.claims;

        // Additional expiration check (should be caught by validation, but be defensive)
        if claims.is_expired() {
            warn!(username = %claims.sub, "Token expired (manual check)");
            return Err(AppError::TokenExpired);
        }

        debug!(
            username = %claims.sub,
            role = ?claims.role,
            "Token validated successfully"
        );

        Ok(claims)
    }

    /// Extract username from a token without full validation
    ///
    /// WARNING: This does NOT validate the token signature or expiration.
    /// Only use for non-security-critical operations like logging.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token
    ///
    /// # Returns
    ///
    /// Returns the username if it can be extracted, None otherwise
    pub fn extract_username_unsafe(&self, token: &str) -> Option<String> {
        let validation = Validation::default();

        decode::<Claims>(token, &self.decoding_key, &validation)
            .ok()
            .map(|data| data.claims.sub)
    }

    /// Get the expiration time for tokens created by this service
    pub fn expiration_secs(&self) -> i64 {
        self.expiration_secs
    }
}

impl std::fmt::Debug for JwtService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtService")
            .field("expiration_secs", &self.expiration_secs)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-testing-12345";

    #[test]
    fn test_create_and_validate_token() {
        let jwt_service = JwtService::new(TEST_SECRET, 3600);
        let claims = Claims::new("testuser", UserRole::Admin);

        let token = jwt_service.create_token(&claims).unwrap();
        assert!(!token.is_empty());

        let decoded = jwt_service.validate_token(&token).unwrap();
        assert_eq!(decoded.sub, "testuser");
        assert_eq!(decoded.role, UserRole::Admin);
    }

    #[test]
    fn test_create_token_for_user() {
        let jwt_service = JwtService::new(TEST_SECRET, 3600);

        let token = jwt_service
            .create_token_for_user("admin", UserRole::Superadmin)
            .unwrap();

        let claims = jwt_service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, UserRole::Superadmin);
    }

    #[test]
    fn test_expired_token() {
        let jwt_service = JwtService::new(TEST_SECRET, 1);

        // Create token with 1 second expiration
        let claims = Claims::with_expiration("user", UserRole::Operator, -10);
        let token = jwt_service.create_token(&claims).unwrap();

        // Token should be expired
        let result = jwt_service.validate_token(&token);
        assert!(matches!(result, Err(AppError::TokenExpired)));
    }

    #[test]
    fn test_invalid_token() {
        let jwt_service = JwtService::new(TEST_SECRET, 3600);

        let result = jwt_service.validate_token("invalid.token.here");
        assert!(matches!(result, Err(AppError::InvalidToken(_))));
    }

    #[test]
    fn test_token_with_different_secret() {
        let jwt_service1 = JwtService::new("secret1", 3600);
        let jwt_service2 = JwtService::new("secret2", 3600);

        let claims = Claims::new("user", UserRole::Operator);
        let token = jwt_service1.create_token(&claims).unwrap();

        // Token created with secret1 should not validate with secret2
        let result = jwt_service2.validate_token(&token);
        assert!(matches!(result, Err(AppError::InvalidToken(_))));
    }

    #[test]
    fn test_extract_username_unsafe() {
        let jwt_service = JwtService::new(TEST_SECRET, 3600);
        let claims = Claims::new("extractuser", UserRole::Admin);
        let token = jwt_service.create_token(&claims).unwrap();

        let username = jwt_service.extract_username_unsafe(&token);
        assert_eq!(username, Some("extractuser".to_string()));

        let invalid_username = jwt_service.extract_username_unsafe("invalid.token");
        assert_eq!(invalid_username, None);
    }

    #[test]
    fn test_token_expiration_setting() {
        let jwt_service = JwtService::new(TEST_SECRET, 7200);
        let claims = Claims::new("user", UserRole::Operator);

        let token = jwt_service.create_token(&claims).unwrap();
        let decoded = jwt_service.validate_token(&token).unwrap();

        let now = Utc::now().timestamp();
        assert!(decoded.exp > now);
        assert!(decoded.exp <= now + 7200);
    }

    #[test]
    fn test_expiration_secs_getter() {
        let jwt_service = JwtService::new(TEST_SECRET, 1800);
        assert_eq!(jwt_service.expiration_secs(), 1800);
    }

    #[test]
    fn test_debug_impl_hides_secret() {
        let jwt_service = JwtService::new(TEST_SECRET, 3600);
        let debug_str = format!("{:?}", jwt_service);

        assert!(debug_str.contains("JwtService"));
        assert!(debug_str.contains("3600"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains(TEST_SECRET));
    }
}
