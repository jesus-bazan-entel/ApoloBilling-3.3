//! SIP Device DTOs
//!
//! Request and response types for SIP device management endpoints.

use apolo_core::models::{FreeswitchAllowedIp, SipDevice};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// SIP Device creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SipDeviceCreateRequest {
    /// Associated billing account ID
    pub account_id: i32,

    /// SIP username (extension or user identifier)
    #[validate(length(min = 1, max = 64, message = "SIP username must be 1-64 characters"))]
    pub sip_username: String,

    /// SIP domain (realm for authentication)
    #[serde(default = "default_sip_domain")]
    #[validate(length(min = 1, max = 255, message = "SIP domain must be 1-255 characters"))]
    pub sip_domain: String,

    /// Password (if not provided, one will be generated)
    pub password: Option<String>,

    /// Display name for caller ID
    #[validate(length(max = 100, message = "Display name max 100 characters"))]
    pub display_name: Option<String>,

    /// Device description
    pub description: Option<String>,

    /// FreeSWITCH dialplan context
    #[serde(default = "default_context")]
    #[validate(length(min = 1, max = 50, message = "Context must be 1-50 characters"))]
    pub context: String,

    /// Account code for CDR linking
    #[validate(length(max = 50, message = "Account code max 50 characters"))]
    pub accountcode: Option<String>,

    /// Allowed codecs (comma-separated)
    #[serde(default = "default_codecs")]
    pub codecs: String,

    /// Maximum simultaneous registrations
    #[serde(default = "default_max_registrations")]
    #[validate(range(min = 1, max = 10, message = "Max registrations must be 1-10"))]
    pub max_registrations: i32,

    /// Whether device is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_sip_domain() -> String {
    "apolo.local".to_string()
}

fn default_context() -> String {
    "from-pbx".to_string()
}

fn default_codecs() -> String {
    "PCMU,PCMA,G729,opus".to_string()
}

fn default_max_registrations() -> i32 {
    3
}

fn default_true() -> bool {
    true
}

impl SipDeviceCreateRequest {
    /// Convert to SipDevice entity (without password fields - those are set by service)
    pub fn to_device(&self) -> SipDevice {
        SipDevice {
            id: 0,
            account_id: self.account_id,
            sip_username: self.sip_username.clone(),
            sip_domain: self.sip_domain.clone(),
            password_encrypted: Vec::new(),
            password_nonce: Vec::new(),
            a1_hash: String::new(),
            display_name: self.display_name.clone(),
            description: self.description.clone(),
            context: self.context.clone(),
            accountcode: self.accountcode.clone(),
            codecs: self.codecs.clone(),
            max_registrations: self.max_registrations,
            enabled: self.enabled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// SIP Device update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SipDeviceUpdateRequest {
    /// Associated billing account ID
    pub account_id: Option<i32>,

    /// SIP username
    #[validate(length(min = 1, max = 64, message = "SIP username must be 1-64 characters"))]
    pub sip_username: Option<String>,

    /// SIP domain
    #[validate(length(min = 1, max = 255, message = "SIP domain must be 1-255 characters"))]
    pub sip_domain: Option<String>,

    /// Display name
    #[validate(length(max = 100, message = "Display name max 100 characters"))]
    pub display_name: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Context
    #[validate(length(min = 1, max = 50, message = "Context must be 1-50 characters"))]
    pub context: Option<String>,

    /// Account code
    #[validate(length(max = 50, message = "Account code max 50 characters"))]
    pub accountcode: Option<String>,

    /// Codecs
    pub codecs: Option<String>,

    /// Max registrations
    #[validate(range(min = 1, max = 10, message = "Max registrations must be 1-10"))]
    pub max_registrations: Option<i32>,

    /// Enabled status
    pub enabled: Option<bool>,
}

/// SIP Device response (excludes sensitive password data)
#[derive(Debug, Clone, Serialize)]
pub struct SipDeviceResponse {
    pub id: i32,
    pub account_id: i32,
    pub sip_username: String,
    pub sip_domain: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context: String,
    pub accountcode: Option<String>,
    pub codecs: String,
    pub max_registrations: i32,
    pub enabled: bool,
    pub aor: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SipDevice> for SipDeviceResponse {
    fn from(device: SipDevice) -> Self {
        let aor = device.aor();
        Self {
            id: device.id,
            account_id: device.account_id,
            sip_username: device.sip_username,
            sip_domain: device.sip_domain,
            display_name: device.display_name,
            description: device.description,
            context: device.context,
            accountcode: device.accountcode,
            codecs: device.codecs,
            max_registrations: device.max_registrations,
            enabled: device.enabled,
            aor,
            created_at: device.created_at,
            updated_at: device.updated_at,
        }
    }
}

impl From<&SipDevice> for SipDeviceResponse {
    fn from(device: &SipDevice) -> Self {
        let aor = device.aor();
        Self {
            id: device.id,
            account_id: device.account_id,
            sip_username: device.sip_username.clone(),
            sip_domain: device.sip_domain.clone(),
            display_name: device.display_name.clone(),
            description: device.description.clone(),
            context: device.context.clone(),
            accountcode: device.accountcode.clone(),
            codecs: device.codecs.clone(),
            max_registrations: device.max_registrations,
            enabled: device.enabled,
            aor,
            created_at: device.created_at,
            updated_at: device.updated_at,
        }
    }
}

/// SIP Device with password (for creation response or password reveal)
#[derive(Debug, Clone, Serialize)]
pub struct SipDeviceWithPasswordResponse {
    #[serde(flatten)]
    pub device: SipDeviceResponse,
    pub password: String,
}

/// Password regeneration response
#[derive(Debug, Clone, Serialize)]
pub struct PasswordRegenerateResponse {
    pub id: i32,
    pub sip_username: String,
    pub sip_domain: String,
    pub new_password: String,
}

/// SIP Device filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SipDeviceFilterParams {
    /// Filter by account ID
    pub account_id: Option<i32>,

    /// Filter by enabled status
    pub enabled: Option<bool>,

    /// Search by username
    pub search: Option<String>,
}

/// FreeSWITCH allowed IP request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct FreeswitchIpRequest {
    /// IP address (IPv4 or IPv6)
    #[validate(length(min = 1, max = 45, message = "Invalid IP address"))]
    pub ip_address: String,

    /// Description
    #[validate(length(max = 255, message = "Description max 255 characters"))]
    pub description: Option<String>,

    /// Whether IP is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl FreeswitchIpRequest {
    pub fn to_entity(&self) -> FreeswitchAllowedIp {
        FreeswitchAllowedIp {
            id: 0,
            ip_address: self.ip_address.clone(),
            description: self.description.clone(),
            enabled: self.enabled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// FreeSWITCH allowed IP response
#[derive(Debug, Clone, Serialize)]
pub struct FreeswitchIpResponse {
    pub id: i32,
    pub ip_address: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<FreeswitchAllowedIp> for FreeswitchIpResponse {
    fn from(ip: FreeswitchAllowedIp) -> Self {
        Self {
            id: ip.id,
            ip_address: ip.ip_address,
            description: ip.description,
            enabled: ip.enabled,
            created_at: ip.created_at,
            updated_at: ip.updated_at,
        }
    }
}

/// SIP Registration status response from FreeSWITCH
#[derive(Debug, Clone, Serialize)]
pub struct SipRegistrationStatusResponse {
    /// Whether the device is currently registered
    pub registered: bool,
    /// SIP username
    pub sip_username: String,
    /// SIP domain
    pub sip_domain: String,
    /// User agent string (softphone/device type)
    pub user_agent: Option<String>,
    /// Contact URI
    pub contact: Option<String>,
    /// Registration status text
    pub status: Option<String>,
    /// IP address of the registered device
    pub ip: Option<String>,
    /// Port of the registered device
    pub port: Option<u16>,
    /// Ping status (Reachable/Unreachable)
    pub ping_status: Option<String>,
    /// Expiration time in seconds
    pub expires_seconds: Option<i32>,
    /// Expiration datetime
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_request_defaults() {
        let json = r#"{
            "account_id": 1,
            "sip_username": "1001"
        }"#;

        let req: SipDeviceCreateRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.account_id, 1);
        assert_eq!(req.sip_username, "1001");
        assert_eq!(req.sip_domain, "apolo.local");
        assert_eq!(req.context, "from-pbx");
        assert_eq!(req.max_registrations, 3);
        assert!(req.enabled);
    }

    #[test]
    fn test_to_device() {
        let req = SipDeviceCreateRequest {
            account_id: 1,
            sip_username: "1001".to_string(),
            sip_domain: "test.com".to_string(),
            password: None,
            display_name: Some("Test User".to_string()),
            description: None,
            context: "from-pbx".to_string(),
            accountcode: Some("ACC001".to_string()),
            codecs: "PCMU,PCMA".to_string(),
            max_registrations: 2,
            enabled: true,
        };

        let device = req.to_device();

        assert_eq!(device.account_id, 1);
        assert_eq!(device.sip_username, "1001");
        assert_eq!(device.sip_domain, "test.com");
        assert_eq!(device.display_name, Some("Test User".to_string()));
        assert_eq!(device.accountcode, Some("ACC001".to_string()));
    }

    #[test]
    fn test_device_response() {
        let device = SipDevice {
            id: 1,
            account_id: 10,
            sip_username: "1001".to_string(),
            sip_domain: "apolo.local".to_string(),
            ..Default::default()
        };

        let response = SipDeviceResponse::from(device);

        assert_eq!(response.id, 1);
        assert_eq!(response.aor, "1001@apolo.local");
    }
}
