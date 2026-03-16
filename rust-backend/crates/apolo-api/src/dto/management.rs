//! Management DTOs
//!
//! Request and response types for zone, prefix, and tariff management.

use apolo_core::models::{NetworkType, Prefix, RateZone, Zone, ZoneType};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

// ============================================================================
// Zone DTOs
// ============================================================================

/// Zone creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ZoneCreateRequest {
    /// Zone name (required, unique)
    #[validate(length(min = 1, max = 100, message = "Zone name is required"))]
    pub zone_name: String,

    /// Zone code (optional short identifier)
    pub zone_code: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Zone type (GEOGRAPHIC, MOBILE, SPECIAL)
    #[serde(default)]
    pub zone_type: String,

    /// Region name for grouping
    pub region_name: Option<String>,

    /// Country ID (optional)
    pub country_id: Option<i32>,

    /// Whether zone is enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl ZoneCreateRequest {
    /// Convert to Zone entity
    pub fn to_zone(&self) -> Zone {
        let now = Utc::now();
        Zone {
            id: 0,
            country_id: self.country_id,
            zone_name: self.zone_name.clone(),
            zone_code: self.zone_code.clone(),
            description: self.description.clone(),
            zone_type: ZoneType::from_str(&self.zone_type).unwrap_or_default(),
            region_name: self.region_name.clone(),
            enabled: self.enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Zone update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ZoneUpdateRequest {
    /// Zone name
    pub zone_name: Option<String>,

    /// Zone code
    pub zone_code: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Zone type
    pub zone_type: Option<String>,

    /// Region name
    pub region_name: Option<String>,

    /// Enabled flag
    pub enabled: Option<bool>,
}

/// Zone response
#[derive(Debug, Clone, Serialize)]
pub struct ZoneResponse {
    /// Zone ID
    pub id: i32,

    /// Zone name
    pub zone_name: String,

    /// Zone code
    pub zone_code: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Zone type
    pub zone_type: String,

    /// Region name
    pub region_name: Option<String>,

    /// Country ID
    pub country_id: Option<i32>,

    /// Whether enabled
    pub enabled: bool,

    /// Prefix count (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_count: Option<i64>,

    /// Tariff count (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tariff_count: Option<i64>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Update timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<Zone> for ZoneResponse {
    fn from(zone: Zone) -> Self {
        Self {
            id: zone.id,
            zone_name: zone.zone_name,
            zone_code: zone.zone_code,
            description: zone.description,
            zone_type: zone.zone_type.to_string(),
            region_name: zone.region_name,
            country_id: zone.country_id,
            enabled: zone.enabled,
            prefix_count: None,
            tariff_count: None,
            created_at: zone.created_at,
            updated_at: zone.updated_at,
        }
    }
}

// ============================================================================
// Prefix DTOs
// ============================================================================

/// Prefix creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PrefixCreateRequest {
    /// Zone ID (required)
    pub zone_id: i32,

    /// Prefix string (required)
    #[validate(length(min = 1, max = 20, message = "Prefix is required"))]
    pub prefix: String,

    /// Operator name (optional)
    pub operator_name: Option<String>,

    /// Network type (FIXED, MOBILE)
    #[serde(default)]
    pub network_type: String,

    /// Whether enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl PrefixCreateRequest {
    /// Convert to Prefix entity
    pub fn to_prefix(&self) -> Prefix {
        let now = Utc::now();
        let prefix_len = self.prefix.len() as i32;
        Prefix {
            id: 0,
            zone_id: self.zone_id,
            prefix: self.prefix.clone(),
            prefix_length: prefix_len,
            operator_name: self.operator_name.clone(),
            network_type: parse_network_type(&self.network_type),
            enabled: self.enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

fn parse_network_type(s: &str) -> NetworkType {
    match s.to_uppercase().as_str() {
        "MOBILE" | "CELL" => NetworkType::Mobile,
        _ => NetworkType::Fixed,
    }
}

/// Prefix response
#[derive(Debug, Clone, Serialize)]
pub struct PrefixResponse {
    /// Prefix ID
    pub id: i32,

    /// Zone ID
    pub zone_id: i32,

    /// Zone name (if joined)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,

    /// Prefix string
    pub prefix: String,

    /// Prefix length
    pub prefix_length: i32,

    /// Operator name
    pub operator_name: Option<String>,

    /// Network type
    pub network_type: String,

    /// Whether enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Update timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<Prefix> for PrefixResponse {
    fn from(prefix: Prefix) -> Self {
        Self {
            id: prefix.id,
            zone_id: prefix.zone_id,
            zone_name: None,
            prefix: prefix.prefix,
            prefix_length: prefix.prefix_length,
            operator_name: prefix.operator_name,
            network_type: prefix.network_type.to_string(),
            enabled: prefix.enabled,
            created_at: prefix.created_at,
            updated_at: prefix.updated_at,
        }
    }
}

// ============================================================================
// Tariff (RateZone) DTOs
// ============================================================================

/// Tariff creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TariffCreateRequest {
    /// Zone ID (required)
    pub zone_id: i32,

    /// Rate name/description
    pub rate_name: Option<String>,

    /// Rate per minute (required)
    pub rate_per_minute: Decimal,

    /// Connection fee (default: 0)
    #[serde(default)]
    pub rate_per_call: Decimal,

    /// Billing increment in seconds (default: 6)
    #[serde(default = "default_billing_increment")]
    pub billing_increment: i32,

    /// Minimum duration in seconds (default: 0)
    #[serde(default)]
    pub min_duration: i32,

    /// Currency code (default: USD)
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Priority (default: 0)
    #[serde(default)]
    pub priority: i32,

    /// Whether enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_billing_increment() -> i32 {
    6
}

fn default_currency() -> String {
    "USD".to_string()
}

impl TariffCreateRequest {
    /// Convert to RateZone entity
    pub fn to_rate_zone(&self) -> RateZone {
        let now = Utc::now();
        RateZone {
            id: 0,
            zone_id: self.zone_id,
            rate_name: self.rate_name.clone(),
            rate_per_minute: self.rate_per_minute,
            rate_per_call: self.rate_per_call,
            billing_increment: self.billing_increment,
            min_duration: self.min_duration,
            effective_from: now,
            currency: self.currency.clone(),
            priority: self.priority,
            enabled: self.enabled,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Tariff update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TariffUpdateRequest {
    /// Rate name
    pub rate_name: Option<String>,

    /// Rate per minute
    pub rate_per_minute: Option<Decimal>,

    /// Connection fee
    pub rate_per_call: Option<Decimal>,

    /// Billing increment
    pub billing_increment: Option<i32>,

    /// Minimum duration
    pub min_duration: Option<i32>,

    /// Currency
    pub currency: Option<String>,

    /// Priority
    pub priority: Option<i32>,

    /// Enabled flag
    pub enabled: Option<bool>,
}

/// Tariff response
#[derive(Debug, Clone, Serialize)]
pub struct TariffResponse {
    /// Tariff ID
    pub id: i32,

    /// Zone ID
    pub zone_id: i32,

    /// Zone name (if joined)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,

    /// Rate name
    pub rate_name: Option<String>,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Rate per second (calculated)
    pub rate_per_second: Decimal,

    /// Connection fee
    pub rate_per_call: Decimal,

    /// Billing increment
    pub billing_increment: i32,

    /// Minimum duration
    pub min_duration: i32,

    /// Effective from
    pub effective_from: DateTime<Utc>,

    /// Currency
    pub currency: String,

    /// Priority
    pub priority: i32,

    /// Whether enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Update timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<RateZone> for TariffResponse {
    fn from(rate: RateZone) -> Self {
        let rate_per_second = rate.rate_per_second();
        Self {
            id: rate.id,
            zone_id: rate.zone_id,
            zone_name: None,
            rate_name: rate.rate_name,
            rate_per_minute: rate.rate_per_minute,
            rate_per_second,
            rate_per_call: rate.rate_per_call,
            billing_increment: rate.billing_increment,
            min_duration: rate.min_duration,
            effective_from: rate.effective_from,
            currency: rate.currency,
            priority: rate.priority,
            enabled: rate.enabled,
            created_at: rate.created_at,
            updated_at: rate.updated_at,
        }
    }
}

// ============================================================================
// Sync Response
// ============================================================================

/// Sync result response
#[derive(Debug, Clone, Serialize)]
pub struct SyncResponse {
    /// Whether sync was successful
    pub success: bool,

    /// Number of rate cards created/updated
    pub rate_cards_synced: i32,

    /// Number of rate cards deleted
    pub rate_cards_deleted: i32,

    /// Message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_zone_create_request_to_zone() {
        let req = ZoneCreateRequest {
            zone_name: "Peru Mobile".to_string(),
            zone_code: Some("PE-MOB".to_string()),
            description: Some("Peru mobile networks".to_string()),
            zone_type: "MOBILE".to_string(),
            region_name: Some("South America".to_string()),
            country_id: Some(51),
            enabled: true,
        };

        let zone = req.to_zone();
        assert_eq!(zone.zone_name, "Peru Mobile");
        assert_eq!(zone.zone_type, ZoneType::Mobile);
    }

    #[test]
    fn test_tariff_create_request_to_rate_zone() {
        let req = TariffCreateRequest {
            zone_id: 1,
            rate_name: Some("Standard".to_string()),
            rate_per_minute: dec!(0.10),
            rate_per_call: dec!(0.00),
            billing_increment: 6,
            min_duration: 0,
            currency: "USD".to_string(),
            priority: 0,
            enabled: true,
        };

        let rate = req.to_rate_zone();
        assert_eq!(rate.rate_per_minute, dec!(0.10));
        assert_eq!(rate.billing_increment, 6);
    }
}
