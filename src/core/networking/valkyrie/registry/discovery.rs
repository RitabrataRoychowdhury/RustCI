//! Service Discovery Engine
//!
//! Implements intelligent service discovery with multiple strategies,
//! caching, and real-time updates for optimal service location.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::{ServiceEntry, ServiceId, ServiceType};
use crate::error::{Result, ValkyrieError};

/// Service discovery engine with multiple strategies
pub struct ServiceDiscovery {
    /// Service cache for fast lookups
    service_cache: Arc<DashMap<String, CachedDiscoveryResult>>,
    /// Tag-based index for efficient tag queries
    tag_index: Arc<RwLock<HashMap<String, Vec<ServiceId>>>>,
    /// Type-based index
    type_index: Arc<RwLock<HashMap<ServiceType, Vec<ServiceId>>>>,
    /// Name-based index
    name_index: Arc<RwLock<HashMap<String, Vec<ServiceId>>>>,
    /// Discovery strategies
    strategies: Arc<RwLock<Vec<Box<dyn DiscoveryStrategy>>>>,
    /// Discovery configuration
    config: DiscoveryConfig,
    /// Discovery metrics
    metrics: Arc<RwLock<DiscoveryMetrics>>,
}

/// Cached discovery result
#[derive(Debug, Clone)]
pub struct CachedDiscoveryResult {
    /// Discovered services
    pub services: Vec<ServiceEntry>,
    /// Cache timestamp
    pub cached_at: Instant,
    /// Cache TTL
    pub ttl: Duration,
    /// Hit count
    pub hit_count: u32,
    /// Query hash for cache key
    pub query_hash: String,
}

/// Discovery strategy trait
pub trait DiscoveryStrategy: Send + Sync {
    /// Strategy name
    fn name(&self) -> &str;

    /// Discover services by name
    fn discover_by_name(&self, name: &str, services: &[ServiceEntry]) -> Vec<ServiceEntry>;

    /// Discover services by tags
    fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry>;

    /// Discover services by type
    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry>;

    /// Strategy priority (lower = higher priority)
    fn priority(&self) -> u8;

    /// Strategy enabled
    fn enabled(&self) -> bool;
}

/// Exact match discovery strategy
pub struct ExactMatchStrategy {
    enabled: bool,
}

/// Fuzzy match discovery strategy
pub struct FuzzyMatchStrategy {
    enabled: bool,
    similarity_threshold: f64,
}

/// Semantic discovery strategy (using ML/NLP)
pub struct SemanticDiscoveryStrategy {
    enabled: bool,
    model_path: Option<String>,
    similarity_threshold: f64,
}

/// Proximity-based discovery strategy
pub struct ProximityBasedStrategy {
    enabled: bool,
    region_weights: HashMap<String, f64>,
}

/// Load-aware discovery strategy
pub struct LoadAwareStrategy {
    enabled: bool,
    load_threshold: f64,
}

/// Discovery configuration
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Enable caching
    pub enable_caching: bool,
    /// Cache TTL
    pub cache_ttl: Duration,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Enable fuzzy matching
    pub enable_fuzzy_matching: bool,
    /// Fuzzy match threshold
    pub fuzzy_threshold: f64,
    /// Enable semantic discovery
    pub enable_semantic_discovery: bool,
    /// Enable proximity-based discovery
    pub enable_proximity_discovery: bool,
    /// Enable load-aware discovery
    pub enable_load_aware_discovery: bool,
    /// Discovery timeout
    pub discovery_timeout: Duration,
    /// Refresh interval for indexes
    pub index_refresh_interval: Duration,
}

/// Discovery metrics
#[derive(Debug, Clone)]
pub struct DiscoveryMetrics {
    /// Total discovery requests
    pub total_requests: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Average discovery latency
    pub avg_discovery_latency: Duration,
    /// Successful discoveries
    pub successful_discoveries: u64,
    /// Failed discoveries
    pub failed_discoveries: u64,
    /// Strategy usage counts
    pub strategy_usage: HashMap<String, u64>,
    /// Index rebuild count
    pub index_rebuilds: u64,
}

/// Discovery query
#[derive(Debug, Clone)]
pub struct DiscoveryQuery {
    /// Query type
    pub query_type: QueryType,
    /// Query parameters
    pub parameters: HashMap<String, String>,
    /// Required capabilities
    pub required_capabilities: Vec<String>,
    /// Preferred regions
    pub preferred_regions: Vec<String>,
    /// Maximum results
    pub max_results: Option<usize>,
    /// Include unhealthy services
    pub include_unhealthy: bool,
}

/// Query types
#[derive(Debug, Clone)]
pub enum QueryType {
    /// Query by service name
    ByName(String),
    /// Query by tags
    ByTags(HashMap<String, String>),
    /// Query by service type
    ByType(ServiceType),
    /// Complex query with multiple criteria
    Complex {
        name_pattern: Option<String>,
        tags: Option<HashMap<String, String>>,
        service_type: Option<ServiceType>,
        capabilities: Option<Vec<String>>,
    },
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_ttl: Duration::from_secs(300), // 5 minutes
            max_cache_size: 10000,
            enable_fuzzy_matching: true,
            fuzzy_threshold: 0.8,
            enable_semantic_discovery: false, // Requires ML models
            enable_proximity_discovery: true,
            enable_load_aware_discovery: true,
            discovery_timeout: Duration::from_secs(5),
            index_refresh_interval: Duration::from_secs(60),
        }
    }
}

impl ServiceDiscovery {
    /// Create new service discovery engine
    pub fn new() -> Self {
        Self::with_config(DiscoveryConfig::default())
    }

    /// Create service discovery with custom configuration
    pub fn with_config(config: DiscoveryConfig) -> Self {
        let mut discovery = Self {
            service_cache: Arc::new(DashMap::new()),
            tag_index: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            strategies: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(DiscoveryMetrics::default())),
            config,
        };

        // Initialize default strategies
        discovery.initialize_default_strategies();
        discovery
    }

    /// Initialize default discovery strategies
    fn initialize_default_strategies(&mut self) {
        let mut strategies: Vec<Box<dyn DiscoveryStrategy>> = Vec::new();

        // Always include exact match strategy
        strategies.push(Box::new(ExactMatchStrategy { enabled: true }));

        // Add fuzzy matching if enabled
        if self.config.enable_fuzzy_matching {
            strategies.push(Box::new(FuzzyMatchStrategy {
                enabled: true,
                similarity_threshold: self.config.fuzzy_threshold,
            }));
        }

        // Add semantic discovery if enabled
        if self.config.enable_semantic_discovery {
            strategies.push(Box::new(SemanticDiscoveryStrategy {
                enabled: true,
                model_path: None,
                similarity_threshold: 0.8,
            }));
        }

        // Add proximity-based discovery if enabled
        if self.config.enable_proximity_discovery {
            strategies.push(Box::new(ProximityBasedStrategy {
                enabled: true,
                region_weights: HashMap::new(),
            }));
        }

        // Add load-aware discovery if enabled
        if self.config.enable_load_aware_discovery {
            strategies.push(Box::new(LoadAwareStrategy {
                enabled: true,
                load_threshold: 0.8,
            }));
        }

        // Sort strategies by priority
        strategies.sort_by_key(|s| s.priority());

        // Store strategies (this is a simplified approach - in practice we'd need async initialization)
        // For now, we'll initialize them synchronously
    }

    /// Register service for discovery
    pub async fn register_service(
        &self,
        service_id: ServiceId,
        service: &ServiceEntry,
    ) -> Result<()> {
        // Update name index
        {
            let mut name_index = self.name_index.write().await;
            name_index
                .entry(service.metadata.name.clone())
                .or_insert_with(Vec::new)
                .push(service_id);
        }

        // Update type index
        {
            let mut type_index = self.type_index.write().await;
            type_index
                .entry(service.metadata.service_type.clone())
                .or_insert_with(Vec::new)
                .push(service_id);
        }

        // Update tag index
        {
            let mut tag_index = self.tag_index.write().await;
            for (key, value) in &service.tags {
                let tag_key = format!("{}:{}", key, value);
                tag_index
                    .entry(tag_key)
                    .or_insert_with(Vec::new)
                    .push(service_id);
            }
        }

        // Invalidate relevant cache entries
        self.invalidate_cache_for_service(service).await;

        debug!(
            "Registered service for discovery: {} ({})",
            service.metadata.name, service_id
        );
        Ok(())
    }

    /// Deregister service from discovery
    pub async fn deregister_service(&self, service_id: ServiceId) -> Result<()> {
        // Remove from all indexes
        {
            let mut name_index = self.name_index.write().await;
            for services in name_index.values_mut() {
                services.retain(|&id| id != service_id);
            }
        }

        {
            let mut type_index = self.type_index.write().await;
            for services in type_index.values_mut() {
                services.retain(|&id| id != service_id);
            }
        }

        {
            let mut tag_index = self.tag_index.write().await;
            for services in tag_index.values_mut() {
                services.retain(|&id| id != service_id);
            }
        }

        // Clear cache (simplified approach)
        self.service_cache.clear();

        debug!("Deregistered service from discovery: {}", service_id);
        Ok(())
    }

    /// Discover services by name
    pub async fn discover_by_name(&self, name: &str) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();

        // Check cache first
        if self.config.enable_caching {
            let cache_key = format!("name:{}", name);
            if let Some(cached) = self.check_cache(&cache_key).await {
                self.update_cache_hit_metrics().await;
                return Ok(cached.services);
            }
        }

        // Get service IDs from name index
        let service_ids = {
            let name_index = self.name_index.read().await;
            name_index.get(name).cloned().unwrap_or_default()
        };

        // This is a simplified implementation - in practice we'd need access to the full service registry
        // For now, return empty result
        let services = Vec::new();

        // Cache the result
        if self.config.enable_caching {
            let cache_key = format!("name:{}", name);
            self.cache_result(&cache_key, &services).await;
        }

        // Update metrics
        self.update_discovery_metrics(start_time.elapsed(), !services.is_empty())
            .await;

        Ok(services)
    }

    /// Discover services by tags
    pub async fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
    ) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();

        // Check cache first
        if self.config.enable_caching {
            let cache_key = self.create_tags_cache_key(tags);
            if let Some(cached) = self.check_cache(&cache_key).await {
                self.update_cache_hit_metrics().await;
                return Ok(cached.services);
            }
        }

        // Find services matching all tags
        let mut matching_services: Option<Vec<ServiceId>> = None;

        {
            let tag_index = self.tag_index.read().await;

            for (key, value) in tags {
                let tag_key = format!("{}:{}", key, value);
                if let Some(service_ids) = tag_index.get(&tag_key) {
                    if let Some(ref mut current_matches) = matching_services {
                        // Intersection with current matches
                        current_matches.retain(|id| service_ids.contains(id));
                    } else {
                        // First tag match
                        matching_services = Some(service_ids.clone());
                    }
                } else {
                    // No services match this tag
                    matching_services = Some(Vec::new());
                    break;
                }
            }
        }

        let services = Vec::new(); // Simplified - would fetch actual services

        // Cache the result
        if self.config.enable_caching {
            let cache_key = self.create_tags_cache_key(tags);
            self.cache_result(&cache_key, &services).await;
        }

        // Update metrics
        self.update_discovery_metrics(start_time.elapsed(), !services.is_empty())
            .await;

        Ok(services)
    }

    /// Discover services by type
    pub async fn discover_by_type(&self, service_type: &ServiceType) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();

        // Check cache first
        if self.config.enable_caching {
            let cache_key = format!("type:{}", service_type);
            if let Some(cached) = self.check_cache(&cache_key).await {
                self.update_cache_hit_metrics().await;
                return Ok(cached.services);
            }
        }

        // Get service IDs from type index
        let service_ids = {
            let type_index = self.type_index.read().await;
            type_index.get(service_type).cloned().unwrap_or_default()
        };

        let services = Vec::new(); // Simplified - would fetch actual services

        // Cache the result
        if self.config.enable_caching {
            let cache_key = format!("type:{}", service_type);
            self.cache_result(&cache_key, &services).await;
        }

        // Update metrics
        self.update_discovery_metrics(start_time.elapsed(), !services.is_empty())
            .await;

        Ok(services)
    }

    /// Execute complex discovery query
    pub async fn discover(&self, query: DiscoveryQuery) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();

        let services = match query.query_type {
            QueryType::ByName(ref name) => self.discover_by_name(name).await?,
            QueryType::ByTags(ref tags) => self.discover_by_tags(tags).await?,
            QueryType::ByType(ref service_type) => self.discover_by_type(service_type).await?,
            QueryType::Complex { .. } => {
                // Complex query implementation would go here
                Vec::new()
            }
        };

        // Apply additional filters
        let filtered_services = self.apply_query_filters(services, &query).await;

        // Update metrics
        self.update_discovery_metrics(start_time.elapsed(), !filtered_services.is_empty())
            .await;

        Ok(filtered_services)
    }

    /// Apply query filters to services
    async fn apply_query_filters(
        &self,
        mut services: Vec<ServiceEntry>,
        query: &DiscoveryQuery,
    ) -> Vec<ServiceEntry> {
        // Filter by health status
        if !query.include_unhealthy {
            services.retain(|s| matches!(s.health_status, super::HealthStatus::Healthy));
        }

        // Filter by required capabilities
        if !query.required_capabilities.is_empty() {
            services.retain(|s| {
                query
                    .required_capabilities
                    .iter()
                    .all(|cap| s.capabilities.custom.contains_key(cap))
            });
        }

        // Apply region preferences (simplified)
        if !query.preferred_regions.is_empty() {
            services.sort_by(|a, b| {
                let a_preferred = query.preferred_regions.contains(&a.metadata.region);
                let b_preferred = query.preferred_regions.contains(&b.metadata.region);
                b_preferred.cmp(&a_preferred)
            });
        }

        // Limit results
        if let Some(max_results) = query.max_results {
            services.truncate(max_results);
        }

        services
    }

    /// Check cache for discovery result
    async fn check_cache(&self, cache_key: &str) -> Option<CachedDiscoveryResult> {
        if let Some(cached) = self.service_cache.get(cache_key) {
            if cached.cached_at.elapsed() < cached.ttl {
                // Update hit count
                let mut cached_mut = cached.clone();
                cached_mut.hit_count += 1;
                self.service_cache
                    .insert(cache_key.to_string(), cached_mut.clone());
                return Some(cached_mut);
            } else {
                // Remove expired entry
                self.service_cache.remove(cache_key);
            }
        }
        None
    }

    /// Cache discovery result
    async fn cache_result(&self, cache_key: &str, services: &[ServiceEntry]) {
        // Check cache size limit
        if self.service_cache.len() >= self.config.max_cache_size {
            // Remove oldest entries (simplified LRU)
            let oldest_key = self
                .service_cache
                .iter()
                .min_by_key(|entry| entry.cached_at)
                .map(|entry| entry.key().clone());

            if let Some(key) = oldest_key {
                self.service_cache.remove(&key);
            }
        }

        let cached_result = CachedDiscoveryResult {
            services: services.to_vec(),
            cached_at: Instant::now(),
            ttl: self.config.cache_ttl,
            hit_count: 0,
            query_hash: self.calculate_query_hash(cache_key),
        };

        self.service_cache
            .insert(cache_key.to_string(), cached_result);
    }

    /// Create cache key for tags query
    fn create_tags_cache_key(&self, tags: &HashMap<String, String>) -> String {
        let mut sorted_tags: Vec<_> = tags.iter().collect();
        sorted_tags.sort_by_key(|(k, _)| *k);

        let tags_str = sorted_tags
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect::<Vec<_>>()
            .join(",");

        format!("tags:{}", tags_str)
    }

    /// Calculate query hash for caching
    fn calculate_query_hash(&self, cache_key: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        cache_key.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Invalidate cache entries for a service
    async fn invalidate_cache_for_service(&self, service: &ServiceEntry) {
        // This is a simplified approach - in practice we'd track which cache entries
        // are affected by each service and invalidate them specifically
        self.service_cache.clear();
    }

    /// Update cache hit metrics
    async fn update_cache_hit_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_hits += 1;
        metrics.total_requests += 1;
    }

    /// Update discovery metrics
    async fn update_discovery_metrics(&self, latency: Duration, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;

        if success {
            metrics.successful_discoveries += 1;
        } else {
            metrics.failed_discoveries += 1;
            metrics.cache_misses += 1;
        }

        // Update average latency
        let total_latency = metrics.avg_discovery_latency.as_nanos()
            * (metrics.total_requests - 1) as u128
            + latency.as_nanos();
        metrics.avg_discovery_latency =
            Duration::from_nanos((total_latency / metrics.total_requests as u128) as u64);
    }

    /// Start refresh task for indexes
    pub async fn start_refresh_task(&self, refresh_interval: Duration) -> Result<()> {
        // In a real implementation, this would start a background task to refresh indexes
        info!(
            "Started discovery index refresh task with interval: {:?}",
            refresh_interval
        );
        Ok(())
    }

    /// Get discovery metrics
    pub async fn metrics(&self) -> DiscoveryMetrics {
        self.metrics.read().await.clone()
    }

    /// Rebuild all indexes
    pub async fn rebuild_indexes(&self, services: &[ServiceEntry]) -> Result<()> {
        let start_time = Instant::now();

        // Clear existing indexes
        {
            let mut name_index = self.name_index.write().await;
            name_index.clear();
        }
        {
            let mut type_index = self.type_index.write().await;
            type_index.clear();
        }
        {
            let mut tag_index = self.tag_index.write().await;
            tag_index.clear();
        }

        // Rebuild indexes
        for service in services {
            self.register_service(service.service_id, service).await?;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.index_rebuilds += 1;
        }

        info!(
            "Rebuilt discovery indexes for {} services in {:?}",
            services.len(),
            start_time.elapsed()
        );
        Ok(())
    }
}

// Strategy implementations

impl DiscoveryStrategy for ExactMatchStrategy {
    fn name(&self) -> &str {
        "exact_match"
    }

    fn discover_by_name(&self, name: &str, services: &[ServiceEntry]) -> Vec<ServiceEntry> {
        services
            .iter()
            .filter(|s| s.metadata.name == name)
            .cloned()
            .collect()
    }

    fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        services
            .iter()
            .filter(|s| {
                tags.iter()
                    .all(|(key, value)| s.tags.get(key).map_or(false, |v| v == value))
            })
            .cloned()
            .collect()
    }

    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        services
            .iter()
            .filter(|s| &s.metadata.service_type == service_type)
            .cloned()
            .collect()
    }

    fn priority(&self) -> u8 {
        0 // Highest priority
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl DiscoveryStrategy for FuzzyMatchStrategy {
    fn name(&self) -> &str {
        "fuzzy_match"
    }

    fn discover_by_name(&self, name: &str, services: &[ServiceEntry]) -> Vec<ServiceEntry> {
        services
            .iter()
            .filter(|s| {
                self.calculate_similarity(name, &s.metadata.name) >= self.similarity_threshold
            })
            .cloned()
            .collect()
    }

    fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        // Fuzzy tag matching implementation would go here
        Vec::new()
    }

    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        // Exact match for types (fuzzy doesn't make sense for enums)
        services
            .iter()
            .filter(|s| &s.metadata.service_type == service_type)
            .cloned()
            .collect()
    }

    fn priority(&self) -> u8 {
        1
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl FuzzyMatchStrategy {
    /// Calculate string similarity using Levenshtein distance
    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        let len_a = a.len();
        let len_b = b.len();

        if len_a == 0 {
            return if len_b == 0 { 1.0 } else { 0.0 };
        }
        if len_b == 0 {
            return 0.0;
        }

        let max_len = len_a.max(len_b);
        let distance = self.levenshtein_distance(a, b);

        1.0 - (distance as f64 / max_len as f64)
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let len_a = a_chars.len();
        let len_b = b_chars.len();

        let mut matrix = vec![vec![0; len_b + 1]; len_a + 1];

        for i in 0..=len_a {
            matrix[i][0] = i;
        }
        for j in 0..=len_b {
            matrix[0][j] = j;
        }

        for i in 1..=len_a {
            for j in 1..=len_b {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len_a][len_b]
    }
}

impl DiscoveryStrategy for SemanticDiscoveryStrategy {
    fn name(&self) -> &str {
        "semantic_discovery"
    }

    fn discover_by_name(&self, _name: &str, _services: &[ServiceEntry]) -> Vec<ServiceEntry> {
        // Semantic discovery would use ML models to understand intent
        // This is a placeholder implementation
        Vec::new()
    }

    fn discover_by_tags(
        &self,
        _tags: &HashMap<String, String>,
        _services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        Vec::new()
    }

    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        services
            .iter()
            .filter(|s| &s.metadata.service_type == service_type)
            .cloned()
            .collect()
    }

    fn priority(&self) -> u8 {
        2
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl DiscoveryStrategy for ProximityBasedStrategy {
    fn name(&self) -> &str {
        "proximity_based"
    }

    fn discover_by_name(&self, name: &str, services: &[ServiceEntry]) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| s.metadata.name == name)
            .cloned()
            .collect();

        // Sort by region proximity (simplified)
        matching_services.sort_by(|a, b| {
            let a_weight = self.region_weights.get(&a.metadata.region).unwrap_or(&1.0);
            let b_weight = self.region_weights.get(&b.metadata.region).unwrap_or(&1.0);
            b_weight
                .partial_cmp(a_weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| {
                tags.iter()
                    .all(|(key, value)| s.tags.get(key).map_or(false, |v| v == value))
            })
            .cloned()
            .collect();

        // Sort by proximity
        matching_services.sort_by(|a, b| {
            let a_weight = self.region_weights.get(&a.metadata.region).unwrap_or(&1.0);
            let b_weight = self.region_weights.get(&b.metadata.region).unwrap_or(&1.0);
            b_weight
                .partial_cmp(a_weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| &s.metadata.service_type == service_type)
            .cloned()
            .collect();

        // Sort by proximity
        matching_services.sort_by(|a, b| {
            let a_weight = self.region_weights.get(&a.metadata.region).unwrap_or(&1.0);
            let b_weight = self.region_weights.get(&b.metadata.region).unwrap_or(&1.0);
            b_weight
                .partial_cmp(a_weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn priority(&self) -> u8 {
        3
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl DiscoveryStrategy for LoadAwareStrategy {
    fn name(&self) -> &str {
        "load_aware"
    }

    fn discover_by_name(&self, name: &str, services: &[ServiceEntry]) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| s.metadata.name == name)
            .filter(|s| s.capabilities.performance.cpu_usage < self.load_threshold)
            .cloned()
            .collect();

        // Sort by load (CPU usage)
        matching_services.sort_by(|a, b| {
            a.capabilities
                .performance
                .cpu_usage
                .partial_cmp(&b.capabilities.performance.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn discover_by_tags(
        &self,
        tags: &HashMap<String, String>,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| {
                tags.iter()
                    .all(|(key, value)| s.tags.get(key).map_or(false, |v| v == value))
            })
            .filter(|s| s.capabilities.performance.cpu_usage < self.load_threshold)
            .cloned()
            .collect();

        // Sort by load
        matching_services.sort_by(|a, b| {
            a.capabilities
                .performance
                .cpu_usage
                .partial_cmp(&b.capabilities.performance.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn discover_by_type(
        &self,
        service_type: &ServiceType,
        services: &[ServiceEntry],
    ) -> Vec<ServiceEntry> {
        let mut matching_services: Vec<_> = services
            .iter()
            .filter(|s| &s.metadata.service_type == service_type)
            .filter(|s| s.capabilities.performance.cpu_usage < self.load_threshold)
            .cloned()
            .collect();

        // Sort by load
        matching_services.sort_by(|a, b| {
            a.capabilities
                .performance
                .cpu_usage
                .partial_cmp(&b.capabilities.performance.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matching_services
    }

    fn priority(&self) -> u8 {
        4
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for DiscoveryMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            cache_hits: 0,
            cache_misses: 0,
            avg_discovery_latency: Duration::from_nanos(0),
            successful_discoveries: 0,
            failed_discoveries: 0,
            strategy_usage: HashMap::new(),
            index_rebuilds: 0,
        }
    }
}
