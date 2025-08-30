//! API Response Optimization System
//!
//! This module provides response pagination, compression, caching, and performance
//! monitoring to optimize API response times and reduce bandwidth usage.

use axum::{
    extract::{Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Pagination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationConfig {
    /// Default page size
    pub default_page_size: u32,
    /// Maximum page size
    pub max_page_size: u32,
    /// Minimum page size
    pub min_page_size: u32,
    /// Enable cursor-based pagination
    pub enable_cursor_pagination: bool,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            default_page_size: 20,
            max_page_size: 100,
            min_page_size: 1,
            enable_cursor_pagination: true,
        }
    }
}

/// Pagination query parameters
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationQuery {
    /// Page number (1-based, for offset pagination)
    pub page: Option<u32>,
    /// Number of items per page
    pub limit: Option<u32>,
    /// Cursor for cursor-based pagination
    pub cursor: Option<String>,
    /// Sort field
    pub sort_by: Option<String>,
    /// Sort order
    pub sort_order: Option<SortOrder>,
}

/// Sort order enumeration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    /// Response data
    pub data: Vec<T>,
    /// Pagination metadata
    pub pagination: PaginationMetadata,
}

/// Pagination metadata
#[derive(Debug, Clone, Serialize)]
pub struct PaginationMetadata {
    /// Current page (for offset pagination)
    pub page: Option<u32>,
    /// Items per page
    pub limit: u32,
    /// Total number of items
    pub total_count: u64,
    /// Total number of pages
    pub total_pages: Option<u32>,
    /// Whether there are more items
    pub has_more: bool,
    /// Next cursor (for cursor-based pagination)
    pub next_cursor: Option<String>,
    /// Previous cursor (for cursor-based pagination)
    pub prev_cursor: Option<String>,
    /// Links for navigation
    pub links: PaginationLinks,
}

/// Pagination navigation links
#[derive(Debug, Clone, Serialize)]
pub struct PaginationLinks {
    /// Link to first page
    pub first: Option<String>,
    /// Link to previous page
    pub prev: Option<String>,
    /// Link to next page
    pub next: Option<String>,
    /// Link to last page
    pub last: Option<String>,
    /// Link to current page
    pub self_link: String,
}

/// Response compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Enable gzip compression
    pub enable_gzip: bool,
    /// Enable brotli compression
    pub enable_brotli: bool,
    /// Minimum response size to compress (bytes)
    pub min_size: usize,
    /// Compression level (1-9)
    pub compression_level: u32,
    /// Content types to compress
    pub compress_content_types: Vec<String>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enable_gzip: true,
            enable_brotli: true,
            min_size: 1024, // 1KB
            compression_level: 6,
            compress_content_types: vec![
                "application/json".to_string(),
                "text/plain".to_string(),
                "text/html".to_string(),
                "text/css".to_string(),
                "application/javascript".to_string(),
            ],
        }
    }
}

/// Response caching configuration
#[derive(Debug, Clone)]
pub struct CachingConfig {
    /// Enable response caching
    pub enabled: bool,
    /// Default cache TTL in seconds
    pub default_ttl: u64,
    /// Maximum cache size (number of entries)
    pub max_cache_size: usize,
    /// Cache key prefix
    pub key_prefix: String,
    /// Cacheable HTTP methods
    pub cacheable_methods: Vec<String>,
    /// Cacheable status codes
    pub cacheable_status_codes: Vec<u16>,
}

impl Default for CachingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl: 300, // 5 minutes
            max_cache_size: 10000,
            key_prefix: "rustci_api".to_string(),
            cacheable_methods: vec!["GET".to_string()],
            cacheable_status_codes: vec![200, 201, 202, 204],
        }
    }
}

/// Cached response entry
#[derive(Debug, Clone)]
pub struct CachedResponse {
    /// Response body
    pub body: Vec<u8>,
    /// Response headers
    pub headers: HeaderMap,
    /// Status code
    pub status: StatusCode,
    /// Cache timestamp
    pub cached_at: Instant,
    /// TTL in seconds
    pub ttl: u64,
    /// Access count
    pub access_count: u64,
}

impl CachedResponse {
    /// Check if cache entry is expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > Duration::from_secs(self.ttl)
    }
}

/// Response cache implementation
#[derive(Debug)]
pub struct ResponseCache {
    config: CachingConfig,
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
}

impl ResponseCache {
    /// Create new response cache
    pub fn new(config: CachingConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate cache key for request
    pub fn generate_cache_key(&self, method: &str, path: &str, query: Option<&str>) -> String {
        let query_part = query.unwrap_or("");
        format!("{}:{}:{}:{}", self.config.key_prefix, method, path, query_part)
    }

    /// Get cached response
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        let mut cache = self.cache.write().await;
        
        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired() {
                cache.remove(key);
                None
            } else {
                entry.access_count += 1;
                Some(entry.clone())
            }
        } else {
            None
        }
    }

    /// Store response in cache
    pub async fn set(&self, key: String, response: CachedResponse) {
        let mut cache = self.cache.write().await;
        
        // Check cache size limit
        if cache.len() >= self.config.max_cache_size {
            // Remove oldest entries (simple LRU-like behavior)
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, entry)| entry.cached_at)
                .map(|(k, _)| k.clone());
            
            if let Some(key_to_remove) = oldest_key {
                cache.remove(&key_to_remove);
            }
        }
        
        cache.insert(key, response);
    }

    /// Clear expired entries
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total_entries = cache.len();
        let total_access_count = cache.values().map(|entry| entry.access_count).sum();
        
        CacheStats {
            total_entries,
            total_access_count,
            max_size: self.config.max_cache_size,
            hit_rate: 0.0, // Would need to track hits/misses separately
        }
    }
}

/// Cache statistics
#[derive(Debug, Serialize)]
pub struct CacheStats {
    /// Total number of cached entries
    pub total_entries: usize,
    /// Total access count across all entries
    pub total_access_count: u64,
    /// Maximum cache size
    pub max_size: usize,
    /// Cache hit rate
    pub hit_rate: f64,
}

/// Performance monitoring for API responses
#[derive(Debug)]
pub struct ResponsePerformanceMonitor {
    metrics: Arc<RwLock<HashMap<String, EndpointMetrics>>>,
}

impl ResponsePerformanceMonitor {
    /// Create new performance monitor
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record response metrics
    pub async fn record_response(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        response_time_ms: u64,
        response_size_bytes: usize,
        cached: bool,
    ) {
        let endpoint_key = format!("{}:{}", method, path);
        let mut metrics = self.metrics.write().await;
        
        let endpoint_metrics = metrics.entry(endpoint_key).or_insert_with(|| EndpointMetrics {
            method: method.to_string(),
            path: path.to_string(),
            total_requests: 0,
            total_response_time_ms: 0,
            total_response_size_bytes: 0,
            cached_responses: 0,
            status_code_counts: HashMap::new(),
            min_response_time_ms: u64::MAX,
            max_response_time_ms: 0,
        });
        
        endpoint_metrics.total_requests += 1;
        endpoint_metrics.total_response_time_ms += response_time_ms;
        endpoint_metrics.total_response_size_bytes += response_size_bytes;
        
        if cached {
            endpoint_metrics.cached_responses += 1;
        }
        
        *endpoint_metrics.status_code_counts.entry(status_code).or_insert(0) += 1;
        
        endpoint_metrics.min_response_time_ms = endpoint_metrics.min_response_time_ms.min(response_time_ms);
        endpoint_metrics.max_response_time_ms = endpoint_metrics.max_response_time_ms.max(response_time_ms);
    }

    /// Get metrics for all endpoints
    pub async fn get_all_metrics(&self) -> Vec<EndpointMetrics> {
        let metrics = self.metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// Get metrics for specific endpoint
    pub async fn get_endpoint_metrics(&self, method: &str, path: &str) -> Option<EndpointMetrics> {
        let endpoint_key = format!("{}:{}", method, path);
        let metrics = self.metrics.read().await;
        metrics.get(&endpoint_key).cloned()
    }
}

impl Default for ResponsePerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Endpoint performance metrics
#[derive(Debug, Clone, Serialize)]
pub struct EndpointMetrics {
    /// HTTP method
    pub method: String,
    /// Endpoint path
    pub path: String,
    /// Total number of requests
    pub total_requests: u64,
    /// Total response time in milliseconds
    pub total_response_time_ms: u64,
    /// Total response size in bytes
    pub total_response_size_bytes: usize,
    /// Number of cached responses
    pub cached_responses: u64,
    /// Status code distribution
    pub status_code_counts: HashMap<u16, u64>,
    /// Minimum response time
    pub min_response_time_ms: u64,
    /// Maximum response time
    pub max_response_time_ms: u64,
}

impl EndpointMetrics {
    /// Calculate average response time
    pub fn avg_response_time_ms(&self) -> f64 {
        if self.total_requests > 0 {
            self.total_response_time_ms as f64 / self.total_requests as f64
        } else {
            0.0
        }
    }

    /// Calculate average response size
    pub fn avg_response_size_bytes(&self) -> f64 {
        if self.total_requests > 0 {
            self.total_response_size_bytes as f64 / self.total_requests as f64
        } else {
            0.0
        }
    }

    /// Calculate cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_requests > 0 {
            self.cached_responses as f64 / self.total_requests as f64
        } else {
            0.0
        }
    }
}

/// Response optimization service
#[derive(Debug)]
pub struct ResponseOptimizationService {
    pagination_config: PaginationConfig,
    compression_config: CompressionConfig,
    cache: ResponseCache,
    performance_monitor: ResponsePerformanceMonitor,
}

impl ResponseOptimizationService {
    /// Create new response optimization service
    pub fn new(
        pagination_config: PaginationConfig,
        compression_config: CompressionConfig,
        caching_config: CachingConfig,
    ) -> Self {
        Self {
            pagination_config,
            compression_config,
            cache: ResponseCache::new(caching_config),
            performance_monitor: ResponsePerformanceMonitor::new(),
        }
    }

    /// Create paginated response
    pub fn paginate<T: Serialize>(
        &self,
        data: Vec<T>,
        query: &PaginationQuery,
        total_count: u64,
        base_url: &str,
    ) -> PaginatedResponse<T> {
        let limit = query.limit
            .unwrap_or(self.pagination_config.default_page_size)
            .clamp(self.pagination_config.min_page_size, self.pagination_config.max_page_size);

        let page = query.page.unwrap_or(1).max(1);
        let total_pages = if limit > 0 {
            Some(((total_count as f64) / (limit as f64)).ceil() as u32)
        } else {
            None
        };

        let has_more = if let Some(total_pages) = total_pages {
            page < total_pages
        } else {
            false
        };

        // Generate navigation links
        let links = self.generate_pagination_links(base_url, page, limit, total_pages, query);

        let pagination = PaginationMetadata {
            page: Some(page),
            limit,
            total_count,
            total_pages,
            has_more,
            next_cursor: None, // Would be implemented for cursor-based pagination
            prev_cursor: None,
            links,
        };

        PaginatedResponse { data, pagination }
    }

    /// Generate pagination navigation links
    fn generate_pagination_links(
        &self,
        base_url: &str,
        current_page: u32,
        limit: u32,
        total_pages: Option<u32>,
        query: &PaginationQuery,
    ) -> PaginationLinks {
        let mut base_params = vec![format!("limit={}", limit)];
        
        if let Some(sort_by) = &query.sort_by {
            base_params.push(format!("sort_by={}", sort_by));
        }
        
        if let Some(sort_order) = &query.sort_order {
            let order_str = match sort_order {
                SortOrder::Ascending => "asc",
                SortOrder::Descending => "desc",
            };
            base_params.push(format!("sort_order={}", order_str));
        }

        let base_query = if base_params.is_empty() {
            String::new()
        } else {
            format!("?{}", base_params.join("&"))
        };

        let query_prefix = if base_query.is_empty() { 
            "?".to_string() 
        } else { 
            format!("{}&", base_query) 
        };

        let self_link = format!("{}{}page={}", base_url, query_prefix, current_page);

        let first = if current_page > 1 {
            Some(format!("{}{}page=1", base_url, query_prefix))
        } else {
            None
        };

        let prev = if current_page > 1 {
            Some(format!("{}{}page={}", base_url, query_prefix, current_page - 1))
        } else {
            None
        };

        let next = if let Some(total_pages) = total_pages {
            if current_page < total_pages {
                Some(format!("{}{}page={}", base_url, query_prefix, current_page + 1))
            } else {
                None
            }
        } else {
            None
        };

        let last = if let Some(total_pages) = total_pages {
            if current_page < total_pages {
                Some(format!("{}{}page={}", base_url, query_prefix, total_pages))
            } else {
                None
            }
        } else {
            None
        };

        PaginationLinks {
            first,
            prev,
            next,
            last,
            self_link,
        }
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        self.cache.get_stats().await
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Vec<EndpointMetrics> {
        self.performance_monitor.get_all_metrics().await
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_cache(&self) {
        self.cache.cleanup_expired().await;
    }
}

/// Response optimization middleware
pub async fn response_optimization_middleware(
    State(optimization_service): State<Arc<ResponseOptimizationService>>,
    req: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|s| s.to_string());

    // Check cache for GET requests
    if method == "GET" && optimization_service.cache.config.enabled {
        let cache_key = optimization_service.cache.generate_cache_key(&method, &path, query.as_deref());
        
        if let Some(cached_response) = optimization_service.cache.get(&cache_key).await {
            debug!(
                method = %method,
                path = %path,
                cache_key = %cache_key,
                "Serving cached response"
            );

            // Record cache hit metrics
            let response_time_ms = start_time.elapsed().as_millis() as u64;
            optimization_service.performance_monitor.record_response(
                &method,
                &path,
                cached_response.status.as_u16(),
                response_time_ms,
                cached_response.body.len(),
                true, // cached
            ).await;

            // Create response from cache
            let mut response = Response::builder()
                .status(cached_response.status)
                .body(axum::body::Body::from(cached_response.body))
                .unwrap();

            // Copy cached headers
            *response.headers_mut() = cached_response.headers;
            
            // Add cache headers
            response.headers_mut().insert("X-Cache", HeaderValue::from_static("HIT"));
            
            return response;
        }
    }

    // Process request
    let mut response = next.run(req).await;
    let response_time_ms = start_time.elapsed().as_millis() as u64;

    // Get response size (approximate)
    let response_size = estimate_response_size(&response);

    // Record performance metrics
    optimization_service.performance_monitor.record_response(
        &method,
        &path,
        response.status().as_u16(),
        response_time_ms,
        response_size,
        false, // not cached
    ).await;

    // Cache response if applicable
    if method == "GET" 
        && optimization_service.cache.config.enabled
        && optimization_service.cache.config.cacheable_status_codes.contains(&response.status().as_u16())
    {
        let cache_key = optimization_service.cache.generate_cache_key(&method, &path, query.as_deref());
        
        // Extract response body for caching (this is simplified - in practice you'd need to handle streaming)
        // For now, we'll just add cache headers
        response.headers_mut().insert("X-Cache", HeaderValue::from_static("MISS"));
        
        // In a real implementation, you'd extract the body, cache it, and reconstruct the response
    }

    // Add performance headers
    response.headers_mut().insert(
        "X-Response-Time",
        HeaderValue::from_str(&format!("{}ms", response_time_ms)).unwrap(),
    );

    response
}

/// Estimate response size (simplified implementation)
fn estimate_response_size(response: &Response) -> usize {
    // This is a simplified estimation
    // In practice, you'd need to measure the actual body size
    response.headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024) // Default estimate
}

/// Helper function to create paginated response
pub fn create_paginated_response<T: Serialize>(
    data: Vec<T>,
    pagination_query: &PaginationQuery,
    total_count: u64,
    base_url: &str,
    pagination_config: &PaginationConfig,
) -> PaginatedResponse<T> {
    let limit = pagination_query.limit
        .unwrap_or(pagination_config.default_page_size)
        .clamp(pagination_config.min_page_size, pagination_config.max_page_size);

    let page = pagination_query.page.unwrap_or(1).max(1);
    let total_pages = if limit > 0 {
        Some(((total_count as f64) / (limit as f64)).ceil() as u32)
    } else {
        None
    };

    let has_more = if let Some(total_pages) = total_pages {
        page < total_pages
    } else {
        false
    };

    // Generate basic links (simplified)
    let self_link = format!("{}?page={}&limit={}", base_url, page, limit);
    let next = if has_more {
        Some(format!("{}?page={}&limit={}", base_url, page + 1, limit))
    } else {
        None
    };
    let prev = if page > 1 {
        Some(format!("{}?page={}&limit={}", base_url, page - 1, limit))
    } else {
        None
    };

    let links = PaginationLinks {
        first: if page > 1 { Some(format!("{}?page=1&limit={}", base_url, limit)) } else { None },
        prev,
        next,
        last: total_pages.map(|tp| format!("{}?page={}&limit={}", base_url, tp, limit)),
        self_link,
    };

    let pagination = PaginationMetadata {
        page: Some(page),
        limit,
        total_count,
        total_pages,
        has_more,
        next_cursor: None,
        prev_cursor: None,
        links,
    };

    PaginatedResponse { data, pagination }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_config_default() {
        let config = PaginationConfig::default();
        assert_eq!(config.default_page_size, 20);
        assert_eq!(config.max_page_size, 100);
        assert_eq!(config.min_page_size, 1);
        assert!(config.enable_cursor_pagination);
    }

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert!(config.enable_gzip);
        assert!(config.enable_brotli);
        assert_eq!(config.min_size, 1024);
        assert_eq!(config.compression_level, 6);
        assert!(config.compress_content_types.contains(&"application/json".to_string()));
    }

    #[test]
    fn test_caching_config_default() {
        let config = CachingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_ttl, 300);
        assert_eq!(config.max_cache_size, 10000);
        assert!(config.cacheable_methods.contains(&"GET".to_string()));
        assert!(config.cacheable_status_codes.contains(&200));
    }

    #[tokio::test]
    async fn test_response_cache() {
        let config = CachingConfig::default();
        let cache = ResponseCache::new(config);
        
        let key = "test_key".to_string();
        let cached_response = CachedResponse {
            body: b"test response".to_vec(),
            headers: HeaderMap::new(),
            status: StatusCode::OK,
            cached_at: Instant::now(),
            ttl: 300,
            access_count: 0,
        };
        
        // Store in cache
        cache.set(key.clone(), cached_response.clone()).await;
        
        // Retrieve from cache
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some());
        
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.body, b"test response");
        assert_eq!(retrieved.status, StatusCode::OK);
        assert_eq!(retrieved.access_count, 1); // Should increment on access
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CachingConfig::default();
        let cache = ResponseCache::new(config);
        
        let key = "test_key".to_string();
        let cached_response = CachedResponse {
            body: b"test response".to_vec(),
            headers: HeaderMap::new(),
            status: StatusCode::OK,
            cached_at: Instant::now() - Duration::from_secs(400), // Expired
            ttl: 300,
            access_count: 0,
        };
        
        cache.set(key.clone(), cached_response).await;
        
        // Should return None for expired entry
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = ResponsePerformanceMonitor::new();
        
        monitor.record_response(
            "GET",
            "/api/v2/pipelines",
            200,
            150, // 150ms
            1024, // 1KB
            false,
        ).await;
        
        monitor.record_response(
            "GET",
            "/api/v2/pipelines",
            200,
            200, // 200ms
            2048, // 2KB
            true, // cached
        ).await;
        
        let metrics = monitor.get_endpoint_metrics("GET", "/api/v2/pipelines").await;
        assert!(metrics.is_some());
        
        let metrics = metrics.unwrap();
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.cached_responses, 1);
        assert_eq!(metrics.avg_response_time_ms(), 175.0); // (150 + 200) / 2
        assert_eq!(metrics.cache_hit_rate(), 0.5); // 1/2
    }

    #[test]
    fn test_pagination_query_parsing() {
        let query = PaginationQuery {
            page: Some(2),
            limit: Some(50),
            cursor: None,
            sort_by: Some("created_at".to_string()),
            sort_order: Some(SortOrder::Descending),
        };
        
        assert_eq!(query.page, Some(2));
        assert_eq!(query.limit, Some(50));
        assert_eq!(query.sort_by, Some("created_at".to_string()));
    }

    #[test]
    fn test_create_paginated_response() {
        let data = vec!["item1", "item2", "item3"];
        let query = PaginationQuery {
            page: Some(1),
            limit: Some(10),
            cursor: None,
            sort_by: None,
            sort_order: None,
        };
        let config = PaginationConfig::default();
        
        let response = create_paginated_response(
            data,
            &query,
            100, // total count
            "http://example.com/api/items",
            &config,
        );
        
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.pagination.total_count, 100);
        assert_eq!(response.pagination.page, Some(1));
        assert_eq!(response.pagination.limit, 10);
        assert_eq!(response.pagination.total_pages, Some(10));
        assert!(response.pagination.has_more);
        assert!(response.pagination.links.next.is_some());
    }

    #[test]
    fn test_cache_key_generation() {
        let config = CachingConfig::default();
        let cache = ResponseCache::new(config);
        
        let key1 = cache.generate_cache_key("GET", "/api/v2/pipelines", None);
        let key2 = cache.generate_cache_key("GET", "/api/v2/pipelines", Some("page=1&limit=10"));
        
        assert_ne!(key1, key2);
        assert!(key1.contains("GET"));
        assert!(key1.contains("/api/v2/pipelines"));
        assert!(key2.contains("page=1&limit=10"));
    }

    #[test]
    fn test_sort_order_serialization() {
        let asc = SortOrder::Ascending;
        let desc = SortOrder::Descending;
        
        let asc_json = serde_json::to_value(&asc).unwrap();
        let desc_json = serde_json::to_value(&desc).unwrap();
        
        assert_eq!(asc_json, "asc");
        assert_eq!(desc_json, "desc");
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
        
        let data = vec!["item1", "item2", "item3"];
        let query = PaginationQuery {
            page: Some(1),
            limit: Some(2),
            cursor: None,
            sort_by: None,
            sort_order: None,
        };
        
        let response = service.paginate(
            data,
            &query,
            10,
            "http://example.com/api/items",
        );
        
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.pagination.limit, 2);
        assert_eq!(response.pagination.total_count, 10);
    }
}