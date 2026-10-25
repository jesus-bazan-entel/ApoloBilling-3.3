//! Provisioning DTOs
//!
//! Request/response types for the provisioning module, which creates a billing
//! account and its associated SIP device in a single operation (individually or
//! in bulk from a CSV/Excel template).

use serde::{Deserialize, Serialize};
use validator::Validate;

/// A single provisioning request: one customer = one account + one SIP device.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ProvisionRequest {
    /// Customer name (CLIENTE) — stored on the account as account_name
    pub customer_name: Option<String>,

    /// Username — used as both the account number and the SIP username
    #[validate(length(min = 1, max = 50, message = "Username is required (1-50 chars)"))]
    pub username: String,

    /// SIP password. If omitted, a strong password is generated.
    pub password: Option<String>,

    /// SIP domain (realm). Defaults to the configured default domain if omitted.
    pub domain: Option<String>,

    /// Simultaneous registrations allowed for the SIP device (default 2, 1–10).
    pub max_registrations: Option<i32>,

    /// Order code (COD_PED) — stored on the account and used as SIP display_name
    pub cod_ped: Option<String>,

    /// Customer location: department/region
    pub departamento: Option<String>,

    /// Customer location: province
    pub provincia: Option<String>,

    /// Customer location: district
    pub distrito: Option<String>,
}

/// Result of provisioning a single customer.
#[derive(Debug, Clone, Serialize)]
pub struct ProvisionResult {
    pub account_id: i32,
    pub account_number: String,
    pub sip_device_id: i32,
    pub sip_username: String,
    pub sip_domain: String,
    /// Password used (generated or provided) — returned only on creation.
    pub password: String,
    pub max_registrations: i32,
}

/// Bulk provisioning request: a list of rows from the uploaded template.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkProvisionRequest {
    pub rows: Vec<ProvisionRequest>,
}

/// Per-row outcome status.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BulkRowStatus {
    Created,
    Skipped,
    Error,
}

/// Outcome for a single row in a bulk run.
#[derive(Debug, Clone, Serialize)]
pub struct BulkRowResult {
    /// 1-based row index, for reporting back to the user.
    pub row: usize,
    pub username: String,
    pub status: BulkRowStatus,
    /// Human-readable detail (reason skipped / error message).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sip_device_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Summary + per-row results of a bulk provisioning run.
#[derive(Debug, Clone, Serialize)]
pub struct BulkProvisionResponse {
    pub total: usize,
    pub created: usize,
    pub skipped: usize,
    pub errors: usize,
    pub results: Vec<BulkRowResult>,
}
