//! Fuzzy Matching Engine for Service Discovery
//!
//! This module implements a high-performance fuzzy matching engine using nucleo
//! for service discovery operations. It provides ranked fuzzy search capabilities
//! with timeout protection and caching to ensure control plane operations don't
//! impact hot-path performance.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;

use nucleo::{Matcher, Utf32Str};

use crate::valkyrie::routing::enhanced_registries::{EnhancedServiceEntry, ServiceHealthStatus};

/// Maximum number of fuzzy match results to return
const MAX_FUZZY_RESULTS: usize = 100;

/// Default timeout for fuzzy matching operations
const DEFAULT_FUZZY_TIMEOUT: Duration = Duration::from_millis(50);

/// Minimum score threshold for fuzzy matches
const MIN_MATCH_SCORE: u16 = 10;

/// Fuzzy match result with score and matched ranges
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// Service entry that matched
    pub service: EnhancedServiceEntry,
    /// Match score (higher is better)
    pub score: u16,
    /// Matched character ranges in the service name
    pub matched_ranges: Vec<(usize, usize)>,
    /// Match timestamp
    pub matched_at: SystemTime,
}

/// Fuzzy matching engine configuration
#[derive(Debug, Clone)]
pub struct FuzzyMatchingConfig {
    /// Maximum number of results to return
    pub max_results: usize,
    /// Timeout for matching operations
    pub timeout: Duration,
    /// Minimum score threshold
    pub min_score: u16,
    /// Enable result caching
    pub enable_caching: bool,
    /// Cache TTL
    pub cache_ttl: Duration,
}

impl Default for FuzzyMatchingConfig {
    fn default() -> Self {
        Self {
            max_results: MAX_FUZZY_RESULTS,
            timeout: DEFAULT_FUZZY_TIMEOUT,
            min_score: MIN_MATCH_SCORE,
            enable_caching: true,
            cache_ttl: Duration::from_secs(60),
        }
    }
}

/// Service index entry for fuzzy matching
#[derive(Debug, Clone)]
struct ServiceIndexEntry {
    /// Service ID
    pub service_id: Uuid,
    /// Searchable text (service name + tags)
    pub searchable_text: String,
    /// Last updated timestamp
    pub updated_at: SystemTime,
}

/// Cached fuzzy match result
#[derive(Debug, Clone)]
struct CachedResult {
    /// Cached matches
    pub matches: Vec<FuzzyMatch>,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Cache TTL
    pub ttl: Duration,
}

impl CachedResult {
    /// Check if the cached result is still valid
    pub fn is_valid(&self) -> bool {
        SystemTime::now()
            .duration_since(self.cached_at)
            .map(|elapsed| elapsed < self.ttl)
            .unwrap_or(false)
    }
}

/// High-performance fuzzy matching engine for service discovery
pub struct FuzzyMatchingEngine {
    /// Configuration
    config: FuzzyMatchingConfig,
    /// Service index for fuzzy matching
    service_index: Arc<RwLock<HashMap<Uuid, ServiceIndexEntry>>>,
    /// Services data
    services: Arc<RwLock<HashMap<Uuid, EnhancedServiceEntry>>>,
    /// Result cache
    result_cache: Arc<RwLock<HashMap<String, CachedResult>>>,
    /// Nucleo matcher
    matcher: Arc<RwLock<Matcher>>,
    /// Performance metrics
    metrics: Arc<RwLock<FuzzyMatchingMetrics>>,
}

/// Performance metrics for fuzzy matching
#[derive(Debug, Clone)]
pub struct FuzzyMatchingMetrics {
    /// Total number of queries
    pub total_queries: u64,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Total matching time (nanoseconds)
    pub total_match_time_ns: u64,
    /// Average matching time (nanoseconds)
    pub avg_match_time_ns: u64,
    /// Number of timeouts
    pub timeouts: u64,
    /// Last updated timestamp
    pub last_updated: SystemTime,
}

impl Default for FuzzyMatchingMetrics {
    fn default() -> Self {
        Self {
            total_queries: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_match_time_ns: 0,
            avg_match_time_ns: 0,
            timeouts: 0,
            last_updated: SystemTime::now(),
        }
    }
}

impl FuzzyMatchingEngine {
    /// Create a new fuzzy matching engine
    pub fn new(config: FuzzyMatchingConfig) -> Self {
        Self {
            config,
            service_index: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(RwLock::new(HashMap::new())),
            result_cache: Arc::new(RwLock::new(HashMap::new())),
            matcher: Arc::new(RwLock::new(Matcher::new(nucleo::Config::DEFAULT))),
            metrics: Arc::new(RwLock::new(FuzzyMatchingMetrics::default())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(FuzzyMatchingConfig::default())
    }

    /// Index a service for fuzzy matching
    pub async fn index_service(&self, service: EnhancedServiceEntry) {
        let service_id = service.id;

        // Create searchable text from service name and tags
        let mut searchable_parts = vec![service.name.clone()];
        for (key, value) in &service.tags {
            searchable_parts.push(format!("{}:{}", key, value));
            searchable_parts.push(value.clone());
        }
        let searchable_text = searchable_parts.join(" ");

        // Update service index
        {
            let mut index = self.service_index.write().await;
            index.insert(
                service_id,
                ServiceIndexEntry {
                    service_id,
                    searchable_text,
                    updated_at: SystemTime::now(),
                },
            );
        }

        // Update services data
        {
            let mut services = self.services.write().await;
            services.insert(service_id, service);
        }

        // Clear cache since index has changed
        if self.config.enable_caching {
            let mut cache = self.result_cache.write().await;
            cache.clear();
        }
    }

    /// Remove a service from the index
    pub async fn remove_service(&self, service_id: &Uuid) {
        // Remove from index
        {
            let mut index = self.service_index.write().await;
            index.remove(service_id);
        }

        // Remove from services
        {
            let mut services = self.services.write().await;
            services.remove(service_id);
        }

        // Clear cache since index has changed
        if self.config.enable_caching {
            let mut cache = self.result_cache.write().await;
            cache.clear();
        }
    }

    /// Perform fuzzy matching with timeout protection
    pub async fn fuzzy_match(&self, query: &str) -> Result<Vec<FuzzyMatch>, FuzzyMatchError> {
        let start_time = Instant::now();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_queries += 1;
        }

        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.get_cached_result(query).await {
                let mut metrics = self.metrics.write().await;
                metrics.cache_hits += 1;
                return Ok(cached.matches);
            } else {
                let mut metrics = self.metrics.write().await;
                metrics.cache_misses += 1;
            }
        }

        // Perform fuzzy matching with timeout
        let matches = tokio::time::timeout(self.config.timeout, self.perform_fuzzy_match(query))
            .await
            .map_err(|_| FuzzyMatchError::Timeout)?;

        let matches = matches?;

        // Cache the result
        if self.config.enable_caching {
            self.cache_result(query, &matches).await;
        }

        // Update metrics
        let elapsed = start_time.elapsed();
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_match_time_ns += elapsed.as_nanos() as u64;
            metrics.avg_match_time_ns = metrics.total_match_time_ns / metrics.total_queries;
            metrics.last_updated = SystemTime::now();
        }

        Ok(matches)
    }

    /// Perform the actual fuzzy matching
    async fn perform_fuzzy_match(&self, query: &str) -> Result<Vec<FuzzyMatch>, FuzzyMatchError> {
        let mut results = Vec::new();

        // Get current service index and services
        let (index, services) = {
            let index = self.service_index.read().await;
            let services = self.services.read().await;
            (index.clone(), services.clone())
        };

        // Prepare query for nucleo
        let mut query_buf = Vec::new();
        let query_utf32 = Utf32Str::new(query, &mut query_buf);

        // Get matcher
        let mut matcher = self.matcher.write().await;

        // Score each service
        let mut scored_matches = Vec::new();

        for (service_id, index_entry) in &index {
            if let Some(service) = services.get(service_id) {
                // Skip unhealthy services
                if service.health_status == ServiceHealthStatus::Unhealthy {
                    continue;
                }

                // Prepare service text for matching
                let mut service_buf = Vec::new();
                let service_utf32 = Utf32Str::new(&index_entry.searchable_text, &mut service_buf);

                // Perform fuzzy matching
                if let Some(score) = matcher.fuzzy_match(service_utf32, query_utf32) {
                    if score >= self.config.min_score {
                        // For now, create empty ranges since we don't have match indices
                        // In a full implementation, we'd use a different nucleo API to get indices
                        let matched_ranges = Vec::new();

                        scored_matches.push((
                            score,
                            FuzzyMatch {
                                service: service.clone(),
                                score,
                                matched_ranges,
                                matched_at: SystemTime::now(),
                            },
                        ));
                    }
                }
            }
        }

        // Sort by score (descending) and take top results
        scored_matches.sort_by(|a, b| b.0.cmp(&a.0));
        results.extend(
            scored_matches
                .into_iter()
                .take(self.config.max_results)
                .map(|(_, fuzzy_match)| fuzzy_match),
        );

        Ok(results)
    }

    /// Convert match indices to character ranges
    fn indices_to_ranges(&self, indices: &[u32]) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut start = None;
        let mut last_idx = None;

        for &idx in indices {
            let idx = idx as usize;

            match (start, last_idx) {
                (None, None) => {
                    start = Some(idx);
                    last_idx = Some(idx);
                }
                (Some(_s), Some(last)) if idx == last + 1 => {
                    last_idx = Some(idx);
                }
                (Some(s), Some(last)) => {
                    ranges.push((s, last + 1));
                    start = Some(idx);
                    last_idx = Some(idx);
                }
                _ => unreachable!(),
            }
        }

        // Add the final range
        if let (Some(s), Some(last)) = (start, last_idx) {
            ranges.push((s, last + 1));
        }

        ranges
    }

    /// Get cached result if available and valid
    async fn get_cached_result(&self, query: &str) -> Option<CachedResult> {
        let cache = self.result_cache.read().await;
        if let Some(cached) = cache.get(query) {
            if cached.is_valid() {
                return Some(cached.clone());
            }
        }
        None
    }

    /// Cache a result
    async fn cache_result(&self, query: &str, matches: &[FuzzyMatch]) {
        let mut cache = self.result_cache.write().await;
        cache.insert(
            query.to_string(),
            CachedResult {
                matches: matches.to_vec(),
                cached_at: SystemTime::now(),
                ttl: self.config.cache_ttl,
            },
        );
    }

    /// Get performance metrics
    pub async fn get_metrics(&self) -> FuzzyMatchingMetrics {
        self.metrics.read().await.clone()
    }

    /// Clear the result cache
    pub async fn clear_cache(&self) {
        let mut cache = self.result_cache.write().await;
        cache.clear();
    }

    /// Get the number of indexed services
    pub async fn indexed_service_count(&self) -> usize {
        let index = self.service_index.read().await;
        index.len()
    }

    /// Update engine configuration
    pub async fn update_config(&self, new_config: FuzzyMatchingConfig) {
        // Note: This would require making config mutable or using interior mutability
        // For now, this is a placeholder showing the intended API
        let _ = new_config;
    }
}

/// Errors that can occur during fuzzy matching
#[derive(Debug, thiserror::Error)]
pub enum FuzzyMatchError {
    #[error("Fuzzy matching operation timed out")]
    Timeout,
    #[error("Invalid query: {message}")]
    InvalidQuery { message: String },
    #[error("Index error: {message}")]
    IndexError { message: String },
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_service(id: Uuid, name: &str, tags: Vec<(&str, &str)>) -> EnhancedServiceEntry {
        let mut tag_map = HashMap::new();
        for (key, value) in tags {
            tag_map.insert(key.to_string(), value.to_string());
        }

        EnhancedServiceEntry {
            id,
            name: name.to_string(),
            version: "1.0.0".to_string(),
            endpoints: vec![],
            tags: tag_map,
            health_status: ServiceHealthStatus::Healthy,
            metrics: Default::default(),
            registered_at: SystemTime::now(),
            last_health_check: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_fuzzy_matching_engine_creation() {
        let engine = FuzzyMatchingEngine::with_defaults();
        assert_eq!(engine.indexed_service_count().await, 0);
    }

    #[tokio::test]
    async fn test_service_indexing() {
        let engine = FuzzyMatchingEngine::with_defaults();
        let service = create_test_service(
            Uuid::new_v4(),
            "user-service",
            vec![("env", "production"), ("team", "backend")],
        );

        engine.index_service(service).await;
        assert_eq!(engine.indexed_service_count().await, 1);
    }

    #[tokio::test]
    async fn test_fuzzy_matching() {
        let engine = FuzzyMatchingEngine::with_defaults();

        // Index some test services
        let services = vec![
            create_test_service(Uuid::new_v4(), "user-service", vec![("env", "prod")]),
            create_test_service(Uuid::new_v4(), "user-auth-service", vec![("env", "prod")]),
            create_test_service(Uuid::new_v4(), "payment-service", vec![("env", "prod")]),
            create_test_service(
                Uuid::new_v4(),
                "notification-service",
                vec![("env", "prod")],
            ),
        ];

        for service in services {
            engine.index_service(service).await;
        }

        // Test fuzzy matching
        let matches = engine.fuzzy_match("user").await.unwrap();
        assert!(!matches.is_empty());

        // Should find user-related services
        let service_names: Vec<String> = matches.iter().map(|m| m.service.name.clone()).collect();

        assert!(service_names.iter().any(|name| name.contains("user")));
    }

    #[tokio::test]
    async fn test_service_removal() {
        let engine = FuzzyMatchingEngine::with_defaults();
        let service_id = Uuid::new_v4();
        let service = create_test_service(service_id, "test-service", vec![]);

        engine.index_service(service).await;
        assert_eq!(engine.indexed_service_count().await, 1);

        engine.remove_service(&service_id).await;
        assert_eq!(engine.indexed_service_count().await, 0);
    }

    #[tokio::test]
    async fn test_caching() {
        let mut config = FuzzyMatchingConfig::default();
        config.enable_caching = true;
        config.cache_ttl = Duration::from_secs(1);

        let engine = FuzzyMatchingEngine::new(config);
        let service = create_test_service(Uuid::new_v4(), "cached-service", vec![]);

        engine.index_service(service).await;

        // First query should be a cache miss
        let _matches1 = engine.fuzzy_match("cached").await.unwrap();
        let metrics1 = engine.get_metrics().await;
        assert_eq!(metrics1.cache_misses, 1);
        assert_eq!(metrics1.cache_hits, 0);

        // Second query should be a cache hit
        let _matches2 = engine.fuzzy_match("cached").await.unwrap();
        let metrics2 = engine.get_metrics().await;
        assert_eq!(metrics2.cache_hits, 1);
    }

    #[tokio::test]
    async fn test_timeout_protection() {
        let mut config = FuzzyMatchingConfig::default();
        config.timeout = Duration::from_nanos(1); // Very short timeout

        let engine = FuzzyMatchingEngine::new(config);

        // Add many services to make matching slow
        for i in 0..1000 {
            let service = create_test_service(
                Uuid::new_v4(),
                &format!("service-{}", i),
                vec![("index", &i.to_string())],
            );
            engine.index_service(service).await;
        }

        // This should timeout
        let result = engine.fuzzy_match("service").await;
        assert!(matches!(result, Err(FuzzyMatchError::Timeout)));
    }
}
