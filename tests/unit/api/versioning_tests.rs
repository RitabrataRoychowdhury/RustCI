//! Tests for API versioning and backward compatibility

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Router,
};
use serde_json::Value;
use std::convert::Infallible;
use tower::ServiceExt;

use RustAutoDevOps::api::{
    routing::{MigrationGuideGenerator, VersionedResponse, VersionedRouter},
    versioning::{
        api_versioning_middleware, ApiVersion, ApiVersionExtractor, ApiVersionInfo,
        DefaultVersionTransformer, VersionSource, VersionTransformer,
    },
};

#[tokio::test]
async fn test_api_version_extraction_from_header() {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v1"));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let mut req = req;
    *req.headers_mut() = headers;

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    assert_eq!(version_info.version, ApiVersion::V1);
    assert_eq!(version_info.source, VersionSource::Header);
}

#[tokio::test]
async fn test_api_version_extraction_from_accept_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        HeaderValue::from_static("application/vnd.rustci.v2+json"),
    );

    let req = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let mut req = req;
    *req.headers_mut() = headers;

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    assert_eq!(version_info.version, ApiVersion::V2);
    assert_eq!(version_info.source, VersionSource::Header);
}

#[tokio::test]
async fn test_api_version_extraction_from_path() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/pipelines")
        .body(Body::empty())
        .unwrap();

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    assert_eq!(version_info.version, ApiVersion::V1);
    assert_eq!(version_info.source, VersionSource::Path);
}

#[tokio::test]
async fn test_api_version_extraction_from_query() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/test?version=v2&other=param")
        .body(Body::empty())
        .unwrap();

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    assert_eq!(version_info.version, ApiVersion::V2);
    assert_eq!(version_info.source, VersionSource::QueryParam);
}

#[tokio::test]
async fn test_api_version_default_fallback() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    assert_eq!(version_info.version, ApiVersion::current());
    assert_eq!(version_info.source, VersionSource::Default);
}

#[tokio::test]
async fn test_version_parsing() {
    assert_eq!(ApiVersion::from_str("v1").unwrap(), ApiVersion::V1);
    assert_eq!(ApiVersion::from_str("V2").unwrap(), ApiVersion::V2);
    assert_eq!(ApiVersion::from_str("1").unwrap(), ApiVersion::V1);
    assert_eq!(ApiVersion::from_str("2.0").unwrap(), ApiVersion::V2);
    assert!(ApiVersion::from_str("v3").is_err());
    assert!(ApiVersion::from_str("invalid").is_err());
}

#[tokio::test]
async fn test_version_support_and_deprecation() {
    assert!(ApiVersion::V1.is_supported());
    assert!(ApiVersion::V2.is_supported());
    
    assert!(ApiVersion::V1.is_deprecated());
    assert!(!ApiVersion::V2.is_deprecated());
    
    assert!(ApiVersion::V1.deprecation_notice().is_some());
    assert!(ApiVersion::V2.deprecation_notice().is_none());
}

#[tokio::test]
async fn test_versioning_middleware() {
    async fn test_handler(req: Request) -> Result<Response, Infallible> {
        let version_info = req.extensions().get::<ApiVersionExtractor>().unwrap();
        let response_body = format!("Version: {}", version_info.version);
        Ok(Response::new(Body::from(response_body)))
    }

    async fn next_fn(req: Request) -> Response {
        test_handler(req).await.unwrap()
    }

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/test")
        .body(Body::empty())
        .unwrap();

    let response = api_versioning_middleware(req, Next::new(next_fn)).await;
    
    // Check that version headers are added
    assert!(response.headers().contains_key("X-API-Version"));
    assert!(response.headers().contains_key("X-Supported-Versions"));
}

#[tokio::test]
async fn test_unsupported_version_response() {
    // This test would need to be implemented when we add support for detecting
    // truly unsupported versions (like v3, v4, etc.)
    // For now, we only support v1 and v2, both of which are supported
}

#[tokio::test]
async fn test_deprecation_headers() {
    async fn test_handler(_req: Request) -> Result<Response, Infallible> {
        Ok(Response::new(Body::from("test")))
    }

    async fn next_fn(req: Request) -> Response {
        test_handler(req).await.unwrap()
    }

    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v1"));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let mut req = req;
    *req.headers_mut() = headers;

    let response = api_versioning_middleware(req, Next::new(next_fn)).await;
    
    // V1 is deprecated, so deprecation headers should be present
    assert_eq!(
        response.headers().get("X-API-Deprecated").unwrap(),
        "true"
    );
    assert!(response.headers().contains_key("X-API-Sunset"));
    assert!(response.headers().contains_key("X-API-Migration-Guide"));
}

#[tokio::test]
async fn test_version_transformer() {
    let transformer = DefaultVersionTransformer::new();
    
    // Test V2 to V1 transformation
    let v2_response = serde_json::json!({
        "pipeline_id": "123",
        "pagination": {
            "page": 1,
            "limit": 10
        },
        "error": {
            "message": "Test error"
        }
    });

    let v1_response = transformer.transform_response(ApiVersion::V1, v2_response);
    
    // Check V1 format transformations
    assert_eq!(v1_response["id"], "123");
    assert_eq!(v1_response["pagination"]["page_number"], 1);
    assert_eq!(v1_response["pagination"]["per_page"], 10);
    assert_eq!(v1_response["error"]["error_message"], "Test error");
}

#[tokio::test]
async fn test_versioned_response() {
    let data = serde_json::json!({"test": "data"});
    let versioned_response = VersionedResponse::new(ApiVersion::V1, data);
    
    assert_eq!(versioned_response.version, ApiVersion::V1);
    
    // Test response conversion
    let response = versioned_response.into_response();
    assert_eq!(
        response.headers().get("X-API-Version").unwrap(),
        "v1"
    );
}

#[tokio::test]
async fn test_versioned_router() {
    let v1_router = Router::new().route("/test", get(|| async { "v1 response" }));
    let v2_router = Router::new().route("/test", get(|| async { "v2 response" }));

    let app = VersionedRouter::new()
        .version(ApiVersion::V1, v1_router)
        .version(ApiVersion::V2, v2_router)
        .build();

    // Test V1 endpoint
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/test")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test V2 endpoint
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_version_info() {
    let version_info = ApiVersionInfo::new();
    
    assert_eq!(version_info.current_version, ApiVersion::current());
    assert!(version_info.supported_versions.contains(&ApiVersion::V1));
    assert!(version_info.supported_versions.contains(&ApiVersion::V2));
    assert!(!version_info.migration_guides.is_empty());
}

#[tokio::test]
async fn test_migration_guide_generation() {
    let guide = MigrationGuideGenerator::generate_guide(ApiVersion::V1, ApiVersion::V2);
    
    assert_eq!(guide.from_version, ApiVersion::V1);
    assert_eq!(guide.to_version, ApiVersion::V2);
    assert!(!guide.breaking_changes.is_empty());
    assert!(!guide.compatibility_notes.is_empty());
    
    // Check that authentication breaking change is included
    let auth_change = guide
        .breaking_changes
        .iter()
        .find(|change| change.category == "Authentication");
    assert!(auth_change.is_some());
    
    // Check that error response breaking change is included
    let error_change = guide
        .breaking_changes
        .iter()
        .find(|change| change.category == "Error Responses");
    assert!(error_change.is_some());
}

#[tokio::test]
async fn test_custom_version_transformer() {
    struct CustomTransformer;
    
    impl VersionTransformer for CustomTransformer {
        fn transform_request(&self, _version: ApiVersion, request: Value) -> Value {
            request
        }
        
        fn transform_response(&self, version: ApiVersion, mut response: Value) -> Value {
            if version == ApiVersion::V1 {
                if let Some(obj) = response.as_object_mut() {
                    obj.insert("custom_field".to_string(), Value::String("v1_custom".to_string()));
                }
            }
            response
        }
    }
    
    let transformer = CustomTransformer;
    let data = serde_json::json!({"original": "data"});
    
    let transformed = transformer.transform_response(ApiVersion::V1, data);
    assert_eq!(transformed["custom_field"], "v1_custom");
}

#[tokio::test]
async fn test_version_precedence() {
    // Test that header takes precedence over path
    let mut headers = HeaderMap::new();
    headers.insert("x-api-version", HeaderValue::from_static("v2"));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/test")  // Path says v1
        .body(Body::empty())
        .unwrap();

    let mut req = req;
    *req.headers_mut() = headers;

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    // Header should take precedence
    assert_eq!(version_info.version, ApiVersion::V2);
    assert_eq!(version_info.source, VersionSource::Header);
}

#[tokio::test]
async fn test_accept_header_precedence() {
    // Test that Accept header takes precedence over custom header
    let mut headers = HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/vnd.rustci.v1+json"));
    headers.insert("x-api-version", HeaderValue::from_static("v2"));

    let req = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let mut req = req;
    *req.headers_mut() = headers;

    let version_info = ApiVersionExtractor::extract_from_request(&req);
    // Accept header should take precedence
    assert_eq!(version_info.version, ApiVersion::V1);
    assert_eq!(version_info.source, VersionSource::Header);
}