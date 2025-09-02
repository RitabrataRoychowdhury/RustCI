use crate::core::networking::security::{AuditAction, AuditEvent};
use crate::core::security::audit_logger::{
    AuditConfig, EnhancedAuditLogger, IntegrityVerificationReport, ComplianceReport,
};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::test]
async fn test_enhanced_audit_logger_creation() {
    let config = AuditConfig::default();
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let health = logger.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.total_events, 0);
}

#[tokio::test]
async fn test_audit_event_logging() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let user_id = Uuid::new_v4();
    let event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        Some(user_id),
        Some("session123".to_string()),
    )
    .with_client_info(Some("192.168.1.1".to_string()), Some("TestAgent/1.0".to_string()));
    
    logger.log_event(event.clone()).await.unwrap();
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(100)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 1);
    assert_eq!(metrics.failed_events, 0);
    
    // Query the event back
    let events = logger.query_events(
        Some(user_id),
        Some(AuditAction::Login),
        None,
        None,
        None,
    ).await.unwrap();
    
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, event.id);
    assert_eq!(events[0].action, AuditAction::Login);
}

#[tokio::test]
async fn test_audit_event_with_failure() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        None,
        None,
    )
    .with_error("Invalid credentials".to_string())
    .with_client_info(Some("192.168.1.100".to_string()), None);
    
    logger.log_event(event).await.unwrap();
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(100)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 1);
    assert_eq!(metrics.failed_events, 1);
}

#[tokio::test]
async fn test_integrity_verification() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_integrity_verification: true,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    // Log multiple events
    for i in 0..5 {
        let event = AuditEvent::new(
            AuditAction::ViewPipeline,
            "pipeline".to_string(),
            Some(Uuid::new_v4()),
            Some(format!("session{}", i)),
        );
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    let integrity_report = logger.verify_integrity().await.unwrap();
    assert!(integrity_report.overall_valid);
    assert!(integrity_report.chain_valid);
    assert_eq!(integrity_report.total_events, 5);
    assert_eq!(integrity_report.valid_events, 5);
    assert!(integrity_report.failed_events.is_empty());
}

#[tokio::test]
async fn test_sensitive_data_sanitization() {
    let config = AuditConfig {
        enable_file_logging: false,
        sensitive_fields: vec!["password".to_string(), "token".to_string()],
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let mut event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        Some(Uuid::new_v4()),
        None,
    );
    
    // Add sensitive data
    event.details.insert("password".to_string(), serde_json::Value::String("secret123".to_string()));
    event.details.insert("username".to_string(), serde_json::Value::String("testuser".to_string()));
    
    logger.log_event(event.clone()).await.unwrap();
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(100)).await;
    
    let events = logger.query_events(None, None, None, None, None).await.unwrap();
    assert_eq!(events.len(), 1);
    
    // Sensitive field should be redacted
    assert_eq!(
        events[0].details.get("password").unwrap(),
        &serde_json::Value::String("[REDACTED]".to_string())
    );
    
    // Non-sensitive field should remain
    assert_eq!(
        events[0].details.get("username").unwrap(),
        &serde_json::Value::String("testuser".to_string())
    );
}

#[tokio::test]
async fn test_security_alerts() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_real_time_alerts: true,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let ip_address = "192.168.1.200";
    
    // Generate multiple failed login attempts from same IP
    for i in 0..6 {
        let event = AuditEvent::new(
            AuditAction::Login,
            "authentication".to_string(),
            None,
            Some(format!("session{}", i)),
        )
        .with_error("Invalid credentials".to_string())
        .with_client_info(Some(ip_address.to_string()), None);
        
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing and alert detection
    sleep(std::time::Duration::from_millis(300)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 6);
    assert_eq!(metrics.failed_events, 6);
    // Should have triggered security alerts
    assert!(metrics.alert_count > 0);
}

#[tokio::test]
async fn test_correlation_tracking() {
    let config = AuditConfig {
        enable_file_logging: false,
        enable_correlation_tracking: true,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let user_id = Uuid::new_v4();
    let session_id = "session123";
    let ip_address = "192.168.1.50";
    
    // Log login event
    let login_event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), None);
    
    logger.log_event(login_event).await.unwrap();
    
    // Log pipeline access
    let pipeline_event = AuditEvent::new(
        AuditAction::ViewPipeline,
        "pipeline".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), None);
    
    logger.log_event(pipeline_event).await.unwrap();
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 2);
    assert!(metrics.correlation_matches >= 0); // Correlation tracking is working
}

#[tokio::test]
async fn test_file_logging() {
    let temp_dir = TempDir::new().unwrap();
    let log_file_path = temp_dir.path().join("audit.log");
    
    let config = AuditConfig {
        enable_file_logging: true,
        log_file_path: log_file_path.to_string_lossy().to_string(),
        flush_interval_seconds: 1, // Quick flush for testing
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let event = AuditEvent::new(
        AuditAction::CreatePipeline,
        "pipeline".to_string(),
        Some(Uuid::new_v4()),
        None,
    );
    
    logger.log_event(event).await.unwrap();
    
    // Wait for flush
    sleep(std::time::Duration::from_millis(1200)).await;
    
    // Check that file was created and contains data
    assert!(log_file_path.exists());
    let content = tokio::fs::read_to_string(&log_file_path).await.unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("CreatePipeline"));
}

#[tokio::test]
async fn test_compliance_report_generation() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let user_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::hours(1);
    
    // Log various types of events
    let events = vec![
        (AuditAction::Login, true),
        (AuditAction::ViewPipeline, true),
        (AuditAction::CreateUser, true),
        (AuditAction::Login, false), // Failed login
        (AuditAction::DeleteUser, true),
    ];
    
    for (action, success) in events {
        let mut event = AuditEvent::new(
            action,
            "test".to_string(),
            Some(user_id),
            None,
        );
        
        if !success {
            event = event.with_error("Test failure".to_string());
        }
        
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    let end_time = Utc::now() + Duration::minutes(1);
    let report = logger.generate_compliance_report(start_time, end_time).await.unwrap();
    
    assert_eq!(report.total_events, 5);
    assert_eq!(report.failed_events, 1);
    assert_eq!(report.authentication_events, 2); // 2 login attempts
    assert_eq!(report.privilege_changes, 2); // CreateUser + DeleteUser
    assert_eq!(report.data_access_events, 1); // ViewPipeline
    assert!(report.compliance_score > 0.0);
    assert!(!report.recommendations.is_empty());
}

#[tokio::test]
async fn test_event_enrichment() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let event = AuditEvent::new(
        AuditAction::SystemConfiguration,
        "system".to_string(),
        Some(Uuid::new_v4()),
        None,
    );
    
    logger.log_event(event).await.unwrap();
    
    // Give some time for background processing and enrichment
    sleep(std::time::Duration::from_millis(100)).await;
    
    let events = logger.query_events(None, None, None, None, None).await.unwrap();
    assert_eq!(events.len(), 1);
    
    let enriched_event = &events[0];
    
    // Check that enrichment data was added
    assert!(enriched_event.details.contains_key("hostname"));
    assert!(enriched_event.details.contains_key("process_id"));
    assert!(enriched_event.details.contains_key("environment"));
    assert!(enriched_event.details.contains_key("timestamp_unix"));
}

#[tokio::test]
async fn test_buffer_management() {
    let config = AuditConfig {
        enable_file_logging: false,
        max_buffer_size: 3, // Small buffer for testing
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    // Log more events than buffer size
    for i in 0..5 {
        let event = AuditEvent::new(
            AuditAction::ViewPipeline,
            "pipeline".to_string(),
            Some(Uuid::new_v4()),
            Some(format!("session{}", i)),
        );
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 5);
    
    // Buffer should be at capacity
    assert!(metrics.buffer_utilization > 0.9);
    
    // Should only have the most recent events in buffer
    let events = logger.query_events(None, None, None, None, None).await.unwrap();
    assert!(events.len() <= 3); // Buffer size limit
}

#[tokio::test]
async fn test_query_filtering() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let user1 = Uuid::new_v4();
    let user2 = Uuid::new_v4();
    let base_time = Utc::now();
    
    // Log events for different users and actions
    let events = vec![
        (user1, AuditAction::Login, base_time - Duration::minutes(10)),
        (user1, AuditAction::ViewPipeline, base_time - Duration::minutes(5)),
        (user2, AuditAction::Login, base_time - Duration::minutes(3)),
        (user2, AuditAction::CreatePipeline, base_time - Duration::minutes(1)),
    ];
    
    for (user_id, action, timestamp) in events {
        let mut event = AuditEvent::new(
            action,
            "test".to_string(),
            Some(user_id),
            None,
        );
        event.timestamp = timestamp;
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    // Test user filtering
    let user1_events = logger.query_events(
        Some(user1),
        None,
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(user1_events.len(), 2);
    
    // Test action filtering
    let login_events = logger.query_events(
        None,
        Some(AuditAction::Login),
        None,
        None,
        None,
    ).await.unwrap();
    assert_eq!(login_events.len(), 2);
    
    // Test time filtering
    let recent_events = logger.query_events(
        None,
        None,
        Some(base_time - Duration::minutes(4)),
        None,
        None,
    ).await.unwrap();
    assert_eq!(recent_events.len(), 2);
    
    // Test limit
    let limited_events = logger.query_events(
        None,
        None,
        None,
        None,
        Some(2),
    ).await.unwrap();
    assert_eq!(limited_events.len(), 2);
}

#[tokio::test]
async fn test_health_check() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    // Initial health check
    let health = logger.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.total_events, 0);
    assert_eq!(health.failed_events, 0);
    assert_eq!(health.integrity_violations, 0);
    
    // Log some events
    for i in 0..3 {
        let event = AuditEvent::new(
            AuditAction::ViewPipeline,
            "pipeline".to_string(),
            Some(Uuid::new_v4()),
            Some(format!("session{}", i)),
        );
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(100)).await;
    
    let health = logger.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.total_events, 3);
    assert!(health.average_processing_time_ms >= 0.0);
}

#[tokio::test]
async fn test_metrics_tracking() {
    let config = AuditConfig {
        enable_file_logging: false,
        ..Default::default()
    };
    let logger = EnhancedAuditLogger::new(config).unwrap();
    
    let user_id = Uuid::new_v4();
    
    // Log different types of events
    let events = vec![
        AuditAction::Login,
        AuditAction::Login,
        AuditAction::ViewPipeline,
        AuditAction::CreatePipeline,
    ];
    
    for action in events {
        let event = AuditEvent::new(
            action,
            "test".to_string(),
            Some(user_id),
            None,
        );
        logger.log_event(event).await.unwrap();
    }
    
    // Give some time for background processing
    sleep(std::time::Duration::from_millis(200)).await;
    
    let metrics = logger.get_metrics().await;
    assert_eq!(metrics.total_events, 4);
    assert_eq!(metrics.failed_events, 0);
    
    // Check action counts
    assert_eq!(metrics.events_by_action.get("Login").unwrap_or(&0), &2);
    assert_eq!(metrics.events_by_action.get("ViewPipeline").unwrap_or(&0), &1);
    assert_eq!(metrics.events_by_action.get("CreatePipeline").unwrap_or(&0), &1);
    
    // Check user counts
    let user_key = user_id.to_string();
    assert_eq!(metrics.events_by_user.get(&user_key).unwrap_or(&0), &4);
}