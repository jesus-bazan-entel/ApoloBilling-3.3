//! Data Transfer Objects for Endpoint Monitor System
//!
//! DTOs for SIP endpoint connectivity monitoring, probe results,
//! and evidence/report downloads.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Database Models
// ============================================================================

/// Probe record from the endpoint_monitor_probes table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProbeRecord {
    pub id: i64,
    pub ts: i64,
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub trunk_type: String,
    pub ip: String,
    pub port: i16,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
    pub evidence: Option<String>,
}

// ============================================================================
// API Response DTOs
// ============================================================================

/// Current status of a single endpoint (from latest probe cycle)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStatus {
    pub trunk_id: Uuid,
    pub name: String,
    pub ip: String,
    pub port: i16,
    pub trunk_type: String,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
    pub last_ts: i64,
}

/// Response for GET /monitor/endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStatusResponse {
    pub endpoints: Vec<EndpointStatus>,
    pub generated_at: i64,
}

/// A single point in the time series for charting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesPoint {
    pub t: i64, // Bucket timestamp
    pub rtt: Option<Decimal>,
    pub status: String, // 'up' or 'down' (MIN of bucket)
    pub downs: i32,     // Count of down probes in bucket
    pub n: i32,         // Total probes in bucket
}

/// Time series data for a single endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSeries {
    pub trunk_id: Uuid,
    pub name: String,
    pub ip: String,
    pub points: Vec<SeriesPoint>,
}

/// A row in the probes table (without evidence blob)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeRow {
    pub id: i64,
    pub ts: i64,
    pub trunk_id: Uuid,
    pub name: String,
    pub ip: String,
    pub trunk_type: String,
    pub port: i16,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
}

/// Response for GET /monitor/results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResultsResponse {
    pub frm: i64,
    pub to: i64,
    pub bucket_seconds: i64,
    pub generated_at: i64,
    pub series: Vec<EndpointSeries>,
    pub rows: Vec<ProbeRow>,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for GET /monitor/results
#[derive(Debug, Clone, Deserialize)]
pub struct ProbeResultsQuery {
    /// Filter by endpoint name (substring match)
    pub name: Option<String>,
    /// Filter by IP address (substring match)
    pub ip: Option<String>,
    /// Start timestamp (Unix epoch seconds)
    pub frm: Option<i64>,
    /// End timestamp (Unix epoch seconds)
    pub to: Option<i64>,
    /// Maximum rows for table (default 500, max 5000)
    pub limit: Option<i32>,
}

impl ProbeResultsQuery {
    /// Get the from timestamp, defaulting to 3 hours ago
    pub fn from_ts(&self) -> i64 {
        self.frm
            .unwrap_or_else(|| chrono::Utc::now().timestamp() - 3 * 3600)
    }

    /// Get the to timestamp, defaulting to now
    pub fn to_ts(&self) -> i64 {
        self.to.unwrap_or_else(|| chrono::Utc::now().timestamp())
    }

    /// Get the limit, clamped to 1-5000
    pub fn get_limit(&self) -> i32 {
        self.limit.unwrap_or(500).clamp(1, 5000)
    }

    /// Calculate bucket size for downsampling (~360 points max)
    pub fn bucket_seconds(&self) -> i64 {
        let span = self.to_ts() - self.from_ts();
        let raw_bucket = ((span as f64 / 360.0) / 60.0 + 1.0) as i64 * 60;
        raw_bucket.max(60)
    }
}

/// Query parameters for GET /monitor/evidence
#[derive(Debug, Clone, Deserialize)]
pub struct EvidenceQuery {
    /// Probe ID to download evidence for
    pub id: i64,
}

/// Query parameters for GET /monitor/evidence_report
#[derive(Debug, Clone, Deserialize)]
pub struct EvidenceReportQuery {
    /// Filter by endpoint name (substring match)
    pub name: Option<String>,
    /// Filter by IP address (substring match)
    pub ip: Option<String>,
    /// Start timestamp (Unix epoch seconds)
    pub frm: Option<i64>,
    /// End timestamp (Unix epoch seconds)
    pub to: Option<i64>,
    /// Include full traces in report (1 = yes)
    pub full: Option<i32>,
}

impl EvidenceReportQuery {
    /// Get the from timestamp, defaulting to 24 hours ago
    pub fn from_ts(&self) -> i64 {
        self.frm
            .unwrap_or_else(|| chrono::Utc::now().timestamp() - 24 * 3600)
    }

    /// Get the to timestamp, defaulting to now
    pub fn to_ts(&self) -> i64 {
        self.to.unwrap_or_else(|| chrono::Utc::now().timestamp())
    }

    /// Whether to include full traces
    pub fn include_full(&self) -> bool {
        self.full.unwrap_or(0) == 1
    }
}

// ============================================================================
// Internal Query Result Types
// ============================================================================

/// Row type for bucketed series query
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BucketedSeriesRow {
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub ip: String,
    pub tb: i64,
    pub rtt: Option<Decimal>,
    pub status: String,
    pub downs: i64,
    pub n: i64,
}

/// Row type for latest cycle query
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LatestProbeRow {
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub ip: String,
    pub port: i16,
    pub trunk_type: String,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
    pub ts: i64,
}

/// Row type for probe table query (without evidence)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProbeTableRow {
    pub id: i64,
    pub ts: i64,
    pub trunk_id: Uuid,
    pub trunk_name: String,
    pub ip: String,
    pub trunk_type: String,
    pub port: i16,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
}

/// Evidence record for single probe download
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EvidenceRecord {
    pub id: i64,
    pub ts: i64,
    pub trunk_name: String,
    pub ip: String,
    pub port: i16,
    pub status: String,
    pub resp_code: i16,
    pub rtt_ms: Option<Decimal>,
    pub evidence: Option<String>,
}

/// Report statistics for evidence report
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportStats {
    pub total_probes: i64,
    pub up_count: i64,
    pub down_count: i64,
    pub min_rtt: Option<Decimal>,
    pub avg_rtt: Option<Decimal>,
    pub max_rtt: Option<Decimal>,
}
