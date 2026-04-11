//! FreeSWITCH Directory Handler
//!
//! Provides mod_xml_curl endpoint for SIP device authentication.
//! FreeSWITCH queries this endpoint during REGISTER to lookup user credentials.

use actix_web::{web, HttpRequest, HttpResponse};
use apolo_core::traits::{FreeswitchIpRepository, SipDeviceRepository};
use apolo_core::AppError;
use apolo_db::{PgFreeswitchIpRepository, PgSipDeviceRepository};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{debug, info, warn, instrument};

/// FreeSWITCH mod_xml_curl request parameters
///
/// FreeSWITCH sends these as form-urlencoded POST data when querying for user directory.
/// Note: FreeSWITCH sends BOTH sip_auth_username AND user fields, so we accept them separately.
#[derive(Debug, Deserialize)]
pub struct FreeswitchDirectoryRequest {
    /// The SIP username being authenticated (primary)
    pub sip_auth_username: Option<String>,

    /// Alternative username field
    pub user: Option<String>,

    /// The SIP realm/domain (primary)
    pub sip_auth_realm: Option<String>,

    /// Alternative domain field
    pub domain: Option<String>,

    /// The action being performed (typically "sip_auth")
    pub action: Option<String>,

    /// The section being queried (typically "directory")
    pub section: Option<String>,

    /// The purpose (auth, registration, etc.)
    pub purpose: Option<String>,

    /// Tag name requested
    pub tag_name: Option<String>,

    /// Key name
    pub key_name: Option<String>,

    /// Key value
    pub key_value: Option<String>,
}

/// Handle FreeSWITCH mod_xml_curl directory lookup
///
/// POST /api/v1/freeswitch/directory
///
/// This endpoint is NOT authenticated via JWT - it uses IP whitelist instead.
/// FreeSWITCH sends form-urlencoded data with user credentials to look up.
#[instrument(skip(pool, req, http_req))]
pub async fn directory_lookup(
    pool: web::Data<PgPool>,
    req: web::Form<FreeswitchDirectoryRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    // Get client IP
    let client_ip = get_client_ip(&http_req);
    debug!(client_ip = %client_ip, "FreeSWITCH directory lookup request");

    // Verify IP is allowed
    let ip_repo = PgFreeswitchIpRepository::new(pool.get_ref().clone());
    if !ip_repo.is_ip_allowed(&client_ip).await? {
        warn!(
            client_ip = %client_ip,
            "FreeSWITCH directory lookup denied: IP not allowed"
        );
        return Ok(not_found_response());
    }

    // Extract username and realm (prefer sip_auth_* fields, fall back to user/domain)
    let username = req.sip_auth_username.as_ref()
        .filter(|u| !u.is_empty())
        .or(req.user.as_ref().filter(|u| !u.is_empty()))
        .cloned();

    let username = match username {
        Some(u) => u,
        None => {
            debug!("No username provided in request");
            return Ok(not_found_response());
        }
    };

    let realm = req.sip_auth_realm.as_ref()
        .filter(|r| !r.is_empty())
        .or(req.domain.as_ref().filter(|r| !r.is_empty()))
        .cloned()
        .unwrap_or_else(|| "apolo.local".to_string());

    debug!(
        username = %username,
        realm = %realm,
        action = ?req.action,
        purpose = ?req.purpose,
        "Looking up SIP device"
    );

    // Lookup device
    let device_repo = PgSipDeviceRepository::new(pool.get_ref().clone());
    let device = match device_repo.find_by_username_domain(&username, &realm).await? {
        Some(d) if d.enabled => d,
        Some(_) => {
            debug!(
                username = %username,
                realm = %realm,
                "Device found but disabled"
            );
            return Ok(not_found_response());
        }
        None => {
            debug!(
                username = %username,
                realm = %realm,
                "Device not found"
            );
            return Ok(not_found_response());
        }
    };

    info!(
        id = device.id,
        username = %device.sip_username,
        realm = %device.sip_domain,
        "FreeSWITCH directory lookup successful"
    );

    // Build XML response
    let xml = build_directory_xml(&device);

    Ok(HttpResponse::Ok()
        .content_type("text/xml; charset=utf-8")
        .body(xml))
}

/// Build FreeSWITCH directory XML response
fn build_directory_xml(device: &apolo_core::models::SipDevice) -> String {
    // Build effective caller ID
    let caller_id_name = device
        .display_name
        .as_deref()
        .unwrap_or(&device.sip_username);

    // Build accountcode (prefer explicit, fallback to account_id)
    let account_id_str = device.account_id.to_string();
    let accountcode = device
        .accountcode
        .as_deref()
        .unwrap_or(&account_id_str);

    // Build codecs (format for FreeSWITCH variable)
    let codecs = &device.codecs;

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<document type="freeswitch/xml">
  <section name="directory">
    <domain name="{domain}">
      <params>
        <param name="dial-string" value="{{^^:sip_invite_domain=${{dialed_domain}}:presence_id=${{dialed_user}}@${{dialed_domain}}}}${{sofia_contact(*/${{dialed_user}}@${{dialed_domain}})}}"/>
      </params>
      <groups>
        <group name="default">
          <users>
            <user id="{username}">
              <params>
                <param name="a1-hash" value="{a1_hash}"/>
                <param name="vm-password" value=""/>
              </params>
              <variables>
                <variable name="toll_allow" value="domestic,international,local"/>
                <variable name="accountcode" value="{accountcode}"/>
                <variable name="user_context" value="{context}"/>
                <variable name="effective_caller_id_name" value="{caller_id_name}"/>
                <variable name="effective_caller_id_number" value="{username}"/>
                <variable name="outbound_caller_id_name" value="{caller_id_name}"/>
                <variable name="outbound_caller_id_number" value="{username}"/>
                <variable name="callgroup" value="default"/>
                <variable name="absolute_codec_string" value="{codecs}"/>
                <variable name="max_registrations" value="{max_registrations}"/>
                <variable name="sip-force-expires" value="300"/>
              </variables>
            </user>
          </users>
        </group>
      </groups>
    </domain>
  </section>
</document>"#,
        domain = xml_escape(&device.sip_domain),
        username = xml_escape(&device.sip_username),
        a1_hash = xml_escape(&device.a1_hash),
        accountcode = xml_escape(accountcode),
        context = xml_escape(&device.context),
        caller_id_name = xml_escape(caller_id_name),
        codecs = xml_escape(codecs),
        max_registrations = device.max_registrations,
    )
}

/// Build not-found XML response
fn not_found_response() -> HttpResponse {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<document type="freeswitch/xml">
  <section name="result">
    <result status="not found"/>
  </section>
</document>"#;

    HttpResponse::Ok()
        .content_type("text/xml; charset=utf-8")
        .body(xml)
}

/// Extract client IP from request (handles proxies)
fn get_client_ip(req: &HttpRequest) -> String {
    // Check X-Forwarded-For header first (for reverse proxies)
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(s) = forwarded.to_str() {
            // X-Forwarded-For can be comma-separated; take the first IP
            if let Some(ip) = s.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Check X-Real-IP header (nginx)
    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(s) = real_ip.to_str() {
            return s.trim().to_string();
        }
    }

    // Fall back to connection info
    req.connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string()
}

/// Escape XML special characters
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Configure FreeSWITCH directory routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/freeswitch")
            .route("/directory", web::post().to(directory_lookup))
            // Also support GET for testing (though FreeSWITCH uses POST)
            .route("/directory", web::get().to(directory_lookup_get)),
    );
}

/// GET handler for testing (returns method not allowed hint)
async fn directory_lookup_get() -> HttpResponse {
    HttpResponse::MethodNotAllowed()
        .content_type("text/plain")
        .body("FreeSWITCH mod_xml_curl endpoint. Use POST with form data.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolo_core::models::SipDevice;

    #[test]
    fn test_build_directory_xml() {
        let device = SipDevice {
            id: 1,
            account_id: 100,
            sip_username: "1001".to_string(),
            sip_domain: "apolo.local".to_string(),
            a1_hash: "abc123".to_string(),
            display_name: Some("John Doe".to_string()),
            context: "from-pbx".to_string(),
            accountcode: Some("ACC-001".to_string()),
            codecs: "PCMU,PCMA".to_string(),
            max_registrations: 3,
            ..Default::default()
        };

        let xml = build_directory_xml(&device);

        assert!(xml.contains("1001"));
        assert!(xml.contains("apolo.local"));
        assert!(xml.contains("abc123"));
        assert!(xml.contains("John Doe"));
        assert!(xml.contains("ACC-001"));
        assert!(xml.contains("from-pbx"));
        assert!(xml.contains("PCMU,PCMA"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("test<>\"'&"), "test&lt;&gt;&quot;&apos;&amp;");
        assert_eq!(xml_escape("normal"), "normal");
    }

    #[test]
    fn test_not_found_response() {
        let response = not_found_response();
        // Just verify it returns a response (actual HTTP testing would need actix-rt)
        assert!(response.status().is_success());
    }
}
