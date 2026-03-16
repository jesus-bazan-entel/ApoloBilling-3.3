//! CDR (Call Detail Record) model
//!
//! Represents completed call records for billing and reporting.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// CDR (Call Detail Record)
///
/// Stores complete information about a finished call including
/// timing, billing, and termination details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cdr {
    /// Unique identifier
    pub id: i64,

    /// Call unique identifier (from PBX)
    pub call_uuid: String,

    /// Associated account ID
    pub account_id: Option<i32>,

    /// Caller number (ANI/CLI)
    pub caller_number: String,

    /// Called number (DNIS)
    pub called_number: String,

    /// Matched destination prefix
    pub destination_prefix: Option<String>,

    /// Call start timestamp
    pub start_time: DateTime<Utc>,

    /// Call answer timestamp (None if not answered)
    pub answer_time: Option<DateTime<Utc>>,

    /// Call end timestamp
    pub end_time: DateTime<Utc>,

    /// Total call duration in seconds
    pub duration: i32,

    /// Billable duration in seconds (from answer to hangup)
    pub billsec: i32,

    /// Applied rate card ID
    pub rate_id: Option<i32>,

    /// Applied rate per minute
    pub rate_per_minute: Option<Decimal>,

    /// Total cost of the call
    pub cost: Option<Decimal>,

    /// Hangup cause code
    pub hangup_cause: String,

    /// Call direction (inbound/outbound)
    pub direction: String,

    /// FreeSWITCH server that handled the call
    pub freeswitch_server_id: Option<String>,

    /// Associated balance reservation ID
    pub reservation_id: Option<Uuid>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// When billing was processed
    pub processed_at: Option<DateTime<Utc>>,
}

impl Cdr {
    /// Check if the call was answered
    #[inline]
    pub fn was_answered(&self) -> bool {
        self.answer_time.is_some() && self.billsec > 0
    }

    /// Check if this is an outbound call
    #[inline]
    pub fn is_outbound(&self) -> bool {
        self.direction.to_lowercase() == "outbound"
    }

    /// Check if this is an inbound call
    #[inline]
    pub fn is_inbound(&self) -> bool {
        self.direction.to_lowercase() == "inbound"
    }

    /// Check if call was successful (answered and normal hangup)
    pub fn is_successful(&self) -> bool {
        self.was_answered() && self.hangup_cause.to_uppercase() == "NORMAL_CLEARING"
    }

    /// Get effective duration for display
    pub fn effective_duration(&self) -> String {
        let mins = self.duration / 60;
        let secs = self.duration % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    /// Get billable duration for display
    pub fn billable_duration(&self) -> String {
        let mins = self.billsec / 60;
        let secs = self.billsec % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

impl Default for Cdr {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            call_uuid: Uuid::new_v4().to_string(),
            account_id: None,
            caller_number: String::new(),
            called_number: String::new(),
            destination_prefix: None,
            start_time: now,
            answer_time: None,
            end_time: now,
            duration: 0,
            billsec: 0,
            rate_id: None,
            rate_per_minute: None,
            cost: None,
            hangup_cause: "NORMAL_CLEARING".to_string(),
            direction: "outbound".to_string(),
            freeswitch_server_id: None,
            reservation_id: None,
            created_at: now,
            processed_at: None,
        }
    }
}

/// Active call tracking
///
/// Represents a currently active call for real-time monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCall {
    /// Unique identifier
    pub id: i64,

    /// Call unique identifier
    pub call_uuid: String,

    /// Associated account ID
    pub account_id: Option<i32>,

    /// Caller number
    pub caller_number: String,

    /// Called number
    pub called_number: String,

    /// Matched zone/destination
    pub zone_name: Option<String>,

    /// Applied rate per minute
    pub rate_per_minute: Option<Decimal>,

    /// Call start timestamp
    pub start_time: DateTime<Utc>,

    /// Call answer timestamp
    pub answer_time: Option<DateTime<Utc>>,

    /// Current duration in seconds
    pub current_duration: i32,

    /// Current accumulated cost
    pub current_cost: Decimal,

    /// Maximum allowed duration in seconds
    pub max_duration: Option<i32>,

    /// FreeSWITCH server handling the call
    pub freeswitch_server_id: Option<String>,

    /// Associated reservation ID
    pub reservation_id: Option<Uuid>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl ActiveCall {
    /// Calculate current duration from start/answer time
    pub fn calculate_duration(&self) -> i32 {
        let reference_time = self.answer_time.unwrap_or(self.start_time);
        let elapsed = Utc::now() - reference_time;
        elapsed.num_seconds().max(0) as i32
    }

    /// Check if call has exceeded max duration
    pub fn is_over_limit(&self) -> bool {
        self.max_duration
            .map(|max| self.current_duration >= max)
            .unwrap_or(false)
    }

    /// Get remaining duration in seconds
    pub fn remaining_duration(&self) -> Option<i32> {
        self.max_duration
            .map(|max| (max - self.current_duration).max(0))
    }
}

impl Default for ActiveCall {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            call_uuid: Uuid::new_v4().to_string(),
            account_id: None,
            caller_number: String::new(),
            called_number: String::new(),
            zone_name: None,
            rate_per_minute: None,
            start_time: now,
            answer_time: None,
            current_duration: 0,
            current_cost: Decimal::ZERO,
            max_duration: None,
            freeswitch_server_id: None,
            reservation_id: None,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_was_answered() {
        let mut cdr = Cdr::default();
        assert!(!cdr.was_answered());

        cdr.answer_time = Some(Utc::now());
        cdr.billsec = 30;
        assert!(cdr.was_answered());
    }

    #[test]
    fn test_cdr_direction() {
        let mut cdr = Cdr::default();
        cdr.direction = "outbound".to_string();
        assert!(cdr.is_outbound());
        assert!(!cdr.is_inbound());

        cdr.direction = "inbound".to_string();
        assert!(cdr.is_inbound());
        assert!(!cdr.is_outbound());
    }

    #[test]
    fn test_effective_duration() {
        let cdr = Cdr {
            duration: 125, // 2:05
            ..Default::default()
        };
        assert_eq!(cdr.effective_duration(), "02:05");
    }

    #[test]
    fn test_active_call_remaining() {
        let call = ActiveCall {
            max_duration: Some(300),
            current_duration: 100,
            ..Default::default()
        };
        assert_eq!(call.remaining_duration(), Some(200));
        assert!(!call.is_over_limit());
    }
}
