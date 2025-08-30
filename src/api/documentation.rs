//! Enhanced API Documentation System
//!
//! This module provides comprehensive OpenAPI specifications with detailed
//! endpoint documentation, request/response examples, and error scenarios.

use crate::api::{
    errors::{ApiError, ApiErrorCode, ApiErrorResponse},
    versioning::ApiVersion,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{
    openapi::{
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
        Contact, Info, License, Server,
    },
    OpenApi, ToSchema,
};

/// Enhanced OpenAPI documentation with versioning support
#[derive(OpenApi)]
#[openapi(
    info(
        title = "RustCI API",
        version = "2.0.0",
        description = r#"
# RustCI API Documentation

A high-performance CI/CD platform built in Rust with comprehensive API support.

## Features

- **Multi-version API support** - Supports both v1 and v2 API versions
- **Comprehensive error handling** - Standardized error responses with recovery suggestions
- **Rate limiting** - Built-in rate limiting with configurable policies
- **Authentication** - JWT-based authentication with refresh token support
- **Real-time monitoring** - Pipeline execution monitoring and logging
- **Scalable architecture** - Horizontal scaling with load balancing

## Authentication

All API endpoints require authentication using JWT tokens. Include the token in the Authorization header:

```
Authorization: Bearer <your-jwt-token>
```

## API Versioning

The API supports multiple versions. You can specify the version using:

1. **Accept Header** (Recommended): `Accept: application/vnd.rustci.v2+json`
2. **Custom Header**: `X-API-Version: v2`
3. **URL Path**: `/api/v2/pipelines`
4. **Query Parameter**: `?version=v2`

## Rate Limiting

API requests are rate limited. Check the following headers in responses:
- `X-RateLimit-Limit`: Maximum requests per window
- `X-RateLimit-Remaining`: Remaining requests in current window
- `X-RateLimit-Reset`: Time when the rate limit resets

## Error Handling

All errors follow a consistent format with actionable recovery suggestions:

```json
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Input validation failed",
    "details": {...},
    "recovery_suggestions": [...],
    "documentation_url": "/docs/errors/validation"
  },
  "request_id": "uuid",
  "timestamp": "2024-01-01T00:00:00Z"
}
```
        "#,
        contact(
            name = "RustCI Team",
            email = "api-support@rustci.dev",
            url = "https://rustci.dev/support"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development server"),
        (url = "https://api-staging.rustci.dev", description = "Staging server"),
        (url = "https://api.rustci.dev", description = "Production server")
    ),
    paths(
        // CI/CD Pipeline endpoints
        crate::application::handlers::ci::create_pipeline,
        crate::application::handlers::ci::create_pipeline_multipart,
        crate::application::handlers::ci::list_pipelines,
        crate::application::handlers::ci::get_pipeline,
        crate::application::handlers::ci::get_pipeline_yaml,
        crate::application::handlers::ci::trigger_pipeline,
        // crate::application::handlers::ci::webhook_handler,
        // crate::application::handlers::ci::get_execution,
        // crate::application::handlers::ci::list_executions,
        // crate::application::handlers::ci::cancel_execution,
        
        // Authentication endpoints
        crate::application::handlers::auth::google_oauth_handler,
        crate::application::handlers::auth::github_oauth_handler,
        crate::application::handlers::auth::github_oauth_callback,
        crate::application::handlers::auth::get_me_handler,
        crate::application::handlers::auth::logout_handler,
        
        // API Information endpoints
        // crate::presentation::routes::api_info::get_api_version_info,
        // crate::presentation::routes::api_info::get_supported_versions,
        // crate::presentation::routes::api_info::get_migration_guide,
        // crate::presentation::routes::api_info::get_compatibility_info,
    ),
    components(
        schemas(
            // Enhanced API types
            // ApiErrorResponse,
            // ApiError,
            ApiVersionInfo,
            MigrationGuide,
            BreakingChange,
            MigrationTimeline,
            
            // Pipeline types
            PipelineRequest,
            PipelineResponse,
            PipelineListResponse,
            ExecutionRequest,
            ExecutionResponse,
            ExecutionListResponse,
            
            // Common types
            PaginationQuery,
            PaginationResponse,
            ErrorDetails,
            RecoverySuggestion,
            
            // Authentication types
            UserResponse,
            AuthResponse,
            
            // Status and health types
            HealthCheckResponse,
            SystemInfo,
        )
    ),
    tags(
        (name = "pipelines", description = "CI/CD Pipeline Management"),
        (name = "executions", description = "Pipeline Execution Management"),
        (name = "authentication", description = "User Authentication and Authorization"),
        (name = "api-info", description = "API Version and Documentation Information"),
        (name = "health", description = "System Health and Status"),
        (name = "webhooks", description = "Webhook Integration"),
    )
)]
pub struct EnhancedApiDoc;

impl EnhancedApiDoc {
    /// Create OpenAPI spec with security schemes
    pub fn with_security() -> utoipa::openapi::OpenApi {
        let mut openapi = Self::openapi();

        // Add JWT Bearer authentication
        let jwt_auth = SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("JWT")
                .description(Some("JWT token for API authentication. Obtain from /auth/login endpoint."))
                .build(),
        );

        // Add API Key authentication for service-to-service calls
        let api_key_auth = SecurityScheme::ApiKey(
            utoipa::openapi::security::ApiKey::Header(
                utoipa::openapi::security::ApiKeyValue::new("X-API-Key")
            )
        );

        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme("jwt_auth", jwt_auth);
        components.add_security_scheme("api_key_auth", api_key_auth);

        openapi
    }

    /// Create version-specific OpenAPI spec
    pub fn for_version(version: ApiVersion) -> utoipa::openapi::OpenApi {
        let mut openapi = Self::with_security();
        
        // Update info for specific version
        let version_str = match version {
            ApiVersion::V1 => "1.0.0",
            ApiVersion::V2 => "2.0.0",
        };
        
        openapi.info.version = version_str.to_string();
        
        // Add version-specific description
        let version_description = match version {
            ApiVersion::V1 => {
                "Legacy API version (v1) - Deprecated. Please migrate to v2. \
                 This version will be sunset on 2024-12-31."
            }
            ApiVersion::V2 => {
                "Current API version (v2) - Recommended for all new integrations. \
                 Includes enhanced error handling, improved pagination, and JWT authentication."
            }
        };
        
        openapi.info.description = Some(format!(
            "{}\n\n## Version Information\n\n{}",
            openapi.info.description.unwrap_or_default(),
            version_description
        ));
        
        // Update server URLs for version
        openapi.servers = Some(vec![
            Server::new(format!("http://localhost:8080/api/{}", version)),
            Server::new(format!("https://api-staging.rustci.dev/api/{}", version)),
            Server::new(format!("https://api.rustci.dev/api/{}", version)),
        ]);
        
        openapi
    }
}

/// API version information schema
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiVersionInfo {
    /// Current API version
    pub current_version: ApiVersion,
    /// List of supported API versions
    pub supported_versions: Vec<ApiVersion>,
    /// Deprecated versions with notices
    pub deprecated_versions: Vec<DeprecationNotice>,
    /// Available migration guides
    pub migration_guides: HashMap<String, String>,
}

/// Deprecation notice for API versions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeprecationNotice {
    /// Deprecated version
    pub version: ApiVersion,
    /// Date when version was deprecated
    pub deprecated_since: String,
    /// Date when version will be sunset (removed)
    pub sunset_date: Option<String>,
    /// URL to migration guide
    pub migration_guide_url: Option<String>,
    /// Replacement version
    pub replacement_version: ApiVersion,
    /// List of breaking changes
    pub breaking_changes: Vec<String>,
}

/// Migration guide between API versions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MigrationGuide {
    /// Source version
    pub from_version: ApiVersion,
    /// Target version
    pub to_version: ApiVersion,
    /// List of breaking changes
    pub breaking_changes: Vec<BreakingChange>,
    /// Compatibility notes
    pub compatibility_notes: Vec<String>,
    /// Migration timeline
    pub timeline: MigrationTimeline,
}

/// Breaking change description
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BreakingChange {
    /// Category of the change
    pub category: String,
    /// Description of the change
    pub description: String,
    /// Old format (if applicable)
    pub old_format: Option<String>,
    /// New format (if applicable)
    pub new_format: Option<String>,
    /// Steps to migrate
    pub migration_steps: Vec<String>,
}

/// Migration timeline information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MigrationTimeline {
    /// Date when version was deprecated
    pub deprecation_date: String,
    /// Date when version will be sunset
    pub sunset_date: String,
    /// Recommended migration completion date
    pub recommended_migration_date: String,
}

/// Enhanced pipeline request with comprehensive documentation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(
    title = "Pipeline Request",
    example = json!({
        "name": "my-awesome-pipeline",
        "description": "A pipeline for building and testing my application",
        "repository_url": "https://github.com/user/repo.git",
        "branch": "main",
        "config": {
            "stages": [
                {
                    "name": "build",
                    "steps": [
                        {"run": "cargo build --release"}
                    ]
                },
                {
                    "name": "test",
                    "steps": [
                        {"run": "cargo test"}
                    ]
                }
            ]
        },
        "triggers": {
            "push": true,
            "pull_request": true,
            "schedule": "0 2 * * *"
        }
    })
)]
pub struct PipelineRequest {
    /// Pipeline name (must be unique within organization)
    #[schema(min_length = 1, max_length = 100, pattern = "^[a-zA-Z0-9-_]+$")]
    pub name: String,
    
    /// Pipeline description
    #[schema(max_length = 500)]
    pub description: Option<String>,
    
    /// Repository URL
    #[schema(format = "uri")]
    pub repository_url: String,
    
    /// Git branch to track
    #[schema(default = "main")]
    pub branch: Option<String>,
    
    /// Pipeline configuration (YAML or JSON)
    pub config: serde_json::Value,
    
    /// Pipeline triggers configuration
    pub triggers: Option<PipelineTriggers>,
    
    /// Environment variables
    pub environment: Option<HashMap<String, String>>,
    
    /// Pipeline tags for organization
    pub tags: Option<Vec<String>>,
}

/// Pipeline triggers configuration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PipelineTriggers {
    /// Trigger on push to branch
    pub push: Option<bool>,
    
    /// Trigger on pull requests
    pub pull_request: Option<bool>,
    
    /// Cron schedule for automatic triggers
    #[schema(pattern = r"^(\*|[0-5]?\d) (\*|[01]?\d|2[0-3]) (\*|[0-2]?\d|3[01]) (\*|[0]?\d|1[0-2]) (\*|[0-6])$")]
    pub schedule: Option<String>,
    
    /// Manual trigger only
    pub manual: Option<bool>,
}

/// Enhanced pipeline response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(
    title = "Pipeline Response"
)]
pub struct PipelineResponse {
    /// Unique pipeline identifier
    pub pipeline_id: String,
    
    /// Pipeline name
    pub name: String,
    
    /// Pipeline description
    pub description: Option<String>,
    
    /// Repository URL
    pub repository_url: String,
    
    /// Current branch
    pub branch: String,
    
    /// Pipeline status
    pub status: PipelineStatus,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    
    /// Last execution information
    pub last_execution: Option<ExecutionSummary>,
    
    /// Execution statistics
    pub statistics: PipelineStatistics,
    
    /// Pipeline tags
    pub tags: Vec<String>,
}

/// Pipeline status enumeration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum PipelineStatus {
    /// Pipeline is active and ready for execution
    Active,
    /// Pipeline is paused (won't trigger automatically)
    Paused,
    /// Pipeline is disabled
    Disabled,
    /// Pipeline has configuration errors
    Error,
}

/// Execution summary information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionSummary {
    /// Execution ID
    pub execution_id: String,
    /// Execution status
    pub status: ExecutionStatus,
    /// Start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Completion time (if finished)
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Duration in seconds
    pub duration_seconds: Option<u64>,
}

/// Execution status enumeration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ExecutionStatus {
    /// Execution is queued
    Queued,
    /// Execution is running
    Running,
    /// Execution completed successfully
    Success,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
    /// Execution timed out
    Timeout,
}

/// Pipeline execution statistics
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PipelineStatistics {
    /// Total number of executions
    pub total_executions: u64,
    /// Number of successful executions
    pub successful_executions: u64,
    /// Number of failed executions
    pub failed_executions: u64,
    /// Average execution duration in seconds
    pub average_duration_seconds: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}

/// Paginated pipeline list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PipelineListResponse {
    /// List of pipelines
    pub data: Vec<PipelineResponse>,
    /// Pagination information
    pub pagination: PaginationResponse,
}

/// Enhanced pagination query parameters
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationQuery {
    /// Page number (1-based)
    #[schema(minimum = 1, default = 1)]
    pub page: Option<u32>,
    
    /// Number of items per page
    #[schema(minimum = 1, maximum = 100, default = 10)]
    pub limit: Option<u32>,
    
    /// Sort field
    pub sort_by: Option<String>,
    
    /// Sort order
    pub sort_order: Option<SortOrder>,
    
    /// Filter by status
    pub status: Option<PipelineStatus>,
    
    /// Filter by tags
    pub tags: Option<Vec<String>>,
    
    /// Search query
    pub search: Option<String>,
}

/// Sort order enumeration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

/// Pagination response information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationResponse {
    /// Current page number
    pub page: u32,
    /// Items per page
    pub limit: u32,
    /// Total number of pages
    pub total_pages: u32,
    /// Total number of items
    pub total_count: u64,
    /// Whether there is a next page
    pub has_next: bool,
    /// Whether there is a previous page
    pub has_previous: bool,
}

/// Error details for validation errors
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorDetails {
    /// Field-specific validation errors
    pub field_errors: Option<HashMap<String, Vec<String>>>,
    /// Global validation errors
    pub global_errors: Option<Vec<String>>,
    /// Additional context information
    pub context: Option<serde_json::Value>,
}

/// Recovery suggestion for error resolution
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecoverySuggestion {
    /// Suggested action to take
    pub action: String,
    /// Description of the action
    pub description: String,
    /// Example of how to perform the action
    pub example: Option<String>,
    /// Priority level of the suggestion
    pub priority: SuggestionPriority,
}

/// Priority level for recovery suggestions
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum SuggestionPriority {
    High,
    Medium,
    Low,
}

/// User response for authentication endpoints
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    /// User ID
    pub user_id: String,
    /// User email
    pub email: String,
    /// Display name
    pub display_name: String,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Account creation date
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last login date
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    /// User roles
    pub roles: Vec<String>,
    /// User permissions
    pub permissions: Vec<String>,
}

/// Authentication response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    /// JWT access token
    pub access_token: String,
    /// Refresh token
    pub refresh_token: String,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Token expiration time in seconds
    pub expires_in: u64,
    /// User information
    pub user: UserResponse,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthCheckResponse {
    /// Overall system status
    pub status: HealthStatus,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Individual component health checks
    pub checks: HashMap<String, ComponentHealth>,
    /// System information
    pub system_info: SystemInfo,
    /// Response timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health status enumeration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ComponentHealth {
    /// Component status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Error message (if unhealthy)
    pub error: Option<String>,
    /// Last check timestamp
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// System information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemInfo {
    /// Application version
    pub version: String,
    /// Build timestamp
    pub build_time: String,
    /// Git commit hash
    pub git_commit: Option<String>,
    /// Rust version used for build
    pub rust_version: String,
    /// Operating system
    pub os: String,
    /// Architecture
    pub arch: String,
}

/// Execution request for triggering pipelines
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionRequest {
    /// Git branch to execute (optional, defaults to pipeline's default branch)
    pub branch: Option<String>,
    /// Git commit SHA to execute (optional, defaults to latest)
    pub commit_sha: Option<String>,
    /// Environment variables for this execution
    pub environment: Option<HashMap<String, String>>,
    /// Execution parameters
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    /// Execution priority
    pub priority: Option<ExecutionPriority>,
}

/// Execution priority levels
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ExecutionPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Detailed execution response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionResponse {
    /// Execution ID
    pub execution_id: String,
    /// Pipeline ID
    pub pipeline_id: String,
    /// Execution status
    pub status: ExecutionStatus,
    /// Git branch
    pub branch: String,
    /// Git commit SHA
    pub commit_sha: String,
    /// Execution start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Execution completion time
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Execution duration in seconds
    pub duration_seconds: Option<u64>,
    /// Execution logs URL
    pub logs_url: Option<String>,
    /// Execution artifacts
    pub artifacts: Vec<ExecutionArtifact>,
    /// Execution steps
    pub steps: Vec<ExecutionStep>,
}

/// Execution artifact information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionArtifact {
    /// Artifact name
    pub name: String,
    /// Artifact type
    pub artifact_type: String,
    /// Artifact size in bytes
    pub size_bytes: u64,
    /// Download URL
    pub download_url: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Execution step information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionStep {
    /// Step name
    pub name: String,
    /// Step status
    pub status: ExecutionStatus,
    /// Step start time
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Step completion time
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Step duration in seconds
    pub duration_seconds: Option<u64>,
    /// Step logs
    pub logs: Option<String>,
    /// Exit code (for command steps)
    pub exit_code: Option<i32>,
}

/// Paginated execution list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionListResponse {
    /// List of executions
    pub data: Vec<ExecutionResponse>,
    /// Pagination information
    pub pagination: PaginationResponse,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_generation() {
        let openapi = EnhancedApiDoc::openapi();
        assert!(!openapi.paths.paths.is_empty());
        assert!(openapi.components.is_some());
    }

    #[test]
    fn test_security_schemes() {
        let openapi = EnhancedApiDoc::with_security();
        let components = openapi.components.unwrap();
        assert!(components.security_schemes.contains_key("jwt_auth"));
        assert!(components.security_schemes.contains_key("api_key_auth"));
    }

    #[test]
    fn test_version_specific_spec() {
        let v1_spec = EnhancedApiDoc::for_version(ApiVersion::V1);
        assert_eq!(v1_spec.info.version, "1.0.0");
        assert!(v1_spec.info.description.unwrap().contains("Legacy API version"));

        let v2_spec = EnhancedApiDoc::for_version(ApiVersion::V2);
        assert_eq!(v2_spec.info.version, "2.0.0");
        assert!(v2_spec.info.description.unwrap().contains("Current API version"));
    }
}