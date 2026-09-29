//! Module catalog and per-user module grant repository
//!
//! Backs the module-visibility system: `system_modules` is the configurable
//! catalog of menu/route modules, `usuario_modulos` holds explicit grants for
//! `Usuario` accounts. See `UserRole::effective_modules` in apolo-core for how
//! a session's visible module set is computed from these two pieces.

use apolo_core::{
    models::{SystemModule, User, UserRole},
    AppError, AppResult,
};
use sqlx::{PgPool, Row};
use tracing::{error, instrument};

/// PostgreSQL implementation of the module catalog / grant repository
pub struct PgModuleRepository {
    pool: PgPool,
}

impl PgModuleRepository {
    /// Create a new module repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_role(s: &str) -> UserRole {
        UserRole::from_str(s).unwrap_or(UserRole::Usuario)
    }

    /// List the full active module catalog, ordered for menu display.
    #[instrument(skip(self))]
    pub async fn list_catalog(&self) -> AppResult<Vec<SystemModule>> {
        let rows = sqlx::query(
            r#"
            SELECT id, module_key, nombre, seccion, min_role, always_visible, orden, activo
            FROM system_modules
            WHERE activo = true
            ORDER BY orden, id
            "#,
        )
        .map(|row: sqlx::postgres::PgRow| SystemModule {
            id: row.get("id"),
            module_key: row.get("module_key"),
            nombre: row.get("nombre"),
            seccion: row.get("seccion"),
            min_role: Self::parse_role(row.get("min_role")),
            always_visible: row.get("always_visible"),
            orden: row.get("orden"),
            activo: row.get("activo"),
        })
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error listing module catalog: {}", e);
            AppError::Database(format!("Failed to list module catalog: {}", e))
        })?;

        Ok(rows)
    }

    /// List the module keys explicitly granted to a `Usuario` account.
    #[instrument(skip(self))]
    pub async fn list_granted_keys(&self, usuario_id: i32) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT module_key FROM usuario_modulos WHERE usuario_id = $1")
                .bind(usuario_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    error!(
                        "Database error listing granted modules for user {}: {}",
                        usuario_id, e
                    );
                    AppError::Database(format!("Failed to list granted modules: {}", e))
                })?;

        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    /// Replace the full set of module grants for a `Usuario` account.
    ///
    /// The caller is responsible for validating that every key in
    /// `module_keys` is a real, usuario-eligible catalog entry (see
    /// `handlers::module::put_user_modules`).
    #[instrument(skip(self, module_keys))]
    pub async fn replace_granted(
        &self,
        usuario_id: i32,
        module_keys: &[String],
        granted_by: Option<i32>,
    ) -> AppResult<Vec<String>> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start module grant transaction: {}", e);
            AppError::Database(format!("Failed to start transaction: {}", e))
        })?;

        sqlx::query("DELETE FROM usuario_modulos WHERE usuario_id = $1")
            .bind(usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!(
                    "Database error clearing module grants for user {}: {}",
                    usuario_id, e
                );
                AppError::Database(format!("Failed to clear module grants: {}", e))
            })?;

        for key in module_keys {
            sqlx::query(
                r#"
                INSERT INTO usuario_modulos (usuario_id, module_key, otorgado_por)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(usuario_id)
            .bind(key)
            .bind(granted_by)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!(
                    "Database error granting module {} to user {}: {}",
                    key, usuario_id, e
                );
                AppError::Database(format!("Failed to grant module: {}", e))
            })?;
        }

        tx.commit().await.map_err(|e| {
            error!("Failed to commit module grant transaction: {}", e);
            AppError::Database(format!("Failed to commit transaction: {}", e))
        })?;

        Ok(module_keys.to_vec())
    }

    /// Compute the effective (visible) module set for a given user, combining
    /// the catalog with their explicit grants (if any).
    #[instrument(skip(self, user))]
    pub async fn effective_modules(&self, user: &User) -> AppResult<Vec<String>> {
        let catalog = self.list_catalog().await?;
        let granted = if user.role == UserRole::Usuario {
            self.list_granted_keys(user.id).await?
        } else {
            Vec::new()
        };

        Ok(user.role.effective_modules(&granted, &catalog))
    }
}
