//! SIP Endpoint repository implementation
//!
//! Provides PostgreSQL-backed storage for the unified `sip_endpoints` table
//! (extensions and trunks, credential- or IP-authenticated).

use apolo_core::{
    models::{AuthMode, EndpointType, FreeswitchAllowedIp, SipEndpoint},
    traits::{FreeswitchIpRepository, Repository, SipEndpointRepository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, error, instrument};
use uuid::Uuid;

/// PostgreSQL implementation of SipEndpointRepository
pub struct PgSipEndpointRepository {
    pool: PgPool,
}

impl PgSipEndpointRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const SELECT_COLUMNS: &str = r#"
    id, name, description, endpoint_type, auth_mode, account_id,
    sip_username, sip_domain, password_encrypted, password_nonce, a1_hash,
    max_registrations, max_concurrent_calls, host, port, transport, kamailio_gwid, sync_status,
    sync_error, is_default, did_ranges, context, fs_profile, display_name, accountcode,
    codecs, enabled, created_at, updated_at
"#;

#[async_trait]
impl Repository<SipEndpoint, Uuid> for PgSipEndpointRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: Uuid) -> AppResult<Option<SipEndpoint>> {
        let query = format!("SELECT {} FROM sip_endpoints WHERE id = $1", SELECT_COLUMNS);
        let result = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding SIP endpoint {}: {}", id, e);
                AppError::Database(format!("Failed to find SIP endpoint: {}", e))
            })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<SipEndpoint>> {
        let query = format!(
            "SELECT {} FROM sip_endpoints ORDER BY name LIMIT $1 OFFSET $2",
            SELECT_COLUMNS
        );
        let rows = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding SIP endpoints: {}", e);
                AppError::Database(format!("Failed to fetch SIP endpoints: {}", e))
            })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sip_endpoints")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting SIP endpoints: {}", e);
                AppError::Database(format!("Failed to count SIP endpoints: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &SipEndpoint) -> AppResult<SipEndpoint> {
        debug!("Creating SIP endpoint: {}", entity.name);

        let query = format!(
            r#"
            INSERT INTO sip_endpoints (
                name, description, endpoint_type, auth_mode, account_id,
                sip_username, sip_domain, password_encrypted, password_nonce, a1_hash,
                max_registrations, max_concurrent_calls, host, port, transport, context, fs_profile,
                display_name, accountcode, codecs, enabled, is_default, did_ranges
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
            RETURNING {}
            "#,
            SELECT_COLUMNS
        );

        let row = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(&entity.name)
            .bind(&entity.description)
            .bind(entity.endpoint_type.to_string())
            .bind(entity.auth_mode.to_string())
            .bind(entity.account_id)
            .bind(&entity.sip_username)
            .bind(&entity.sip_domain)
            .bind(&entity.password_encrypted)
            .bind(&entity.password_nonce)
            .bind(&entity.a1_hash)
            .bind(entity.max_registrations)
            .bind(entity.max_concurrent_calls)
            .bind(&entity.host)
            .bind(entity.port)
            .bind(&entity.transport)
            .bind(&entity.context)
            .bind(&entity.fs_profile)
            .bind(&entity.display_name)
            .bind(&entity.accountcode)
            .bind(&entity.codecs)
            .bind(entity.enabled)
            .bind(entity.is_default)
            .bind(&entity.did_ranges)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error creating SIP endpoint: {}", e);
                if e.to_string().contains("sip_endpoints_name_key") {
                    AppError::AlreadyExists(format!(
                        "SIP endpoint '{}' already exists",
                        entity.name
                    ))
                } else if e.to_string().contains("idx_sip_endpoints_user_domain") {
                    AppError::AlreadyExists(format!(
                        "SIP endpoint {}@{} already exists",
                        entity.sip_username.as_deref().unwrap_or(""),
                        entity.sip_domain.as_deref().unwrap_or("")
                    ))
                } else {
                    AppError::Database(format!("Failed to create SIP endpoint: {}", e))
                }
            })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &SipEndpoint) -> AppResult<SipEndpoint> {
        debug!("Updating SIP endpoint: {}", entity.id);

        let query = format!(
            r#"
            UPDATE sip_endpoints
            SET name = $2,
                description = $3,
                account_id = $4,
                sip_username = $5,
                sip_domain = $6,
                max_registrations = $7,
                max_concurrent_calls = $8,
                host = $9,
                port = $10,
                transport = $11,
                display_name = $12,
                accountcode = $13,
                codecs = $14,
                enabled = $15,
                is_default = $16,
                did_ranges = $17,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {}
            "#,
            SELECT_COLUMNS
        );

        let row = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(entity.id)
            .bind(&entity.name)
            .bind(&entity.description)
            .bind(entity.account_id)
            .bind(&entity.sip_username)
            .bind(&entity.sip_domain)
            .bind(entity.max_registrations)
            .bind(entity.max_concurrent_calls)
            .bind(&entity.host)
            .bind(entity.port)
            .bind(&entity.transport)
            .bind(&entity.display_name)
            .bind(&entity.accountcode)
            .bind(&entity.codecs)
            .bind(entity.enabled)
            .bind(entity.is_default)
            .bind(&entity.did_ranges)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error updating SIP endpoint {}: {}", entity.id, e);
                AppError::Database(format!("Failed to update SIP endpoint: {}", e))
            })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: Uuid) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM sip_endpoints WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting SIP endpoint {}: {}", id, e);
                AppError::Database(format!("Failed to delete SIP endpoint: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl SipEndpointRepository for PgSipEndpointRepository {
    #[instrument(skip(self))]
    async fn find_by_username_domain(
        &self,
        username: &str,
        domain: &str,
    ) -> AppResult<Option<SipEndpoint>> {
        let query = format!(
            "SELECT {} FROM sip_endpoints WHERE sip_username = $1 AND sip_domain = $2 AND auth_mode = 'credentials' AND enabled = true",
            SELECT_COLUMNS
        );
        let result = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(username)
            .bind(domain)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding SIP endpoint by username: {}", e);
                AppError::Database(format!("Failed to find SIP endpoint: {}", e))
            })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_by_account(&self, account_id: i32) -> AppResult<Vec<SipEndpoint>> {
        let query = format!(
            "SELECT {} FROM sip_endpoints WHERE account_id = $1 ORDER BY name",
            SELECT_COLUMNS
        );
        let rows = sqlx::query_as::<_, SipEndpointRow>(&query)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error finding SIP endpoints for account: {}", e);
                AppError::Database(format!("Failed to fetch SIP endpoints: {}", e))
            })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn list_filtered(
        &self,
        account_id: Option<i32>,
        enabled: Option<bool>,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<SipEndpoint>, i64)> {
        let mut query_str = format!("SELECT {} FROM sip_endpoints WHERE 1=1", SELECT_COLUMNS);
        let mut count_query = String::from("SELECT COUNT(*) FROM sip_endpoints WHERE 1=1");

        if let Some(aid) = account_id {
            query_str.push_str(&format!(" AND account_id = {}", aid));
            count_query.push_str(&format!(" AND account_id = {}", aid));
        }

        if let Some(e) = enabled {
            query_str.push_str(&format!(" AND enabled = {}", e));
            count_query.push_str(&format!(" AND enabled = {}", e));
        }

        query_str.push_str(&format!(" ORDER BY name LIMIT {} OFFSET {}", limit, offset));

        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting SIP endpoints: {}", e);
                AppError::Database(format!("Failed to count SIP endpoints: {}", e))
            })?;

        let rows = sqlx::query_as::<_, SipEndpointRow>(&query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error fetching SIP endpoints: {}", e);
                AppError::Database(format!("Failed to fetch SIP endpoints: {}", e))
            })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }

    #[instrument(skip(self))]
    async fn username_exists(&self, username: &str, domain: &str) -> AppResult<bool> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM sip_endpoints WHERE sip_username = $1 AND sip_domain = $2)",
        )
        .bind(username)
        .bind(domain)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error checking username existence: {}", e);
            AppError::Database(format!("Failed to check username: {}", e))
        })?;

        Ok(result.0)
    }

    #[instrument(skip(self, password_encrypted, password_nonce))]
    async fn update_password(
        &self,
        id: Uuid,
        password_encrypted: &[u8],
        password_nonce: &[u8],
        a1_hash: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE sip_endpoints
            SET password_encrypted = $2,
                password_nonce = $3,
                a1_hash = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(password_encrypted)
        .bind(password_nonce)
        .bind(a1_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating SIP endpoint password: {}", e);
            AppError::Database(format!("Failed to update password: {}", e))
        })?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn update_sync_status(
        &self,
        id: Uuid,
        kamailio_gwid: Option<i32>,
        sync_status: &str,
        sync_error: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE sip_endpoints
            SET kamailio_gwid = COALESCE($2, kamailio_gwid),
                sync_status = $3,
                sync_error = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(kamailio_gwid)
        .bind(sync_status)
        .bind(sync_error)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating SIP endpoint sync status: {}", e);
            AppError::Database(format!("Failed to update sync status: {}", e))
        })?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn clear_default_trunk(&self, except_id: Option<Uuid>) -> AppResult<()> {
        sqlx::query(
            "UPDATE sip_endpoints SET is_default = false WHERE is_default = true AND ($1::uuid IS NULL OR id != $1)",
        )
        .bind(except_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error clearing default trunk: {}", e);
            AppError::Database(format!("Failed to clear default trunk: {}", e))
        })?;

        Ok(())
    }
}

/// Helper struct for mapping database rows. Fields that aren't relevant to
/// every endpoint_type/auth_mode combination stay nullable here even where
/// a CHECK constraint or DEFAULT makes them effectively always-present in
/// practice, matching the DB schema exactly.
#[derive(Debug, sqlx::FromRow)]
struct SipEndpointRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    endpoint_type: String,
    auth_mode: String,
    account_id: Option<i32>,
    sip_username: Option<String>,
    sip_domain: Option<String>,
    password_encrypted: Option<Vec<u8>>,
    password_nonce: Option<Vec<u8>>,
    a1_hash: Option<String>,
    max_registrations: Option<i32>,
    max_concurrent_calls: Option<i32>,
    host: Option<String>,
    port: Option<i32>,
    transport: Option<String>,
    kamailio_gwid: Option<i32>,
    sync_status: Option<String>,
    sync_error: Option<String>,
    is_default: Option<bool>,
    did_ranges: Option<String>,
    context: String,
    fs_profile: String,
    display_name: Option<String>,
    accountcode: Option<String>,
    codecs: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SipEndpointRow> for SipEndpoint {
    fn from(row: SipEndpointRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            endpoint_type: row.endpoint_type.parse().unwrap_or(EndpointType::Extension),
            auth_mode: row.auth_mode.parse().unwrap_or(AuthMode::Credentials),
            account_id: row.account_id,
            sip_username: row.sip_username,
            sip_domain: row.sip_domain,
            password_encrypted: row.password_encrypted,
            password_nonce: row.password_nonce,
            a1_hash: row.a1_hash,
            max_registrations: row.max_registrations.unwrap_or(3),
            max_concurrent_calls: row.max_concurrent_calls.unwrap_or(2),
            host: row.host,
            port: row.port.unwrap_or(5060),
            transport: row.transport.unwrap_or_else(|| "udp".to_string()),
            kamailio_gwid: row.kamailio_gwid,
            sync_status: row.sync_status.unwrap_or_else(|| "pending".to_string()),
            sync_error: row.sync_error,
            is_default: row.is_default.unwrap_or(false),
            did_ranges: row.did_ranges.unwrap_or_default(),
            context: row.context,
            fs_profile: row.fs_profile,
            display_name: row.display_name,
            accountcode: row.accountcode,
            codecs: row
                .codecs
                .unwrap_or_else(|| "PCMU,PCMA,G729,opus".to_string()),
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// PostgreSQL implementation of FreeswitchIpRepository
pub struct PgFreeswitchIpRepository {
    pool: PgPool,
}

impl PgFreeswitchIpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repository<FreeswitchAllowedIp, i32> for PgFreeswitchIpRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i32) -> AppResult<Option<FreeswitchAllowedIp>> {
        let result = sqlx::query_as::<_, FreeswitchIpRow>(
            "SELECT id, ip_address::TEXT, description, enabled, created_at, updated_at
             FROM freeswitch_allowed_ips WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to find IP: {}", e)))?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<FreeswitchAllowedIp>> {
        let rows = sqlx::query_as::<_, FreeswitchIpRow>(
            "SELECT id, ip_address::TEXT, description, enabled, created_at, updated_at
             FROM freeswitch_allowed_ips ORDER BY id LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to fetch IPs: {}", e)))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM freeswitch_allowed_ips")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to count IPs: {}", e)))?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &FreeswitchAllowedIp) -> AppResult<FreeswitchAllowedIp> {
        let row = sqlx::query_as::<_, FreeswitchIpRow>(
            r#"
            INSERT INTO freeswitch_allowed_ips (ip_address, description, enabled)
            VALUES ($1::INET, $2, $3)
            RETURNING id, ip_address::TEXT, description, enabled, created_at, updated_at
            "#,
        )
        .bind(&entity.ip_address)
        .bind(&entity.description)
        .bind(entity.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") {
                AppError::AlreadyExists(format!("IP {} already exists", entity.ip_address))
            } else {
                AppError::Database(format!("Failed to create IP: {}", e))
            }
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &FreeswitchAllowedIp) -> AppResult<FreeswitchAllowedIp> {
        let row = sqlx::query_as::<_, FreeswitchIpRow>(
            r#"
            UPDATE freeswitch_allowed_ips
            SET ip_address = $2::INET, description = $3, enabled = $4, updated_at = NOW()
            WHERE id = $1
            RETURNING id, ip_address::TEXT, description, enabled, created_at, updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&entity.ip_address)
        .bind(&entity.description)
        .bind(entity.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update IP: {}", e)))?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i32) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM freeswitch_allowed_ips WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to delete IP: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl FreeswitchIpRepository for PgFreeswitchIpRepository {
    #[instrument(skip(self))]
    async fn is_ip_allowed(&self, ip: &str) -> AppResult<bool> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM freeswitch_allowed_ips WHERE ip_address = $1::INET AND enabled = true)",
        )
        .bind(ip)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to check IP: {}", e)))?;

        Ok(result.0)
    }

    #[instrument(skip(self))]
    async fn find_enabled(&self) -> AppResult<Vec<FreeswitchAllowedIp>> {
        let rows = sqlx::query_as::<_, FreeswitchIpRow>(
            "SELECT id, ip_address::TEXT, description, enabled, created_at, updated_at
             FROM freeswitch_allowed_ips WHERE enabled = true ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to fetch enabled IPs: {}", e)))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct FreeswitchIpRow {
    id: i32,
    ip_address: String,
    description: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<FreeswitchIpRow> for FreeswitchAllowedIp {
    fn from(row: FreeswitchIpRow) -> Self {
        Self {
            id: row.id,
            ip_address: row.ip_address,
            description: row.description,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
