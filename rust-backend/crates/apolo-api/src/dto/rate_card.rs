//! Rate Card DTOs
//!
//! Request and response types for rate card management endpoints.

use apolo_core::models::RateCard;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Rate card creation request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RateCardCreateRequest {
    /// Destination prefix (e.g., "51" for Peru)
    #[validate(length(min = 1, max = 20, message = "Destination prefix is required"))]
    pub destination_prefix: String,

    /// Destination name (e.g., "Peru Mobile")
    #[validate(length(min = 1, max = 100, message = "Destination name is required"))]
    pub destination_name: String,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Billing increment in seconds (default: 6)
    #[serde(default = "default_billing_increment")]
    pub billing_increment: i32,

    /// Connection fee (default: 0)
    #[serde(default)]
    pub connection_fee: Decimal,

    /// Priority for LPM conflict resolution (default: 0)
    #[serde(default)]
    pub priority: i32,
}

fn default_billing_increment() -> i32 {
    6
}

impl RateCardCreateRequest {
    /// Convert to RateCard entity
    pub fn to_rate_card(&self) -> RateCard {
        RateCard {
            id: 0,
            rate_name: Some(self.destination_name.clone()),
            destination_prefix: self.destination_prefix.clone(),
            destination_name: self.destination_name.clone(),
            rate_per_minute: self.rate_per_minute,
            billing_increment: self.billing_increment,
            connection_fee: self.connection_fee,
            effective_start: Utc::now(),
            effective_end: None,
            priority: self.priority,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Rate card update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RateCardUpdateRequest {
    /// Destination name
    pub destination_name: Option<String>,

    /// Rate per minute
    pub rate_per_minute: Option<Decimal>,

    /// Billing increment
    pub billing_increment: Option<i32>,

    /// Connection fee
    pub connection_fee: Option<Decimal>,

    /// Priority
    pub priority: Option<i32>,

    /// Effective end date (for soft delete or expiration)
    pub effective_end: Option<DateTime<Utc>>,
}

/// Bulk create request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct BulkCreateRequest {
    /// Rate cards to create
    #[validate(length(
        min = 1,
        max = 1000,
        message = "Must provide between 1 and 1000 rate cards"
    ))]
    pub rates: Vec<RateCardCreateRequest>,
}

/// Bulk create response
#[derive(Debug, Clone, Serialize)]
pub struct BulkCreateResponse {
    /// Number of rate cards created
    pub created: i32,

    /// Number of errors
    pub errors: i32,

    /// Prefixes that were created
    pub created_prefixes: Vec<String>,

    /// Error details
    pub errors_detail: Vec<BulkError>,
}

/// Bulk error detail
#[derive(Debug, Clone, Serialize)]
pub struct BulkError {
    /// Prefix that failed
    pub prefix: String,

    /// Error message
    pub error: String,
}

/// Rate card response
#[derive(Debug, Clone, Serialize)]
pub struct RateCardResponse {
    /// Rate card ID
    pub id: i32,

    /// Destination prefix
    pub destination_prefix: String,

    /// Destination name
    pub destination_name: String,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Billing increment
    pub billing_increment: i32,

    /// Connection fee
    pub connection_fee: Decimal,

    /// Effective start date
    pub effective_start: DateTime<Utc>,

    /// Effective end date
    pub effective_end: Option<DateTime<Utc>>,

    /// Priority
    pub priority: i32,

    /// Whether rate is currently effective
    pub is_effective: bool,

    /// Rate per second (calculated)
    pub rate_per_second: Decimal,
}

impl From<RateCard> for RateCardResponse {
    fn from(rate: RateCard) -> Self {
        let is_effective = rate.is_effective();
        let rate_per_second = rate.rate_per_second();
        Self {
            id: rate.id,
            destination_prefix: rate.destination_prefix,
            destination_name: rate.destination_name,
            rate_per_minute: rate.rate_per_minute,
            billing_increment: rate.billing_increment,
            connection_fee: rate.connection_fee,
            effective_start: rate.effective_start,
            effective_end: rate.effective_end,
            priority: rate.priority,
            is_effective,
            rate_per_second,
        }
    }
}

impl From<&RateCard> for RateCardResponse {
    fn from(rate: &RateCard) -> Self {
        Self {
            id: rate.id,
            destination_prefix: rate.destination_prefix.clone(),
            destination_name: rate.destination_name.clone(),
            rate_per_minute: rate.rate_per_minute,
            billing_increment: rate.billing_increment,
            connection_fee: rate.connection_fee,
            effective_start: rate.effective_start,
            effective_end: rate.effective_end,
            priority: rate.priority,
            is_effective: rate.is_effective(),
            rate_per_second: rate.rate_per_second(),
        }
    }
}

/// Rate search response
#[derive(Debug, Clone, Serialize)]
pub struct RateSearchResponse {
    /// Phone number searched
    pub phone_number: String,

    /// Matched prefix
    pub matched_prefix: String,

    /// Destination name
    pub destination_name: String,

    /// Rate per minute
    pub rate_per_minute: Decimal,

    /// Billing increment
    pub billing_increment: i32,

    /// Connection fee
    pub connection_fee: Decimal,

    /// Rate card ID
    pub rate_id: i32,

    /// Estimated cost for 1 minute
    pub estimated_cost_1min: Decimal,

    /// Estimated cost for 5 minutes
    pub estimated_cost_5min: Decimal,
}

impl RateSearchResponse {
    pub fn from_rate_card(rate: &RateCard, phone_number: &str) -> Self {
        Self {
            phone_number: phone_number.to_string(),
            matched_prefix: rate.destination_prefix.clone(),
            destination_name: rate.destination_name.clone(),
            rate_per_minute: rate.rate_per_minute,
            billing_increment: rate.billing_increment,
            connection_fee: rate.connection_fee,
            rate_id: rate.id,
            estimated_cost_1min: rate.calculate_cost(60),
            estimated_cost_5min: rate.calculate_cost(300),
        }
    }
}

/// Rate card filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RateCardFilterParams {
    /// Filter by prefix (partial match)
    pub prefix: Option<String>,

    /// Filter by destination name (partial match)
    pub name: Option<String>,

    /// Only show effective rates
    #[serde(default)]
    pub effective_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_rate_card_create_request_to_rate_card() {
        let req = RateCardCreateRequest {
            destination_prefix: "51".to_string(),
            destination_name: "Peru".to_string(),
            rate_per_minute: dec!(0.05),
            billing_increment: 6,
            connection_fee: dec!(0.01),
            priority: 1,
        };

        let rate = req.to_rate_card();
        assert_eq!(rate.destination_prefix, "51");
        assert_eq!(rate.rate_per_minute, dec!(0.05));
        assert!(rate.is_effective());
    }

    #[test]
    fn test_rate_card_response_from_rate_card() {
        let rate = RateCard {
            id: 1,
            rate_name: Some("Peru Mobile".to_string()),
            destination_prefix: "519".to_string(),
            destination_name: "Peru Mobile".to_string(),
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            effective_start: Utc::now() - chrono::Duration::hours(1),
            effective_end: None,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let response = RateCardResponse::from(&rate);
        assert_eq!(response.destination_prefix, "519");
        assert!(response.is_effective);
    }

    #[test]
    fn test_rate_search_response() {
        let rate = RateCard {
            id: 1,
            rate_name: Some("Peru".to_string()),
            destination_prefix: "51".to_string(),
            destination_name: "Peru".to_string(),
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            ..Default::default()
        };

        let response = RateSearchResponse::from_rate_card(&rate, "+51999888777");
        assert_eq!(response.matched_prefix, "51");
        assert_eq!(response.estimated_cost_1min, dec!(0.10));
    }
}
