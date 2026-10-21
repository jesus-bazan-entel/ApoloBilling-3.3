//! SIP Endpoint DTOs
//!
//! Request and response types for the unified SIP Endpoint management
//! endpoints (extensions and trunks, credential- or IP-authenticated).

use super::carriers::Transport;
use apolo_core::models::{AuthMode, EndpointType, FreeswitchAllowedIp, SipEndpoint};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

fn default_sip_domain() -> String {
    "apolo.local".to_string()
}

fn default_codecs() -> String {
    "PCMU,PCMA,G729,opus".to_string()
}

fn default_max_registrations() -> i32 {
    3
}

fn default_max_concurrent_calls() -> i32 {
    2
}

fn default_true() -> bool {
    true
}

/// SIP Endpoint creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SipEndpointCreateRequest {
    #[validate(length(min = 1, max = 100, message = "Name must be 1-100 characters"))]
    pub name: String,

    pub description: Option<String>,

    pub endpoint_type: EndpointType,
    pub auth_mode: AuthMode,

    /// Optional billing account link
    pub account_id: Option<i32>,

    // auth_mode = credentials
    #[validate(length(min = 1, max = 64, message = "SIP username must be 1-64 characters"))]
    pub sip_username: Option<String>,
    #[serde(default = "default_sip_domain")]
    pub sip_domain: String,
    pub password: Option<String>,
    // Limite exterior: 1-150 (trunk). El limite por tipo (extension: 1-20)
    // se enforcea en el handler segun endpoint_type (ver validate_channel_limit).
    #[serde(default = "default_max_registrations")]
    #[validate(range(min = 1, max = 150, message = "Must be 1-150"))]
    pub max_registrations: i32,
    /// Llamadas simultaneas (canales). Lo enforcea el billing engine sobre
    /// eventos FreeSWITCH (ESL) para endpoints que bypassean Kamailio
    /// (extensiones). Limite por tipo en el handler (ver validate_call_limit).
    #[serde(default = "default_max_concurrent_calls")]
    #[validate(range(min = 1, max = 150, message = "Must be 1-150"))]
    pub max_concurrent_calls: i32,

    // auth_mode = ip
    pub host: Option<String>,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub is_default: Option<bool>,
    /// Rangos DID propiedad de la PBX (trunk/ip): prefijos separados por
    /// coma, ej "174020,174021". El L3 selectivo enruta cada prefijo SOLO aqui.
    pub did_ranges: Option<String>,

    #[validate(length(max = 100, message = "Display name max 100 characters"))]
    pub display_name: Option<String>,
    #[validate(length(max = 50, message = "Account code max 50 characters"))]
    pub accountcode: Option<String>,
    #[serde(default = "default_codecs")]
    pub codecs: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl SipEndpointCreateRequest {
    /// Convert to a SipEndpoint skeleton (without password fields - those
    /// are set by SipEndpointService once the password is known).
    pub fn to_endpoint(&self) -> SipEndpoint {
        let (context, fs_profile) = SipEndpoint::derive_context_and_profile(self.endpoint_type);
        let transport = self.transport.clone().unwrap_or_default();

        SipEndpoint {
            id: Uuid::nil(),
            name: self.name.clone(),
            description: self.description.clone(),
            endpoint_type: self.endpoint_type,
            auth_mode: self.auth_mode,
            account_id: self.account_id,
            sip_username: self.sip_username.clone(),
            sip_domain: Some(self.sip_domain.clone()),
            password_encrypted: None,
            password_nonce: None,
            a1_hash: None,
            max_registrations: self.max_registrations,
            max_concurrent_calls: self.max_concurrent_calls,
            host: self.host.clone(),
            port: self.port.unwrap_or(5060),
            transport: transport.to_string(),
            kamailio_gwid: None,
            sync_status: "pending".to_string(),
            sync_error: None,
            is_default: self.is_default.unwrap_or(false),
            did_ranges: self.did_ranges.clone().unwrap_or_default(),
            context: context.to_string(),
            fs_profile: fs_profile.to_string(),
            display_name: self.display_name.clone(),
            accountcode: self.accountcode.clone(),
            codecs: self.codecs.clone(),
            enabled: self.enabled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// SIP Endpoint update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SipEndpointUpdateRequest {
    #[validate(length(min = 1, max = 100, message = "Name must be 1-100 characters"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub account_id: Option<i32>,

    #[validate(length(min = 1, max = 64, message = "SIP username must be 1-64 characters"))]
    pub sip_username: Option<String>,
    pub sip_domain: Option<String>,
    // Limite exterior: 1-150. El limite por tipo se enforcea en el handler
    // contra el endpoint_type almacenado (extension: 1-20, trunk: 1-150).
    #[validate(range(min = 1, max = 150, message = "Must be 1-150"))]
    pub max_registrations: Option<i32>,
    // Llamadas simultaneas (ver campo en Create). Limite por tipo en handler.
    #[validate(range(min = 1, max = 150, message = "Must be 1-150"))]
    pub max_concurrent_calls: Option<i32>,

    pub host: Option<String>,
    pub port: Option<i32>,
    pub transport: Option<Transport>,
    pub is_default: Option<bool>,
    /// Rangos DID propiedad de la PBX (trunk/ip): prefijos separados por coma.
    pub did_ranges: Option<String>,

    #[validate(length(max = 100, message = "Display name max 100 characters"))]
    pub display_name: Option<String>,
    #[validate(length(max = 50, message = "Account code max 50 characters"))]
    pub accountcode: Option<String>,
    pub codecs: Option<String>,
    pub enabled: Option<bool>,
}

/// SIP Endpoint response (excludes sensitive password data)
#[derive(Debug, Clone, Serialize)]
pub struct SipEndpointResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub endpoint_type: EndpointType,
    pub auth_mode: AuthMode,
    pub account_id: Option<i32>,

    pub sip_username: Option<String>,
    pub sip_domain: Option<String>,
    pub aor: Option<String>,
    pub max_registrations: i32,
    pub max_concurrent_calls: i32,

    pub host: Option<String>,
    pub port: i32,
    pub transport: String,
    pub kamailio_gwid: Option<i32>,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub is_default: bool,
    pub did_ranges: String,

    pub display_name: Option<String>,
    pub accountcode: Option<String>,
    pub codecs: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SipEndpoint> for SipEndpointResponse {
    fn from(e: SipEndpoint) -> Self {
        let aor = e.aor();
        Self {
            id: e.id,
            name: e.name,
            description: e.description,
            endpoint_type: e.endpoint_type,
            auth_mode: e.auth_mode,
            account_id: e.account_id,
            sip_username: e.sip_username,
            sip_domain: e.sip_domain,
            aor,
            max_registrations: e.max_registrations,
            max_concurrent_calls: e.max_concurrent_calls,
            host: e.host,
            port: e.port,
            transport: e.transport,
            kamailio_gwid: e.kamailio_gwid,
            sync_status: e.sync_status,
            sync_error: e.sync_error,
            is_default: e.is_default,
            did_ranges: e.did_ranges,
            display_name: e.display_name,
            accountcode: e.accountcode,
            codecs: e.codecs,
            enabled: e.enabled,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

/// SIP Endpoint with password (for creation response or password reveal)
#[derive(Debug, Clone, Serialize)]
pub struct SipEndpointWithPasswordResponse {
    #[serde(flatten)]
    pub endpoint: SipEndpointResponse,
    pub password: String,
}

/// Password regeneration response
#[derive(Debug, Clone, Serialize)]
pub struct PasswordRegenerateResponse {
    pub id: Uuid,
    pub sip_username: String,
    pub sip_domain: String,
    pub new_password: String,
}

/// SIP Endpoint filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SipEndpointFilterParams {
    pub account_id: Option<i32>,
    pub enabled: Option<bool>,
}

/// Result of a Kamailio sync operation (auth_mode = ip only)
#[derive(Debug, Clone, Serialize)]
pub struct EndpointSyncResult {
    pub endpoint_id: Uuid,
    pub endpoint_name: String,
    pub kamailio_gwid: Option<i32>,
    pub success: bool,
    pub error: Option<String>,
}

/// FreeSWITCH allowed IP request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct FreeswitchIpRequest {
    #[validate(length(min = 1, max = 45, message = "Invalid IP address"))]
    pub ip_address: String,
    #[validate(length(max = 255, message = "Description max 255 characters"))]
    pub description: Option<String>,
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

/// SIP Registration status response from FreeSWITCH (auth_mode = credentials only)
#[derive(Debug, Clone, Serialize)]
pub struct SipRegistrationStatusResponse {
    pub registered: bool,
    pub sip_username: String,
    pub sip_domain: String,
    pub user_agent: Option<String>,
    pub contact: Option<String>,
    pub status: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub ping_status: Option<String>,
    pub expires_seconds: Option<i32>,
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_endpoint_credentials() {
        let req = SipEndpointCreateRequest {
            name: "1001".to_string(),
            description: None,
            endpoint_type: EndpointType::Extension,
            auth_mode: AuthMode::Credentials,
            account_id: Some(1),
            sip_username: Some("1001".to_string()),
            sip_domain: "test.com".to_string(),
            password: None,
            max_registrations: 2,
            max_concurrent_calls: 2,
            host: None,
            port: None,
            transport: None,
            is_default: None,
            did_ranges: None,
            display_name: Some("Test User".to_string()),
            accountcode: Some("ACC001".to_string()),
            codecs: "PCMU,PCMA".to_string(),
            enabled: true,
        };

        let endpoint = req.to_endpoint();

        assert_eq!(endpoint.account_id, Some(1));
        assert_eq!(endpoint.sip_username, Some("1001".to_string()));
        assert_eq!(endpoint.sip_domain, Some("test.com".to_string()));
        assert_eq!(endpoint.context, "from-pbx");
        assert_eq!(endpoint.fs_profile, "internal");
    }

    #[test]
    fn test_to_endpoint_ip_trunk() {
        let req = SipEndpointCreateRequest {
            name: "PBX Sucursal".to_string(),
            description: None,
            endpoint_type: EndpointType::Trunk,
            auth_mode: AuthMode::Ip,
            account_id: None,
            sip_username: None,
            sip_domain: default_sip_domain(),
            password: None,
            max_registrations: 3,
            max_concurrent_calls: 10,
            host: Some("10.0.0.5".to_string()),
            port: Some(5060),
            transport: Some(Transport::Udp),
            is_default: Some(true),
            did_ranges: Some("174020,174021".to_string()),
            display_name: None,
            accountcode: None,
            codecs: default_codecs(),
            enabled: true,
        };

        let endpoint = req.to_endpoint();

        assert_eq!(endpoint.host, Some("10.0.0.5".to_string()));
        assert_eq!(endpoint.did_ranges, "174020,174021");
        assert_eq!(endpoint.sip_username, None);
    }
}
