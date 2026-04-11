//! SIP Device model
//!
//! Represents SIP endpoints (phones, softphones) that authenticate
//! via FreeSWITCH mod_xml_curl for registration and call routing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// SIP Device entity
///
/// Represents a SIP endpoint registered in the system.
/// Devices are associated with billing accounts and authenticate
/// using digest authentication via mod_xml_curl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipDevice {
    /// Unique identifier
    pub id: i32,

    /// Associated billing account ID
    pub account_id: i32,

    /// SIP username (extension or user identifier)
    pub sip_username: String,

    /// SIP domain (realm for authentication)
    pub sip_domain: String,

    /// Encrypted password (AES-256-GCM)
    #[serde(skip_serializing)]
    pub password_encrypted: Vec<u8>,

    /// Nonce used for AES-256-GCM encryption
    #[serde(skip_serializing)]
    pub password_nonce: Vec<u8>,

    /// A1 hash for SIP digest authentication: MD5(username:realm:password)
    #[serde(skip_serializing)]
    pub a1_hash: String,

    /// Display name for caller ID
    pub display_name: Option<String>,

    /// Device description
    pub description: Option<String>,

    /// FreeSWITCH dialplan context
    pub context: String,

    /// Account code for CDR linking (matches accounts.account_number)
    pub accountcode: Option<String>,

    /// Allowed codecs (comma-separated)
    pub codecs: String,

    /// Maximum simultaneous registrations
    pub max_registrations: i32,

    /// Whether device is enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl SipDevice {
    /// Check if device can register
    pub fn can_register(&self) -> bool {
        self.enabled
    }

    /// Get the SIP AOR (Address of Record)
    pub fn aor(&self) -> String {
        format!("{}@{}", self.sip_username, self.sip_domain)
    }

    /// Get codecs as a vector
    pub fn codec_list(&self) -> Vec<&str> {
        self.codecs.split(',').map(|s| s.trim()).collect()
    }

    /// Normalize SIP username (lowercase, remove special chars except _-)
    pub fn normalize_username(username: &str) -> String {
        username
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    }
}

impl Default for SipDevice {
    fn default() -> Self {
        Self {
            id: 0,
            account_id: 0,
            sip_username: String::new(),
            sip_domain: "apolo.local".to_string(),
            password_encrypted: Vec::new(),
            password_nonce: Vec::new(),
            a1_hash: String::new(),
            display_name: None,
            description: None,
            context: "from-pbx".to_string(),
            accountcode: None,
            codecs: "PCMU,PCMA,G729,opus".to_string(),
            max_registrations: 3,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// FreeSWITCH allowed IP for mod_xml_curl
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeswitchAllowedIp {
    /// Unique identifier
    pub id: i32,

    /// IP address (IPv4 or IPv6)
    pub ip_address: String,

    /// Description
    pub description: Option<String>,

    /// Whether IP is enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl FreeswitchAllowedIp {
    /// Check if this IP matches the given address
    pub fn matches(&self, ip: &str) -> bool {
        self.enabled && self.ip_address == ip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sip_device_aor() {
        let device = SipDevice {
            sip_username: "1001".to_string(),
            sip_domain: "apolo.local".to_string(),
            ..Default::default()
        };

        assert_eq!(device.aor(), "1001@apolo.local");
    }

    #[test]
    fn test_sip_device_codec_list() {
        let device = SipDevice {
            codecs: "PCMU, PCMA, G729".to_string(),
            ..Default::default()
        };

        let codecs = device.codec_list();
        assert_eq!(codecs.len(), 3);
        assert_eq!(codecs[0], "PCMU");
        assert_eq!(codecs[1], "PCMA");
        assert_eq!(codecs[2], "G729");
    }

    #[test]
    fn test_normalize_username() {
        assert_eq!(SipDevice::normalize_username("User-123_test"), "user-123_test");
        assert_eq!(SipDevice::normalize_username("user@domain.com"), "userdomaincom");
        assert_eq!(SipDevice::normalize_username("1001"), "1001");
    }

    #[test]
    fn test_can_register() {
        let mut device = SipDevice::default();
        assert!(device.can_register());

        device.enabled = false;
        assert!(!device.can_register());
    }
}
