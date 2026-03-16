//! User repository implementation
//!
//! Provides PostgreSQL-backed storage for user authentication and authorization.

use apolo_core::{
    models::{User, UserRole},
    traits::{Repository, UserRepository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Row};
use tracing::{debug, error, instrument};

/// PostgreSQL implementation of UserRepository
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    /// Create a new user repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Parse user role from string
    fn parse_role(s: &str) -> UserRole {
        UserRole::from_str(s).unwrap_or(UserRole::Operator)
    }
}

#[async_trait]
impl Repository<User, i32> for PgUserRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i32) -> AppResult<Option<User>> {
        debug!("Finding user by id: {}", id);

        let result = sqlx::query(
            r#"
            SELECT
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            FROM usuarios
            WHERE id = $1
            "#,
        )
        .bind(id)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding user {}: {}", id, e);
            AppError::Database(format!("Failed to find user: {}", e))
        })?;

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<User>> {
        debug!("Finding all users with limit {} offset {}", limit, offset);

        let rows = sqlx::query(
            r#"
            SELECT
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            FROM usuarios
            ORDER BY id
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding users: {}", e);
            AppError::Database(format!("Failed to fetch users: {}", e))
        })?;

        Ok(rows)
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM usuarios")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting users: {}", e);
                AppError::Database(format!("Failed to count users: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &User) -> AppResult<User> {
        debug!("Creating user: {}", entity.username);

        let row = sqlx::query(
            r#"
            INSERT INTO usuarios (
                username, password, nombre, apellido,
                email, role, activo
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            "#,
        )
        .bind(&entity.username)
        .bind(&entity.password_hash)
        .bind(&entity.nombre)
        .bind(&entity.apellido)
        .bind(&entity.email)
        .bind(entity.role.to_string())
        .bind(entity.activo)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating user: {}", e);
            if e.to_string().contains("unique constraint") {
                AppError::AlreadyExists(format!("User {} already exists", entity.username))
            } else {
                AppError::Database(format!("Failed to create user: {}", e))
            }
        })?;

        Ok(row)
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &User) -> AppResult<User> {
        debug!("Updating user: {}", entity.id);

        let row = sqlx::query(
            r#"
            UPDATE usuarios
            SET username = $2,
                password = $3,
                nombre = $4,
                apellido = $5,
                email = $6,
                role = $7,
                activo = $8,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&entity.username)
        .bind(&entity.password_hash)
        .bind(&entity.nombre)
        .bind(&entity.apellido)
        .bind(&entity.email)
        .bind(entity.role.to_string())
        .bind(entity.activo)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating user {}: {}", entity.id, e);
            AppError::Database(format!("Failed to update user: {}", e))
        })?;

        Ok(row)
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i32) -> AppResult<bool> {
        debug!("Deleting user: {}", id);

        let result = sqlx::query("DELETE FROM usuarios WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting user {}: {}", id, e);
                AppError::Database(format!("Failed to delete user: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    #[instrument(skip(self))]
    async fn find_by_username(&self, username: &str) -> AppResult<Option<User>> {
        debug!("Finding user by username: {}", username);

        let result = sqlx::query(
            r#"
            SELECT
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            FROM usuarios
            WHERE username = $1
            "#,
        )
        .bind(username)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding user by username: {}", e);
            AppError::Database(format!("Failed to find user: {}", e))
        })?;

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        debug!("Finding user by email: {}", email);

        let result = sqlx::query(
            r#"
            SELECT
                id, username, password as password_hash,
                nombre, apellido, email, role, activo,
                ultimo_login,
                COALESCE(created_at, NOW()) as created_at,
                COALESCE(updated_at, NOW()) as updated_at
            FROM usuarios
            WHERE email = $1
            "#,
        )
        .bind(email)
        .map(|row: sqlx::postgres::PgRow| User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            nombre: row.get("nombre"),
            apellido: row.get("apellido"),
            email: row.get("email"),
            role: Self::parse_role(row.get("role")),
            activo: row.get("activo"),
            ultimo_login: row.get("ultimo_login"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding user by email: {}", e);
            AppError::Database(format!("Failed to find user: {}", e))
        })?;

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn update_last_login(&self, id: i32) -> AppResult<()> {
        debug!("Updating last login for user: {}", id);

        sqlx::query(
            r#"
            UPDATE usuarios
            SET ultimo_login = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating last login for user {}: {}", id, e);
            AppError::Database(format!("Failed to update last login: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_role() {
        assert_eq!(PgUserRepository::parse_role("admin"), UserRole::Admin);
        assert_eq!(PgUserRepository::parse_role("operator"), UserRole::Operator);
        assert_eq!(
            PgUserRepository::parse_role("superadmin"),
            UserRole::Superadmin
        );
        assert_eq!(PgUserRepository::parse_role("invalid"), UserRole::Operator);
    }
}
