//! Integration Example for High-Performance Routing
//!
//! This example shows how to integrate the high-performance routing system
//! with the existing Valkyrie MessageRouter.

use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use async_trait::async_trait;

use crate::core::networking::valkyrie::routing::{
    DijkstraStrategy, NetworkTopology, NetworkNode, NodeCapabilities, NodeMetrics, NodeStatus,
    SecurityLevel, RoutingContext, QoSRequirements, MessagePriority, SLAClass, SecurityContext,
    RoutingHints, CostCalculator, NetworkLink,
};
use crate::valkyrie::routing::HighPerformanceMessageRouter;
use crate::valkyrie::lockfree::fingerprinted_id_table::{RouteEntry, RouteId, NodeId as FitNodeId, ShardId};

/// Simple cost calculator for testing
struct SimpleCostCalculator;

#[async_trait::async_trait]
impl CostCalculator for SimpleCostCalculator {
    async fn calculate_cost(
        &self,
        link: &NetworkLink,
        _context: &RoutingContext,
        current_load: Option<f64>,
    ) -> f64 {
        // Simple cost calculation based on latency and utilization
        let latency_cost = link.latency.as_micros() as f64 / 1000.0; // Convert to ms
        let utilization_penalty = link.utilization * 100.0; // Penalize high utilization
        let load_penalty = current_load.unwrap_or(0.0) * 50.0; // Penalize high load
        latency_cost + utilization_penalty + load_penalty
    }
}

/// Example of setting up high-performance routing with FIT
pub async fn setup_high_performance_routing() -> Result<Arc<HighPerformanceMessageRouter>, Box<dyn std::error::Error>> {
    // Create a fallback strategy (Dijkstra) with a simple cost calculator
    let cost_calculator = Arc::new(SimpleCostCalculator);
    let fallback_strategy = Arc::new(DijkstraStrategy::new(cost_calculator));
    
    // Create high-performance router with 64K FIT capacity
    let hp_router = HighPerformanceMessageRouter::new(65536, fallback_strategy)?;
    
    // Pre-warm the cache with some hot routes
    let hot_routes = vec![
        RouteEntry::new(
            RouteId::new(1),
            FitNodeId::new(100),
            ShardId::new(1),
            25, // 25µs latency
            65000, // High reliability
        ),
        RouteEntry::new(
            RouteId::new(2),
            FitNodeId::new(200),
            ShardId::new(2),
            30, // 30µs latency
            60000, // Good reliability
        ),
        RouteEntry::new(
            RouteId::new(3),
            FitNodeId::new(300),
            ShardId::new(1),
            20, // 20µs latency
            65535, // Maximum reliability
        ),
    ];
    
    let cached_count = hp_router.warm_cache(hot_routes).await?;
    println!("Pre-warmed FIT cache with {} routes", cached_count);
    
    Ok(Arc::new(hp_router))
}

/// Example of creating a test topology for routing
pub fn create_test_topology() -> NetworkTopology {
    let mut nodes = HashMap::new();
    let mut links = HashMap::new();
    
    // Create test nodes
    for i in 1..=5 {
        let node_id = Uuid::from_u128(i);
        let node = NetworkNode {
            id: node_id,
            address: format!("127.0.0.{}:8080", i).parse().unwrap(),
            region: "us-west-1".to_string(),
            zone: format!("us-west-1{}", char::from(b'a' + i as u8 - 1)),
            capabilities: NodeCapabilities {
                supported_transports: vec![
                    crate::core::networking::valkyrie::routing::TransportType::QUIC,
                    crate::core::networking::valkyrie::routing::TransportType::TCP,
                ],
                max_connections: 1000,
                max_bandwidth: 1_000_000_000, // 1Gbps
                encryption_support: vec!["AES256".to_string(), "ChaCha20".to_string()],
                features: vec!["high-performance".to_string()],
            },
            metrics: NodeMetrics {
                cpu_usage: 0.1 + (i as f64 * 0.1),
                memory_usage: 0.2 + (i as f64 * 0.05),
                network_utilization: 0.05 + (i as f64 * 0.02),
                connection_count: (i * 10) as u32,
                message_rate: (i * 1000) as u64,
                error_rate: 0.001,
                last_updated: std::time::SystemTime::now(),
            },
            status: NodeStatus::Active,
            security_level: SecurityLevel::Internal,
        };
        nodes.insert(node_id, node);
    }
    
    // Create test links (simplified mesh)
    for i in 1..=4 {
        let from = Uuid::from_u128(i);
        let to = Uuid::from_u128(i + 1);
        let link_id = Uuid::new_v4();
        
        let link = crate::core::networking::valkyrie::routing::NetworkLink {
            id: link_id,
            from,
            to,
            transport: crate::core::networking::valkyrie::routing::TransportType::QUIC,
            latency: std::time::Duration::from_micros((10 + i * 5) as u64),
            bandwidth: 1_000_000_000,
            reliability: 0.99 - (i as f64 * 0.01),
            cost: i as f64 * 10.0,
            utilization: 0.1 + (i as f64 * 0.05),
            status: crate::core::networking::valkyrie::routing::LinkStatus::Active,
            last_updated: std::time::SystemTime::now(),
        };
        links.insert(link_id, link);
    }
    
    NetworkTopology {
        nodes,
        links,
        regions: HashMap::new(),
        last_updated: std::time::SystemTime::now(),
        version: 1,
    }
}

/// Example of performing high-performance routing
pub async fn example_routing_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up high-performance routing...");
    let hp_router = setup_high_performance_routing().await?;
    let topology = create_test_topology();
    
    // Create routing context
    let context = RoutingContext {
        message_id: Uuid::new_v4(),
        source: Uuid::from_u128(1),
        destination: Uuid::from_u128(5),
        qos_requirements: QoSRequirements {
            max_latency: Some(std::time::Duration::from_micros(82)),
            min_bandwidth: Some(100_000_000), // 100Mbps
            reliability_threshold: Some(0.99),
            priority: MessagePriority::High,
            sla_class: Some(SLAClass::Gold),
        },
        security_context: SecurityContext {
            user_id: Some("test_user".to_string()),
            tenant_id: Some("test_tenant".to_string()),
            security_level: SecurityLevel::Internal,
            allowed_regions: vec!["us-west-1".to_string()],
            encryption_required: true,
            audit_required: false,
        },
        routing_hints: RoutingHints::default(),
        deadline: Some(std::time::SystemTime::now() + std::time::Duration::from_millis(100)),
        created_at: std::time::SystemTime::now(),
    };
    
    // Perform routing benchmark
    println!("Running routing benchmark...");
    let mut total_latency = std::time::Duration::ZERO;
    let iterations = 10_000;
    
    for i in 0..iterations {
        let start = std::time::Instant::now();
        
        let source = Uuid::from_u128(1 + (i % 3));
        let destination = Uuid::from_u128(3 + (i % 3));
        
        match hp_router.route_message_fast(&source, &destination, &context, &topology).await {
            Ok(route) => {
                let latency = start.elapsed();
                total_latency += latency;
                
                if latency > std::time::Duration::from_micros(82) {
                    println!("Warning: Routing latency exceeded budget: {:?}", latency);
                }
                
                if i % 1000 == 0 {
                    println!("Iteration {}: Route found with {} hops, latency: {:?}", 
                             i, route.hops.len(), latency);
                }
            }
            Err(e) => {
                println!("Routing failed at iteration {}: {}", i, e);
            }
        }
    }
    
    let avg_latency = total_latency / (iterations as u32);
    println!("Benchmark completed:");
    println!("  Iterations: {}", iterations);
    println!("  Average latency: {:?}", avg_latency);
    println!("  Total time: {:?}", total_latency);
    
    // Get performance statistics
    let stats = hp_router.get_routing_stats();
    println!("Performance Statistics:");
    println!("  Total requests: {}", stats.total_requests);
    println!("  FIT hit rate: {:.2}%", stats.fit_hit_rate * 100.0);
    println!("  Average latency: {}ns", stats.avg_latency_ns);
    println!("  Max latency: {}ns", stats.max_latency_ns);
    println!("  Budget violations: {}", stats.budget_violations);
    
    let health_status = if hp_router.is_healthy() { "HEALTHY" } else { "UNHEALTHY" };
    println!("  Health status: {}", health_status);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_high_performance_routing_setup() {
        let result = setup_high_performance_routing().await;
        assert!(result.is_ok());
        
        let hp_router = result.unwrap();
        assert!(hp_router.is_healthy());
    }
    
    #[tokio::test]
    async fn test_routing_benchmark() {
        let result = example_routing_benchmark().await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_topology_creation() {
        let topology = create_test_topology();
        assert_eq!(topology.nodes.len(), 5);
        assert_eq!(topology.links.len(), 4);
        assert_eq!(topology.version, 1);
    }
}