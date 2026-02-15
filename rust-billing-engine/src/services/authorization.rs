// src/services/authorization.rs

use crate::models::{AuthRequest, AuthResponse, Account, AccountStatus, AccountType};
use crate::database::DbPool;
use crate::cache::RedisClient;
use crate::services::ReservationManager;
use crate::error::BillingError;
use std::sync::Arc;
use uuid::Uuid;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

pub struct AuthorizationService {
    db_pool: DbPool,
    redis: RedisClient,
    reservation_mgr: Arc<ReservationManager>,
}

impl AuthorizationService {
    pub fn new(
        db_pool: DbPool,
        redis: RedisClient,
        reservation_mgr: Arc<ReservationManager>,
    ) -> Self {
        Self {
            db_pool,
            redis,
            reservation_mgr,
        }
    }

    pub async fn authorize(&self, req: &AuthRequest) -> Result<AuthResponse, BillingError> {
        let call_uuid = req.uuid.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let direction = req.direction.as_deref().unwrap_or("outbound");

        info!(
            "🔍 Authorizing call [v2] {}: {} → {} [{}]",
            call_uuid, req.caller, req.callee, direction
        );

        // 🔒 Try to acquire authorization lock to prevent duplicate reservations
        let lock_key = format!("auth_lock:{}", call_uuid);
        let lock_acquired = self.redis.setnx_ex(&lock_key, "1", 30).await.unwrap_or(false);

        if !lock_acquired {
            // Another process is authorizing this call, wait for reservation to appear
            info!("⏳ Authorization lock exists for {}, waiting for existing reservation...", call_uuid);

            // Wait up to 500ms for the reservation to appear
            for _ in 0..5 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let client = self.db_pool.get().await
                    .map_err(|e| BillingError::Internal(e.to_string()))?;

                let existing = client.query_opt(
                    "SELECT id, reserved_amount, rate_per_minute, account_id
                     FROM balance_reservations WHERE call_uuid = $1 AND status = 'active' LIMIT 1",
                    &[&call_uuid]
                ).await.map_err(|e| BillingError::Database(e))?;

                if let Some(row) = existing {
                    let reservation_id: Uuid = row.get(0);
                    let reserved_amount: Decimal = row.get(1);
                    let rate_per_minute: Decimal = row.get(2);
                    let account_id: i32 = row.get(3);

                    info!(
                        "✅ Found existing reservation {} for call {} (waited for lock)",
                        reservation_id, call_uuid
                    );

                    return Ok(AuthResponse {
                        authorized: true,
                        reason: "authorized_existing".to_string(),
                        uuid: call_uuid,
                        account_id: Some(account_id.into()),
                        account_number: None,
                        reservation_id: Some(reservation_id),
                        reserved_amount: Some(reserved_amount.to_f64().unwrap_or(0.0)),
                        max_duration_seconds: Some(324),
                        rate_per_minute: Some(rate_per_minute.to_f64().unwrap_or(0.0)),
                    });
                }
            }

            // If still no reservation after waiting, it might be denied
            warn!("⚠️ Lock exists but no reservation found for {} after waiting", call_uuid);
        }

        // 🔍 Check if callee is a toll-free number (0800, 0801, 1800)
        // For toll-free, the account is looked up by CALLEE (the 0800 owner pays)
        let is_toll_free = req.callee.starts_with("0800")
            || req.callee.starts_with("0801")
            || req.callee.starts_with("1800");

        // 1. Find account by ANI (caller) or by DNIS (callee) for toll-free
        let (account, lookup_number) = if is_toll_free {
            info!("📞 Toll-free call detected: {} → {} - looking up account by callee", req.caller, req.callee);
            (self.find_account_by_ani(&req.callee).await?, req.callee.clone())
        } else {
            (self.find_account_by_ani(&req.caller).await?, req.caller.clone())
        };

        let account = match account {
            Some(acc) => acc,
            None => {
                warn!("❌ Account not found for {}: {}", if is_toll_free { "callee (toll-free)" } else { "caller" }, lookup_number);
                return Ok(AuthResponse {
                    authorized: false,
                    reason: "account_not_found".to_string(),
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
            warn!("❌ Account {} is {:?}", account.account_number, account.status);
            return Ok(AuthResponse {
                authorized: false,
                reason: format!("account_{:?}", account.status).to_lowercase(),
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

        // 3. Get rate for destination
        let rate = match self.get_rate(&req.callee).await? {
            Some(r) => r,
            None => {
                warn!("❌ No rate found for destination: {}", req.callee);
                return Ok(AuthResponse {
                    authorized: false,
                    reason: "no_rate_found".to_string(),
                    uuid: call_uuid,
                    account_id: Some(account.id.into()),
                    account_number: Some(account.account_number),
                    reservation_id: None,
                    reserved_amount: None,
                    max_duration_seconds: None,
                    rate_per_minute: None,
                });
            }
        };

        info!(
            "📊 Rate found: {} - ${}/min",
            rate.destination_name, rate.rate_per_minute
        );

        // 4. Check if reservation already exists for this call (prevent duplicates)
        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let existing = client.query_opt(
            "SELECT id, reserved_amount FROM balance_reservations WHERE call_uuid = $1 AND status = 'active' LIMIT 1",
            &[&call_uuid]
        ).await.map_err(|e| BillingError::Database(e))?;

        if let Some(row) = existing {
            let reservation_id: Uuid = row.get(0);
            let reserved_amount: Decimal = row.get(1);
            info!(
                "⏭️  Reservation already exists for call {}: {} (${:.2})",
                call_uuid, reservation_id, reserved_amount
            );

            return Ok(AuthResponse {
                authorized: true,
                reason: "authorized_existing".to_string(),
                uuid: call_uuid,
                account_id: Some(account.id.into()),
                account_number: Some(account.account_number.clone()),
                reservation_id: Some(reservation_id),
                reserved_amount: Some(reserved_amount.to_f64().unwrap_or(0.0)),
                max_duration_seconds: Some(300), // Default 5 minutes
                rate_per_minute: Some(rate.rate_per_minute.to_f64().unwrap_or(0.0)),
            });
        }

        // 5. Create new reservation (pass account's concurrent call limit)
        let reservation_result = self.reservation_mgr
            .create_reservation(
                account.id.into(),
                &call_uuid,
                &req.callee,
                rate.rate_per_minute,
                account.max_concurrent_calls,
            )
            .await?;

        if !reservation_result.success {
            warn!("❌ Reservation failed: {}", reservation_result.reason);
            return Ok(AuthResponse {
                authorized: false,
                reason: reservation_result.reason,
                uuid: call_uuid,
                account_id: Some(account.id.into()),
                account_number: Some(account.account_number),
                reservation_id: None,
                reserved_amount: None,
                max_duration_seconds: None,
                rate_per_minute: None,
            });
        }

        // 6. AUTHORIZED ✅
        info!(
            "✅ Call AUTHORIZED: {} for account {}",
            call_uuid, account.account_number
        );

        Ok(AuthResponse {
            authorized: true,
            reason: "authorized".to_string(),
            uuid: call_uuid,
            account_id: Some(account.id.into()),
            account_number: Some(account.account_number.clone()),
            reservation_id: Some(reservation_result.reservation_id),
            reserved_amount: Some(reservation_result.reserved_amount),
            max_duration_seconds: Some(reservation_result.max_duration_seconds),
            rate_per_minute: Some(rate.rate_per_minute.to_f64().unwrap_or(0.0)),
        })
    }

    async fn find_account_by_ani(&self, ani: &str) -> Result<Option<Account>, BillingError> {
        let normalized = ani.replace('+', "").replace(' ', "").replace('-', "");

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        
        let row = client
            .query_opt(
                "SELECT id, account_number, account_type::text, balance,
                        max_concurrent_calls, status::text,
                        created_at, updated_at
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

        match row {
            Some(r) => {
                let id: i32 = r.get(0);
                let account_number: String = r.get(1);
                let account_type_str: String = r.get(2);
                let balance: Decimal = r.get(3);
                let max_concurrent_calls: i32 = r.get(4);
                let status_str: String = r.get(5);
                
                // ✅ TIMESTAMP WITH TIME ZONE se deserializa directamente como DateTime<Utc>
                let created_at: DateTime<Utc> = r.get(6);
                let updated_at: DateTime<Utc> = r.get(7);

                info!(
                    "✅ Found account: {} (ID: {}, Type: {}, Balance: ${}, Status: {})",
                    account_number, id, account_type_str, balance, status_str
                );

                Ok(Some(Account {
                    id,
                    account_number,
                    account_type: AccountType::from_str(&account_type_str),
                    balance,
                    credit_limit: Decimal::ZERO,
                    currency: "USD".to_string(),
                    status: AccountStatus::from_str(&status_str),
                    max_concurrent_calls: Some(max_concurrent_calls),
                    created_at,
                    updated_at,
                }))
            }
            None => {
                info!("ℹ️  No account found for ANI: {} (normalized: {})", ani, normalized);
                Ok(None)
            }
        }
    }

    async fn get_rate(&self, destination: &str) -> Result<Option<crate::models::RateCard>, BillingError> {
        let normalized = destination.replace('+', "");

        let cache_key = format!("rate:{}", &normalized[..std::cmp::min(10, normalized.len())]);
        if let Ok(Some(cached)) = self.redis.get(&cache_key).await {
            if let Ok(rate) = serde_json::from_str(&cached) {
                return Ok(Some(rate));
            }
        }

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        
        let mut prefixes: Vec<String> = Vec::new();
        for i in (1..=normalized.len()).rev() {
            prefixes.push(normalized[..i].to_string());
        }
        info!("🔎 Generated prefixes for {}: {:?}", normalized, prefixes);

        let row = client
            .query_opt(
                "SELECT id, destination_prefix, destination_name, rate_per_minute,
                        billing_increment, connection_fee, 
                        effective_start, effective_end, priority
                FROM rate_cards
                WHERE destination_prefix = ANY($1)
                AND effective_start <= NOW()
                AND (effective_end IS NULL OR effective_end >= NOW())
                ORDER BY LENGTH(destination_prefix) DESC, priority DESC
                LIMIT 1",
                &[&prefixes],
            )
            .await
            .map_err(|e| {
                error!("❌ Database error getting rate: {:?}", e);
                BillingError::Database(e)
            })?;

        match row {
            Some(r) => {
                // TIMESTAMP WITH TIME ZONE se deserializa directamente como DateTime<Utc>
                let effective_start: DateTime<Utc> = r.get(6);
                let effective_end: Option<DateTime<Utc>> = r.try_get(7).ok();

                let rate = crate::models::RateCard {
                    id: r.get(0),
                    destination_prefix: r.get(1),
                    destination_name: r.get(2),
                    rate_per_minute: r.get(3),
                    billing_increment: r.get(4),
                    connection_fee: r.get(5),
                    effective_start,
                    effective_end,
                    priority: r.get(8),
                };

                info!(
                    "✅ Rate card loaded: {} (${}/min, {} sec increment, priority {})",
                    rate.destination_name, rate.rate_per_minute, rate.billing_increment, rate.priority
                );

                if let Ok(json) = serde_json::to_string(&rate) {
                    let _ = self.redis.set(&cache_key, &json, 300).await;
                }

                Ok(Some(rate))
            }
            None => Ok(None),
        }
    }
}
