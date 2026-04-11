//! SIP Device repository implementation
//!
//! Provides PostgreSQL-backed storage for SIP device entities with optimized queries
//! for authentication lookups via mod_xml_curl.

use apolo_core::{
    models::{FreeswitchAllowedIp, SipDevice},
    traits::{FreeswitchIpRepository, Repository, SipDeviceRepository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, error, instrument};

/// PostgreSQL implementation of SipDeviceRepository
pub struct PgSipDeviceRepository {
    pool: PgPool,
}

impl PgSipDeviceRepository {
    /// Create a new SIP device repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repository<SipDevice, i32> for PgSipDeviceRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i32) -> AppResult<Option<SipDevice>> {
        debug!("Finding SIP device by id: {}", id);

        let result = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            SELECT
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            FROM sip_devices
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding SIP device {}: {}", id, e);
            AppError::Database(format!("Failed to find SIP device: {}", e))
        })?;

        Ok(result.map(|row| row.into()))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<SipDevice>> {
        debug!("Finding all SIP devices with limit {} offset {}", limit, offset);

        let rows = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            SELECT
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            FROM sip_devices
            ORDER BY id
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding SIP devices: {}", e);
            AppError::Database(format!("Failed to fetch SIP devices: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sip_devices")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting SIP devices: {}", e);
                AppError::Database(format!("Failed to count SIP devices: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &SipDevice) -> AppResult<SipDevice> {
        debug!("Creating SIP device: {}", entity.sip_username);

        let row = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            INSERT INTO sip_devices (
                account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            "#,
        )
        .bind(entity.account_id)
        .bind(&entity.sip_username)
        .bind(&entity.sip_domain)
        .bind(&entity.password_encrypted)
        .bind(&entity.password_nonce)
        .bind(&entity.a1_hash)
        .bind(&entity.display_name)
        .bind(&entity.description)
        .bind(&entity.context)
        .bind(&entity.accountcode)
        .bind(&entity.codecs)
        .bind(entity.max_registrations)
        .bind(entity.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating SIP device: {}", e);
            if e.to_string().contains("unique_sip_user_domain") {
                AppError::AlreadyExists(format!(
                    "SIP device {}@{} already exists",
                    entity.sip_username, entity.sip_domain
                ))
            } else {
                AppError::Database(format!("Failed to create SIP device: {}", e))
            }
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &SipDevice) -> AppResult<SipDevice> {
        debug!("Updating SIP device: {}", entity.id);

        let row = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            UPDATE sip_devices
            SET account_id = $2,
                sip_username = $3,
                sip_domain = $4,
                display_name = $5,
                description = $6,
                context = $7,
                accountcode = $8,
                codecs = $9,
                max_registrations = $10,
                enabled = $11,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            "#,
        )
        .bind(entity.id)
        .bind(entity.account_id)
        .bind(&entity.sip_username)
        .bind(&entity.sip_domain)
        .bind(&entity.display_name)
        .bind(&entity.description)
        .bind(&entity.context)
        .bind(&entity.accountcode)
        .bind(&entity.codecs)
        .bind(entity.max_registrations)
        .bind(entity.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating SIP device {}: {}", entity.id, e);
            AppError::Database(format!("Failed to update SIP device: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i32) -> AppResult<bool> {
        debug!("Deleting SIP device: {}", id);

        let result = sqlx::query("DELETE FROM sip_devices WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting SIP device {}: {}", id, e);
                AppError::Database(format!("Failed to delete SIP device: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl SipDeviceRepository for PgSipDeviceRepository {
    #[instrument(skip(self))]
    async fn find_by_username_domain(
        &self,
        username: &str,
        domain: &str,
    ) -> AppResult<Option<SipDevice>> {
        debug!("Finding SIP device by username: {}@{}", username, domain);

        let result = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            SELECT
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            FROM sip_devices
            WHERE sip_username = $1 AND sip_domain = $2 AND enabled = true
            "#,
        )
        .bind(username)
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding SIP device by username: {}", e);
            AppError::Database(format!("Failed to find SIP device: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_by_account(&self, account_id: i32) -> AppResult<Vec<SipDevice>> {
        debug!("Finding SIP devices for account: {}", account_id);

        let rows = sqlx::query_as::<_, SipDeviceRow>(
            r#"
            SELECT
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            FROM sip_devices
            WHERE account_id = $1
            ORDER BY sip_username
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding SIP devices for account: {}", e);
            AppError::Database(format!("Failed to fetch SIP devices: {}", e))
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
    ) -> AppResult<(Vec<SipDevice>, i64)> {
        debug!(
            "Listing SIP devices with filters: account_id={:?}, enabled={:?}",
            account_id, enabled
        );

        // Build dynamic query
        let mut query_str = String::from(
            r#"
            SELECT
                id, account_id, sip_username, sip_domain,
                password_encrypted, password_nonce, a1_hash,
                display_name, description, context, accountcode,
                codecs, max_registrations, enabled,
                created_at, updated_at
            FROM sip_devices
            WHERE 1=1
            "#,
        );

        let mut count_query = String::from("SELECT COUNT(*) FROM sip_devices WHERE 1=1");

        if let Some(aid) = account_id {
            query_str.push_str(&format!(" AND account_id = {}", aid));
            count_query.push_str(&format!(" AND account_id = {}", aid));
        }

        if let Some(e) = enabled {
            query_str.push_str(&format!(" AND enabled = {}", e));
            count_query.push_str(&format!(" AND enabled = {}", e));
        }

        query_str.push_str(&format!(" ORDER BY id LIMIT {} OFFSET {}", limit, offset));

        // Get total count
        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting SIP devices: {}", e);
                AppError::Database(format!("Failed to count SIP devices: {}", e))
            })?;

        // Get devices
        let rows = sqlx::query_as::<_, SipDeviceRow>(&query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error fetching SIP devices: {}", e);
                AppError::Database(format!("Failed to fetch SIP devices: {}", e))
            })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }

    #[instrument(skip(self))]
    async fn username_exists(&self, username: &str, domain: &str) -> AppResult<bool> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM sip_devices WHERE sip_username = $1 AND sip_domain = $2)",
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
        id: i32,
        password_encrypted: &[u8],
        password_nonce: &[u8],
        a1_hash: &str,
    ) -> AppResult<()> {
        debug!("Updating password for SIP device: {}", id);

        sqlx::query(
            r#"
            UPDATE sip_devices
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
            error!("Database error updating SIP device password: {}", e);
            AppError::Database(format!("Failed to update password: {}", e))
        })?;

        Ok(())
    }
}

/// Helper struct for mapping database rows
#[derive(Debug, sqlx::FromRow)]
struct SipDeviceRow {
    id: i32,
    account_id: i32,
    sip_username: String,
    sip_domain: String,
    password_encrypted: Vec<u8>,
    password_nonce: Vec<u8>,
    a1_hash: String,
    display_name: Option<String>,
    description: Option<String>,
    context: String,
    accountcode: Option<String>,
    codecs: String,
    max_registrations: i32,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SipDeviceRow> for SipDevice {
    fn from(row: SipDeviceRow) -> Self {
        Self {
            id: row.id,
            account_id: row.account_id,
            sip_username: row.sip_username,
            sip_domain: row.sip_domain,
            password_encrypted: row.password_encrypted,
            password_nonce: row.password_nonce,
            a1_hash: row.a1_hash,
            display_name: row.display_name,
            description: row.description,
            context: row.context,
            accountcode: row.accountcode,
            codecs: row.codecs,
            max_registrations: row.max_registrations,
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
