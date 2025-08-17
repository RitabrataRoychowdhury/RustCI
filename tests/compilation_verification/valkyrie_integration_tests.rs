// Valkyrie integration compilation tests
// Verifies that Valkyrie components compile and integrate correctly

use std::sync::Arc;
use std::time::Duration;

// Test basic Valkyrie types compilation
#[tokio::test]
async fn test_valkyrie_types_compilation() {
    use RustAutoDevOps::core::networking::valkyrie::types::{
        ValkyrieMessage, MessageHeader, MessagePriority, MessageType, MessageFlags,
        ProtocolInfo, EndpointId, NodeId, CorrelationId, RoutingInfo, CompressionInfo,
        TraceContext
    };
    
    // Test that we can create basic Valkyrie types
    let node_id = NodeId::new();
    let endpoint_id = EndpointId::new(node_id);
    let correlation_id = CorrelationId::new();
    
    // Test TraceContext creation
    let trace_context = TraceContext::default();
    assert!(!trace_context.trace_id.is_empty());
    assert!(!trace_context.span_id.is_empty());
    
    // Test MessageHeader creation
    let header = MessageHeader {
        id: uuid::Uuid::new_v4(),
        source: endpoint_id,
        destination: None,
        protocol_info: ProtocolInfo::default(),
        message_type: MessageType::JobExecution,
        stream_id: None,
        flags: MessageFlags::default(),
        priority: MessagePriority::Normal,
        timestamp: chrono::Utc::now(),
        ttl: Some(Duration::from_secs(30)),
        correlation_id: Some(correlation_id),
        routing: RoutingInfo::default(),
        compression: CompressionInfo::default(),
        sequence_number: 1,
        ack_number: None,
    };
    
    // Test ValkyrieMessage creation
    let message = ValkyrieMessage {
        header,
        payload: vec![1, 2, 3, 4],
        signature: None,
        trace_context: Some(trace_context),
    };
    
    assert!(!message.payload.is_empty());
}

/// Test Valkyrie engine configuration
#[tokio::test]
async fn test_valkyrie_engine_config() {
    use RustAutoDevOps::core::networking::valkyrie::engine::ValkyrieConfig;
    
    let config = ValkyrieConfig {
        node_id: uuid::Uuid::new_v4(),
        listen_address: "127.0.0.1:8080".to_string(),
        max_connections: 1000,
        connection_timeout: Duration::from_secs(30),
        enable_metrics: true,
        enable_tracing: true,
    };
    
    assert_eq!(config.max_connections, 1000);
    assert!(config.enable_metrics);
    assert!(config.enable_tracing);
}

/// Test Valkyrie adapter types
#[tokio::test]
async fn test_valkyrie_adapter_types() {
    use RustAutoDevOps::infrastructure::runners::valkyrie_adapter::{
        ValkyrieJob, JobType, JobPriority, JobPayload, JobRequirements, JobMetadata
    };
    
    // Test job creation
    let job_metadata = JobMetadata {
        correlation_id: Some("test-correlation".to_string()),
        tags: std::collections::HashMap::new(),
        retry_count: 0,
        max_retries: 3,
    };
    
    let job_requirements = JobRequirements {
        cpu_cores: Some(2),
        memory_mb: Some(1024),
        disk_mb: Some(512),
        network_bandwidth_mbps: None,
        gpu_required: false,
        special_hardware: None,
    };
    
    let job = ValkyrieJob {
        id: uuid::Uuid::new_v4(),
        job_type: JobType::Build,
        priority: JobPriority::Normal,
        payload: JobPayload::Build {
            repository_url: "https://github.com/test/repo".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            build_config: serde_json::json!({"target": "release"}),
        },
        requirements: job_requirements,
        metadata: job_metadata,
        created_at: std::time::SystemTime::now(),
        deadline: None,
        routing_hints: None,
        trace_context: None,
    };
    
    assert_eq!(job.priority, JobPriority::Normal);
    assert!(matches!(job.job_type, JobType::Build));
}

/// Test that Valkyrie integration service compiles
#[tokio::test]
async fn test_valkyrie_integration_service_compilation() {
    use RustAutoDevOps::application::services::valkyrie_integration::ValkyrieIntegrationService;
    use RustAutoDevOps::config::valkyrie_integration::ValkyrieIntegrationConfig;
    
    let config = ValkyrieIntegrationConfig {
        enabled: true,
        valkyrie_listen_address: "127.0.0.1:9090".to_string(),
        max_connections: 1000,
        connection_timeout: Duration::from_secs(30),
        metrics_enabled: true,
        tracing_enabled: true,
        fallback_to_http: true,
        performance_monitoring_enabled: true,
        health_check_interval: Duration::from_secs(30),
    };
    
    // Test that we can create the service (this tests compilation)
    let service = ValkyrieIntegrationService::new(config);
    assert!(service.is_ok());
}

/// Test routing system compilation
#[tokio::test]
async fn test_routing_system_compilation() {
    use RustAutoDevOps::core::networking::valkyrie::routing::{
        SecurityLevel, RoutingContext, Route, RouteMetadata
    };
    
    // Test SecurityLevel
    let security_level = SecurityLevel::Internal;
    assert_eq!(security_level, SecurityLevel::Internal);
    
    // Test that we can create routing contexts
    let routing_context = RoutingContext {
        source: uuid::Uuid::new_v4(),
        destination: uuid::Uuid::new_v4(),
        message_type: "test".to_string(),
        priority: 1,
        security_context: Default::default(),
        routing_hints: Default::default(),
        deadline: None,
        created_at: std::time::SystemTime::now(),
    };
    
    assert_ne!(routing_context.source, routing_context.destination);
}

/// Test QoS system compilation
#[tokio::test]
async fn test_qos_system_compilation() {
    use RustAutoDevOps::core::networking::valkyrie::routing::qos::CongestionState;
    
    // Test CongestionState
    let congestion_state = CongestionState::Normal;
    assert_eq!(congestion_state, CongestionState::Normal);
    
    let mild_congestion = CongestionState::Mild;
    assert_eq!(mild_congestion, CongestionState::Mild);
}

/// Test streaming system compilation
#[tokio::test]
async fn test_streaming_system_compilation() {
    use RustAutoDevOps::core::networking::valkyrie::streaming::CongestionState;
    
    // Test streaming CongestionState
    let congestion_state = CongestionState {
        cwnd: 10,
        ssthresh: 65536,
        rtt: Duration::from_millis(50),
        bandwidth_estimate: 1000000,
        loss_rate: 0.01,
    };
    
    assert_eq!(congestion_state.cwnd, 10);
    assert_eq!(congestion_state.ssthresh, 65536);
}

/// Test that observability integration works with Valkyrie
#[tokio::test]
async fn test_valkyrie_observability_integration() {
    use RustAutoDevOps::core::networking::valkyrie::observability::adapter_system::{
        ObservabilityManager, ObservabilityConfig, LogLevel
    };
    
    // Create observability manager
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Test that we can use observability with Valkyrie operations
    manager.record_counter("valkyrie_messages_sent", 1, &[("type", "job_execution")]).await;
    manager.record_gauge("valkyrie_active_connections", 10.0, &[]).await;
    
    manager.log(LogLevel::Info, "Valkyrie message processed", &[
        ("message_id", "test-123"),
        ("source", "node-1"),
        ("destination", "node-2")
    ]).await;
    
    // Test span creation for Valkyrie operations
    let spans = manager.start_span("valkyrie_message_processing", None).await;
    assert!(!spans.is_empty());
    
    for span_id in spans {
        manager.add_attributes(span_id, &[
            ("valkyrie.message.type", "job_execution"),
            ("valkyrie.message.priority", "normal")
        ]).await;
        
        manager.add_event(span_id, "message_validated", &[]).await;
        manager.add_event(span_id, "message_routed", &[]).await;
        manager.add_event(span_id, "message_delivered", &[]).await;
        
        manager.end_span(span_id).await;
    }
    
    // Verify system health
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty());
}