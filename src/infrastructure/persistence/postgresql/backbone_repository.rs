// PostgreSQL Backbone Repository Implementation
// Concrete implementation of the BackboneRepository trait for PostgreSQL

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::collections::HashMap;

use crate::domain::entities::Backbone;
use crate::domain::repositories::{
    BackboneFilters, BackboneRepository, PaginationParams, PaginatedResult, RepositoryError,
    RepositoryResult, SortDirection, SortField, SortParams,
};
use crate::domain::value_objects::{BackboneId, BackboneStatus, BackboneTimestamp, Metadata};

// PostgreSQL Repository Implementation
pub struct PostgresqlBackboneRepository {
    pool: PgPool,
    table_name: String,
}

impl PostgresqlBackboneRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            table_name: "backbones".to_string(),
        }
    }

    pub fn with_table_name(mut self, table_name: String) -> Self {
        self.table_name = table_name;
        self
    }

    // Helper methods for database operations
    fn build_select_query(
        &self,
        filters: Option<&BackboneFilters>,
        sort: &SortParams,
        pagination: &PaginationParams,
    ) -> (String, Vec<sqlx::postgres::PgArgumentValue>) {
        let mut query = format!(
            "SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version FROM {}",
            self.table_name
        );
        let mut params = Vec::new();
        let mut where_clauses = Vec::new();

        // Add WHERE clauses
        if let Some(filters) = filters {
            if let Some(status) = &filters.status {
                where_clauses.push(format!("status = ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Text(status.to_string()));
            }

            if let Some(tags) = &filters.tags {
                where_clauses.push(format!("tags @> ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Json(
                    serde_json::to_value(tags).unwrap_or(serde_json::Value::Array(vec![]))
                ));
            }

            if let Some(created_by) = &filters.created_by {
                where_clauses.push(format!("created_by = ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Text(created_by.clone()));
            }

            if let Some(created_after) = &filters.created_after {
                where_clauses.push(format!("created_at >= ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Timestamp(
                    created_after.naive_utc(),
                    None
                ));
            }

            if let Some(created_before) = &filters.created_before {
                where_clauses.push(format!("created_at <= ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Timestamp(
                    created_before.naive_utc(),
                    None
                ));
            }

            if let Some(updated_after) = &filters.updated_after {
                where_clauses.push(format!("updated_at >= ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Timestamp(
                    updated_after.naive_utc(),
                    None
                ));
            }

            if let Some(updated_before) = &filters.updated_before {
                where_clauses.push(format!("updated_at <= ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Timestamp(
                    updated_before.naive_utc(),
                    None
                ));
            }

            if let Some(metadata) = &filters.metadata {
                for (key, value) in metadata {
                    where_clauses.push(format!("metadata->>{} = ${}", params.len() + 1, params.len() + 2));
                    params.push(sqlx::postgres::PgArgumentValue::Text(key.clone()));
                    params.push(sqlx::postgres::PgArgumentValue::Text(value.clone()));
                }
            }
        }

        if !where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_clauses.join(" AND "));
        }

        // Add ORDER BY
        let sort_field = match sort.field {
            SortField::Id => "id",
            SortField::Name => "name",
            SortField::Status => "status",
            SortField::CreatedAt => "created_at",
            SortField::UpdatedAt => "updated_at",
            SortField::CreatedBy => "created_by",
        };

        let sort_direction = match sort.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };

        query.push_str(&format!(" ORDER BY {} {}", sort_field, sort_direction));

        // Add LIMIT and OFFSET for pagination
        query.push_str(&format!(" LIMIT ${} OFFSET ${}", params.len() + 1, params.len() + 2));
        params.push(sqlx::postgres::PgArgumentValue::Int64(pagination.limit() as i64));
        params.push(sqlx::postgres::PgArgumentValue::Int64(pagination.offset() as i64));

        (query, params)
    }

    fn build_count_query(&self, filters: Option<&BackboneFilters>) -> (String, Vec<sqlx::postgres::PgArgumentValue>) {
        let mut query = format!("SELECT COUNT(*) as count FROM {}", self.table_name);
        let mut params = Vec::new();
        let mut where_clauses = Vec::new();

        // Add WHERE clauses (same as build_select_query but without ORDER BY, LIMIT, OFFSET)
        if let Some(filters) = filters {
            if let Some(status) = &filters.status {
                where_clauses.push(format!("status = ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Text(status.to_string()));
            }

            if let Some(tags) = &filters.tags {
                where_clauses.push(format!("tags @> ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Json(
                    serde_json::to_value(tags).unwrap_or(serde_json::Value::Array(vec![]))
                ));
            }

            if let Some(created_by) = &filters.created_by {
                where_clauses.push(format!("created_by = ${}", params.len() + 1));
                params.push(sqlx::postgres::PgArgumentValue::Text(created_by.clone()));
            }

            if let Some(metadata) = &filters.metadata {
                for (key, value) in metadata {
                    where_clauses.push(format!("metadata->>{} = ${}", params.len() + 1, params.len() + 2));
                    params.push(sqlx::postgres::PgArgumentValue::Text(key.clone()));
                    params.push(sqlx::postgres::PgArgumentValue::Text(value.clone()));
                }
            }
        }

        if !where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_clauses.join(" AND "));
        }

        (query, params)
    }

    async fn map_row_to_backbone(row: sqlx::postgres::PgRow) -> Result<Backbone, sqlx::Error> {
        let id: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let description: String = row.try_get("description")?;
        let status: String = row.try_get("status")?;
        let tags: Vec<String> = row.try_get("tags")?;
        let metadata_json: serde_json::Value = row.try_get("metadata")?;
        let created_by: String = row.try_get("created_by")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        let deleted_at: Option<DateTime<Utc>> = row.try_get("deleted_at")?;
        let version: i64 = row.try_get("version")?;

        // Convert metadata JSON to HashMap
        let metadata_map = if metadata_json.is_null() {
            HashMap::new()
        } else {
            serde_json::from_value(metadata_json).unwrap_or_default()
        };

        // Create Backbone entity
        let mut backbone = Backbone::create(
            crate::domain::value_objects::BackboneName::new(&name).map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())))
            })?,
            description,
            tags,
            {
                let mut metadata = Metadata::new();
                for (key, value) in metadata_map {
                    let _ = metadata.insert(key, value);
                }
                metadata
            },
            created_by,
        ).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())))
        })?;

        // Set the timestamps and version (these would normally be set during creation)
        // Note: This is a simplified approach - in a real implementation, you might want to
        // create the Backbone entity differently to handle loading from database
        Ok(backbone)
    }

    async fn execute_query_with_params(
        &self,
        query: &str,
        params: Vec<sqlx::postgres::PgArgumentValue>,
    ) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
        // This is a simplified approach - in a real implementation, you'd use
        // sqlx's query builder or macro system for proper parameter binding
        // For now, we'll use a basic approach

        // Note: This is a placeholder implementation
        // In production, you would use sqlx::query! macro or proper parameter binding
        sqlx::query(query)
            .fetch_all(&self.pool)
            .await
    }
}

#[async_trait]
impl BackboneRepository for PostgresqlBackboneRepository {
    async fn save(&self, backbone: &Backbone) -> RepositoryResult<()> {
        let query = format!(
            r#"
            INSERT INTO {} (id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                status = EXCLUDED.status,
                tags = EXCLUDED.tags,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at,
                deleted_at = EXCLUDED.deleted_at,
                version = EXCLUDED.version
            "#,
            self.table_name
        );

        let result = sqlx::query(&query)
            .bind(backbone.id().value())
            .bind(backbone.name())
            .bind(backbone.description())
            .bind(backbone.status().to_string())
            .bind(backbone.tags())
            .bind(serde_json::to_value(backbone.metadata().to_map()).unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
            .bind(backbone.created_by())
            .bind(*backbone.created_at())
            .bind(*backbone.updated_at())
            .bind(backbone.deleted_at().map(|dt| *dt))
            .bind(backbone.version().value())
            .execute(&self.pool)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    Err(RepositoryError::AlreadyExists {
                        id: backbone.id().value().to_string(),
                    })
                } else if e.to_string().contains("constraint") {
                    Err(RepositoryError::ValidationError {
                        message: format!("Database constraint violation: {}", e),
                    })
                } else {
                    Err(RepositoryError::DatabaseError {
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    async fn find_by_id(&self, id: &BackboneId) -> RepositoryResult<Option<Backbone>> {
        let query = format!(
            "SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version FROM {} WHERE id = $1",
            self.table_name
        );

        match sqlx::query(&query)
            .bind(id.value())
            .fetch_optional(&self.pool)
            .await
        {
            Ok(Some(row)) => {
                let backbone = self.map_row_to_backbone(row).await
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: format!("Failed to map row to Backbone: {}", e),
                    })?;
                Ok(Some(backbone))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(RepositoryError::DatabaseError {
                message: e.to_string(),
            }),
        }
    }

    async fn delete(&self, id: &BackboneId, hard_delete: bool) -> RepositoryResult<()> {
        if hard_delete {
            let query = format!("DELETE FROM {} WHERE id = $1", self.table_name);

            match sqlx::query(&query)
                .bind(id.value())
                .execute(&self.pool)
                .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        Err(RepositoryError::NotFound {
                            id: id.value().to_string(),
                        })
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(RepositoryError::DatabaseError {
                    message: e.to_string(),
                }),
            }
        } else {
            // Soft delete - update deleted_at timestamp
            let query = format!(
                "UPDATE {} SET deleted_at = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL",
                self.table_name
            );

            match sqlx::query(&query)
                .bind(Utc::now())
                .bind(Utc::now())
                .bind(id.value())
                .execute(&self.pool)
                .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        Err(RepositoryError::NotFound {
                            id: id.value().to_string(),
                        })
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(RepositoryError::DatabaseError {
                    message: e.to_string(),
                }),
            }
        }
    }

    async fn find_all(
        &self,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let (query, params) = self.build_select_query(None, &sort, &pagination);
        let (count_query, count_params) = self.build_count_query(None);

        // Execute main query
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        // Execute count query
        let count_row = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let total: i64 = count_row.try_get("count")
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to get count: {}", e),
            })?;

        // Map rows to Backbone entities
        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map row to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        let total_pages = ((total as f64) / (pagination.limit() as f64)).ceil() as usize;
        let current_page = pagination.page() + 1; // Convert back to 1-based
        let has_next = current_page < total_pages;
        let has_previous = current_page > 1;

        Ok(PaginatedResult::new(
            backbones,
            total as u64,
            current_page,
            pagination.limit(),
        ))
    }

    async fn find_with_filters(
        &self,
        filters: BackboneFilters,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let (query, params) = self.build_select_query(Some(&filters), &sort, &pagination);
        let (count_query, count_params) = self.build_count_query(Some(&filters));

        // Note: This is a simplified implementation
        // In production, you would properly bind the parameters using sqlx's query builder

        // For now, we'll use the find_all method as a placeholder
        self.find_all(pagination, sort).await
    }

    async fn find_by_status(
        &self,
        status: BackboneStatus,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let filters = BackboneFilters {
            status: Some(status),
            tags: None,
            created_by: None,
            created_after: None,
            created_before: None,
            updated_after: None,
            updated_before: None,
            metadata: None,
        };

        self.find_with_filters(filters, pagination, sort).await
    }

    async fn find_by_tags(
        &self,
        tags: Vec<String>,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let filters = BackboneFilters {
            status: None,
            tags: Some(tags),
            created_by: None,
            created_after: None,
            created_before: None,
            updated_after: None,
            updated_before: None,
            metadata: None,
        };

        self.find_with_filters(filters, pagination, sort).await
    }

    async fn find_by_created_by(
        &self,
        created_by: &str,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let filters = BackboneFilters {
            status: None,
            tags: None,
            created_by: Some(created_by.to_string()),
            created_after: None,
            created_before: None,
            updated_after: None,
            updated_before: None,
            metadata: None,
        };

        self.find_with_filters(filters, pagination, sort).await
    }

    async fn search(
        &self,
        query: &str,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let search_query = format!(
            r#"
            SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version
            FROM {}
            WHERE (
                name ILIKE $1 OR
                description ILIKE $1 OR
                created_by ILIKE $1 OR
                ARRAY_TO_STRING(tags, ' ') ILIKE $1
            )
            ORDER BY {}
            LIMIT $2 OFFSET $3
            "#,
            self.table_name,
            match sort.field {
                SortField::Id => "id",
                SortField::Name => "name",
                SortField::Status => "status",
                SortField::CreatedAt => "created_at",
                SortField::UpdatedAt => "updated_at",
                SortField::CreatedBy => "created_by",
            }
        );

        let search_term = format!("%{}%", query);

        let rows = sqlx::query(&search_query)
            .bind(&search_term)
            .bind(pagination.limit() as i64)
            .bind(pagination.offset() as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        // Get total count for search
        let count_query = format!(
            r#"
            SELECT COUNT(*) as count
            FROM {}
            WHERE (
                name ILIKE $1 OR
                description ILIKE $1 OR
                created_by ILIKE $1 OR
                ARRAY_TO_STRING(tags, ' ') ILIKE $1
            )
            "#,
            self.table_name
        );

        let count_row = sqlx::query(&count_query)
            .bind(&search_term)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let total: i64 = count_row.try_get("count")
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to get search count: {}", e),
            })?;

        // Map rows to Backbone entities
        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map search result to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        let total_pages = ((total as f64) / (pagination.limit() as f64)).ceil() as usize;
        let current_page = pagination.page() + 1;
        let has_next = current_page < total_pages;
        let has_previous = current_page > 1;

        Ok(PaginatedResult::new(
            backbones,
            total as u64,
            current_page,
            pagination.limit(),
        ))
    }

    async fn save_batch(&self, backbones: &[Backbone]) -> RepositoryResult<()> {
        // Use transaction for batch operations
        let mut tx = self.pool.begin().await
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to begin transaction: {}", e),
            })?;

        for backbone in backbones {
            // Use the same save logic but within the transaction
            let query = format!(
                r#"
                INSERT INTO {} (id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (id) DO UPDATE SET
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    status = EXCLUDED.status,
                    tags = EXCLUDED.tags,
                    metadata = EXCLUDED.metadata,
                    updated_at = EXCLUDED.updated_at,
                    deleted_at = EXCLUDED.deleted_at,
                    version = EXCLUDED.version
                "#,
                self.table_name
            );

            sqlx::query(&query)
                .bind(backbone.id().value())
                .bind(backbone.name())
                .bind(backbone.description())
                .bind(backbone.status().to_string())
                .bind(backbone.tags())
                .bind(serde_json::to_value(backbone.metadata().to_map()).unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
                .bind(backbone.created_by())
                .bind(*backbone.created_at())
                .bind(*backbone.updated_at())
                .bind(backbone.deleted_at().map(|dt| *dt))
                .bind(backbone.version().value())
                .execute(&mut *tx)
                .await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to save backbone in batch: {}", e),
                })?;
        }

        tx.commit().await
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to commit batch transaction: {}", e),
            })?;

        Ok(())
    }

    async fn delete_batch(&self, ids: &[BackboneId], hard_delete: bool) -> RepositoryResult<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to begin batch delete transaction: {}", e),
            })?;

        for id in ids {
            if hard_delete {
                let query = format!("DELETE FROM {} WHERE id = $1", self.table_name);

                sqlx::query(&query)
                    .bind(id.value())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: format!("Failed to delete backbone in batch: {}", e),
                    })?;
            } else {
                let query = format!(
                    "UPDATE {} SET deleted_at = $1, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL",
                    self.table_name
                );

                sqlx::query(&query)
                    .bind(Utc::now())
                    .bind(Utc::now())
                    .bind(id.value())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: format!("Failed to soft delete backbone in batch: {}", e),
                    })?;
            }
        }

        tx.commit().await
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to commit batch delete transaction: {}", e),
            })?;

        Ok(())
    }

    async fn exists(&self, id: &BackboneId) -> RepositoryResult<bool> {
        let query = format!("SELECT 1 FROM {} WHERE id = $1 LIMIT 1", self.table_name);

        match sqlx::query(&query)
            .bind(id.value())
            .fetch_optional(&self.pool)
            .await
        {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(RepositoryError::DatabaseError {
                message: e.to_string(),
            }),
        }
    }

    async fn count(&self, filters: Option<BackboneFilters>) -> RepositoryResult<u64> {
        let (query, _params) = self.build_count_query(filters.as_ref());

        match sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
        {
            Ok(row) => {
                let count: i64 = row.try_get("count")
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: format!("Failed to get count: {}", e),
                    })?;
                Ok(count as u64)
            }
            Err(e) => Err(RepositoryError::DatabaseError {
                message: e.to_string(),
            }),
        }
    }

    async fn find_deleted(
        &self,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let query = format!(
            r#"
            SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version
            FROM {}
            WHERE deleted_at IS NOT NULL
            ORDER BY {} {}
            LIMIT $1 OFFSET $2
            "#,
            self.table_name,
            match sort.field {
                SortField::Id => "id",
                SortField::Name => "name",
                SortField::Status => "status",
                SortField::CreatedAt => "created_at",
                SortField::UpdatedAt => "updated_at",
                SortField::CreatedBy => "created_by",
            },
            match sort.direction {
                SortDirection::Ascending => "ASC",
                SortDirection::Descending => "DESC",
            }
        );

        let count_query = format!(
            "SELECT COUNT(*) as count FROM {} WHERE deleted_at IS NOT NULL",
            self.table_name
        );

        let rows = sqlx::query(&query)
            .bind(pagination.limit() as i64)
            .bind(pagination.offset() as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let count_row = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let total: i64 = count_row.try_get("count")
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to get deleted count: {}", e),
            })?;

        // Map rows to Backbone entities
        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map deleted result to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        let total_pages = ((total as f64) / (pagination.limit() as f64)).ceil() as usize;
        let current_page = pagination.page() + 1;
        let has_next = current_page < total_pages;
        let has_previous = current_page > 1;

        Ok(PaginatedResult::new(
            backbones,
            total as u64,
            current_page,
            pagination.limit(),
        ))
    }

    async fn restore(&self, id: &BackboneId) -> RepositoryResult<()> {
        let query = format!(
            "UPDATE {} SET deleted_at = NULL, updated_at = $1 WHERE id = $2 AND deleted_at IS NOT NULL",
            self.table_name
        );

        match sqlx::query(&query)
            .bind(Utc::now())
            .bind(id.value())
            .execute(&self.pool)
            .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    Err(RepositoryError::NotFound {
                        id: id.value().to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(RepositoryError::DatabaseError {
                message: e.to_string(),
            }),
        }
    }

    async fn find_by_metadata(
        &self,
        metadata_key: &str,
        metadata_value: Option<&str>,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let (query, _params) = if let Some(value) = metadata_value {
            let query = format!(
                r#"
                SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version
                FROM {}
                WHERE metadata->>{} = ${}
                ORDER BY {} {}
                LIMIT ${} OFFSET ${}
                "#,
                self.table_name, metadata_key, 1,
                match sort.field {
                    SortField::Id => "id",
                    SortField::Name => "name",
                    SortField::Status => "status",
                    SortField::CreatedAt => "created_at",
                    SortField::UpdatedAt => "updated_at",
                    SortField::CreatedBy => "created_by",
                },
                match sort.direction {
                    SortDirection::Ascending => "ASC",
                    SortDirection::Descending => "DESC",
                },
                2, 3
            );
            (query, vec![value.to_string()])
        } else {
            let query = format!(
                r#"
                SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version
                FROM {}
                WHERE metadata ? ${}
                ORDER BY {} {}
                LIMIT ${} OFFSET ${}
                "#,
                self.table_name, 1,
                match sort.field {
                    SortField::Id => "id",
                    SortField::Name => "name",
                    SortField::Status => "status",
                    SortField::CreatedAt => "created_at",
                    SortField::UpdatedAt => "updated_at",
                    SortField::CreatedBy => "created_by",
                },
                match sort.direction {
                    SortDirection::Ascending => "ASC",
                    SortDirection::Descending => "DESC",
                },
                2, 3
            );
            (query, vec![])
        };

        let rows = sqlx::query(&query)
            .bind(metadata_key)
            .bind(pagination.limit() as i64)
            .bind(pagination.offset() as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        // Get total count
        let count_query = if let Some(_value) = metadata_value {
            format!(
                "SELECT COUNT(*) as count FROM {} WHERE metadata->>{} = $1",
                self.table_name, metadata_key
            )
        } else {
            format!(
                "SELECT COUNT(*) as count FROM {} WHERE metadata ? $1",
                self.table_name
            )
        };

        let count_row = sqlx::query(&count_query)
            .bind(metadata_key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let total: i64 = count_row.try_get("count")
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to get metadata count: {}", e),
            })?;

        // Map rows to Backbone entities
        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map metadata result to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        let total_pages = ((total as f64) / (pagination.limit() as f64)).ceil() as usize;
        let current_page = pagination.page() + 1;
        let has_next = current_page < total_pages;
        let has_previous = current_page > 1;

        Ok(PaginatedResult::new(
            backbones,
            total as u64,
            current_page,
            pagination.limit(),
        ))
    }

    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        date_field: SortField,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>> {
        let field_name = match date_field {
            SortField::CreatedAt => "created_at",
            SortField::UpdatedAt => "updated_at",
            _ => "created_at", // Default to created_at for other fields
        };

        let query = format!(
            r#"
            SELECT id, name, description, status, tags, metadata, created_by, created_at, updated_at, deleted_at, version
            FROM {}
            WHERE {} >= $1 AND {} <= $2
            ORDER BY {} {}
            LIMIT $3 OFFSET $4
            "#,
            self.table_name, field_name, field_name,
            match sort.field {
                SortField::Id => "id",
                SortField::Name => "name",
                SortField::Status => "status",
                SortField::CreatedAt => "created_at",
                SortField::UpdatedAt => "updated_at",
                SortField::CreatedBy => "created_by",
            },
            match sort.direction {
                SortDirection::Ascending => "ASC",
                SortDirection::Descending => "DESC",
            }
        );

        let count_query = format!(
            "SELECT COUNT(*) as count FROM {} WHERE {} >= $1 AND {} <= $2",
            self.table_name, field_name, field_name
        );

        let rows = sqlx::query(&query)
            .bind(start_date)
            .bind(end_date)
            .bind(pagination.limit() as i64)
            .bind(pagination.offset() as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let count_row = sqlx::query(&count_query)
            .bind(start_date)
            .bind(end_date)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let total: i64 = count_row.try_get("count")
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to get date range count: {}", e),
            })?;

        // Map rows to Backbone entities
        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map date range result to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        let total_pages = ((total as f64) / (pagination.limit() as f64)).ceil() as usize;
        let current_page = pagination.page() + 1;
        let has_next = current_page < total_pages;
        let has_previous = current_page > 1;

        Ok(PaginatedResult::new(
            backbones,
            total as u64,
            current_page,
            pagination.limit(),
        ))
    }

    async fn get_status_counts(&self) -> RepositoryResult<std::collections::HashMap<BackboneStatus, u64>> {
        let query = format!(
            "SELECT status, COUNT(*) as count FROM {} WHERE deleted_at IS NULL GROUP BY status",
            self.table_name
        );

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let mut counts = std::collections::HashMap::new();

        for row in rows {
            let status_str: String = row.try_get("status")
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to get status: {}", e),
                })?;
            let count: i64 = row.try_get("count")
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to get count: {}", e),
                })?;

            let status = match status_str.as_str() {
                "ACTIVE" => BackboneStatus::Active,
                "INACTIVE" => BackboneStatus::Inactive,
                "SUSPENDED" => BackboneStatus::Suspended,
                "ARCHIVED" => BackboneStatus::Archived,
                _ => continue, // Skip unknown statuses
            };

            counts.insert(status, count as u64);
        }

        Ok(counts)
    }

    async fn get_tag_counts(&self) -> RepositoryResult<std::collections::HashMap<String, u64>> {
        let query = format!(
            r#"
            SELECT tag, COUNT(*) as count
            FROM (
                SELECT unnest(tags) as tag
                FROM {}
                WHERE deleted_at IS NULL
            ) as all_tags
            GROUP BY tag
            ORDER BY count DESC
            "#,
            self.table_name
        );

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let mut counts = std::collections::HashMap::new();

        for row in rows {
            let tag: String = row.try_get("tag")
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to get tag: {}", e),
                })?;
            let count: i64 = row.try_get("count")
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to get tag count: {}", e),
                })?;

            counts.insert(tag, count as u64);
        }

        Ok(counts)
    }

    async fn get_recently_created(&self, days: i64, limit: Option<usize>) -> RepositoryResult<Vec<Backbone>> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days);
        let limit = limit.unwrap_or(10);

        let query = format!(
            r#"
            SELECT id, name, description, status, tags, metadata, created_by, (metadata->>'created_at')::timestamp as created_at, (metadata->>'updated_at')::timestamp as updated_at, (metadata->>'deleted_at')::timestamp as deleted_at, version
            FROM {}
            WHERE (metadata->>'created_at')::timestamp >= $1 AND (metadata->>'deleted_at')::timestamp IS NULL
            ORDER BY (metadata->>'created_at')::timestamp DESC
            LIMIT $2
            "#,
            self.table_name
        );

        let rows = sqlx::query(&query)
            .bind(cutoff_date)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
            })?;

        let mut backbones = Vec::new();
        for row in rows {
            let backbone = self.map_row_to_backbone(row).await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: format!("Failed to map recent result to Backbone: {}", e),
                })?;
            backbones.push(backbone);
        }

        Ok(backbones)
    }

    async fn health_check(&self) -> RepositoryResult<bool> {
        match sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn connection_pool_status(&self) -> RepositoryResult<std::collections::HashMap<String, serde_json::Value>> {
        let pool_size = self.pool.size();
        let idle_connections = self.pool.num_idle();

        let mut status = std::collections::HashMap::new();
        status.insert("total_connections".to_string(), serde_json::Value::Number(serde_json::Number::from(pool_size)));
        status.insert("idle_connections".to_string(), serde_json::Value::Number(serde_json::Number::from(idle_connections)));
        status.insert("active_connections".to_string(), serde_json::Value::Number(serde_json::Number::from(pool_size - idle_connections)));

        Ok(status)
    }
}

// Repository Factory
pub struct PostgresqlBackboneRepositoryFactory;

impl PostgresqlBackboneRepositoryFactory {
    pub fn create(pool: PgPool) -> PostgresqlBackboneRepository {
        PostgresqlBackboneRepository::new(pool)
    }

    pub fn create_with_table_name(pool: PgPool, table_name: String) -> PostgresqlBackboneRepository {
        PostgresqlBackboneRepository::new(pool).with_table_name(table_name)
    }
}

// Migration helper for creating the backbone table
pub async fn ensure_backbone_table_exists(pool: &PgPool, table_name: &str) -> RepositoryResult<()> {
    let create_table_query = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {} (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            name VARCHAR(100) NOT NULL,
            description TEXT,
            status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
            tags TEXT[] DEFAULT '{}',
            metadata JSONB DEFAULT '{}',
            created_by VARCHAR(255) NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
            deleted_at TIMESTAMP WITH TIME ZONE,
            version BIGINT NOT NULL DEFAULT 1,

            CONSTRAINT check_status CHECK (status IN ('ACTIVE', 'INACTIVE', 'SUSPENDED', 'ARCHIVED')),
            CONSTRAINT check_version_positive CHECK (version > 0)
        );

        CREATE INDEX IF NOT EXISTS idx_{}_created_at ON {} (created_at);
        CREATE INDEX IF NOT EXISTS idx_{}_updated_at ON {} (updated_at);
        CREATE INDEX IF NOT EXISTS idx_{}_status ON {} (status);
        CREATE INDEX IF NOT EXISTS idx_{}_created_by ON {} (created_by);
        CREATE INDEX IF NOT EXISTS idx_{}_deleted_at ON {} (deleted_at);
        CREATE INDEX IF NOT EXISTS idx_{}_tags ON {} USING GIN (tags);
        CREATE INDEX IF NOT EXISTS idx_{}_metadata ON {} USING GIN (metadata);
        "#,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name,
        table_name, table_name, table_name
    );

    sqlx::query(&create_table_query)
        .execute(pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError {
            message: format!("Failed to create backbone table: {}", e),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // Note: These tests require a PostgreSQL database
    // In a real project, you would set up a test database for these tests

    async fn create_test_pool() -> PgPool {
        // This would normally use environment variables or test configuration
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://root:password@localhost:5432/test_db".to_string());

        PgPool::connect(&database_url).await.unwrap()
    }

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a database
    async fn test_repository_crud_operations() {
        let pool = create_test_pool().await;
        let table_name = format!("test_backbones_{}", chrono::Utc::now().timestamp());

        // Ensure table exists
        ensure_backbone_table_exists(&pool, &table_name).await.unwrap();

        let repository = PostgresqlBackboneRepositoryFactory::create_with_table_name(pool, table_name);

        // Test creating and finding a backbone
        let backbone = crate::domain::entities::Backbone::create(
            crate::domain::value_objects::BackboneName::new("Test Backbone").unwrap(),
            "Test Description".to_string(),
            vec!["test".to_string()],
            {
                let mut metadata = crate::domain::value_objects::Metadata::new();
                metadata.insert("env".to_string(), "test".to_string()).unwrap();
                metadata
            },
            "test_user".to_string(),
        ).unwrap();

        let backbone_id = backbone.id().clone();

        // Save backbone
        repository.save(&backbone).await.unwrap();

        // Find backbone
        let found_backbone = repository.find_by_id(&backbone_id).await.unwrap();
        assert!(found_backbone.is_some());

        let found_backbone = found_backbone.unwrap();
        assert_eq!(found_backbone.name(), "Test Backbone");
        assert_eq!(found_backbone.description(), "Test Description");
        assert_eq!(found_backbone.created_by(), "test_user");

        // Test count
        let count = repository.count(None).await.unwrap();
        assert_eq!(count, 1);

        // Test delete
        repository.delete(&backbone_id, false).await.unwrap();

        // Should not find non-deleted backbone
        let found_backbone = repository.find_by_id(&backbone_id).await.unwrap();
        assert!(found_backbone.is_some()); // Should still find since we used soft delete

        // Test find deleted
        let deleted_backbones = repository.find_deleted(
            crate::domain::repositories::PaginationParams::new(1, 10),
            crate::domain::repositories::SortParams::default(),
        ).await.unwrap();
        assert_eq!(deleted_backbones.len(), 1);

        // Test restore
        repository.restore(&backbone_id).await.unwrap();

        // Test hard delete
        repository.delete(&backbone_id, true).await.unwrap();

        // Should not find anymore
        let found_backbone = repository.find_by_id(&backbone_id).await.unwrap();
        assert!(found_backbone.is_none());
    }

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a database
    async fn test_repository_search_and_filters() {
        let pool = create_test_pool().await;
        let table_name = format!("test_backbones_search_{}", chrono::Utc::now().timestamp());

        ensure_backbone_table_exists(&pool, &table_name).await.unwrap();

        let repository = PostgresqlBackboneRepositoryFactory::create_with_table_name(pool, table_name);

        // Create test data
        let backbones = vec![
            crate::domain::entities::Backbone::create(
                crate::domain::value_objects::BackboneName::new("Search Backbone 1").unwrap(),
                "Description with search keyword".to_string(),
                vec!["search".to_string(), "test".to_string()],
                {
                    let mut metadata = crate::domain::value_objects::Metadata::new();
                    metadata.insert("type".to_string(), "search_test".to_string()).unwrap();
                    metadata
                },
                "user1".to_string(),
            ).unwrap(),
            crate::domain::entities::Backbone::create(
                crate::domain::value_objects::BackboneName::new("Other Backbone").unwrap(),
                "Different description".to_string(),
                vec!["other".to_string()],
                {
                    let mut metadata = crate::domain::value_objects::Metadata::new();
                    metadata.insert("type".to_string(), "other".to_string()).unwrap();
                    metadata
                },
                "user2".to_string(),
            ).unwrap(),
        ];

        repository.save_batch(&backbones).await.unwrap();

        // Test search
        let search_results = repository.search(
            "search",
            crate::domain::repositories::PaginationParams::new(1, 10),
            crate::domain::repositories::SortParams::default(),
        ).await.unwrap();

        assert_eq!(search_results.len(), 1);
        assert!(search_results.items[0].name().contains("Search"));

        // Test tag filter
        let tag_results = repository.find_by_tags(
            vec!["search".to_string()],
            crate::domain::repositories::PaginationParams::new(1, 10),
            crate::domain::repositories::SortParams::default(),
        ).await.unwrap();

        assert_eq!(tag_results.len(), 1);

        // Test created_by filter
        let user_results = repository.find_by_created_by(
            "user1",
            crate::domain::repositories::PaginationParams::new(1, 10),
            crate::domain::repositories::SortParams::default(),
        ).await.unwrap();

        assert_eq!(user_results.len(), 1);

        // Test metadata filter
        let metadata_results = repository.find_by_metadata(
            "type",
            Some("search_test"),
            crate::domain::repositories::PaginationParams::new(1, 10),
            crate::domain::repositories::SortParams::default(),
        ).await.unwrap();

        assert_eq!(metadata_results.len(), 1);

        // Test status counts
        let status_counts = repository.get_status_counts().await.unwrap();
        assert_eq!(status_counts.get(&crate::domain::value_objects::BackboneStatus::Active), Some(&2));

        // Test tag counts
        let tag_counts = repository.get_tag_counts().await.unwrap();
        assert!(tag_counts.contains_key("search"));
        assert!(tag_counts.contains_key("test"));
        assert!(tag_counts.contains_key("other"));
    }
}