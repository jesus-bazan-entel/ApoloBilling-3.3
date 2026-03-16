//! Active Call DTOs
//!
//! Request and response types for active call tracking endpoints.

use apolo_core::models::ActiveCall;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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

    /// Current cost
    #[serde(default)]
    pub cost: Decimal,

    /// Connection/server ID
    pub connection_id: Option<String>,

    /// Server identifier
    pub server: Option<String>,
}

fn default_direction() -> String {
    "outbound".to_string()
}

impl ActiveCallRequest {
    /// Convert to ActiveCall entity
    pub fn to_active_call(&self) -> ActiveCall {
        let now = Utc::now();
        ActiveCall {
            id: 0,
            call_uuid: self.call_id.clone(),
            account_id: None,
            caller_number: self.calling_number.clone().unwrap_or_default(),
            called_number: self.called_number.clone().unwrap_or_default(),
            zone_name: None,
            rate_per_minute: None,
            start_time: self.start_time.unwrap_or(now),
            answer_time: None,
            current_duration: self.duration,
            current_cost: self.cost,
            max_duration: None,
            freeswitch_server_id: self.server.clone().or(self.connection_id.clone()),
            reservation_id: None,
            updated_at: now,
        }
    }
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

    /// Current duration in seconds
    pub duration_seconds: i32,

    /// Current cost
    pub current_cost: Decimal,

    /// Zone/destination name
    pub zone_name: Option<String>,

    /// Rate per minute
    pub rate_per_minute: Option<Decimal>,

    /// Account ID
    pub account_id: Option<i32>,

    /// Max allowed duration
    pub max_duration: Option<i32>,

    /// Remaining duration
    pub remaining_duration: Option<i32>,

    /// Server ID
    pub server_id: Option<String>,

    /// Last updated
    pub updated_at: DateTime<Utc>,
}

impl From<ActiveCall> for ActiveCallResponse {
    fn from(call: ActiveCall) -> Self {
        let remaining = call.remaining_duration();
        let status = if call.answer_time.is_some() {
            "answered".to_string()
        } else {
            "dialing".to_string()
        };
        Self {
            call_uuid: call.call_uuid,
            caller_number: call.caller_number,
            callee_number: call.called_number,
            direction: "outbound".to_string(), // Default, could be determined
            start_time: call.start_time,
            status,
            duration_seconds: call.current_duration,
            current_cost: call.current_cost,
            zone_name: call.zone_name,
            rate_per_minute: call.rate_per_minute,
            account_id: call.account_id,
            max_duration: call.max_duration,
            remaining_duration: remaining,
            server_id: call.freeswitch_server_id,
            updated_at: call.updated_at,
        }
    }
}

impl From<&ActiveCall> for ActiveCallResponse {
    fn from(call: &ActiveCall) -> Self {
        let status = if call.answer_time.is_some() {
            "answered".to_string()
        } else {
            "dialing".to_string()
        };
        Self {
            call_uuid: call.call_uuid.clone(),
            caller_number: call.caller_number.clone(),
            callee_number: call.called_number.clone(),
            direction: "outbound".to_string(),
            start_time: call.start_time,
            status,
            duration_seconds: call.current_duration,
            current_cost: call.current_cost,
            zone_name: call.zone_name.clone(),
            rate_per_minute: call.rate_per_minute,
            account_id: call.account_id,
            max_duration: call.max_duration,
            remaining_duration: call.remaining_duration(),
            server_id: call.freeswitch_server_id.clone(),
            updated_at: call.updated_at,
        }
    }
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

    /// Rate ID (optional)
    pub rate_id: Option<i32>,

    /// Cost (optional, will be calculated if not provided)
    pub cost: Option<Decimal>,

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

    /// Final cost
    pub cost: Option<Decimal>,

    /// Message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_active_call_request_to_active_call() {
        let req = ActiveCallRequest {
            call_id: "test-uuid".to_string(),
            calling_number: Some("+51999888777".to_string()),
            called_number: Some("+1555123456".to_string()),
            direction: "outbound".to_string(),
            start_time: None,
            duration: 60,
            cost: dec!(0.10),
            connection_id: None,
            server: Some("fs1".to_string()),
        };

        let call = req.to_active_call();
        assert_eq!(call.call_uuid, "test-uuid");
        assert_eq!(call.current_duration, 60);
    }

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
            rate_id: Some(1),
            cost: Some(dec!(0.10)),
            direction: "outbound".to_string(),
            freeswitch_server_id: Some("fs1".to_string()),
        };
        assert!(valid_req.validate().is_ok());
    }
}
