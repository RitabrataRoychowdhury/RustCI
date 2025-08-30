//! Tests for API response optimization system

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

use RustAutoDevOps::api::response_optimization::{
    CachingConfig, CompressionConfig, PaginationConfig, PaginationQuery, ResponseCache,
    ResponseOptimizationService, ResponsePerformanceMonitor, SortOrder,
    create_paginated_response, response_optimization_middleware,
};

#[test]
fn test_pagination_config_validation() {
    let config = PaginationConfig {
        default_page_size: 25,
        max_page_size: 200,
        min_page_size: 5,
        enable_cursor_pagination: false,
    };
    
    assert_eq!(config.default_page_size, 25);
    assert_eq!(config.max_page_size, 200);
    assert_eq!(config.min_page_size, 5);
    assert!(!config.enable_cursor_pagination);
}

#[test]
fn test_pagination_query_defaults() {
    let query = PaginationQuery {
        page: None,
        limit: None,
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    
    assert!(query.page.is_none());
    assert!(query.limit.is_none());
    assert!(query.cursor.is_none());
    assert!(query.sort_by.is_none());
    assert!(query.sort_order.is_none());
}

#[test]
fn test_pagination_query_with_values() {
    let query = PaginationQuery {
        page: Some(3),
        limit: Some(25),
        cursor: Some("cursor123".to_string()),
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Descending),
    };
    
    assert_eq!(query.page, Some(3));
    assert_eq!(query.limit, Some(25));
    assert_eq!(query.cursor, Some("cursor123".to_string()));
    assert_eq!(query.sort_by, Some("created_at".to_string()));
    assert!(matches!(query.sort_order, Some(SortOrder::Descending)));
}

#[test]
fn test_sort_order_serialization() {
    let asc = SortOrder::Ascending;
    let desc = SortOrder::Descending;
    
    let asc_json = serde_json::to_value(&asc).unwrap();
    let desc_json = serde_json::to_value(&desc).unwrap();
    
    assert_eq!(asc_json, "asc");
    assert_eq!(desc_json, "desc");
    
    // Test deserialization
    let asc_deserialized: SortOrder = serde_json::from_value(asc_json).unwrap();
    let desc_deserialized: SortOrder = serde_json::from_value(desc_json).unwrap();
    
    assert!(matches!(asc_deserialized, SortOrder::Ascending));
    assert!(matches!(desc_deserialized, SortOrder::Descending));
}

#[test]
fn test_create_paginated_response_basic() {
    let data = vec!["item1", "item2", "item3", "item4", "item5"];
    let query = PaginationQuery {
        page: Some(1),
        limit: Some(3),
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    let config = PaginationConfig::default();
    
    let response = create_paginated_response(
        data,
        &query,
        10, // total count
        "http://example.com/api/items",
        &config,
    );
    
    assert_eq!(response.data.len(), 5);
    assert_eq!(response.pagination.page, Some(1));
    assert_eq!(response.pagination.limit, 3);
    assert_eq!(response.pagination.total_count, 10);
    assert_eq!(response.pagination.total_pages, Some(4)); // ceil(10/3) = 4
    assert!(response.pagination.has_more);
    
    // Check links
    assert!(response.pagination.links.next.is_some());
    assert!(response.pagination.links.prev.is_none()); // First page
    assert!(response.pagination.links.last.is_some());
    assert!(response.pagination.links.first.is_none()); // Already on first page
}

#[test]
fn test_create_paginated_response_middle_page() {
    let data = vec!["item6", "item7", "item8"];
    let query = PaginationQuery {
        page: Some(2),
        limit: Some(3),
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    let config = PaginationConfig::default();
    
    let response = create_paginated_response(
        data,
        &query,
        10,
        "http://example.com/api/items",
        &config,
    );
    
    assert_eq!(response.pagination.page, Some(2));
    assert!(response.pagination.has_more);
    
    // Check links for middle page
    assert!(response.pagination.links.next.is_some());
    assert!(response.pagination.links.prev.is_some());
    assert!(response.pagination.links.first.is_some());
    assert!(response.pagination.links.last.is_some());
}

#[test]
fn test_create_paginated_response_last_page() {
    let data = vec!["item10"];
    let query = PaginationQuery {
        page: Some(4),
        limit: Some(3),
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    let config = PaginationConfig::default();
    
    let response = create_paginated_response(
        data,
        &query,
        10,
        "http://example.com/api/items",
        &config,
    );
    
    assert_eq!(response.pagination.page, Some(4));
    assert!(!response.pagination.has_more); // Last page
    
    // Check links for last page
    assert!(response.pagination.links.next.is_none()); // No next page
    assert!(response.pagination.links.prev.is_some());
    assert!(response.pagination.links.first.is_some());
    assert!(response.pagination.links.last.is_none()); // Already on last page
}

#[test]
fn test_pagination_limit_clamping() {
    let config = PaginationConfig {
        default_page_size: 20,
        max_page_size: 50,
        min_page_size: 5,
        enable_cursor_pagination: true,
    };
    
    // Test limit too high
    let query_high = PaginationQuery {
        page: Some(1),
        limit: Some(100), // Above max
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    
    let response_high = create_paginated_response(
        vec!["item1"],
        &query_high,
        100,
        "http://example.com/api/items",
        &config,
    );
    
    assert_eq!(response_high.pagination.limit, 50); // Clamped to max
    
    // Test limit too low
    let query_low = PaginationQuery {
        page: Some(1),
        limit: Some(1), // Below min
        cursor: None,
        sort_by: None,
        sort_order: None,
    };
    
    let response_low = create_paginated_response(
        vec!["item1"],
        &query_low,
        100,
        "http://example.com/api/items",
        &config,
    );
    
    assert_eq!(response_low.pagination.limit, 5); // Clamped to min
}

#[tokio::test]
async fn test_response_cache_basic_operations() {
    let config = CachingConfig::default();
    let cache = ResponseCache::new(config);
    
    let key = "test_key".to_string();
    let cached_response = RustAutoDevOps::api::response_optimization::CachedResponse {
        body: b"test response body".to_vec(),
        headers: axum::http::HeaderMap::new(),
        status: StatusCode::OK,
        cached_at: std::time::Instant::now(),
        ttl: 300,
        access_count: 0,
    };
    
    // Initially, cache should be empty
    let result = cache.get(&key).await;
    assert!(result.is_none());
    
    // Store response in cache
    cache.set(key.clone(), cached_response.clone()).await;
    
    // Retrieve from cache
    let retrieved = cache.get(&key).await;
    assert!(retrieved.is_some());
    
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.body, b"test response body");
    assert_eq!(retrieved.status, StatusCode::OK);
    assert_eq!(retrieved.access_count, 1); // Should increment on access
    
    // Access again to test counter increment
    let retrieved_again = cache.get(&key).await;
    assert!(retrieved_again.is_some());
    assert_eq!(retrieved_again.unwrap().access_count, 2);
}

#[tokio::test]
async fn test_response_cache_expiration() {
    let config = CachingConfig::default();
    let cache = ResponseCache::new(config);
    
    let key = "expired_key".to_string();
    let expired_response = RustAutoDevOps::api::response_optimization::CachedResponse {
        body: b"expired response".to_vec(),
        headers: axum::http::HeaderMap::new(),
        status: StatusCode::OK,
        cached_at: std::time::Instant::now() - Duration::from_secs(400), // Expired (TTL is 300)
        ttl: 300,
        access_count: 0,
    };
    
    cache.set(key.clone(), expired_response).await;
    
    // Should return None for expired entry
    let result = cache.get(&key).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_response_cache_key_generation() {
    let config = CachingConfig::default();
    let cache = ResponseCache::new(config);
    
    let key1 = cache.generate_cache_key("GET", "/api/v2/pipelines", None);
    let key2 = cache.generate_cache_key("GET", "/api/v2/pipelines", Some("page=1&limit=10"));
    let key3 = cache.generate_cache_key("POST", "/api/v2/pipelines", None);
    
    // Different queries should generate different keys
    assert_ne!(key1, key2);
    // Different methods should generate different keys
    assert_ne!(key1, key3);
    
    // Keys should contain relevant information
    assert!(key1.contains("GET"));
    assert!(key1.contains("/api/v2/pipelines"));
    assert!(key2.contains("page=1&limit=10"));
    assert!(key3.contains("POST"));
}

#[tokio::test]
async fn test_response_cache_cleanup() {
    let config = CachingConfig {
        enabled: true,
        default_ttl: 1, // Very short TTL for testing
        max_cache_size: 10,
        key_prefix: "test".to_string(),
        cacheable_methods: vec!["GET".to_string()],
        cacheable_status_codes: vec![200],
    };
    let cache = ResponseCache::new(config);
    
    // Add some entries, some expired
    let fresh_response = RustAutoDevOps::api::response_optimization::CachedResponse {
        body: b"fresh".to_vec(),
        headers: axum::http::HeaderMap::new(),
        status: StatusCode::OK,
        cached_at: std::time::Instant::now(),
        ttl: 300,
        access_count: 0,
    };
    
    let expired_response = RustAutoDevOps::api::response_optimization::CachedResponse {
        body: b"expired".to_vec(),
        headers: axum::http::HeaderMap::new(),
        status: StatusCode::OK,
        cached_at: std::time::Instant::now() - Duration::from_secs(400),
        ttl: 1,
        access_count: 0,
    };
    
    cache.set("fresh_key".to_string(), fresh_response).await;
    cache.set("expired_key".to_string(), expired_response).await;
    
    // Before cleanup, both should be in cache (though expired one won't be returned)
    let stats_before = cache.get_stats().await;
    assert_eq!(stats_before.total_entries, 2);
    
    // Cleanup expired entries
    cache.cleanup_expired().await;
    
    // After cleanup, only fresh entry should remain
    let stats_after = cache.get_stats().await;
    assert_eq!(stats_after.total_entries, 1);
    
    // Fresh entry should still be accessible
    let fresh_result = cache.get("fresh_key").await;
    assert!(fresh_result.is_some());
    
    // Expired entry should be gone
    let expired_result = cache.get("expired_key").await;
    assert!(expired_result.is_none());
}

#[tokio::test]
async fn test_performance_monitor() {
    let monitor = ResponsePerformanceMonitor::new();
    
    // Record some metrics
    monitor.record_response(
        "GET",
        "/api/v2/pipelines",
        200,
        150, // 150ms response time
        1024, // 1KB response size
        false, // not cached
    ).await;
    
    monitor.record_response(
        "GET",
        "/api/v2/pipelines",
        200,
        200, // 200ms response time
        2048, // 2KB response size
        true, // cached
    ).await;
    
    monitor.record_response(
        "GET",
        "/api/v2/pipelines",
        404,
        50, // 50ms response time
        512, // 512B response size
        false, // not cached
    ).await;
    
    // Get metrics for the endpoint
    let metrics = monitor.get_endpoint_metrics("GET", "/api/v2/pipelines").await;
    assert!(metrics.is_some());
    
    let metrics = metrics.unwrap();
    assert_eq!(metrics.method, "GET");
    assert_eq!(metrics.path, "/api/v2/pipelines");
    assert_eq!(metrics.total_requests, 3);
    assert_eq!(metrics.cached_responses, 1);
    
    // Check calculated averages
    assert_eq!(metrics.avg_response_time_ms(), (150.0 + 200.0 + 50.0) / 3.0);
    assert_eq!(metrics.avg_response_size_bytes(), (1024.0 + 2048.0 + 512.0) / 3.0);
    assert_eq!(metrics.cache_hit_rate(), 1.0 / 3.0);
    
    // Check min/max response times
    assert_eq!(metrics.min_response_time_ms, 50);
    assert_eq!(metrics.max_response_time_ms, 200);
    
    // Check status code distribution
    assert_eq!(metrics.status_code_counts.get(&200), Some(&2));
    assert_eq!(metrics.status_code_counts.get(&404), Some(&1));
}

#[tokio::test]
async fn test_performance_monitor_multiple_endpoints() {
    let monitor = ResponsePerformanceMonitor::new();
    
    // Record metrics for different endpoints
    monitor.record_response("GET", "/api/v2/pipelines", 200, 100, 1024, false).await;
    monitor.record_response("POST", "/api/v2/pipelines", 201, 200, 2048, false).await;
    monitor.record_response("GET", "/api/v2/executions", 200, 150, 1536, true).await;
    
    // Get all metrics
    let all_metrics = monitor.get_all_metrics().await;
    assert_eq!(all_metrics.len(), 3); // Three different endpoints
    
    // Check that each endpoint has correct metrics
    let pipeline_get = monitor.get_endpoint_metrics("GET", "/api/v2/pipelines").await.unwrap();
    assert_eq!(pipeline_get.total_requests, 1);
    assert_eq!(pipeline_get.cached_responses, 0);
    
    let pipeline_post = monitor.get_endpoint_metrics("POST", "/api/v2/pipelines").await.unwrap();
    assert_eq!(pipeline_post.total_requests, 1);
    assert_eq!(pipeline_post.cached_responses, 0);
    
    let executions_get = monitor.get_endpoint_metrics("GET", "/api/v2/executions").await.unwrap();
    assert_eq!(executions_get.total_requests, 1);
    assert_eq!(executions_get.cached_responses, 1);
}

#[tokio::test]
async fn test_response_optimization_service() {
    let pagination_config = PaginationConfig::default();
    let compression_config = CompressionConfig::default();
    let caching_config = CachingConfig::default();
    
    let service = ResponseOptimizationService::new(
        pagination_config,
        compression_config,
        caching_config,
    );
    
    // Test pagination
    let data = vec!["item1", "item2", "item3", "item4", "item5"];
    let query = PaginationQuery {
        page: Some(1),
        limit: Some(3),
        cursor: None,
        sort_by: Some("name".to_string()),
        sort_order: Some(SortOrder::Ascending),
    };
    
    let paginated = service.paginate(
        data,
        &query,
        20,
        "http://example.com/api/items",
    );
    
    assert_eq!(paginated.data.len(), 5);
    assert_eq!(paginated.pagination.limit, 3);
    assert_eq!(paginated.pagination.total_count, 20);
    assert!(paginated.pagination.has_more);
    
    // Test cache stats (should be empty initially)
    let cache_stats = service.get_cache_stats().await;
    assert_eq!(cache_stats.total_entries, 0);
    
    // Test performance metrics (should be empty initially)
    let perf_metrics = service.get_performance_metrics().await;
    assert_eq!(perf_metrics.len(), 0);
}

// Test handlers for middleware testing
async fn test_handler() -> &'static str {
    "test response"
}

async fn json_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "message": "test json response",
        "data": [1, 2, 3, 4, 5]
    }))
}

#[tokio::test]
async fn test_response_optimization_middleware() {
    let pagination_config = PaginationConfig::default();
    let compression_config = CompressionConfig::default();
    let caching_config = CachingConfig::default();
    
    let service = Arc::new(ResponseOptimizationService::new(
        pagination_config,
        compression_config,
        caching_config,
    ));
    
    let app = Router::new()
        .route("/test", get(test_handler))
        .route("/json", get(json_handler))
        .layer(axum::middleware::from_fn_with_state(
            service.clone(),
            response_optimization_middleware,
        ))
        .with_state(service);
    
    // Test basic request
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check that performance headers are added
    assert!(response.headers().contains_key("X-Response-Time"));
    assert!(response.headers().contains_key("X-Cache"));
    
    let cache_header = response.headers().get("X-Cache").unwrap();
    assert_eq!(cache_header, "MISS"); // First request should be a cache miss
    
    // Test JSON endpoint
    let json_request = Request::builder()
        .method(Method::GET)
        .uri("/json")
        .body(Body::empty())
        .unwrap();
    
    let json_response = app.oneshot(json_request).await.unwrap();
    assert_eq!(json_response.status(), StatusCode::OK);
    assert!(json_response.headers().contains_key("X-Response-Time"));
}

#[test]
fn test_compression_config() {
    let config = CompressionConfig {
        enable_gzip: true,
        enable_brotli: false,
        min_size: 2048,
        compression_level: 9,
        compress_content_types: vec![
            "application/json".to_string(),
            "text/html".to_string(),
        ],
    };
    
    assert!(config.enable_gzip);
    assert!(!config.enable_brotli);
    assert_eq!(config.min_size, 2048);
    assert_eq!(config.compression_level, 9);
    assert_eq!(config.compress_content_types.len(), 2);
}

#[test]
fn test_caching_config_customization() {
    let config = CachingConfig {
        enabled: false,
        default_ttl: 600,
        max_cache_size: 5000,
        key_prefix: "custom_prefix".to_string(),
        cacheable_methods: vec!["GET".to_string(), "HEAD".to_string()],
        cacheable_status_codes: vec![200, 201, 202, 204, 304],
    };
    
    assert!(!config.enabled);
    assert_eq!(config.default_ttl, 600);
    assert_eq!(config.max_cache_size, 5000);
    assert_eq!(config.key_prefix, "custom_prefix");
    assert_eq!(config.cacheable_methods.len(), 2);
    assert_eq!(config.cacheable_status_codes.len(), 5);
}

#[test]
fn test_pagination_with_sorting() {
    let data = vec!["item1", "item2", "item3"];
    let query = PaginationQuery {
        page: Some(1),
        limit: Some(10),
        cursor: None,
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Descending),
    };
    let config = PaginationConfig::default();
    
    let response = create_paginated_response(
        data,
        &query,
        50,
        "http://example.com/api/items",
        &config,
    );
    
    // Links should include sort parameters
    assert!(response.pagination.links.self_link.contains("sort_by=created_at"));
    assert!(response.pagination.links.self_link.contains("sort_order=desc"));
    
    if let Some(next_link) = &response.pagination.links.next {
        assert!(next_link.contains("sort_by=created_at"));
        assert!(next_link.contains("sort_order=desc"));
    }
}