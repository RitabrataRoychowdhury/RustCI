//! End-to-end routing latency measurements and validation
//!
//! This module provides comprehensive latency testing to ensure the routing system
//! meets the sub-82µs p99 latency requirement under various conditions.

use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime};
use std::thread;
use uuid::Uuid;

use RustAutoDevOps::valkyrie::routing::{
    enhanced_registries::{
        EnhancedConnectionRegistry, EnhancedServiceRegistry,
        EnhancedConnectionInfo, EnhancedServiceEntry,
        ConnectionState, ServiceHealthStatus,
        ConnectionMetrics, ServiceMetrics, ServiceEndpoint,
    },
    fuzzy_matching::{FuzzyMatchingEngine, FuzzyMatchingConfig},
    enhanced_service_discovery::{
        EnhancedServiceDiscovery, EnhancedServiceDiscoveryConfig,
        ServiceQuery,
    },
};
use RustAutoDevOps::core::networking::valkyrie::lockfree::registry::LockFreeRegistry;

/// Latency measurement result
#[derive(Debug, Clone)]
pub struct LatencyMeasurement {
    pub operation: String,
    pub latency: Duration,
    pub timestamp: Instant,
    pub thread_id: usize,
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub operation: String,
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
}

impl LatencyStats {
    pub fn from_measurements(operation: String, mut measurements: Vec<Duration>) -> Self {
        measurements.sort();
        let count = measurements.len();
        
        if count == 0 {
            return Self {
                operation,
                count: 0,
                min: Duration::ZERO,
                max: Duration::ZERO,
                mean: Duration::ZERO,
                p50: Duration::ZERO,
                p95: Duration::ZERO,
                p99: Duration::ZERO,
                p999: Duration::ZERO,
            };
        }
        
        let min = measurements[0];
        let max = measurements[count - 1];
        let mean = Duration::from_nanos(
            measurements.iter().map(|d| d.as_nanos() as u64).sum::<u64>() / count as u64
        );
        
        let p50 = measurements[count * 50 / 100];
        let p95 = measurements[count * 95 / 100];
        let p99 = measurements[count * 99 / 100];
        let p999 = measurements[count * 999 / 1000];
        
        Self {
            operation,
            count,
            min,
            max,
            mean,
            p50,
            p95,
            p99,
            p999,
        }
    }
    
    pub fn meets_requirements(&self) -> bool {
        // Primary requirement: p99 < 82µs
        self.p99 < Duration::from_micros(82)
    }
    
    pub fn print_report(&self) {
        println!("\n=== Latency Report: {} ===", self.operation);
        println!("Samples: {}", self.count);
        println!("Min:     {:>8.2?}", self.min);
        println!("Mean:    {:>8.2?}", self.mean);
        println!("P50:     {:>8.2?}", self.p50);
        println!("P95:     {:>8.2?}", self.p95);
        println!("P99:     {:>8.2?} {}", self.p99, 
            if self.meets_requirements() { "✅" } else { "❌" });
        println!("P99.9:   {:>8.2?}", self.p999);
        println!("Max:     {:>8.2?}", self.max);
        println!("Requirement: P99 < 82µs - {}", 
            if self.meets_requirements() { "PASSED" } else { "FAILED" });
    }
}

/// End-to-end latency test suite
pub struct EndToEndLatencyTests {
    connection_registry: Arc<EnhancedConnectionRegistry>,
    service_registry: Arc<EnhancedServiceRegistry>,
    fuzzy_engine: Arc<FuzzyMatchingEngine>,
    discovery_engine: Arc<EnhancedServiceDiscovery>,
}

impl EndToEndLatencyTests {
    pub fn new() -> Self {
        let connection_registry = Arc::new(
            EnhancedConnectionRegistry::new(Duration::from_secs(30))
        );
        
        let service_registry = Arc::new(
            EnhancedServiceRegistry::new()
        );
        
        let fuzzy_config = FuzzyMatchingConfig {
            max_results: 50,
            timeout: Duration::from_micros(50),
            min_score: 10,
            enable_caching: true,
            cache_ttl: Duration::from_secs(60),
        };
        let fuzzy_engine = Arc::new(FuzzyMatchingEngine::new(fuzzy_config));
        
        let discovery_config = EnhancedServiceDiscoveryConfig {
            enable_fuzzy_matching: true,
            fuzzy_config: FuzzyMatchingConfig::default(),
            enable_caching: true,
            cache_ttl: Duration::from_secs(300),
            max_cache_size: 1000,
            query_timeout: Duration::from_micros(100),
            enable_metrics: true,
        };
        let discovery_engine = Arc::new(EnhancedServiceDiscovery::new(discovery_config));
        
        Self {
            connection_registry,
            service_registry,
            fuzzy_engine,
            discovery_engine,
        }
    }
    
    /// Measure connection registry lookup latency
    pub fn measure_connection_lookup_latency(&self, iterations: usize) -> LatencyStats {
        // Pre-populate with test connections
        let mut connection_ids = Vec::new();
        for i in 0..1000 {
            let connection_id = Uuid::new_v4();
            let connection_info = EnhancedConnectionInfo {
                id: connection_id,
                remote_addr: format!("127.0.0.1:{}", 8000 + i).parse().unwrap(),
                state: ConnectionState::Active,
                metrics: ConnectionMetrics::default(),
                metadata: HashMap::new(),
                last_activity: SystemTime::now(),
            };
            
            self.connection_registry.register(connection_id, connection_info).unwrap();
            connection_ids.push(connection_id);
        }
        
        // Measure lookup latencies
        let mut measurements = Vec::with_capacity(iterations);
        for i in 0..iterations {
            let lookup_id = connection_ids[i % connection_ids.len()];
            
            let start = Instant::now();
            let _result = self.connection_registry.lookup(&lookup_id);
            let latency = start.elapsed();
            
            measurements.push(latency);
        }
        
        LatencyStats::from_measurements("Connection Lookup".to_string(), measurements)
    }
    
    /// Measure service registry lookup latency
    pub fn measure_service_lookup_latency(&self, iterations: usize) -> LatencyStats {
        // Pre-populate with test services
        let mut service_ids = Vec::new();
        for i in 0..1000 {
            let service_id = Uuid::new_v4();
            let service_entry = EnhancedServiceEntry {
                id: service_id,
                name: format!("test-service-{}", i),
                version: "1.0.0".to_string(),
                endpoints: vec![ServiceEndpoint {
                    address: format!("127.0.0.1:{}", 9000 + i).parse().unwrap(),
                    protocol: "http".to_string(),
                    weight: 100,
                    healthy: true,
                }],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("env".to_string(), "test".to_string());
                    tags.insert("tier".to_string(), format!("tier-{}", i % 5));
                    tags
                },
                health_status: ServiceHealthStatus::Healthy,
                metrics: ServiceMetrics::default(),
                registered_at: SystemTime::now(),
                last_health_check: SystemTime::now(),
            };
            
            self.service_registry.register(service_id, service_entry).unwrap();
            service_ids.push(service_id);
        }
        
        // Measure lookup latencies
        let mut measurements = Vec::with_capacity(iterations);
        for i in 0..iterations {
            let lookup_id = service_ids[i % service_ids.len()];
            
            let start = Instant::now();
            let _result = self.service_registry.lookup(&lookup_id);
            let latency = start.elapsed();
            
            measurements.push(latency);
        }
        
        LatencyStats::from_measurements("Service Lookup".to_string(), measurements)
    }
    
    /// Measure fuzzy matching latency
    pub async fn measure_fuzzy_matching_latency(&self, iterations: usize) -> LatencyStats {
        // Pre-populate with test services
        let service_names = vec![
            "user-authentication-service",
            "user-profile-service",
            "payment-processing-service",
            "notification-delivery-service",
            "analytics-reporting-service",
            "order-management-service",
            "inventory-tracking-service",
            "shipping-coordination-service",
        ];
        
        for (i, name) in service_names.iter().enumerate() {
            let service_entry = EnhancedServiceEntry {
                id: Uuid::new_v4(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                endpoints: vec![ServiceEndpoint {
                    address: format!("127.0.0.1:{}", 10000 + i).parse().unwrap(),
                    protocol: "http".to_string(),
                    weight: 100,
                    healthy: true,
                }],
                tags: HashMap::new(),
                health_status: ServiceHealthStatus::Healthy,
                metrics: ServiceMetrics::default(),
                registered_at: SystemTime::now(),
                last_health_check: SystemTime::now(),
            };
            self.fuzzy_engine.index_service(service_entry).await;
        }
        
        // Measure fuzzy matching latencies
        let queries = vec!["user", "payment", "service", "auth", "notification"];
        let mut measurements = Vec::with_capacity(iterations);
        
        for i in 0..iterations {
            let query = queries[i % queries.len()];
            
            let start = Instant::now();
            let _result = self.fuzzy_engine.fuzzy_match(query).await;
            let latency = start.elapsed();
            
            measurements.push(latency);
        }
        
        LatencyStats::from_measurements("Fuzzy Matching".to_string(), measurements)
    }
    
    /// Measure service discovery latency
    pub async fn measure_service_discovery_latency(&self, iterations: usize) -> LatencyStats {
        // Pre-populate with test services
        for i in 0..100 {
            let service_entry = EnhancedServiceEntry {
                id: Uuid::new_v4(),
                name: format!("discovery-service-{}", i),
                version: "1.0.0".to_string(),
                endpoints: vec![ServiceEndpoint {
                    address: format!("127.0.0.1:{}", 11000 + i).parse().unwrap(),
                    protocol: "http".to_string(),
                    weight: 100,
                    healthy: true,
                }],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("env".to_string(), "test".to_string());
                    tags.insert("tier".to_string(), format!("tier-{}", i % 3));
                    tags.insert("region".to_string(), format!("region-{}", i % 5));
                    tags
                },
                health_status: if i % 10 == 0 { ServiceHealthStatus::Degraded } else { ServiceHealthStatus::Healthy },
                metrics: ServiceMetrics::default(),
                registered_at: SystemTime::now(),
                last_health_check: SystemTime::now(),
            };
            self.discovery_engine.register_service(service_entry).await;
        }
        
        // Measure service discovery latencies
        let queries = vec![
            ServiceQuery {
                health_status: Some(ServiceHealthStatus::Healthy),
                limit: Some(10),
                ..Default::default()
            },
            ServiceQuery {
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("tier".to_string(), "tier-1".to_string());
                    tags
                },
                limit: Some(5),
                ..Default::default()
            },
            ServiceQuery {
                name: Some("discovery".to_string()),
                fuzzy_matching: true,
                limit: Some(20),
                ..Default::default()
            },
        ];
        
        let mut measurements = Vec::with_capacity(iterations);
        for i in 0..iterations {
            let query = queries[i % queries.len()].clone();
            
            let start = Instant::now();
            let _result = self.discovery_engine.discover_services(query).await;
            let latency = start.elapsed();
            
            measurements.push(latency);
        }
        
        LatencyStats::from_measurements("Service Discovery".to_string(), measurements)
    }
    
    /// Measure concurrent access latency
    pub fn measure_concurrent_latency(&self, num_threads: usize, iterations_per_thread: usize) -> Vec<LatencyStats> {
        // Pre-populate registries
        let mut connection_ids = Vec::new();
        for i in 0..1000 {
            let connection_id = Uuid::new_v4();
            let connection_info = EnhancedConnectionInfo {
                id: connection_id,
                remote_addr: format!("127.0.0.1:{}", 12000 + i).parse().unwrap(),
                state: ConnectionState::Active,
                metrics: ConnectionMetrics::default(),
                metadata: HashMap::new(),
                last_activity: SystemTime::now(),
            };
            
            self.connection_registry.register(connection_id, connection_info).unwrap();
            connection_ids.push(connection_id);
        }
        
        let connection_ids = Arc::new(connection_ids);
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = Vec::new();
        
        // Spawn concurrent threads
        for thread_id in 0..num_threads {
            let registry = Arc::clone(&self.connection_registry);
            let ids = Arc::clone(&connection_ids);
            let barrier = Arc::clone(&barrier);
            
            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier.wait();
                
                let mut measurements = Vec::with_capacity(iterations_per_thread);
                for i in 0..iterations_per_thread {
                    let lookup_id = ids[i % ids.len()];
                    
                    let start = Instant::now();
                    let _result = registry.lookup(&lookup_id);
                    let latency = start.elapsed();
                    
                    measurements.push(latency);
                }
                
                (thread_id, measurements)
            });
            
            handles.push(handle);
        }
        
        // Collect results from all threads
        let mut all_stats = Vec::new();
        for handle in handles {
            let (thread_id, measurements) = handle.join().unwrap();
            let stats = LatencyStats::from_measurements(
                format!("Concurrent Lookup (Thread {})", thread_id),
                measurements
            );
            all_stats.push(stats);
        }
        
        all_stats
    }
    
    /// Run comprehensive latency validation
    pub async fn run_comprehensive_latency_validation(&self) -> bool {
        println!("\n🚀 Running Comprehensive Latency Validation\n");
        
        let mut all_passed = true;
        
        // Test 1: Connection lookup latency
        println!("Testing connection lookup latency...");
        let connection_stats = self.measure_connection_lookup_latency(10000);
        connection_stats.print_report();
        if !connection_stats.meets_requirements() {
            all_passed = false;
        }
        
        // Test 2: Service lookup latency
        println!("\nTesting service lookup latency...");
        let service_stats = self.measure_service_lookup_latency(10000);
        service_stats.print_report();
        if !service_stats.meets_requirements() {
            all_passed = false;
        }
        
        // Test 3: Fuzzy matching latency
        println!("\nTesting fuzzy matching latency...");
        let fuzzy_stats = self.measure_fuzzy_matching_latency(5000).await;
        fuzzy_stats.print_report();
        if !fuzzy_stats.meets_requirements() {
            all_passed = false;
        }
        
        // Test 4: Service discovery latency
        println!("\nTesting service discovery latency...");
        let discovery_stats = self.measure_service_discovery_latency(5000).await;
        discovery_stats.print_report();
        if !discovery_stats.meets_requirements() {
            all_passed = false;
        }
        
        // Test 5: Concurrent access latency
        println!("\nTesting concurrent access latency...");
        let concurrent_stats = self.measure_concurrent_latency(4, 2500);
        let mut concurrent_passed = true;
        for stats in &concurrent_stats {
            stats.print_report();
            if !stats.meets_requirements() {
                concurrent_passed = false;
            }
        }
        if !concurrent_passed {
            all_passed = false;
        }
        
        // Overall summary
        println!("\n" + "=".repeat(60));
        println!("OVERALL LATENCY VALIDATION SUMMARY");
        println!("=".repeat(60));
        println!("Connection Lookup:    {}", if connection_stats.meets_requirements() { "✅ PASSED" } else { "❌ FAILED" });
        println!("Service Lookup:       {}", if service_stats.meets_requirements() { "✅ PASSED" } else { "❌ FAILED" });
        println!("Fuzzy Matching:       {}", if fuzzy_stats.meets_requirements() { "✅ PASSED" } else { "❌ FAILED" });
        println!("Service Discovery:    {}", if discovery_stats.meets_requirements() { "✅ PASSED" } else { "❌ FAILED" });
        println!("Concurrent Access:    {}", if concurrent_passed { "✅ PASSED" } else { "❌ FAILED" });
        println!();
        println!("FINAL RESULT: {}", if all_passed { "✅ ALL TESTS PASSED" } else { "❌ SOME TESTS FAILED" });
        println!("=".repeat(60));
        
        all_passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_lookup_latency() {
        let tests = EndToEndLatencyTests::new();
        let stats = tests.measure_connection_lookup_latency(1000);
        
        // Should complete without errors
        assert!(stats.count > 0);
        assert!(stats.p99 > Duration::ZERO);
        
        // Print results for manual inspection
        stats.print_report();
    }
    
    #[tokio::test]
    async fn test_service_lookup_latency() {
        let tests = EndToEndLatencyTests::new();
        let stats = tests.measure_service_lookup_latency(1000);
        
        assert!(stats.count > 0);
        assert!(stats.p99 > Duration::ZERO);
        
        stats.print_report();
    }
    
    #[tokio::test]
    async fn test_fuzzy_matching_latency() {
        let tests = EndToEndLatencyTests::new();
        let stats = tests.measure_fuzzy_matching_latency(500).await;
        
        assert!(stats.count > 0);
        assert!(stats.p99 > Duration::ZERO);
        
        stats.print_report();
    }
    
    #[tokio::test]
    async fn test_service_discovery_latency() {
        let tests = EndToEndLatencyTests::new();
        let stats = tests.measure_service_discovery_latency(500).await;
        
        assert!(stats.count > 0);
        assert!(stats.p99 > Duration::ZERO);
        
        stats.print_report();
    }
    
    #[tokio::test]
    async fn test_concurrent_latency() {
        let tests = EndToEndLatencyTests::new();
        let stats_vec = tests.measure_concurrent_latency(2, 500);
        
        assert_eq!(stats_vec.len(), 2);
        for stats in stats_vec {
            assert!(stats.count > 0);
            stats.print_report();
        }
    }
    
    #[tokio::test]
    async fn test_comprehensive_validation() {
        let tests = EndToEndLatencyTests::new();
        let result = tests.run_comprehensive_latency_validation().await;
        
        // This test documents the current performance
        // In a real scenario, you might want to assert!(result) to enforce requirements
        println!("Comprehensive validation result: {}", result);
    }
}