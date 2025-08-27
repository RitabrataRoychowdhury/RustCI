//! QoS-Aware Stream Management Example
//! 
//! Demonstrates intelligent message routing with QoS guarantees, priority-based
//! classification, and adaptive bandwidth allocation for optimal performance.

use std::collections::HashMap;
use std::time::Duration;
use tokio;
use tracing::{info, warn, error};

use RustAutoDevOps::core::networking::valkyrie::streaming::*;
use RustAutoDevOps::core::networking::valkyrie::streaming::router::{QoSRouterConfig, RouteDestination};
use RustAutoDevOps::core::networking::valkyrie::streaming::classifier::ClassifierConfig;
use RustAutoDevOps::core::networking::valkyrie::streaming::bandwidth::BandwidthConfig;
use RustAutoDevOps::core::networking::valkyrie::adapters::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("🚀 Starting QoS-Aware Stream Management Example");
    
    // Initialize components
    let (router, classifier, allocator) = initialize_qos_components().await?;
    
    // Demonstrate different QoS scenarios
    demo_critical_message_routing(&router).await?;
    demo_job_execution_routing(&router).await?;
    demo_large_data_transfer(&router, &allocator).await?;
    demo_adaptive_bandwidth_management(&allocator).await?;
    demo_congestion_handling(&router, &allocator).await?;
    demo_ml_classification(&classifier).await?;
    demo_pattern_recognition(&classifier).await?;
    demo_load_balancing(&router).await?;
    
    // Show final metrics
    show_final_metrics(&router, &classifier, &allocator).await?;
    
    info!("✅ QoS-Aware Stream Management Example completed successfully");
    Ok(())
}

/// Initialize QoS components with optimal configurations
async fn initialize_qos_components() -> Result<(QoSStreamRouter, MessageClassifier, BandwidthAllocator), Box<dyn std::error::Error>> {
    info!("🔧 Initializing QoS components...");
    
    // Configure QoS router for optimal performance
    let router_config = QoSRouterConfig {
        max_queue_size: 10000,
        processing_interval: Duration::from_millis(5),
        congestion_threshold: 0.8,
        max_concurrent_streams: 1000,
        health_check_interval: Duration::from_secs(30),
        metrics_interval: Duration::from_secs(10),
        adaptive_routing: true,
        load_balancing: true,
    };
    let router = QoSStreamRouter::new(router_config);
    
    // Configure message classifier with ML and pattern recognition
    let classifier_config = ClassifierConfig {
        enable_ml: true,
        enable_patterns: true,
        enable_caching: true,
        cache_ttl: Duration::from_secs(300),
        max_cache_size: 10000,
        learning_rate: 0.01,
        pattern_threshold: 0.7,
        ml_threshold: 0.8,
        adaptive_learning: true,
    };
    let classifier = MessageClassifier::with_config(classifier_config);
    
    // Configure bandwidth allocator with adaptive features
    let bandwidth_config = BandwidthConfig {
        total_bandwidth: 1_000_000_000, // 1 Gbps
        adaptive_allocation: true,
        burst_allowance: true,
        burst_multiplier: 2.0,
        tracking_interval: Duration::from_secs(1),
        throttling_enabled: true,
        congestion_detection: true,
        ..Default::default()
    };
    let allocator = BandwidthAllocator::with_config(bandwidth_config);
    
    info!("  ✅ QoS router initialized");
    info!("  ✅ Message classifier initialized with ML and patterns");
    info!("  ✅ Bandwidth allocator initialized with adaptive features");
    
    Ok((router, classifier, allocator))
}

/// Demonstrate critical message routing with highest priority
async fn demo_critical_message_routing(router: &QoSStreamRouter) -> Result<(), Box<dyn std::error::Error>> {
    info!("🚨 Demo: Critical Message Routing");
    
    // Create critical health check message
    let critical_message = AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Heartbeat,
        payload: create_health_check_payload(),
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("service".to_string(), "control-plane".to_string());
            metadata.insert("urgency".to_string(), "critical".to_string());
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Critical,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Broadcast,
            hints: HashMap::new(),
        },
    };
    
    let qos_params = QoSParams {
        max_latency: Duration::from_millis(10), // Ultra-low latency requirement
        reliability: 0.999, // Very high reliability
        priority: MessagePriority::Critical,
        ..Default::default()
    };
    
    let destination = RouteDestination::Broadcast;
    
    let start_time = std::time::Instant::now();
    let result = router.route_message(critical_message, qos_params, destination).await?;
    let total_latency = start_time.elapsed();
    
    info!("  📊 Critical message routing results:");
    info!("    QoS Class: {}", result.qos_class);
    info!("    Success: {}", result.success);
    info!("    Routing Latency: {:?}", result.routing_latency);
    info!("    Total Latency: {:?}", total_latency);
    info!("    Queue Position: {}", result.queue_position);
    
    assert_eq!(result.qos_class, QoSClass::Critical);
    assert!(result.success);
    assert!(total_latency < Duration::from_millis(50));
    
    Ok(())
}

/// Demonstrate job execution message routing
async fn demo_job_execution_routing(router: &QoSStreamRouter) -> Result<(), Box<dyn std::error::Error>> {
    info!("⚙️  Demo: Job Execution Message Routing");
    
    // Create job execution messages with different priorities
    let job_messages = vec![
        create_job_message("build-pipeline", MessagePriority::High),
        create_job_message("test-suite", MessagePriority::Normal),
        create_job_message("deploy-staging", MessagePriority::High),
        create_job_message("cleanup-logs", MessagePriority::Low),
    ];
    
    let mut results = Vec::new();
    
    for (i, message) in job_messages.into_iter().enumerate() {
        let qos_params = QoSParams {
            max_latency: Duration::from_millis(100),
            reliability: 0.95,
            priority: message.priority,
            ..Default::default()
        };
        
        let destination = RouteDestination::Service(format!("job-executor-{}", i % 3));
        
        let result = router.route_message(message, qos_params, destination).await?;
        results.push(result);
    }
    
    info!("  📊 Job execution routing results:");
    for (i, result) in results.iter().enumerate() {
        info!("    Job {}: QoS={}, Success={}, Latency={:?}", 
              i, result.qos_class, result.success, result.routing_latency);
    }
    
    // All job messages should be classified as JobExecution
    for result in &results {
        assert_eq!(result.qos_class, QoSClass::JobExecution);
        assert!(result.success);
    }
    
    Ok(())
}

/// Demonstrate large data transfer with bandwidth management
async fn demo_large_data_transfer(
    router: &QoSStreamRouter,
    allocator: &BandwidthAllocator,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📦 Demo: Large Data Transfer with Bandwidth Management");
    
    // Create large data transfer message
    let large_message = AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: vec![0u8; 10 * 1024 * 1024], // 10MB payload
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("transfer_type".to_string(), "artifact".to_string());
            metadata.insert("compression".to_string(), "gzip".to_string());
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Normal,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    };
    
    // Route the large message
    let qos_params = QoSParams {
        max_latency: Duration::from_secs(30), // More tolerant for large transfers
        reliability: 0.99,
        priority: MessagePriority::Normal,
        ..Default::default()
    };
    
    let destination = RouteDestination::Direct(uuid::Uuid::new_v4());
    let routing_result = router.route_message(large_message, qos_params.clone(), destination).await?;
    
    // Allocate bandwidth for the transfer
    let stream_id = uuid::Uuid::new_v4();
    let requested_bandwidth = 50_000_000; // 50 Mbps for large transfer
    
    let bandwidth_result = allocator.allocate_bandwidth(
        stream_id,
        routing_result.qos_class,
        requested_bandwidth,
        &qos_params,
    ).await?;
    
    info!("  📊 Large data transfer results:");
    info!("    Message Size: 10 MB");
    info!("    QoS Class: {}", routing_result.qos_class);
    info!("    Routing Success: {}", routing_result.success);
    info!("    Bandwidth Allocated: {} Mbps", bandwidth_result.allocated_bandwidth / 1_000_000);
    info!("    Allocation Success: {}", bandwidth_result.success);
    info!("    Throttling Applied: {}", bandwidth_result.throttling_applied);
    
    // Simulate data transfer progress
    let chunk_size = 1_000_000; // 1MB chunks
    let total_chunks = 10;
    
    for chunk in 0..total_chunks {
        // Simulate chunk transfer
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Update bandwidth usage
        allocator.update_usage(
            stream_id,
            chunk_size,
            Duration::from_millis(100),
        ).await?;
        
        if chunk % 3 == 0 {
            info!("    Progress: {}/10 chunks transferred", chunk + 1);
        }
    }
    
    // Release bandwidth after transfer
    allocator.release_bandwidth(stream_id).await?;
    
    assert_eq!(routing_result.qos_class, QoSClass::DataTransfer);
    assert!(bandwidth_result.success);
    
    Ok(())
}

/// Demonstrate adaptive bandwidth management
async fn demo_adaptive_bandwidth_management(allocator: &BandwidthAllocator) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Demo: Adaptive Bandwidth Management");
    
    // Create multiple streams with different usage patterns
    let streams = vec![
        (uuid::Uuid::new_v4(), QoSClass::Critical, 10_000_000, "steady"),
        (uuid::Uuid::new_v4(), QoSClass::System, 20_000_000, "bursty"),
        (uuid::Uuid::new_v4(), QoSClass::JobExecution, 30_000_000, "growing"),
        (uuid::Uuid::new_v4(), QoSClass::DataTransfer, 40_000_000, "declining"),
    ];
    
    let qos_params = QoSParams::default();
    
    // Allocate bandwidth for all streams
    for (stream_id, qos_class, bandwidth, pattern) in &streams {
        let allocation = allocator.allocate_bandwidth(
            *stream_id,
            *qos_class,
            *bandwidth,
            &qos_params,
        ).await?;
        
        info!("  📊 Stream {} ({}): {} Mbps allocated", 
              stream_id, pattern, allocation.allocated_bandwidth / 1_000_000);
    }
    
    // Simulate different usage patterns over time
    for round in 0..5 {
        info!("  🔄 Adaptation round {}", round + 1);
        
        for (stream_id, qos_class, base_bandwidth, pattern) in &streams {
            let usage = match *pattern {
                "steady" => *base_bandwidth / 2,
                "bursty" => if round % 2 == 0 { *base_bandwidth } else { *base_bandwidth / 4 },
                "growing" => (*base_bandwidth / 4) * (round + 1) as u64,
                "declining" => (*base_bandwidth / 2) / (round + 1) as u64,
                _ => *base_bandwidth / 2,
            };
            
            allocator.update_usage(
                *stream_id,
                usage / 10, // Convert to bytes per update
                Duration::from_millis(100),
            ).await?;
        }
        
        // Perform adaptive reallocation
        allocator.adaptive_reallocation().await?;
        
        // Show current metrics
        let metrics = allocator.metrics().await;
        info!("    Total Utilization: {:.1}%", metrics.total_utilization * 100.0);
        info!("    Efficiency: {:.1}%", metrics.efficiency * 100.0);
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Clean up streams
    for (stream_id, _, _, _) in &streams {
        allocator.release_bandwidth(*stream_id).await?;
    }
    
    Ok(())
}

/// Demonstrate congestion handling
async fn demo_congestion_handling(
    router: &QoSStreamRouter,
    allocator: &BandwidthAllocator,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🚦 Demo: Congestion Handling");
    
    // Create high load to trigger congestion
    let high_load_messages = (0..20).map(|i| {
        create_job_message(&format!("high-load-job-{}", i), MessagePriority::Normal)
    }).collect::<Vec<_>>();
    
    let qos_params = QoSParams {
        max_latency: Duration::from_millis(50),
        reliability: 0.95,
        priority: MessagePriority::Normal,
        ..Default::default()
    };
    
    // Route messages rapidly to create congestion
    let mut routing_results = Vec::new();
    let start_time = std::time::Instant::now();
    
    for message in high_load_messages {
        let destination = RouteDestination::LoadBalanced(vec![
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        ]);
        
        let result = router.route_message(message, qos_params.clone(), destination).await?;
        routing_results.push(result);
    }
    
    let total_routing_time = start_time.elapsed();
    
    // Allocate bandwidth for multiple streams simultaneously
    let mut bandwidth_results = Vec::new();
    for i in 0..10 {
        let stream_id = uuid::Uuid::new_v4();
        let allocation = allocator.allocate_bandwidth(
            stream_id,
            QoSClass::JobExecution,
            50_000_000, // 50 Mbps each
            &qos_params,
        ).await?;
        bandwidth_results.push((stream_id, allocation));
    }
    
    info!("  📊 Congestion handling results:");
    info!("    Messages routed: {}", routing_results.len());
    info!("    Total routing time: {:?}", total_routing_time);
    info!("    Average routing latency: {:?}", 
          total_routing_time / routing_results.len() as u32);
    
    let successful_routes = routing_results.iter().filter(|r| r.success).count();
    info!("    Successful routes: {}/{}", successful_routes, routing_results.len());
    
    let successful_allocations = bandwidth_results.iter().filter(|(_, a)| a.success).count();
    info!("    Successful bandwidth allocations: {}/{}", successful_allocations, bandwidth_results.len());
    
    let throttled_allocations = bandwidth_results.iter().filter(|(_, a)| a.throttling_applied).count();
    info!("    Throttled allocations: {}", throttled_allocations);
    
    // Show router and allocator metrics
    let router_metrics = router.metrics().await;
    let allocator_metrics = allocator.metrics().await;
    
    info!("    Router congestion events: {}", router_metrics.congestion_events);
    info!("    Allocator throttling events: {}", allocator_metrics.throttling_events);
    
    // Clean up
    for (stream_id, allocation) in bandwidth_results {
        if allocation.success {
            allocator.release_bandwidth(stream_id).await?;
        }
    }
    
    Ok(())
}

/// Demonstrate ML-based classification
async fn demo_ml_classification(classifier: &MessageClassifier) -> Result<(), Box<dyn std::error::Error>> {
    info!("🤖 Demo: ML-Based Message Classification");
    
    // Create diverse training messages
    let training_messages = vec![
        (create_health_check_message(), QoSClass::Critical, "health check"),
        (create_system_message(), QoSClass::System, "system management"),
        (create_job_message("ml-training", MessagePriority::Normal), QoSClass::JobExecution, "job execution"),
        (create_large_data_message(), QoSClass::DataTransfer, "large data"),
        (create_log_message(), QoSClass::LogsMetrics, "logs/metrics"),
    ];
    
    let qos_params = QoSParams::default();
    
    // Classify messages to build ML training data
    info!("  🎯 Training ML classifier...");
    for (message, expected_class, description) in &training_messages {
        let classified_class = classifier.classify(message, &qos_params).await?;
        info!("    {} -> Expected: {}, Classified: {}", 
              description, expected_class, classified_class);
    }
    
    // Test classification on new messages
    info!("  🧪 Testing ML classification on new messages...");
    let test_messages = vec![
        (create_health_check_message(), "new health check"),
        (create_job_message("new-deployment", MessagePriority::High), "new job"),
        (create_large_data_message(), "new large transfer"),
    ];
    
    for (message, description) in test_messages {
        let start_time = std::time::Instant::now();
        let classified_class = classifier.classify(&message, &qos_params).await?;
        let classification_time = start_time.elapsed();
        
        info!("    {} -> {}, Time: {:?}", description, classified_class, classification_time);
    }
    
    // Show classification metrics
    let metrics = classifier.metrics().await;
    info!("  📊 ML Classification metrics:");
    info!("    Total classifications: {}", metrics.total_classifications);
    info!("    Average classification time: {:?}", metrics.avg_classification_time);
    info!("    Cache hit rate: {:.1}%", metrics.cache_hit_rate * 100.0);
    info!("    ML accuracy: {:.1}%", metrics.ml_accuracy * 100.0);
    
    Ok(())
}

/// Demonstrate pattern recognition
async fn demo_pattern_recognition(classifier: &MessageClassifier) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔍 Demo: Pattern Recognition Classification");
    
    // Create messages with recognizable patterns
    let pattern_messages = vec![
        (create_repeated_health_checks(), "repeated health checks"),
        (create_batch_job_messages(), "batch job pattern"),
        (create_log_burst_messages(), "log burst pattern"),
        (create_large_file_sequence(), "large file sequence"),
    ];
    
    let qos_params = QoSParams::default();
    
    for (messages, pattern_name) in pattern_messages {
        info!("  🔍 Testing pattern: {}", pattern_name);
        
        let mut classifications = Vec::new();
        for message in messages {
            let classified_class = classifier.classify(&message, &qos_params).await?;
            classifications.push(classified_class);
        }
        
        // Analyze pattern consistency
        let unique_classes: std::collections::HashSet<_> = classifications.iter().collect();
        info!("    Pattern consistency: {} unique classes from {} messages", 
              unique_classes.len(), classifications.len());
        
        if unique_classes.len() == 1 {
            info!("    ✅ Consistent classification: {:?}", classifications[0]);
        } else {
            info!("    ⚠️  Mixed classifications: {:?}", unique_classes);
        }
    }
    
    Ok(())
}

/// Demonstrate load balancing across multiple adapters
async fn demo_load_balancing(router: &QoSStreamRouter) -> Result<(), Box<dyn std::error::Error>> {
    info!("⚖️  Demo: Load Balancing Across Adapters");
    
    // Create multiple adapter endpoints
    let adapters = vec![
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    ];
    
    info!("  📡 Available adapters: {}", adapters.len());
    
    // Route multiple messages with load balancing
    let messages = (0..12).map(|i| {
        create_job_message(&format!("load-balanced-job-{}", i), MessagePriority::Normal)
    }).collect::<Vec<_>>();
    
    let qos_params = QoSParams::default();
    let destination = RouteDestination::LoadBalanced(adapters.clone());
    
    let mut results = Vec::new();
    for message in messages {
        let result = router.route_message(message, qos_params.clone(), destination.clone()).await?;
        results.push(result);
    }
    
    info!("  📊 Load balancing results:");
    info!("    Messages routed: {}", results.len());
    info!("    Successful routes: {}", results.iter().filter(|r| r.success).count());
    
    let avg_latency = results.iter()
        .map(|r| r.routing_latency.as_nanos())
        .sum::<u128>() / results.len() as u128;
    info!("    Average routing latency: {:?}", Duration::from_nanos(avg_latency as u64));
    
    // In a real implementation, we would track which adapter each message was routed to
    // and verify even distribution
    
    Ok(())
}

/// Show final metrics from all components
async fn show_final_metrics(
    router: &QoSStreamRouter,
    classifier: &MessageClassifier,
    allocator: &BandwidthAllocator,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Final QoS System Metrics");
    
    // Router metrics
    let router_metrics = router.metrics().await;
    info!("  🚦 Router Metrics:");
    info!("    Total messages routed: {}", router_metrics.total_messages);
    info!("    Average routing latency: {:?}", router_metrics.avg_routing_latency);
    info!("    Congestion events: {}", router_metrics.congestion_events);
    info!("    Bandwidth utilization: {:.1}%", router_metrics.bandwidth_utilization * 100.0);
    
    for (qos_class, count) in &router_metrics.messages_by_class {
        info!("    {} messages: {}", qos_class, count);
    }
    
    // Classifier metrics
    let classifier_metrics = classifier.metrics().await;
    info!("  🎯 Classifier Metrics:");
    info!("    Total classifications: {}", classifier_metrics.total_classifications);
    info!("    Average classification time: {:?}", classifier_metrics.avg_classification_time);
    info!("    Cache hit rate: {:.1}%", classifier_metrics.cache_hit_rate * 100.0);
    info!("    ML accuracy: {:.1}%", classifier_metrics.ml_accuracy * 100.0);
    info!("    Pattern accuracy: {:.1}%", classifier_metrics.pattern_accuracy * 100.0);
    
    // Allocator metrics
    let allocator_metrics = allocator.metrics().await;
    info!("  📊 Bandwidth Allocator Metrics:");
    info!("    Total utilization: {:.1}%", allocator_metrics.total_utilization * 100.0);
    info!("    Active streams: {}", allocator_metrics.active_streams);
    info!("    Throttling events: {}", allocator_metrics.throttling_events);
    info!("    Congestion events: {}", allocator_metrics.congestion_events);
    info!("    Efficiency: {:.1}%", allocator_metrics.efficiency * 100.0);
    info!("    Average allocation latency: {:?}", allocator_metrics.avg_allocation_latency);
    
    for (qos_class, utilization) in &allocator_metrics.class_utilization {
        info!("    {} utilization: {:.1}%", qos_class, utilization * 100.0);
    }
    
    Ok(())
}

// Helper functions for creating test messages

fn create_health_check_payload() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "type": "health_check",
        "timestamp": chrono::Utc::now(),
        "service": "control-plane",
        "status": "checking"
    })).unwrap()
}

fn create_job_message(job_name: &str, priority: MessagePriority) -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: serde_json::to_vec(&serde_json::json!({
            "job_name": job_name,
            "action": "execute",
            "timestamp": chrono::Utc::now()
        })).unwrap(),
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("job_type".to_string(), "pipeline".to_string());
            metadata.insert("priority".to_string(), format!("{:?}", priority));
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Service("job-executor".to_string()),
            hints: HashMap::new(),
        },
    }
}

fn create_health_check_message() -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Heartbeat,
        payload: create_health_check_payload(),
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Critical,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Broadcast,
            hints: HashMap::new(),
        },
    }
}

fn create_system_message() -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: serde_json::to_vec(&serde_json::json!({
            "type": "system_management",
            "action": "update_config",
            "timestamp": chrono::Utc::now()
        })).unwrap(),
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("system".to_string(), "true".to_string());
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::High,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Service("system-manager".to_string()),
            hints: HashMap::new(),
        },
    }
}

fn create_large_data_message() -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Request,
        payload: vec![0u8; 5 * 1024 * 1024], // 5MB payload
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("data_type".to_string(), "artifact".to_string());
            metadata.insert("size".to_string(), "5MB".to_string());
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Normal,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Direct(uuid::Uuid::new_v4()),
            hints: HashMap::new(),
        },
    }
}

fn create_log_message() -> AdapterMessage {
    AdapterMessage {
        id: uuid::Uuid::new_v4(),
        message_type: AdapterMessageType::Event,
        payload: serde_json::to_vec(&serde_json::json!({
            "level": "info",
            "message": "Application started successfully",
            "timestamp": chrono::Utc::now(),
            "service": "web-server"
        })).unwrap(),
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("log_level".to_string(), "info".to_string());
            metadata
        },
        timestamp: chrono::Utc::now(),
        priority: MessagePriority::Low,
        routing: RoutingInfo {
            source: Some(uuid::Uuid::new_v4()),
            destination: DestinationType::Service("log-aggregator".to_string()),
            hints: HashMap::new(),
        },
    }
}

fn create_repeated_health_checks() -> Vec<AdapterMessage> {
    (0..5).map(|_| create_health_check_message()).collect()
}

fn create_batch_job_messages() -> Vec<AdapterMessage> {
    (0..3).map(|i| create_job_message(&format!("batch-job-{}", i), MessagePriority::Normal)).collect()
}

fn create_log_burst_messages() -> Vec<AdapterMessage> {
    (0..10).map(|_| create_log_message()).collect()
}

fn create_large_file_sequence() -> Vec<AdapterMessage> {
    (0..3).map(|_| create_large_data_message()).collect()
}