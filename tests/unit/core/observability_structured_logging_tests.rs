use rustci::core::observability::structured_logging::{
    StructuredLogging, LoggingConfig, CorrelationTracker, RequestTracker,
    ErrorInfo, PerformanceInfo, RequestLogContext,
};
use std::collections::HashMap;
use uuid::Uuid;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_correlation_tracking_lifecycle() {
    let tracker = CorrelationTracker::new();

    // Start correlation
    let correlation_id = tracker
        .start_correlation(
            "test_operation".to_string(),
            "test_component".to_string(),
            None,
        )
        .await;

    // Add breadcrumbs
    tracker
        .add_breadcrumb(correlation_id, "Step 1: Initialize".to_string())
        .await;
    tracker
        .add_breadcrumb(correlation_id, "Step 2: Process".to_string())
        .await;
    tracker
        .add_breadcrumb(correlation_id, "Step 3: Complete".to_string())
        .await;

    // Update correlation with additional context
    let mut attributes = HashMap::new();
    attributes.insert("user_type".to_string(), "admin".to_string());
    attributes.insert("priority".to_string(), "high".to_string());

    tracker
        .update_correlation(
            correlation_id,
            Some("user123".to_string()),
            Some("session456".to_string()),
            Some("req789".to_string()),
            attributes,
        )
        .await;

    // Verify context
    let context = tracker.get_correlation(correlation_id).await;
    assert!(context.is_some());
    let context = context.unwrap();
    assert_eq!(context.operation, "test_operation");
    assert_eq!(context.component, "test_component");
    assert_eq!(context.user_id, Some("user123".to_string()));
    assert_eq!(context.session_id, Some("session456".to_string()));
    assert_eq!(context.request_id, Some("req789".to_string()));
    assert_eq!(context.breadcrumbs.len(), 4); // Including initial breadcrumb
    assert!(context.attributes.contains_key("user_type"));

    // End correlation
    let ended_context = tracker.end_correlation(correlation_id).await;
    assert!(ended_context.is_some());
    assert_eq!(tracker.get_active_count().await, 0);
}

#[tokio::test]
async fn test_request_tracking_full_lifecycle() {
    let tracker = RequestTracker::new();

    // Start request tracking
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let mut query_params = HashMap::new();
    query_params.insert("page".to_string(), "1".to_string());
    query_params.insert("limit".to_string(), "10".to_string());

    let request_id = tracker
        .start_request(
            "POST".to_string(),
            "/api/v1/jobs".to_string(),
            None,
            Some("user123".to_string()),
            Some("session456".to_string()),
            Some("192.168.1.100".to_string()),
            Some("Mozilla/5.0 (Test Agent)".to_string()),
            headers,
            query_params,
        )
        .await;

    assert!(request_id.starts_with("req_"));
    assert_eq!(tracker.get_active_count().await, 1);

    // Get request context
    let context = tracker.get_request(&request_id).await;
    assert!(context.is_some());
    let context = context.unwrap();
    assert_eq!(context.method, "POST");
    assert_eq!(context.path, "/api/v1/jobs");
    assert_eq!(context.user_id, Some("user123".to_string()));
    assert_eq!(context.client_ip, Some("192.168.1.100".to_string()));
    assert!(context.headers.contains_key("Authorization"));
    assert!(context.query_params.contains_key("page"));

    // Simulate processing time
    sleep(Duration::from_millis(10)).await;

    // Complete request
    let completed_context = tracker
        .complete_request(&request_id, 201, Some(2048))
        .await;

    assert!(completed_context.is_some());
    let completed = completed_context.unwrap();
    assert_eq!(completed.response_status, Some(201));
    assert_eq!(completed.response_size, Some(2048));
    assert!(completed.processing_time_ms.is_some());
    assert!(completed.processing_time_ms.unwrap() >= 10);
    assert_eq!(tracker.get_active_count().await, 0);
}

#[tokio::test]
async fn test_structured_logging_with_full_context() {
    let config = LoggingConfig {
        service_name: "test-service".to_string(),
        environment: "test".to_string(),
        log_level: "debug".to_string(),
        enable_json_format: true,
        enable_console_output: false, // Disable for testing
        enable_file_output: false,
        file_path: None,
        max_buffer_size: 1000,
        enable_correlation_tracking: true,
        enable_performance_logging: true,
    };

    let logger = StructuredLogging::new(config).unwrap();

    // Start correlation
    let correlation_id = logger
        .correlation_tracker()
        .start_correlation(
            "api_request".to_string(),
            "user_service".to_string(),
            None,
        )
        .await;

    // Start request tracking
    let request_id = logger
        .request_tracker()
        .start_request(
            "GET".to_string(),
            "/api/users/123".to_string(),
            Some(correlation_id),
            Some("user456".to_string()),
            Some("session789".to_string()),
            Some("10.0.0.1".to_string()),
            Some("TestAgent/1.0".to_string()),
            HashMap::new(),
            HashMap::new(),
        )
        .await;

    // Log with full context
    let mut attributes = HashMap::new();
    attributes.insert(
        "user_id".to_string(),
        serde_json::Value::String("user456".to_string()),
    );
    attributes.insert(
        "action".to_string(),
        serde_json::Value::String("get_profile".to_string()),
    );

    let performance_info = PerformanceInfo {
        operation_duration_ms: 150,
        memory_usage_bytes: Some(1024 * 1024),
        cpu_usage_percent: Some(15.5),
        db_query_count: Some(2),
        db_query_duration_ms: Some(45),
        cache_hits: Some(1),
        cache_misses: Some(1),
    };

    logger
        .log_with_full_context(
            "info",
            "User profile retrieved successfully",
            Some(correlation_id),
            Some("get_user_profile".to_string()),
            Some("user_service".to_string()),
            attributes,
            None,
            Some(performance_info),
            vec!["api".to_string(), "user".to_string(), "success".to_string()],
            Some("trace123".to_string()),
            Some("span456".to_string()),
            Some(request_id.clone()),
        )
        .await;

    // Complete request
    logger
        .request_tracker()
        .complete_request(&request_id, 200, Some(512))
        .await;

    // Verify log entry
    let logs = logger.search_logs_by_correlation(correlation_id).await;
    assert_eq!(logs.len(), 1);

    let log_entry = &logs[0];
    assert_eq!(log_entry.message, "User profile retrieved successfully");
    assert_eq!(log_entry.level, "info");
    assert_eq!(log_entry.service, "test-service");
    assert_eq!(log_entry.environment, "test");
    assert_eq!(log_entry.operation, Some("get_user_profile".to_string()));
    assert_eq!(log_entry.component, Some("user_service".to_string()));
    assert_eq!(log_entry.trace_id, Some("trace123".to_string()));
    assert_eq!(log_entry.span_id, Some("span456".to_string()));
    assert!(log_entry.tags.contains(&"api".to_string()));
    assert!(log_entry.tags.contains(&"user".to_string()));
    assert!(log_entry.tags.contains(&"success".to_string()));
    assert!(log_entry.performance.is_some());
    assert!(log_entry.request_context.is_some());
    assert!(log_entry.sequence > 0);
    assert!(!log_entry.timestamp_iso.is_empty());

    let request_context = log_entry.request_context.as_ref().unwrap();
    assert_eq!(request_context.method, "GET");
    assert_eq!(request_context.path, "/api/users/123");
    assert_eq!(request_context.response_status, Some(200));
    assert_eq!(request_context.response_size, Some(512));
}

#[tokio::test]
async fn test_error_logging_with_context() {
    let config = LoggingConfig::default();
    let logger = StructuredLogging::new(config).unwrap();

    let correlation_id = logger
        .correlation_tracker()
        .start_correlation(
            "database_operation".to_string(),
            "data_service".to_string(),
            None,
        )
        .await;

    let error_info = ErrorInfo {
        error_type: "DatabaseConnectionError".to_string(),
        error_message: "Failed to connect to database: connection timeout".to_string(),
        stack_trace: Some("at database.rs:123\nat service.rs:456".to_string()),
        error_code: Some("DB_CONN_TIMEOUT".to_string()),
    };

    let mut attributes = HashMap::new();
    attributes.insert(
        "database_host".to_string(),
        serde_json::Value::String("db.example.com".to_string()),
    );
    attributes.insert(
        "retry_count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(3)),
    );

    logger
        .log_with_correlation(
            "error",
            "Database operation failed",
            Some(correlation_id),
            Some("connect_database".to_string()),
            Some("data_service".to_string()),
            attributes,
            Some(error_info),
            None,
        )
        .await;

    let logs = logger.search_logs_by_correlation(correlation_id).await;
    assert_eq!(logs.len(), 1);

    let log_entry = &logs[0];
    assert_eq!(log_entry.level, "error");
    assert!(log_entry.error.is_some());

    let error = log_entry.error.as_ref().unwrap();
    assert_eq!(error.error_type, "DatabaseConnectionError");
    assert!(error.error_message.contains("connection timeout"));
    assert!(error.stack_trace.is_some());
    assert_eq!(error.error_code, Some("DB_CONN_TIMEOUT".to_string()));
}

#[tokio::test]
async fn test_log_search_and_aggregation() {
    let config = LoggingConfig::default();
    let logger = StructuredLogging::new(config).unwrap();

    // Create multiple log entries with different characteristics
    for i in 0..10 {
        let level = if i % 3 == 0 { "error" } else if i % 2 == 0 { "warn" } else { "info" };
        let component = if i < 5 { "service_a" } else { "service_b" };
        let tags = if i % 2 == 0 { 
            vec!["production".to_string(), "critical".to_string()] 
        } else { 
            vec!["development".to_string(), "debug".to_string()] 
        };

        logger
            .log_with_tags(
                level,
                &format!("Test message {}", i),
                None,
                Some(format!("operation_{}", i)),
                Some(component.to_string()),
                tags,
                HashMap::new(),
            )
            .await;
    }

    // Test search by tags
    let production_logs = logger
        .search_logs_by_tags(&["production".to_string()])
        .await;
    assert_eq!(production_logs.len(), 5);

    let debug_logs = logger
        .search_logs_by_tags(&["debug".to_string()])
        .await;
    assert_eq!(debug_logs.len(), 5);

    // Test search by component
    let service_a_logs = logger
        .search_logs_by_component_operation(Some("service_a"), None)
        .await;
    assert_eq!(service_a_logs.len(), 5);

    let service_b_logs = logger
        .search_logs_by_component_operation(Some("service_b"), None)
        .await;
    assert_eq!(service_b_logs.len(), 5);

    // Test aggregations
    let level_agg = logger.get_log_aggregation_by_level().await;
    assert!(level_agg.get("info").unwrap() > &0);
    assert!(level_agg.get("warn").unwrap() > &0);
    assert!(level_agg.get("error").unwrap() > &0);

    let component_agg = logger.get_log_aggregation_by_component().await;
    assert_eq!(component_agg.get("service_a"), Some(&5));
    assert_eq!(component_agg.get("service_b"), Some(&5));

    // Test statistics
    let stats = logger.get_statistics().await;
    assert_eq!(stats.total_entries, 10);
    assert!(stats.level_counts.contains_key("info"));
    assert!(stats.component_counts.contains_key("service_a"));
    assert!(stats.tag_counts.contains_key("production"));
}

#[tokio::test]
async fn test_time_range_search() {
    let config = LoggingConfig::default();
    let logger = StructuredLogging::new(config).unwrap();

    let start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Log some entries
    for i in 0..3 {
        logger
            .log_with_correlation(
                "info",
                &format!("Message {}", i),
                None,
                None,
                None,
                HashMap::new(),
                None,
                None,
            )
            .await;

        // Small delay to ensure different timestamps
        sleep(Duration::from_millis(1)).await;
    }

    let end_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Search by time range
    let logs_in_range = logger
        .search_logs_by_time_range(start_time, end_time)
        .await;
    assert_eq!(logs_in_range.len(), 3);

    // Search with narrow range (should find fewer logs)
    let logs_narrow = logger
        .search_logs_by_time_range(start_time, start_time + 1)
        .await;
    assert!(logs_narrow.len() <= 3);
}

#[tokio::test]
async fn test_correlation_with_parent_child_relationships() {
    let tracker = CorrelationTracker::new();

    // Start parent correlation
    let parent_id = tracker
        .start_correlation(
            "parent_operation".to_string(),
            "parent_component".to_string(),
            None,
        )
        .await;

    // Start child correlation
    let child_id = tracker
        .start_correlation(
            "child_operation".to_string(),
            "child_component".to_string(),
            Some(parent_id),
        )
        .await;

    // Verify parent-child relationship
    let child_context = tracker.get_correlation(child_id).await;
    assert!(child_context.is_some());
    assert_eq!(child_context.unwrap().parent_correlation_id, Some(parent_id));

    // End correlations
    tracker.end_correlation(child_id).await;
    tracker.end_correlation(parent_id).await;

    assert_eq!(tracker.get_active_count().await, 0);
}

#[tokio::test]
async fn test_breadcrumb_overflow_protection() {
    let tracker = CorrelationTracker::new();

    let correlation_id = tracker
        .start_correlation(
            "test_operation".to_string(),
            "test_component".to_string(),
            None,
        )
        .await;

    // Add more than 50 breadcrumbs to test overflow protection
    for i in 0..60 {
        tracker
            .add_breadcrumb(correlation_id, format!("Breadcrumb {}", i))
            .await;
    }

    let context = tracker.get_correlation(correlation_id).await;
    assert!(context.is_some());
    let context = context.unwrap();
    
    // Should be limited to 50 breadcrumbs (plus initial one, but overflow removes oldest)
    assert_eq!(context.breadcrumbs.len(), 50);
    
    // Should contain the most recent breadcrumbs
    assert!(context.breadcrumbs.last().unwrap().contains("Breadcrumb 59"));

    tracker.end_correlation(correlation_id).await;
}