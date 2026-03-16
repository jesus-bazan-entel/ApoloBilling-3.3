//! Password hashing and verification using Argon2
//!
//! Provides secure password hashing using the Argon2id algorithm,
//! which is recommended for password hashing due to its resistance
//! to GPU cracking attacks and side-channel attacks.

use apolo_core::error::AppError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use tracing::{debug, error};

/// Password hashing service using Argon2
///
/// Uses Argon2id with default parameters for secure password hashing.
#[derive(Debug, Clone)]
pub struct PasswordService {
    argon2: Argon2<'static>,
}

impl PasswordService {
    /// Create a new password service with default Argon2 parameters
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::PasswordService;
    ///
    /// let password_service = PasswordService::new();
    /// ```
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Hash a password using Argon2
    ///
    /// # Arguments
    ///
    /// * `password` - The plaintext password to hash
    ///
    /// # Returns
    ///
    /// Returns the password hash as a PHC string format
    ///
    /// # Errors
    ///
    /// Returns `AppError::PasswordHash` if hashing fails
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::PasswordService;
    ///
    /// let password_service = PasswordService::new();
    /// let hash = password_service.hash_password("my_secure_password")?;
    /// # Ok::<(), apolo_core::error::AppError>(())
    /// ```
    pub fn hash_password(&self, password: &str) -> Result<String, AppError> {
        debug!("Hashing password");

        // Generate a random salt
        let salt = SaltString::generate(&mut OsRng);

        // Hash the password
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                error!(error = %e, "Failed to hash password");
                AppError::PasswordHash(format!("Password hashing failed: {}", e))
            })?;

        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    ///
    /// # Arguments
    ///
    /// * `password` - The plaintext password to verify
    /// * `hash` - The password hash to verify against
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the password matches the hash,
    /// `Ok(false)` if it doesn't match,
    /// or an error if verification fails
    ///
    /// # Errors
    ///
    /// Returns `AppError::PasswordHash` if the hash is invalid or verification fails
    ///
    /// # Examples
    ///
    /// ```
    /// use apolo_auth::PasswordService;
    ///
    /// let password_service = PasswordService::new();
    /// let hash = password_service.hash_password("my_password")?;
    ///
    /// let is_valid = password_service.verify_password("my_password", &hash)?;
    /// assert!(is_valid);
    ///
    /// let is_invalid = password_service.verify_password("wrong_password", &hash)?;
    /// assert!(!is_invalid);
    /// # Ok::<(), apolo_core::error::AppError>(())
    /// ```
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        debug!("Verifying password");

        // Parse the hash
        let parsed_hash = PasswordHash::new(hash).map_err(|e| {
            error!(error = %e, "Failed to parse password hash");
            AppError::PasswordHash(format!("Invalid password hash format: {}", e))
        })?;

        // Verify the password
        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(_) => {
                debug!("Password verification successful");
                Ok(true)
            }
            Err(argon2::password_hash::Error::Password) => {
                debug!("Password verification failed: incorrect password");
                Ok(false)
            }
            Err(e) => {
                error!(error = %e, "Password verification error");
                Err(AppError::PasswordHash(format!(
                    "Password verification failed: {}",
                    e
                )))
            }
        }
    }
}

impl Default for PasswordService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let service = PasswordService::new();
        let hash = service.hash_password("test_password").unwrap();

        // Hash should not be empty
        assert!(!hash.is_empty());

        // Hash should start with Argon2 identifier
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_correct_password() {
        let service = PasswordService::new();
        let password = "correct_password";
        let hash = service.hash_password(password).unwrap();

        let result = service.verify_password(password, &hash).unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_incorrect_password() {
        let service = PasswordService::new();
        let hash = service.hash_password("correct_password").unwrap();

        let result = service.verify_password("wrong_password", &hash).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let service = PasswordService::new();
        let password = "same_password";

        let hash1 = service.hash_password(password).unwrap();
        let hash2 = service.hash_password(password).unwrap();

        // Hashes should be different due to different salts
        assert_ne!(hash1, hash2);

        // But both should verify correctly
        assert!(service.verify_password(password, &hash1).unwrap());
        assert!(service.verify_password(password, &hash2).unwrap());
    }

    #[test]
    fn test_invalid_hash_format() {
        let service = PasswordService::new();
        let result = service.verify_password("password", "not_a_valid_hash");

        assert!(matches!(result, Err(AppError::PasswordHash(_))));
    }

    #[test]
    fn test_empty_password() {
        let service = PasswordService::new();
        let hash = service.hash_password("").unwrap();

        assert!(service.verify_password("", &hash).unwrap());
        assert!(!service.verify_password("not_empty", &hash).unwrap());
    }

    #[test]
    fn test_long_password() {
        let service = PasswordService::new();
        let long_password = "a".repeat(1000);
        let hash = service.hash_password(&long_password).unwrap();

        assert!(service.verify_password(&long_password, &hash).unwrap());
        assert!(!service.verify_password("short", &hash).unwrap());
    }

    #[test]
    fn test_special_characters() {
        let service = PasswordService::new();
        let password = "p@ssw0rd!#$%^&*()_+-=[]{}|;':\",./<>?";
        let hash = service.hash_password(password).unwrap();

        assert!(service.verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_unicode_password() {
        let service = PasswordService::new();
        let password = "パスワード🔐密码";
        let hash = service.hash_password(password).unwrap();

        assert!(service.verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_default_impl() {
        let service1 = PasswordService::new();
        let service2 = PasswordService::default();

        let password = "test";
        let hash = service1.hash_password(password).unwrap();

        // Both services should be able to verify the hash
        assert!(service2.verify_password(password, &hash).unwrap());
    }
}
