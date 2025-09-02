use rustci::core::observability::distributed_tracing::{
    DistributedTracing, TracingConfig, TraceAnalyzer, SpanProcessor,
    TraceStatus, ProcessedSpan, CompletedTrace, TraceSearchCriteria,
    ResourceUsage, TraceVisualization, PerformanceInsights,
};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[tokio::test]
async fn test_distributed_tracing_initialization() {
    let config = TracingConfig {
        service_name: "test-service".to_string(),
        service_version: "1.0.0".to_string(),
        enable_jaeger_exporter: false, // Disable for testing
        enable_otlp_exporter: false,
        enable_console_exporter: false,
        ..TracingConfig::default()
    };

    let tracing = DistributedTracing::new(config).await;
    assert!(tracing.is_ok());
}

#[tokio::test]
async fn test_trace_lifecycle_with_opentelemetry() {
    let config = TracingConfig {
        service_name: "test-service".to_string(),
        enable_jaeger_exporter: false,
        enable_otlp_exporter: false,
        enable_console_exporter: false,
        ..TracingConfig::default()
    };

    let tracing = DistributedTracing::new(config).await.unwrap();

    // Start a trace
    let correlation_id = tracing
        .start_trace("test_operation", "test_component", None)
        .await;

    // Add attributes and events
    let mut attributes = HashMap::new();
    attributes.insert("user_id".to_string(), "user123".to_string());
    attributes.insert("request_type".to_string(), "api_call".to_string());

    tracing
        .add_trace_attributes(correlation_id, attributes)
        .await;

    tracing
        .add_trace_event(
            correlation_id,
            "processing_started",
            crate::core::observability::distributed_tracing::TraceEventLevel::Info,
            HashMap::new(),
        )
        .await;

    // End the trace
    let result = tracing.end_trace(correlation_id, TraceStatus::Ok).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_child_span_creation() {
    let config = TracingConfig {
        service_name: "test-service".to_string(),
        enable_jaeger_exporter: false,
        enable_otlp_exporter: false,
        enable_console_exporter: false,
        ..TracingConfig::default()
    };

    let tracing = DistributedTracing::new(config).await.unwrap();

    // Start parent trace
    let parent_correlation_id = tracing
        .start_trace("parent_operation", "parent_component", None)
        .await;

    // Start child span
    let child_correlation_id = tracing
        .start_child_span(parent_correlation_id, "child_operation", "child_component")
        .await;

    assert!(child_correlation_id.is_ok());
    let child_id = child_correlation_id.unwrap();

    // End child span first
    tracing.end_trace(child_id, TraceStatus::Ok).await.unwrap();

    // End parent trace
    tracing.end_trace(parent_correlation_id, TraceStatus::Ok).await.unwrap();
}

#[tokio::test]
async fn test_trace_analyzer() {
    let config = TracingConfig::default();
    let analyzer = TraceAnalyzer::new(config);

    // Create a mock completed trace
    let trace = create_mock_completed_trace();

    // Analyze the trace
    let result = analyzer.analyze_trace(trace.clone()).await;
    assert!(result.is_ok());

    // Get performance metrics
    let metrics = analyzer.get_performance_metrics().await;
    assert_eq!(metrics.total_traces, 1);
    assert_eq!(metrics.successful_traces, 1);

    // Get completed traces
    let traces = analyzer.get_completed_traces(10).await;
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].trace_id, trace.trace_id);
}

#[tokio::test]
async fn test_span_processor() {
    let config = TracingConfig::default();
    let processor = SpanProcessor::new(config);

    // Create mock spans
    let root_span = create_mock_processed_span("root", None);
    let child_span = create_mock_processed_span("child", Some(root_span.span_id.clone()));

    // Process spans
    processor.process_span(child_span.clone()).await.unwrap();
    processor.process_span(root_span.clone()).await.unwrap();

    // Complete child span
    let result = processor.complete_span(&child_span.span_id).await.unwrap();
    assert!(result.is_none()); // Should not complete trace yet

    // Complete root span
    let result = processor.complete_span(&root_span.span_id).await.unwrap();
    assert!(result.is_some()); // Should complete trace

    let completed_trace = result.unwrap();
    assert_eq!(completed_trace.trace_id, root_span.trace_id);
    assert_eq!(completed_trace.span_count, 2);
}

#[tokio::test]
async fn test_trace_search() {
    let config = TracingConfig::default();
    let analyzer = TraceAnalyzer::new(config);

    // Create multiple traces with different characteristics
    let fast_trace = create_mock_completed_trace_with_duration(Duration::from_millis(100));
    let slow_trace = create_mock_completed_trace_with_duration(Duration::from_secs(5));
    let error_trace = create_mock_completed_trace_with_errors();

    analyzer.analyze_trace(fast_trace).await.unwrap();
    analyzer.analyze_trace(slow_trace).await.unwrap();
    analyzer.analyze_trace(error_trace).await.unwrap();

    // Search for slow traces
    let criteria = TraceSearchCriteria {
        operation: None,
        min_duration: Some(Duration::from_secs(1)),
        max_duration: None,
        has_errors: false,
    };

    let slow_traces = analyzer.search_traces(criteria).await;
    assert_eq!(slow_traces.len(), 1);

    // Search for error traces
    let criteria = TraceSearchCriteria {
        operation: None,
        min_duration: None,
        max_duration: None,
        has_errors: true,
    };

    let error_traces = analyzer.search_traces(criteria).await;
    assert_eq!(error_traces.len(), 1);
}

#[tokio::test]
async fn test_performance_insights() {
    let config = TracingConfig {
        service_name: "test-service".to_string(),
        enable_jaeger_exporter: false,
        enable_otlp_exporter: false,
        enable_console_exporter: false,
        ..TracingConfig::default()
    };

    let tracing = DistributedTracing::new(config).await.unwrap();

    // Simulate some traces with performance issues
    for i in 0..10 {
        let correlation_id = tracing
            .start_trace(&format!("operation_{}", i), "test_component", None)
            .await;

        // Simulate different performance characteristics
        let status = if i % 5 == 0 {
            TraceStatus::Error
        } else {
            TraceStatus::Ok
        };

        tracing.end_trace(correlation_id, status).await.unwrap();
    }

    // Get performance insights
    let insights = tracing.get_performance_insights().await;
    assert!(insights.is_ok());

    let insights = insights.unwrap();
    assert!(insights.metrics.total_traces > 0);
    // Should have some insights or recommendations
    assert!(!insights.insights.is_empty() || !insights.recommendations.is_empty());
}

#[tokio::test]
async fn test_trace_visualization() {
    let config = TracingConfig {
        service_name: "test-service".to_string(),
        enable_jaeger_exporter: false,
        enable_otlp_exporter: false,
        enable_console_exporter: false,
        ..TracingConfig::default()
    };

    let tracing = DistributedTracing::new(config).await.unwrap();

    // Create a trace with child spans
    let parent_correlation_id = tracing
        .start_trace("parent_operation", "parent_component", None)
        .await;

    let child1_correlation_id = tracing
        .start_child_span(parent_correlation_id, "child1_operation", "child1_component")
        .await
        .unwrap();

    let child2_correlation_id = tracing
        .start_child_span(parent_correlation_id, "child2_operation", "child2_component")
        .await
        .unwrap();

    // End spans
    tracing.end_trace(child1_correlation_id, TraceStatus::Ok).await.unwrap();
    tracing.end_trace(child2_correlation_id, TraceStatus::Ok).await.unwrap();
    tracing.end_trace(parent_correlation_id, TraceStatus::Ok).await.unwrap();

    // Wait a bit for trace processing
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Get completed traces to find trace ID
    let analyzer = tracing.get_trace_analyzer();
    let traces = analyzer.get_completed_traces(1).await;
    
    if !traces.is_empty() {
        let trace_id = &traces[0].trace_id;
        
        // Generate visualization
        let visualization = tracing.generate_trace_visualization(trace_id).await;
        assert!(visualization.is_ok());

        let viz = visualization.unwrap();
        assert_eq!(viz.trace_id, *trace_id);
        assert!(viz.spans.len() >= 1); // Should have at least the root span
    }
}

#[tokio::test]
async fn test_resource_usage_tracking() {
    let config = TracingConfig::default();
    let processor = SpanProcessor::new(config);

    // Create span with resource usage
    let mut span = create_mock_processed_span("test_operation", None);
    span.resource_usage = Some(ResourceUsage {
        cpu_time_ms: Some(150),
        memory_bytes: Some(1024 * 1024),
        network_bytes_sent: Some(2048),
        network_bytes_received: Some(4096),
        disk_bytes_read: Some(8192),
        disk_bytes_written: Some(1024),
        database_queries: Some(3),
        cache_operations: Some(5),
    });

    processor.process_span(span.clone()).await.unwrap();

    let result = processor.complete_span(&span.span_id).await.unwrap();
    assert!(result.is_some());

    let completed_trace = result.unwrap();
    assert!(completed_trace.spans.iter().any(|s| s.resource_usage.is_some()));
}

#[tokio::test]
async fn test_critical_path_calculation() {
    let config = TracingConfig::default();
    let processor = SpanProcessor::new(config);

    // Create spans with different durations
    let root_span = create_mock_processed_span_with_duration("root", None, Duration::from_millis(1000));
    let fast_child = create_mock_processed_span_with_duration("fast_child", Some(root_span.span_id.clone()), Duration::from_millis(100));
    let slow_child = create_mock_processed_span_with_duration("slow_child", Some(root_span.span_id.clone()), Duration::from_millis(800));

    processor.process_span(fast_child).await.unwrap();
    processor.process_span(slow_child.clone()).await.unwrap();
    processor.process_span(root_span.clone()).await.unwrap();

    let result = processor.complete_span(&root_span.span_id).await.unwrap();
    assert!(result.is_some());

    let completed_trace = result.unwrap();
    assert!(!completed_trace.critical_path.is_empty());
    assert!(completed_trace.performance_summary.critical_path_duration > Duration::from_millis(0));
}

// Helper functions for creating mock data

fn create_mock_completed_trace() -> CompletedTrace {
    create_mock_completed_trace_with_duration(Duration::from_millis(500))
}

fn create_mock_completed_trace_with_duration(duration: Duration) -> CompletedTrace {
    let trace_id = Uuid::new_v4().to_string();
    let root_span = create_mock_processed_span_with_duration("root_operation", None, duration);

    CompletedTrace {
        trace_id: trace_id.clone(),
        root_span: root_span.clone(),
        spans: vec![root_span],
        total_duration: duration,
        span_count: 1,
        error_count: 0,
        critical_path: vec![],
        performance_summary: rustci::core::observability::distributed_tracing::TracePerformanceSummary {
            total_duration: duration,
            critical_path_duration: duration,
            parallelism_factor: 1.0,
            slowest_span: None,
            bottleneck_spans: vec![],
            error_spans: vec![],
            performance_score: 90.0,
        },
        completed_at: SystemTime::now(),
    }
}

fn create_mock_completed_trace_with_errors() -> CompletedTrace {
    let trace_id = Uuid::new_v4().to_string();
    let mut root_span = create_mock_processed_span("root_operation", None);
    root_span.status = TraceStatus::Error;

    CompletedTrace {
        trace_id: trace_id.clone(),
        root_span: root_span.clone(),
        spans: vec![root_span],
        total_duration: Duration::from_millis(500),
        span_count: 1,
        error_count: 1,
        critical_path: vec![],
        performance_summary: rustci::core::observability::distributed_tracing::TracePerformanceSummary {
            total_duration: Duration::from_millis(500),
            critical_path_duration: Duration::from_millis(500),
            parallelism_factor: 1.0,
            slowest_span: None,
            bottleneck_spans: vec![],
            error_spans: vec!["span_1".to_string()],
            performance_score: 50.0,
        },
        completed_at: SystemTime::now(),
    }
}

fn create_mock_processed_span(operation: &str, parent_span_id: Option<String>) -> ProcessedSpan {
    create_mock_processed_span_with_duration(operation, parent_span_id, Duration::from_millis(100))
}

fn create_mock_processed_span_with_duration(
    operation: &str, 
    parent_span_id: Option<String>, 
    duration: Duration
) -> ProcessedSpan {
    let span_id = Uuid::new_v4().to_string();
    let trace_id = Uuid::new_v4().to_string();
    let now = SystemTime::now();

    ProcessedSpan {
        span_id,
        trace_id,
        parent_span_id,
        operation_name: operation.to_string(),
        component: "test_component".to_string(),
        start_time: now - duration,
        end_time: Some(now),
        duration: Some(duration),
        attributes: HashMap::new(),
        events: vec![],
        status: TraceStatus::Ok,
        tags: vec!["test".to_string()],
        resource_usage: None,
    }
}