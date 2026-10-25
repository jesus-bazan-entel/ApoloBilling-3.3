//! CDR-related DTOs

use super::common::{ExportFormat, PaginationParams};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// CDR filter parameters for list queries
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CdrFilterParams {
    /// Pagination parameters
    #[serde(flatten)]
    #[validate(nested)]
    pub pagination: PaginationParams,

    /// Filter by account ID
    pub account_id: Option<i32>,

    /// Filter by caller number (prefix match)
    pub caller: Option<String>,

    /// Filter by called number (prefix match)
    pub callee: Option<String>,

    /// Filter by minimum start time (ISO 8601)
    #[serde(default)]
    pub start_date: Option<String>,

    /// Filter by maximum start time (ISO 8601)
    #[serde(default)]
    pub end_date: Option<String>,

    /// Filter by call direction
    pub direction: Option<CdrDirection>,

    /// Filter by hangup cause
    pub hangup_cause: Option<String>,

    /// Only show answered calls
    #[serde(default)]
    pub answered_only: bool,

    /// Customer number (accounts.account_number); resolved to account_id
    pub account: Option<String>,

    /// `answered` / `unanswered` / `shutdown` / `flagged`
    #[validate(custom(function = "validate_outcome"))]
    pub outcome: Option<String>,

    /// Minimum talk time in seconds
    #[validate(range(min = 0, max = 86400))]
    pub min_billsec: Option<i32>,

    /// Snapshot from the first page (`X-Cdr-Snapshot`): later pages ignore
    /// CDRs written after it, so rows never shift while paging
    pub max_id: Option<i64>,
}

fn validate_outcome(value: &str) -> Result<(), validator::ValidationError> {
    match value {
        "" | "answered" | "unanswered" | "shutdown" | "flagged" => Ok(()),
        _ => Err(validator::ValidationError::new("outcome")),
    }
}

/// CDR export parameters
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CdrExportParams {
    /// Account ID filter
    pub account_id: Option<i32>,

    /// Caller number filter (prefix match)
    pub caller: Option<String>,

    /// Called number filter (prefix match)
    pub callee: Option<String>,

    /// Start date filter (YYYY-MM-DD in Lima time or RFC 3339), resolved
    /// with the same rules as the CDR list so both return the same rows
    pub start_date: Option<String>,

    /// End date filter (YYYY-MM-DD means end of that day in Lima time)
    pub end_date: Option<String>,

    /// Call direction filter
    pub direction: Option<CdrDirection>,

    /// Customer number (accounts.account_number); resolved to account_id
    pub account: Option<String>,

    /// `answered` / `unanswered` / `shutdown` / `flagged`
    #[validate(custom(function = "validate_outcome"))]
    pub outcome: Option<String>,

    /// Minimum talk time in seconds
    #[validate(range(min = 0, max = 86400))]
    pub min_billsec: Option<i32>,

    /// Export format
    #[serde(default)]
    pub format: ExportFormat,

    /// Maximum number of records to export
    #[serde(default = "default_export_limit")]
    #[validate(range(min = 1, max = 1000000))]
    pub limit: i64,
}

/// Covers the longest export window (31 days ≈ 870k CDRs at Oct-2026
/// volume), so a full-month export is never silently cut short
fn default_export_limit() -> i64 {
    1_000_000
}

/// CDR statistics parameters
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct StatsParams {
    /// Account ID to get stats for
    pub account_id: Option<i32>,

    /// Start date for statistics
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub start_date: Option<DateTime<Utc>>,

    /// End date for statistics
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub end_date: Option<DateTime<Utc>>,

    /// Group by period (hour, day, week, month)
    pub group_by: Option<StatsGroupBy>,
}

/// Statistics grouping options
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StatsGroupBy {
    /// Group by hour
    Hour,
    /// Group by day
    Day,
    /// Group by week
    Week,
    /// Group by month
    Month,
}

/// CDR direction filter
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CdrDirection {
    /// Inbound calls
    Inbound,
    /// Outbound calls
    Outbound,
    /// Internal calls
    Internal,
}

impl CdrDirection {
    /// Convert to database string value
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::Internal => "internal",
        }
    }
}

/// CDR statistics response
#[derive(Debug, Clone, Serialize)]
pub struct CdrStats {
    /// Total number of calls
    pub total_calls: i64,

    /// Number of answered calls
    pub answered_calls: i64,

    /// Number of failed calls
    pub failed_calls: i64,

    /// Answer-seizure ratio (ASR) as percentage
    pub asr: Decimal,

    /// Total duration in seconds
    pub total_duration: i64,

    /// Total billable seconds
    pub total_billsec: i64,

    /// Average call duration in seconds
    pub avg_duration: Decimal,

    /// Average billable duration in seconds
    pub avg_billsec: Decimal,

    /// Breakdown by period (if grouped)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_series: Option<Vec<TimeSeriesPoint>>,
}

/// Time series data point for grouped statistics
#[derive(Debug, Clone, Serialize)]
pub struct TimeSeriesPoint {
    /// Period label (e.g., "2024-01-20", "2024-01-20 14:00")
    pub period: String,

    /// Timestamp of period start
    pub timestamp: DateTime<Utc>,

    /// Number of calls in this period
    pub calls: i64,

    /// Total duration in seconds
    pub duration: i64,
}

/// CDR response DTO (simplified for API responses)
#[derive(Debug, Clone, Serialize)]
pub struct CdrResponse {
    /// CDR ID
    pub id: i64,

    /// Call UUID
    pub call_uuid: String,

    /// Account ID
    pub account_id: Option<i32>,

    /// Caller number
    pub caller: String,

    /// Called number
    pub callee: String,

    /// Destination prefix
    pub destination: Option<String>,

    /// Call start time
    pub start_time: DateTime<Utc>,

    /// Call answer time
    pub answer_time: Option<DateTime<Utc>>,

    /// Call end time
    pub end_time: DateTime<Utc>,

    /// Total duration in seconds
    pub duration: i32,

    /// Billable duration in seconds
    pub billsec: i32,

    /// Hangup cause
    pub hangup_cause: String,

    /// Call direction
    pub direction: String,

    /// Was the call answered
    pub answered: bool,

    /// Caller in E.164
    pub caller_e164: Option<String>,

    /// Final SIP status sent on the a-leg (None before 2026-10-25)
    pub sip_code: Option<i16>,

    /// IP that sent the INVITE
    pub source_ip: Option<String>,

    /// Number the call left the SBC with (b-leg To user), its next hop IP
    /// and gateway; an egress_number without egress_ip was rebuilt from the
    /// dialplan (history before 2026-10-25)
    pub egress_number: Option<String>,
    pub egress_ip: Option<String>,
    pub egress_gateway: Option<String>,

    /// Customer number / name (filled by the handler)
    pub account_number: Option<String>,
    pub account_name: Option<String>,
}

impl From<apolo_core::models::Cdr> for CdrResponse {
    fn from(cdr: apolo_core::models::Cdr) -> Self {
        let answered = cdr.was_answered();
        Self {
            id: cdr.id,
            call_uuid: cdr.call_uuid,
            account_id: cdr.account_id,
            caller: cdr.caller_number,
            callee: cdr.called_number,
            destination: cdr.destination_prefix,
            start_time: cdr.start_time,
            answer_time: cdr.answer_time,
            end_time: cdr.end_time,
            duration: cdr.duration,
            billsec: cdr.billsec,
            hangup_cause: cdr.hangup_cause,
            direction: cdr.direction,
            answered,
            caller_e164: cdr.caller_e164,
            sip_code: cdr.sip_code,
            source_ip: cdr.source_ip,
            egress_number: cdr.egress_number,
            egress_ip: cdr.egress_ip,
            egress_gateway: cdr.egress_gateway,
            account_number: None,
            account_name: None,
        }
    }
}

/// CDR export row for CSV/structured export
#[derive(Debug, Clone, Serialize)]
pub struct CdrExportRow {
    /// CDR ID
    pub id: i64,
    /// Call UUID
    pub call_uuid: String,
    /// Account ID
    pub account_id: String,
    /// Caller number
    pub caller: String,
    /// Called number
    pub callee: String,
    /// Destination prefix
    pub destination: String,
    /// Start time (ISO 8601)
    pub start_time: String,
    /// Answer time (ISO 8601)
    pub answer_time: String,
    /// End time (ISO 8601)
    pub end_time: String,
    /// Duration in seconds
    pub duration: i32,
    /// Billable seconds
    pub billsec: i32,
    /// Hangup cause
    pub hangup_cause: String,
    /// Direction
    pub direction: String,
}

impl From<apolo_core::models::Cdr> for CdrExportRow {
    fn from(cdr: apolo_core::models::Cdr) -> Self {
        Self {
            id: cdr.id,
            call_uuid: cdr.call_uuid,
            account_id: cdr
                .account_id
                .map_or_else(|| String::new(), |id| id.to_string()),
            caller: cdr.caller_number,
            callee: cdr.called_number,
            destination: cdr.destination_prefix.unwrap_or_default(),
            start_time: cdr.start_time.to_rfc3339(),
            answer_time: cdr.answer_time.map_or_else(String::new, |t| t.to_rfc3339()),
            end_time: cdr.end_time.to_rfc3339(),
            duration: cdr.duration,
            billsec: cdr.billsec,
            hangup_cause: cdr.hangup_cause,
            direction: cdr.direction,
        }
    }
}

// Custom deserializer for optional DateTime
fn deserialize_optional_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) => {
            if s.is_empty() {
                return Ok(None);
            }

            // Try parsing as ISO 8601
            if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                return Ok(Some(dt.with_timezone(&Utc)));
            }

            // Try parsing as simple date (assume start of day UTC)
            if let Ok(naive) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                let naive_dt = naive
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| Error::custom("Invalid time component"))?;
                return Ok(Some(DateTime::from_naive_utc_and_offset(naive_dt, Utc)));
            }

            Err(Error::custom(format!(
                "Invalid datetime format: {}. Expected ISO 8601 or YYYY-MM-DD",
                s
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_direction() {
        assert_eq!(CdrDirection::Inbound.as_str(), "inbound");
        assert_eq!(CdrDirection::Outbound.as_str(), "outbound");
    }

    #[test]
    fn test_export_format_default() {
        assert_eq!(ExportFormat::default(), ExportFormat::Csv);
    }

    #[test]
    fn test_cdr_response_from_cdr() {
        use apolo_core::models::Cdr;

        let cdr = Cdr::default();
        let response = CdrResponse::from(cdr.clone());

        assert_eq!(response.id, cdr.id);
        assert_eq!(response.call_uuid, cdr.call_uuid);
        assert_eq!(response.answered, cdr.was_answered());
    }
}
