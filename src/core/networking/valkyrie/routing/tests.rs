// Tests for Advanced Message Routing & Load Balancing System
// Task 3.1.10: Testing and Validation

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    /// Create a test network topology
    fn create_test_topology() -> NetworkTopology {
        let mut topology = NetworkTopology::new();
        
        // Create test nodes
        let node1 = NetworkNode {
            id: Uuid::new_v4(),
            address: SocketAddr::from_str("127.0.0.1:8001").unwrap(),
            region: "us-east-1".to_string(),
            zone: "us-east-1a".to_string(),
            capabilities: NodeCapabilities::default(),
            metrics: NodeMetrics::default(),
            status: NodeStatus::Active,
            security_level: SecurityLevel::Internal,
        };
        
        let node2 = NetworkNode {
            id: Uuid::new_v4(),
            address: SocketAddr::from_str("127.0.0.1:8002").unwrap(),
            region: "us-east-1".to_string(),
            zone: "us-east-1b".to_string(),
            capabilities: NodeCapabilities::default(),
            metrics: NodeMetrics::default(),
            status: NodeStatus::Active,
            security_level: SecurityLevel::Internal,
        };
        
        let node3 = NetworkNode {
            id: Uuid::new_v4(),
            address: SocketAddr::from_str("127.0.0.1:8003").unwrap(),
            region: "us-west-2".to_string(),
            zone: "us-west-2a".to_string(),
            capabilities: NodeCapabilities::default(),
            metrics: NodeMetrics::default(),
            status: NodeStatus::Active,
            security_level: SecurityLevel::Internal,
        };
        
        // Create test links
        let link1 = NetworkLink {
            id: Uuid::new_v4(),
            from: node1.id,
            to: node2.id,
            transport: TransportType::TCP,
            latency: Duration::from_millis(10),
            bandwidth: 1_000_000,
            reliability: 0.99,
            cost: 1.0,
            utilization: 0.3,
            status: LinkStatus::Active,
            last_updated: Instant::now(),
        };
        
        let link2 = NetworkLink {
            id: Uuid::new_v4(),
            from: node2.id,
            to: node3.id,
            transport: TransportType::TCP,
            latency: Duration::from_millis(50),
            bandwidth: 500_000,
            reliability: 0.95,
            cost: 2.0,
            utilization: 0.5,
            status: LinkStatus::Active,
            last_updated: Instant::now(),
        };
        
        let link3 = NetworkLink {
            id: Uuid::new_v4(),
            from: node1.id,
            to: node3.id,
            transport: TransportType::QUIC,
            latency: Duration::from_millis(30),
            bandwidth: 2_000_000,
            reliability: 0.98,
            cost: 1.5,
            utilization: 0.2,
            status: LinkStatus::Active,
            last_updated: Instant::now(),
        };
        
        // Add nodes and links to topology
        topology.nodes.insert(node1.id, node1);
        topology.nodes.insert(node2.id, node2);
        topology.nodes.insert(node3.id, node3);
        topology.links.insert(link1.id, link1);
        topology.links.insert(link2.id, link2);
        topology.links.insert(link3.id, link3);
        
        topology
    }

    /// Create a test routing context
    fn create_test_context(source: NodeId, destination: NodeId) -> RoutingContext {
        RoutingContext {
            message_id: Uuid::new_v4(),
            source,
            destination,
            qos_requirements: QoSRequirements {
                max_latency: Some(Duration::from_millis(100)),
                min_bandwidth: Some(100_000),
                reliability_threshold: Some(0.95),
                priority: MessagePriority::Normal,
                sla_class: None,
            },
            security_context: SecurityContext::default(),
            routing_hints: RoutingHints::default(),
            deadline: Some(Instant::now() + Duration::from_secs(10)),
            created_at: Instant::now(),
        }
    }

    #[tokio::test]
    async fn test_dijkstra_shortest_path() {
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let cost_calculator = Arc::new(DefaultCostCalculator);
        let strategy = DijkstraStrategy::new(cost_calculator);
        
        let context = create_test_context(nodes[0], nodes[2]);
        
        let route = strategy
            .calculate_route(&nodes[0], &nodes[2], &context, &topology)
            .await
            .expect("Should find a route");
        
        assert!(!route.path.is_empty());
        assert_eq!(route.path[0], nodes[0]);
        assert_eq!(route.path[route.path.len() - 1], nodes[2]);
        assert!(route.total_cost > 0.0);
        assert!(route.estimated_latency > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_load_aware_routing() {
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let cost_calculator = Arc::new(DefaultCostCalculator);
        let load_monitor = Arc::new(SimpleLoadMonitor::new());
        let heuristic_calculator = Arc::new(DefaultHeuristicCalculator);
        
        let strategy = LoadAwareStrategy::new(
            cost_calculator,
            load_monitor,
            heuristic_calculator,
        );
        
        let context = create_test_context(nodes[0], nodes[2]);
        
        let route = strategy
            .calculate_route(&nodes[0], &nodes[2], &context, &topology)
            .await
            .expect("Should find a route");
        
        assert!(!route.path.is_empty());
        assert_eq!(route.path[0], nodes[0]);
        assert_eq!(route.path[route.path.len() - 1], nodes[2]);
    }

    #[tokio::test]
    async fn test_algorithm_manager() {
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let mut manager = AlgorithmManager::new(
            RoutingAlgorithm::LoadAware,
            Arc::new(SimpleAlgorithmSelector),
        );
        
        // Register strategies
        let cost_calculator = Arc::new(DefaultCostCalculator);
        manager.register_strategy(
            RoutingAlgorithm::ShortestPath,
            Arc::new(DijkstraStrategy::new(cost_calculator.clone())),
        );
        
        let load_monitor = Arc::new(SimpleLoadMonitor::new());
        let heuristic_calculator = Arc::new(DefaultHeuristicCalculator);
        manager.register_strategy(
            RoutingAlgorithm::LoadAware,
            Arc::new(LoadAwareStrategy::new(
                cost_calculator,
                load_monitor,
                heuristic_calculator,
            )),
        );
        
        let context = create_test_context(nodes[0], nodes[2]);
        
        let route = manager
            .calculate_route(&context, &topology)
            .await
            .expect("Should calculate route");
        
        assert!(!route.path.is_empty());
        assert!(route.metadata.algorithm_used.len() > 0);
    }

    #[tokio::test]
    async fn test_round_robin_load_balancing() {
        let endpoints = create_test_endpoints();
        let strategy = RoundRobinStrategy::new();
        let context = create_test_request_context();
        let health_status = create_healthy_status(&endpoints);
        
        let mut selected_endpoints = Vec::new();
        
        // Test multiple selections
        for _ in 0..endpoints.len() * 2 {
            let selected = strategy
                .select_endpoint(&endpoints, &context, &health_status)
                .await
                .expect("Should select endpoint");
            selected_endpoints.push(selected.id);
        }
        
        // Verify round-robin behavior
        assert_eq!(selected_endpoints.len(), endpoints.len() * 2);
        
        // Check that all endpoints were selected
        for endpoint in &endpoints {
            assert!(selected_endpoints.contains(&endpoint.id));
        }
    }

    #[tokio::test]
    async fn test_consistent_hashing_load_balancing() {
        let endpoints = create_test_endpoints();
        let strategy = ConsistentHashingStrategy::new(150, HashFunction::Fnv1a);
        let health_status = create_healthy_status(&endpoints);
        
        // Test with different request contexts (different session IDs)
        let contexts = vec![
            create_request_context_with_session("session1"),
            create_request_context_with_session("session2"),
            create_request_context_with_session("session3"),
        ];
        
        let mut selections = HashMap::new();
        
        for context in &contexts {
            let selected = strategy
                .select_endpoint(&endpoints, context, &health_status)
                .await
                .expect("Should select endpoint");
            selections.insert(context.session_id.clone().unwrap(), selected.id);
        }
        
        // Test consistency - same session should get same endpoint
        for context in &contexts {
            let selected = strategy
                .select_endpoint(&endpoints, context, &health_status)
                .await
                .expect("Should select endpoint");
            
            let session_id = context.session_id.clone().unwrap();
            assert_eq!(selections[&session_id], selected.id);
        }
    }

    #[tokio::test]
    async fn test_route_cache() {
        let cache_config = CacheConfig::default();
        let cache = RouteCache::new(cache_config);
        
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let key = RouteCacheKey {
            source: nodes[0],
            destination: nodes[2],
            qos_class: QoSClass::Normal,
            security_level: SecurityLevel::Internal,
            topology_version: topology.version,
        };
        
        // Test cache miss
        let cached_route = cache.get(&key).await;
        assert!(cached_route.is_none());
        
        // Create and cache a route
        let route = Route {
            id: Uuid::new_v4(),
            path: vec![nodes[0], nodes[1], nodes[2]],
            hops: Vec::new(),
            total_cost: 3.0,
            estimated_latency: Duration::from_millis(60),
            reliability_score: 0.97,
            bandwidth_available: 500_000,
            created_at: Instant::now(),
            expires_at: Some(Instant::now() + Duration::from_secs(300)),
            metadata: RouteMetadata::default(),
        };
        
        cache.put(key.clone(), route.clone()).await;
        
        // Test cache hit
        let cached_route = cache.get(&key).await;
        assert!(cached_route.is_some());
        let cached = cached_route.unwrap();
        assert_eq!(cached.id, route.id);
        assert_eq!(cached.path, route.path);
    }

    #[tokio::test]
    async fn test_qos_router() {
        let qos_router = QoSRouter::new(10_000_000, SchedulingAlgorithm::WeightedFairQueuing);
        
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let cost_calculator = Arc::new(DefaultCostCalculator);
        let mut routing_engine = AlgorithmManager::new(
            RoutingAlgorithm::LoadAware,
            Arc::new(SimpleAlgorithmSelector),
        );
        
        routing_engine.register_strategy(
            RoutingAlgorithm::ShortestPath,
            Arc::new(DijkstraStrategy::new(cost_calculator)),
        );
        
        let context = create_test_context(nodes[0], nodes[2]);
        
        let route = qos_router
            .route_with_qos(&context, &routing_engine, &topology)
            .await
            .expect("Should route with QoS");
        
        assert!(!route.path.is_empty());
        assert!(route.estimated_latency <= context.qos_requirements.max_latency.unwrap());
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let metrics_collector = RoutingMetricsCollector::new();
        
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let route = Route {
            id: Uuid::new_v4(),
            path: vec![nodes[0], nodes[2]],
            hops: Vec::new(),
            total_cost: 1.5,
            estimated_latency: Duration::from_millis(30),
            reliability_score: 0.98,
            bandwidth_available: 2_000_000,
            created_at: Instant::now(),
            expires_at: Some(Instant::now() + Duration::from_secs(300)),
            metadata: RouteMetadata::default(),
        };
        
        // Record successful routing operation
        metrics_collector
            .record_routing_operation(
                RoutingAlgorithm::ShortestPath,
                Duration::from_micros(500),
                &Ok(route),
                MessagePriority::Normal,
            )
            .await;
        
        // Record failed routing operation
        metrics_collector
            .record_routing_operation(
                RoutingAlgorithm::LoadAware,
                Duration::from_millis(2),
                &Err(RoutingError::NoRouteFound {
                    source: nodes[0],
                    destination: nodes[2],
                }),
                MessagePriority::High,
            )
            .await;
        
        let snapshot = metrics_collector.get_metrics_snapshot().await;
        
        assert_eq!(snapshot.routing.total_routes_calculated, 2);
        assert_eq!(snapshot.routing.successful_routes, 1);
        assert_eq!(snapshot.routing.failed_routes, 1);
        assert_eq!(snapshot.routing.success_rate, 0.5);
    }

    // Helper functions for tests
    fn create_test_endpoints() -> Vec<ServiceEndpoint> {
        vec![
            ServiceEndpoint {
                id: Uuid::new_v4(),
                address: SocketAddr::from_str("127.0.0.1:9001").unwrap(),
                weight: 100,
                metadata: EndpointMetadata::default(),
                health_status: EndpointHealthStatus::Healthy,
                performance_metrics: EndpointPerformanceMetrics::default(),
            },
            ServiceEndpoint {
                id: Uuid::new_v4(),
                address: SocketAddr::from_str("127.0.0.1:9002").unwrap(),
                weight: 200,
                metadata: EndpointMetadata::default(),
                health_status: EndpointHealthStatus::Healthy,
                performance_metrics: EndpointPerformanceMetrics::default(),
            },
            ServiceEndpoint {
                id: Uuid::new_v4(),
                address: SocketAddr::from_str("127.0.0.1:9003").unwrap(),
                weight: 150,
                metadata: EndpointMetadata::default(),
                health_status: EndpointHealthStatus::Healthy,
                performance_metrics: EndpointPerformanceMetrics::default(),
            },
        ]
    }

    fn create_test_request_context() -> RequestContext {
        RequestContext {
            request_id: Uuid::new_v4(),
            session_id: None,
            user_id: Some("test_user".to_string()),
            service_name: "test_service".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            source_ip: std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            created_at: Instant::now(),
        }
    }

    fn create_request_context_with_session(session_id: &str) -> RequestContext {
        RequestContext {
            request_id: Uuid::new_v4(),
            session_id: Some(session_id.to_string()),
            user_id: Some("test_user".to_string()),
            service_name: "test_service".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            source_ip: std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            created_at: Instant::now(),
        }
    }

    fn create_healthy_status(endpoints: &[ServiceEndpoint]) -> HealthStatus {
        let mut endpoint_health = HashMap::new();
        
        for endpoint in endpoints {
            endpoint_health.insert(
                endpoint.id,
                EndpointHealth {
                    status: EndpointHealthStatus::Healthy,
                    response_time: Duration::from_millis(50),
                    error_rate: 0.01,
                    throughput: 1000,
                    cpu_usage: 0.5,
                    memory_usage: 0.6,
                    connection_count: 100,
                    consecutive_failures: 0,
                    last_check: Instant::now(),
                },
            );
        }
        
        HealthStatus {
            endpoint_health,
            last_updated: Instant::now(),
            global_health_score: 1.0,
        }
    }

    // Simple algorithm selector for testing
    struct SimpleAlgorithmSelector;

    #[async_trait::async_trait]
    impl AlgorithmSelector for SimpleAlgorithmSelector {
        async fn select_algorithm(
            &self,
            context: &RoutingContext,
            available_algorithms: &[RoutingAlgorithm],
            _performance_history: &HashMap<RoutingAlgorithm, PerformanceHistory>,
        ) -> RoutingAlgorithm {
            // Simple selection based on priority
            match context.qos_requirements.priority {
                MessagePriority::Critical | MessagePriority::High => {
                    if available_algorithms.contains(&RoutingAlgorithm::LoadAware) {
                        RoutingAlgorithm::LoadAware
                    } else {
                        RoutingAlgorithm::ShortestPath
                    }
                }
                _ => {
                    if available_algorithms.contains(&RoutingAlgorithm::ShortestPath) {
                        RoutingAlgorithm::ShortestPath
                    } else {
                        available_algorithms[0]
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn test_topology_manager() {
        let nodes = vec![
            NetworkNode {
                id: Uuid::new_v4(),
                address: SocketAddr::from_str("127.0.0.1:8001").unwrap(),
                region: "us-east-1".to_string(),
                zone: "us-east-1a".to_string(),
                capabilities: NodeCapabilities::default(),
                metrics: NodeMetrics::default(),
                status: NodeStatus::Active,
                security_level: SecurityLevel::Internal,
            },
        ];
        
        let links = vec![
            NetworkLink {
                id: Uuid::new_v4(),
                from: nodes[0].id,
                to: nodes[0].id,
                transport: TransportType::TCP,
                latency: Duration::from_millis(1),
                bandwidth: 1_000_000,
                reliability: 0.99,
                cost: 0.1,
                utilization: 0.1,
                status: LinkStatus::Active,
                last_updated: Instant::now(),
            },
        ];
        
        let discovery_agents: Vec<Arc<dyn DiscoveryAgent>> = vec![
            Arc::new(StaticDiscoveryAgent::new(nodes, links)),
        ];
        
        let metric_collectors: Vec<Arc<dyn MetricCollector>> = vec![
            Arc::new(SimpleMetricsCollector),
        ];
        
        let topology_manager = TopologyManager::new(discovery_agents, metric_collectors);
        
        // Start topology management
        topology_manager.start().await.expect("Should start topology manager");
        
        // Wait a bit for discovery
        sleep(Duration::from_millis(100)).await;
        
        let topology = topology_manager.get_topology().await;
        assert!(!topology.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_performance_under_load() {
        let topology = create_test_topology();
        let nodes: Vec<_> = topology.nodes.keys().copied().collect();
        
        let cost_calculator = Arc::new(DefaultCostCalculator);
        let strategy = DijkstraStrategy::new(cost_calculator);
        
        let start_time = Instant::now();
        let mut successful_routes = 0;
        
        // Test 1000 route calculations
        for _ in 0..1000 {
            let context = create_test_context(nodes[0], nodes[2]);
            
            if let Ok(_route) = strategy
                .calculate_route(&nodes[0], &nodes[2], &context, &topology)
                .await
            {
                successful_routes += 1;
            }
        }
        
        let duration = start_time.elapsed();
        let avg_time_per_route = duration / 1000;
        
        println!("Performance test results:");
        println!("  Total time: {:?}", duration);
        println!("  Average time per route: {:?}", avg_time_per_route);
        println!("  Successful routes: {}/1000", successful_routes);
        
        // Verify performance requirements
        assert!(avg_time_per_route < Duration::from_millis(1), "Route calculation should be < 1ms");
        assert!(successful_routes >= 999, "Success rate should be > 99.9%");
    }
}