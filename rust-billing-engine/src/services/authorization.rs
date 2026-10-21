// src/services/authorization.rs

use crate::models::{AuthRequest, AuthResponse, Account, AccountStatus};
use crate::database::DbPool;
use crate::cache::RedisClient;
use crate::services::MinuteBucketManager;
use crate::error::BillingError;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;
use rust_decimal::Decimal;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

pub struct AuthorizationService {
    db_pool: DbPool,
    redis: RedisClient,
    minute_bucket_mgr: Arc<MinuteBucketManager>,
}

impl AuthorizationService {
    pub fn new(
        db_pool: DbPool,
        redis: RedisClient,
        minute_bucket_mgr: Arc<MinuteBucketManager>,
    ) -> Self {
        Self {
            db_pool,
            redis,
            minute_bucket_mgr,
        }
    }

    pub async fn authorize(&self, req: &AuthRequest) -> Result<AuthResponse, BillingError> {
        let call_uuid = req.uuid.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let direction = req.direction.as_deref().unwrap_or("outbound");
        let source_ip = req.source_ip.as_deref();

        info!(
            "🔍 Authorizing call [v2] {}: {} → {} [{}] source_ip={:?}",
            call_uuid, req.caller, req.callee, direction, source_ip
        );

        // 🔒 Límite de llamadas simultáneas por endpoint con auth_mode=credentials
        // (extensiones y troncales que se registran por usuario/contraseña).
        // Bypassean el routing de Kamailio (RELAY_LANEXT), así que
        // CHECK_CALL_LIMIT nunca las ve: su límite
        // (sip_endpoints.max_concurrent_calls) se enforcea aquí contando
        // active_calls. Las troncales IP (auth_mode=ip) se enforcean en
        // Kamailio (ver sync_trunk_call_settings), no acá. Corre antes que el
        // billing porque es control de capacidad, no de saldo. Fail-open ante
        // errores de DB.
        if let Some(reason) = self
            .check_endpoint_concurrent_limit(&req.caller, &req.callee)
            .await
        {
            warn!(
                "🚫 Endpoint concurrent limit exceeded: {} → {} [{}]",
                req.caller, req.callee, reason
            );
            return Ok(AuthResponse {
                authorized: false,
                reason,
                uuid: call_uuid,
                account_id: None,
                account_number: None,
                reservation_id: None,
                reserved_amount: None,
                max_duration_seconds: None,
                rate_per_minute: None,
            });
        }

        // Check if outbound authorization is required (system setting)
        if direction == "outbound" {
            let auth_required = self.is_outbound_auth_required().await;
            if !auth_required {
                info!(
                    "⏭️  Outbound authorization DISABLED (system setting). Allowing call {} without billing.",
                    call_uuid
                );
                return Ok(AuthResponse {
                    authorized: true,
                    reason: "authorized_no_auth_required".to_string(),
                    uuid: call_uuid,
                    account_id: None,
                    account_number: None,
                    reservation_id: None,
                    reserved_amount: None,
                    max_duration_seconds: None,
                    rate_per_minute: None,
                });
            }
        }

        // 🔍 Check if callee is a toll-free number (0800, 0801, 1800)
        // For toll-free, the account is looked up by CALLEE (the 0800 owner pays)
        let is_toll_free = req.callee.starts_with("0800")
            || req.callee.starts_with("0801")
            || req.callee.starts_with("1800");

        // 1. Find account by ANI (caller) or by DNIS (callee) for toll-free
        // Also pass source_ip for IP-authenticated trunk lookup
        let (account, lookup_number) = if is_toll_free {
            info!("📞 Toll-free call detected: {} → {} - looking up account by callee", req.caller, req.callee);
            (self.find_account_by_ani_and_ip(&req.callee, source_ip).await?, req.callee.clone())
        } else {
            (self.find_account_by_ani_and_ip(&req.caller, source_ip).await?, req.caller.clone())
        };

        let account = match account {
            Some(acc) => acc,
            None => {
                // No account found: check if we should reject the call
                if direction == "outbound" && self.require_account_for_outbound().await {
                    warn!(
                        "🚫 No account found for {} ({}), REJECTING call (require_account_for_outbound=true)",
                        lookup_number, if is_toll_free { "callee (toll-free)" } else { "caller" }
                    );
                    return Ok(AuthResponse {
                        authorized: false,
                        reason: "no_account_associated".to_string(),
                        uuid: call_uuid,
                        account_id: None,
                        account_number: None,
                        reservation_id: None,
                        reserved_amount: None,
                        max_duration_seconds: None,
                        rate_per_minute: None,
                    });
                }
                if direction == "inbound" && self.require_account_for_inbound().await {
                    warn!(
                        "🚫 No account found for {} ({}), REJECTING call (require_account_for_inbound=true)",
                        lookup_number, if is_toll_free { "callee (toll-free)" } else { "caller" }
                    );
                    return Ok(AuthResponse {
                        authorized: false,
                        reason: "no_account_associated".to_string(),
                        uuid: call_uuid,
                        account_id: None,
                        account_number: None,
                        reservation_id: None,
                        reserved_amount: None,
                        max_duration_seconds: None,
                        rate_per_minute: None,
                    });
                }

                // No account found: authorize without billing (legacy behavior)
                info!(
                    "✅ No account found for {} ({}), authorizing WITHOUT billing",
                    lookup_number, if is_toll_free { "callee (toll-free)" } else { "caller" }
                );
                return Ok(AuthResponse {
                    authorized: true,
                    reason: "authorized_no_account".to_string(),
                    uuid: call_uuid,
                    account_id: None,
                    account_number: None,
                    reservation_id: None,
                    reserved_amount: None,
                    max_duration_seconds: None,
                    rate_per_minute: None,
                });
            }
        };

        // 2. Check account status
        if account.status != AccountStatus::Active {
            warn!("⚠️ Account {} is {:?}, authorizing WITHOUT billing", account.account_number, account.status);
            return Ok(AuthResponse {
                authorized: true,
                reason: "authorized_account_inactive".to_string(),
                uuid: call_uuid,
                account_id: Some(account.id.into()),
                account_number: Some(account.account_number),
                reservation_id: None,
                reserved_amount: None,
                max_duration_seconds: None,
                rate_per_minute: None,
            });
        }

        // 📞 INBOUND CALLS: Solo registrar, NO tarificar (EXCEPTO toll-free que SÍ se tarifica)
        if direction == "inbound" && !is_toll_free {
            info!(
                "✅ INBOUND Call AUTHORIZED (no billing): {} for account {}",
                call_uuid, account.account_number
            );

            return Ok(AuthResponse {
                authorized: true,
                reason: "authorized_inbound".to_string(),
                uuid: call_uuid,
                account_id: Some(account.id.into()),
                account_number: Some(account.account_number),
                reservation_id: None,
                reserved_amount: None,
                max_duration_seconds: None,
                rate_per_minute: None,
            });
        }

        // 📞 TOLL-FREE INBOUND: Se tarifica al dueño del número
        if is_toll_free {
            info!(
                "📞 TOLL-FREE Call: {} → {} - billing to account {} (owner of {})",
                req.caller, req.callee, account.account_number, req.callee
            );
        }

        // 3. Every account bills via the minute-bucket model (Plan Control /
        // Plan Abierto) — authorization is a minute count, not a dollar check.
        self.authorize_minute_bucket(account, &req.callee, call_uuid).await
    }

    /// Authorization for the minute-bucket model (Plan Control / Plan
    /// Abierto). Control denies brand-new calls once the bucket is
    /// exhausted; Abierto never denies. Neither branch computes a dollar
    /// reservation or a hard max-duration cap — a call already in progress
    /// when the bucket empties is never cut, its tail bills as overage when
    /// the CDR is generated (see CdrGenerator).
    async fn authorize_minute_bucket(
        &self,
        account: Account,
        callee: &str,
        call_uuid: String,
    ) -> Result<AuthResponse, BillingError> {
        match self.minute_bucket_mgr.check_and_reserve(&account, &call_uuid).await? {
            crate::services::minute_bucket_manager::BucketAuthResult::Denied { reason } => {
                warn!(
                    "🚫 Bucket call DENIED: {} for account {} ({})",
                    call_uuid, account.account_number, reason
                );
                Ok(AuthResponse {
                    authorized: false,
                    reason,
                    uuid: call_uuid,
                    account_id: Some(account.id.into()),
                    account_number: Some(account.account_number),
                    reservation_id: None,
                    reserved_amount: None,
                    max_duration_seconds: None,
                    rate_per_minute: None,
                })
            }
            crate::services::minute_bucket_manager::BucketAuthResult::Authorized { cycle_id } => {
                info!(
                    "✅ Bucket call AUTHORIZED: {} for account {} (cycle {}) -> {}",
                    call_uuid, account.account_number, cycle_id, callee
                );
                Ok(AuthResponse {
                    authorized: true,
                    reason: "authorized_bucket".to_string(),
                    uuid: call_uuid,
                    account_id: Some(account.id.into()),
                    account_number: Some(account.account_number),
                    reservation_id: None,
                    reserved_amount: None,
                    max_duration_seconds: None,
                    rate_per_minute: None,
                })
            }
        }
    }

    /// Concurrent-call limit for `auth_mode = 'credentials'` endpoints
    /// (extensions and credential-auth trunks), FreeSWITCH side.
    ///
    /// These endpoints bypass Kamailio routing (RELAY_LANEXT), so Kamailio's
    /// `CHECK_CALL_LIMIT` never sees them. This counts the endpoint's live
    /// rows in `active_calls` — both directions, as caller and as callee —
    /// and rejects the new call once `max_concurrent_calls` is reached.
    /// Only `auth_mode = 'credentials'` rows are considered: IP trunks are
    /// enforced in Kamailio (see `sync_trunk_call_settings`) and their caller
    /// IDs are external numbers that never match a sip_username anyway.
    /// Returns `Some(reason)` when the call must be rejected, `None` when it
    /// may proceed. Fail-open: DB errors log and allow the call (never drop
    /// traffic because of infra issues).
    /// NOTE: count-then-insert is not atomic; under exactly-simultaneous
    /// setup two calls can both see N-1 and both proceed (overshoot by one).
    /// Same best-effort trade-off as the rest of this service.
    pub async fn check_endpoint_concurrent_limit(
        &self,
        caller: &str,
        callee: &str,
    ) -> Option<String> {
        if caller.is_empty() && callee.is_empty() {
            return None;
        }
        let client = match self.db_pool.get().await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "❌ Endpoint limit check: DB unavailable, allowing call: {:?}",
                    e
                );
                return None;
            }
        };
        let rows = match client
            .query(
                "SELECT se.sip_username, se.max_concurrent_calls,
                        (SELECT COUNT(*) FROM active_calls ac
                         WHERE ac.calling_number = se.sip_username
                            OR ac.called_number = se.sip_username)
                 FROM sip_endpoints se
                 WHERE se.auth_mode = 'credentials'
                   AND se.sip_username IS NOT NULL
                   AND se.sip_username IN ($1, $2)",
                &[&caller, &callee],
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("❌ Endpoint limit check failed, allowing call: {:?}", e);
                return None;
            }
        };
        let mut seen = HashSet::new();
        for r in rows {
            let username: String = r.get(0);
            if !seen.insert(username.clone()) {
                continue;
            }
            let max: i32 = r.get(1);
            let active: i64 = r.get(2);
            if active >= max as i64 {
                info!(
                    "🚫 Endpoint {} at concurrent limit ({}/{} active), rejecting new call",
                    username, active, max
                );
                return Some("endpoint_concurrent_limit_exceeded".to_string());
            }
        }
        None
    }

    async fn find_account_by_ani(&self, ani: &str) -> Result<Option<Account>, BillingError> {
        let normalized = ani.replace('+', "").replace(' ', "").replace('-', "");

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let row = client
            .query_opt(
                "SELECT id, account_number, max_concurrent_calls, status::text,
                        created_at, updated_at,
                        bucket_type, overage_flat_rate_per_minute, overage_tariff_group
                FROM accounts
                WHERE (account_number = $1 OR account_number = $2)
                LIMIT 1",
                &[&ani, &normalized],
            )
            .await
            .map_err(|e| {
                error!("❌ Database error finding account: {:?}", e);
                BillingError::Database(e)
            })?;

        // The ANI rarely equals the account_number directly once calls come
        // from the unified Dispositivos SIP module: an extension/trunk's
        // sip_username is an arbitrary identifier linked to its billing
        // account via sip_endpoints.account_id, not via account_number. Fall
        // back to that link before giving up on the lookup.
        let row = match row {
            Some(r) => Some(r),
            None => client
                .query_opt(
                    "SELECT a.id, a.account_number, a.max_concurrent_calls, a.status::text,
                            a.created_at, a.updated_at,
                            a.bucket_type, a.overage_flat_rate_per_minute, a.overage_tariff_group
                    FROM sip_endpoints se
                    JOIN accounts a ON a.id = se.account_id
                    WHERE se.sip_username = $1
                    LIMIT 1",
                    &[&ani],
                )
                .await
                .map_err(|e| {
                    error!("❌ Database error finding account via sip_endpoints: {:?}", e);
                    BillingError::Database(e)
                })?,
        };

        match row {
            Some(r) => {
                let id: i32 = r.get(0);
                let account_number: String = r.get(1);
                let max_concurrent_calls: i32 = r.get(2);
                let status_str: String = r.get(3);

                // ✅ TIMESTAMP WITH TIME ZONE se deserializa directamente como DateTime<Utc>
                let created_at: DateTime<Utc> = r.get(4);
                let updated_at: DateTime<Utc> = r.get(5);
                let bucket_type: Option<String> = r.get(6);
                let overage_flat_rate_per_minute: Option<Decimal> = r.get(7);
                let overage_tariff_group: Option<String> = r.get(8);

                info!(
                    "✅ Found account: {} (ID: {}, Status: {}, BucketType: {:?})",
                    account_number, id, status_str, bucket_type
                );

                Ok(Some(Account {
                    id,
                    account_number,
                    status: AccountStatus::from_str(&status_str),
                    max_concurrent_calls: Some(max_concurrent_calls),
                    created_at,
                    updated_at,
                    bucket_type,
                    overage_flat_rate_per_minute,
                    overage_tariff_group,
                }))
            }
            None => {
                info!("ℹ️  No account found for ANI: {} (normalized: {})", ani, normalized);
                Ok(None)
            }
        }
    }

    /// Check if outbound authorization is required based on system_settings table.
    /// Caches the result in Redis for 30 seconds to avoid hitting the DB on every call.
    async fn is_outbound_auth_required(&self) -> bool {
        let cache_key = "setting:require_outbound_authorization";

        // Check Redis cache first
        if let Ok(Some(cached)) = self.redis.get(cache_key).await {
            return cached != "false";
        }

        // Query database
        let result = match self.db_pool.get().await {
            Ok(client) => {
                match client.query_opt(
                    "SELECT value FROM system_settings WHERE key = 'require_outbound_authorization'",
                    &[],
                ).await {
                    Ok(Some(row)) => {
                        let value: String = row.get(0);
                        value != "false"
                    }
                    Ok(None) => true, // Default: authorization required
                    Err(e) => {
                        error!("❌ Error checking system setting: {:?}", e);
                        true // Default to requiring auth on error
                    }
                }
            }
            Err(e) => {
                error!("❌ Error getting DB connection for system setting: {:?}", e);
                true // Default to requiring auth on error
            }
        };

        // Cache the result for 30 seconds
        let cache_value = if result { "true" } else { "false" };
        let _ = self.redis.set(cache_key, cache_value, 30).await;

        result
    }

    /// Check if outbound calls require an associated account.
    /// When enabled, calls from endpoints without account_id are rejected.
    /// Caches the result in Redis for 30 seconds.
    async fn require_account_for_outbound(&self) -> bool {
        let cache_key = "setting:require_account_for_outbound";

        // Check Redis cache first
        if let Ok(Some(cached)) = self.redis.get(cache_key).await {
            return cached == "true";
        }

        // Query database
        let result = match self.db_pool.get().await {
            Ok(client) => {
                match client.query_opt(
                    "SELECT value FROM system_settings WHERE key = 'require_account_for_outbound'",
                    &[],
                ).await {
                    Ok(Some(row)) => {
                        let value: String = row.get(0);
                        value == "true"
                    }
                    Ok(None) => false, // Default: not required
                    Err(e) => {
                        error!("❌ Error checking require_account_for_outbound setting: {:?}", e);
                        false // Default to not required on error
                    }
                }
            }
            Err(e) => {
                error!("❌ Error getting DB connection for require_account_for_outbound: {:?}", e);
                false // Default to not required on error
            }
        };

        // Cache the result for 30 seconds
        let cache_value = if result { "true" } else { "false" };
        let _ = self.redis.set(cache_key, cache_value, 30).await;

        result
    }

    /// Check if inbound calls require an associated account.
    /// When enabled, calls from endpoints without account_id are rejected.
    /// Caches the result in Redis for 30 seconds.
    async fn require_account_for_inbound(&self) -> bool {
        let cache_key = "setting:require_account_for_inbound";

        // Check Redis cache first
        if let Ok(Some(cached)) = self.redis.get(cache_key).await {
            return cached == "true";
        }

        // Query database
        let result = match self.db_pool.get().await {
            Ok(client) => {
                match client.query_opt(
                    "SELECT value FROM system_settings WHERE key = 'require_account_for_inbound'",
                    &[],
                ).await {
                    Ok(Some(row)) => {
                        let value: String = row.get(0);
                        value == "true"
                    }
                    Ok(None) => false, // Default: not required
                    Err(e) => {
                        error!("❌ Error checking require_account_for_inbound setting: {:?}", e);
                        false // Default to not required on error
                    }
                }
            }
            Err(e) => {
                error!("❌ Error getting DB connection for require_account_for_inbound: {:?}", e);
                false // Default to not required on error
            }
        };

        // Cache the result for 30 seconds
        let cache_value = if result { "true" } else { "false" };
        let _ = self.redis.set(cache_key, cache_value, 30).await;

        result
    }

    /// Find account by ANI (caller) or by SIP endpoint (username or IP).
    /// For IP-authenticated trunks, also looks up by source_ip.
    async fn find_account_by_ani_and_ip(&self, ani: &str, source_ip: Option<&str>) -> Result<Option<Account>, BillingError> {
        let normalized = ani.replace('+', "").replace(' ', "").replace('-', "");

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        // 1. Try direct account_number match
        let row = client
            .query_opt(
                "SELECT id, account_number, max_concurrent_calls, status::text,
                        created_at, updated_at,
                        bucket_type, overage_flat_rate_per_minute, overage_tariff_group
                FROM accounts
                WHERE (account_number = $1 OR account_number = $2)
                LIMIT 1",
                &[&ani, &normalized],
            )
            .await
            .map_err(|e| {
                error!("❌ Database error finding account: {:?}", e);
                BillingError::Database(e)
            })?;

        // 2. Fallback: look up by sip_username (credentials auth)
        let row = match row {
            Some(r) => Some(r),
            None => client
                .query_opt(
                    "SELECT a.id, a.account_number, a.max_concurrent_calls, a.status::text,
                            a.created_at, a.updated_at,
                            a.bucket_type, a.overage_flat_rate_per_minute, a.overage_tariff_group
                    FROM sip_endpoints se
                    JOIN accounts a ON a.id = se.account_id
                    WHERE se.sip_username = $1
                    LIMIT 1",
                    &[&ani],
                )
                .await
                .map_err(|e| {
                    error!("❌ Database error finding account via sip_endpoints: {:?}", e);
                    BillingError::Database(e)
                })?,
        };

        // 3. For IP-authenticated trunks, look up by source_ip
        let row = match row {
            Some(r) => Some(r),
            None => {
                if let Some(ip) = source_ip {
                    client
                        .query_opt(
                            "SELECT a.id, a.account_number, a.max_concurrent_calls, a.status::text,
                                    a.created_at, a.updated_at,
                                    a.bucket_type, a.overage_flat_rate_per_minute, a.overage_tariff_group
                            FROM sip_endpoints se
                            JOIN accounts a ON a.id = se.account_id
                            WHERE se.host = $1
                            LIMIT 1",
                            &[&ip],
                        )
                        .await
                        .map_err(|e| {
                            error!("❌ Database error finding account via sip_endpoints by IP: {:?}", e);
                            BillingError::Database(e)
                        })?
                } else {
                    None
                }
            }
        };

        match row {
            Some(r) => {
                let id: i32 = r.get(0);
                let account_number: String = r.get(1);
                let max_concurrent_calls: i32 = r.get(2);
                let status_str: String = r.get(3);

                // ✅ TIMESTAMP WITH TIME ZONE se deserializa directamente como DateTime<Utc>
                let created_at: DateTime<Utc> = r.get(4);
                let updated_at: DateTime<Utc> = r.get(5);
                let bucket_type: Option<String> = r.get(6);
                let overage_flat_rate_per_minute: Option<Decimal> = r.get(7);
                let overage_tariff_group: Option<String> = r.get(8);

                info!(
                    "✅ Found account: {} (ID: {}, Status: {}, BucketType: {:?})",
                    account_number, id, status_str, bucket_type
                );

                Ok(Some(Account {
                    id,
                    account_number,
                    status: AccountStatus::from_str(&status_str),
                    max_concurrent_calls: Some(max_concurrent_calls),
                    created_at,
                    updated_at,
                    bucket_type,
                    overage_flat_rate_per_minute,
                    overage_tariff_group,
                }))
            }
            None => {
                info!("ℹ️  No account found for ANI: {} (normalized: {}) source_ip: {:?}", ani, normalized, source_ip);
                Ok(None)
            }
        }
    }
}
