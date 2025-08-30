//! Integration tests for API versioning and backward compatibility

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use serde_json::Value;
use std::collections::HashMap;
use tower::ServiceExt;

use RustAutoDevOps::{
    api::{
        routing::VersionedRouter,
        versioning::{api_versioning_middleware, ApiVersion},
    },
    presentation::routes::{api_info_router, ci_router_v1, ci_router_v2},
    AppState,
};

/// Helper function to create a test app with versioned routes
fn create_test_app() -> Router {
    // Create mock state for testing
    let state = create_mock_app_state();

    VersionedRouter::new()
        .version(ApiVersion::V1, ci_router_v1())
        .version(ApiVersion::V2, ci_router_v2())
        .default_version(ci_router_v2())
        .build()
        .nest("/info", api_info_router())
        .layer(axum::middleware::from_fn(api_versioning_middleware))
        .with_state(state)
}

/// Create a mock AppState for testing
fn create_mock_app_state() -> AppState {
    // This would need to be implemented based on your actual AppState structure
    // For now, we'll assume it exists or create a minimal mock
    todo!("Implement mock AppState for testing")
}

#[tokio::test]
async fn test_version_extraction_from_header() {
    let app = create_test_app();

    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v1"));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/test") // Path says v2, but header says v1
        .body(Body::empty())
        .unwrap();

    let mut request = request;
    *request.headers_mut() = headers;

    let response = app.oneshot(request).await.unwrap();
    
    // Should use v1 because header takes precedence
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v1");
}

#[tokio::test]
async fn test_version_extraction_from_accept_header() {
    let app = create_test_app();

    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        HeaderValue::from_static("application/vnd.rustci.v1+json"),
    );

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();

    let mut request = request;
    *request.headers_mut() = headers;

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v1");
}

#[tokio::test]
async fn test_version_extraction_from_path() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v1");
}

#[tokio::test]
async fn test_version_extraction_from_query_param() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test?version=v2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v2");
}

#[tokio::test]
async fn test_default_version_fallback() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // Should default to current version (v2)
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v2");
}

#[tokio::test]
async fn test_deprecation_headers_for_v1() {
    let app = create_test_app();

    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v1"));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();

    let mut request = request;
    *request.headers_mut() = headers;

    let response = app.oneshot(request).await.unwrap();
    
    // V1 is deprecated, so deprecation headers should be present
    assert_eq!(response.headers().get("X-API-Deprecated").unwrap(), "true");
    assert!(response.headers().contains_key("X-API-Sunset"));
    assert!(response.headers().contains_key("X-API-Migration-Guide"));
}

#[tokio::test]
async fn test_no_deprecation_headers_for_v2() {
    let app = create_test_app();

    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v2"));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();

    let mut request = request;
    *request.headers_mut() = headers;

    let response = app.oneshot(request).await.unwrap();
    
    // V2 is current, so no deprecation headers
    assert!(response.headers().get("X-API-Deprecated").is_none());
}

#[tokio::test]
async fn test_supported_versions_header() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.headers().get("X-Supported-Versions").unwrap(),
        "v1,v2"
    );
}

#[tokio::test]
async fn test_api_version_info_endpoint() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/version")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let version_info: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(version_info["current_version"], "v2");
    assert!(version_info["supported_versions"].is_array());
}

#[tokio::test]
async fn test_supported_versions_endpoint() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/versions")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let versions_info: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(versions_info["supported_versions"].is_array());
    assert_eq!(versions_info["current_version"], "v2");
}

#[tokio::test]
async fn test_migration_guide_endpoint() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/migration/v1/v2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let migration_guide: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(migration_guide["from_version"], "v1");
    assert_eq!(migration_guide["to_version"], "v2");
    assert!(migration_guide["breaking_changes"].is_array());
}

#[tokio::test]
async fn test_invalid_migration_guide_versions() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/migration/v1/v99")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_compatibility_info_endpoint() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/compatibility")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let compatibility_info: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(compatibility_info["backward_compatibility"].is_object());
    assert!(compatibility_info["forward_compatibility"].is_object());
}

#[tokio::test]
async fn test_deprecation_notices_endpoint() {
    let app = create_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/info/deprecation")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let deprecation_info: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(deprecation_info["deprecation_notices"].is_array());
    assert!(deprecation_info["active_deprecations"].is_number());
}

#[tokio::test]
async fn test_version_precedence_order() {
    let app = create_test_app();

    // Test that Accept header takes precedence over X-API-Version header
    let mut headers = HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/vnd.rustci.v1+json"));
    headers.insert("x-api-version", HeaderValue::from_static("v2"));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/test") // Path says v2
        .body(Body::empty())
        .unwrap();

    let mut request = request;
    *request.headers_mut() = headers;

    let response = app.oneshot(request).await.unwrap();
    
    // Accept header should take precedence
    assert_eq!(response.headers().get("X-API-Version").unwrap(), "v1");
}

#[tokio::test]
async fn test_content_negotiation() {
    let app = create_test_app();

    // Test different Accept header formats
    let test_cases = vec![
        ("application/vnd.rustci.v1+json", "v1"),
        ("application/vnd.rustci.v2+json", "v2"),
        ("application/json", "v2"), // Should default to current
    ];

    for (accept_header, expected_version) in test_cases {
        let mut headers = HeaderMap::new();
        headers.insert("accept", HeaderValue::from_static(accept_header));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.headers().get("X-API-Version").unwrap(),
            expected_version,
            "Failed for Accept header: {}",
            accept_header
        );
    }
}

// Mock implementations for testing
// These would need to be implemented based on your actual application structure

impl Default for AppState {
    fn default() -> Self {
        // Minimal mock implementation for testing
        todo!("Implement default AppState for testing")
    }
}