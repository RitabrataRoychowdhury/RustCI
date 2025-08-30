use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::AppError;
use crate::core::performance::{CriticalPathOptimizer, OptimizerConfig, CompressedData};

/// Response optimization middleware for API endpoints
pub struct ResponseOptimizer {
    config: ResponseOptimizerConfig,
    critical_path_optimizer: Arc<CriticalPathOptimizer>,
    response_cache: Arc<ResponseCache>,
    compression_engine: Arc<ResponseCompressionEngine>,
    metrics: Arc<ResponseOptimizationMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseOptimizerConfig {
    pub enable_caching: bool,
    pub cache_ttl: Duration,
    pub cache_size_limit: usize,
    pub enable_compression: bool,
    pub compression_threshold: usize,
    pub compression_level: u32,
    pub enable_etag: bool,
    pub enable_streaming: bool,
    pub max_response_size: usize,
    pub metrics_enabled: bool,
}

impl Default for ResponseOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_ttl: Duration::from_secs(300), // 5 minutes
            cache_size_limit: 100 * 1024 * 1024, // 100MB
            enable_compression: true,
            compression_threshold: 1024, // 1KB
            compression_level: 6,
            enable_etag: true,
            enable_streaming: true,
            max_response_size: 10 * 1024 * 1024, // 10MB
            metrics_enabled: true,
        }
    }
}

/// Metrics for response optimization
#[derive(Debug, Default)]
pub struct ResponseOptimizationMetrics {
    pub total_requests: std::sync::atomic::AtomicU64,
    pub cache_hits: std::sync::atomic::AtomicU64,
    pub cache_misses: std::sync::atomic::AtomicU64,
    pub compressed_responses: std::sync::atomic::AtomicU64,
    pub compression_savings: std::sync::atomic::AtomicU64,
    pub average_response_time: std::sync::atomic::AtomicU64, // microseconds
    pub etag_matches: std::sync::atomic::AtomicU64,
    pub streaming_responses: std::sync::atomic::AtomicU64,
}

impl ResponseOptimizationMetrics {
    pub fn record_request(&self, response_time: Duration, cache_hit: bool, compressed: bool, etag_match: bool, streamed: bool) {
        use std::sync::atomic::Ordering;
        
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        
        if cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
        
        if compressed {
            self.compressed_responses.fetch_add(1, Ordering::Relaxed);
        }
        
        if etag_match {
            self.etag_matches.fetch_add(1, Ordering::Relaxed);
        }
        
        if streamed {
            self.streaming_responses.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update average response time
        let response_micros = response_time.as_micros() as u64;
        let current_avg = self.average_response_time.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            response_micros
        } else {
            (current_avg * 9 + response_micros) / 10
        };
        self.average_response_time.store(new_avg, Ordering::Relaxed);
    }
    
    pub fn record_compression_savings(&self, savings: u64) {
        use std::sync::atomic::Ordering;
        self.compression_savings.fetch_add(savings, Ordering::Relaxed);
    }
    
    pub fn get_cache_hit_rate(&self) -> f64 {
        use std::sync::atomic::Ordering;
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

/// Response cache for storing frequently accessed responses
pub struct ResponseCache {
    config: ResponseOptimizerConfig,
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    current_size: Arc<std::sync::atomic::AtomicUsize>,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    data: Bytes,
    headers: HeaderMap,
    status: StatusCode,
    created_at: Instant,
    access_count: u32,
    last_accessed: Instant,
    etag: Option<String>,
}

impl ResponseCache {
    pub fn new(config: ResponseOptimizerConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            current_size: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
    
    /// Get cached response
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        let mut cache = self.cache.write().await;
        
        if let Some(cached) = cache.get_mut(key) {
            // Check if still valid
            if cached.created_at.elapsed() < self.config.cache_ttl {
                cached.access_count += 1;
                cached.last_accessed = Instant::now();
                return Some(cached.clone());
            } else {
                // Expired, remove it
                let size = cached.data.len();
                cache.remove(key);
                self.current_size.fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
        }
        
        None
    }
    
    /// Store response in cache
    pub async fn put(
        &self,
        key: String,
        data: Bytes,
        headers: HeaderMap,
        status: StatusCode,
        etag: Option<String>,
    ) -> Result<(), AppError> {
        let data_size = data.len();
        
        // Check size limits
        if data_size > self.config.max_response_size {
            return Ok(()); // Don't cache oversized responses
        }
        
        let mut cache = self.cache.write().await;
        
        // Ensure we don't exceed cache size limit
        while self.current_size.load(std::sync::atomic::Ordering::Relaxed) + data_size > self.config.cache_size_limit {
            self.evict_lru(&mut cache).await;
        }
        
        let cached_response = CachedResponse {
            data,
            headers,
            status,
            created_at: Instant::now(),
            access_count: 1,
            last_accessed: Instant::now(),
            etag,
        };
        
        cache.insert(key, cached_response);
        self.current_size.fetch_add(data_size, std::sync::atomic::Ordering::Relaxed);
        
        Ok(())
    }
    
    /// Evict least recently used entry
    async fn evict_lru(&self, cache: &mut HashMap<String, CachedResponse>) {
        if let Some((lru_key, lru_response)) = cache
            .iter()
            .min_by_key(|(_, response)| response.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            let size = lru_response.data.len();
            cache.remove(&lru_key);
            self.current_size.fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
        }
    }
    
    /// Generate cache key from request
    pub fn generate_cache_key(req: &Request) -> String {
        let uri = req.uri();
        let method = req.method();
        let query = uri.query().unwrap_or("");
        
        // Include relevant headers in cache key
        let mut key_parts = vec![
            method.to_string(),
            uri.path().to_string(),
            query.to_string(),
        ];
        
        // Add accept header to key for content negotiation
        if let Some(accept) = req.headers().get("accept") {
            if let Ok(accept_str) = accept.to_str() {
                key_parts.push(accept_str.to_string());
            }
        }
        
        key_parts.join("|")
    }
}

/// Response compression engine
pub struct ResponseCompressionEngine {
    config: ResponseOptimizerConfig,
}

impl ResponseCompressionEngine {
    pub fn new(config: ResponseOptimizerConfig) -> Self {
        Self { config }
    }
    
    /// Compress response if beneficial
    pub async fn compress_response(
        &self,
        data: &[u8],
        headers: &HeaderMap,
    ) -> Result<Option<CompressedResponseData>, AppError> {
        // Check if compression is enabled and beneficial
        if !self.config.enable_compression || data.len() < self.config.compression_threshold {
            return Ok(None);
        }
        
        // Check if content is already compressed
        if let Some(encoding) = headers.get("content-encoding") {
            if encoding != "identity" {
                return Ok(None); // Already compressed
            }
        }
        
        // Check content type - only compress text-based content
        let should_compress = if let Some(content_type) = headers.get("content-type") {
            if let Ok(ct_str) = content_type.to_str() {
                ct_str.starts_with("text/") ||
                ct_str.starts_with("application/json") ||
                ct_str.starts_with("application/xml") ||
                ct_str.starts_with("application/javascript")
            } else {
                false
            }
        } else {
            true // Assume compressible if no content-type
        };
        
        if !should_compress {
            return Ok(None);
        }
        
        let start_time = Instant::now();
        
        // Use gzip compression
        let compressed = self.gzip_compress(data)?;
        
        let compression_time = start_time.elapsed();
        let compression_ratio = data.len() as f64 / compressed.len() as f64;
        
        // Only use compression if it provides significant savings
        if compression_ratio < 1.1 {
            return Ok(None);
        }
        
        tracing::debug!(
            "Compressed response: {} -> {} bytes (ratio: {:.2}) in {:?}",
            data.len(),
            compressed.len(),
            compression_ratio,
            compression_time
        );
        
        let compressed_size = compressed.len();
        Ok(Some(CompressedResponseData {
            data: compressed,
            original_size: data.len(),
            compressed_size,
            compression_ratio,
            encoding: "gzip".to_string(),
        }))
    }
    
    /// Compress data using gzip
    fn gzip_compress(&self, data: &[u8]) -> Result<Vec<u8>, AppError> {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;
        
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(data)
            .map_err(|e| AppError::Internal(format!("Compression write failed: {}", e)))?;
        
        encoder.finish()
            .map_err(|e| AppError::Internal(format!("Compression finish failed: {}", e)))
    }
}

#[derive(Debug, Clone)]
pub struct CompressedResponseData {
    pub data: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub encoding: String,
}

/// ETag generator for cache validation
pub struct ETagGenerator;

impl ETagGenerator {
    /// Generate ETag for response data
    pub fn generate_etag(data: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        
        format!("\"{}\"", hex::encode(&hash[..8])) // Use first 8 bytes for ETag
    }
    
    /// Check if ETag matches
    pub fn etag_matches(etag: &str, if_none_match: Option<&HeaderValue>) -> bool {
        if let Some(client_etag) = if_none_match {
            if let Ok(client_etag_str) = client_etag.to_str() {
                return client_etag_str == etag || client_etag_str == "*";
            }
        }
        false
    }
}

impl ResponseOptimizer {
    pub fn new(config: ResponseOptimizerConfig) -> Self {
        let optimizer_config = OptimizerConfig {
            cache_size: config.cache_size_limit / 1024, // Convert to entries
            cache_ttl: config.cache_ttl,
            compression_threshold: config.compression_threshold,
            compression_level: config.compression_level,
            ..Default::default()
        };
        
        let critical_path_optimizer = Arc::new(CriticalPathOptimizer::new(optimizer_config));
        let response_cache = Arc::new(ResponseCache::new(config.clone()));
        let compression_engine = Arc::new(ResponseCompressionEngine::new(config.clone()));
        let metrics = Arc::new(ResponseOptimizationMetrics::default());
        
        Self {
            config,
            critical_path_optimizer,
            response_cache,
            compression_engine,
            metrics,
        }
    }
    
    /// Middleware function for response optimization
    pub async fn optimize_response(
        &self,
        req: Request,
        next: Next,
    ) -> Result<Response, AppError> {
        let start_time = Instant::now();
        let cache_key = ResponseCache::generate_cache_key(&req);
        
        let mut cache_hit = false;
        let mut etag_match = false;
        let mut compressed = false;
        let mut streamed = false;
        
        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.response_cache.get(&cache_key).await {
                cache_hit = true;
                
                // Check ETag if enabled
                if self.config.enable_etag {
                    if let Some(etag) = &cached.etag {
                        if ETagGenerator::etag_matches(etag, req.headers().get("if-none-match")) {
                            etag_match = true;
                            
                            let response_time = start_time.elapsed();
                            self.metrics.record_request(response_time, cache_hit, compressed, etag_match, streamed);
                            
                            return Ok((StatusCode::NOT_MODIFIED, cached.headers).into_response());
                        }
                    }
                }
                
                let response_time = start_time.elapsed();
                self.metrics.record_request(response_time, cache_hit, compressed, etag_match, streamed);
                
                return Ok((cached.status, cached.headers, cached.data).into_response());
            }
        }
        
        // Process request through next middleware/handler
        let response = next.run(req).await;
        
        // Extract response parts for optimization
        let (parts, body) = response.into_parts();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await
            .map_err(|e| AppError::Internal(format!("Failed to read response body: {}", e)))?;
        
        let mut optimized_headers = parts.headers.clone();
        let mut optimized_body = body_bytes.clone();
        
        // Generate ETag if enabled
        let etag = if self.config.enable_etag {
            let etag = ETagGenerator::generate_etag(&body_bytes);
            optimized_headers.insert(
                HeaderName::from_static("etag"),
                HeaderValue::from_str(&etag).unwrap(),
            );
            Some(etag)
        } else {
            None
        };
        
        // Compress response if beneficial
        if let Ok(Some(compressed_data)) = self.compression_engine.compress_response(&body_bytes, &parts.headers).await {
            compressed = true;
            optimized_body = Bytes::from(compressed_data.data);
            optimized_headers.insert(
                HeaderName::from_static("content-encoding"),
                HeaderValue::from_str(&compressed_data.encoding).unwrap(),
            );
            optimized_headers.insert(
                HeaderName::from_static("content-length"),
                HeaderValue::from_str(&compressed_data.compressed_size.to_string()).unwrap(),
            );
            
            let savings = compressed_data.original_size.saturating_sub(compressed_data.compressed_size) as u64;
            self.metrics.record_compression_savings(savings);
        }
        
        // Cache response if enabled and appropriate
        if self.config.enable_caching && parts.status.is_success() {
            if let Err(e) = self.response_cache.put(
                cache_key,
                optimized_body.clone(),
                optimized_headers.clone(),
                parts.status,
                etag,
            ).await {
                tracing::warn!("Failed to cache response: {}", e);
            }
        }
        
        let response_time = start_time.elapsed();
        self.metrics.record_request(response_time, cache_hit, compressed, etag_match, streamed);
        
        Ok((parts.status, optimized_headers, optimized_body).into_response())
    }
    
    /// Get optimization metrics
    pub fn get_metrics(&self) -> ResponseOptimizationSummary {
        use std::sync::atomic::Ordering;
        
        let total_requests = self.metrics.total_requests.load(Ordering::Relaxed);
        let cache_hits = self.metrics.cache_hits.load(Ordering::Relaxed);
        let compressed_responses = self.metrics.compressed_responses.load(Ordering::Relaxed);
        let compression_savings = self.metrics.compression_savings.load(Ordering::Relaxed);
        let etag_matches = self.metrics.etag_matches.load(Ordering::Relaxed);
        
        ResponseOptimizationSummary {
            total_requests,
            cache_hit_rate: self.metrics.get_cache_hit_rate(),
            compression_rate: if total_requests > 0 {
                compressed_responses as f64 / total_requests as f64
            } else {
                0.0
            },
            total_compression_savings: compression_savings,
            etag_match_rate: if total_requests > 0 {
                etag_matches as f64 / total_requests as f64
            } else {
                0.0
            },
            average_response_time: Duration::from_micros(
                self.metrics.average_response_time.load(Ordering::Relaxed)
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResponseOptimizationSummary {
    pub total_requests: u64,
    pub cache_hit_rate: f64,
    pub compression_rate: f64,
    pub total_compression_savings: u64,
    pub etag_match_rate: f64,
    pub average_response_time: Duration,
}

/// Create response optimization middleware
pub fn create_response_optimizer_middleware(
    config: ResponseOptimizerConfig,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, AppError>> + Send>> + Clone {
    let optimizer = Arc::new(ResponseOptimizer::new(config));
    
    move |req: Request, next: Next| {
        let optimizer = Arc::clone(&optimizer);
        Box::pin(async move {
            optimizer.optimize_response(req, next).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::Method, body::Body};
    
    #[tokio::test]
    async fn test_response_cache() {
        let config = ResponseOptimizerConfig::default();
        let cache = ResponseCache::new(config);
        
        let key = "test_key".to_string();
        let data = Bytes::from("test response data");
        let headers = HeaderMap::new();
        let status = StatusCode::OK;
        
        // Store response
        cache.put(key.clone(), data.clone(), headers.clone(), status, None).await.unwrap();
        
        // Retrieve response
        let cached = cache.get(&key).await;
        assert!(cached.is_some());
        
        let cached_response = cached.unwrap();
        assert_eq!(cached_response.data, data);
        assert_eq!(cached_response.status, status);
    }
    
    #[tokio::test]
    async fn test_compression_engine() {
        let config = ResponseOptimizerConfig {
            compression_threshold: 10,
            ..Default::default()
        };
        let engine = ResponseCompressionEngine::new(config);
        
        let data = b"This is a test response that should be compressed because it's longer than the threshold";
        let headers = HeaderMap::new();
        
        let compressed = engine.compress_response(data, &headers).await.unwrap();
        assert!(compressed.is_some());
        
        let compressed_data = compressed.unwrap();
        assert!(compressed_data.compressed_size < compressed_data.original_size);
        assert!(compressed_data.compression_ratio > 1.0);
    }
    
    #[tokio::test]
    async fn test_etag_generation() {
        let data = b"test data for etag";
        let etag = ETagGenerator::generate_etag(data);
        
        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert!(etag.len() > 2);
        
        // Same data should generate same ETag
        let etag2 = ETagGenerator::generate_etag(data);
        assert_eq!(etag, etag2);
    }
    
    #[test]
    fn test_cache_key_generation() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/test?param=value")
            .header("accept", "application/json")
            .body(Body::empty())
            .unwrap();
        
        let key = ResponseCache::generate_cache_key(&req);
        assert!(key.contains("GET"));
        assert!(key.contains("/api/test"));
        assert!(key.contains("param=value"));
        assert!(key.contains("application/json"));
    }
}