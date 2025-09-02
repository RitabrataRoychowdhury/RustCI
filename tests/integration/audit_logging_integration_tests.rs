use crate::core::networking::security::{AuditAction, AuditEvent};
use crate::core::security::audit_logger::{AuditConfig, EnhancedAuditLogger};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::test]
async fn test_audit_logging_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let log_file_path = temp_dir.path().join("integration_audit.log");
    
    let config = AuditConfig {
        enable_file_logging: true,
        enable_integrity_verification: true,
        enable_real_time_alerts: true,
        enable_correlation_tracking: true,
        log_file_path: log_file_path.to_string_lossy().to_string(),
        flush_interval_seconds: 1,
        max_buffer_size: 1000,
        ..Default::default()
    };
    
    let logger = Arc::new(EnhancedAuditLogger::new(config).unwrap());
    
    // Simulate a complete user session
    let user_id = Uuid::new_v4();
    let session_id = "integration_session_123";
    let ip_address = "192.168.1.100";
    
    // 1. User login
    let login_event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), Some("TestBrowser/1.0".to_string()));
    
    logger.log_event(login_event).await.unwrap();
    
    // 2. User views pipelines
    for i in 0..3 {
        let view_event = AuditEvent::new(
            AuditAction::ViewPipeline,
            "pipeline".to_string(),
            Some(user_id),
            Some(session_id.to_string()),
        )
        .with_resource_id(format!("pipeline_{}", i))
        .with_client_info(Some(ip_address.to_string()), Some("TestBrowser/1.0".to_string()));
        
        logger.log_event(view_event).await.unwrap();
    }
    
    // 3. User creates a pipeline
    let create_event = AuditEvent::new(
        AuditAction::CreatePipeline,
        "pipeline".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_resource_id("new_pipeline_123".to_string())
    .with_details("name".to_string(), serde_json::Value::String("Test Pipeline".to_string()))
    .with_client_info(Some(ip_address.to_string()), Some("TestBrowser/1.0".to_string()));
    
    logger.log_event(create_event).await.unwrap();
    
    // 4. User executes the pipeline
    let execute_event = AuditEvent::new(
        AuditAction::ExecutePipeline,
        "pipeline".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_resource_id("new_pipeline_123".to_string())
    .with_client_info(Some(ip_address.to_string()), Some("TestBrowser/1.0".to_string()));
    
    logger.log_event(execute_event).await.unwrap();
    
    // 5. User logs out
    let logout_event = AuditEvent::new(
        AuditAction::Logout,
        "authentication".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), Some("TestBrowser/1.0".to_string()));
    
    logger.log_event(logout_event).await.unwrap();
    
    // Wait for all events to be processed and flushed
    sleep(std::time::Duration::from_millis(1500)).await;
    
    // Verify metrics
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 6);
    assert_eq!(metrics.failed_events, 0);
    assert!(metrics.events_by_action.get("Login").unwrap_or(&0) >= &1);
    assert!(metrics.events_by_action.get("ViewPipeline").unwrap_or(&0) >= &3);
    assert!(metrics.events_by_action.get("CreatePipeline").unwrap_or(&0) >= &1);
    assert!(metrics.events_by_action.get("ExecutePipeline").unwrap_or(&0) >= &1);
    assert!(metrics.events_by_action.get("Logout").unwrap_or(&0) >= &1);
    
    // Verify integrity
    let integrity_report = logger.verify_integrity().await.unwrap();
    assert!(integrity_report.overall_valid);
    assert!(integrity_report.chain_valid);
    assert_eq!(integrity_report.total_events, 6);
    assert_eq!(integrity_report.valid_events, 6);
    assert!(integrity_report.failed_events.is_empty());
    
    // Verify file logging
    assert!(log_file_path.exists());
    let log_content = tokio::fs::read_to_string(&log_file_path).await.unwrap();
    assert!(!log_content.is_empty());
    assert!(log_content.contains("Login"));
    assert!(log_content.contains("ViewPipeline"));
    assert!(log_content.contains("CreatePipeline"));
    assert!(log_content.contains("ExecutePipeline"));
    assert!(log_content.contains("Logout"));
    
    // Verify compliance report
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now() + Duration::minutes(1);
    let compliance_report = logger.generate_compliance_report(start_time, end_time).await.unwrap();
    
    assert_eq!(compliance_report.total_events, 6);
    assert_eq!(compliance_report.failed_events, 0);
    assert_eq!(compliance_report.authentication_events, 2); // Login + Logout
    assert_eq!(compliance_report.data_access_events, 3); // 3 ViewPipeline events
    assert!(compliance_report.integrity_verified);
    assert!(compliance_report.compliance_score > 90.0); // Should be high with no failures
    
    // Query events by different criteria
    let user_events = logger.query_events(
        Some(user_id),
        None,
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(user_events.len(), 6);
    
    let login_events = logger.query_events(
        None,
        Some(AuditAction::Login),
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(login_events.len(), 1);
    
    let pipeline_events = logger.query_events(
        None,
        Some(AuditAction::ViewPipeline),
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(pipeline_events.len(), 3);
}

#[tokio::test]
async fn test_security_alert_integration() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_real_time_alerts: true,
        enable_correlation_tracking: true,
        ..Default::default()
    };
    
    let logger = Arc::new(EnhancedAuditLogger::new(config).unwrap());
    
    let attacker_ip = "192.168.1.200";
    
    // Simulate brute force attack
    for i in 0..10 {
        let failed_login = AuditEvent::new(
            AuditAction::Login,
            "authentication".to_string(),
            None,
            Some(format!("attack_session_{}", i)),
        )
        .with_error("Invalid credentials".to_string())
        .with_client_info(Some(attacker_ip.to_string()), Some("AttackBot/1.0".to_string()));
        
        logger.log_event(failed_login).await.unwrap();
        
        // Small delay between attempts
        sleep(std::time::Duration::from_millis(50)).await;
    }
    
    // Wait for processing and alert detection
    sleep(std::time::Duration::from_millis(500)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 10);
    assert_eq!(metrics.failed_events, 10);
    assert!(metrics.alert_count > 0); // Should have triggered security alerts
    
    // Verify integrity is maintained even with attack events
    let integrity_report = logger.verify_integrity().await.unwrap();
    assert!(integrity_report.overall_valid);
    assert_eq!(integrity_report.total_events, 10);
    assert_eq!(integrity_report.valid_events, 10);
}

#[tokio::test]
async fn test_concurrent_audit_logging() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_integrity_verification: true,
        max_buffer_size: 1000,
        ..Default::default()
    };
    
    let logger = Arc::new(EnhancedAuditLogger::new(config).unwrap());
    
    // Simulate concurrent users
    let mut handles = Vec::new();
    
    for user_num in 0..5 {
        let logger_clone = Arc::clone(&logger);
        let handle = tokio::spawn(async move {
            let user_id = Uuid::new_v4();
            let session_id = format!("concurrent_session_{}", user_num);
            
            // Each user performs multiple actions
            for action_num in 0..10 {
                let event = match action_num % 4 {
                    0 => AuditEvent::new(
                        AuditAction::Login,
                        "authentication".to_string(),
                        Some(user_id),
                        Some(session_id.clone()),
                    ),
                    1 => AuditEvent::new(
                        AuditAction::ViewPipeline,
                        "pipeline".to_string(),
                        Some(user_id),
                        Some(session_id.clone()),
                    )
                    .with_resource_id(format!("pipeline_{}", action_num)),
                    2 => AuditEvent::new(
                        AuditAction::CreatePipeline,
                        "pipeline".to_string(),
                        Some(user_id),
                        Some(session_id.clone()),
                    )
                    .with_resource_id(format!("new_pipeline_{}_{}", user_num, action_num)),
                    _ => AuditEvent::new(
                        AuditAction::Logout,
                        "authentication".to_string(),
                        Some(user_id),
                        Some(session_id.clone()),
                    ),
                };
                
                logger_clone.log_event(event).await.unwrap();
                
                // Small delay to simulate real usage
                sleep(std::time::Duration::from_millis(10)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Wait for background processing
    sleep(std::time::Duration::from_millis(500)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 50); // 5 users * 10 actions each
    assert_eq!(metrics.failed_events, 0);
    
    // Verify integrity is maintained under concurrent load
    let integrity_report = logger.verify_integrity().await.unwrap();
    assert!(integrity_report.overall_valid);
    assert!(integrity_report.chain_valid);
    assert_eq!(integrity_report.total_events, 50);
    assert_eq!(integrity_report.valid_events, 50);
    assert!(integrity_report.failed_events.is_empty());
    
    // Verify all events can be queried
    let all_events = logger.query_events(None, None, None, None, None).await.unwrap();
    assert_eq!(all_events.len(), 50);
}

#[tokio::test]
async fn test_audit_log_persistence_and_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let log_file_path = temp_dir.path().join("persistence_audit.log");
    
    let config = AuditConfig {
        enable_file_logging: true,
        log_file_path: log_file_path.to_string_lossy().to_string(),
        flush_interval_seconds: 1,
        ..Default::default()
    };
    
    // First logger instance
    {
        let logger = EnhancedAuditLogger::new(config.clone()).unwrap();
        
        // Log some events
        for i in 0..5 {
            let event = AuditEvent::new(
                AuditAction::ViewPipeline,
                "pipeline".to_string(),
                Some(Uuid::new_v4()),
                Some(format!("session_{}", i)),
            );
            logger.log_event(event).await.unwrap();
        }
        
        // Wait for flush
        sleep(std::time::Duration::from_millis(1200)).await;
    } // Logger goes out of scope
    
    // Verify file was written
    assert!(log_file_path.exists());
    let log_content = tokio::fs::read_to_string(&log_file_path).await.unwrap();
    assert!(!log_content.is_empty());
    
    // Count lines in log file
    let line_count = log_content.lines().count();
    assert_eq!(line_count, 5);
    
    // Create second logger instance (simulating restart)
    let logger2 = EnhancedAuditLogger::new(config).unwrap();
    
    // Log more events
    for i in 5..8 {
        let event = AuditEvent::new(
            AuditAction::CreatePipeline,
            "pipeline".to_string(),
            Some(Uuid::new_v4()),
            Some(format!("session_{}", i)),
        );
        logger2.log_event(event).await.unwrap();
    }
    
    // Wait for flush
    sleep(std::time::Duration::from_millis(1200)).await;
    
    // Verify both sets of events are in the file
    let final_log_content = tokio::fs::read_to_string(&log_file_path).await.unwrap();
    let final_line_count = final_log_content.lines().count();
    assert_eq!(final_line_count, 8);
    
    // Verify content contains both ViewPipeline and CreatePipeline events
    assert!(final_log_content.contains("ViewPipeline"));
    assert!(final_log_content.contains("CreatePipeline"));
}

#[tokio::test]
async fn test_audit_log_analysis_and_alerting() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_real_time_alerts: true,
        enable_correlation_tracking: true,
        ..Default::default()
    };
    
    let logger = Arc::new(EnhancedAuditLogger::new(config).unwrap());
    
    let admin_user = Uuid::new_v4();
    let regular_user = Uuid::new_v4();
    
    // Simulate suspicious admin activity
    let suspicious_events = vec![
        AuditAction::CreateUser,
        AuditAction::UpdateUser,
        AuditAction::DeleteUser,
        AuditAction::SystemConfiguration,
    ];
    
    for action in suspicious_events {
        let event = AuditEvent::new(
            action,
            "admin".to_string(),
            Some(admin_user),
            Some("admin_session".to_string()),
        )
        .with_client_info(Some("192.168.1.10".to_string()), Some("AdminTool/1.0".to_string()));
        
        logger.log_event(event).await.unwrap();
    }
    
    // Simulate normal user activity
    for i in 0..10 {
        let event = AuditEvent::new(
            AuditAction::ViewPipeline,
            "pipeline".to_string(),
            Some(regular_user),
            Some("user_session".to_string()),
        )
        .with_resource_id(format!("pipeline_{}", i))
        .with_client_info(Some("192.168.1.50".to_string()), Some("WebBrowser/1.0".to_string()));
        
        logger.log_event(event).await.unwrap();
    }
    
    // Wait for processing
    sleep(std::time::Duration::from_millis(300)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 14);
    assert_eq!(metrics.failed_events, 0);
    
    // Generate compliance report
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now() + Duration::minutes(1);
    let compliance_report = logger.generate_compliance_report(start_time, end_time).await.unwrap();
    
    assert_eq!(compliance_report.total_events, 14);
    assert_eq!(compliance_report.privilege_changes, 3); // CreateUser, UpdateUser, DeleteUser
    assert_eq!(compliance_report.data_access_events, 10); // ViewPipeline events
    assert!(compliance_report.integrity_verified);
    
    // Should have recommendations due to high privilege changes
    assert!(!compliance_report.recommendations.is_empty());
    
    // Query admin events specifically
    let admin_events = logger.query_events(
        Some(admin_user),
        None,
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(admin_events.len(), 4);
    
    // Query user management events
    let user_mgmt_events = logger.query_events(
        None,
        Some(AuditAction::CreateUser),
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(user_mgmt_events.len(), 1);
}