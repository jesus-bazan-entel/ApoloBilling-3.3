//! Common traits for repositories and services
//!
//! Defines abstractions for database access and business logic.

use crate::error::AppError;
use crate::models::{Account, BalanceReservation, Cdr, RateCard, User};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// Generic repository trait for CRUD operations
#[async_trait]
pub trait Repository<T, ID>: Send + Sync {
    /// Find entity by ID
    async fn find_by_id(&self, id: ID) -> Result<Option<T>, AppError>;

    /// Find all entities with pagination
    async fn find_all(&self, limit: i64, offset: i64) -> Result<Vec<T>, AppError>;

    /// Count total entities
    async fn count(&self) -> Result<i64, AppError>;

    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<T, AppError>;

    /// Update an existing entity
    async fn update(&self, entity: &T) -> Result<T, AppError>;

    /// Delete entity by ID
    async fn delete(&self, id: ID) -> Result<bool, AppError>;
}

/// Account repository trait with specialized methods
#[async_trait]
pub trait AccountRepository: Repository<Account, i32> {
    /// Find account by account number
    async fn find_by_number(&self, account_number: &str) -> Result<Option<Account>, AppError>;

    /// Find account by phone number (ANI)
    async fn find_by_phone(&self, phone: &str) -> Result<Option<Account>, AppError>;

    /// Update account balance
    async fn update_balance(&self, id: i32, amount: Decimal) -> Result<Decimal, AppError>;

    /// List accounts with filtering
    async fn list_filtered(
        &self,
        status: Option<&str>,
        account_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Account>, i64), AppError>;
}

/// Rate card repository trait with specialized methods
#[async_trait]
pub trait RateRepository: Repository<RateCard, i32> {
    /// Find rate by destination using Longest Prefix Match
    async fn find_by_destination(&self, destination: &str) -> Result<Option<RateCard>, AppError>;

    /// Search rates by prefix pattern
    async fn search(
        &self,
        prefix: Option<&str>,
        name: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RateCard>, i64), AppError>;

    /// Bulk insert rate cards
    async fn bulk_create(&self, rates: &[RateCard]) -> Result<usize, AppError>;
}

/// CDR repository trait with specialized methods
#[async_trait]
pub trait CdrRepository: Repository<Cdr, i64> {
    /// Find CDR by call UUID
    async fn find_by_uuid(&self, uuid: &str) -> Result<Option<Cdr>, AppError>;

    /// List CDRs with filtering
    async fn list_filtered(
        &self,
        account_id: Option<i32>,
        caller: Option<&str>,
        callee: Option<&str>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Cdr>, i64), AppError>;
}

/// User repository trait with specialized methods
#[async_trait]
pub trait UserRepository: Repository<User, i32> {
    /// Find user by username
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError>;

    /// Find user by email
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError>;

    /// Update last login timestamp
    async fn update_last_login(&self, id: i32) -> Result<(), AppError>;
}

/// Reservation repository trait with specialized methods
#[async_trait]
pub trait ReservationRepository: Repository<BalanceReservation, Uuid> {
    /// Find reservation by call UUID
    async fn find_by_call_uuid(
        &self,
        call_uuid: &str,
    ) -> Result<Option<BalanceReservation>, AppError>;

    /// Find active reservations for account
    async fn find_active_by_account(
        &self,
        account_id: i32,
    ) -> Result<Vec<BalanceReservation>, AppError>;

    /// Count active reservations for account
    async fn count_active_by_account(&self, account_id: i32) -> Result<i64, AppError>;

    /// Update reservation status
    async fn update_status(
        &self,
        id: Uuid,
        status: crate::models::ReservationStatus,
        consumed: Option<Decimal>,
        released: Option<Decimal>,
    ) -> Result<BalanceReservation, AppError>;

    /// Expire old reservations
    async fn expire_old(&self) -> Result<i64, AppError>;
}

/// Rating service trait
#[async_trait]
pub trait RatingService: Send + Sync {
    /// Find rate for a destination
    async fn find_rate(&self, destination: &str) -> Result<Option<RateCard>, AppError>;

    /// Calculate cost for a destination and duration
    async fn calculate_cost(
        &self,
        destination: &str,
        duration_seconds: i32,
    ) -> Result<Decimal, AppError>;
}

/// Authorization response for billing
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    pub authorized: bool,
    pub reason: String,
    pub account_id: Option<i32>,
    pub reservation_id: Option<Uuid>,
    pub reserved_amount: Option<Decimal>,
    pub max_duration_seconds: Option<i32>,
    pub rate_per_minute: Option<Decimal>,
}

/// Billing service trait
#[async_trait]
pub trait BillingService: Send + Sync {
    /// Authorize a call
    async fn authorize(
        &self,
        caller: &str,
        callee: &str,
        call_uuid: Option<&str>,
    ) -> Result<AuthorizationResult, AppError>;

    /// Consume a reservation (charge for call)
    async fn consume(
        &self,
        call_uuid: &str,
        actual_cost: Decimal,
        actual_billsec: i32,
    ) -> Result<ConsumeResult, AppError>;

    /// Release unused reservation
    async fn release(&self, call_uuid: &str) -> Result<Decimal, AppError>;
}

/// Consumption result
#[derive(Debug, Clone)]
pub struct ConsumeResult {
    pub success: bool,
    pub total_reserved: Decimal,
    pub consumed: Decimal,
    pub released: Decimal,
    pub deficit: Option<Decimal>,
}

/// Cache service trait
#[async_trait]
pub trait CacheService: Send + Sync {
    /// Get value from cache
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, AppError>;

    /// Set value in cache with TTL
    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<(), AppError>;

    /// Delete value from cache
    async fn delete(&self, key: &str) -> Result<bool, AppError>;

    /// Check if key exists
    async fn exists(&self, key: &str) -> Result<bool, AppError>;

    /// Add to set
    async fn sadd(&self, key: &str, member: &str) -> Result<bool, AppError>;

    /// Remove from set
    async fn srem(&self, key: &str, member: &str) -> Result<bool, AppError>;

    /// Count set members
    async fn scard(&self, key: &str) -> Result<i64, AppError>;

    /// Set expiration
    async fn expire(&self, key: &str, ttl_secs: u64) -> Result<bool, AppError>;
}

/// Pagination parameters
#[derive(Debug, Clone, Default)]
pub struct Pagination {
    pub page: i64,
    pub per_page: i64,
}

impl Pagination {
    pub fn new(page: i64, per_page: i64) -> Self {
        Self {
            page: page.max(1),
            per_page: per_page.clamp(1, 1000),
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> i64 {
        self.per_page
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationMeta,
}

/// Pagination metadata
#[derive(Debug, Clone, Serialize)]
pub struct PaginationMeta {
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

impl PaginationMeta {
    pub fn new(total: i64, page: i64, per_page: i64) -> Self {
        let total_pages = if per_page > 0 {
            (total + per_page - 1) / per_page
        } else {
            0
        };

        Self {
            total,
            page,
            per_page,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination() {
        let p = Pagination::new(1, 10);
        assert_eq!(p.offset(), 0);
        assert_eq!(p.limit(), 10);

        let p = Pagination::new(3, 20);
        assert_eq!(p.offset(), 40);
        assert_eq!(p.limit(), 20);
    }

    #[test]
    fn test_pagination_bounds() {
        let p = Pagination::new(0, 10); // page 0 becomes 1
        assert_eq!(p.page, 1);

        let p = Pagination::new(1, 2000); // per_page capped at 1000
        assert_eq!(p.per_page, 1000);
    }

    #[test]
    fn test_pagination_meta() {
        let meta = PaginationMeta::new(95, 1, 10);
        assert_eq!(meta.total_pages, 10);

        let meta = PaginationMeta::new(100, 1, 10);
        assert_eq!(meta.total_pages, 10);

        let meta = PaginationMeta::new(101, 1, 10);
        assert_eq!(meta.total_pages, 11);
    }
}
