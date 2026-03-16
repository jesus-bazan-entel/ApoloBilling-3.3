//! Rate card model
//!
//! Represents billing rates for different destinations.
//! Supports longest prefix matching for rate lookup.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Rate card entity
///
/// Defines the billing rate for calls to a specific destination prefix.
/// Multiple rate cards can exist for overlapping prefixes, resolved by
/// longest prefix match and priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCard {
    /// Unique identifier
    pub id: i32,

    /// Rate name/description
    pub rate_name: Option<String>,

    /// Destination prefix for matching (e.g., "51" for Peru, "519" for Peru Mobile)
    pub destination_prefix: String,

    /// Human-readable destination name
    pub destination_name: String,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Billing increment in seconds (e.g., 6 for 6-second billing)
    pub billing_increment: i32,

    /// Connection fee (charged per call)
    pub connection_fee: Decimal,

    /// When this rate becomes effective
    pub effective_start: DateTime<Utc>,

    /// When this rate expires (None = no expiry)
    pub effective_end: Option<DateTime<Utc>>,

    /// Priority for conflict resolution (higher = preferred)
    pub priority: i32,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl RateCard {
    /// Calculate the cost for a given duration
    ///
    /// Rounds up to the nearest billing increment before calculating.
    ///
    /// # Arguments
    /// * `billsec` - Billable seconds (duration from answer to hangup)
    ///
    /// # Returns
    /// Total cost including connection fee
    #[inline]
    pub fn calculate_cost(&self, billsec: i32) -> Decimal {
        if billsec <= 0 {
            return self.connection_fee;
        }

        // Round up to billing increment
        let increment = self.billing_increment.max(1);
        let rounded_seconds = ((billsec + increment - 1) / increment) * increment;

        // Calculate cost: (seconds / 60) * rate_per_minute + connection_fee
        let minutes = Decimal::from(rounded_seconds) / Decimal::from(60);
        (minutes * self.rate_per_minute) + self.connection_fee
    }

    /// Get rate per second (for reservation calculations)
    #[inline]
    pub fn rate_per_second(&self) -> Decimal {
        self.rate_per_minute / Decimal::from(60)
    }

    /// Check if rate is currently effective
    pub fn is_effective(&self) -> bool {
        let now = Utc::now();
        now >= self.effective_start && self.effective_end.map_or(true, |end| now < end)
    }

    /// Get estimated cost for a given duration in minutes
    pub fn estimate_cost_minutes(&self, minutes: i32) -> Decimal {
        let seconds = minutes * 60;
        self.calculate_cost(seconds)
    }

    /// Normalize a phone number for prefix matching
    pub fn normalize_destination(destination: &str) -> String {
        destination.chars().filter(|c| c.is_ascii_digit()).collect()
    }

    /// Generate all possible prefixes for a destination (for LPM lookup)
    ///
    /// Returns prefixes from longest to shortest.
    pub fn generate_prefixes(destination: &str) -> Vec<String> {
        let normalized = Self::normalize_destination(destination);
        (1..=normalized.len())
            .rev()
            .map(|i| normalized[..i].to_string())
            .collect()
    }
}

impl Default for RateCard {
    fn default() -> Self {
        Self {
            id: 0,
            rate_name: None,
            destination_prefix: String::new(),
            destination_name: String::new(),
            rate_per_minute: Decimal::ZERO,
            billing_increment: 6,
            connection_fee: Decimal::ZERO,
            effective_start: Utc::now(),
            effective_end: None,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_cost_basic() {
        let rate = RateCard {
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            ..Default::default()
        };

        // 60 seconds = 1 minute = $0.10
        assert_eq!(rate.calculate_cost(60), dec!(0.10));

        // 30 seconds = 0.5 minutes, rounded to 30 seconds = $0.05
        assert_eq!(rate.calculate_cost(30), dec!(0.05));
    }

    #[test]
    fn test_calculate_cost_with_rounding() {
        let rate = RateCard {
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            ..Default::default()
        };

        // 7 seconds rounds up to 12 seconds (2 increments)
        // 12/60 * 0.10 = 0.02
        assert_eq!(rate.calculate_cost(7), dec!(0.02));

        // 1 second rounds up to 6 seconds (1 increment)
        // 6/60 * 0.10 = 0.01
        assert_eq!(rate.calculate_cost(1), dec!(0.01));
    }

    #[test]
    fn test_calculate_cost_with_connection_fee() {
        let rate = RateCard {
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.05),
            ..Default::default()
        };

        // 60 seconds = $0.10 + $0.05 = $0.15
        assert_eq!(rate.calculate_cost(60), dec!(0.15));

        // 0 seconds = just connection fee
        assert_eq!(rate.calculate_cost(0), dec!(0.05));
    }

    #[test]
    fn test_rate_per_second() {
        let rate = RateCard {
            rate_per_minute: dec!(0.60),
            ..Default::default()
        };

        assert_eq!(rate.rate_per_second(), dec!(0.01));
    }

    #[test]
    fn test_normalize_destination() {
        assert_eq!(
            RateCard::normalize_destination("+51999888777"),
            "51999888777"
        );
        assert_eq!(
            RateCard::normalize_destination("1-555-123-4567"),
            "15551234567"
        );
    }

    #[test]
    fn test_generate_prefixes() {
        let prefixes = RateCard::generate_prefixes("5199");
        assert_eq!(prefixes, vec!["5199", "519", "51", "5"]);
    }

    #[test]
    fn test_is_effective() {
        let now = Utc::now();

        // Active rate (no end date)
        let rate = RateCard {
            effective_start: now - chrono::Duration::hours(1),
            effective_end: None,
            ..Default::default()
        };
        assert!(rate.is_effective());

        // Expired rate
        let rate = RateCard {
            effective_start: now - chrono::Duration::hours(2),
            effective_end: Some(now - chrono::Duration::hours(1)),
            ..Default::default()
        };
        assert!(!rate.is_effective());

        // Future rate
        let rate = RateCard {
            effective_start: now + chrono::Duration::hours(1),
            effective_end: None,
            ..Default::default()
        };
        assert!(!rate.is_effective());
    }
}
