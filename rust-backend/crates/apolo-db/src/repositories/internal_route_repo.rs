use apolo_core::models::{InternalRoute, InternalRouteWithEndpoints, SystemEndpoint};
use sqlx::PgPool;
use uuid::Uuid;

/// Repository for system endpoints
pub struct SystemEndpointRepository;

impl SystemEndpointRepository {
    /// List all system endpoints
    pub async fn list(pool: &PgPool) -> Result<Vec<SystemEndpoint>, sqlx::Error> {
        sqlx::query_as!(
            SystemEndpoint,
            r#"
            SELECT id, name, endpoint_type, description, ip_address, port, transport,
                   fs_profile, fs_context, kam_gwid, kam_gw_type,
                   enabled, is_primary, created_at, updated_at
            FROM system_endpoints
            ORDER BY endpoint_type, is_primary DESC, name
            "#
        )
        .fetch_all(pool)
        .await
    }

    /// Get endpoint by ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<SystemEndpoint>, sqlx::Error> {
        sqlx::query_as!(
            SystemEndpoint,
            r#"
            SELECT id, name, endpoint_type, description, ip_address, port, transport,
                   fs_profile, fs_context, kam_gwid, kam_gw_type,
                   enabled, is_primary, created_at, updated_at
            FROM system_endpoints
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// List endpoints by type
    pub async fn list_by_type(
        pool: &PgPool,
        endpoint_type: &str,
    ) -> Result<Vec<SystemEndpoint>, sqlx::Error> {
        sqlx::query_as!(
            SystemEndpoint,
            r#"
            SELECT id, name, endpoint_type, description, ip_address, port, transport,
                   fs_profile, fs_context, kam_gwid, kam_gw_type,
                   enabled, is_primary, created_at, updated_at
            FROM system_endpoints
            WHERE endpoint_type = $1
            ORDER BY is_primary DESC, name
            "#,
            endpoint_type
        )
        .fetch_all(pool)
        .await
    }

    /// Create a new endpoint
    pub async fn create(
        pool: &PgPool,
        name: &str,
        endpoint_type: &str,
        description: Option<&str>,
        ip_address: &str,
        port: i32,
        transport: &str,
        fs_profile: Option<&str>,
        fs_context: Option<&str>,
        kam_gwid: Option<i32>,
        kam_gw_type: Option<i32>,
        is_primary: bool,
    ) -> Result<SystemEndpoint, sqlx::Error> {
        sqlx::query_as!(
            SystemEndpoint,
            r#"
            INSERT INTO system_endpoints (
                name, endpoint_type, description, ip_address, port, transport,
                fs_profile, fs_context, kam_gwid, kam_gw_type, is_primary
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, name, endpoint_type, description, ip_address, port, transport,
                      fs_profile, fs_context, kam_gwid, kam_gw_type,
                      enabled, is_primary, created_at, updated_at
            "#,
            name,
            endpoint_type,
            description,
            ip_address,
            port,
            transport,
            fs_profile,
            fs_context,
            kam_gwid,
            kam_gw_type,
            is_primary
        )
        .fetch_one(pool)
        .await
    }

    /// Update an endpoint
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        ip_address: &str,
        port: i32,
        transport: &str,
        fs_profile: Option<&str>,
        fs_context: Option<&str>,
        kam_gwid: Option<i32>,
        kam_gw_type: Option<i32>,
        enabled: bool,
        is_primary: bool,
    ) -> Result<SystemEndpoint, sqlx::Error> {
        sqlx::query_as!(
            SystemEndpoint,
            r#"
            UPDATE system_endpoints SET
                name = $2, description = $3, ip_address = $4, port = $5, transport = $6,
                fs_profile = $7, fs_context = $8, kam_gwid = $9, kam_gw_type = $10,
                enabled = $11, is_primary = $12, updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, endpoint_type, description, ip_address, port, transport,
                      fs_profile, fs_context, kam_gwid, kam_gw_type,
                      enabled, is_primary, created_at, updated_at
            "#,
            id,
            name,
            description,
            ip_address,
            port,
            transport,
            fs_profile,
            fs_context,
            kam_gwid,
            kam_gw_type,
            enabled,
            is_primary
        )
        .fetch_one(pool)
        .await
    }

    /// Delete an endpoint
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM system_endpoints WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Repository for internal routes
pub struct InternalRouteRepository;

impl InternalRouteRepository {
    /// List all internal routes with endpoint details
    pub async fn list(pool: &PgPool) -> Result<Vec<InternalRouteWithEndpoints>, sqlx::Error> {
        sqlx::query_as!(
            InternalRouteWithEndpoints,
            r#"
            SELECT
                r.id, r.name, r.description, r.route_type,
                r.source_endpoint_id,
                src.name as source_name,
                src.endpoint_type as source_type,
                src.ip_address as source_ip,
                src.port as source_port,
                r.dest_endpoint_id,
                dst.name as dest_name,
                dst.endpoint_type as dest_type,
                dst.ip_address as dest_ip,
                dst.port as dest_port,
                r.bypass_media, r.inherit_codec, r.enable_100rel, r.call_timeout,
                r.prefix_pattern, r.sync_status, r.sync_error, r.last_sync,
                r.priority, r.enabled, r.created_at, r.updated_at
            FROM internal_routes r
            LEFT JOIN system_endpoints src ON r.source_endpoint_id = src.id
            LEFT JOIN system_endpoints dst ON r.dest_endpoint_id = dst.id
            ORDER BY r.route_type, r.priority
            "#
        )
        .fetch_all(pool)
        .await
    }

    /// Get route by ID with endpoint details
    pub async fn get_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<InternalRouteWithEndpoints>, sqlx::Error> {
        sqlx::query_as!(
            InternalRouteWithEndpoints,
            r#"
            SELECT
                r.id, r.name, r.description, r.route_type,
                r.source_endpoint_id,
                src.name as source_name,
                src.endpoint_type as source_type,
                src.ip_address as source_ip,
                src.port as source_port,
                r.dest_endpoint_id,
                dst.name as dest_name,
                dst.endpoint_type as dest_type,
                dst.ip_address as dest_ip,
                dst.port as dest_port,
                r.bypass_media, r.inherit_codec, r.enable_100rel, r.call_timeout,
                r.prefix_pattern, r.sync_status, r.sync_error, r.last_sync,
                r.priority, r.enabled, r.created_at, r.updated_at
            FROM internal_routes r
            LEFT JOIN system_endpoints src ON r.source_endpoint_id = src.id
            LEFT JOIN system_endpoints dst ON r.dest_endpoint_id = dst.id
            WHERE r.id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// List routes by type
    pub async fn list_by_type(
        pool: &PgPool,
        route_type: &str,
    ) -> Result<Vec<InternalRouteWithEndpoints>, sqlx::Error> {
        sqlx::query_as!(
            InternalRouteWithEndpoints,
            r#"
            SELECT
                r.id, r.name, r.description, r.route_type,
                r.source_endpoint_id,
                src.name as source_name,
                src.endpoint_type as source_type,
                src.ip_address as source_ip,
                src.port as source_port,
                r.dest_endpoint_id,
                dst.name as dest_name,
                dst.endpoint_type as dest_type,
                dst.ip_address as dest_ip,
                dst.port as dest_port,
                r.bypass_media, r.inherit_codec, r.enable_100rel, r.call_timeout,
                r.prefix_pattern, r.sync_status, r.sync_error, r.last_sync,
                r.priority, r.enabled, r.created_at, r.updated_at
            FROM internal_routes r
            LEFT JOIN system_endpoints src ON r.source_endpoint_id = src.id
            LEFT JOIN system_endpoints dst ON r.dest_endpoint_id = dst.id
            WHERE r.route_type = $1
            ORDER BY r.priority
            "#,
            route_type
        )
        .fetch_all(pool)
        .await
    }

    /// Create a new internal route
    pub async fn create(
        pool: &PgPool,
        name: &str,
        description: Option<&str>,
        route_type: &str,
        source_endpoint_id: Option<Uuid>,
        dest_endpoint_id: Option<Uuid>,
        bypass_media: bool,
        inherit_codec: bool,
        enable_100rel: bool,
        call_timeout: i32,
        prefix_pattern: Option<&str>,
        priority: i32,
    ) -> Result<InternalRoute, sqlx::Error> {
        sqlx::query_as!(
            InternalRoute,
            r#"
            INSERT INTO internal_routes (
                name, description, route_type, source_endpoint_id, dest_endpoint_id,
                bypass_media, inherit_codec, enable_100rel, call_timeout,
                prefix_pattern, priority
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, name, description, route_type, source_endpoint_id, dest_endpoint_id,
                      bypass_media, inherit_codec, enable_100rel, call_timeout,
                      prefix_pattern, sync_status, sync_error, last_sync,
                      priority, enabled, created_at, updated_at
            "#,
            name,
            description,
            route_type,
            source_endpoint_id,
            dest_endpoint_id,
            bypass_media,
            inherit_codec,
            enable_100rel,
            call_timeout,
            prefix_pattern,
            priority
        )
        .fetch_one(pool)
        .await
    }

    /// Update an internal route
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        source_endpoint_id: Option<Uuid>,
        dest_endpoint_id: Option<Uuid>,
        bypass_media: bool,
        inherit_codec: bool,
        enable_100rel: bool,
        call_timeout: i32,
        prefix_pattern: Option<&str>,
        priority: i32,
        enabled: bool,
    ) -> Result<InternalRoute, sqlx::Error> {
        sqlx::query_as!(
            InternalRoute,
            r#"
            UPDATE internal_routes SET
                name = $2, description = $3, source_endpoint_id = $4, dest_endpoint_id = $5,
                bypass_media = $6, inherit_codec = $7, enable_100rel = $8, call_timeout = $9,
                prefix_pattern = $10, priority = $11, enabled = $12,
                sync_status = 'pending', updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, route_type, source_endpoint_id, dest_endpoint_id,
                      bypass_media, inherit_codec, enable_100rel, call_timeout,
                      prefix_pattern, sync_status, sync_error, last_sync,
                      priority, enabled, created_at, updated_at
            "#,
            id,
            name,
            description,
            source_endpoint_id,
            dest_endpoint_id,
            bypass_media,
            inherit_codec,
            enable_100rel,
            call_timeout,
            prefix_pattern,
            priority,
            enabled
        )
        .fetch_one(pool)
        .await
    }

    /// Update sync status
    pub async fn update_sync_status(
        pool: &PgPool,
        id: Uuid,
        sync_status: &str,
        sync_error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE internal_routes SET
                sync_status = $2, sync_error = $3, last_sync = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            sync_status,
            sync_error
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete an internal route
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM internal_routes WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
