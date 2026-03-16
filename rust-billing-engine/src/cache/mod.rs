// src/cache/mod.rs
pub mod redis_client;

pub use redis_client::RedisClient;

use uuid::Uuid;

/// Helper for generating consistent cache keys
pub struct CacheKeys;

impl CacheKeys {
    /// Key for a reservation: `reservation:{uuid}`
    pub fn reservation(reservation_id: &Uuid) -> String {
        format!("reservation:{}", reservation_id)
    }

    /// Key for active reservations set: `active_reservations:{account_id}`
    pub fn active_reservations(account_id: i64) -> String {
        format!("active_reservations:{}", account_id)
    }

    /// Key for call session: `call_session:{uuid}`
    pub fn call_session(call_uuid: &str) -> String {
        format!("call_session:{}", call_uuid)
    }

    /// Key for rate cache: `rate:{prefix}`
    pub fn rate(prefix: &str) -> String {
        format!("rate:{}", prefix)
    }
}
