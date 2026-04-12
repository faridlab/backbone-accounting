// Backbone HTTP Handlers
// HTTP REST API handlers for Backbone operations

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::application::{
    ApplicationServices, CreateBackboneCommand, CreateBackboneResponse, GetBackboneQuery,
    GetBackboneResponse, ListBackbonesQuery, ListBackbonesResponse, SearchBackbonesQuery,
};
use crate::domain::repositories::{PaginationParams, SortParams};
use crate::domain::value_objects::BackboneStatus;

// HTTP Request DTOs
#[derive(Debug, Deserialize)]
pub struct CreateBackboneRequest {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl From<CreateBackboneRequest> for CreateBackboneCommand {
    fn from(request: CreateBackboneRequest) -> Self {
        Self {
            name: request.name,
            description: request.description,
            tags: request.tags,
            metadata: request.metadata,
            created_by: "system".to_string(), // This should come from authentication context
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListBackbonesRequest {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_by: Option<String>,
    pub search: Option<String>,
}

impl From<ListBackbonesRequest> for ListBackbonesQuery {
    fn from(request: ListBackbonesRequest) -> Self {
        let mut filters = None;

        if request.status.is_some() || request.tags.is_some() || request.created_by.is_some() {
            let mut backbone_filters = crate::application::BackboneFilters::new();

            if let Some(status) = request.status {
                backbone_filters = backbone_filters.with_status(status);
            }

            if let Some(tags) = request.tags {
                backbone_filters = backbone_filters.with_tags(tags);
            }

            if let Some(created_by) = request.created_by {
                backbone_filters = backbone_filters.with_created_by(created_by);
            }

            filters = Some(backbone_filters);
        }

        let mut query = Self::new()
            .with_pagination(
                request.page.unwrap_or(1),
                request.page_size.unwrap_or(20),
            );

        if let Some(sort_by) = request.sort_by {
            if let Some(sort_direction) = request.sort_direction {
                query = query.with_sort(sort_by, sort_direction);
            }
        }

        if let Some(filters) = filters {
            query = query.with_filters(filters);
        }

        query
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchBackbonesRequest {
    pub query: String,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
}

impl From<SearchBackbonesRequest> for SearchBackbonesQuery {
    fn from(request: SearchBackbonesRequest) -> Self {
        let mut query = Self::new(request.query);

        query = query.with_pagination(
            request.page.unwrap_or(1),
            request.page_size.unwrap_or(20),
        );

        if let Some(sort_by) = request.sort_by {
            if let Some(sort_direction) = request.sort_direction {
                query = query.with_sort(sort_by, sort_direction);
            }
        }

        query
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateBackboneRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BackboneStatusRequest {
    pub status: String,
    pub reason: Option<String>,
}

// HTTP Response DTOs
#[derive(Debug, Serialize)]
pub struct BackboneListResponse {
    pub data: Vec<crate::application::BackboneDto>,
    pub pagination: PaginationInfo,
    pub filters: Option<AppliedFilters>,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub page: usize,
    pub page_size: usize,
    pub total: u64,
    pub total_pages: usize,
    pub has_next: bool,
    pub has_previous: bool,
}

#[derive(Debug, Serialize)]
pub struct AppliedFilters {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ApiError {
    pub fn new(error: String, message: String) -> Self {
        Self {
            error,
            message,
            details: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl Responder for ApiError {
    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse {
        HttpResponse::InternalServerError().json(self)
    }
}

// HTTP Handlers
pub struct BackboneHttpHandler {
    services: ApplicationServices,
}

impl BackboneHttpHandler {
    pub fn new(services: ApplicationServices) -> Self {
        Self { services }
    }

    // Create a new Backbone
    pub async fn create_backbone(
        &self,
        request: web::Json<CreateBackboneRequest>,
    ) -> Result<HttpResponse, ApiError> {
        let command = CreateBackboneCommand::from(request.into_inner());

        let handler = self.services.create_backbone_handler();

        match self.services.execute_command(command, handler).await {
            Ok(response) => {
                if response.success {
                    Ok(HttpResponse::Created().json(response))
                } else {
                    Err(ApiError::new(
                        "CREATION_FAILED".to_string(),
                        response.message,
                    ))
                }
            }
            Err(e) => Err(ApiError::new(
                "COMMAND_ERROR".to_string(),
                e.to_string(),
            )),
        }
    }

    // Get a Backbone by ID
    pub async fn get_backbone(
        &self,
        path: web::Path<String>,
    ) -> Result<HttpResponse, ApiError> {
        let query = GetBackboneQuery::new(path.into_inner());

        let handler = self.services.get_backbone_handler();

        match self.services.execute_query(query, handler).await {
            Ok(response) => {
                if response.success {
                    Ok(HttpResponse::Ok().json(response))
                } else {
                    Ok(HttpResponse::NotFound().json(response))
                }
            }
            Err(e) => Err(ApiError::new(
                "QUERY_ERROR".to_string(),
                e.to_string(),
            )),
        }
    }

    // List Backbones with optional filters
    pub async fn list_backbones(
        &self,
        query: web::Query<ListBackbonesRequest>,
    ) -> Result<HttpResponse, ApiError> {
        let list_query = ListBackbonesQuery::from(query.into_inner());

        let handler = self.services.list_backbones_handler();

        match self.services.execute_query(list_query, handler).await {
            Ok(response) => {
                let backbone_response = BackboneListResponse {
                    data: response.backbones,
                    pagination: PaginationInfo {
                        page: response.page,
                        page_size: response.page_size,
                        total: response.total,
                        total_pages: response.total_pages,
                        has_next: response.has_next,
                        has_previous: response.has_previous,
                    },
                    filters: None, // TODO: Extract filters from query
                };

                Ok(HttpResponse::Ok().json(backbone_response))
            }
            Err(e) => Err(ApiError::new(
                "QUERY_ERROR".to_string(),
                e.to_string(),
            )),
        }
    }

    // Search Backbones
    pub async fn search_backbones(
        &self,
        query: web::Query<SearchBackbonesRequest>,
    ) -> Result<HttpResponse, ApiError> {
        let search_query = SearchBackbonesQuery::from(query.into_inner());

        let handler = self.services.search_backbones_handler();

        match self.services.execute_query(search_query, handler).await {
            Ok(response) => {
                let backbone_response = BackboneListResponse {
                    data: response.backbones,
                    pagination: PaginationInfo {
                        page: response.page,
                        page_size: response.page_size,
                        total: response.total,
                        total_pages: response.total_pages,
                        has_next: response.has_next,
                        has_previous: response.has_previous,
                    },
                    filters: None,
                };

                Ok(HttpResponse::Ok().json(backbone_response))
            }
            Err(e) => Err(ApiError::new(
                "SEARCH_ERROR".to_string(),
                e.to_string(),
            )),
        }
    }

    // Update a Backbone
    pub async fn update_backbone(
        &self,
        path: web::Path<String>,
        request: web::Json<UpdateBackboneRequest>,
    ) -> Result<HttpResponse, ApiError> {
        let id = path.into_inner();

        // For now, this is a placeholder implementation
        // In a complete implementation, you would create UpdateBackboneCommand and UpdateBackboneHandler

        Err(ApiError::new(
            "NOT_IMPLEMENTED".to_string(),
            "Update functionality not yet implemented".to_string(),
        ))
    }

    // Delete a Backbone
    pub async fn delete_backbone(
        &self,
        path: web::Path<String>,
    ) -> Result<HttpResponse, ApiError> {
        let id = path.into_inner();

        // For now, this is a placeholder implementation
        // In a complete implementation, you would create DeleteBackboneCommand and DeleteBackboneHandler

        Ok(HttpResponse::NoContent().finish())
    }

    // Update Backbone status
    pub async fn update_backbone_status(
        &self,
        path: web::Path<String>,
        request: web::Json<BackboneStatusRequest>,
    ) -> Result<HttpResponse, ApiError> {
        let id = path.into_inner();
        let new_status = request.status.clone();

        // For now, this is a placeholder implementation
        // In a complete implementation, you would create UpdateStatusCommand and handler

        Err(ApiError::new(
            "NOT_IMPLEMENTED".to_string(),
            format!("Update status functionality not yet implemented for status: {}", new_status),
        ))
    }

    // Get Backbone statistics
    pub async fn get_backbone_stats(&self) -> Result<HttpResponse, ApiError> {
        // For now, this is a placeholder implementation
        // In a complete implementation, you would create StatsQuery and handler

        let stats = serde_json::json!({
            "total_backbones": 0,
            "by_status": {
                "ACTIVE": 0,
                "INACTIVE": 0,
                "SUSPENDED": 0,
                "ARCHIVED": 0
            },
            "by_tags": {},
            "recently_created": []
        });

        Ok(HttpResponse::Ok().json(stats))
    }

    // Health check endpoint
    pub async fn health_check(&self) -> Result<HttpResponse, ApiError> {
        match self.services.health_check().await {
            Ok(health_status) => {
                let status_code = match health_status.status {
                    crate::infrastructure::health::HealthStatus::Healthy => actix_web::http::StatusCode::OK,
                    crate::infrastructure::health::HealthStatus::Degraded => {
                        actix_web::http::StatusCode::OK // Still return 200 but with degraded status
                    }
                    crate::infrastructure::health::HealthStatus::Unhealthy => {
                        actix_web::http::StatusCode::SERVICE_UNAVAILABLE
                    }
                    crate::infrastructure::health::HealthStatus::Unknown => {
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
                    }
                };

                Ok(HttpResponse::build(status_code).json(health_status))
            }
            Err(e) => Err(ApiError::new(
                "HEALTH_CHECK_ERROR".to_string(),
                e.to_string(),
            )),
        }
    }

    // Get API information
    pub async fn api_info(&self) -> Result<HttpResponse, ApiError> {
        let info = serde_json::json!({
            "name": "Backbone API",
            "version": "1.0.0",
            "description": "Backbone bounded context REST API",
            "endpoints": {
                "create": "POST /api/v1/backbones",
                "get": "GET /api/v1/backbones/{id}",
                "list": "GET /api/v1/backbones",
                "search": "GET /api/v1/backbones/search",
                "update": "PUT /api/v1/backbones/{id}",
                "delete": "DELETE /api/v1/backbones/{id}",
                "status": "PATCH /api/v1/backbones/{id}/status",
                "stats": "GET /api/v1/backbones/stats",
                "health": "GET /health",
                "info": "GET /api/v1/info"
            },
            "documentation": "/swagger-ui",
            "features": [
                "CQRS pattern",
                "Domain-driven design",
                "Clean architecture",
                "Event sourcing",
                "PostgreSQL persistence"
            ]
        });

        Ok(HttpResponse::Ok().json(info))
    }
}

// Helper functions for routing
pub fn configure_backbone_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/backbones")
            .route("", web::post().to(create_backbone))
            .route("", web::get().to(list_backbones))
            .route("/search", web::get().to(search_backbones))
            .route("/stats", web::get().to(get_backbone_stats))
            .route("/{id}", web::get().to(get_backbone))
            .route("/{id}", web::put().to(update_backbone))
            .route("/{id}", web::delete().to(delete_backbone))
            .route("/{id}/status", web::patch().to(update_backbone_status)),
    );

    cfg.route("/health", web::get().to(health_check));
    cfg.route("/api/v1/info", web::get().to(api_info));
}

// Actix-web handler functions that use the application services
async fn create_backbone(
    handler: web::Data<BackboneHttpHandler>,
    request: web::Json<CreateBackboneRequest>,
) -> Result<HttpResponse, ApiError> {
    handler.create_backbone(request).await
}

async fn get_backbone(
    handler: web::Data<BackboneHttpHandler>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    handler.get_backbone(path).await
}

async fn list_backbones(
    handler: web::Data<BackboneHttpHandler>,
    query: web::Query<ListBackbonesRequest>,
) -> Result<HttpResponse, ApiError> {
    handler.list_backbones(query).await
}

async fn search_backbones(
    handler: web::Data<BackboneHttpHandler>,
    query: web::Query<SearchBackbonesRequest>,
) -> Result<HttpResponse, ApiError> {
    handler.search_backbones(query).await
}

async fn update_backbone(
    handler: web::Data<BackboneHttpHandler>,
    path: web::Path<String>,
    request: web::Json<UpdateBackboneRequest>,
) -> Result<HttpResponse, ApiError> {
    handler.update_backbone(path, request).await
}

async fn delete_backbone(
    handler: web::Data<BackboneHttpHandler>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    handler.delete_backbone(path).await
}

async fn update_backbone_status(
    handler: web::Data<BackboneHttpHandler>,
    path: web::Path<String>,
    request: web::Json<BackboneStatusRequest>,
) -> Result<HttpResponse, ApiError> {
    handler.update_backbone_status(path, request).await
}

async fn get_backbone_stats(
    handler: web::Data<BackboneHttpHandler>,
) -> Result<HttpResponse, ApiError> {
    handler.get_backbone_stats().await
}

async fn health_check(
    handler: web::Data<BackboneHttpHandler>,
) -> Result<HttpResponse, ApiError> {
    handler.health_check().await
}

async fn api_info(
    handler: web::Data<BackboneHttpHandler>,
) -> Result<HttpResponse, ApiError> {
    handler.api_info().await
}

// Middleware for error handling
pub async fn error_handler(
    err: actix_web::Error,
    _req: &actix_web::HttpRequest,
) -> HttpResponse {
    let api_error = ApiError::new(
        "INTERNAL_ERROR".to_string(),
        err.to_string(),
    );

    HttpResponse::InternalServerError().json(api_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use serde_json::json;

    fn create_test_handler() -> BackboneHttpHandler {
        // This would normally use real ApplicationServices
        // For testing, we'll create a mock implementation
        use crate::domain::repositories::BackboneRepository;
        use async_trait::async_trait;

        struct MockRepository;

        #[async_trait]
        impl BackboneRepository for MockRepository {
            async fn save(&self, _backbone: &crate::domain::entities::Backbone) -> crate::domain::repositories::RepositoryResult<()> {
                Ok(())
            }

            async fn find_by_id(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<Option<crate::domain::entities::Backbone>> {
                Ok(None)
            }

            async fn delete(&self, _id: &crate::domain::value_objects::BackboneId, _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
                Ok(())
            }

            async fn find_all(
                &self,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn find_with_filters(
                &self,
                _filters: crate::domain::repositories::BackboneFilters,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn find_by_status(
                &self,
                _status: crate::domain::value_objects::BackboneStatus,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn find_by_tags(
                &self,
                _tags: Vec<String>,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn find_by_created_by(
                &self,
                _created_by: &str,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn search(
                &self,
                _query: &str,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn save_batch(&self, _backbones: &[crate::domain::entities::Backbone]) -> crate::domain::repositories::RepositoryResult<()> {
                Ok(())
            }

            async fn delete_batch(&self, _ids: &[crate::domain::value_objects::BackboneId], _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
                Ok(())
            }

            async fn exists(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<bool> {
                Ok(false)
            }

            async fn count(&self, _filters: Option<crate::domain::repositories::BackboneFilters>) -> crate::domain::repositories::RepositoryResult<u64> {
                Ok(0)
            }

            async fn find_deleted(
                &self,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn restore(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<()> {
                Ok(())
            }

            async fn find_by_metadata(
                &self,
                _metadata_key: &str,
                _metadata_value: Option<&str>,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn find_by_date_range(
                &self,
                _start_date: chrono::DateTime<chrono::Utc>,
                _end_date: chrono::DateTime<chrono::Utc>,
                _date_field: crate::domain::repositories::SortField,
                _pagination: crate::domain::repositories::PaginationParams,
                _sort: crate::domain::repositories::SortParams,
            ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
                Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
            }

            async fn get_status_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<crate::domain::value_objects::BackboneStatus, u64>> {
                Ok(std::collections::HashMap::new())
            }

            async fn get_tag_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, u64>> {
                Ok(std::collections::HashMap::new())
            }

            async fn get_recently_created(&self, _days: i64, _limit: Option<usize>) -> crate::domain::repositories::RepositoryResult<Vec<crate::domain::entities::Backbone>> {
                Ok(Vec::new())
            }

            async fn health_check(&self) -> crate::domain::repositories::RepositoryResult<bool> {
                Ok(true)
            }

            async fn connection_pool_status(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, serde_json::Value>> {
                Ok(std::collections::HashMap::new())
            }
        }

        // Create minimal services for testing
        let repository = Box::new(MockRepository);
        let services = ApplicationServices::builder()
            .with_repository(repository)
            .build()
            .unwrap();

        BackboneHttpHandler::new(services)
    }

    #[actix_web::test]
    async fn test_create_backbone_request_conversion() {
        let request = CreateBackboneRequest {
            name: "Test Backbone".to_string(),
            description: "Test Description".to_string(),
            tags: vec!["test".to_string()],
            metadata: HashMap::new(),
        };

        let command = CreateBackboneCommand::from(request);

        assert_eq!(command.name, "Test Backbone");
        assert_eq!(command.description, "Test Description");
        assert_eq!(command.tags, vec!["test"]);
        assert!(command.metadata.is_empty());
    }

    #[actix_web::test]
    async fn test_api_error_creation() {
        let error = ApiError::new(
            "TEST_ERROR".to_string(),
            "Test error message".to_string(),
        );

        assert_eq!(error.error, "TEST_ERROR");
        assert_eq!(error.message, "Test error message");
        assert!(error.details.is_none());
    }

    #[actix_web::test]
    async fn test_health_check_endpoint() {
        let handler = create_test_handler();
        let req = test::TestRequest::get()
            .uri("/health")
            .to_request();

        let app = test::init_service(
            App::new()
                .app_data(handler)
                .route("/health", web::get().to(health_check)),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_api_info_endpoint() {
        let handler = create_test_handler();
        let req = test::TestRequest::get()
            .uri("/api/v1/info")
            .to_request();

        let app = test::init_service(
            App::new()
                .app_data(handler)
                .route("/api/v1/info", web::get().to(api_info)),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_list_backbones_endpoint() {
        let handler = create_test_handler();
        let req = test::TestRequest::get()
            .uri("/api/v1/backbones?page=1&page_size=10")
            .to_request();

        let app = test::init_service(
            App::new()
                .app_data(handler)
                .route("/api/v1/backbones", web::get().to(list_backbones)),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_search_backbones_endpoint() {
        let handler = create_test_handler();
        let req = test::TestRequest::get()
            .uri("/api/v1/backbones/search?query=test&page=1&page_size=10")
            .to_request();

        let app = test::init_service(
            App::new()
                .app_data(handler)
                .route("/api/v1/backbones/search", web::get().to(search_backbones)),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_backbone_endpoint_not_found() {
        let handler = create_test_handler();
        let req = test::TestRequest::get()
            .uri("/api/v1/backbones/non-existent-id")
            .to_request();

        let app = test::init_service(
            App::new()
                .app_data(handler)
                .route("/api/v1/backbones/{id}", web::get().to(get_backbone)),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }
}