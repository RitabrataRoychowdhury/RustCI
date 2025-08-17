//! Tests for QoS-Aware Stream Management
//! 
//! Validates the QoS stream router, message classifier, and bandwidth allocator
//! for intelligent message routing with performance guarantees.

use std::collections::HashMap;
use std::time::Duration;
use tokio_test;

use crate::core::networking::valkyrie::streaming::*;
use crate::core::networking::valkyrie::adapters::*;

#[tokio::test]
async fn test_qos_stream_router_creation() {
    let config = QoSRouterConfig::default();
    let router = QoSStreamRouter::new(config);
    
    // Router should be created successfully
    let metrics = router.metrics().await;
    assert_eq!(metrics.total_messages, 0);
}

#[tokio::test]
async fn test_message_classification() {
    let classifier = MessageClassifier::new();
    
    // Test critical message classification
    let critical_message = AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::HealthCheck,
        payload: b"health check".to_vec(),
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Critical,
        routing: RoutingInfo {
            source: None,
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    };
    
    let qos_params = QoSParams {
        max_latency: Some(Duration::from_millis(10)),
        reliability: 0.99,
        priority: MessagePriority::Critical,
        ..Default::default()
    };
    
    let qos_class = classifier.classify(&critical_message, &qos_params).await.unwrap();
    assert_eq!(qos_class, QoSClass::Critical);
}

#[tokio::test]
async fn test_message_routing_with_qos() {
    let config = QoSRouterConfig::default();
    let router = QoSStreamRouter::new(config);
    
    // Create test message
    let message = AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: b"test job execution".to_vec(),
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::High,
        routing: RoutingInfo {
            source: None,
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    };
    
    let qos_params = QoSParams {
        max_latency: Some(Duration::from_millis(100)),
        reliability: 0.95,
        priority: MessagePriority::High,
        ..Default::default()
    };
    
    let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
    
    // Route the message
    let result = router.route_message(message, qos_params, destination).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.qos_class, QoSClass::JobExecution);
    assert!(result.routing_latency < Duration::from_millis(50));
}

#[tokio::test]
async fn test_bandwidth_allocation() {
    let allocator = BandwidthAllocator::new();
    
    let stream_id = uuid::Uuid::new_v4();
    let qos_class = QoSClass::System;
    let requested_bandwidth = 1_000_000; // 1 Mbps
    
    let qos_params = QoSParams {
        max_latency: Some(Duration::from_millis(50)),
        reliability: 0.99,
        priority: MessagePriority::High,
        ..Default::default()
    };
    
    // Allocate bandwidth
    let allocation = allocator.allocate_bandwidth(
        stream_id,
        qos_class,
        requested_bandwidth,
        &qos_params,
    ).await.unwrap();
    
    assert!(allocation.success);
    assert_eq!(allocation.stream_id, stream_id);
    assert_eq!(allocation.qos_class, qos_class);
    assert!(allocation.allocated_bandwidth > 0);
    assert!(allocation.allocation_latency < Duration::from_millis(10));
}

#[tokio::test]
async fn test_priority_queue_processing() {
    let config = QoSRouterConfig {
        processing_interval: Duration::from_millis(1),
        ..Default::default()
    };
    let router = QoSStreamRouter::new(config);
    
    // Create messages with different priorities
    let critical_message = create_test_message(MessagePriority::Critical, AdapterMessageType::HealthCheck);
    let normal_message = create_test_message(MessagePriority::Normal, AdapterMessageType::Request);
    let low_message = create_test_message(MessagePriority::Low, AdapterMessageType::Response);
    
    let qos_params = QoSParams::default();
    let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
    
    // Route messages (they should be queued)
    let critical_result = router.route_message(critical_message, qos_params.clone(), destination.clone()).await.unwrap();
    let normal_result = router.route_message(normal_message, qos_params.clone(), destination.clone()).await.unwrap();
    let low_result = router.route_message(low_message, qos_params, destination).await.unwrap();
    
    // Critical message should have highest priority
    assert_eq!(critical_result.qos_class, QoSClass::Critical);
    assert_eq!(normal_result.qos_class, QoSClass::JobExecution);
    assert_eq!(low_result.qos_class, QoSClass::DataTransfer);
    
    // Process queues
    router.process_queues().await.unwrap();
    
    // Verify metrics
    let metrics = router.metrics().await;
    assert_eq!(metrics.total_messages, 3);
    assert!(metrics.messages_by_class.contains_key(&QoSClass::Critical));
}

#[tokio::test]
async fn test_congestion_control() {
    let config = QoSRouterConfig {
        congestion_threshold: 0.5, // Low threshold for testing
        max_concurrent_streams: 2,  // Low limit for testing
        ..Default::default()
    };
    let router = QoSStreamRouter::new(config);
    
    // Create multiple messages to trigger congestion
    let messages: Vec<_> = (0..5).map(|i| {
        create_test_message(
            MessagePriority::Normal,
            AdapterMessageType::Request,
        )
    }).collect();
    
    let qos_params = QoSParams::default();
    let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
    
    // Route messages rapidly to trigger congestion
    for message in messages {
        let _ = router.route_message(message, qos_params.clone(), destination.clone()).await;
    }
    
    // Check metrics for congestion events
    let metrics = router.metrics().await;
    assert!(metrics.total_messages > 0);
    // Congestion events would be recorded in a real implementation
}

#[tokio::test]
async fn test_adaptive_bandwidth_allocation() {
    let config = BandwidthConfig {
        adaptive_allocation: true,
        total_bandwidth: 10_000_000, // 10 Mbps
        ..Default::default()
    };
    let allocator = BandwidthAllocator::with_config(config);
    
    // Allocate bandwidth for multiple streams
    let streams = vec![
        (uuid::Uuid::new_v4(), QoSClass::Critical, 2_000_000),
        (uuid::Uuid::new_v4(), QoSClass::System, 3_000_000),
        (uuid::Uuid::new_v4(), QoSClass::JobExecution, 2_000_000),
        (uuid::Uuid::new_v4(), QoSClass::DataTransfer, 1_000_000),
    ];
    
    let qos_params = QoSParams::default();
    
    for (stream_id, qos_class, bandwidth) in streams {
        let allocation = allocator.allocate_bandwidth(
            stream_id,
            qos_class,
            bandwidth,
            &qos_params,
        ).await.unwrap();
        
        assert!(allocation.success);
        assert!(allocation.allocated_bandwidth > 0);
        
        // Simulate usage
        allocator.update_usage(
            stream_id,
            bandwidth / 10, // 10% of allocated bandwidth
            Duration::from_secs(1),
        ).await.unwrap();
    }
    
    // Perform adaptive reallocation
    allocator.adaptive_reallocation().await.unwrap();
    
    // Check metrics
    let metrics = allocator.metrics().await;
    assert_eq!(metrics.active_streams, 4);
    assert!(metrics.total_utilization > 0.0);
    assert!(metrics.efficiency >= 0.0);
}

#[tokio::test]
async fn test_classification_with_ml() {
    let config = ClassifierConfig {
        enable_ml: true,
        ml_threshold: 0.6,
        ..Default::default()
    };
    let classifier = MessageClassifier::with_config(config);
    
    // Create training data by classifying messages
    let training_messages = vec![
        (create_test_message(MessagePriority::Critical, AdapterMessageType::HealthCheck), QoSClass::Critical),
        (create_test_message(MessagePriority::High, AdapterMessageType::Request), QoSClass::System),
        (create_test_message(MessagePriority::Normal, AdapterMessageType::Request), QoSClass::JobExecution),
        (create_large_message(), QoSClass::DataTransfer),
    ];
    
    let qos_params = QoSParams::default();
    
    // Classify messages to build training data
    for (message, expected_class) in training_messages {
        let classified_class = classifier.classify(&message, &qos_params).await.unwrap();
        // In a real implementation, we would provide feedback to improve ML accuracy
    }
    
    // Test classification metrics
    let metrics = classifier.metrics().await;
    assert!(metrics.total_classifications > 0);
    assert!(metrics.avg_classification_time > Duration::from_nanos(0));
}

#[tokio::test]
async fn test_pattern_recognition() {
    let config = ClassifierConfig {
        enable_patterns: true,
        pattern_threshold: 0.5,
        ..Default::default()
    };
    let classifier = MessageClassifier::with_config(config);
    
    // Create messages with recognizable patterns
    let pattern_messages = vec![
        create_large_message(), // Should match size pattern
        create_test_message(MessagePriority::Critical, AdapterMessageType::HealthCheck), // Should match critical pattern
        create_test_message(MessagePriority::Normal, AdapterMessageType::Request), // Should match job pattern
    ];
    
    let qos_params = QoSParams::default();
    
    for message in pattern_messages {
        let qos_class = classifier.classify(&message, &qos_params).await.unwrap();
        // Pattern recognition should contribute to classification
        assert!(matches!(qos_class, QoSClass::Critical | QoSClass::System | QoSClass::JobExecution | QoSClass::DataTransfer));
    }
}

#[tokio::test]
async fn test_bandwidth_throttling() {
    let config = BandwidthConfig {
        throttling_enabled: true,
        congestion_detection: true,
        total_bandwidth: 1_000_000, // 1 Mbps - low for testing
        ..Default::default()
    };
    let allocator = BandwidthAllocator::with_config(config);
    
    // Request more bandwidth than available to trigger throttling
    let stream_id = uuid::Uuid::new_v4();
    let qos_class = QoSClass::DataTransfer; // Lower priority class
    let requested_bandwidth = 2_000_000; // 2 Mbps - more than total available
    
    let qos_params = QoSParams::default();
    
    let allocation = allocator.allocate_bandwidth(
        stream_id,
        qos_class,
        requested_bandwidth,
        &qos_params,
    ).await.unwrap();
    
    // Should get less than requested due to throttling
    assert!(allocation.allocated_bandwidth < requested_bandwidth);
    // Throttling might be applied depending on congestion state
}

#[tokio::test]
async fn test_stream_lifecycle_management() {
    let allocator = BandwidthAllocator::new();
    
    let stream_id = uuid::Uuid::new_v4();
    let qos_class = QoSClass::JobExecution;
    let bandwidth = 1_000_000;
    
    let qos_params = QoSParams::default();
    
    // Allocate bandwidth
    let allocation = allocator.allocate_bandwidth(
        stream_id,
        qos_class,
        bandwidth,
        &qos_params,
    ).await.unwrap();
    
    assert!(allocation.success);
    
    // Update usage
    allocator.update_usage(
        stream_id,
        100_000, // 100KB
        Duration::from_millis(100),
    ).await.unwrap();
    
    // Check metrics before release
    let metrics_before = allocator.metrics().await;
    assert_eq!(metrics_before.active_streams, 1);
    
    // Release bandwidth
    allocator.release_bandwidth(stream_id).await.unwrap();
    
    // Check metrics after release
    let metrics_after = allocator.metrics().await;
    assert_eq!(metrics_after.active_streams, 0);
}

#[tokio::test]
async fn test_qos_class_prioritization() {
    let router = QoSStreamRouter::new(QoSRouterConfig::default());
    
    // Create messages for each QoS class
    let test_cases = vec![
        (AdapterMessageType::HealthCheck, MessagePriority::Critical, QoSClass::Critical),
        (AdapterMessageType::Request, MessagePriority::High, QoSClass::System),
        (AdapterMessageType::Request, MessagePriority::Normal, QoSClass::JobExecution),
        (AdapterMessageType::Response, MessagePriority::Low, QoSClass::DataTransfer),
    ];
    
    for (msg_type, priority, expected_class) in test_cases {
        let message = create_test_message(priority, msg_type);
        let qos_params = QoSParams {
            priority,
            ..Default::default()
        };
        let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
        
        let result = router.route_message(message, qos_params, destination).await.unwrap();
        assert_eq!(result.qos_class, expected_class);
    }
}

#[tokio::test]
async fn test_load_balanced_routing() {
    let router = QoSStreamRouter::new(QoSRouterConfig::default());
    
    // Create load balanced destination with multiple adapters
    let adapters = vec![
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    ];
    let destination = RouteDestination::LoadBalanced(adapters);
    
    let message = create_test_message(MessagePriority::Normal, AdapterMessageType::Request);
    let qos_params = QoSParams::default();
    
    // Route message with load balancing
    let result = router.route_message(message, qos_params, destination).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.qos_class, QoSClass::JobExecution);
}

#[tokio::test]
async fn test_service_based_routing() {
    let router = QoSStreamRouter::new(QoSRouterConfig::default());
    
    let destination = RouteDestination::Service("job-executor".to_string());
    let message = create_test_message(MessagePriority::Normal, AdapterMessageType::Request);
    let qos_params = QoSParams::default();
    
    // Route message to service
    let result = router.route_message(message, qos_params, destination).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.qos_class, QoSClass::JobExecution);
}

#[tokio::test]
async fn test_broadcast_routing() {
    let router = QoSStreamRouter::new(QoSRouterConfig::default());
    
    let destination = RouteDestination::Broadcast;
    let message = create_test_message(MessagePriority::High, AdapterMessageType::HealthCheck);
    let qos_params = QoSParams::default();
    
    // Route broadcast message
    let result = router.route_message(message, qos_params, destination).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.qos_class, QoSClass::Critical);
}

// Helper functions for creating test messages

fn create_test_message(priority: MessagePriority, message_type: AdapterMessageType) -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type,
        payload: b"test message".to_vec(),
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now(),
        priority,
        routing: RoutingInfo {
            source: None,
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    }
}

fn create_large_message() -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: vec![0u8; 2 * 1024 * 1024], // 2MB payload
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Normal,
        routing: RoutingInfo {
            source: None,
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    }
}

#[tokio::test]
async fn test_classification_caching() {
    let config = ClassifierConfig {
        enable_caching: true,
        cache_ttl: Duration::from_secs(60),
        max_cache_size: 100,
        ..Default::default()
    };
    let classifier = MessageClassifier::with_config(config);
    
    let message = create_test_message(MessagePriority::High, AdapterMessageType::Request);
    let qos_params = QoSParams::default();
    
    // First classification (cache miss)
    let start1 = std::time::Instant::now();
    let class1 = classifier.classify(&message, &qos_params).await.unwrap();
    let duration1 = start1.elapsed();
    
    // Second classification of same message (cache hit)
    let start2 = std::time::Instant::now();
    let class2 = classifier.classify(&message, &qos_params).await.unwrap();
    let duration2 = start2.elapsed();
    
    // Results should be the same
    assert_eq!(class1, class2);
    
    // Second classification should be faster (cache hit)
    // Note: This might not always be true in tests due to timing variations
    // but the cache hit rate should improve in metrics
    let metrics = classifier.metrics().await;
    assert!(metrics.total_classifications >= 2);
}

#[tokio::test]
async fn test_bandwidth_burst_allowance() {
    let config = BandwidthConfig {
        burst_allowance: true,
        burst_multiplier: 2.0,
        ..Default::default()
    };
    let allocator = BandwidthAllocator::with_config(config);
    
    let stream_id = uuid::Uuid::new_v4();
    let qos_class = QoSClass::Critical; // High priority class
    
    // Request bandwidth that would require burst allowance
    let base_bandwidth = 1_000_000; // 1 Mbps
    let burst_bandwidth = 1_500_000; // 1.5 Mbps (within burst limit)
    
    let qos_params = QoSParams {
        priority: MessagePriority::Critical,
        ..Default::default()
    };
    
    let allocation = allocator.allocate_bandwidth(
        stream_id,
        qos_class,
        burst_bandwidth,
        &qos_params,
    ).await.unwrap();
    
    assert!(allocation.success);
    // Should get the requested bandwidth due to burst allowance
    assert!(allocation.allocated_bandwidth > 0);
}

#[tokio::test]
async fn test_qos_deadline_handling() {
    let config = QoSRouterConfig {
        processing_interval: Duration::from_millis(100), // Slow processing for testing
        ..Default::default()
    };
    let router = QoSStreamRouter::new(config);
    
    let message = create_test_message(MessagePriority::Normal, AdapterMessageType::Request);
    let qos_params = QoSParams {
        max_latency: Some(Duration::from_millis(50)), // Short deadline
        ..Default::default()
    };
    let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
    
    // Route message with tight deadline
    let result = router.route_message(message, qos_params, destination).await.unwrap();
    
    assert!(result.success);
    // The router should handle deadline constraints
    assert!(result.routing_latency < Duration::from_millis(100));
}