use std::time::Duration;
use uuid::Uuid;

use rustci::core::performance::{
    LoadBalancer, ProductionLoadBalancer, LoadBalancerConfig, LoadBalancingAlgorithm,
    ServiceEndpoint, HealthStatus, RoutingContext
};

#[tokio::test]
async fn test_load_balancer_creation() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.total_endpoints, 0);
    assert_eq!(stats.healthy_endpoints, 0);
}

#[tokio::test]
async fn test_add_and_remove_endpoints() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoint
    let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let endpoint_id = endpoint.id;
    
    lb.add_endpoint(endpoint).await.unwrap();
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.total_endpoints, 1);
    
    // Remove endpoint
    lb.remove_endpoint(endpoint_id).await.unwrap();
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.total_endpoints, 0);
}

#[tokio::test]
async fn test_round_robin_algorithm() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::RoundRobin;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    let endpoint1_id = endpoint1.id;
    let endpoint2_id = endpoint2.id;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    
    // Should alternate between endpoints
    let selected1 = lb.select_endpoint(&context).await.unwrap();
    let selected2 = lb.select_endpoint(&context).await.unwrap();
    let selected3 = lb.select_endpoint(&context).await.unwrap();
    
    // Should cycle through endpoints
    assert_ne!(selected1.id, selected2.id);
    assert_eq!(selected1.id, selected3.id);
}

#[tokio::test]
async fn test_weighted_round_robin() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::WeightedRoundRobin;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoints with different weights
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint1.weight = 100;
    endpoint2.health_status = HealthStatus::Healthy;
    endpoint2.weight = 200; // Higher weight
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    
    // Collect selections to verify weight distribution
    let mut selections = Vec::new();
    for _ in 0..300 { // Test with many requests
        let selected = lb.select_endpoint(&context).await.unwrap();
        selections.push(selected.id);
    }
    
    let endpoint2_count = selections.iter().filter(|&&id| id == endpoint2.id).count();
    let endpoint1_count = selections.iter().filter(|&&id| id == endpoint1.id).count();
    
    // endpoint2 should be selected roughly twice as often due to higher weight
    assert!(endpoint2_count > endpoint1_count);
}

#[tokio::test]
async fn test_least_connections_algorithm() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::LeastConnections;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoints with different connection counts
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint1.current_connections = 10;
    endpoint2.health_status = HealthStatus::Healthy;
    endpoint2.current_connections = 5; // Fewer connections
    
    let endpoint2_id = endpoint2.id;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let selected = lb.select_endpoint(&context).await.unwrap();
    
    // Should select endpoint with fewer connections
    assert_eq!(selected.id, endpoint2_id);
}

#[tokio::test]
async fn test_ip_hash_algorithm() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::IpHash;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    // Same IP should always get same endpoint
    let mut context = RoutingContext::new("/test".to_string(), "GET".to_string());
    context.client_ip = Some("192.168.1.100".to_string());
    
    let selected1 = lb.select_endpoint(&context).await.unwrap();
    let selected2 = lb.select_endpoint(&context).await.unwrap();
    let selected3 = lb.select_endpoint(&context).await.unwrap();
    
    assert_eq!(selected1.id, selected2.id);
    assert_eq!(selected2.id, selected3.id);
}

#[tokio::test]
async fn test_health_aware_algorithm() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::HealthAware;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoints with different health scores
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint1.current_connections = 50;
    endpoint1.max_connections = 100;
    endpoint1.response_time = Duration::from_millis(200);
    
    endpoint2.health_status = HealthStatus::Healthy;
    endpoint2.current_connections = 20;
    endpoint2.max_connections = 100;
    endpoint2.response_time = Duration::from_millis(50);
    
    let endpoint2_id = endpoint2.id;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let selected = lb.select_endpoint(&context).await.unwrap();
    
    // Should select endpoint with better health score (endpoint2)
    assert_eq!(selected.id, endpoint2_id);
}

#[tokio::test]
async fn test_no_healthy_endpoints() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    // Add unhealthy endpoint
    let mut endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    endpoint.health_status = HealthStatus::Unhealthy;
    
    lb.add_endpoint(endpoint).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let result = lb.select_endpoint(&context).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_endpoint_at_max_connections() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoint at max connections
    let mut endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    endpoint.health_status = HealthStatus::Healthy;
    endpoint.current_connections = 100;
    endpoint.max_connections = 100;
    
    lb.add_endpoint(endpoint).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let result = lb.select_endpoint(&context).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_request_lifecycle_tracking() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let endpoint_id = endpoint.id;
    
    lb.add_endpoint(endpoint).await.unwrap();
    
    // Record request start
    lb.record_request_start(endpoint_id).await.unwrap();
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.active_connections, 1);
    
    // Record request end
    lb.record_request_end(endpoint_id, true, Duration::from_millis(100)).await.unwrap();
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.active_connections, 0);
    assert!(stats.average_response_time.as_millis() > 0);
}

#[tokio::test]
async fn test_health_status_updates() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let endpoint_id = endpoint.id;
    
    lb.add_endpoint(endpoint).await.unwrap();
    
    // Update health status
    lb.update_endpoint_health(endpoint_id, HealthStatus::Healthy).await.unwrap();
    
    let healthy_endpoints = lb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 1);
    assert_eq!(healthy_endpoints[0].health_status, HealthStatus::Healthy);
    
    // Update to unhealthy
    lb.update_endpoint_health(endpoint_id, HealthStatus::Unhealthy).await.unwrap();
    
    let healthy_endpoints = lb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 0);
}

#[tokio::test]
async fn test_sticky_sessions() {
    let mut config = LoadBalancerConfig::default();
    config.sticky_sessions = true;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    // Same session should get same endpoint
    let mut context = RoutingContext::new("/test".to_string(), "GET".to_string());
    context.session_id = Some("session123".to_string());
    
    let selected1 = lb.select_endpoint(&context).await.unwrap();
    let selected2 = lb.select_endpoint(&context).await.unwrap();
    let selected3 = lb.select_endpoint(&context).await.unwrap();
    
    assert_eq!(selected1.id, selected2.id);
    assert_eq!(selected2.id, selected3.id);
}

#[tokio::test]
async fn test_load_balancer_stats() {
    let config = LoadBalancerConfig::default();
    let lb = ProductionLoadBalancer::new(config);
    
    // Add endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Unhealthy;
    
    lb.add_endpoint(endpoint1.clone()).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    // Record some requests
    lb.record_request_start(endpoint1.id).await.unwrap();
    lb.record_request_end(endpoint1.id, true, Duration::from_millis(150)).await.unwrap();
    
    lb.record_request_start(endpoint1.id).await.unwrap();
    lb.record_request_end(endpoint1.id, false, Duration::from_millis(200)).await.unwrap();
    
    let stats = lb.get_stats().await;
    assert_eq!(stats.total_endpoints, 2);
    assert_eq!(stats.healthy_endpoints, 1);
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.failed_requests, 1);
    assert!(stats.average_response_time.as_millis() > 0);
    assert_eq!(stats.requests_per_endpoint.get(&endpoint1.id), Some(&2));
}

#[tokio::test]
async fn test_random_algorithm() {
    let mut config = LoadBalancerConfig::default();
    config.algorithm = LoadBalancingAlgorithm::Random;
    let lb = ProductionLoadBalancer::new(config);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    lb.add_endpoint(endpoint1).await.unwrap();
    lb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    
    // Test multiple selections - should get both endpoints over time
    let mut endpoint_ids = std::collections::HashSet::new();
    for _ in 0..20 {
        let selected = lb.select_endpoint(&context).await.unwrap();
        endpoint_ids.insert(selected.id);
    }
    
    // Should have selected both endpoints at some point
    assert_eq!(endpoint_ids.len(), 2);
}

#[tokio::test]
async fn test_endpoint_utilization_calculation() {
    let mut endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    endpoint.current_connections = 50;
    endpoint.max_connections = 100;
    
    assert_eq!(endpoint.connection_utilization(), 0.5);
    
    endpoint.current_connections = 0;
    assert_eq!(endpoint.connection_utilization(), 0.0);
    
    endpoint.current_connections = 100;
    assert_eq!(endpoint.connection_utilization(), 1.0);
    
    // Test edge case
    endpoint.max_connections = 0;
    assert_eq!(endpoint.connection_utilization(), 0.0);
}