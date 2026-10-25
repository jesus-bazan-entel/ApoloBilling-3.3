//! Active Call DTOs
//!
//! Request and response types for active call tracking endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Active call report/upsert request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ActiveCallRequest {
    /// Call unique identifier
    #[validate(length(min = 1, message = "Call ID is required"))]
    pub call_id: String,

    /// Caller number
    pub calling_number: Option<String>,

    /// Called number
    pub called_number: Option<String>,

    /// Call direction
    #[serde(default = "default_direction")]
    pub direction: String,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// Current duration in seconds
    #[serde(default)]
    pub duration: i32,

    /// Connection/server ID
    pub connection_id: Option<String>,

    /// Server identifier
    pub server: Option<String>,
}

fn default_direction() -> String {
    "outbound".to_string()
}

/// Active call response
#[derive(Debug, Clone, Serialize)]
pub struct ActiveCallResponse {
    /// Call unique identifier (frontend expects call_uuid)
    pub call_uuid: String,

    /// Caller number (frontend expects caller_number)
    pub caller_number: String,

    /// Called number (frontend expects callee_number)
    pub callee_number: String,

    /// Call direction
    pub direction: String,

    /// Start time
    pub start_time: DateTime<Utc>,

    /// Call status
    pub status: String,

    /// Answer time (None while dialing/ringing)
    pub answer_time: Option<DateTime<Utc>>,

    /// Current duration in seconds
    pub duration_seconds: i32,

    /// Account ID
    pub account_id: Option<i32>,

    /// Server ID
    pub server_id: Option<String>,

    /// Last updated
    pub updated_at: DateTime<Utc>,
}

/// CDR creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CdrCreateRequest {
    /// Call unique identifier
    #[validate(length(min = 1, message = "Call UUID is required"))]
    pub call_uuid: String,

    /// Account ID
    pub account_id: Option<i32>,

    /// Caller number
    #[validate(length(min = 1, message = "Caller number is required"))]
    pub caller_number: String,

    /// Called number
    #[validate(length(min = 1, message = "Called number is required"))]
    pub called_number: String,

    /// Start time
    pub start_time: DateTime<Utc>,

    /// Answer time (None if not answered)
    pub answer_time: Option<DateTime<Utc>>,

    /// End time
    pub end_time: DateTime<Utc>,

    /// Total duration in seconds
    pub duration: i32,

    /// Billable seconds
    pub billsec: i32,

    /// Hangup cause
    #[serde(default = "default_hangup_cause")]
    pub hangup_cause: String,

    /// Call direction
    #[serde(default = "default_direction")]
    pub direction: String,

    /// FreeSWITCH server ID
    pub freeswitch_server_id: Option<String>,
}

fn default_hangup_cause() -> String {
    "NORMAL_CLEARING".to_string()
}

/// CDR creation response
#[derive(Debug, Clone, Serialize)]
pub struct CdrCreateResponse {
    /// Created CDR ID
    pub id: i64,

    /// Call UUID
    pub call_uuid: String,

    /// Message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_cdr_create_request_validation() {
        let valid_req = CdrCreateRequest {
            call_uuid: "test-uuid".to_string(),
            account_id: Some(1),
            caller_number: "+51999888777".to_string(),
            called_number: "+1555123456".to_string(),
            start_time: Utc::now(),
            answer_time: Some(Utc::now()),
            end_time: Utc::now(),
            duration: 60,
            billsec: 55,
            hangup_cause: "NORMAL_CLEARING".to_string(),
            direction: "outbound".to_string(),
            freeswitch_server_id: Some("fs1".to_string()),
        };
        assert!(valid_req.validate().is_ok());
    }
}
