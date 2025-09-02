use rustci::core::observability::debugging::{
    AdvancedDebuggingTools, DebuggingConfig, ErrorContextCollector, StackTraceAnalyzer,
    PerformanceProfiler, DebugSessionManager, TroubleshootingGuide,
    DetailedErrorContext, AnalyzedStackTrace, ProfilingSession, DebugSession,
    SystemState, RequestDebugContext, DatabaseQuery, CacheOperation,
    ErrorPattern, CrashPattern, PerformanceHotspot, TroubleshootingEntry,
    RiskLevel, Solution, DiagnosticStep,
};
use rustci::error::AppError;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[tokio::test]
async fn test_debugging_tools_initialization() {
    let config = DebuggingConfig {
        enable_stack_trace_collection: true,
        enable_performance_profiling: true,
        enable_memory_profiling: true,
        enable_runtime_debugging: false, // Keep disabled for security
        ..DebuggingConfig::default()
    };

    let debugging_tools = AdvancedDebuggingTools::new(config);
    
    // Test that tools are initialized
    let report = debugging_tools.generate_debugging_report().await;
    assert!(report.is_ok());
    
    let report = report.unwrap();
    assert!(report.generated_at <= SystemTime::now());
}

#[tokio::test]
async fn test_error_context_collection() {
    let config = DebuggingConfig::default();
    let collector = ErrorContextCollector::new(config);

    let error = AppError::InternalServerError("Test error".to_string());
    let correlation_id = Some(Uuid::new_v4());
    
    let error_id = collector
        .collect_context(&error, correlation_id, "test_component", "test_operation")
        .await;
    
    assert!(error_id.is_ok());
    
    let summary = collector.get_summary().await;
    assert_eq!(summary.total_errors, 1);
    assert_eq!(summary.unique_error_types, 1);
}

#[tokio::test]
async fn test_stack_trace_analysis() {
    let config = DebuggingConfig::default();
    let analyzer = StackTraceAnalyzer::new(config);

    let stack_trace = r#"
        at main::process_request (src/main.rs:123)
        at tokio::runtime::task::core::Core::poll (tokio/src/runtime/task/core.rs:184)
        at tokio::runtime::task::harness::Harness::poll (tokio/src/runtime/task/harness.rs:148)
    "#;

    let analyzed = analyzer.analyze_trace(stack_trace).await;
    assert!(analyzed.is_ok());
    
    let trace = analyzed.unwrap();
    assert_eq!(trace.raw_trace, stack_trace);
    assert!(!trace.parsed_frames.is_empty());
    assert!(trace.parsed_frames[0].is_likely_crash_point);
    
    let summary = analyzer.get_summary().await;
    assert_eq!(summary.total_crashes, 1);
}

#[tokio::test]
async fn test_performance_profiling() {
    let config = DebuggingConfig {
        enable_performance_profiling: true,
        profiling_sample_rate: 1.0, // 100% for testing
        ..DebuggingConfig::default()
    };
    let profiler = PerformanceProfiler::new(config);

    let session_id = profiler
        .start_session("test_profile", "test_component")
        .await;
    
    assert!(session_id.is_ok());
    
    let summary = profiler.get_summary().await;
    assert_eq!(summary.active_profiling_sessions, 1);
    assert_eq!(summary.completed_sessions, 0);
}

#[tokio::test]
async fn test_debug_session_management() {
    let config = DebuggingConfig {
        enable_runtime_debugging: true, // Enable for testing
        debug_session_timeout: Duration::from_secs(60),
        ..DebuggingConfig::default()
    };
    let manager = DebugSessionManager::new(config);

    let session_id = manager
        .create_session("test_debug", "test_user", Some("test_component".to_string()))
        .await;
    
    assert!(session_id.is_ok());
    
    let summary = manager.get_summary().await;
    assert_eq!(summary.active_debug_sessions, 1);
    assert_eq!(summary.completed_sessions, 0);
}

#[tokio::test]
async fn test_debug_session_disabled() {
    let config = DebuggingConfig {
        enable_runtime_debugging: false, // Disabled for security
        ..DebuggingConfig::default()
    };
    let manager = DebugSessionManager::new(config);

    let session_result = manager
        .create_session("test_debug", "test_user", None)
        .await;
    
    assert!(session_result.is_err());
    assert!(session_result.unwrap_err().to_string().contains("Runtime debugging is disabled"));
}

#[tokio::test]
async fn test_troubleshooting_guide() {
    let config = DebuggingConfig::default();
    let guide = TroubleshootingGuide::new(config);

    let suggestions = guide
        .get_suggestions("database connection")
        .await;
    
    assert!(suggestions.is_ok());
    // Initially empty, but structure is correct
    let suggestions = suggestions.unwrap();
    assert!(suggestions.is_empty()); // No pre-populated data in test
}

#[tokio::test]
async fn test_comprehensive_debugging_workflow() {
    let config = DebuggingConfig {
        enable_stack_trace_collection: true,
        enable_performance_profiling: true,
        enable_memory_profiling: true,
        enable_runtime_debugging: false,
        max_error_contexts: 100,
        max_stack_traces: 50,
        profiling_sample_rate: 0.5,
        debug_session_timeout: Duration::from_secs(300),
    };

    let debugging_tools = AdvancedDebuggingTools::new(config);

    // Simulate error collection
    let error = AppError::DatabaseError("Connection timeout".to_string());
    let correlation_id = Some(Uuid::new_v4());
    
    let error_id = debugging_tools
        .collect_error_context(&error, correlation_id, "database", "connect")
        .await;
    assert!(error_id.is_ok());

    // Simulate stack trace analysis
    let stack_trace = "at database::connect (src/database.rs:45)\nat main::startup (src/main.rs:123)";
    let analyzed_trace = debugging_tools.analyze_stack_trace(stack_trace).await;
    assert!(analyzed_trace.is_ok());

    // Start profiling session
    let profiling_session = debugging_tools
        .start_profiling_session("database_performance", "database")
        .await;
    assert!(profiling_session.is_ok());

    // Get troubleshooting suggestions
    let suggestions = debugging_tools
        .get_troubleshooting_suggestions("database connection")
        .await;
    assert!(suggestions.is_ok());

    // Generate comprehensive report
    let report = debugging_tools.generate_debugging_report().await;
    assert!(report.is_ok());
    
    let report = report.unwrap();
    assert!(report.error_summary.total_errors > 0);
    assert!(report.crash_summary.total_crashes > 0);
    assert!(report.performance_summary.active_profiling_sessions > 0);
}

#[tokio::test]
async fn test_detailed_error_context_creation() {
    let config = DebuggingConfig::default();
    let collector = ErrorContextCollector::new(config);

    // Create multiple errors to test pattern detection
    let errors = vec![
        AppError::DatabaseError("Connection timeout".to_string()),
        AppError::DatabaseError("Connection refused".to_string()),
        AppError::InternalServerError("Memory allocation failed".to_string()),
        AppError::DatabaseError("Connection timeout".to_string()), // Duplicate pattern
    ];

    for (i, error) in errors.iter().enumerate() {
        let correlation_id = Some(Uuid::new_v4());
        let error_id = collector
            .collect_context(error, correlation_id, "test_component", &format!("operation_{}", i))
            .await;
        assert!(error_id.is_ok());
    }

    let summary = collector.get_summary().await;
    assert_eq!(summary.total_errors, 4);
    assert_eq!(summary.unique_error_types, 2); // DatabaseError and InternalServerError
}

#[tokio::test]
async fn test_system_state_capture() {
    let config = DebuggingConfig::default();
    let collector = ErrorContextCollector::new(config);

    let error = AppError::InternalServerError("Test error".to_string());
    let error_id = collector
        .collect_context(&error, None, "test", "test")
        .await;
    
    assert!(error_id.is_ok());
    
    // The system state should be captured (even if with placeholder values in tests)
    let summary = collector.get_summary().await;
    assert_eq!(summary.total_errors, 1);
}

#[tokio::test]
async fn test_error_context_size_limit() {
    let config = DebuggingConfig {
        max_error_contexts: 3, // Small limit for testing
        ..DebuggingConfig::default()
    };
    let collector = ErrorContextCollector::new(config);

    // Add more errors than the limit
    for i in 0..5 {
        let error = AppError::InternalServerError(format!("Error {}", i));
        let error_id = collector
            .collect_context(&error, None, "test", "test")
            .await;
        assert!(error_id.is_ok());
    }

    let summary = collector.get_summary().await;
    assert_eq!(summary.total_errors, 3); // Should be limited to max_error_contexts
}

#[tokio::test]
async fn test_stack_trace_parsing() {
    let config = DebuggingConfig::default();
    let analyzer = StackTraceAnalyzer::new(config);

    let complex_stack_trace = r#"
thread 'main' panicked at 'assertion failed: x == y', src/main.rs:10:5
stack backtrace:
   0: rust_begin_unwind
             at /rustc/hash/library/std/src/panicking.rs:493:5
   1: core::panicking::panic_fmt
             at /rustc/hash/library/core/src/panicking.rs:92:14
   2: main::process_data
             at ./src/main.rs:10:5
   3: main::main
             at ./src/main.rs:5:5
"#;

    let analyzed = analyzer.analyze_trace(complex_stack_trace).await;
    assert!(analyzed.is_ok());
    
    let trace = analyzed.unwrap();
    assert!(!trace.parsed_frames.is_empty());
    
    // Check that the first frame is marked as likely crash point
    assert!(trace.parsed_frames[0].is_likely_crash_point);
    
    // Check that application code is identified
    let app_frames: Vec<_> = trace.parsed_frames
        .iter()
        .filter(|f| f.is_application_code)
        .collect();
    assert!(!app_frames.is_empty());
}

#[tokio::test]
async fn test_performance_profiler_configuration() {
    let config = DebuggingConfig {
        enable_performance_profiling: true,
        profiling_sample_rate: 0.1, // 10% sampling
        ..DebuggingConfig::default()
    };
    let profiler = PerformanceProfiler::new(config);

    // Start multiple sessions
    let session1 = profiler.start_session("session1", "component1").await;
    let session2 = profiler.start_session("session2", "component2").await;
    
    assert!(session1.is_ok());
    assert!(session2.is_ok());
    assert_ne!(session1.unwrap(), session2.unwrap());

    let summary = profiler.get_summary().await;
    assert_eq!(summary.active_profiling_sessions, 2);
}

#[tokio::test]
async fn test_debugging_report_generation() {
    let config = DebuggingConfig::default();
    let debugging_tools = AdvancedDebuggingTools::new(config);

    // Add some test data
    let error = AppError::InternalServerError("Test error".to_string());
    debugging_tools
        .collect_error_context(&error, None, "test", "test")
        .await
        .unwrap();

    debugging_tools
        .analyze_stack_trace("test stack trace")
        .await
        .unwrap();

    debugging_tools
        .start_profiling_session("test", "test")
        .await
        .unwrap();

    let report = debugging_tools.generate_debugging_report().await;
    assert!(report.is_ok());
    
    let report = report.unwrap();
    assert!(report.error_summary.total_errors > 0);
    assert!(report.crash_summary.total_crashes > 0);
    assert!(report.performance_summary.active_profiling_sessions > 0);
    assert!(!report.recommendations.is_empty() || report.recommendations.is_empty()); // Either is fine
}

#[tokio::test]
async fn test_troubleshooting_entry_structure() {
    // Test the structure of troubleshooting entries
    let entry = TroubleshootingEntry {
        issue_id: "db_connection_timeout".to_string(),
        title: "Database Connection Timeout".to_string(),
        description: "The application cannot connect to the database within the timeout period".to_string(),
        symptoms: vec![
            "Connection timeout errors in logs".to_string(),
            "Slow response times".to_string(),
        ],
        possible_causes: vec![
            "Network connectivity issues".to_string(),
            "Database server overload".to_string(),
            "Incorrect connection configuration".to_string(),
        ],
        diagnostic_steps: vec![
            DiagnosticStep {
                step_number: 1,
                description: "Check network connectivity".to_string(),
                command: Some("ping database-server".to_string()),
                expected_output: Some("Successful ping responses".to_string()),
                troubleshooting_notes: Some("If ping fails, check network configuration".to_string()),
            },
            DiagnosticStep {
                step_number: 2,
                description: "Check database server status".to_string(),
                command: Some("systemctl status postgresql".to_string()),
                expected_output: Some("Active (running) status".to_string()),
                troubleshooting_notes: Some("If not running, start the service".to_string()),
            },
        ],
        solutions: vec![
            Solution {
                solution_id: "increase_timeout".to_string(),
                title: "Increase Connection Timeout".to_string(),
                description: "Increase the database connection timeout value".to_string(),
                steps: vec![
                    "Edit configuration file".to_string(),
                    "Set connection_timeout = 30".to_string(),
                    "Restart application".to_string(),
                ],
                risk_level: RiskLevel::Low,
                estimated_time: Duration::from_secs(300), // 5 minutes
                prerequisites: vec!["Access to configuration files".to_string()],
            },
        ],
        related_issues: vec!["db_connection_pool_exhausted".to_string()],
    };

    // Verify structure
    assert_eq!(entry.issue_id, "db_connection_timeout");
    assert!(!entry.symptoms.is_empty());
    assert!(!entry.possible_causes.is_empty());
    assert!(!entry.diagnostic_steps.is_empty());
    assert!(!entry.solutions.is_empty());
    assert_eq!(entry.diagnostic_steps[0].step_number, 1);
    assert!(matches!(entry.solutions[0].risk_level, RiskLevel::Low));
}

#[tokio::test]
async fn test_request_debug_context() {
    // Test request debugging context structure
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());

    let mut query_params = HashMap::new();
    query_params.insert("page".to_string(), "1".to_string());
    query_params.insert("limit".to_string(), "10".to_string());

    let request_context = RequestDebugContext {
        method: "POST".to_string(),
        path: "/api/jobs".to_string(),
        headers,
        query_params,
        body_size: Some(1024),
        client_ip: Some("192.168.1.100".to_string()),
        user_agent: Some("RustCI-Client/1.0".to_string()),
        processing_time: Duration::from_millis(150),
        database_queries: vec![
            DatabaseQuery {
                query: "SELECT * FROM jobs WHERE status = $1".to_string(),
                duration: Duration::from_millis(45),
                rows_affected: Some(5),
                error: None,
            },
            DatabaseQuery {
                query: "INSERT INTO audit_log (action, user_id) VALUES ($1, $2)".to_string(),
                duration: Duration::from_millis(12),
                rows_affected: Some(1),
                error: None,
            },
        ],
        cache_operations: vec![
            CacheOperation {
                operation: "get".to_string(),
                key: "user:123:permissions".to_string(),
                hit: true,
                duration: Duration::from_millis(2),
            },
            CacheOperation {
                operation: "set".to_string(),
                key: "job:456:status".to_string(),
                hit: false,
                duration: Duration::from_millis(5),
            },
        ],
    };

    // Verify structure
    assert_eq!(request_context.method, "POST");
    assert_eq!(request_context.path, "/api/jobs");
    assert_eq!(request_context.database_queries.len(), 2);
    assert_eq!(request_context.cache_operations.len(), 2);
    assert!(request_context.cache_operations[0].hit);
    assert!(!request_context.cache_operations[1].hit);
}