// Route Caching System
// Task 3.1.9.1: Route Caching

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use dashmap::DashMap;

/// Route cache for performance optimization
pub struct RouteCache {
    cache: Arc<DashMap<RouteCacheKey, CachedRoute>>,
    ttl_manager: Arc<TTLManager>,
    invalidation_manager: Arc<InvalidationManager>,
    metrics: Arc<CacheMetrics>,
    config: CacheConfig,
}

/// Cache key for routes
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct RouteCacheKey {
    pub source: NodeId,
    pub destination: NodeId,
    pub qos_class: QoSClass,
    pub security_level: SecurityLevel,
    pub topology_version: u64,
}

/// QoS class for caching
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum QoSClass {
    Critical,
    High,
    Normal,
    Low,
    Background,
}

/// Cached route with metadata
#[derive(Debug)]
pub struct CachedRoute {
    pub route: Route,
    pub created_at: Instant,
    pub ttl: Duration,
    pub hit_count: AtomicU64,
    pub last_accessed: AtomicU64,
    pub cache_score: f64,
    pub invalidation_triggers: Vec<InvalidationTrigger>,
}

impl Clone for CachedRoute {
    fn clone(&self) -> Self {
        Self {
            route: self.route.clone(),
            created_at: self.created_at,
            ttl: self.ttl,
            hit_count: AtomicU64::new(self.hit_count.load(std::sync::atomic::Ordering::Relaxed)),
            last_accessed: AtomicU64::new(self.last_accessed.load(std::sync::atomic::Ordering::Relaxed)),
            cache_score: self.cache_score,
            invalidation_triggers: self.invalidation_triggers.clone(),
        }
    }
}

/// TTL (Time To Live) manager
pub struct TTLManager {
    cleanup_interval: Duration,
    default_ttl: Duration,
    ttl_by_qos: HashMap<QoSClass, Duration>,
    last_cleanup: Arc<RwLock<Instant>>,
}

/// Cache invalidation manager
pub struct InvalidationManager {
    topology_listeners: Arc<RwLock<Vec<Arc<dyn TopologyChangeListener>>>>,
    invalidation_rules: Arc<RwLock<Vec<InvalidationRule>>>,
    pending_invalidations: Arc<DashMap<InvalidationTrigger, Instant>>,
}

/// Invalidation triggers
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum InvalidationTrigger {
    TopologyChange,
    NodeStatusChange(NodeId),
    LinkStatusChange(LinkId),
    QoSPolicyChange,
    SecurityPolicyChange,
    ManualInvalidation,
    TTLExpired,
}

/// Invalidation rules
#[derive(Debug, Clone)]
pub struct InvalidationRule {
    pub trigger: InvalidationTrigger,
    pub scope: InvalidationScope,
    pub delay: Option<Duration>,
    pub condition: Option<InvalidationCondition>,
}

/// Scope of invalidation
pub enum InvalidationScope {
    All,
    BySource(NodeId),
    ByDestination(NodeId),
    ByNode(NodeId),
    ByLink(LinkId),
    ByQoSClass(QoSClass),
    BySecurityLevel(SecurityLevel),
    Custom(Box<dyn Fn(&RouteCacheKey) -> bool + Send + Sync>),
}

impl std::fmt::Debug for InvalidationScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::BySource(id) => f.debug_tuple("BySource").field(id).finish(),
            Self::ByDestination(id) => f.debug_tuple("ByDestination").field(id).finish(),
            Self::ByNode(id) => f.debug_tuple("ByNode").field(id).finish(),
            Self::ByLink(id) => f.debug_tuple("ByLink").field(id).finish(),
            Self::ByQoSClass(qos) => f.debug_tuple("ByQoSClass").field(qos).finish(),
            Self::BySecurityLevel(level) => f.debug_tuple("BySecurityLevel").field(level).finish(),
            Self::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

impl Clone for InvalidationScope {
    fn clone(&self) -> Self {
        match self {
            Self::All => Self::All,
            Self::BySource(id) => Self::BySource(*id),
            Self::ByDestination(id) => Self::ByDestination(*id),
            Self::ByNode(id) => Self::ByNode(*id),
            Self::ByLink(id) => Self::ByLink(*id),
            Self::ByQoSClass(qos) => Self::ByQoSClass(qos.clone()),
            Self::BySecurityLevel(level) => Self::BySecurityLevel(*level),
            Self::Custom(_) => Self::Custom(Box::new(|_| false)),
        }
    }
}

/// Condition for invalidation
pub enum InvalidationCondition {
    Always,
    IfOlderThan(Duration),
    IfHitCountBelow(u64),
    IfCacheScoreBelow(f64),
    Custom(Box<dyn Fn(&CachedRoute) -> bool + Send + Sync>),
}

impl std::fmt::Debug for InvalidationCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Always => write!(f, "Always"),
            Self::IfOlderThan(duration) => f.debug_tuple("IfOlderThan").field(duration).finish(),
            Self::IfHitCountBelow(count) => f.debug_tuple("IfHitCountBelow").field(count).finish(),
            Self::IfCacheScoreBelow(score) => f.debug_tuple("IfCacheScoreBelow").field(score).finish(),
            Self::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

impl Clone for InvalidationCondition {
    fn clone(&self) -> Self {
        match self {
            Self::Always => Self::Always,
            Self::IfOlderThan(duration) => Self::IfOlderThan(*duration),
            Self::IfHitCountBelow(count) => Self::IfHitCountBelow(*count),
            Self::IfCacheScoreBelow(score) => Self::IfCacheScoreBelow(*score),
            Self::Custom(_) => Self::Custom(Box::new(|_| false)),
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub default_ttl: Duration,
    pub cleanup_interval: Duration,
    pub enable_precomputation: bool,
    pub enable_warming: bool,
    pub eviction_policy: EvictionPolicy,
    pub compression_enabled: bool,
}

/// Cache eviction policies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    LRU,  // Least Recently Used
    LFU,  // Least Frequently Used
    FIFO, // First In, First Out
    TTL,  // Time To Live based
    Score, // Cache score based
}

/// Cache metrics
pub struct CacheMetrics {
    pub total_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub evictions: AtomicU64,
    pub invalidations: AtomicU64,
    pub precomputations: AtomicU64,
    pub warming_operations: AtomicU64,
    pub average_lookup_time: AtomicU64,
    pub memory_usage: AtomicU64,
}

impl RouteCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            ttl_manager: Arc::new(TTLManager::new(
                config.cleanup_interval,
                config.default_ttl,
            )),
            invalidation_manager: Arc::new(InvalidationManager::new()),
            metrics: Arc::new(CacheMetrics::new()),
            config,
        }
    }

    /// Get a route from cache
    pub async fn get(&self, key: &RouteCacheKey) -> Option<Route> {
        let start_time = Instant::now();
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

        let result = if let Some(cached_route) = self.cache.get(key) {
            // Check if route is still valid
            if self.is_valid(&cached_route) {
                // Update access statistics
                cached_route.hit_count.fetch_add(1, Ordering::Relaxed);
                cached_route.last_accessed.store(
                    SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64,
                    Ordering::Relaxed,
                );
                
                self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                Some(cached_route.route.clone())
            } else {
                // Remove expired route
                self.cache.remove(key);
                self.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        } else {
            self.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
            None
        };

        // Update lookup time metrics
        let lookup_time = start_time.elapsed().as_nanos() as u64;
        self.update_average_lookup_time(lookup_time);

        result
    }

    /// Put a route in cache
    pub async fn put(&self, key: RouteCacheKey, route: Route) {
        // Check cache size and evict if necessary
        if self.cache.len() >= self.config.max_entries {
            self.evict_entries().await;
        }

        // Determine TTL based on QoS class
        let ttl = self.ttl_manager.get_ttl_for_qos(&self.qos_class_from_route(&route));

        // Create cached route
        let cached_route = CachedRoute {
            route,
            created_at: Instant::now(),
            ttl,
            hit_count: AtomicU64::new(0),
            last_accessed: AtomicU64::new(
                SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
            ),
            cache_score: self.calculate_cache_score(&key),
            invalidation_triggers: self.get_invalidation_triggers(&key),
        };

        self.cache.insert(key, cached_route);
        self.update_memory_usage();
    }

    /// Invalidate cache entries based on trigger
    pub async fn invalidate(&self, trigger: InvalidationTrigger) {
        let rules = self.invalidation_manager.get_applicable_rules(&trigger).await;
        let mut invalidated_count = 0;

        for rule in rules {
            let keys_to_remove: Vec<_> = self.cache
                .iter()
                .filter(|entry| self.should_invalidate_entry(entry.key(), entry.value(), &rule))
                .map(|entry| entry.key().clone())
                .collect();

            for key in keys_to_remove {
                self.cache.remove(&key);
                invalidated_count += 1;
            }
        }

        self.metrics.invalidations.fetch_add(invalidated_count, Ordering::Relaxed);
        self.update_memory_usage();
    }

    /// Precompute routes for common patterns
    pub async fn precompute_routes(
        &self,
        routing_engine: &AlgorithmManager,
        topology: &NetworkTopology,
        patterns: &[RoutePattern],
    ) -> Result<usize, RoutingError> {
        if !self.config.enable_precomputation {
            return Ok(0);
        }

        let mut precomputed_count = 0;

        for pattern in patterns {
            let contexts = self.generate_contexts_from_pattern(pattern, topology);
            
            for context in contexts {
                let key = self.create_cache_key(&context, topology.version);
                
                // Skip if already cached
                if self.cache.contains_key(&key) {
                    continue;
                }

                // Calculate route
                match routing_engine.calculate_route(&context, topology).await {
                    Ok(route) => {
                        self.put(key, route).await;
                        precomputed_count += 1;
                    }
                    Err(e) => {
                        eprintln!("Precomputation failed for pattern {:?}: {}", pattern, e);
                    }
                }
            }
        }

        self.metrics.precomputations.fetch_add(precomputed_count, Ordering::Relaxed);
        Ok(precomputed_count as usize)
    }

    /// Warm cache with frequently used routes
    pub async fn warm_cache(
        &self,
        routing_engine: &AlgorithmManager,
        topology: &NetworkTopology,
        warming_strategy: &dyn WarmingStrategy,
    ) -> Result<usize, RoutingError> {
        if !self.config.enable_warming {
            return Ok(0);
        }

        let routes_to_warm = warming_strategy.get_routes_to_warm(topology).await;
        let mut warmed_count = 0;

        for (source, destination, qos) in routes_to_warm {
            let context = RoutingContext::new(source, destination, qos);
            let key = self.create_cache_key(&context, topology.version);

            if !self.cache.contains_key(&key) {
                match routing_engine.calculate_route(&context, topology).await {
                    Ok(route) => {
                        self.put(key, route).await;
                        warmed_count += 1;
                    }
                    Err(e) => {
                        eprintln!("Cache warming failed for {}->{}: {}", source, destination, e);
                    }
                }
            }
        }

        self.metrics.warming_operations.fetch_add(warmed_count, Ordering::Relaxed);
        Ok(warmed_count as usize)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let total_requests = self.metrics.total_requests.load(Ordering::Relaxed);
        let cache_hits = self.metrics.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.metrics.cache_misses.load(Ordering::Relaxed);

        let hit_rate = if total_requests > 0 {
            cache_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        CacheStats {
            total_entries: self.cache.len(),
            total_requests,
            cache_hits,
            cache_misses,
            hit_rate,
            evictions: self.metrics.evictions.load(Ordering::Relaxed),
            invalidations: self.metrics.invalidations.load(Ordering::Relaxed),
            precomputations: self.metrics.precomputations.load(Ordering::Relaxed),
            warming_operations: self.metrics.warming_operations.load(Ordering::Relaxed),
            average_lookup_time: Duration::from_nanos(
                self.metrics.average_lookup_time.load(Ordering::Relaxed)
            ),
            memory_usage: self.metrics.memory_usage.load(Ordering::Relaxed),
        }
    }

    /// Start background cleanup task
    pub async fn start_cleanup_task(&self) {
        let cache = Arc::clone(&self.cache);
        let ttl_manager = Arc::clone(&self.ttl_manager);
        let metrics = Arc::clone(&self.metrics);
        let cleanup_interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(cleanup_interval).await;
                
                let expired_keys: Vec<_> = cache
                    .iter()
                    .filter(|entry| !Self::is_valid_static(entry.value()))
                    .map(|entry| entry.key().clone())
                    .collect();

                let mut removed_count = 0;
                for key in expired_keys {
                    cache.remove(&key);
                    removed_count += 1;
                }

                if removed_count > 0 {
                    metrics.evictions.fetch_add(removed_count, Ordering::Relaxed);
                }

                ttl_manager.mark_cleanup_complete().await;
            }
        });
    }

    // Private helper methods
    fn is_valid(&self, cached_route: &CachedRoute) -> bool {
        Self::is_valid_static(cached_route)
    }

    fn is_valid_static(cached_route: &CachedRoute) -> bool {
        Instant::now().duration_since(cached_route.created_at) < cached_route.ttl
    }

    fn qos_class_from_route(&self, route: &Route) -> QoSClass {
        // Extract QoS class from route metadata or use default
        QoSClass::Normal
    }

    fn calculate_cache_score(&self, key: &RouteCacheKey) -> f64 {
        // Calculate cache score based on various factors
        let mut score = 1.0;

        // Higher score for critical QoS
        match key.qos_class {
            QoSClass::Critical => score *= 2.0,
            QoSClass::High => score *= 1.5,
            QoSClass::Normal => score *= 1.0,
            QoSClass::Low => score *= 0.8,
            QoSClass::Background => score *= 0.5,
        }

        // Higher score for higher security levels
        match key.security_level {
            SecurityLevel::TopSecret => score *= 1.8,
            SecurityLevel::Secret => score *= 1.6,
            SecurityLevel::Confidential => score *= 1.4,
            SecurityLevel::Internal => score *= 1.0,
            SecurityLevel::Public => score *= 0.8,
        }

        score
    }

    fn get_invalidation_triggers(&self, key: &RouteCacheKey) -> Vec<InvalidationTrigger> {
        vec![
            InvalidationTrigger::TopologyChange,
            InvalidationTrigger::NodeStatusChange(key.source),
            InvalidationTrigger::NodeStatusChange(key.destination),
            InvalidationTrigger::QoSPolicyChange,
            InvalidationTrigger::SecurityPolicyChange,
        ]
    }

    async fn evict_entries(&self) {
        let eviction_count = self.cache.len() / 10; // Evict 10% of entries
        
        match self.config.eviction_policy {
            EvictionPolicy::LRU => self.evict_lru(eviction_count).await,
            EvictionPolicy::LFU => self.evict_lfu(eviction_count).await,
            EvictionPolicy::FIFO => self.evict_fifo(eviction_count).await,
            EvictionPolicy::TTL => self.evict_ttl(eviction_count).await,
            EvictionPolicy::Score => self.evict_by_score(eviction_count).await,
        }
    }

    async fn evict_lru(&self, count: usize) {
        let mut entries: Vec<_> = self.cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().last_accessed.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|(_, last_accessed)| *last_accessed);
        
        for (key, _) in entries.into_iter().take(count) {
            self.cache.remove(&key);
        }
        
        self.metrics.evictions.fetch_add(count as u64, Ordering::Relaxed);
    }

    async fn evict_lfu(&self, count: usize) {
        let mut entries: Vec<_> = self.cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().hit_count.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|(_, hit_count)| *hit_count);
        
        for (key, _) in entries.into_iter().take(count) {
            self.cache.remove(&key);
        }
        
        self.metrics.evictions.fetch_add(count as u64, Ordering::Relaxed);
    }

    async fn evict_fifo(&self, count: usize) {
        let mut entries: Vec<_> = self.cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().created_at))
            .collect();
        
        entries.sort_by_key(|(_, created_at)| *created_at);
        
        for (key, _) in entries.into_iter().take(count) {
            self.cache.remove(&key);
        }
        
        self.metrics.evictions.fetch_add(count as u64, Ordering::Relaxed);
    }

    async fn evict_ttl(&self, count: usize) {
        let now = Instant::now();
        let mut entries: Vec<_> = self.cache
            .iter()
            .map(|entry| {
                let remaining_ttl = entry.value().ttl
                    .saturating_sub(now.duration_since(entry.value().created_at));
                (entry.key().clone(), remaining_ttl)
            })
            .collect();
        
        entries.sort_by_key(|(_, remaining_ttl)| *remaining_ttl);
        
        for (key, _) in entries.into_iter().take(count) {
            self.cache.remove(&key);
        }
        
        self.metrics.evictions.fetch_add(count as u64, Ordering::Relaxed);
    }

    async fn evict_by_score(&self, count: usize) {
        let mut entries: Vec<_> = self.cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().cache_score))
            .collect();
        
        entries.sort_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap());
        
        for (key, _) in entries.into_iter().take(count) {
            self.cache.remove(&key);
        }
        
        self.metrics.evictions.fetch_add(count as u64, Ordering::Relaxed);
    }

    fn should_invalidate_entry(
        &self,
        key: &RouteCacheKey,
        cached_route: &CachedRoute,
        rule: &InvalidationRule,
    ) -> bool {
        // Check scope
        let scope_matches = match &rule.scope {
            InvalidationScope::All => true,
            InvalidationScope::BySource(source) => key.source == *source,
            InvalidationScope::ByDestination(dest) => key.destination == *dest,
            InvalidationScope::ByNode(node) => key.source == *node || key.destination == *node,
            InvalidationScope::ByQoSClass(qos) => key.qos_class == *qos,
            InvalidationScope::BySecurityLevel(level) => key.security_level == *level,
            InvalidationScope::ByLink(_) => false, // Would need route analysis
            InvalidationScope::Custom(func) => func(key),
        };

        if !scope_matches {
            return false;
        }

        // Check condition
        match &rule.condition {
            Some(InvalidationCondition::Always) | None => true,
            Some(InvalidationCondition::IfOlderThan(duration)) => {
                Instant::now().duration_since(cached_route.created_at) > *duration
            }
            Some(InvalidationCondition::IfHitCountBelow(threshold)) => {
                cached_route.hit_count.load(Ordering::Relaxed) < *threshold
            }
            Some(InvalidationCondition::IfCacheScoreBelow(threshold)) => {
                cached_route.cache_score < *threshold
            }
            Some(InvalidationCondition::Custom(func)) => func(cached_route),
        }
    }

    fn create_cache_key(&self, context: &RoutingContext, topology_version: u64) -> RouteCacheKey {
        RouteCacheKey {
            source: context.source,
            destination: context.destination,
            qos_class: self.qos_class_from_priority(context.qos_requirements.priority),
            security_level: context.security_context.security_level,
            topology_version,
        }
    }

    fn qos_class_from_priority(&self, priority: MessagePriority) -> QoSClass {
        match priority {
            MessagePriority::Critical => QoSClass::Critical,
            MessagePriority::High => QoSClass::High,
            MessagePriority::Normal => QoSClass::Normal,
            MessagePriority::Low => QoSClass::Low,
            MessagePriority::Background => QoSClass::Background,
        }
    }

    fn generate_contexts_from_pattern(
        &self,
        pattern: &RoutePattern,
        topology: &NetworkTopology,
    ) -> Vec<RoutingContext> {
        // Implementation would generate routing contexts based on patterns
        Vec::new()
    }

    fn update_average_lookup_time(&self, lookup_time: u64) {
        let current_avg = self.metrics.average_lookup_time.load(Ordering::Relaxed);
        let total_requests = self.metrics.total_requests.load(Ordering::Relaxed);
        
        if total_requests > 0 {
            let new_avg = (current_avg * (total_requests - 1) + lookup_time) / total_requests;
            self.metrics.average_lookup_time.store(new_avg, Ordering::Relaxed);
        }
    }

    fn update_memory_usage(&self) {
        // Estimate memory usage (simplified)
        let entry_size = std::mem::size_of::<RouteCacheKey>() + std::mem::size_of::<CachedRoute>();
        let total_memory = self.cache.len() * entry_size;
        self.metrics.memory_usage.store(total_memory as u64, Ordering::Relaxed);
    }
}

/// Route pattern for precomputation
#[derive(Debug, Clone)]
pub struct RoutePattern {
    pub source_pattern: NodePattern,
    pub destination_pattern: NodePattern,
    pub qos_requirements: QoSRequirements,
    pub frequency_weight: f64,
}

/// Node pattern for matching
#[derive(Debug, Clone)]
pub enum NodePattern {
    Specific(NodeId),
    InRegion(RegionId),
    WithCapability(String),
    All,
}

/// Cache warming strategy
#[async_trait::async_trait]
pub trait WarmingStrategy: Send + Sync {
    async fn get_routes_to_warm(
        &self,
        topology: &NetworkTopology,
    ) -> Vec<(NodeId, NodeId, QoSRequirements)>;
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_rate: f64,
    pub evictions: u64,
    pub invalidations: u64,
    pub precomputations: u64,
    pub warming_operations: u64,
    pub average_lookup_time: Duration,
    pub memory_usage: u64,
}

impl TTLManager {
    pub fn new(cleanup_interval: Duration, default_ttl: Duration) -> Self {
        let mut ttl_by_qos = HashMap::new();
        ttl_by_qos.insert(QoSClass::Critical, Duration::from_secs(60));
        ttl_by_qos.insert(QoSClass::High, Duration::from_secs(120));
        ttl_by_qos.insert(QoSClass::Normal, Duration::from_secs(300));
        ttl_by_qos.insert(QoSClass::Low, Duration::from_secs(600));
        ttl_by_qos.insert(QoSClass::Background, Duration::from_secs(1200));

        Self {
            cleanup_interval,
            default_ttl,
            ttl_by_qos,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub fn get_ttl_for_qos(&self, qos_class: &QoSClass) -> Duration {
        self.ttl_by_qos.get(qos_class).copied().unwrap_or(self.default_ttl)
    }

    pub async fn mark_cleanup_complete(&self) {
        *self.last_cleanup.write().await = Instant::now();
    }
}

impl InvalidationManager {
    pub fn new() -> Self {
        Self {
            topology_listeners: Arc::new(RwLock::new(Vec::new())),
            invalidation_rules: Arc::new(RwLock::new(Vec::new())),
            pending_invalidations: Arc::new(DashMap::new()),
        }
    }

    pub async fn get_applicable_rules(&self, trigger: &InvalidationTrigger) -> Vec<InvalidationRule> {
        let rules = self.invalidation_rules.read().await;
        rules.iter()
            .filter(|rule| rule.trigger == *trigger)
            .cloned()
            .collect()
    }

    pub async fn add_rule(&self, rule: InvalidationRule) {
        let mut rules = self.invalidation_rules.write().await;
        rules.push(rule);
    }
}

impl CacheMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
            precomputations: AtomicU64::new(0),
            warming_operations: AtomicU64::new(0),
            average_lookup_time: AtomicU64::new(0),
            memory_usage: AtomicU64::new(0),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            default_ttl: Duration::from_secs(300),
            cleanup_interval: Duration::from_secs(60),
            enable_precomputation: true,
            enable_warming: true,
            eviction_policy: EvictionPolicy::LRU,
            compression_enabled: false,
        }
    }
}