//! Zone and prefix models
//!
//! Represents geographic zones and their associated prefixes and rates.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Zone type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum ZoneType {
    /// Geographic/landline zone
    #[default]
    Geographic,
    /// Mobile network zone
    Mobile,
    /// Special services (toll-free, premium, etc.)
    Special,
}

impl fmt::Display for ZoneType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneType::Geographic => write!(f, "GEOGRAPHIC"),
            ZoneType::Mobile => write!(f, "MOBILE"),
            ZoneType::Special => write!(f, "SPECIAL"),
        }
    }
}

impl ZoneType {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GEOGRAPHIC" | "GEO" | "LANDLINE" | "FIXED" => Some(ZoneType::Geographic),
            "MOBILE" | "CELL" | "CELLULAR" => Some(ZoneType::Mobile),
            "SPECIAL" | "PREMIUM" | "TOLLFREE" => Some(ZoneType::Special),
            _ => None,
        }
    }
}

/// Zone entity
///
/// Represents a geographic or service zone for rate grouping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    /// Unique identifier
    pub id: i32,

    /// Country ID (optional, for international zones)
    pub country_id: Option<i32>,

    /// Zone name (unique identifier)
    pub zone_name: String,

    /// Zone code (short identifier)
    pub zone_code: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Zone type
    pub zone_type: ZoneType,

    /// Region name (for grouping)
    pub region_name: Option<String>,

    /// Whether zone is enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Default for Zone {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            country_id: None,
            zone_name: String::new(),
            zone_code: None,
            description: None,
            zone_type: ZoneType::Geographic,
            region_name: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Network type for prefixes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum NetworkType {
    /// Fixed/landline network
    #[default]
    Fixed,
    /// Mobile network
    Mobile,
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkType::Fixed => write!(f, "FIXED"),
            NetworkType::Mobile => write!(f, "MOBILE"),
        }
    }
}

/// Prefix entity
///
/// Maps phone number prefixes to zones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefix {
    /// Unique identifier
    pub id: i32,

    /// Associated zone ID
    pub zone_id: i32,

    /// Phone number prefix
    pub prefix: String,

    /// Length of the prefix
    pub prefix_length: i32,

    /// Operator/carrier name
    pub operator_name: Option<String>,

    /// Network type
    pub network_type: NetworkType,

    /// Whether prefix is enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Prefix {
    /// Calculate prefix length
    pub fn calculate_length(&self) -> i32 {
        self.prefix.len() as i32
    }

    /// Check if a destination matches this prefix
    pub fn matches(&self, destination: &str) -> bool {
        let normalized = destination
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        normalized.starts_with(&self.prefix)
    }
}

impl Default for Prefix {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            zone_id: 0,
            prefix: String::new(),
            prefix_length: 0,
            operator_name: None,
            network_type: NetworkType::Fixed,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Rate zone entity
///
/// Defines rates for a specific zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateZone {
    /// Unique identifier
    pub id: i32,

    /// Associated zone ID
    pub zone_id: i32,

    /// Rate name/description
    pub rate_name: Option<String>,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Rate per call (connection fee)
    pub rate_per_call: Decimal,

    /// Billing increment in seconds
    pub billing_increment: i32,

    /// Minimum billable duration in seconds
    pub min_duration: i32,

    /// When this rate becomes effective
    pub effective_from: DateTime<Utc>,

    /// Currency code
    pub currency: String,

    /// Priority for conflict resolution
    pub priority: i32,

    /// Whether rate is enabled
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl RateZone {
    /// Calculate cost for given duration
    pub fn calculate_cost(&self, duration_seconds: i32) -> Decimal {
        if duration_seconds <= 0 {
            return self.rate_per_call;
        }

        // Apply minimum duration
        let effective_duration = duration_seconds.max(self.min_duration);

        // Round up to billing increment
        let increment = self.billing_increment.max(1);
        let rounded_seconds = ((effective_duration + increment - 1) / increment) * increment;

        // Calculate: (seconds / 60) * rate_per_minute + connection_fee
        let minutes = Decimal::from(rounded_seconds) / Decimal::from(60);
        (minutes * self.rate_per_minute) + self.rate_per_call
    }

    /// Get rate per second
    pub fn rate_per_second(&self) -> Decimal {
        self.rate_per_minute / Decimal::from(60)
    }
}

impl Default for RateZone {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            zone_id: 0,
            rate_name: None,
            rate_per_minute: Decimal::ZERO,
            rate_per_call: Decimal::ZERO,
            billing_increment: 6,
            min_duration: 0,
            effective_from: now,
            currency: "USD".to_string(),
            priority: 0,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Combined zone rate information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneRateInfo {
    pub zone_id: i32,
    pub zone_name: String,
    pub zone_type: String,
    pub prefix: String,
    pub rate_per_minute: Decimal,
    pub billing_increment: i32,
    pub currency: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_zone_type_parsing() {
        assert_eq!(ZoneType::from_str("GEOGRAPHIC"), Some(ZoneType::Geographic));
        assert_eq!(ZoneType::from_str("mobile"), Some(ZoneType::Mobile));
        assert_eq!(ZoneType::from_str("SPECIAL"), Some(ZoneType::Special));
        assert_eq!(ZoneType::from_str("invalid"), None);
    }

    #[test]
    fn test_prefix_matches() {
        let prefix = Prefix {
            prefix: "519".to_string(),
            ..Default::default()
        };

        assert!(prefix.matches("51999888777"));
        assert!(prefix.matches("+51999888777"));
        assert!(!prefix.matches("52999888777"));
    }

    #[test]
    fn test_rate_zone_calculate_cost() {
        let rate = RateZone {
            rate_per_minute: dec!(0.10),
            rate_per_call: dec!(0.00),
            billing_increment: 6,
            min_duration: 0,
            ..Default::default()
        };

        // 60 seconds = 1 minute = $0.10
        assert_eq!(rate.calculate_cost(60), dec!(0.10));

        // 7 seconds rounds to 12 seconds = 0.02
        assert_eq!(rate.calculate_cost(7), dec!(0.02));
    }

    #[test]
    fn test_rate_zone_with_minimum() {
        let rate = RateZone {
            rate_per_minute: dec!(0.10),
            rate_per_call: dec!(0.05),
            billing_increment: 6,
            min_duration: 30, // Minimum 30 seconds
            ..Default::default()
        };

        // 10 seconds -> minimum 30 seconds -> $0.05 + (30/60 * 0.10) = $0.10
        assert_eq!(rate.calculate_cost(10), dec!(0.10));
    }
}
