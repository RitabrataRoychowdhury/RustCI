// Valkyrie Protocol External Observability Integration Tests

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig, MetricValue, LogLevel,
    ExternalObservabilityIntegration, ExternalObservabilityConfig, IntegrationStatus,
};

#[tokio::test]
async fn test_external_observability_integration_lifecycle() {
    let external_config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        prometheus_push_gateway: Some("http://localhost:9091".to_string()),
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        jaeger_enabled: true,
        jaeger_endpoint: "http://localhost:14268".to_string(),
        grafana_enabled: true,
        grafana_endpoint: "http://localhost:3000".to_string(),
        grafana_api_key: Some("test-api-key".to_string()),
        export_interval_seconds: 5,
        batch_size: 100,
    };

    let mut integration = ExternalObservabilityIntegration::new(external_config);

    // Start external integrations
    assert!(integration.start().await.is_ok());

    // Check status
    let status = integration.status().await;
    assert!(status.prometheus_enabled);
    assert!(status.opentelemetry_enabled);
    assert!(status.jaeger_enabled);
    assert!(status.grafana_enabled);

    // Stop external integrations
    assert!(integration.stop().await.is_ok());
}

#[tokio::test]
async fn test_observability_manager_with_external_integrations() {
    let external_config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        jaeger_enabled: false, // Disable for this test
        grafana_enabled: false, // Disable for this test
        ..Default::default()
    };

    let config = ObservabilityConfig {
        metrics_enabled: true,
        logging_enabled: true,
        health_enabled: true,
        dashboard_enabled: true,
        external_config: Some(external_config),
        ..Default::default()
    };

    let mut manager = ObservabilityManager::new(config);

    // Start the observability manager with external integrations
    assert!(manager.start().await.is_ok());

    // Verify external integrations are available
    assert!(manager.external().is_some());

    let external = manager.external().unwrap();
    let status = external.status().await;
    assert!(status.prometheus_enabled);
    assert!(status.opentelemetry_enabled);
    assert!(!status.jaeger_enabled);
    assert!(!status.grafana_enabled);

    // Stop the observability manager
    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_prometheus_integration() {
    let config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        prometheus_push_gateway: Some("http://localhost:9091".to_string()),
        export_interval_seconds: 1,
        ..Default::default()
    };

    let integration = ExternalObservabilityIntegration::new(config);

    // Test metrics export (would normally require actual Prometheus instance)
    let metrics_collector = rustci::core::networking::valkyrie::observability::MetricsCollector::new(3600);
    
    // Record some test metrics
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "prometheus".to_string());
    
    assert!(metrics_collector.increment_counter("test_counter", labels.clone()).await.is_ok());
    assert!(metrics_collector.set_gauge("test_gauge", 42.0, labels.clone()).await.is_ok());

    // Export metrics (this would fail without actual Prometheus, but tests the interface)
    let export_result = integration.export_metrics(&metrics_collector).await;
    // We expect this to work in the interface layer even if the actual export fails
    assert!(export_result.is_ok());
}

#[tokio::test]
async fn test_opentelemetry_integration() {
    let config = ExternalObservabilityConfig {
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        ..Default::default()
    };

    let integration = ExternalObservabilityIntegration::new(config);

    // Test metrics export
    let metrics_collector = rustci::core::networking::valkyrie::observability::MetricsCollector::new(3600);
    assert!(integration.export_metrics(&metrics_collector).await.is_ok());

    // Test trace export
    let correlation_id = rustci::core::networking::valkyrie::observability::CorrelationId::new();
    let spans = vec![];
    assert!(integration.export_traces(correlation_id, &spans).await.is_ok());

    // Test log export
    let logger = rustci::core::networking::valkyrie::observability::StructuredLogger::new(86400);
    assert!(integration.export_logs(&logger).await.is_ok());
}

#[tokio::test]
async fn test_jaeger_integration() {
    let config = ExternalObservabilityConfig {
        jaeger_enabled: true,
        jaeger_endpoint: "http://localhost:14268".to_string(),
        ..Default::default()
    };

    let integration = ExternalObservabilityIntegration::new(config);

    // Test trace export
    let correlation_id = rustci::core::networking::valkyrie::observability::CorrelationId::new();
    let spans = vec![
        rustci::core::networking::valkyrie::observability::external::TraceSpan {
            span_id: "test-span-1".to_string(),
            parent_span_id: None,
            operation_name: "test_operation".to_string(),
            start_time: 1000000,
            end_time: Some(1001000),
            duration_us: Some(1000),
            tags: {
                let mut tags = HashMap::new();
                tags.insert("component".to_string(), "test".to_string());
                tags
            },
            logs: vec![],
        }
    ];

    assert!(integration.export_traces(correlation_id, &spans).await.is_ok());
}

#[tokio::test]
async fn test_grafana_integration() {
    let config = ExternalObservabilityConfig {
        grafana_enabled: true,
        grafana_endpoint: "http://localhost:3000".to_string(),
        grafana_api_key: Some("test-api-key".to_string()),
        ..Default::default()
    };

    let integration = ExternalObservabilityIntegration::new(config);

    // Test dashboard update
    assert!(integration.update_dashboards().await.is_ok());
}

#[tokio::test]
async fn test_integration_status_reporting() {
    let config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        opentelemetry_enabled: true,
        jaeger_enabled: false,
        grafana_enabled: false,
        ..Default::default()
    };

    let mut integration = ExternalObservabilityIntegration::new(config);

    // Initially stopped
    let status = integration.status().await;
    assert!(matches!(status.prometheus_status, IntegrationStatus::Stopped));
    assert!(matches!(status.opentelemetry_status, IntegrationStatus::Stopped));
    assert!(matches!(status.jaeger_status, IntegrationStatus::Disabled));
    assert!(matches!(status.grafana_status, IntegrationStatus::Disabled));

    // Start integrations
    assert!(integration.start().await.is_ok());

    let status = integration.status().await;
    // Note: In real implementation, these would be Connected if services are available
    // For tests, they might be Error status due to unavailable services
    assert!(status.prometheus_enabled);
    assert!(status.opentelemetry_enabled);
    assert!(!status.jaeger_enabled);
    assert!(!status.grafana_enabled);

    // Stop integrations
    assert!(integration.stop().await.is_ok());
}

#[tokio::test]
async fn test_end_to_end_external_observability() {
    // Create observability manager with external integrations
    let external_config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        export_interval_seconds: 1,
        ..Default::default()
    };

    let config = ObservabilityConfig {
        metrics_enabled: true,
        logging_enabled: true,
        health_enabled: true,
        dashboard_enabled: true,
        external_config: Some(external_config),
        ..Default::default()
    };

    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    // Record some metrics
    let metrics = manager.metrics();
    let mut labels = HashMap::new();
    labels.insert("test".to_string(), "end_to_end".to_string());

    for i in 1..=5 {
        assert!(metrics.increment_counter("e2e_test_counter", labels.clone()).await.is_ok());
        assert!(metrics.set_gauge("e2e_test_gauge", i as f64 * 10.0, labels.clone()).await.is_ok());
    }

    // Log some messages
    let logger = manager.logger();
    assert!(logger.info("End-to-end test started").await.is_ok());

    let mut context = HashMap::new();
    context.insert("test_type".to_string(), serde_json::Value::String("e2e".to_string()));
    assert!(logger.log(LogLevel::Info, "Test in progress", context).await.is_ok());

    // Create correlation tracking
    let correlation = manager.correlation();
    let correlation_id = correlation.start_correlation("e2e_test", None).await.unwrap();

    // Add some metadata
    assert!(correlation.add_metadata(
        correlation_id,
        "test_id",
        serde_json::Value::String("e2e_001".to_string())
    ).await.is_ok());

    // Start and end a span
    let span_id = correlation.start_span(
        correlation_id,
        "test_operation",
        "test_component",
        None
    ).await.unwrap();

    sleep(Duration::from_millis(10)).await;

    assert!(correlation.end_span(
        correlation_id,
        span_id,
        rustci::core::networking::valkyrie::observability::correlation::SpanStatus::Ok
    ).await.is_ok());

    assert!(correlation.end_correlation(
        correlation_id,
        rustci::core::networking::valkyrie::observability::correlation::CorrelationStatus::Completed
    ).await.is_ok());

    // Test external integration export
    if let Some(external) = manager.external() {
        // Export metrics to external systems
        assert!(external.export_metrics(&metrics).await.is_ok());

        // Export logs to external systems
        assert!(external.export_logs(&logger).await.is_ok());

        // Export traces to external systems
        let spans = vec![];
        assert!(external.export_traces(correlation_id, &spans).await.is_ok());

        // Update dashboards
        assert!(external.update_dashboards().await.is_ok());
    }

    // Verify final status
    let status = manager.status().await;
    assert!(status.metrics_enabled);
    assert!(status.logging_enabled);
    assert!(status.health_enabled);
    assert!(status.dashboard_enabled);
    assert!(status.metrics_count > 0);

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_external_observability_configuration_validation() {
    // Test default configuration
    let default_config = ExternalObservabilityConfig::default();
    assert!(!default_config.prometheus_enabled);
    assert!(!default_config.opentelemetry_enabled);
    assert!(!default_config.jaeger_enabled);
    assert!(!default_config.grafana_enabled);

    // Test custom configuration
    let custom_config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://custom-prometheus:9090".to_string(),
        prometheus_push_gateway: Some("http://custom-pushgateway:9091".to_string()),
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://custom-otel:4317".to_string(),
        jaeger_enabled: true,
        jaeger_endpoint: "http://custom-jaeger:14268".to_string(),
        grafana_enabled: true,
        grafana_endpoint: "http://custom-grafana:3000".to_string(),
        grafana_api_key: Some("custom-api-key".to_string()),
        export_interval_seconds: 30,
        batch_size: 500,
    };

    let integration = ExternalObservabilityIntegration::new(custom_config.clone());
    let status = integration.status().await;

    assert_eq!(status.prometheus_enabled, custom_config.prometheus_enabled);
    assert_eq!(status.opentelemetry_enabled, custom_config.opentelemetry_enabled);
    assert_eq!(status.jaeger_enabled, custom_config.jaeger_enabled);
    assert_eq!(status.grafana_enabled, custom_config.grafana_enabled);
}

#[tokio::test]
async fn test_external_observability_error_handling() {
    // Test with invalid endpoints (should not crash, but may report errors)
    let config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://invalid-endpoint:9999".to_string(),
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://invalid-endpoint:9999".to_string(),
        ..Default::default()
    };

    let mut integration = ExternalObservabilityIntegration::new(config);

    // Should start without error (actual connection errors handled gracefully)
    assert!(integration.start().await.is_ok());

    // Export operations should handle errors gracefully
    let metrics_collector = rustci::core::networking::valkyrie::observability::MetricsCollector::new(3600);
    let export_result = integration.export_metrics(&metrics_collector).await;
    // Should not panic, may return error or handle gracefully
    assert!(export_result.is_ok() || export_result.is_err());

    assert!(integration.stop().await.is_ok());
}