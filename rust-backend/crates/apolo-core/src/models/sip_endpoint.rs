//! SIP Endpoint model
//!
//! Unified representation of anything that talks SIP to this platform: a
//! single extension/softphone or a whole PBX/trunk, authenticated either by
//! SIP digest credentials (REGISTER) or by trusted source IP. See
//! `EndpointType` and `AuthMode` for the two orthogonal axes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Whether this endpoint represents a single device or a whole PBX/trunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointType {
    /// A single subscriber device (softphone, desk phone).
    Extension,
    /// A PBX or trunk that may carry many calls/extensions behind it.
    Trunk,
}

impl fmt::Display for EndpointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EndpointType::Extension => "extension",
            EndpointType::Trunk => "trunk",
        })
    }
}

impl std::str::FromStr for EndpointType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "extension" => Ok(EndpointType::Extension),
            "trunk" => Ok(EndpointType::Trunk),
            other => Err(format!("Invalid endpoint_type: {}", other)),
        }
    }
}

/// How this endpoint authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// SIP digest authentication via REGISTER (username/password, A1 hash).
    Credentials,
    /// Trusted by source IP, no registration (Kamailio ACL).
    Ip,
}

impl fmt::Display for AuthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AuthMode::Credentials => "credentials",
            AuthMode::Ip => "ip",
        })
    }
}

impl std::str::FromStr for AuthMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "credentials" => Ok(AuthMode::Credentials),
            "ip" => Ok(AuthMode::Ip),
            other => Err(format!("Invalid auth_mode: {}", other)),
        }
    }
}

/// SIP Endpoint entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipEndpoint {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,

    pub endpoint_type: EndpointType,
    pub auth_mode: AuthMode,

    pub account_id: Option<i32>,

    // auth_mode = Credentials
    pub sip_username: Option<String>,
    pub sip_domain: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<Vec<u8>>,
    #[serde(skip_serializing)]
    pub password_nonce: Option<Vec<u8>>,
    #[serde(skip_serializing)]
    pub a1_hash: Option<String>,
    pub max_registrations: i32,
    /// Limite de llamadas simultaneas (canales) para endpoints que no pasan
    /// por el enforcement de Kamailio (extensiones: RELAY_LANEXT bypass).
    /// Lo enforcea el billing engine (ESL CHANNEL_CREATE vs active_calls).
    /// Extension: 1-20. Trunk: 1-150 (troncal IP se enforcea en Kamailio
    /// via max_registrations; este campo queda reservado).
    pub max_concurrent_calls: i32,

    // auth_mode = Ip
    pub host: Option<String>,
    pub port: i32,
    pub transport: String,
    pub kamailio_gwid: Option<i32>,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub is_default: bool,
    /// Rangos DID propiedad de esta PBX (trunk/ip): prefijos separados por
    /// coma, ej "174020,174021". El L3 selectivo enruta cada prefijo SOLO aqui.
    pub did_ranges: String,
    /// Numero principal/cabecera (trunk/ip): reemplaza el From cuando la PBX
    /// manda "Anonymous" o nada. Sincronizado a Kamailio address.tag.
    pub default_caller_id: Option<String>,

    // Derived server-side, never set directly from the API
    pub context: String,
    pub fs_profile: String,

    pub display_name: Option<String>,
    pub accountcode: Option<String>,
    pub codecs: String,
    pub enabled: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SipEndpoint {
    /// Get the SIP AOR (Address of Record), only meaningful for auth_mode=Credentials.
    pub fn aor(&self) -> Option<String> {
        match (&self.sip_username, &self.sip_domain) {
            (Some(u), Some(d)) => Some(format!("{}@{}", u, d)),
            _ => None,
        }
    }

    /// Get codecs as a vector
    pub fn codec_list(&self) -> Vec<&str> {
        self.codecs.split(',').map(|s| s.trim()).collect()
    }

    /// Check if endpoint can register/be used
    pub fn can_register(&self) -> bool {
        self.enabled
    }

    /// Normalize SIP username (lowercase, remove special chars except _-)
    pub fn normalize_username(username: &str) -> String {
        username
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    }

    /// The FreeSWITCH dialplan context/profile this endpoint should use.
    /// Both endpoint types use the same internal profile today; this is
    /// centralized here so the two can diverge later without an API change.
    pub fn derive_context_and_profile(
        _endpoint_type: EndpointType,
    ) -> (&'static str, &'static str) {
        ("from-pbx", "internal")
    }
}

impl Default for SipEndpoint {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            name: String::new(),
            description: None,
            endpoint_type: EndpointType::Extension,
            auth_mode: AuthMode::Credentials,
            account_id: None,
            sip_username: None,
            sip_domain: Some("apolo.local".to_string()),
            password_encrypted: None,
            password_nonce: None,
            a1_hash: None,
            max_registrations: 3,
            max_concurrent_calls: 2,
            host: None,
            port: 5060,
            transport: "udp".to_string(),
            kamailio_gwid: None,
            sync_status: "pending".to_string(),
            sync_error: None,
            is_default: false,
            did_ranges: String::new(),
            default_caller_id: None,
            context: "from-pbx".to_string(),
            fs_profile: "internal".to_string(),
            display_name: None,
            accountcode: None,
            codecs: "PCMU,PCMA,G729,opus".to_string(),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// FreeSWITCH allowed IP for mod_xml_curl (whitelists which FreeSWITCH boxes
/// may query the directory endpoint; unrelated to a given endpoint's own
/// `auth_mode`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeswitchAllowedIp {
    pub id: i32,
    pub ip_address: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FreeswitchAllowedIp {
    pub fn matches(&self, ip: &str) -> bool {
        self.enabled && self.ip_address == ip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_endpoint() -> SipEndpoint {
        SipEndpoint {
            id: Uuid::nil(),
            name: "test".to_string(),
            description: None,
            endpoint_type: EndpointType::Extension,
            auth_mode: AuthMode::Credentials,
            account_id: None,
            sip_username: None,
            sip_domain: None,
            password_encrypted: None,
            password_nonce: None,
            a1_hash: None,
            max_registrations: 3,
            max_concurrent_calls: 2,
            host: None,
            port: 5060,
            transport: "udp".to_string(),
            kamailio_gwid: None,
            sync_status: "pending".to_string(),
            sync_error: None,
            is_default: false,
            did_ranges: String::new(),
            default_caller_id: None,
            context: "from-pbx".to_string(),
            fs_profile: "internal".to_string(),
            display_name: None,
            accountcode: None,
            codecs: "PCMU,PCMA,G729,opus".to_string(),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_aor_requires_credentials() {
        let mut e = base_endpoint();
        assert_eq!(e.aor(), None);

        e.sip_username = Some("1001".to_string());
        e.sip_domain = Some("apolo.local".to_string());
        assert_eq!(e.aor(), Some("1001@apolo.local".to_string()));
    }

    #[test]
    fn test_codec_list() {
        let mut e = base_endpoint();
        e.codecs = "PCMU, PCMA, G729".to_string();
        let codecs = e.codec_list();
        assert_eq!(codecs, vec!["PCMU", "PCMA", "G729"]);
    }

    #[test]
    fn test_normalize_username() {
        assert_eq!(
            SipEndpoint::normalize_username("User-123_test"),
            "user-123_test"
        );
        assert_eq!(SipEndpoint::normalize_username("1001"), "1001");
    }

    #[test]
    fn test_endpoint_type_roundtrip() {
        assert_eq!(
            "extension".parse::<EndpointType>().unwrap(),
            EndpointType::Extension
        );
        assert_eq!(
            "trunk".parse::<EndpointType>().unwrap(),
            EndpointType::Trunk
        );
        assert_eq!(EndpointType::Extension.to_string(), "extension");
        assert!("bogus".parse::<EndpointType>().is_err());
    }

    #[test]
    fn test_auth_mode_roundtrip() {
        assert_eq!(
            "credentials".parse::<AuthMode>().unwrap(),
            AuthMode::Credentials
        );
        assert_eq!("ip".parse::<AuthMode>().unwrap(), AuthMode::Ip);
        assert_eq!(AuthMode::Ip.to_string(), "ip");
        assert!("bogus".parse::<AuthMode>().is_err());
    }
}
