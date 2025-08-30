//! Tests for API documentation and OpenAPI specifications

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    routing::get,
    Router,
};
use serde_json::Value;
use tower::ServiceExt;

use RustAutoDevOps::api::{
    documentation::{
        EnhancedApiDoc, ApiVersionInfo, MigrationGuide, PipelineRequest, ExecutionRequest,
        PaginationQuery, HealthCheckResponse,
    },
    interactive_docs::interactive_docs_router,
    versioning::ApiVersion,
};

#[tokio::test]
async fn test_openapi_spec_generation() {
    let openapi = EnhancedApiDoc::openapi();
    
    // Check basic structure
    assert_eq!(openapi.info.title, "RustCI API");
    assert_eq!(openapi.info.version, "2.0.0");
    assert!(openapi.info.description.is_some());
    
    // Check that paths are included
    assert!(!openapi.paths.paths.is_empty());
    
    // Check that components are included
    assert!(openapi.components.is_some());
    let components = openapi.components.unwrap();
    assert!(!components.schemas.is_empty());
}

#[tokio::test]
async fn test_security_schemes() {
    let openapi = EnhancedApiDoc::with_security();
    let components = openapi.components.unwrap();
    
    // Check JWT authentication scheme
    assert!(components.security_schemes.contains_key("jwt_auth"));
    
    // Check API key authentication scheme
    assert!(components.security_schemes.contains_key("api_key_auth"));
    
    // Verify JWT scheme details
    if let Some(jwt_scheme) = components.security_schemes.get("jwt_auth") {
        match jwt_scheme {
            utoipa::openapi::security::SecurityScheme::Http(http_scheme) => {
                assert_eq!(http_scheme.scheme, utoipa::openapi::security::HttpAuthScheme::Bearer);
                assert_eq!(http_scheme.bearer_format, Some("JWT".to_string()));
            }
            _ => panic!("Expected HTTP security scheme for JWT"),
        }
    }
}

#[tokio::test]
async fn test_version_specific_specs() {
    let v1_spec = EnhancedApiDoc::for_version(ApiVersion::V1);
    let v2_spec = EnhancedApiDoc::for_version(ApiVersion::V2);
    
    // Check version numbers
    assert_eq!(v1_spec.info.version, "1.0.0");
    assert_eq!(v2_spec.info.version, "2.0.0");
    
    // Check descriptions contain version info
    assert!(v1_spec.info.description.unwrap().contains("Legacy API version"));
    assert!(v2_spec.info.description.unwrap().contains("Current API version"));
    
    // Check server URLs are version-specific
    let v1_servers = v1_spec.servers.unwrap();
    let v2_servers = v2_spec.servers.unwrap();
    
    assert!(v1_servers[0].url.contains("/api/v1"));
    assert!(v2_servers[0].url.contains("/api/v2"));
}

#[tokio::test]
async fn test_schema_definitions() {
    let openapi = EnhancedApiDoc::with_security();
    let components = openapi.components.unwrap();
    
    // Check that important schemas are defined
    let expected_schemas = vec![
        "ApiErrorResponse",
        "PipelineRequest", 
        "PipelineResponse",
        "ExecutionRequest",
        "ExecutionResponse",
        "PaginationQuery",
        "HealthCheckResponse",
    ];
    
    for schema_name in expected_schemas {
        assert!(
            components.schemas.contains_key(schema_name),
            "Schema '{}' should be defined",
            schema_name
        );
    }
}

#[tokio::test]
async fn test_pipeline_request_schema() {
    // Test that PipelineRequest can be serialized/deserialized
    let pipeline_request = PipelineRequest {
        name: "test-pipeline".to_string(),
        description: Some("A test pipeline".to_string()),
        repository_url: "https://github.com/user/repo.git".to_string(),
        branch: Some("main".to_string()),
        config: serde_json::json!({
            "stages": [
                {
                    "name": "build",
                    "steps": [{"run": "cargo build"}]
                }
            ]
        }),
        triggers: None,
        environment: None,
        tags: Some(vec!["rust".to_string(), "backend".to_string()]),
    };
    
    // Test serialization
    let json = serde_json::to_value(&pipeline_request).unwrap();
    assert_eq!(json["name"], "test-pipeline");
    assert_eq!(json["repository_url"], "https://github.com/user/repo.git");
    
    // Test deserialization
    let deserialized: PipelineRequest = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.name, "test-pipeline");
    assert_eq!(deserialized.tags.unwrap().len(), 2);
}

#[tokio::test]
async fn test_pagination_query_schema() {
    let pagination = PaginationQuery {
        page: Some(2),
        limit: Some(20),
        sort_by: Some("created_at".to_string()),
        sort_order: Some(crate::api::documentation::SortOrder::Descending),
        status: None,
        tags: Some(vec!["rust".to_string()]),
        search: Some("test".to_string()),
    };
    
    let json = serde_json::to_value(&pagination).unwrap();
    assert_eq!(json["page"], 2);
    assert_eq!(json["limit"], 20);
    assert_eq!(json["sort_order"], "desc");
}

#[tokio::test]
async fn test_api_version_info_schema() {
    let version_info = ApiVersionInfo {
        current_version: ApiVersion::V2,
        supported_versions: vec![ApiVersion::V1, ApiVersion::V2],
        deprecated_versions: vec![],
        migration_guides: std::collections::HashMap::new(),
    };
    
    let json = serde_json::to_value(&version_info).unwrap();
    assert_eq!(json["current_version"], "v2");
    assert!(json["supported_versions"].is_array());
}

#[tokio::test]
async fn test_interactive_docs_router() {
    let app = interactive_docs_router();
    
    // Test Swagger UI endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/swagger-ui")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Test OpenAPI spec endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Test examples endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/examples")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_versioned_openapi_endpoint() {
    let app = interactive_docs_router();
    
    // Test valid version
    let request = Request::builder()
        .method(Method::GET)
        .uri("/openapi/v2")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let openapi_spec: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(openapi_spec["info"]["version"], "2.0.0");
    
    // Test invalid version
    let request = Request::builder()
        .method(Method::GET)
        .uri("/openapi/v99")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_examples_endpoint() {
    let app = interactive_docs_router();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/examples")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let examples: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(examples["endpoints"].is_object());
    assert!(examples["response_examples"].is_object());
    assert!(examples["endpoints"]["pipelines"].is_object());
}

#[tokio::test]
async fn test_endpoint_specific_examples() {
    let app = interactive_docs_router();
    
    // Test valid endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/examples/create_pipeline")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let examples: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(examples["endpoint"], "POST /api/v2/pipelines");
    assert!(examples["request_examples"].is_array());
    assert!(examples["response_examples"].is_array());
    
    // Test invalid endpoint
    let request = Request::builder()
        .method(Method::GET)
        .uri("/examples/nonexistent")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_schemas_endpoint() {
    let app = interactive_docs_router();
    
    // Test schemas list
    let request = Request::builder()
        .method(Method::GET)
        .uri("/schemas")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let schemas: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(schemas["schemas"].is_object());
    assert!(schemas["schemas"]["PipelineRequest"].is_object());
    
    // Test specific schema
    let request = Request::builder()
        .method(Method::GET)
        .uri("/schemas/PipelineRequest")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let schema: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(schema["name"], "PipelineRequest");
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert!(schema["required"].is_array());
}

#[tokio::test]
async fn test_try_it_interface() {
    let app = interactive_docs_router();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/try-it")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check that it returns HTML
    let content_type = response.headers().get("content-type");
    assert!(content_type.is_some());
    // Note: The exact content-type header might vary based on Axum version
}

#[tokio::test]
async fn test_swagger_ui_with_version_parameter() {
    let app = interactive_docs_router();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/swagger-ui?version=v1&theme=dark")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    
    // Check that version parameter is used
    assert!(html.contains("/docs/openapi/v1"));
    assert!(html.contains("selected") && html.contains("v1"));
}

#[tokio::test]
async fn test_redoc_interface() {
    let app = interactive_docs_router();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/redoc?version=v2")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    
    // Check ReDoc-specific content
    assert!(html.contains("redoc"));
    assert!(html.contains("/docs/openapi/v2"));
}

#[tokio::test]
async fn test_schema_validation() {
    // Test that our schemas can be properly validated
    use serde_json::json;
    
    // Valid pipeline request
    let valid_pipeline = json!({
        "name": "test-pipeline",
        "repository_url": "https://github.com/user/repo.git",
        "config": {
            "stages": [
                {
                    "name": "build",
                    "steps": [{"run": "cargo build"}]
                }
            ]
        }
    });
    
    let pipeline: Result<PipelineRequest, _> = serde_json::from_value(valid_pipeline);
    assert!(pipeline.is_ok());
    
    // Invalid pipeline request (missing required fields)
    let invalid_pipeline = json!({
        "description": "Missing required fields"
    });
    
    let pipeline: Result<PipelineRequest, _> = serde_json::from_value(invalid_pipeline);
    assert!(pipeline.is_err());
}

#[tokio::test]
async fn test_execution_request_schema() {
    let execution_request = ExecutionRequest {
        branch: Some("feature/test".to_string()),
        commit_sha: Some("abc123".to_string()),
        environment: Some(std::collections::HashMap::from([
            ("NODE_ENV".to_string(), "test".to_string()),
        ])),
        parameters: Some(std::collections::HashMap::from([
            ("debug".to_string(), serde_json::Value::Bool(true)),
        ])),
        priority: Some(crate::api::documentation::ExecutionPriority::High),
    };
    
    let json = serde_json::to_value(&execution_request).unwrap();
    assert_eq!(json["branch"], "feature/test");
    assert_eq!(json["commit_sha"], "abc123");
    assert_eq!(json["priority"], "High");
    
    // Test deserialization
    let deserialized: ExecutionRequest = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.branch.unwrap(), "feature/test");
}

#[tokio::test]
async fn test_health_check_response_schema() {
    use std::collections::HashMap;
    
    let health_response = HealthCheckResponse {
        status: crate::api::documentation::HealthStatus::Healthy,
        uptime_seconds: 3600,
        checks: HashMap::from([
            ("database".to_string(), crate::api::documentation::ComponentHealth {
                status: crate::api::documentation::HealthStatus::Healthy,
                response_time_ms: Some(50),
                error: None,
                last_check: chrono::Utc::now(),
            }),
        ]),
        system_info: crate::api::documentation::SystemInfo {
            version: "1.0.0".to_string(),
            build_time: "2024-01-01T00:00:00Z".to_string(),
            git_commit: Some("abc123".to_string()),
            rust_version: "1.70.0".to_string(),
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
        },
        timestamp: chrono::Utc::now(),
    };
    
    let json = serde_json::to_value(&health_response).unwrap();
    assert_eq!(json["status"], "Healthy");
    assert_eq!(json["uptime_seconds"], 3600);
    assert!(json["checks"]["database"].is_object());
    assert_eq!(json["system_info"]["version"], "1.0.0");
}