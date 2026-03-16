// tests/esl_simulator.rs
//! ESL Event Simulator for Integration Testing
//!
//! This module provides utilities to simulate FreeSWITCH ESL events for testing
//! the billing engine without requiring a real FreeSWITCH instance.

use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use std::collections::HashMap;

/// ESL Event builder for creating realistic test events
pub struct EslEventBuilder {
    headers: HashMap<String, String>,
}

impl EslEventBuilder {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Set a header value
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the event name
    pub fn event_name(self, name: &str) -> Self {
        self.header("Event-Name", name)
    }

    /// Set the unique call ID
    pub fn call_uuid(self, uuid: &str) -> Self {
        self.header("Unique-ID", uuid)
            .header("Channel-Call-UUID", uuid)
            .header("variable_uuid", uuid)
            .header("variable_call_uuid", uuid)
    }

    /// Set caller/callee numbers
    pub fn caller(self, number: &str) -> Self {
        self.header("Caller-Caller-ID-Number", number)
            .header("Caller-Caller-ID-Name", number)
            .header("Caller-Username", number)
            .header("variable_sip_from_user", number)
            .header("variable_account_code", number)
    }

    pub fn callee(self, number: &str) -> Self {
        self.header("Caller-Destination-Number", number)
            .header("variable_sip_to_user", number)
    }

    /// Set timestamp in microseconds
    pub fn timestamp(self, dt: DateTime<Utc>) -> Self {
        let micros = dt.timestamp_micros();
        let secs = dt.timestamp();
        self.header("Event-Date-Timestamp", &micros.to_string())
            .header("variable_start_epoch", &secs.to_string())
    }

    /// Set answer time
    pub fn answer_time(self, dt: DateTime<Utc>) -> Self {
        let secs = dt.timestamp();
        let micros = dt.timestamp_micros();
        self.header("variable_answer_epoch", &secs.to_string())
            .header("variable_answer_uepoch", &micros.to_string())
    }

    /// Set end time
    pub fn end_time(self, dt: DateTime<Utc>) -> Self {
        let secs = dt.timestamp();
        let micros = dt.timestamp_micros();
        self.header("variable_end_epoch", &secs.to_string())
            .header("variable_end_uepoch", &micros.to_string())
    }

    /// Set duration and billsec
    pub fn duration(self, total_secs: i64, billable_secs: i64) -> Self {
        self.header("variable_duration", &total_secs.to_string())
            .header("variable_billsec", &billable_secs.to_string())
    }

    /// Set hangup cause
    pub fn hangup_cause(self, cause: &str) -> Self {
        self.header("Hangup-Cause", cause)
            .header("variable_hangup_cause", cause)
    }

    /// Build the event as a string (ESL format)
    pub fn build(&self) -> String {
        let mut result = String::new();
        for (key, value) in &self.headers {
            result.push_str(&format!("{}: {}\n", key, value));
        }
        result.push('\n'); // End of headers
        result
    }
}

/// Simulated call that tracks its lifecycle
pub struct SimulatedCall {
    pub uuid: String,
    pub caller: String,
    pub callee: String,
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub hangup_cause: String,
}

impl SimulatedCall {
    pub fn new(caller: &str, callee: &str) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            caller: caller.to_string(),
            callee: callee.to_string(),
            start_time: Utc::now(),
            answer_time: None,
            end_time: None,
            hangup_cause: "NORMAL_CLEARING".to_string(),
        }
    }

    pub fn with_uuid(mut self, uuid: &str) -> Self {
        self.uuid = uuid.to_string();
        self
    }

    pub fn with_start_time(mut self, time: DateTime<Utc>) -> Self {
        self.start_time = time;
        self
    }

    /// Generate CHANNEL_CREATE event
    pub fn channel_create_event(&self) -> String {
        EslEventBuilder::new()
            .event_name("CHANNEL_CREATE")
            .call_uuid(&self.uuid)
            .caller(&self.caller)
            .callee(&self.callee)
            .timestamp(self.start_time)
            .header("Core-UUID", &Uuid::new_v4().to_string())
            .header("FreeSWITCH-Hostname", "billing-test")
            .header("FreeSWITCH-Switchname", "apolo-billing")
            .header("FreeSWITCH-IPv4", "127.0.0.1")
            .header("Channel-State", "CS_INIT")
            .header("Channel-Call-State", "DOWN")
            .header("Channel-Name", &format!("sofia/external/{}@gateway", self.callee))
            .header("Call-Direction", "outbound")
            .header("Answer-State", "ringing")
            .header("Caller-Network-Addr", "127.0.0.1")
            .header("variable_direction", "outbound")
            .build()
    }

    /// Simulate call being answered
    pub fn answer(&mut self, delay_secs: i64) {
        self.answer_time = Some(self.start_time + Duration::seconds(delay_secs));
    }

    /// Generate CHANNEL_ANSWER event
    pub fn channel_answer_event(&self) -> Option<String> {
        let answer_time = self.answer_time?;

        Some(EslEventBuilder::new()
            .event_name("CHANNEL_ANSWER")
            .call_uuid(&self.uuid)
            .caller(&self.caller)
            .callee(&self.callee)
            .timestamp(self.start_time)
            .answer_time(answer_time)
            .header("Core-UUID", &Uuid::new_v4().to_string())
            .header("FreeSWITCH-Hostname", "billing-test")
            .header("FreeSWITCH-Switchname", "apolo-billing")
            .header("FreeSWITCH-IPv4", "127.0.0.1")
            .header("Channel-State", "CS_EXECUTE")
            .header("Channel-Call-State", "ACTIVE")
            .header("Channel-Name", &format!("sofia/external/{}@gateway", self.callee))
            .header("Call-Direction", "outbound")
            .header("Answer-State", "answered")
            .header("variable_direction", "outbound")
            .build())
    }

    /// Simulate call ending
    pub fn hangup(&mut self, billable_secs: i64, cause: &str) {
        let answer_time = self.answer_time.unwrap_or(self.start_time);
        self.end_time = Some(answer_time + Duration::seconds(billable_secs));
        self.hangup_cause = cause.to_string();
    }

    /// Generate CHANNEL_HANGUP_COMPLETE event
    pub fn channel_hangup_event(&self) -> Option<String> {
        let answer_time = self.answer_time?;
        let end_time = self.end_time?;

        let total_duration = (end_time - self.start_time).num_seconds();
        let billsec = (end_time - answer_time).num_seconds();

        Some(EslEventBuilder::new()
            .event_name("CHANNEL_HANGUP_COMPLETE")
            .call_uuid(&self.uuid)
            .caller(&self.caller)
            .callee(&self.callee)
            .timestamp(self.start_time)
            .answer_time(answer_time)
            .end_time(end_time)
            .duration(total_duration, billsec)
            .hangup_cause(&self.hangup_cause)
            .header("Core-UUID", &Uuid::new_v4().to_string())
            .header("FreeSWITCH-Hostname", "billing-test")
            .header("FreeSWITCH-Switchname", "apolo-billing")
            .header("FreeSWITCH-IPv4", "127.0.0.1")
            .header("Channel-State", "CS_DESTROY")
            .header("Channel-Call-State", "HANGUP")
            .header("Channel-Name", &format!("sofia/external/{}@gateway", self.callee))
            .header("Call-Direction", "outbound")
            .header("Answer-State", "hangup")
            .header("variable_direction", "outbound")
            .build())
    }

    /// Calculate expected cost for the call
    pub fn expected_cost(&self, rate_per_minute: f64, billing_increment: i64) -> f64 {
        let answer_time = match self.answer_time {
            Some(t) => t,
            None => return 0.0,
        };
        let end_time = match self.end_time {
            Some(t) => t,
            None => return 0.0,
        };

        let billsec = (end_time - answer_time).num_seconds();

        // Round up to billing increment
        let rounded_secs = if billing_increment > 0 {
            ((billsec + billing_increment - 1) / billing_increment) * billing_increment
        } else {
            billsec
        };

        let minutes = rounded_secs as f64 / 60.0;
        minutes * rate_per_minute
    }
}

/// Test scenario representing a complete call flow
#[derive(Debug, Clone)]
pub struct CallScenario {
    pub name: String,
    pub caller: String,
    pub callee: String,
    pub answer_delay_secs: Option<i64>,
    pub call_duration_secs: Option<i64>,
    pub hangup_cause: String,
    pub expected_authorized: bool,
    pub expected_rejection_reason: Option<String>,
}

impl CallScenario {
    pub fn authorized_call(name: &str, caller: &str, callee: &str, duration_secs: i64) -> Self {
        Self {
            name: name.to_string(),
            caller: caller.to_string(),
            callee: callee.to_string(),
            answer_delay_secs: Some(2),
            call_duration_secs: Some(duration_secs),
            hangup_cause: "NORMAL_CLEARING".to_string(),
            expected_authorized: true,
            expected_rejection_reason: None,
        }
    }

    pub fn rejected_call(name: &str, caller: &str, callee: &str, reason: &str) -> Self {
        Self {
            name: name.to_string(),
            caller: caller.to_string(),
            callee: callee.to_string(),
            answer_delay_secs: None,
            call_duration_secs: None,
            hangup_cause: "CALL_REJECTED".to_string(),
            expected_authorized: false,
            expected_rejection_reason: Some(reason.to_string()),
        }
    }

    pub fn unanswered_call(name: &str, caller: &str, callee: &str) -> Self {
        Self {
            name: name.to_string(),
            caller: caller.to_string(),
            callee: callee.to_string(),
            answer_delay_secs: None,
            call_duration_secs: None,
            hangup_cause: "NO_ANSWER".to_string(),
            expected_authorized: true,
            expected_rejection_reason: None,
        }
    }

    /// Execute the scenario and return a SimulatedCall
    pub fn execute(&self) -> SimulatedCall {
        let mut call = SimulatedCall::new(&self.caller, &self.callee);

        if let Some(answer_delay) = self.answer_delay_secs {
            call.answer(answer_delay);

            if let Some(duration) = self.call_duration_secs {
                call.hangup(duration, &self.hangup_cause);
            }
        }

        call
    }
}

/// Collection of predefined test scenarios
pub fn standard_test_scenarios() -> Vec<CallScenario> {
    vec![
        // === AUTHORIZATION TESTS ===
        CallScenario::authorized_call(
            "Basic prepaid call to Peru Mobile",
            "100001",
            "51987654321",
            30,
        ),
        CallScenario::authorized_call(
            "Prepaid call to USA",
            "100001",
            "12125551234",
            60,
        ),
        CallScenario::authorized_call(
            "Prepaid call to UK Mobile",
            "100002",
            "447911123456",
            45,
        ),
        CallScenario::rejected_call(
            "Suspended account call",
            "100005",
            "51987654321",
            "account_suspended",
        ),
        CallScenario::rejected_call(
            "Zero balance call",
            "100004",
            "51987654321",
            "insufficient_balance",
        ),
        CallScenario::rejected_call(
            "Unknown account call",
            "999999",
            "51987654321",
            "account_not_found",
        ),

        // === POSTPAID TESTS ===
        CallScenario::authorized_call(
            "Postpaid call within credit",
            "200001",
            "51987654321",
            120,
        ),

        // === DURATION TESTS ===
        CallScenario::authorized_call(
            "Very short call (6 seconds)",
            "100001",
            "51987654321",
            6,
        ),
        CallScenario::authorized_call(
            "Medium call (2 minutes)",
            "100001",
            "12125551234",
            120,
        ),

        // === TOLL FREE TEST ===
        CallScenario::authorized_call(
            "Toll free call",
            "100001",
            "18005551234",
            180,
        ),

        // === EDGE CASES ===
        CallScenario::unanswered_call(
            "Unanswered call",
            "100001",
            "51987654321",
        ),

        // === LOW BALANCE TESTS ===
        CallScenario::authorized_call(
            "Low balance short call",
            "100003",
            "51987654321",
            10,
        ),
    ]
}

/// Rate information for cost verification
#[derive(Debug, Clone)]
pub struct RateInfo {
    pub prefix: String,
    pub rate_per_minute: f64,
    pub billing_increment: i64,
}

impl RateInfo {
    pub fn peru_mobile() -> Self {
        Self { prefix: "519".to_string(), rate_per_minute: 0.025, billing_increment: 6 }
    }
    pub fn peru_lima() -> Self {
        Self { prefix: "511".to_string(), rate_per_minute: 0.012, billing_increment: 6 }
    }
    pub fn usa_general() -> Self {
        Self { prefix: "1".to_string(), rate_per_minute: 0.010, billing_increment: 6 }
    }
    pub fn usa_ny() -> Self {
        Self { prefix: "1212".to_string(), rate_per_minute: 0.008, billing_increment: 6 }
    }
    pub fn usa_toll_free() -> Self {
        Self { prefix: "1800".to_string(), rate_per_minute: 0.0, billing_increment: 60 }
    }
    pub fn uk_mobile() -> Self {
        Self { prefix: "447".to_string(), rate_per_minute: 0.050, billing_increment: 6 }
    }
}

/// Find the best matching rate for a destination
pub fn find_rate_for_destination(destination: &str) -> Option<RateInfo> {
    let normalized = destination.trim_start_matches('+');

    // Try longest prefix first
    let rates = vec![
        RateInfo::usa_toll_free(),
        RateInfo::usa_ny(),
        RateInfo::peru_mobile(),
        RateInfo::peru_lima(),
        RateInfo::uk_mobile(),
        RateInfo::usa_general(),
    ];

    rates.into_iter()
        .filter(|r| normalized.starts_with(&r.prefix))
        .max_by_key(|r| r.prefix.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulated_call_basic() {
        let mut call = SimulatedCall::new("100001", "51987654321");
        assert!(!call.uuid.is_empty());

        call.answer(2);
        assert!(call.answer_time.is_some());

        call.hangup(30, "NORMAL_CLEARING");
        assert!(call.end_time.is_some());
        assert_eq!(call.hangup_cause, "NORMAL_CLEARING");
    }

    #[test]
    fn test_channel_create_event() {
        let call = SimulatedCall::new("100001", "51987654321");
        let event = call.channel_create_event();

        assert!(event.contains("Event-Name: CHANNEL_CREATE"));
        assert!(event.contains("Caller-Caller-ID-Number: 100001"));
        assert!(event.contains("Caller-Destination-Number: 51987654321"));
    }

    #[test]
    fn test_channel_answer_event() {
        let mut call = SimulatedCall::new("100001", "51987654321");

        // No answer yet
        assert!(call.channel_answer_event().is_none());

        call.answer(2);
        let event = call.channel_answer_event().unwrap();

        assert!(event.contains("Event-Name: CHANNEL_ANSWER"));
        assert!(event.contains("Answer-State: answered"));
    }

    #[test]
    fn test_channel_hangup_event() {
        let mut call = SimulatedCall::new("100001", "51987654321");
        call.answer(2);
        call.hangup(30, "NORMAL_CLEARING");

        let event = call.channel_hangup_event().unwrap();

        assert!(event.contains("Event-Name: CHANNEL_HANGUP_COMPLETE"));
        assert!(event.contains("variable_billsec: 30"));
        assert!(event.contains("Hangup-Cause: NORMAL_CLEARING"));
    }

    #[test]
    fn test_expected_cost_calculation() {
        let mut call = SimulatedCall::new("100001", "51987654321");
        call.answer(2);
        call.hangup(30, "NORMAL_CLEARING");

        // 30 seconds at $0.025/min with 6-second billing increment
        // 30 seconds = 0.5 minutes
        // Cost = 0.5 * 0.025 = 0.0125
        let cost = call.expected_cost(0.025, 6);
        assert!((cost - 0.0125).abs() < 0.0001);
    }

    #[test]
    fn test_cost_with_rounding() {
        let mut call = SimulatedCall::new("100001", "51987654321");
        call.answer(2);
        call.hangup(32, "NORMAL_CLEARING"); // 32 seconds

        // 32 seconds with 6-second increment rounds to 36 seconds
        // 36 seconds = 0.6 minutes
        // Cost = 0.6 * 0.025 = 0.015
        let cost = call.expected_cost(0.025, 6);
        assert!((cost - 0.015).abs() < 0.0001);
    }

    #[test]
    fn test_scenario_execution() {
        let scenario = CallScenario::authorized_call(
            "Test call",
            "100001",
            "51987654321",
            60,
        );

        let call = scenario.execute();
        assert!(call.answer_time.is_some());
        assert!(call.end_time.is_some());
    }

    #[test]
    fn test_find_rate_for_destination() {
        // Peru mobile
        let rate = find_rate_for_destination("51987654321").unwrap();
        assert_eq!(rate.prefix, "519");
        assert_eq!(rate.rate_per_minute, 0.025);

        // USA toll free
        let rate = find_rate_for_destination("18005551234").unwrap();
        assert_eq!(rate.prefix, "1800");
        assert_eq!(rate.rate_per_minute, 0.0);

        // USA NY
        let rate = find_rate_for_destination("12125551234").unwrap();
        assert_eq!(rate.prefix, "1212");
    }
}
