//! SIP Endpoint Service
//!
//! Provides password encryption/decryption and A1 hash generation
//! for credential-authenticated SIP endpoints via FreeSWITCH mod_xml_curl.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use apolo_core::{models::SipEndpoint, AppError, AppResult};
use md5::{Digest, Md5};
use rand::Rng;
use std::sync::Arc;
use tracing::{debug, error, instrument};

/// SIP Endpoint Service for password management
pub struct SipEndpointService {
    /// AES-256-GCM cipher for password encryption
    cipher: Arc<Aes256Gcm>,
}

impl SipEndpointService {
    /// Create a new SipEndpointService
    ///
    /// # Arguments
    /// * `encryption_key` - 32-byte key for AES-256-GCM (64 hex characters)
    ///
    /// # Errors
    /// Returns error if the key is invalid
    pub fn new(encryption_key: &str) -> AppResult<Self> {
        let key_bytes = hex::decode(encryption_key).map_err(|e| {
            error!("Invalid SIP encryption key format: {}", e);
            AppError::Config(format!(
                "SIP_ENCRYPTION_KEY must be 64 hex characters: {}",
                e
            ))
        })?;

        if key_bytes.len() != 32 {
            return Err(AppError::Config(format!(
                "SIP_ENCRYPTION_KEY must be 32 bytes (64 hex chars), got {} bytes",
                key_bytes.len()
            )));
        }

        let key: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| AppError::Config("Failed to convert key to fixed array".to_string()))?;

        let cipher = Aes256Gcm::new(&key.into());

        Ok(Self {
            cipher: Arc::new(cipher),
        })
    }

    /// Generate a random SIP password
    ///
    /// Generates a 16-character alphanumeric password suitable for SIP devices.
    pub fn generate_password() -> String {
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
        let mut rng = rand::thread_rng();

        (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Encrypt a password using AES-256-GCM
    ///
    /// # Returns
    /// Tuple of (encrypted_bytes, nonce_bytes)
    #[instrument(skip(self, password))]
    pub fn encrypt_password(&self, password: &str) -> AppResult<(Vec<u8>, Vec<u8>)> {
        let mut rng = rand::thread_rng();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes);

        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, password.as_bytes())
            .map_err(|e| {
                error!("Password encryption failed: {}", e);
                AppError::Internal(format!("Encryption failed: {}", e))
            })?;

        debug!("Password encrypted successfully");
        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    /// Decrypt a password using AES-256-GCM
    #[instrument(skip(self, encrypted, nonce))]
    pub fn decrypt_password(&self, encrypted: &[u8], nonce: &[u8]) -> AppResult<String> {
        if nonce.len() != 12 {
            return Err(AppError::Internal("Invalid nonce length".to_string()));
        }

        let nonce = Nonce::from_slice(nonce);

        let plaintext = self.cipher.decrypt(nonce, encrypted).map_err(|e| {
            error!("Password decryption failed: {}", e);
            AppError::Internal(format!("Decryption failed: {}", e))
        })?;

        String::from_utf8(plaintext).map_err(|e| {
            error!("Invalid UTF-8 in decrypted password: {}", e);
            AppError::Internal(format!("Invalid password encoding: {}", e))
        })
    }

    /// Generate A1 hash for SIP digest authentication
    ///
    /// A1 = MD5(username:realm:password)
    ///
    /// This hash is used by FreeSWITCH for digest authentication.
    #[instrument(skip(password))]
    pub fn generate_a1_hash(username: &str, realm: &str, password: &str) -> String {
        let input = format!("{}:{}:{}", username, realm, password);
        let mut hasher = Md5::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Prepare a credential-authenticated endpoint with an encrypted password
    ///
    /// Sets password_encrypted, password_nonce, and a1_hash. Requires
    /// `endpoint.sip_username`/`sip_domain` to already be set.
    #[instrument(skip(self, endpoint, password))]
    pub fn prepare_device_with_password(
        &self,
        endpoint: &mut SipEndpoint,
        password: &str,
    ) -> AppResult<()> {
        let username = endpoint.sip_username.clone().ok_or_else(|| {
            AppError::Validation("sip_username is required for credential auth".to_string())
        })?;
        let domain = endpoint
            .sip_domain
            .clone()
            .unwrap_or_else(|| "apolo.local".to_string());

        let (encrypted, nonce) = self.encrypt_password(password)?;
        let a1_hash = Self::generate_a1_hash(&username, &domain, password);

        endpoint.password_encrypted = Some(encrypted);
        endpoint.password_nonce = Some(nonce);
        endpoint.a1_hash = Some(a1_hash);
        endpoint.sip_domain = Some(domain);

        debug!(
            username = %username,
            "Endpoint prepared with encrypted password"
        );

        Ok(())
    }

    /// Get the plaintext password for a credential-authenticated endpoint
    #[instrument(skip(self, endpoint))]
    pub fn get_device_password(&self, endpoint: &SipEndpoint) -> AppResult<String> {
        let encrypted = endpoint
            .password_encrypted
            .as_deref()
            .ok_or_else(|| AppError::Validation("Endpoint has no password set".to_string()))?;
        let nonce = endpoint
            .password_nonce
            .as_deref()
            .ok_or_else(|| AppError::Validation("Endpoint has no password set".to_string()))?;

        self.decrypt_password(encrypted, nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> SipEndpointService {
        // Test key: 32 bytes = 64 hex chars
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        SipEndpointService::new(key).unwrap()
    }

    #[test]
    fn test_generate_password() {
        let password = SipEndpointService::generate_password();
        assert_eq!(password.len(), 16);
        assert!(password.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_encrypt_decrypt_password() {
        let service = test_service();
        let original = "MySecretPassword123";

        let (encrypted, nonce) = service.encrypt_password(original).unwrap();
        let decrypted = service.decrypt_password(&encrypted, &nonce).unwrap();

        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_a1_hash() {
        // Test vector from RFC 2617
        // Note: This is a simplified test - actual RFC uses different values
        let hash = SipEndpointService::generate_a1_hash("1001", "apolo.local", "secret");

        // Verify it's a valid 32-char hex string (MD5 output)
        assert_eq!(hash.len(), 32);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_prepare_device() {
        let service = test_service();
        let mut endpoint = SipEndpoint {
            sip_username: Some("1001".to_string()),
            sip_domain: Some("apolo.local".to_string()),
            ..Default::default()
        };

        service
            .prepare_device_with_password(&mut endpoint, "test123")
            .unwrap();

        assert!(!endpoint.password_encrypted.as_ref().unwrap().is_empty());
        assert!(!endpoint.password_nonce.as_ref().unwrap().is_empty());
        assert_eq!(endpoint.a1_hash.as_ref().unwrap().len(), 32);

        // Verify we can decrypt
        let decrypted = service.get_device_password(&endpoint).unwrap();
        assert_eq!(decrypted, "test123");
    }

    #[test]
    fn test_invalid_key_length() {
        let result = SipEndpointService::new("short");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_format() {
        let result = SipEndpointService::new("not-hex-at-all-!!!!");
        assert!(result.is_err());
    }
}
