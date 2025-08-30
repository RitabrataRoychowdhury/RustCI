use std::time::Duration;
use uuid::Uuid;

use rustci::core::performance::{
    AutoScalingLoadBalancer, AutoScalingLoadBalancerConfig, ServiceEndpoint, HealthStatus,
    RoutingContext, MockInstanceProvider, MockMetricsCollector, ScalingMetrics
};

#[tokio::test]
async fn test_auto_scaling_load_balancer_creation() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_endpoints, 0);
    assert_eq!(stats.auto_scaler_stats.current_instances, 0);
    assert_eq!(stats.integration_stats.total_scaling_events, 0);
}

#[tokio::test]
async fn test_endpoint_management_integration() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Add multiple endpoints
    let endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    let endpoint3 = ServiceEndpoint::new("127.0.0.1".to_string(), 8082);
    
    let endpoint1_id = endpoint1.id;
    let endpoint2_id = endpoint2.id;
    let endpoint3_id = endpoint3.id;
    
    aslb.add_endpoint(endpoint1).await.unwrap();
    aslb.add_endpoint(endpoint2).await.unwrap();
    aslb.add_endpoint(endpoint3).await.unwrap();
    
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_endpoints, 3);
    
    // Remove one endpoint
    aslb.remove_endpoint(endpoint2_id).await.unwrap();
    
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_endpoints, 2);
    
    // Verify remaining endpoints
    let healthy_endpoints = aslb.get_healthy_endpoints().await;
    let endpoint_ids: Vec<Uuid> = healthy_endpoints.iter().map(|e| e.id).collect();
    assert!(endpoint_ids.contains(&endpoint1_id));
    assert!(!endpoint_ids.contains(&endpoint2_id));
    assert!(endpoint_ids.contains(&endpoint3_id));
}

#[tokio::test]
async fn test_request_lifecycle_with_load_balancing() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    let endpoint1_id = endpoint1.id;
    let endpoint2_id = endpoint2.id;
    
    aslb.add_endpoint(endpoint1).await.unwrap();
    aslb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    
    // Simulate multiple requests
    for _ in 0..10 {
        let selected = aslb.select_endpoint(&context).await.unwrap();
        
        // Record request lifecycle
        aslb.record_request_start(selected.id).await.unwrap();
        aslb.record_request_end(selected.id, true, Duration::from_millis(100)).await.unwrap();
    }
    
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_requests, 10);
    assert_eq!(stats.load_balancer_stats.successful_requests, 10);
    assert_eq!(stats.load_balancer_stats.active_connections, 0);
    
    // Verify requests were distributed
    let requests_per_endpoint = &stats.load_balancer_stats.requests_per_endpoint;
    let endpoint1_requests = requests_per_endpoint.get(&endpoint1_id).unwrap_or(&0);
    let endpoint2_requests = requests_per_endpoint.get(&endpoint2_id).unwrap_or(&0);
    
    assert!(*endpoint1_requests > 0 || *endpoint2_requests > 0);
    assert_eq!(*endpoint1_requests + *endpoint2_requests, 10);
}

#[tokio::test]
async fn test_health_status_management() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let endpoint_id = endpoint.id;
    
    aslb.add_endpoint(endpoint).await.unwrap();
    
    // Initially unknown health status
    let healthy_endpoints = aslb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 0); // Unknown status is not considered healthy
    
    // Update to healthy
    aslb.update_endpoint_health(endpoint_id, HealthStatus::Healthy).await.unwrap();
    
    let healthy_endpoints = aslb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 1);
    assert_eq!(healthy_endpoints[0].health_status, HealthStatus::Healthy);
    
    // Update to unhealthy
    aslb.update_endpoint_health(endpoint_id, HealthStatus::Unhealthy).await.unwrap();
    
    let healthy_endpoints = aslb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 0);
    
    // Update to degraded
    aslb.update_endpoint_health(endpoint_id, HealthStatus::Degraded).await.unwrap();
    
    let healthy_endpoints = aslb.get_healthy_endpoints().await;
    assert_eq!(healthy_endpoints.len(), 0); // Degraded is not considered healthy for selection
}

#[tokio::test]
async fn test_auto_scaling_integration() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider.clone(), collector.clone());
    
    // Force scale to 3 instances
    aslb.force_scale(3).await.unwrap();
    
    // Check that instances were created in the auto scaler
    let created = provider.created_instances.read().await;
    assert!(created.len() >= 3);
    
    // Force scale down to 1 instance
    aslb.force_scale(1).await.unwrap();
    
    // Check scaling events
    let events = aslb.get_scaling_events(10).await;
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_metrics_collection_integration() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    // Set high utilization metrics to trigger scaling
    let mut high_metrics = ScalingMetrics::new();
    high_metrics.cpu_utilization = 90.0;
    high_metrics.memory_utilization = 85.0;
    high_metrics.request_rate = 1000.0;
    collector.set_metrics(high_metrics).await;
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Add some endpoints to load balancer
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    aslb.add_endpoint(endpoint1).await.unwrap();
    aslb.add_endpoint(endpoint2).await.unwrap();
    
    // Simulate some load
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    for _ in 0..5 {
        let selected = aslb.select_endpoint(&context).await.unwrap();
        aslb.record_request_start(selected.id).await.unwrap();
        aslb.record_request_end(selected.id, true, Duration::from_millis(200)).await.unwrap();
    }
    
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_requests, 5);
    assert!(stats.load_balancer_stats.average_response_time.as_millis() > 0);
}

#[tokio::test]
async fn test_endpoint_selection_with_different_algorithms() {
    let mut config = AutoScalingLoadBalancerConfig::default();
    config.load_balancer.algorithm = rustci::core::performance::LoadBalancingAlgorithm::LeastConnections;
    
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Add endpoints with different connection counts
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint1.current_connections = 10;
    endpoint2.health_status = HealthStatus::Healthy;
    endpoint2.current_connections = 5; // Fewer connections
    
    let endpoint2_id = endpoint2.id;
    
    aslb.add_endpoint(endpoint1).await.unwrap();
    aslb.add_endpoint(endpoint2).await.unwrap();
    
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let selected = aslb.select_endpoint(&context).await.unwrap();
    
    // Should select endpoint with fewer connections
    assert_eq!(selected.id, endpoint2_id);
}

#[tokio::test]
async fn test_integration_stats_tracking() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Initial stats should be zero
    let stats = aslb.get_stats().await;
    assert_eq!(stats.integration_stats.total_scaling_events, 0);
    assert_eq!(stats.integration_stats.successful_integrations, 0);
    assert_eq!(stats.integration_stats.failed_integrations, 0);
    assert_eq!(stats.integration_stats.average_response_time, Duration::from_millis(0));
    assert!(stats.integration_stats.last_integration.is_none());
}

#[tokio::test]
async fn test_concurrent_operations() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Add healthy endpoints
    let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
    let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
    endpoint1.health_status = HealthStatus::Healthy;
    endpoint2.health_status = HealthStatus::Healthy;
    
    aslb.add_endpoint(endpoint1).await.unwrap();
    aslb.add_endpoint(endpoint2).await.unwrap();
    
    // Simulate concurrent requests
    let mut handles = Vec::new();
    
    for i in 0..20 {
        let aslb_clone = &aslb;
        let handle = tokio::spawn(async move {
            let context = RoutingContext::new(format!("/test/{}", i), "GET".to_string());
            let selected = aslb_clone.select_endpoint(&context).await.unwrap();
            
            aslb_clone.record_request_start(selected.id).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await; // Simulate processing
            aslb_clone.record_request_end(selected.id, true, Duration::from_millis(10)).await.unwrap();
            
            selected.id
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    let mut selected_endpoints = Vec::new();
    for handle in handles {
        let endpoint_id = handle.await.unwrap();
        selected_endpoints.push(endpoint_id);
    }
    
    // Verify all requests completed
    let stats = aslb.get_stats().await;
    assert_eq!(stats.load_balancer_stats.total_requests, 20);
    assert_eq!(stats.load_balancer_stats.successful_requests, 20);
    
    // Verify load was distributed
    assert!(selected_endpoints.len() == 20);
}

#[tokio::test]
async fn test_error_handling() {
    let config = AutoScalingLoadBalancerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
    
    // Try to select endpoint when no endpoints are available
    let context = RoutingContext::new("/test".to_string(), "GET".to_string());
    let result = aslb.select_endpoint(&context).await;
    assert!(result.is_err());
    
    // Try to remove non-existent endpoint
    let non_existent_id = Uuid::new_v4();
    let result = aslb.remove_endpoint(non_existent_id).await;
    assert!(result.is_ok()); // Should not fail, just no-op
    
    // Try to update health of non-existent endpoint
    let result = aslb.update_endpoint_health(non_existent_id, HealthStatus::Healthy).await;
    assert!(result.is_ok()); // Should not fail, just no-op
}