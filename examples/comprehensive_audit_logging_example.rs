use rustci::core::networking::security::{AuditAction, AuditEvent};
use rustci::core::security::audit_logger::{AuditConfig, EnhancedAuditLogger};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🔒 Comprehensive Audit Logging System Demo");
    println!("==========================================");
    
    // Create temporary directory for audit logs
    let temp_dir = TempDir::new()?;
    let log_file_path = temp_dir.path().join("demo_audit.log");
    
    // Configure enhanced audit logger
    let config = AuditConfig {
        enable_file_logging: true,
        enable_integrity_verification: true,
        enable_real_time_alerts: true,
        enable_correlation_tracking: true,
        log_file_path: log_file_path.to_string_lossy().to_string(),
        flush_interval_seconds: 2,
        max_buffer_size: 1000,
        retention_days: 365,
        compression_enabled: true,
        encryption_enabled: true,
        sensitive_fields: vec![
            "password".to_string(),
            "token".to_string(),
            "secret".to_string(),
            "api_key".to_string(),
        ],
        ..Default::default()
    };
    
    println!("📋 Configuration:");
    println!("  - File logging: {}", config.enable_file_logging);
    println!("  - Integrity verification: {}", config.enable_integrity_verification);
    println!("  - Real-time alerts: {}", config.enable_real_time_alerts);
    println!("  - Correlation tracking: {}", config.enable_correlation_tracking);
    println!("  - Log file: {}", config.log_file_path);
    println!("  - Buffer size: {}", config.max_buffer_size);
    println!();
    
    // Create enhanced audit logger
    let logger = Arc::new(EnhancedAuditLogger::new(config)?);
    println!("✅ Enhanced audit logger initialized");
    
    // Demo 1: Normal user session
    println!("\n🎭 Demo 1: Normal User Session");
    println!("------------------------------");
    
    let user_id = Uuid::new_v4();
    let session_id = "demo_session_001";
    let ip_address = "192.168.1.100";
    
    // User login
    let login_event = AuditEvent::new(
        AuditAction::Login,
        "authentication".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), Some("DemoApp/1.0".to_string()));
    
    logger.log_event(login_event).await?;
    println!("  ✓ User login logged");
    
    // User activities
    let activities = vec![
        ("View Pipeline", AuditAction::ViewPipeline, "pipeline_001"),
        ("Create Pipeline", AuditAction::CreatePipeline, "pipeline_002"),
        ("Execute Pipeline", AuditAction::ExecutePipeline, "pipeline_002"),
        ("View Pipeline", AuditAction::ViewPipeline, "pipeline_003"),
    ];
    
    for (desc, action, resource_id) in activities {
        let event = AuditEvent::new(
            action,
            "pipeline".to_string(),
            Some(user_id),
            Some(session_id.to_string()),
        )
        .with_resource_id(resource_id.to_string())
        .with_client_info(Some(ip_address.to_string()), Some("DemoApp/1.0".to_string()));
        
        logger.log_event(event).await?;
        println!("  ✓ {} logged", desc);
        
        // Small delay to simulate real usage
        sleep(std::time::Duration::from_millis(100)).await;
    }
    
    // User logout
    let logout_event = AuditEvent::new(
        AuditAction::Logout,
        "authentication".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    )
    .with_client_info(Some(ip_address.to_string()), Some("DemoApp/1.0".to_string()));
    
    logger.log_event(logout_event).await?;
    println!("  ✓ User logout logged");
    
    // Demo 2: Security incident simulation
    println!("\n🚨 Demo 2: Security Incident Simulation");
    println!("----------------------------------------");
    
    let attacker_ip = "192.168.1.200";
    
    // Simulate brute force attack
    println!("  Simulating brute force attack from {}", attacker_ip);
    for i in 0..8 {
        let failed_login = AuditEvent::new(
            AuditAction::Login,
            "authentication".to_string(),
            None,
            Some(format!("attack_session_{}", i)),
        )
        .with_error("Invalid credentials".to_string())
        .with_client_info(Some(attacker_ip.to_string()), Some("AttackBot/1.0".to_string()));
        
        logger.log_event(failed_login).await?;
        sleep(std::time::Duration::from_millis(50)).await;
    }
    println!("  ✓ {} failed login attempts logged", 8);
    
    // Demo 3: Administrative actions
    println!("\n👑 Demo 3: Administrative Actions");
    println!("----------------------------------");
    
    let admin_user = Uuid::new_v4();
    let admin_session = "admin_session_001";
    let admin_ip = "192.168.1.10";
    
    let admin_actions = vec![
        ("Create User", AuditAction::CreateUser, "new_user_001"),
        ("Update User", AuditAction::UpdateUser, "user_002"),
        ("System Configuration", AuditAction::SystemConfiguration, "config_001"),
        ("Delete User", AuditAction::DeleteUser, "old_user_003"),
    ];
    
    for (desc, action, resource_id) in admin_actions {
        let event = AuditEvent::new(
            action,
            "administration".to_string(),
            Some(admin_user),
            Some(admin_session.to_string()),
        )
        .with_resource_id(resource_id.to_string())
        .with_client_info(Some(admin_ip.to_string()), Some("AdminPanel/1.0".to_string()));
        
        logger.log_event(event).await?;
        println!("  ✓ {} logged", desc);
        sleep(std::time::Duration::from_millis(100)).await;
    }
    
    // Demo 4: Sensitive data handling
    println!("\n🔐 Demo 4: Sensitive Data Handling");
    println!("-----------------------------------");
    
    let mut sensitive_event = AuditEvent::new(
        AuditAction::ChangePassword,
        "user_management".to_string(),
        Some(user_id),
        Some(session_id.to_string()),
    );
    
    // Add sensitive data that should be sanitized
    sensitive_event.details.insert("password".to_string(), serde_json::Value::String("secret123".to_string()));
    sensitive_event.details.insert("old_password".to_string(), serde_json::Value::String("oldSecret456".to_string()));
    sensitive_event.details.insert("username".to_string(), serde_json::Value::String("demo_user".to_string()));
    
    logger.log_event(sensitive_event).await?;
    println!("  ✓ Password change event logged (sensitive data will be sanitized)");
    
    // Wait for all events to be processed
    println!("\n⏳ Processing events...");
    sleep(std::time::Duration::from_millis(500)).await;
    
    // Demo 5: Metrics and analysis
    println!("\n📊 Demo 5: Metrics and Analysis");
    println!("--------------------------------");
    
    let metrics = logger.get_metrics().await;
    println!("  Total events: {}", metrics.total_events);
    println!("  Failed events: {}", metrics.failed_events);
    println!("  Security alerts: {}", metrics.alert_count);
    println!("  Buffer utilization: {:.1}%", metrics.buffer_utilization * 100.0);
    println!("  Average processing time: {:.2}ms", metrics.average_processing_time_ms);
    
    println!("\n  Events by action:");
    for (action, count) in &metrics.events_by_action {
        println!("    {}: {}", action, count);
    }
    
    println!("\n  Events by user:");
    for (user, count) in &metrics.events_by_user {
        println!("    {}: {}", user, count);
    }
    
    // Demo 6: Integrity verification
    println!("\n🔍 Demo 6: Integrity Verification");
    println!("----------------------------------");
    
    let integrity_report = logger.verify_integrity().await?;
    println!("  Overall valid: {}", integrity_report.overall_valid);
    println!("  Chain valid: {}", integrity_report.chain_valid);
    println!("  Total events: {}", integrity_report.total_events);
    println!("  Valid events: {}", integrity_report.valid_events);
    println!("  Failed events: {}", integrity_report.failed_events.len());
    println!("  Genesis hash: {}", &integrity_report.genesis_hash[..16]);
    println!("  Sequence number: {}", integrity_report.sequence_number);
    
    if integrity_report.overall_valid {
        println!("  ✅ Audit log integrity verified - no tampering detected");
    } else {
        println!("  ❌ Audit log integrity compromised - potential tampering detected");
    }
    
    // Demo 7: Compliance reporting
    println!("\n📋 Demo 7: Compliance Reporting");
    println!("--------------------------------");
    
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now() + Duration::minutes(1);
    let compliance_report = logger.generate_compliance_report(start_time, end_time).await?;
    
    println!("  Report period: {} to {}", 
             start_time.format("%Y-%m-%d %H:%M:%S"),
             end_time.format("%Y-%m-%d %H:%M:%S"));
    println!("  Total events: {}", compliance_report.total_events);
    println!("  Failed events: {}", compliance_report.failed_events);
    println!("  Authentication events: {}", compliance_report.authentication_events);
    println!("  Privilege changes: {}", compliance_report.privilege_changes);
    println!("  Data access events: {}", compliance_report.data_access_events);
    println!("  Security alerts: {}", compliance_report.security_alerts);
    println!("  Integrity verified: {}", compliance_report.integrity_verified);
    println!("  Compliance score: {:.1}%", compliance_report.compliance_score);
    
    println!("\n  Recommendations:");
    for (i, recommendation) in compliance_report.recommendations.iter().enumerate() {
        println!("    {}. {}", i + 1, recommendation);
    }
    
    // Demo 8: Event querying
    println!("\n🔍 Demo 8: Event Querying");
    println!("-------------------------");
    
    // Query all events
    let all_events = logger.query_events(None, None, None, None, None).await?;
    println!("  Total events in buffer: {}", all_events.len());
    
    // Query events by user
    let user_events = logger.query_events(Some(user_id), None, None, None, None).await?;
    println!("  Events for user {}: {}", user_id, user_events.len());
    
    // Query login events
    let login_events = logger.query_events(None, Some(AuditAction::Login), None, None, None).await?;
    println!("  Login events: {}", login_events.len());
    
    // Query failed events (by examining the events)
    let failed_events: Vec<_> = all_events.iter().filter(|e| !e.success).collect();
    println!("  Failed events: {}", failed_events.len());
    
    // Query recent events (last 10 minutes)
    let recent_start = Utc::now() - Duration::minutes(10);
    let recent_events = logger.query_events(None, None, Some(recent_start), None, None).await?;
    println!("  Recent events (last 10 min): {}", recent_events.len());
    
    // Demo 9: Health check
    println!("\n🏥 Demo 9: Health Check");
    println!("-----------------------");
    
    let health = logger.health_check().await?;
    println!("  System healthy: {}", health.is_healthy);
    println!("  Buffer utilization: {:.1}%", health.buffer_utilization * 100.0);
    println!("  Total events: {}", health.total_events);
    println!("  Failed events: {}", health.failed_events);
    println!("  Integrity violations: {}", health.integrity_violations);
    println!("  Disk usage: {} bytes", health.disk_usage_bytes);
    
    if let Some(last_flush) = health.last_flush_time {
        println!("  Last flush: {}", last_flush.format("%Y-%m-%d %H:%M:%S"));
    }
    
    // Wait for final flush to disk
    println!("\n💾 Flushing to disk...");
    sleep(std::time::Duration::from_millis(2500)).await;
    
    // Demo 10: File analysis
    println!("\n📁 Demo 10: File Analysis");
    println!("-------------------------");
    
    if log_file_path.exists() {
        let log_content = tokio::fs::read_to_string(&log_file_path).await?;
        let line_count = log_content.lines().count();
        let file_size = log_content.len();
        
        println!("  Log file exists: ✅");
        println!("  File size: {} bytes", file_size);
        println!("  Number of log entries: {}", line_count);
        
        // Show sample log entries
        println!("\n  Sample log entries:");
        for (i, line) in log_content.lines().take(3).enumerate() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(action) = parsed.get("event").and_then(|e| e.get("action")) {
                    if let Some(timestamp) = parsed.get("event").and_then(|e| e.get("timestamp")) {
                        println!("    {}. {} at {}", i + 1, action, timestamp);
                    }
                }
            }
        }
        
        if line_count > 3 {
            println!("    ... and {} more entries", line_count - 3);
        }
    } else {
        println!("  Log file not found (may not have been flushed yet)");
    }
    
    println!("\n🎉 Demo completed successfully!");
    println!("===============================");
    println!("The enhanced audit logging system provides:");
    println!("  ✓ Tamper-proof logging with cryptographic integrity");
    println!("  ✓ Real-time security alert detection");
    println!("  ✓ Comprehensive correlation tracking");
    println!("  ✓ Sensitive data sanitization");
    println!("  ✓ Compliance reporting and analysis");
    println!("  ✓ High-performance concurrent logging");
    println!("  ✓ Persistent storage with automatic cleanup");
    println!("  ✓ Flexible querying and analysis capabilities");
    
    Ok(())
}