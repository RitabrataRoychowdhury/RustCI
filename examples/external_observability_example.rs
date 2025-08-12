// Valkyrie Protocol External Observability Integration Example
// Demonstrates integration with Prometheus, OpenTelemetry, Jaeger, and Grafana

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig, ExternalObservabilityConfig,
    MetricValue, LogLevel, CorrelationStatus,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️ Valkyrie Protocol External Observability Integration Demo");
    println!("============================================================");

    // Configure external observability integrations
    let external_config = ExternalObservabilityConfig {
        // Prometheus configuration
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        prometheus_push_gateway: Some("http://localhost:9091".to_string()),
        
        // OpenTelemetry configuration
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        
        // Jaeger configuration
        jaeger_enabled: true,
        jaeger_endpoint: "http://localhost:14268".to_string(),
        
        // Grafana configuration
        grafana_enabled: true,
        grafana_endpoint: "http://localhost:3000".to_string(),
        grafana_api_key: Some("your-grafana-api-key".to_string()),
        
        // Export settings
        export_interval_seconds: 10,
        batch_size: 1000,
    };

    // Create observability manager with external integrations
    let config = ObservabilityConfig {
        metrics_enabled: true,
        logging_enabled: true,
        health_enabled: true,
        dashboard_enabled: true,
        external_config: Some(external_config),
        metrics_retention_seconds: 3600,
        log_retention_seconds: 86400,
        health_check_interval_seconds: 30,
        dashboard_refresh_seconds: 5,
    };

    let mut manager = ObservabilityManager::new(config);

    // Start the observability system with external integrations
    println!("\n📊 Starting observability system with external integrations...");
    manager.start().await?;

    // Get references to subsystems
    let metrics = manager.metrics();
    let logger = manager.logger();
    let health = manager.health();
    let dashboard = manager.dashboard();
    let correlation = manager.correlation();
    let external = manager.external();

    println!("✅ Observability system started successfully!");

    // Check external integration status
    if let Some(ref external_integration) = external {
        println!("\n🔌 External Integration Status:");
        let status = external_integration.status().await;
        
        println!("  📈 Prometheus: {} - {:?}", 
                 if status.prometheus_enabled { "Enabled" } else { "Disabled" },
                 status.prometheus_status);
        println!("  🔍 OpenTelemetry: {} - {:?}", 
                 if status.opentelemetry_enabled { "Enabled" } else { "Disabled" },
                 status.opentelemetry_status);
        println!("  🕵️ Jaeger: {} - {:?}", 
                 if status.jaeger_enabled { "Enabled" } else { "Disabled" },
                 status.jaeger_status);
        println!("  📊 Grafana: {} - {:?}", 
                 if status.grafana_enabled { "Enabled" } else { "Disabled" },
                 status.grafana_status);
    }

    // Demonstrate comprehensive observability with external export
    println!("\n🚀 Demonstrating comprehensive observability...");

    // Create correlation for distributed tracing
    let correlation_id = correlation.start_correlation("external_demo_operation", None).await?;
    println!("  🔗 Started correlation: {}", correlation_id);

    // Add correlation metadata
    correlation.add_metadata(
        correlation_id,
        "demo_type",
        serde_json::Value::String("external_integration".to_string()),
    ).await?;

    // Record various metrics that will be exported to Prometheus
    println!("\n📈 Recording metrics for external export...");
    let mut labels = HashMap::new();
    labels.insert("service".to_string(), "valkyrie-demo".to_string());
    labels.insert("version".to_string(), "1.0.0".to_string());
    labels.insert("environment".to_string(), "demo".to_string());

    // Business metrics
    for i in 1..=10 {
        // Request metrics
        metrics.increment_counter("demo_requests_total", labels.clone()).await?;
        
        // Latency metrics (simulating sub-millisecond performance)
        let latency = 0.1 + (i as f64 * 0.05); // 0.1ms to 0.6ms
        metrics.record_histogram("demo_request_duration_ms", vec![latency], labels.clone()).await?;
        
        // Throughput metrics
        let throughput = 50000.0 + (i as f64 * 1000.0); // 50k to 60k RPS
        metrics.set_gauge("demo_throughput_rps", throughput, labels.clone()).await?;
        
        // Connection metrics
        let connections = 100000 + (i * 5000); // 100k to 150k connections
        metrics.set_gauge("demo_active_connections", connections as f64, labels.clone()).await?;
        
        // Error rate metrics
        if i % 3 == 0 {
            metrics.increment_counter("demo_errors_total", labels.clone()).await?;
        }
        
        println!("  📊 Recorded metrics batch {} (Latency: {:.3}ms, Throughput: {:.0} RPS)", 
                 i, latency, throughput);
        
        sleep(Duration::from_millis(100)).await;
    }

    // Record system resource metrics
    let mut system_labels = HashMap::new();
    system_labels.insert("component".to_string(), "valkyrie-protocol".to_string());
    
    metrics.set_gauge("demo_memory_usage_bytes", 512.0 * 1024.0 * 1024.0, system_labels.clone()).await?; // 512MB
    metrics.set_gauge("demo_cpu_usage_percent", 25.5, system_labels.clone()).await?;
    metrics.set_gauge("demo_disk_usage_percent", 45.2, system_labels.clone()).await?;

    // Structured logging with correlation
    println!("\n📝 Generating structured logs for external export...");
    
    let mut log_context = HashMap::new();
    log_context.insert("correlation_id".to_string(), serde_json::Value::String(correlation_id.to_string()));
    log_context.insert("operation".to_string(), serde_json::Value::String("external_demo".to_string()));
    log_context.insert("metrics_recorded".to_string(), serde_json::Value::Number(serde_json::Number::from(10)));
    
    logger.log(LogLevel::Info, "External observability demo in progress", log_context).await?;

    // Create distributed trace spans
    println!("\n🕵️ Creating distributed trace spans for Jaeger export...");
    
    let span1_id = correlation.start_span(
        correlation_id,
        "metrics_collection",
        "valkyrie-metrics",
        None,
    ).await?;
    
    sleep(Duration::from_millis(50)).await;
    
    let span2_id = correlation.start_span(
        correlation_id,
        "data_processing",
        "valkyrie-processor",
        Some(span1_id),
    ).await?;
    
    // Add span logs
    let mut span_fields = HashMap::new();
    span_fields.insert("processed_items".to_string(), serde_json::Value::Number(serde_json::Number::from(10)));
    span_fields.insert("processing_time_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(25)));
    
    correlation.add_span_log(
        correlation_id,
        span2_id,
        "INFO",
        "Data processing completed",
        span_fields,
    ).await?;
    
    sleep(Duration::from_millis(25)).await;
    
    // End spans
    correlation.end_span(correlation_id, span2_id, 
        rustci::core::networking::valkyrie::observability::correlation::SpanStatus::Ok).await?;
    correlation.end_span(correlation_id, span1_id, 
        rustci::core::networking::valkyrie::observability::correlation::SpanStatus::Ok).await?;
    
    println!("  🕵️ Created and completed trace spans");

    // Export data to external systems
    if let Some(ref external_integration) = external {
        println!("\n🔄 Exporting data to external observability systems...");
        
        // Export metrics to Prometheus
        println!("  📈 Exporting metrics to Prometheus...");
        if let Err(e) = external_integration.export_metrics(&metrics).await {
            println!("    ⚠️  Prometheus export failed: {} (this is expected if Prometheus is not running)", e);
        } else {
            println!("    ✅ Metrics exported to Prometheus successfully");
        }
        
        // Export logs to OpenTelemetry
        println!("  📝 Exporting logs to OpenTelemetry...");
        if let Err(e) = external_integration.export_logs(&logger).await {
            println!("    ⚠️  OpenTelemetry log export failed: {} (this is expected if OTel collector is not running)", e);
        } else {
            println!("    ✅ Logs exported to OpenTelemetry successfully");
        }
        
        // Export traces to Jaeger
        println!("  🕵️ Exporting traces to Jaeger...");
        let spans = vec![]; // In real implementation, this would contain actual span data
        if let Err(e) = external_integration.export_traces(correlation_id, &spans).await {
            println!("    ⚠️  Jaeger export failed: {} (this is expected if Jaeger is not running)", e);
        } else {
            println!("    ✅ Traces exported to Jaeger successfully");
        }
        
        // Update Grafana dashboards
        println!("  📊 Updating Grafana dashboards...");
        if let Err(e) = external_integration.update_dashboards().await {
            println!("    ⚠️  Grafana dashboard update failed: {} (this is expected if Grafana is not running)", e);
        } else {
            println!("    ✅ Grafana dashboards updated successfully");
        }
    }

    // End correlation
    correlation.end_correlation(correlation_id, CorrelationStatus::Completed).await?;
    println!("  🔗 Ended correlation: {}", correlation_id);

    // Generate internal dashboard
    println!("\n📊 Generating internal dashboard...");
    dashboard.refresh_data().await?;
    let dashboard_data = dashboard.get_data().await;
    let html_dashboard = dashboard.generate_html().await;
    
    println!("  📊 Internal Dashboard Generated:");
    println!("    - Title: {}", dashboard_data.metadata.title);
    println!("    - Total Metrics: {}", dashboard_data.metrics_summary.total_metrics);
    println!("    - Total Data Points: {}", dashboard_data.metrics_summary.total_data_points);
    println!("    - Health Status: {}", dashboard_data.health_status.overall_status);
    println!("    - HTML Size: {} bytes", html_dashboard.len());

    // Show final statistics
    println!("\n📈 Final Observability Statistics:");
    
    let metrics_summary = metrics.summary().await;
    println!("  📊 Metrics: {} total, {} data points", 
             metrics_summary.total_metrics, 
             metrics_summary.total_data_points);

    let log_stats = logger.statistics().await;
    println!("  📝 Logs: {} entries", log_stats.total_entries);

    let health_summary = health.summary().await;
    println!("  🏥 Health: {} checks ({} healthy, {} degraded, {} unhealthy)", 
             health_summary.total_checks,
             health_summary.healthy_checks,
             health_summary.degraded_checks,
             health_summary.unhealthy_checks);

    let correlation_stats = correlation.get_statistics().await;
    println!("  🔗 Correlations: {} created, {} completed, avg duration: {:.2}s", 
             correlation_stats.total_created, 
             correlation_stats.completed_count,
             correlation_stats.average_duration_seconds);

    // Show external integration final status
    if let Some(ref external_integration) = external {
        println!("\n🔌 Final External Integration Status:");
        let final_status = external_integration.status().await;
        
        println!("  📈 Prometheus: {:?}", final_status.prometheus_status);
        println!("  🔍 OpenTelemetry: {:?}", final_status.opentelemetry_status);
        println!("  🕵️ Jaeger: {:?}", final_status.jaeger_status);
        println!("  📊 Grafana: {:?}", final_status.grafana_status);
    }

    // Stop the observability system
    println!("\n🛑 Stopping observability system...");
    manager.stop().await?;
    println!("✅ Observability system stopped successfully!");

    println!("\n🎉 Valkyrie Protocol External Observability Demo Complete!");
    println!("=========================================================");
    println!("\n📋 What was demonstrated:");
    println!("  • Self-contained observability with external integration");
    println!("  • Prometheus metrics export with push gateway support");
    println!("  • OpenTelemetry integration for metrics, logs, and traces");
    println!("  • Jaeger distributed tracing with correlation tracking");
    println!("  • Grafana dashboard integration and updates");
    println!("  • Comprehensive error handling for unavailable services");
    println!("  • Production-ready configuration and monitoring");
    
    println!("\n🚀 Next Steps:");
    println!("  1. Start Prometheus: docker run -p 9090:9090 prom/prometheus");
    println!("  2. Start Jaeger: docker run -p 14268:14268 -p 16686:16686 jaegertracing/all-in-one");
    println!("  3. Start Grafana: docker run -p 3000:3000 grafana/grafana");
    println!("  4. Start OpenTelemetry Collector with appropriate configuration");
    println!("  5. Re-run this demo to see full external integration");
    println!("  6. Access dashboards:");
    println!("     - Prometheus: http://localhost:9090");
    println!("     - Jaeger: http://localhost:16686");
    println!("     - Grafana: http://localhost:3000");
    println!("     - Valkyrie Internal: Generated HTML dashboard");

    Ok(())
}