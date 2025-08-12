// Valkyrie Protocol Observability Example
// Demonstrates the self-contained observability system

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig, MetricValue, LogLevel,
    HealthCheck, HealthCheckType, HealthCheckConfig, CorrelationStatus,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️ Valkyrie Protocol Observability System Demo");
    println!("================================================");

    // Create observability manager with default configuration
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);

    // Start the observability system
    println!("\n📊 Starting observability system...");
    manager.start().await?;

    // Get references to subsystems
    let metrics = manager.metrics();
    let logger = manager.logger();
    let health = manager.health();
    let dashboard = manager.dashboard();
    let correlation = manager.correlation();

    println!("✅ Observability system started successfully!");

    // Demonstrate metrics collection
    println!("\n📈 Demonstrating metrics collection...");
    
    // Record various types of metrics
    let mut labels = HashMap::new();
    labels.insert("component".to_string(), "demo".to_string());
    labels.insert("version".to_string(), "1.0.0".to_string());

    // Counter metrics
    for i in 1..=5 {
        metrics.increment_counter("demo_requests_total", labels.clone()).await?;
        println!("  📊 Recorded counter metric: demo_requests_total = {}", i);
        sleep(Duration::from_millis(100)).await;
    }

    // Gauge metrics
    for i in 1..=3 {
        let cpu_usage = 20.0 + (i as f64 * 10.0);
        metrics.set_gauge("demo_cpu_usage_percent", cpu_usage, labels.clone()).await?;
        println!("  📊 Recorded gauge metric: demo_cpu_usage_percent = {:.1}%", cpu_usage);
        sleep(Duration::from_millis(100)).await;
    }

    // Histogram metrics
    let latencies = vec![1.2, 2.5, 1.8, 3.1, 0.9];
    metrics.record_histogram("demo_request_latency_ms", latencies.clone(), labels.clone()).await?;
    println!("  📊 Recorded histogram metric: demo_request_latency_ms = {:?}", latencies);

    // Summary metrics
    metrics.record_summary("demo_response_size_bytes", 1024.0, 10, labels.clone()).await?;
    println!("  📊 Recorded summary metric: demo_response_size_bytes (sum=1024.0, count=10)");

    // Demonstrate structured logging
    println!("\n📝 Demonstrating structured logging...");

    // Create a correlation ID for tracing
    let correlation_id = manager.new_correlation_id();
    println!("  🔗 Created correlation ID: {}", correlation_id);

    // Log with different levels
    logger.info("Demo application started").await?;
    logger.warn("This is a warning message").await?;

    // Log with context
    let mut context = HashMap::new();
    context.insert("user_id".to_string(), serde_json::Value::String("demo_user".to_string()));
    context.insert("operation".to_string(), serde_json::Value::String("data_processing".to_string()));
    context.insert("duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(150)));

    logger.log(LogLevel::Info, "Processing completed successfully", context).await?;
    println!("  📝 Logged structured message with context");

    // Log with correlation
    manager.log_with_correlation(
        correlation_id,
        LogLevel::Info,
        "Operation completed with correlation tracking",
        HashMap::new(),
    ).await?;
    println!("  📝 Logged message with correlation ID: {}", correlation_id);

    // Demonstrate health monitoring
    println!("\n🏥 Demonstrating health monitoring...");

    // Wait for health checks to run
    sleep(Duration::from_millis(500)).await;

    let health_summary = health.summary().await;
    println!("  🏥 Health Summary:");
    println!("    - Overall Status: {}", health_summary.overall_status);
    println!("    - Total Checks: {}", health_summary.total_checks);
    println!("    - Healthy: {}", health_summary.healthy_checks);
    println!("    - Degraded: {}", health_summary.degraded_checks);
    println!("    - Unhealthy: {}", health_summary.unhealthy_checks);

    // Register a custom health check
    let custom_check = HealthCheck {
        id: "demo_service_check".to_string(),
        name: "Demo Service Health".to_string(),
        description: "Checks if the demo service is responding".to_string(),
        check_type: HealthCheckType::Memory { max_usage_percent: 80.0 },
        config: HealthCheckConfig {
            parameters: HashMap::new(),
            environment: HashMap::new(),
            working_directory: None,
        },
        interval_seconds: 30,
        timeout_seconds: 5,
        failure_threshold: 3,
        success_threshold: 2,
        enabled: true,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    health.register_check(custom_check).await?;
    println!("  🏥 Registered custom health check: demo_service_check");

    // Demonstrate correlation tracking
    println!("\n🔗 Demonstrating correlation tracking...");

    let operation_correlation = correlation.start_correlation("demo_operation", None).await?;
    println!("  🔗 Started correlation: {}", operation_correlation);

    // Add metadata
    correlation.add_metadata(
        operation_correlation,
        "request_id",
        serde_json::Value::String("req_12345".to_string()),
    ).await?;

    // Start a span
    let span_id = correlation.start_span(
        operation_correlation,
        "data_processing",
        "demo_component",
        None,
    ).await?;
    println!("  🔗 Started span: {}", span_id);

    // Simulate some work
    sleep(Duration::from_millis(200)).await;

    // End the span
    correlation.end_span(
        operation_correlation,
        span_id,
        rustci::core::networking::valkyrie::observability::correlation::SpanStatus::Ok,
    ).await?;
    println!("  🔗 Ended span: {}", span_id);

    // End the correlation
    correlation.end_correlation(operation_correlation, CorrelationStatus::Completed).await?;
    println!("  🔗 Ended correlation: {}", operation_correlation);

    // Demonstrate dashboard
    println!("\n📊 Demonstrating internal dashboard...");

    // Refresh dashboard data
    dashboard.refresh_data().await?;
    let dashboard_data = dashboard.get_data().await;

    println!("  📊 Dashboard Data:");
    println!("    - Title: {}", dashboard_data.metadata.title);
    println!("    - Version: {}", dashboard_data.metadata.version);
    println!("    - Last Updated: {}", dashboard_data.last_updated);
    println!("    - System Status: {}", dashboard_data.health_status.overall_status);
    println!("    - Total Metrics: {}", dashboard_data.metrics_summary.total_metrics);
    println!("    - Total Data Points: {}", dashboard_data.metrics_summary.total_data_points);

    // Generate HTML dashboard
    let html_dashboard = dashboard.generate_html().await;
    println!("  📊 Generated HTML dashboard ({} bytes)", html_dashboard.len());

    // Show final statistics
    println!("\n📈 Final Statistics:");

    let metrics_summary = metrics.summary().await;
    println!("  📊 Metrics: {} total, {} data points", 
             metrics_summary.total_metrics, 
             metrics_summary.total_data_points);

    let log_stats = logger.statistics().await;
    println!("  📝 Logs: {} entries", log_stats.total_entries);

    let correlation_stats = correlation.get_statistics().await;
    println!("  🔗 Correlations: {} created, {} completed", 
             correlation_stats.total_created, 
             correlation_stats.completed_count);

    let final_status = manager.status().await;
    println!("  🛡️ Observability Status:");
    println!("    - Metrics: {}", if final_status.metrics_enabled { "✅ Enabled" } else { "❌ Disabled" });
    println!("    - Logging: {}", if final_status.logging_enabled { "✅ Enabled" } else { "❌ Disabled" });
    println!("    - Health: {}", if final_status.health_enabled { "✅ Enabled" } else { "❌ Disabled" });
    println!("    - Dashboard: {}", if final_status.dashboard_enabled { "✅ Enabled" } else { "❌ Disabled" });

    // Stop the observability system
    println!("\n🛑 Stopping observability system...");
    manager.stop().await?;
    println!("✅ Observability system stopped successfully!");

    println!("\n🎉 Valkyrie Protocol Observability Demo Complete!");
    println!("The self-contained observability system provides:");
    println!("  • Built-in metrics collection without external dependencies");
    println!("  • Structured logging with correlation IDs");
    println!("  • Health monitoring with customizable checks");
    println!("  • Internal metrics dashboard with HTML generation");
    println!("  • Distributed tracing with correlation tracking");

    Ok(())
}