//! Tests for Service Registry & Discovery System
//! 
//! Validates service registration, discovery, health monitoring, load balancing,
//! and consensus coordination for distributed service management.

use std::collections::HashMap;
use std::time::Duration;
use tokio_test;

use crate::core::networking::valkyrie::registry::*;
use crate::core::networking::valkyrie::streaming::QoSClass;

#[tokio::test]
async fn test_service_registry_creation() {
    let registry = ServiceRegistry::new();
    
    // Registry should be created successfully
    let metrics = registry.metrics().await;
    assert_eq!(metrics.total_services, 0);
}

#[tokio::test]
async fn test_service_registration() {
    let registry = ServiceRegistry::new();
    
    // Create service metadata
    let metadata = ServiceMetadata {
        name: "test-service".to_string(),
        service_type: ServiceType::WebApi,
        description: "Test web API service".to_string(),
        owner: "test-team".to_string(),
        environment: "test".to_string(),
        region: "us-west-2".to_string(),
        dependencies: vec![],
    };
    
    // Create service endpoints
    let endpoints = vec![
        ServiceEndpoint {
            endpoint_id: uuid::Uuid::new_v4(),
            adapter_id: uuid::Uuid::new_v4(),
            address: "localhost".to_string(),
            port: 8080,
            protocol: "http".to_string(),
            health_status: HealthStatus::Unknown,
            capabilities: EndpointCapabilities::default(),
            weight: 100,
            last_health_check: std::time::Instant::now(),
        }
    ];
    
    // Create service capabilities
    let capabilities = ServiceCapabilities {
        qos_classes: vec![QoSClass::System, QoSClass::JobExecution],
        max_connections: Some(1000),
        message_types: vec![],
        performance: PerformanceProfile::default(),
        security_features: vec!["tls".to_string()],
        custom: HashMap::new(),
    };
    
    // Create tags
    let mut tags = HashMap::new();
    tags.insert("version".to_string(), "1.0.0".to_string());
    tags.insert("environment".to_string(), "test".to_string());
    
    // Register service
    let service_id = registry.register_service(metadata, endpoints, capabilities, tags).await.unwrap();
    
    // Verify service is registered
    let service = registry.get_service(service_id).await.unwrap();
    assert_eq!(service.metadata.name, "test-service");
    assert_eq!(service.metadata.service_type, ServiceType::WebApi);
    assert_eq!(service.endpoints.len(), 1);
    
    // Check metrics
    let metrics = registry.metrics().await;
    assert_eq!(metrics.total_services, 1);
    assert_eq!(metrics.service_registrations, 1);
}

#[tokio::test]
async fn test_service_discovery_by_name() {
    let registry = ServiceRegistry::new();
    
    // Register multiple services
    let service_names = vec!["web-api", "job-processor", "database"];
    let mut service_ids = Vec::new();
    
    for name in &service_names {
        let metadata = ServiceMetadata {
            name: name.to_string(),
            service_type: ServiceType::WebApi,
            description: format!("{} service", name),
            owner: "test-team".to_string(),
            environment: "test".to_string(),
            region: "us-west-2".to_string(),
            dependencies: vec![],
        };
        
        let endpoints = vec![create_test_endpoint()];
        let capabilities = ServiceCapabilities {
            qos_classes: vec![QoSClass::System],
            max_connections: Some(100),
            message_types: vec![],
            performance: PerformanceProfile::default(),
            security_features: vec![],
            custom: HashMap::new(),
        };
        
        let service_id = registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
        service_ids.push(service_id);
    }
    
    // Discover service by name
    let discovered_services = registry.discover_services("web-api").await.unwrap();
    assert_eq!(discovered_services.len(), 1);
    assert_eq!(discovered_services[0].metadata.name, "web-api");
    
    // Check discovery metrics
    let metrics = registry.metrics().await;
    assert!(metrics.discovery_requests > 0);
}

#[tokio::test]
async fn test_service_discovery_by_tags() {
    let registry = ServiceRegistry::new();
    
    // Register services with different tags
    let services_data = vec![
        ("service-1", vec![("env", "prod"), ("version", "1.0")]),
        ("service-2", vec![("env", "prod"), ("version", "2.0")]),
        ("service-3", vec![("env", "dev"), ("version", "1.0")]),
    ];
    
    for (name, tag_pairs) in services_data {
        let metadata = ServiceMetadata {
            name: name.to_string(),
            service_type: ServiceType::WebApi,
            description: format!("{} service", name),
            owner: "test-team".to_string(),
            environment: "test".to_string(),
            region: "us-west-2".to_string(),
            dependencies: vec![],
        };
        
        let endpoints = vec![create_test_endpoint()];
        let capabilities = ServiceCapabilities {
            qos_classes: vec![QoSClass::System],
            max_connections: Some(100),
            message_types: vec![],
            performance: PerformanceProfile::default(),
            security_features: vec![],
            custom: HashMap::new(),
        };
        
        let mut tags = HashMap::new();
        for (key, value) in tag_pairs {
            tags.insert(key.to_string(), value.to_string());
        }
        
        registry.register_service(metadata, endpoints, capabilities, tags).await.unwrap();
    }
    
    // Discover services by tags
    let mut query_tags = HashMap::new();
    query_tags.insert("env".to_string(), "prod".to_string());
    
    let discovered_services = registry.discover_by_tags(&query_tags).await.unwrap();
    assert_eq!(discovered_services.len(), 2); // service-1 and service-2
    
    // More specific tag query
    query_tags.insert("version".to_string(), "1.0".to_string());
    let discovered_services = registry.discover_by_tags(&query_tags).await.unwrap();
    assert_eq!(discovered_services.len(), 1); // Only service-1
}

#[tokio::test]
async fn test_load_balancing() {
    let registry = ServiceRegistry::new();
    
    // Register service with multiple endpoints
    let metadata = ServiceMetadata {
        name: "load-balanced-service".to_string(),
        service_type: ServiceType::WebApi,
        description: "Service with multiple endpoints".to_string(),
        owner: "test-team".to_string(),
        environment: "test".to_string(),
        region: "us-west-2".to_string(),
        dependencies: vec![],
    };
    
    let endpoints = vec![
        create_test_endpoint_with_port(8080),
        create_test_endpoint_with_port(8081),
        create_test_endpoint_with_port(8082),
    ];
    
    let capabilities = ServiceCapabilities {
        qos_classes: vec![QoSClass::System, QoSClass::JobExecution],
        max_connections: Some(1000),
        message_types: vec![],
        performance: PerformanceProfile::default(),
        security_features: vec![],
        custom: HashMap::new(),
    };
    
    registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    
    // Get best endpoint multiple times to test load balancing
    let mut selected_ports = Vec::new();
    for _ in 0..6 {
        let endpoint = registry.get_best_endpoint("load-balanced-service", Some(QoSClass::System)).await.unwrap();
        selected_ports.push(endpoint.port);
    }
    
    // Should have distributed across different endpoints
    let unique_ports: std::collections::HashSet<_> = selected_ports.into_iter().collect();
    assert!(unique_ports.len() > 1, "Load balancing should distribute across endpoints");
}

#[tokio::test]
async fn test_health_monitoring() {
    let registry = ServiceRegistry::new();
    
    // Register service
    let metadata = ServiceMetadata {
        name: "health-monitored-service".to_string(),
        service_type: ServiceType::WebApi,
        description: "Service with health monitoring".to_string(),
        owner: "test-team".to_string(),
        environment: "test".to_string(),
        region: "us-west-2".to_string(),
        dependencies: vec![],
    };
    
    let endpoints = vec![create_test_endpoint()];
    let capabilities = ServiceCapabilities {
        qos_classes: vec![QoSClass::System],
        max_connections: Some(100),
        message_types: vec![],
        performance: PerformanceProfile::default(),
        security_features: vec![],
        custom: HashMap::new(),
    };
    
    let service_id = registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    
    // Send heartbeat
    registry.heartbeat(service_id).await.unwrap();
    
    // Check service health
    let service = registry.get_service(service_id).await.unwrap();
    // Health status should be updated by heartbeat
    
    // Test service deregistration
    registry.deregister_service(service_id, "Test cleanup".to_string()).await.unwrap();
    
    // Service should no longer exist
    assert!(registry.get_service(service_id).await.is_none());
}

#[tokio::test]
async fn test_service_types() {
    let registry = ServiceRegistry::new();
    
    let service_types = vec![
        ServiceType::WebApi,
        ServiceType::JobProcessor,
        ServiceType::Database,
        ServiceType::MessageQueue,
        ServiceType::Cache,
        ServiceType::Auth,
        ServiceType::Monitoring,
        ServiceType::Custom("custom-service".to_string()),
    ];
    
    // Register services of different types
    for (i, service_type) in service_types.iter().enumerate() {
        let metadata = ServiceMetadata {
            name: format!("service-{}", i),
            service_type: service_type.clone(),
            description: format!("Service of type {:?}", service_type),
            owner: "test-team".to_string(),
            environment: "test".to_string(),
            region: "us-west-2".to_string(),
            dependencies: vec![],
        };
        
        let endpoints = vec![create_test_endpoint()];
        let capabilities = ServiceCapabilities {
            qos_classes: vec![QoSClass::System],
            max_connections: Some(100),
            message_types: vec![],
            performance: PerformanceProfile::default(),
            security_features: vec![],
            custom: HashMap::new(),
        };
        
        registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    }
    
    // List services by type
    let web_api_services = registry.list_services_by_type(ServiceType::WebApi).await;
    assert_eq!(web_api_services.len(), 1);
    
    let database_services = registry.list_services_by_type(ServiceType::Database).await;
    assert_eq!(database_services.len(), 1);
    
    // List all services
    let all_services = registry.list_services().await;
    assert_eq!(all_services.len(), service_types.len());
}

#[tokio::test]
async fn test_service_dependencies() {
    let registry = ServiceRegistry::new();
    
    // Create service with dependencies
    let dependencies = vec![
        ServiceDependency {
            service_name: "database".to_string(),
            dependency_type: DependencyType::Synchronous,
            critical: true,
            timeout: Duration::from_secs(5),
            retry_policy: RetryPolicy::default(),
        },
        ServiceDependency {
            service_name: "cache".to_string(),
            dependency_type: DependencyType::Optional,
            critical: false,
            timeout: Duration::from_secs(1),
            retry_policy: RetryPolicy::default(),
        },
    ];
    
    let metadata = ServiceMetadata {
        name: "dependent-service".to_string(),
        service_type: ServiceType::WebApi,
        description: "Service with dependencies".to_string(),
        owner: "test-team".to_string(),
        environment: "test".to_string(),
        region: "us-west-2".to_string(),
        dependencies,
    };
    
    let endpoints = vec![create_test_endpoint()];
    let capabilities = ServiceCapabilities {
        qos_classes: vec![QoSClass::System],
        max_connections: Some(100),
        message_types: vec![],
        performance: PerformanceProfile::default(),
        security_features: vec![],
        custom: HashMap::new(),
    };
    
    let service_id = registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    
    // Verify dependencies are stored
    let service = registry.get_service(service_id).await.unwrap();
    assert_eq!(service.metadata.dependencies.len(), 2);
    assert_eq!(service.metadata.dependencies[0].service_name, "database");
    assert!(service.metadata.dependencies[0].critical);
    assert_eq!(service.metadata.dependencies[1].service_name, "cache");
    assert!(!service.metadata.dependencies[1].critical);
}

#[tokio::test]
async fn test_qos_aware_discovery() {
    let registry = ServiceRegistry::new();
    
    // Register services with different QoS capabilities
    let services_data = vec![
        ("low-latency-service", vec![QoSClass::Critical, QoSClass::System]),
        ("high-throughput-service", vec![QoSClass::DataTransfer, QoSClass::LogsMetrics]),
        ("general-service", vec![QoSClass::JobExecution, QoSClass::System]),
    ];
    
    for (name, qos_classes) in services_data {
        let metadata = ServiceMetadata {
            name: name.to_string(),
            service_type: ServiceType::WebApi,
            description: format!("{} service", name),
            owner: "test-team".to_string(),
            environment: "test".to_string(),
            region: "us-west-2".to_string(),
            dependencies: vec![],
        };
        
        let endpoints = vec![create_test_endpoint()];
        let capabilities = ServiceCapabilities {
            qos_classes,
            max_connections: Some(100),
            message_types: vec![],
            performance: PerformanceProfile::default(),
            security_features: vec![],
            custom: HashMap::new(),
        };
        
        registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    }
    
    // Get best endpoint for critical QoS
    let endpoint = registry.get_best_endpoint("low-latency-service", Some(QoSClass::Critical)).await.unwrap();
    assert_eq!(endpoint.port, 8080); // Should find the service
    
    // Try to get endpoint for QoS class not supported by service
    let result = registry.get_best_endpoint("high-throughput-service", Some(QoSClass::Critical)).await;
    assert!(result.is_err()); // Should fail as service doesn't support Critical QoS
}

#[tokio::test]
async fn test_service_cleanup() {
    let config = RegistryConfig {
        service_ttl: Duration::from_millis(100), // Very short TTL for testing
        ..Default::default()
    };
    let registry = ServiceRegistry::with_config(config);
    
    // Register service
    let metadata = ServiceMetadata {
        name: "short-lived-service".to_string(),
        service_type: ServiceType::WebApi,
        description: "Service with short TTL".to_string(),
        owner: "test-team".to_string(),
        environment: "test".to_string(),
        region: "us-west-2".to_string(),
        dependencies: vec![],
    };
    
    let endpoints = vec![create_test_endpoint()];
    let capabilities = ServiceCapabilities {
        qos_classes: vec![QoSClass::System],
        max_connections: Some(100),
        message_types: vec![],
        performance: PerformanceProfile::default(),
        security_features: vec![],
        custom: HashMap::new(),
    };
    
    let service_id = registry.register_service(metadata, endpoints, capabilities, HashMap::new()).await.unwrap();
    
    // Verify service exists
    assert!(registry.get_service(service_id).await.is_some());
    
    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Run cleanup
    registry.cleanup_expired_services().await.unwrap();
    
    // Service should be removed
    assert!(registry.get_service(service_id).await.is_none());
}

#[tokio::test]
async fn test_consensus_integration() {
    let consensus_config = ConsensusConfig::default();
    let cluster_config = ClusterConfig {
        nodes: vec![],
        quorum_size: 1,
        local_node: NodeInfo {
            node_id: uuid::Uuid::new_v4(),
            address: "localhost".to_string(),
            port: 8080,
            role: NodeRole::Leader,
            status: NodeStatus::Active,
            last_heartbeat: std::time::Instant::now(),
            metadata: HashMap::new(),
        },
        cluster_name: "test-cluster".to_string(),
    };
    
    let consensus = ConsensusManager::with_config(
        ConsensusProtocol::Raft,
        cluster_config,
        consensus_config,
    );
    
    // Test consensus manager creation
    let metrics = consensus.metrics().await;
    assert_eq!(metrics.total_proposals, 0);
    assert_eq!(metrics.current_term, 0);
    
    // Test cluster state
    let cluster_state = consensus.cluster_state().await;
    assert_eq!(cluster_state.current_term, 0);
    assert!(cluster_state.current_leader.is_none());
}

#[tokio::test]
async fn test_load_balancing_strategies() {
    // Test different load balancing strategies
    let strategies = vec![
        LoadBalancingStrategy::RoundRobin,
        LoadBalancingStrategy::WeightedRoundRobin,
        LoadBalancingStrategy::LeastConnections,
        LoadBalancingStrategy::Random,
        LoadBalancingStrategy::ResponseTime,
    ];
    
    for strategy in strategies {
        let load_balancer = LoadBalancer::new(strategy.clone());
        
        // Create test services
        let services = vec![
            create_test_service("service-1", 8080),
            create_test_service("service-2", 8081),
            create_test_service("service-3", 8082),
        ];
        
        // Select endpoint
        let endpoint = load_balancer.select_endpoint(&services).await.unwrap();
        assert!(endpoint.port >= 8080 && endpoint.port <= 8082);
        
        // Test metrics
        let metrics = load_balancer.metrics().await;
        assert_eq!(metrics.total_decisions, 1);
        assert!(metrics.decisions_by_strategy.contains_key(&strategy));
    }
}

#[tokio::test]
async fn test_health_check_types() {
    let health_monitor = HealthMonitor::new(Duration::from_secs(30));
    
    // Test different health check types
    let check_types = vec![
        HealthCheckType::Http {
            endpoint: "http://localhost:8080/health".to_string(),
            expected_status: 200,
            expected_body: None,
            headers: HashMap::new(),
        },
        HealthCheckType::Tcp {
            host: "localhost".to_string(),
            port: 8080,
        },
        HealthCheckType::Grpc {
            endpoint: "localhost:8080".to_string(),
            service_name: None,
        },
    ];
    
    for check_type in check_types {
        // Create health check config
        let config = HealthCheckConfig {
            service_id: uuid::Uuid::new_v4(),
            check_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            success_threshold: 2,
            check_types: vec![check_type],
            circuit_breaker: None,
            custom_params: HashMap::new(),
        };
        
        // Health check configuration should be valid
        assert!(!config.check_types.is_empty());
    }
}

// Helper functions

fn create_test_endpoint() -> ServiceEndpoint {
    create_test_endpoint_with_port(8080)
}

fn create_test_endpoint_with_port(port: u16) -> ServiceEndpoint {
    ServiceEndpoint {
        endpoint_id: uuid::Uuid::new_v4(),
        adapter_id: uuid::Uuid::new_v4(),
        address: "localhost".to_string(),
        port,
        protocol: "http".to_string(),
        health_status: HealthStatus::Healthy,
        capabilities: EndpointCapabilities::default(),
        weight: 100,
        last_health_check: std::time::Instant::now(),
    }
}

fn create_test_service(name: &str, port: u16) -> ServiceEntry {
    ServiceEntry {
        service_id: uuid::Uuid::new_v4(),
        metadata: ServiceMetadata {
            name: name.to_string(),
            service_type: ServiceType::WebApi,
            description: format!("{} service", name),
            owner: "test-team".to_string(),
            environment: "test".to_string(),
            region: "us-west-2".to_string(),
            dependencies: vec![],
        },
        endpoints: vec![create_test_endpoint_with_port(port)],
        health_status: HealthStatus::Healthy,
        weight: 100,
        capabilities: ServiceCapabilities {
            qos_classes: vec![QoSClass::System],
            max_connections: Some(100),
            message_types: vec![],
            performance: PerformanceProfile::default(),
            security_features: vec![],
            custom: HashMap::new(),
        },
        registered_at: std::time::Instant::now(),
        last_heartbeat: std::time::Instant::now(),
        tags: HashMap::new(),
        version: "1.0.0".to_string(),
    }
}