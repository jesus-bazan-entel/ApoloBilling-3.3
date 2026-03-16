//! Cache key constants and builders for ApoloBilling
//!
//! Provides standardized key naming patterns for all cached entities,
//! ensuring consistency across the application and preventing key collisions.
//!
//! # Key Patterns
//!
//! - `rate:{destination}` - Cached rate cards by destination prefix
//! - `call_session:{call_uuid}` - Active call session data
//! - `active_reservations:{account_id}` - Set of active reservation IDs for an account
//! - `reservation:{reservation_id}` - Individual reservation details
//!
//! # Example
//!
//! ```
//! use apolo_cache::keys;
//!
//! let rate_key = keys::rate_key("1234");
//! assert_eq!(rate_key, "rate:1234");
//!
//! let session_key = keys::call_session_key("abc-123");
//! assert_eq!(session_key, "call_session:abc-123");
//! ```

/// Prefix for cached rate cards
///
/// Format: `rate:{destination}`
pub const RATE_KEY_PREFIX: &str = "rate";

/// Prefix for call session data
///
/// Format: `call_session:{call_uuid}`
pub const CALL_SESSION_PREFIX: &str = "call_session";

/// Prefix for active reservations set by account
///
/// Format: `active_reservations:{account_id}`
pub const ACTIVE_RESERVATIONS_PREFIX: &str = "active_reservations";

/// Prefix for individual reservation details
///
/// Format: `reservation:{reservation_id}`
pub const RESERVATION_PREFIX: &str = "reservation";

/// Prefix for account data cache
///
/// Format: `account:{account_id}` or `account:phone:{phone_number}`
pub const ACCOUNT_PREFIX: &str = "account";

/// Prefix for user session tokens
///
/// Format: `session:{token}`
pub const SESSION_PREFIX: &str = "session";

/// Default TTL for rate cards (1 hour)
pub const RATE_TTL_SECS: u64 = 3600;

/// Default TTL for call sessions (4 hours - maximum call duration)
pub const CALL_SESSION_TTL_SECS: u64 = 14400;

/// Default TTL for reservations (15 minutes)
pub const RESERVATION_TTL_SECS: u64 = 900;

/// Default TTL for active reservations set (15 minutes)
pub const ACTIVE_RESERVATIONS_TTL_SECS: u64 = 900;

/// Default TTL for account data (5 minutes)
pub const ACCOUNT_TTL_SECS: u64 = 300;

/// Default TTL for user sessions (24 hours)
pub const SESSION_TTL_SECS: u64 = 86400;

/// Build a cache key for a rate card by destination
///
/// # Arguments
///
/// * `destination` - The destination prefix (e.g., "1212", "44")
///
/// # Returns
///
/// A cache key in the format `rate:{destination}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::rate_key;
///
/// let key = rate_key("1212");
/// assert_eq!(key, "rate:1212");
/// ```
pub fn rate_key(destination: &str) -> String {
    format!("{}:{}", RATE_KEY_PREFIX, destination)
}

/// Build a cache key for a call session by UUID
///
/// # Arguments
///
/// * `call_uuid` - The unique call identifier
///
/// # Returns
///
/// A cache key in the format `call_session:{call_uuid}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::call_session_key;
///
/// let key = call_session_key("abc-123-def");
/// assert_eq!(key, "call_session:abc-123-def");
/// ```
pub fn call_session_key(call_uuid: &str) -> String {
    format!("{}:{}", CALL_SESSION_PREFIX, call_uuid)
}

/// Build a cache key for the set of active reservations for an account
///
/// # Arguments
///
/// * `account_id` - The account ID
///
/// # Returns
///
/// A cache key in the format `active_reservations:{account_id}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::active_reservations_key;
///
/// let key = active_reservations_key(123);
/// assert_eq!(key, "active_reservations:123");
/// ```
pub fn active_reservations_key(account_id: i32) -> String {
    format!("{}:{}", ACTIVE_RESERVATIONS_PREFIX, account_id)
}

/// Build a cache key for a reservation by ID
///
/// # Arguments
///
/// * `reservation_id` - The reservation UUID or ID
///
/// # Returns
///
/// A cache key in the format `reservation:{reservation_id}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::reservation_key;
///
/// let id = "550e8400-e29b-41d4-a716-446655440000";
/// let key = reservation_key(id);
/// assert_eq!(key, format!("reservation:{}", id));
/// ```
pub fn reservation_key(reservation_id: &str) -> String {
    format!("{}:{}", RESERVATION_PREFIX, reservation_id)
}

/// Build a cache key for account data by account ID
///
/// # Arguments
///
/// * `account_id` - The account ID
///
/// # Returns
///
/// A cache key in the format `account:{account_id}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::account_key;
///
/// let key = account_key(456);
/// assert_eq!(key, "account:456");
/// ```
pub fn account_key(account_id: i32) -> String {
    format!("{}:{}", ACCOUNT_PREFIX, account_id)
}

/// Build a cache key for account data by phone number
///
/// # Arguments
///
/// * `phone` - The phone number (ANI)
///
/// # Returns
///
/// A cache key in the format `account:phone:{phone}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::account_phone_key;
///
/// let key = account_phone_key("1234567890");
/// assert_eq!(key, "account:phone:1234567890");
/// ```
pub fn account_phone_key(phone: &str) -> String {
    format!("{}:phone:{}", ACCOUNT_PREFIX, phone)
}

/// Build a cache key for a user session token
///
/// # Arguments
///
/// * `token` - The session token
///
/// # Returns
///
/// A cache key in the format `session:{token}`
///
/// # Example
///
/// ```
/// use apolo_cache::keys::session_key;
///
/// let key = session_key("abc123def456");
/// assert_eq!(key, "session:abc123def456");
/// ```
pub fn session_key(token: &str) -> String {
    format!("{}:{}", SESSION_PREFIX, token)
}

/// Build a pattern for matching all keys with a given prefix
///
/// # Arguments
///
/// * `prefix` - The key prefix (e.g., "rate", "call_session")
///
/// # Returns
///
/// A pattern string in the format `{prefix}:*`
///
/// # Warning
///
/// Use with caution in production. Scanning keys can be expensive on large datasets.
/// Consider using sets or other data structures for tracking instead.
///
/// # Example
///
/// ```
/// use apolo_cache::keys::pattern;
///
/// let pattern = pattern("rate");
/// assert_eq!(pattern, "rate:*");
/// ```
pub fn pattern(prefix: &str) -> String {
    format!("{}:*", prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_key() {
        assert_eq!(rate_key("1212"), "rate:1212");
        assert_eq!(rate_key("44"), "rate:44");
        assert_eq!(rate_key(""), "rate:");
    }

    #[test]
    fn test_call_session_key() {
        assert_eq!(call_session_key("abc-123-def"), "call_session:abc-123-def");
        assert_eq!(call_session_key("uuid-here"), "call_session:uuid-here");
    }

    #[test]
    fn test_active_reservations_key() {
        assert_eq!(active_reservations_key(123), "active_reservations:123");
        assert_eq!(active_reservations_key(0), "active_reservations:0");
        assert_eq!(active_reservations_key(-1), "active_reservations:-1");
    }

    #[test]
    fn test_reservation_key() {
        assert_eq!(reservation_key("abc-123"), "reservation:abc-123");
        assert_eq!(
            reservation_key("550e8400-e29b-41d4-a716-446655440000"),
            "reservation:550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_account_key() {
        assert_eq!(account_key(456), "account:456");
        assert_eq!(account_key(1), "account:1");
    }

    #[test]
    fn test_account_phone_key() {
        assert_eq!(account_phone_key("1234567890"), "account:phone:1234567890");
        assert_eq!(
            account_phone_key("+1-555-123-4567"),
            "account:phone:+1-555-123-4567"
        );
    }

    #[test]
    fn test_session_key() {
        assert_eq!(session_key("abc123"), "session:abc123");
        assert_eq!(
            session_key("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
            "session:eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"
        );
    }

    #[test]
    fn test_pattern() {
        assert_eq!(pattern("rate"), "rate:*");
        assert_eq!(pattern("call_session"), "call_session:*");
        assert_eq!(pattern(""), ":*");
    }

    #[test]
    fn test_key_uniqueness() {
        // Ensure different key types don't collide
        let keys = vec![
            rate_key("123"),
            call_session_key("123"),
            account_key(123),
            reservation_key("123"),
            active_reservations_key(123),
        ];

        // All keys should be unique
        let unique_count = keys.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, keys.len());
    }

    #[test]
    fn test_ttl_constants() {
        // Verify TTL values are reasonable
        assert_eq!(RATE_TTL_SECS, 3600); // 1 hour
        assert_eq!(CALL_SESSION_TTL_SECS, 14400); // 4 hours
        assert_eq!(RESERVATION_TTL_SECS, 900); // 15 minutes
        assert_eq!(ACTIVE_RESERVATIONS_TTL_SECS, 900); // 15 minutes
        assert_eq!(ACCOUNT_TTL_SECS, 300); // 5 minutes
        assert_eq!(SESSION_TTL_SECS, 86400); // 24 hours
    }
}
