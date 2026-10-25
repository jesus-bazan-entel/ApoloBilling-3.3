//! CDR (Call Detail Record) repository implementation
//!
//! Provides PostgreSQL-backed storage for call detail records with
//! optimized queries for date range filtering and UUID lookups.
//! Uses runtime queries (not compile-time macros) to avoid requiring
//! database connection at build time.

use apolo_core::{
    models::Cdr,
    traits::{
        CdrListFilter, CdrRepository, Repository, CDR_LONG_CALL_SECS, CDR_SHORT_CALL_SECS,
        CDR_SYSTEM_CUT_CAUSES,
    },
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument};
use uuid::Uuid;

/// PostgreSQL implementation of CdrRepository
pub struct PgCdrRepository {
    pool: PgPool,
}

impl PgCdrRepository {
    /// Create a new CDR repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const CDR_SELECT_COLUMNS: &str = r#"
    id, call_uuid, account_id,
    caller_number, called_number, destination_prefix,
    start_time, answer_time, end_time,
    duration, billsec,
    rate_id, rate_per_minute, cost,
    hangup_cause, direction,
    freeswitch_server_id, reservation_id,
    created_at, processed_at,
    caller_e164, sip_code, source_ip
"#;

#[async_trait]
impl Repository<Cdr, i64> for PgCdrRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i64) -> AppResult<Option<Cdr>> {
        debug!("Finding CDR by id: {}", id);

        let query = format!("SELECT {} FROM cdrs WHERE id = $1", CDR_SELECT_COLUMNS);

        let result = sqlx::query_as::<sqlx::Postgres, CdrRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding CDR {}: {}", id, e);
                AppError::Database(format!("Failed to find CDR: {}", e))
            })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<Cdr>> {
        debug!("Finding all CDRs with limit {} offset {}", limit, offset);

        let query = format!(
            "SELECT {} FROM cdrs ORDER BY start_time DESC, id DESC LIMIT $1 OFFSET $2",
            CDR_SELECT_COLUMNS
        );

        let rows = sqlx::query_as::<sqlx::Postgres, CdrRow>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding CDRs: {}", e);
                AppError::Database(format!("Failed to fetch CDRs: {}", e))
            })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cdrs")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting CDRs: {}", e);
                AppError::Database(format!("Failed to count CDRs: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &Cdr) -> AppResult<Cdr> {
        debug!("Creating CDR for call: {}", entity.call_uuid);

        let query = format!(
            r#"
            INSERT INTO cdrs (
                call_uuid, account_id,
                caller_number, called_number, destination_prefix,
                start_time, answer_time, end_time,
                duration, billsec,
                rate_id, rate_per_minute, cost,
                hangup_cause, direction,
                freeswitch_server_id, reservation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            RETURNING {}
            "#,
            CDR_SELECT_COLUMNS
        );

        let row = sqlx::query_as::<sqlx::Postgres, CdrRow>(&query)
            .bind(&entity.call_uuid)
            .bind(entity.account_id)
            .bind(&entity.caller_number)
            .bind(&entity.called_number)
            .bind(&entity.destination_prefix)
            .bind(entity.start_time)
            .bind(entity.answer_time)
            .bind(entity.end_time)
            .bind(entity.duration)
            .bind(entity.billsec)
            .bind(entity.rate_id)
            .bind(entity.rate_per_minute)
            .bind(entity.cost)
            .bind(&entity.hangup_cause)
            .bind(&entity.direction)
            .bind(&entity.freeswitch_server_id)
            .bind(entity.reservation_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error creating CDR: {}", e);
                if e.to_string().contains("unique constraint") {
                    AppError::AlreadyExists(format!(
                        "CDR for call {} already exists",
                        entity.call_uuid
                    ))
                } else {
                    AppError::Database(format!("Failed to create CDR: {}", e))
                }
            })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &Cdr) -> AppResult<Cdr> {
        debug!("Updating CDR: {}", entity.id);

        let query = format!(
            r#"
            UPDATE cdrs
            SET call_uuid = $2,
                account_id = $3,
                caller_number = $4,
                called_number = $5,
                destination_prefix = $6,
                start_time = $7,
                answer_time = $8,
                end_time = $9,
                duration = $10,
                billsec = $11,
                rate_id = $12,
                rate_per_minute = $13,
                cost = $14,
                hangup_cause = $15,
                direction = $16,
                freeswitch_server_id = $17,
                reservation_id = $18,
                processed_at = $19
            WHERE id = $1
            RETURNING {}
            "#,
            CDR_SELECT_COLUMNS
        );

        let row = sqlx::query_as::<sqlx::Postgres, CdrRow>(&query)
            .bind(entity.id)
            .bind(&entity.call_uuid)
            .bind(entity.account_id)
            .bind(&entity.caller_number)
            .bind(&entity.called_number)
            .bind(&entity.destination_prefix)
            .bind(entity.start_time)
            .bind(entity.answer_time)
            .bind(entity.end_time)
            .bind(entity.duration)
            .bind(entity.billsec)
            .bind(entity.rate_id)
            .bind(entity.rate_per_minute)
            .bind(entity.cost)
            .bind(&entity.hangup_cause)
            .bind(&entity.direction)
            .bind(&entity.freeswitch_server_id)
            .bind(entity.reservation_id)
            .bind(entity.processed_at)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error updating CDR {}: {}", entity.id, e);
                AppError::Database(format!("Failed to update CDR: {}", e))
            })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i64) -> AppResult<bool> {
        debug!("Deleting CDR: {}", id);

        let result = sqlx::query("DELETE FROM cdrs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting CDR {}: {}", id, e);
                AppError::Database(format!("Failed to delete CDR: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl CdrRepository for PgCdrRepository {
    #[instrument(skip(self))]
    async fn find_by_uuid(&self, uuid: &str) -> AppResult<Option<Cdr>> {
        debug!("Finding CDR by UUID: {}", uuid);

        let query = format!(
            "SELECT {} FROM cdrs WHERE call_uuid = $1",
            CDR_SELECT_COLUMNS
        );

        let result = sqlx::query_as::<sqlx::Postgres, CdrRow>(&query)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding CDR by UUID: {}", e);
                AppError::Database(format!("Failed to find CDR: {}", e))
            })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn list_filtered(
        &self,
        account_id: Option<i32>,
        caller: Option<&str>,
        callee: Option<&str>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        filter: &CdrListFilter,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Cdr>, i64)> {
        debug!(
            "Listing CDRs with filters: account_id={:?}, caller={:?}, callee={:?}, limit={}, offset={}",
            account_id, caller, callee, limit, offset
        );

        // Build parameterized dynamic query
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();
        let mut param_idx = 1;

        if let Some(aid) = account_id {
            conditions.push(format!("account_id = ${}", param_idx));
            params.push(aid.to_string());
            param_idx += 1;
        }

        if let Some(c) = caller {
            conditions.push(format!("caller_number LIKE ${}", param_idx));
            params.push(format!("%{}%", c)); // Buscar en cualquier parte del número
            param_idx += 1;
        }

        if let Some(c) = callee {
            conditions.push(format!("called_number LIKE ${}", param_idx));
            params.push(format!("%{}%", c)); // Buscar en cualquier parte del número
            param_idx += 1;
        }

        if let Some(start) = start_date {
            conditions.push(format!("start_time >= ${}", param_idx));
            params.push(start.to_rfc3339());
            param_idx += 1;
        }

        if let Some(end) = end_date {
            conditions.push(format!("start_time <= ${}", param_idx));
            params.push(end.to_rfc3339());
            param_idx += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Count query
        let count_query = format!("SELECT COUNT(*) FROM cdrs {}", where_clause);

        // Data query
        let data_query = format!(
            "SELECT {} FROM cdrs {} ORDER BY start_time DESC, id DESC LIMIT ${} OFFSET ${}",
            CDR_SELECT_COLUMNS,
            where_clause,
            param_idx,
            param_idx + 1
        );

        // Execute count query with dynamic bindings
        let mut count_query_builder = sqlx::query_as::<sqlx::Postgres, (i64,)>(&count_query);
        for (i, _) in params.iter().enumerate() {
            if i < conditions.len() {
                // We need to bind the correct type for each parameter
                if let Some(aid) = account_id {
                    if i == 0 && conditions[0].contains("account_id") {
                        count_query_builder = count_query_builder.bind(aid);
                        continue;
                    }
                }
            }
        }

        let raw_where = cdr_where(account_id, caller, callee, start_date, end_date, filter);

        let count_sql = format!("SELECT COUNT(*) FROM cdrs {}", raw_where);
        let data_sql = format!(
            "SELECT {} FROM cdrs {} ORDER BY start_time DESC, id DESC LIMIT {} OFFSET {}",
            CDR_SELECT_COLUMNS, raw_where, limit, offset
        );

        // Get total count
        let total: (i64,) = sqlx::query_as(&count_sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting filtered CDRs: {}", e);
                AppError::Database(format!("Failed to count CDRs: {}", e))
            })?;

        // Get CDRs
        let rows = sqlx::query_as::<sqlx::Postgres, CdrRow>(&data_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error fetching filtered CDRs: {}", e);
                AppError::Database(format!("Failed to fetch CDRs: {}", e))
            })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }
}

/// WHERE clause shared by the CDR list, its summary and the export. Values
/// are inlined with quotes escaped (standard_conforming_strings is on).
pub fn cdr_where(
    account_id: Option<i32>,
    caller: Option<&str>,
    callee: Option<&str>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    filter: &CdrListFilter,
) -> String {
    let mut parts = Vec::new();

    if let Some(aid) = account_id {
        parts.push(format!("account_id = {}", aid));
    }
    if let Some(c) = caller {
        let c = c.replace('\'', "''");
        parts.push(format!(
            "(caller_number LIKE '{c}%' OR caller_e164 LIKE '%{c}%')"
        ));
    }
    // The dialed number carries Integratel's routing prefix (3122…), so the
    // callee search also looks at the routed E.164 destination
    if let Some(c) = callee {
        let c = c.replace('\'', "''");
        parts.push(format!(
            "(called_number LIKE '{c}%' OR destination_prefix LIKE '%{c}%')"
        ));
    }
    // RFC 3339 carries the offset; a bare timestamp would be read in the
    // session TimeZone
    if let Some(start) = start_date {
        parts.push(format!("start_time >= '{}'", start.to_rfc3339()));
    }
    if let Some(end) = end_date {
        parts.push(format!("start_time <= '{}'", end.to_rfc3339()));
    }
    if let Some(d) = filter.direction.as_deref().filter(|d| !d.is_empty()) {
        parts.push(format!(
            "LOWER(direction) = '{}'",
            d.to_lowercase().replace('\'', "''")
        ));
    }
    if let Some(c) = filter.hangup_cause.as_deref().filter(|c| !c.is_empty()) {
        parts.push(format!(
            "UPPER(hangup_cause) LIKE '%{}%'",
            c.to_uppercase().replace('\'', "''")
        ));
    }
    if filter.answered_only {
        parts.push("answer_time IS NOT NULL AND billsec > 0".to_string());
    }
    match filter.outcome.as_deref() {
        Some("answered") => parts.push("answer_time IS NOT NULL".to_string()),
        Some("unanswered") => parts.push("answer_time IS NULL".to_string()),
        Some("shutdown") => parts.push(format!("hangup_cause IN ({})", system_cut_list())),
        Some("flagged") => parts.push(format!(
            "answer_time IS NOT NULL AND (billsec >= {} OR billsec <= {})",
            CDR_LONG_CALL_SECS, CDR_SHORT_CALL_SECS
        )),
        _ => {}
    }
    if let Some(min) = filter.min_billsec {
        parts.push(format!("billsec >= {}", min));
    }
    if let Some(max_id) = filter.max_id {
        parts.push(format!("id <= {}", max_id));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", parts.join(" AND "))
    }
}

/// `'SYSTEM_SHUTDOWN', 'CRASH'` for an SQL IN list
pub fn system_cut_list() -> String {
    CDR_SYSTEM_CUT_CAUSES
        .iter()
        .map(|c| format!("'{}'", c))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Totals of a filtered CDR set for the page header
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CdrSummary {
    pub total: i64,
    pub answered: i64,
    pub billsec: i64,
    pub without_customer: i64,
    pub long_calls: i64,
    pub short_answered: i64,
    pub system_cut: i64,
    pub system_cut_answered: i64,
    /// Calls cut by a platform restart, grouped by the minute they ended
    pub system_cut_minutes: Vec<(DateTime<Utc>, i64, i64)>,
}

impl PgCdrRepository {
    /// Highest CDR id right now (snapshot for stable paging)
    pub async fn max_id(&self) -> AppResult<i64> {
        let row: (Option<i64>,) = sqlx::query_as("SELECT max(id) FROM cdrs")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to read max CDR id: {}", e)))?;
        Ok(row.0.unwrap_or(0))
    }

    /// Totals for a filtered set (same WHERE as `list_filtered`)
    pub async fn summary(&self, where_clause: &str) -> AppResult<CdrSummary> {
        let cut = system_cut_list();
        let totals_sql = format!(
            "SELECT COUNT(*)::BIGINT,
                    COUNT(answer_time)::BIGINT,
                    COALESCE(SUM(billsec) FILTER (WHERE answer_time IS NOT NULL), 0)::BIGINT,
                    COUNT(*) FILTER (WHERE account_id IS NULL)::BIGINT,
                    COUNT(*) FILTER (WHERE answer_time IS NOT NULL AND billsec >= {long})::BIGINT,
                    COUNT(*) FILTER (WHERE answer_time IS NOT NULL AND billsec <= {short})::BIGINT,
                    COUNT(*) FILTER (WHERE hangup_cause IN ({cut}))::BIGINT,
                    COUNT(*) FILTER (WHERE hangup_cause IN ({cut}) AND answer_time IS NOT NULL)::BIGINT
             FROM cdrs {where_clause}",
            long = CDR_LONG_CALL_SECS,
            short = CDR_SHORT_CALL_SECS,
        );
        #[allow(clippy::type_complexity)]
        let t: (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(&totals_sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to summarize CDRs: {}", e)))?;

        let cut_where = if where_clause.is_empty() {
            format!("WHERE hangup_cause IN ({cut})")
        } else {
            format!("{where_clause} AND hangup_cause IN ({cut})")
        };
        let minutes: Vec<(DateTime<Utc>, i64, i64)> = sqlx::query_as(&format!(
            "SELECT date_trunc('minute', end_time), COUNT(*)::BIGINT, COUNT(answer_time)::BIGINT
             FROM cdrs {cut_where} GROUP BY 1 ORDER BY 1"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to group system cuts: {}", e)))?;

        Ok(CdrSummary {
            total: t.0,
            answered: t.1,
            billsec: t.2,
            without_customer: t.3,
            long_calls: t.4,
            short_answered: t.5,
            system_cut: t.6,
            system_cut_answered: t.7,
            system_cut_minutes: minutes,
        })
    }
}

/// Helper struct for mapping database rows to domain model
#[derive(Debug, sqlx::FromRow)]
struct CdrRow {
    id: i64,
    call_uuid: String,
    account_id: Option<i32>,
    caller_number: String,
    called_number: String,
    destination_prefix: Option<String>,
    start_time: DateTime<Utc>,
    answer_time: Option<DateTime<Utc>>,
    end_time: DateTime<Utc>,
    duration: i32,
    billsec: i32,
    rate_id: Option<i32>,
    rate_per_minute: Option<Decimal>,
    cost: Option<Decimal>,
    hangup_cause: String,
    direction: String,
    freeswitch_server_id: Option<String>,
    reservation_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    processed_at: Option<DateTime<Utc>>,
    caller_e164: Option<String>,
    sip_code: Option<i16>,
    source_ip: Option<String>,
}

impl From<CdrRow> for Cdr {
    fn from(row: CdrRow) -> Self {
        Self {
            id: row.id,
            call_uuid: row.call_uuid,
            account_id: row.account_id,
            caller_number: row.caller_number,
            called_number: row.called_number,
            destination_prefix: row.destination_prefix,
            start_time: row.start_time,
            answer_time: row.answer_time,
            end_time: row.end_time,
            duration: row.duration,
            billsec: row.billsec,
            rate_id: row.rate_id,
            rate_per_minute: row.rate_per_minute,
            cost: row.cost,
            hangup_cause: row.hangup_cause,
            direction: row.direction,
            freeswitch_server_id: row.freeswitch_server_id,
            reservation_id: row.reservation_id,
            created_at: row.created_at,
            processed_at: row.processed_at,
            caller_e164: row.caller_e164,
            sip_code: row.sip_code,
            source_ip: row.source_ip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_row_conversion() {
        let now = Utc::now();
        let row = CdrRow {
            id: 1,
            call_uuid: "test-uuid".to_string(),
            account_id: Some(1),
            caller_number: "51999888777".to_string(),
            called_number: "15551234567".to_string(),
            destination_prefix: Some("1555".to_string()),
            start_time: now,
            answer_time: Some(now),
            end_time: now,
            duration: 60,
            billsec: 55,
            rate_id: Some(1),
            rate_per_minute: Some(Decimal::new(10, 2)),
            cost: Some(Decimal::new(92, 2)),
            hangup_cause: "NORMAL_CLEARING".to_string(),
            direction: "outbound".to_string(),
            freeswitch_server_id: None,
            reservation_id: None,
            created_at: now,
            processed_at: None,
            caller_e164: None,
            sip_code: None,
            source_ip: None,
        };

        let cdr: Cdr = row.into();
        assert_eq!(cdr.call_uuid, "test-uuid");
        assert_eq!(cdr.duration, 60);
        assert_eq!(cdr.billsec, 55);
    }
}
