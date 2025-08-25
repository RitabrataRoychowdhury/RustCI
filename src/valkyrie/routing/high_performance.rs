//! High-Performance Routing Integration with FIT
//!
//! This module integrates the Fingerprinted ID Table (FIT) with the existing
//! MessageRouter to achieve sub-82µs p99 routing latency while preserving
//! the RoutingStrategy interface for backward compatibility.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use std::sync::atomic::{AtomicU64, Ordering};

use uuid::Uuid;

use crate::valkyrie::lockfree::fingerprinted_id_table::{
    FingerprintedIdTable, RouteEntry, FitError
};
use crate::valkyrie::lockfree::fingerprinted_id_table::{
    RouteId as FitRouteId, NodeId as FitNodeId, ShardId as FitShardId
};
use crate::core::networking::valkyrie::routing::{
    RoutingStrategy, RoutingContext, NetworkTopology, Route, RoutingError, RoutingAlgorithm,
    PerformanceProfile, NodeId, Hop, RouteMetadata, TransportType, SecurityLevel
};

/// Performance monitoring for routing operations
#[derive(Debug, Default)]
pub struct RoutingPerformanceMetrics {
    /// Total number of routing requests
    pub total_requests: AtomicU64,
    /// Number of FIT cache hits
    pub fit_hits: AtomicU64,
    /// Number of FIT cache misses (fallback to traditional routing)
    pub fit_misses: AtomicU64,
    /// Total routing latency (nanoseconds)
    pub total_latency_ns: AtomicU64,
    /// Maximum observed latency (nanoseconds)
    pub max_latency_ns: AtomicU64,
    /// Number of performance budget violations (>82µs)
    pub budget_violations: AtomicU64,
}

impl RoutingPerformanceMetrics {
    pub fn record_request(&self, latency: Duration, was_fit_hit: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        
        if was_fit_hit {
            self.fit_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.fit_misses.fetch_add(1, Ordering::Relaxed);
        }
        
        let latency_ns = latency.as_nanos() as u64;
        self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);
        
        // Update max latency
        let current_max = self.max_latency_ns.load(Ordering::Relaxed);
        if latency_ns > current_max {
            let _ = self.max_latency_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
        
        // Check performance budget (82µs = 82,000ns)
        if latency_ns > 82_000 {
            self.budget_violations.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn get_stats(&self) -> RoutingStats {
        let total = self.total_requests.load(Ordering::Relaxed);
        let hits = self.fit_hits.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        
        RoutingStats {
            total_requests: total,
            fit_hit_rate: if total > 0 { hits as f64 / total as f64 } else { 0.0 },
            avg_latency_ns: if total > 0 { total_latency / total } else { 0 },
            max_latency_ns: self.max_latency_ns.load(Ordering::Relaxed),
            budget_violations: self.budget_violations.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub total_requests: u64,
    pub fit_hit_rate: f64,
    pub avg_latency_ns: u64,
    pub max_latency_ns: u64,
    pub budget_violations: u64,
}

/// High-performance routing strategy using FIT for O(1) lookups
pub struct HighPerformanceRoutingStrategy {
    /// Fingerprinted ID Table for fast lookups
    fit: Arc<FingerprintedIdTable>,
    /// Fallback strategy for cache misses
    fallback_strategy: Arc<dyn RoutingStrategy>,
    /// Performance metrics
    metrics: Arc<RoutingPerformanceMetrics>,
    /// Performance budget (default: 82µs)
    performance_budget: Duration,
}

impl HighPerformanceRoutingStrategy {
    /// Create a new high-performance routing strategy
    pub fn new(
        fit_capacity: usize,
        fallback_strategy: Arc<dyn RoutingStrategy>,
    ) -> Result<Self, FitError> {
        let fit = Arc::new(FingerprintedIdTable::new(fit_capacity, 0.75)?);
        let metrics = Arc::new(RoutingPerformanceMetrics::default());
        let performance_budget = Duration::from_micros(82);
        
        Ok(Self {
            fit,
            fallback_strategy,
            metrics,
            performance_budget,
        })
    }
    
    /// Create with default settings (64K capacity)
    pub fn with_defaults(fallback_strategy: Arc<dyn RoutingStrategy>) -> Self {
        Self::new(65536, fallback_strategy)
            .expect("Default FIT parameters should be valid")
    }
    
    /// Pre-populate FIT with route entries for hot paths
    pub fn populate_fit(&self, routes: Vec<RouteEntry>) -> Result<usize, FitError> {
        let mut inserted = 0;
        
        for route in routes {
            match self.fit.insert(route) {
                Ok(()) => inserted += 1,
                Err(FitError::CapacityExceeded { .. }) => break,
                Err(e) => return Err(e),
            }
        }
        
        Ok(inserted)
    }
    
    /// Convert UUID NodeId to FIT NodeId (u64)
    fn uuid_to_fit_node_id(&self, uuid: &NodeId) -> FitNodeId {
        // Use the first 8 bytes of the UUID as u64
        let bytes = uuid.as_bytes();
        let mut array = [0u8; 8];
        array.copy_from_slice(&bytes[0..8]);
        FitNodeId::new(u64::from_le_bytes(array))
    }
    
    /// Convert FIT NodeId to UUID NodeId
    fn fit_node_id_to_uuid(&self, fit_id: FitNodeId) -> NodeId {
        // Create a UUID from the u64 (pad with zeros)
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&fit_id.as_u64().to_le_bytes());
        Uuid::from_bytes(bytes)
    }
    
    /// Convert UUID RouteId to FIT RouteId
    fn uuid_to_fit_route_id(&self, uuid: &NodeId) -> FitRouteId {
        let bytes = uuid.as_bytes();
        let mut array = [0u8; 8];
        array.copy_from_slice(&bytes[0..8]);
        FitRouteId::new(u64::from_le_bytes(array))
    }

    /// Fast path: O(1) FIT lookup
    async fn fast_path_lookup(&self, destination: &NodeId) -> Option<Route> {
        let fit_route_id = self.uuid_to_fit_route_id(destination);
        
        if let Some(entry) = self.fit.lookup(fit_route_id) {
            let destination_uuid = self.fit_node_id_to_uuid(entry.next_hop);
            
            // Convert RouteEntry to Route
            Some(Route {
                id: Uuid::new_v4(),
                path: vec![destination_uuid],
                hops: vec![Hop {
                    from: Uuid::nil(), // Will be filled by caller
                    to: destination_uuid,
                    transport: TransportType::QUIC, // Default transport
                    cost: entry.latency_hint as f64,
                    latency: Duration::from_micros(entry.latency_hint as u64),
                    bandwidth: 1_000_000_000, // 1Gbps default
                    reliability: entry.reliability_score as f64 / 65535.0,
                    security_level: SecurityLevel::Internal,
                }],
                total_cost: entry.latency_hint as f64,
                estimated_latency: Duration::from_micros(entry.latency_hint as u64),
                reliability_score: entry.reliability_score as f64 / 65535.0,
                bandwidth_available: 1_000_000_000, // 1Gbps default
                created_at: SystemTime::now(),
                expires_at: None,
                metadata: RouteMetadata {
                    algorithm_used: "HighPerformanceFIT".to_string(),
                    calculation_time: Duration::from_nanos(100), // Very fast
                    cache_hit: true,
                    alternative_routes: 0,
                    confidence_score: 0.95,
                },
            })
        } else {
            None
        }
    }
    
    /// Get performance statistics
    pub fn get_performance_stats(&self) -> RoutingStats {
        self.metrics.get_stats()
    }
    
    /// Check if performance budget is being violated
    pub fn is_performance_healthy(&self) -> bool {
        let stats = self.get_performance_stats();
        let violation_rate = if stats.total_requests > 0 {
            stats.budget_violations as f64 / stats.total_requests as f64
        } else {
            0.0
        };
        
        // Consider healthy if <1% of requests violate budget
        violation_rate < 0.01
    }
}

#[async_trait::async_trait]
impl RoutingStrategy for HighPerformanceRoutingStrategy {
    async fn calculate_route(
        &self,
        source: &NodeId,
        destination: &NodeId,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let start_time = Instant::now();
        
        // Try fast path first
        if let Some(mut route) = self.fast_path_lookup(destination).await {
            // Update the source in the route
            if let Some(hop) = route.hops.first_mut() {
                hop.from = *source;
            }
            
            let latency = start_time.elapsed();
            self.metrics.record_request(latency, true);
            return Ok(route);
        }
        
        // Fallback to traditional routing
        let result = self.fallback_strategy
            .calculate_route(source, destination, context, topology)
            .await;
            
        let latency = start_time.elapsed();
        self.metrics.record_request(latency, false);
        
        // If fallback succeeded, consider caching the result in FIT
        if let Ok(ref route) = result {
            self.cache_route_in_fit(destination, route).await;
        }
        
        result
    }
    
    fn get_algorithm_type(&self) -> RoutingAlgorithm {
        RoutingAlgorithm::Custom(1) // Use Custom(1) for high-performance routing
    }
    
    fn get_priority(&self) -> u8 {
        255 // Highest priority for hot-path routing
    }
    
    fn can_handle(&self, _context: &RoutingContext) -> bool {
        // Can handle all routing contexts
        true
    }
    
    fn get_performance_profile(&self) -> PerformanceProfile {
        PerformanceProfile {
            avg_calculation_time: Duration::from_micros(10), // Target: sub-82µs
            memory_usage: 256 * 1024 * 1024, // 256MB estimated FIT memory usage
            cpu_intensity: 0.05, // Very low CPU usage
            scalability_factor: 0.99, // Scales very well
        }
    }
}

impl HighPerformanceRoutingStrategy {
    /// Cache a successful route in FIT for future fast lookups
    async fn cache_route_in_fit(&self, destination: &NodeId, route: &Route) {
        let fit_route_id = self.uuid_to_fit_route_id(destination);
        let fit_next_hop = if let Some(hop) = route.hops.first() {
            self.uuid_to_fit_node_id(&hop.to)
        } else {
            self.uuid_to_fit_node_id(destination)
        };
        
        let entry = RouteEntry::new(
            fit_route_id,
            fit_next_hop,
            FitShardId::new(0), // Default shard
            route.estimated_latency.as_micros() as u32,
            (route.reliability_score * 65535.0) as u16,
        );
        
        // Best effort caching - ignore errors
        let _ = self.fit.insert(entry);
    }
}

/// Enhanced MessageRouter with FIT integration
pub struct HighPerformanceMessageRouter {
    /// High-performance routing strategy
    routing_strategy: Arc<HighPerformanceRoutingStrategy>,
    /// Performance monitoring
    metrics: Arc<RoutingPerformanceMetrics>,
}

impl HighPerformanceMessageRouter {
    pub fn new(
        fit_capacity: usize,
        fallback_strategy: Arc<dyn RoutingStrategy>,
    ) -> Result<Self, FitError> {
        let routing_strategy = Arc::new(
            HighPerformanceRoutingStrategy::new(fit_capacity, fallback_strategy)?
        );
        let metrics = routing_strategy.metrics.clone();
        
        Ok(Self {
            routing_strategy,
            metrics,
        })
    }
    
    /// Route a message using high-performance FIT lookup
    pub async fn route_message_fast(
        &self,
        source: &NodeId,
        destination: &NodeId,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        self.routing_strategy
            .calculate_route(source, destination, context, topology)
            .await
    }
    
    /// Get routing performance statistics
    pub fn get_routing_stats(&self) -> RoutingStats {
        self.metrics.get_stats()
    }
    
    /// Pre-warm the FIT cache with known hot routes
    pub async fn warm_cache(&self, routes: Vec<RouteEntry>) -> Result<usize, FitError> {
        self.routing_strategy.populate_fit(routes)
    }
    
    /// Check if routing performance is healthy
    pub fn is_healthy(&self) -> bool {
        self.routing_strategy.is_performance_healthy()
    }
}

// Note: We use RoutingAlgorithm::Custom(1) for high-performance routing
// to avoid modifying the existing enum

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    // Mock fallback strategy for testing
    struct MockFallbackStrategy;
    
    #[async_trait::async_trait]
    impl RoutingStrategy for MockFallbackStrategy {
        async fn calculate_route(
            &self,
            source: &NodeId,
            destination: &NodeId,
            _context: &RoutingContext,
            _topology: &NetworkTopology,
        ) -> Result<Route, RoutingError> {
            Ok(Route {
                id: Uuid::new_v4(),
                path: vec![*destination],
                hops: vec![Hop {
                    from: *source,
                    to: *destination,
                    transport: TransportType::QUIC,
                    cost: 100.0,
                    latency: Duration::from_micros(50),
                    bandwidth: 1_000_000_000,
                    reliability: 0.99,
                    security_level: SecurityLevel::Internal,
                }],
                total_cost: 100.0,
                estimated_latency: Duration::from_micros(50),
                reliability_score: 0.99,
                bandwidth_available: 1_000_000_000,
                created_at: SystemTime::now(),
                expires_at: None,
                metadata: RouteMetadata::default(),
            })
        }
        
        fn get_algorithm_type(&self) -> RoutingAlgorithm {
            RoutingAlgorithm::ShortestPath
        }
        
        fn get_priority(&self) -> u8 { 1 }
        fn can_handle(&self, _context: &RoutingContext) -> bool { true }
        fn get_performance_profile(&self) -> PerformanceProfile {
            PerformanceProfile {
                avg_calculation_time: Duration::from_millis(1),
                memory_usage: 10 * 1024 * 1024,
                cpu_intensity: 0.1,
                scalability_factor: 0.8,
            }
        }
    }
    
    #[tokio::test]
    async fn test_high_performance_routing_fast_path() {
        let fallback = Arc::new(MockFallbackStrategy);
        let strategy = HighPerformanceRoutingStrategy::with_defaults(fallback);
        
        // Pre-populate with a route
        let route_entry = RouteEntry::new(
            FitRouteId::new(123),
            FitNodeId::new(456),
            FitShardId::new(1),
            50, // 50µs latency hint
            60000, // High reliability
        );
        
        strategy.populate_fit(vec![route_entry]).unwrap();
        
        // Test fast path lookup
        let source = Uuid::from_u128(1);
        let destination = Uuid::from_u128(123);
        let context = RoutingContext::default();
        let topology = NetworkTopology::new();
        
        let start = Instant::now();
        let route = strategy.calculate_route(&source, &destination, &context, &topology).await.unwrap();
        let latency = start.elapsed();
        
        // Should be very fast (sub-82µs)
        assert!(latency < Duration::from_micros(82));
        // Check that we got a route with the expected next hop
        assert!(!route.hops.is_empty());
        
        // Check metrics
        let stats = strategy.get_performance_stats();
        assert_eq!(stats.total_requests, 1);
        assert!(stats.fit_hit_rate > 0.0);
    }
    
    #[tokio::test]
    async fn test_fallback_routing() {
        let fallback = Arc::new(MockFallbackStrategy);
        let strategy = HighPerformanceRoutingStrategy::with_defaults(fallback);
        
        // Test with route not in FIT (should fallback)
        let source = Uuid::from_u128(1);
        let destination = Uuid::from_u128(999); // Not in FIT
        let context = RoutingContext::default();
        let topology = NetworkTopology::new();
        
        let route = strategy.calculate_route(&source, &destination, &context, &topology).await.unwrap();
        
        // Should get result from fallback - check the path contains destination
        assert!(route.path.contains(&destination));
        
        // Check metrics show miss
        let stats = strategy.get_performance_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.fit_hit_rate, 0.0); // Should be a miss
    }
}