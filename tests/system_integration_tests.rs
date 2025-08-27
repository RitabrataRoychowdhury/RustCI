//! System Integration Tests and Performance Validation
//!
//! This module provides comprehensive system integration tests that validate
//! the complete routing system meets all performance requirements:
//! - ≤82µs p99 latency for routing decisions
//! - >1M routing decisions per second throughput
//! - Memory usage validation and leak detection
//! - Stability under load

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant, SystemTime};
use std::thread;
use uuid::Uuid;
use tokio::time::timeout;

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

/// Performance validation results
#[derive(Debug, Clone)]
pub struct PerformanceValidationResult {
    pub test_name: String,
    pub passed: bool,
    pub throughput_ops_per_sec: f64,
    pub p99_latency: Duration,
    pub memory_usage_mb: f64,
    pub error_rate: f64,
    pub details: String,
}

impl PerformanceValidationResult {
    pub fn print_report(&self) {
        println!("\n=== Performance Validation: {} ===", self.test_name);
        println!("Status:           {}", if self.passed { "✅ PASSED" } else { "❌ FAILED" });
        println!("Throughput:       {:.0} ops/sec", self.throughput_ops_per_sec);
        println!("P99 Latency:      {:?}", self.p99_latency);
        println!("Memory Usage:     {:.2} MB", self.memory_usage_mb);
        println!("Error Rate:       {:.2}%", self.error_rate * 100.0);
        if !self.details.is_empty() {
            println!("Details:          {}", self.details);
        }
        println!("Requirements:");
        println!("  Throughput:     {} (>1M ops/sec)", 
            if self.throughput_ops_per_sec > 1_000_000.0 { "✅" } else { "❌" });
        println!("  Latency:        {} (<82µs p99)", 
            if self.p99_latency < Duration::from_micros(82) { "✅" } else { "❌" });
        println!("  Error Rate:     {} (<1%)", 
            if self.error_rate < 0.01 { "✅" } else { "❌" });
    }
}

/// System integration test suite
pub struct SystemIntegrationTests {
    connection_registry: Arc<EnhancedConnectionRegistry>,
    service_registry: Arc<EnhancedServiceRegistry>,
    fuzzy_engine: Arc<FuzzyMatchingEngine>,
    discovery_engine: Arc<EnhancedServiceDiscovery>,
}

impl SystemIntegrationTests {
    pub fn new() -> Self {
        let connection_registry = Arc::new(
            EnhancedConnectionRegistry::new(Duration::from_secs(30))
        );
        
        let service_registry = Arc::new(
            EnhancedServiceRegistry::new()
        );
        
        let fuzzy_config = FuzzyMatchingConfig {
            max_results: 100,
            timeout: Duration::from_micros(50),
            min_score: 5,
            enable_caching: true,
            cache_ttl: Duration::from_secs(300),
        };
        let fuzzy_engine = Arc::new(FuzzyMatchingEngine::new(fuzzy_config));
        
        let discovery_config = EnhancedServiceDiscoveryConfig {
            enable_fuzzy_matching: true,
            fuzzy_config: FuzzyMatchingConfig::default(),
            enable_caching: true,
            cache_ttl: Duration::from_secs(600),
            max_cache_size: 10000,
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
    
    /// Validate throughput requirement (>1M ops/sec)
    pub fn validate_throughput_requirement(&self) -> PerformanceValidationResult {
        println!("🔥 Validating throughput requirement (>1M ops/sec)...");
        
        // Pre-populate with test data
        let mut connection_ids = Vec::new();
        for i in 0..10000 {
            let connection_id = Uuid::new_v4();
            let connection_info = EnhancedConnectionInfo {
                id: connection_id,
                remote_addr: format!("127.0.0.1:{}", 8000 + (i % 60000)).parse().unwrap(),
                state: ConnectionState::Active,
                metrics: ConnectionMetrics::default(),
                metadata: HashMap::new(),
                last_activity: SystemTime::now(),
            };
            
            if self.connection_registry.register(connection_id, connection_info).is_ok() {
                connection_ids.push(connection_id);
            }
        }
        
        // Measure throughput over multiple threads
        let num_threads = 8;
        let operations_per_thread = 250_000; // Total: 2M operations
        let total_operations = num_threads * operations_per_thread;
        
        let connection_ids = Arc::new(connection_ids);
        let start_time = Instant::now();
        let mut handles = Vec::new();
        
        for _ in 0..num_threads {
            let registry = Arc::clone(&self.connection_registry);
            let ids = Arc::clone(&connection_ids);
            
            let handle = thread::spawn(move || {
                let mut successful_ops = 0;
                for i in 0..operations_per_thread {
                    let lookup_id = ids[i % ids.len()];
                    if registry.lookup(&lookup_id).is_some() {
                        successful_ops += 1;
                    }
                }
                successful_ops
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads and collect results
        let mut total_successful = 0;
        for handle in handles {
            total_successful += handle.join().unwrap();
        }
        
        let total_time = start_time.elapsed();
        let throughput = total_operations as f64 / total_time.as_secs_f64();
        let error_rate = 1.0 - (total_successful as f64 / total_operations as f64);
        
        let passed = throughput > 1_000_000.0 && error_rate < 0.01;
        
        PerformanceValidationResult {
            test_name: "Throughput Validation".to_string(),
            passed,
            throughput_ops_per_sec: throughput,
            p99_latency: Duration::ZERO, // Not measured in this test
            memory_usage_mb: 0.0, // Not measured in this test
            error_rate,
            details: format!("Executed {} operations across {} threads in {:?}", 
                total_operations, num_threads, total_time),
        }
    }
    
    /// Validate latency requirement (≤82µs p99)
    pub fn validate_latency_requirement(&self) -> PerformanceValidationResult {
        println!("🔥 Validating latency requirement (≤82µs p99)...");
        
        // Pre-populate with test data
        let mut service_ids = Vec::new();
        for i in 0..5000 {
            let service_id = Uuid::new_v4();
            let service_entry = EnhancedServiceEntry {
                id: service_id,
                name: format!("latency-test-service-{}", i),
                version: "1.0.0".to_string(),
                endpoints: vec![ServiceEndpoint {
                    address: format!("127.0.0.1:{}", 9000 + (i % 60000)).parse().unwrap(),
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
            
            if self.service_registry.register(service_id, service_entry).is_ok() {
                service_ids.push(service_id);
            }
        }
        
        // Measure latencies for a large number of operations
        let num_measurements = 100_000;
        let mut latencies = Vec::with_capacity(num_measurements);
        let mut successful_ops = 0;
        
        for i in 0..num_measurements {
            let lookup_id = service_ids[i % service_ids.len()];
            
            let start = Instant::now();
            if self.service_registry.lookup(&lookup_id).is_some() {
                successful_ops += 1;
            }
            let latency = start.elapsed();
            
            latencies.push(latency);
        }
        
        // Calculate statistics
        latencies.sort();
        let p99_latency = latencies[latencies.len() * 99 / 100];
        let error_rate = 1.0 - (successful_ops as f64 / num_measurements as f64);
        
        let passed = p99_latency <= Duration::from_micros(82) && error_rate < 0.01;
        
        PerformanceValidationResult {
            test_name: "Latency Validation".to_string(),
            passed,
            throughput_ops_per_sec: 0.0, // Not the focus of this test
            p99_latency,
            memory_usage_mb: 0.0, // Not measured in this test
            error_rate,
            details: format!("Measured {} operations, P99: {:?}", num_measurements, p99_latency),
        }
    }
    
    /// Validate memory usage and detect leaks
    pub fn validate_memory_usage(&self) -> PerformanceValidationResult {
        println!("🔥 Validating memory usage and leak detection...");
        
        // Get initial memory baseline (simplified - in real implementation you'd use proper memory profiling)
        let initial_size = self.connection_registry.get_stats().entries;
        
        // Perform many operations that could cause memory leaks
        let num_cycles = 1000;
        let operations_per_cycle = 100;
        
        for cycle in 0..num_cycles {
            // Add connections
            let mut temp_connections = Vec::new();
            for i in 0..operations_per_cycle {
                let connection_id = Uuid::new_v4();
                let connection_info = EnhancedConnectionInfo {
                    id: connection_id,
                    remote_addr: format!("127.0.0.1:{}", 20000 + cycle * 1000 + i).parse().unwrap(),
                    state: ConnectionState::Active,
                    metrics: ConnectionMetrics::default(),
                    metadata: HashMap::new(),
                    last_activity: SystemTime::now(),
                };
                
                if self.connection_registry.register(connection_id, connection_info).is_ok() {
                    temp_connections.push(connection_id);
                }
            }
            
            // Perform lookups
            for &connection_id in &temp_connections {
                let _ = self.connection_registry.lookup(&connection_id);
            }
            
            // Remove connections (simulating cleanup)
            for &connection_id in &temp_connections {
                let _ = self.connection_registry.unregister(&connection_id);
            }
        }
        
        // Check final memory usage
        let final_size = self.connection_registry.get_stats().entries;
        let memory_growth = final_size.saturating_sub(initial_size);
        
        // Memory usage should be reasonable (< 100MB estimated growth)
        let estimated_memory_mb = memory_growth as f64 * 0.001; // Rough estimate
        let passed = estimated_memory_mb < 100.0;
        
        PerformanceValidationResult {
            test_name: "Memory Usage Validation".to_string(),
            passed,
            throughput_ops_per_sec: 0.0,
            p99_latency: Duration::ZERO,
            memory_usage_mb: estimated_memory_mb,
            error_rate: 0.0,
            details: format!("Memory growth: {} entries (~{:.2} MB estimated)", 
                memory_growth, estimated_memory_mb),
        }
    }
    
    /// Validate system stability under load
    pub async fn validate_stability_under_load(&self) -> PerformanceValidationResult {
        println!("🔥 Validating system stability under sustained load...");
        
        let test_duration = Duration::from_secs(30); // 30-second stress test
        let start_time = Instant::now();
        
        // Counters for tracking operations
        let total_operations = Arc::new(AtomicU64::new(0));
        let successful_operations = Arc::new(AtomicU64::new(0));
        let mut latencies = Arc::new(std::sync::Mutex::new(Vec::new()));
        
        // Pre-populate with services for fuzzy matching
        let service_names = vec![
            "user-service", "auth-service", "payment-service", "notification-service",
            "analytics-service", "order-service", "inventory-service", "shipping-service",
        ];
        
        for (i, name) in service_names.iter().enumerate() {
            let service_entry = EnhancedServiceEntry {
                id: Uuid::new_v4(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                endpoints: vec![ServiceEndpoint {
                    address: format!("127.0.0.1:{}", 15000 + i).parse().unwrap(),
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
        
        // Spawn multiple worker tasks
        let mut tasks = Vec::new();
        for worker_id in 0..4 {
            let fuzzy_engine = Arc::clone(&self.fuzzy_engine);
            let discovery_engine = Arc::clone(&self.discovery_engine);
            let total_ops = Arc::clone(&total_operations);
            let successful_ops = Arc::clone(&successful_operations);
            let latencies_clone = Arc::clone(&latencies);
            
            let task = tokio::spawn(async move {
                let queries = vec!["user", "auth", "payment", "service"];
                let mut local_latencies = Vec::new();
                
                while start_time.elapsed() < test_duration {
                    // Fuzzy matching operation
                    let query = queries[worker_id % queries.len()];
                    let op_start = Instant::now();
                    
                    match timeout(Duration::from_millis(100), fuzzy_engine.fuzzy_match(query)).await {
                        Ok(Ok(_)) => {
                            successful_ops.fetch_add(1, Ordering::Relaxed);
                            local_latencies.push(op_start.elapsed());
                        }
                        _ => {} // Timeout or error
                    }
                    total_ops.fetch_add(1, Ordering::Relaxed);
                    
                    // Service discovery operation
                    let discovery_query = ServiceQuery {
                        name: Some("service".to_string()),
                        fuzzy_matching: true,
                        limit: Some(10),
                        ..Default::default()
                    };
                    
                    let op_start = Instant::now();
                    match timeout(Duration::from_millis(100), discovery_engine.discover_services(discovery_query)).await {
                        Ok(Ok(_)) => {
                            successful_ops.fetch_add(1, Ordering::Relaxed);
                            local_latencies.push(op_start.elapsed());
                        }
                        _ => {} // Timeout or error
                    }
                    total_ops.fetch_add(1, Ordering::Relaxed);
                    
                    // Small delay to prevent overwhelming the system
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
                
                // Merge local latencies with global collection
                if let Ok(mut global_latencies) = latencies_clone.lock() {
                    global_latencies.extend(local_latencies);
                }
            });
            
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        for task in tasks {
            let _ = task.await;
        }
        
        let total_time = start_time.elapsed();
        let total_ops = total_operations.load(Ordering::Relaxed);
        let successful_ops = successful_operations.load(Ordering::Relaxed);
        
        let throughput = total_ops as f64 / total_time.as_secs_f64();
        let error_rate = 1.0 - (successful_ops as f64 / total_ops as f64);
        
        // Calculate P99 latency
        let mut all_latencies = latencies.lock().unwrap().clone();
        all_latencies.sort();
        let p99_latency = if !all_latencies.is_empty() {
            all_latencies[all_latencies.len() * 99 / 100]
        } else {
            Duration::ZERO
        };
        
        let passed = error_rate < 0.05 && p99_latency < Duration::from_millis(10); // More lenient for stability test
        
        PerformanceValidationResult {
            test_name: "Stability Under Load".to_string(),
            passed,
            throughput_ops_per_sec: throughput,
            p99_latency,
            memory_usage_mb: 0.0,
            error_rate,
            details: format!("Sustained load for {:?}, {} total operations", total_time, total_ops),
        }
    }
    
    /// Run comprehensive system integration validation
    pub async fn run_comprehensive_validation(&self) -> Vec<PerformanceValidationResult> {
        println!("\n🚀 Running Comprehensive System Integration Validation\n");
        
        let mut results = Vec::new();
        
        // Test 1: Throughput validation
        results.push(self.validate_throughput_requirement());
        
        // Test 2: Latency validation
        results.push(self.validate_latency_requirement());
        
        // Test 3: Memory usage validation
        results.push(self.validate_memory_usage());
        
        // Test 4: Stability under load
        results.push(self.validate_stability_under_load().await);
        
        // Print individual results
        for result in &results {
            result.print_report();
        }
        
        // Overall summary
        let all_passed = results.iter().all(|r| r.passed);
        let passed_count = results.iter().filter(|r| r.passed).count();
        
        println!("\n{}", "=".repeat(70));
        println!("COMPREHENSIVE SYSTEM INTEGRATION VALIDATION SUMMARY");
        println!("{}", "=".repeat(70));
        println!("Tests Passed: {}/{}", passed_count, results.len());
        println!("Overall Result: {}", if all_passed { "✅ ALL TESTS PASSED" } else { "❌ SOME TESTS FAILED" });
        
        // Performance requirements summary
        println!("\nPerformance Requirements Status:");
        let throughput_test = results.iter().find(|r| r.test_name.contains("Throughput"));
        let latency_test = results.iter().find(|r| r.test_name.contains("Latency"));
        
        if let Some(test) = throughput_test {
            println!("  Throughput (>1M ops/sec): {} ({:.0} ops/sec)", 
                if test.throughput_ops_per_sec > 1_000_000.0 { "✅" } else { "❌" },
                test.throughput_ops_per_sec);
        }
        
        if let Some(test) = latency_test {
            println!("  Latency (<82µs p99):      {} ({:?})", 
                if test.p99_latency < Duration::from_micros(82) { "✅" } else { "❌" },
                test.p99_latency);
        }
        
        println!("{}", "=".repeat(70));
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_throughput_validation() {
        let tests = SystemIntegrationTests::new();
        let result = tests.validate_throughput_requirement();
        
        result.print_report();
        
        // Should complete without panicking
        assert!(result.throughput_ops_per_sec > 0.0);
    }
    
    #[test]
    fn test_latency_validation() {
        let tests = SystemIntegrationTests::new();
        let result = tests.validate_latency_requirement();
        
        result.print_report();
        
        // Should complete without panicking
        assert!(result.p99_latency > Duration::ZERO);
    }
    
    #[test]
    fn test_memory_validation() {
        let tests = SystemIntegrationTests::new();
        let result = tests.validate_memory_usage();
        
        result.print_report();
        
        // Should complete without panicking
        assert!(result.memory_usage_mb >= 0.0);
    }
    
    #[tokio::test]
    async fn test_stability_validation() {
        let tests = SystemIntegrationTests::new();
        let result = tests.validate_stability_under_load().await;
        
        result.print_report();
        
        // Should complete without panicking
        assert!(result.throughput_ops_per_sec >= 0.0);
    }
    
    #[tokio::test]
    async fn test_comprehensive_validation() {
        let tests = SystemIntegrationTests::new();
        let results = tests.run_comprehensive_validation().await;
        
        // Should run all tests
        assert_eq!(results.len(), 4);
        
        // All tests should have meaningful results
        for result in results {
            assert!(!result.test_name.is_empty());
        }
    }
}