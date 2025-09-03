//! Version-specific API routing system
//!
//! This module provides version-specific routing capabilities that allow
//! different API versions to coexist and handle requests appropriately.

use crate::api::versioning::{ApiVersion, ApiVersionExtractor, VersionTransformer, DefaultVersionTransformer};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{MethodRouter, Router},
    Json,
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, error};

/// Version-aware router that can handle multiple API versions
#[derive(Clone)]
pub struct VersionedRouter<S = ()> {
    /// Routers for each API version
    version_routers: HashMap<ApiVersion, Router<S>>,
    /// Default router for unversioned requests
    default_router: Option<Router<S>>,
    /// Version transformer for backward compatibility
    transformer: Arc<dyn VersionTransformer + Send + Sync>,
}

impl<S> VersionedRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Create a new versioned router
    pub fn new() -> Self {
        Self {
            version_routers: HashMap::new(),
            default_router: None,
            transformer: Arc::new(DefaultVersionTransformer::new()),
        }
    }

    /// Add a router for a specific API version
    pub fn version(mut self, version: ApiVersion, router: Router<S>) -> Self {
        self.version_routers.insert(version, router);
        self
    }

    /// Set the default router for unversioned requests
    pub fn default_version(mut self, router: Router<S>) -> Self {
        self.default_router = Some(router);
        self
    }

    /// Set a custom version transformer
    pub fn with_transformer<T>(mut self, transformer: T) -> Self
    where
        T: VersionTransformer + Send + Sync + 'static,
    {
        self.transformer = Arc::new(transformer);
        self
    }

    /// Build the final router with version routing middleware
    pub fn build(self) -> Router<S> {
        let mut router = Router::new();

        // Add version-specific routes
        for (version, version_router) in self.version_routers {
            let version_path = format!("/api/{}", version);
            router = router.nest(&version_path, version_router);
        }

        // Add default router if specified
        if let Some(default_router) = self.default_router {
            router = router.nest("/api", default_router);
        }

        // Add version routing middleware
        router.layer(axum::middleware::from_fn(version_routing_middleware))
    }
}

/// Middleware that handles version-specific routing logic
async fn version_routing_middleware(req: Request, next: Next) -> Response {
    let version_info = ApiVersionExtractor::extract_from_request(&req);
    
    debug!(
        "Version routing: {} from {:?} for path: {}",
        version_info.version,
        version_info.source,
        req.uri().path()
    );

    // Add version info to request extensions for handlers to use
    let mut req = req;
    req.extensions_mut().insert(version_info);

    next.run(req).await
}

/// Version-aware handler wrapper
pub struct VersionedHandler<S> {
    handlers: HashMap<ApiVersion, MethodRouter<S>>,
    transformer: Arc<dyn VersionTransformer + Send + Sync>,
}

impl<S> VersionedHandler<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Create a new versioned handler
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            transformer: Arc::new(DefaultVersionTransformer::new()),
        }
    }

    /// Add a handler for a specific version
    pub fn version(mut self, version: ApiVersion, handler: MethodRouter<S>) -> Self {
        self.handlers.insert(version, handler);
        self
    }

    /// Set a custom transformer
    pub fn with_transformer<T>(mut self, transformer: T) -> Self
    where
        T: VersionTransformer + Send + Sync + 'static,
    {
        self.transformer = Arc::new(transformer);
        self
    }

    /// Build the method router with version handling
    pub fn build(self) -> MethodRouter<S> {
        // For now, return the latest version handler
        // In a full implementation, this would need more sophisticated routing
        self.handlers
            .get(&ApiVersion::current())
            .cloned()
            .unwrap_or_else(|| {
                // Fallback to any available handler
                self.handlers
                    .values()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| {
                        axum::routing::get(|| async {
                            (StatusCode::NOT_IMPLEMENTED, "No handler available")
                        })
                    })
            })
    }
}

/// Response wrapper that applies version-specific transformations
pub struct VersionedResponse {
    pub version: ApiVersion,
    pub data: Value,
    pub transformer: Arc<dyn VersionTransformer + Send + Sync>,
}

impl VersionedResponse {
    /// Create a new versioned response
    pub fn new(version: ApiVersion, data: Value) -> Self {
        Self {
            version,
            data,
            transformer: Arc::new(DefaultVersionTransformer::new()),
        }
    }

    /// Create with custom transformer
    pub fn with_transformer<T>(version: ApiVersion, data: Value, transformer: T) -> Self
    where
        T: VersionTransformer + Send + Sync + 'static,
    {
        Self {
            version,
            data,
            transformer: Arc::new(transformer),
        }
    }
}

impl IntoResponse for VersionedResponse {
    fn into_response(self) -> Response {
        let transformed_data = self.transformer.transform_response(self.version, self.data);
        
        let mut response = Json(transformed_data).into_response();
        
        // Add version headers
        let headers = response.headers_mut();
        if let Ok(version_header) = self.version.to_string().parse() {
            headers.insert("X-API-Version", version_header);
        }

        response
    }
}

/// Helper function to extract version from request extensions
pub fn extract_version_from_request(req: &Request) -> ApiVersion {
    req.extensions()
        .get::<ApiVersionExtractor>()
        .map(|info| info.version)
        .unwrap_or_else(ApiVersion::current)
}

/// Macro to create version-aware handlers
#[macro_export]
macro_rules! versioned_handler {
    ($($version:expr => $handler:expr),+ $(,)?) => {
        {
            let mut versioned = $crate::api::routing::VersionedHandler::new();
            $(
                versioned = versioned.version($version, $handler);
            )+
            versioned.build()
        }
    };
}

/// Migration guide generator
pub struct MigrationGuideGenerator;

impl MigrationGuideGenerator {
    /// Generate migration guide from one version to another
    pub fn generate_guide(from: ApiVersion, to: ApiVersion) -> MigrationGuide {
        match (from, to) {
            (ApiVersion::V1, ApiVersion::V2) => MigrationGuide {
                from_version: from,
                to_version: to,
                breaking_changes: vec![
                    BreakingChange {
                        category: "Authentication".to_string(),
                        description: "JWT tokens are now required for all authenticated endpoints".to_string(),
                        old_format: Some("Cookie-based session authentication".to_string()),
                        new_format: Some("Bearer token in Authorization header".to_string()),
                        migration_steps: vec![
                            "Obtain JWT token from /auth/token endpoint".to_string(),
                            "Include 'Authorization: Bearer <token>' header in requests".to_string(),
                        ],
                    },
                    BreakingChange {
                        category: "Error Responses".to_string(),
                        description: "Error response format has been standardized".to_string(),
                        old_format: Some(r#"{"error": "Error message"}"#.to_string()),
                        new_format: Some(r#"{"error": {"code": "ERROR_CODE", "message": "Error message", "details": {}}}"#.to_string()),
                        migration_steps: vec![
                            "Update error handling to parse new error format".to_string(),
                            "Use error codes for programmatic error handling".to_string(),
                        ],
                    },
                    BreakingChange {
                        category: "Pagination".to_string(),
                        description: "Pagination parameters have been renamed".to_string(),
                        old_format: Some("page_number, per_page".to_string()),
                        new_format: Some("page, limit".to_string()),
                        migration_steps: vec![
                            "Replace 'page_number' with 'page'".to_string(),
                            "Replace 'per_page' with 'limit'".to_string(),
                        ],
                    },
                ],
                compatibility_notes: vec![
                    "V1 endpoints will continue to work until sunset date".to_string(),
                    "Use Accept header 'application/vnd.rustci.v1+json' to explicitly request V1 format".to_string(),
                    "New features will only be available in V2".to_string(),
                ],
                timeline: MigrationTimeline {
                    deprecation_date: "2024-01-01".to_string(),
                    sunset_date: "2024-12-31".to_string(),
                    recommended_migration_date: "2024-06-01".to_string(),
                },
            },
            _ => MigrationGuide::default(),
        }
    }
}

/// Migration guide structure
#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationGuide {
    pub from_version: ApiVersion,
    pub to_version: ApiVersion,
    pub breaking_changes: Vec<BreakingChange>,
    pub compatibility_notes: Vec<String>,
    pub timeline: MigrationTimeline,
}

impl Default for MigrationGuide {
    fn default() -> Self {
        Self {
            from_version: ApiVersion::V1,
            to_version: ApiVersion::V2,
            breaking_changes: vec![],
            compatibility_notes: vec![],
            timeline: MigrationTimeline::default(),
        }
    }
}

/// Breaking change description
#[derive(Debug, Clone, serde::Serialize)]
pub struct BreakingChange {
    pub category: String,
    pub description: String,
    pub old_format: Option<String>,
    pub new_format: Option<String>,
    pub migration_steps: Vec<String>,
}

/// Migration timeline
#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationTimeline {
    pub deprecation_date: String,
    pub sunset_date: String,
    pub recommended_migration_date: String,
}

impl Default for MigrationTimeline {
    fn default() -> Self {
        Self {
            deprecation_date: "TBD".to_string(),
            sunset_date: "TBD".to_string(),
            recommended_migration_date: "TBD".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};

    #[tokio::test]
    async fn test_versioned_router_creation() {
        let v1_router: Router<()> = Router::new().route("/test", get(|| async { "v1" }));
        let v2_router: Router<()> = Router::new().route("/test", get(|| async { "v2" }));

        let versioned_router = VersionedRouter::new()
            .version(ApiVersion::V1, v1_router)
            .version(ApiVersion::V2, v2_router)
            .build();

        // Router should be created successfully
        assert!(!format!("{:?}", versioned_router).is_empty());
    }

    #[test]
    fn test_migration_guide_generation() {
        let guide = MigrationGuideGenerator::generate_guide(ApiVersion::V1, ApiVersion::V2);
        
        assert_eq!(guide.from_version, ApiVersion::V1);
        assert_eq!(guide.to_version, ApiVersion::V2);
        assert!(!guide.breaking_changes.is_empty());
    }

    #[test]
    fn test_versioned_response_creation() {
        let data = serde_json::json!({"test": "data"});
        let response = VersionedResponse::new(ApiVersion::V1, data);
        
        assert_eq!(response.version, ApiVersion::V1);
    }
}