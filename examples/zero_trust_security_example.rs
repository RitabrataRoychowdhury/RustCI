//! Zero-Trust Security Example
//! 
//! Demonstrates comprehensive zero-trust security with identity verification,
//! policy enforcement, continuous monitoring, and adaptive threat response.

use std::collections::HashMap;
use std::time::Duration;
use tokio;
use tracing::{info, warn, error};

use RustAutoDevOps::core::networking::valkyrie::security::zero_trust::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("🔒 Starting Zero-Trust Security Example");
    
    // Initialize Zero-Trust manager
    let zero_trust = initialize_zero_trust_manager().await?;
    
    // Demonstrate different security scenarios
    demo_user_authentication(&zero_trust).await?;
    demo_service_authentication(&zero_trust).await?;
    demo_policy_enforcement(&zero_trust).await?;
    demo_risk_based_access(&zero_trust).await?;
    demo_continuous_monitoring(&zero_trust).await?;
    demo_threat_detection(&zero_trust).await?;
    demo_session_management(&zero_trust).await?;
    
    // Show final security metrics
    show_security_metrics(&zero_trust).await?;
    
    info!("✅ Zero-Trust Security Example completed successfully");
    Ok(())
}

/// Initialize Zero-Trust manager with enterprise configuration
async fn initialize_zero_trust_manager() -> Result<ZeroTrustManager, Box<dyn std::error::Error>> {
    info!("🛡️  Initializing Zero-Trust Security Manager...");
    
    let config = ZeroTrustConfig {
        default_trust_score: 0.5,
        trust_decay_rate: 0.05, // 5% decay per hour
        min_trust_threshold: 0.3,
        reauth_interval: Duration::from_hours(4),
        session_timeout: Duration::from_hours(12),
        risk_assessment_interval: Duration::from_minutes(10),
        enable_behavioral_analytics: true,
        enable_threat_intelligence: true,
        max_sessions_per_identity: 3,
    };
    
    let zero_trust = ZeroTrustManager::with_config(config);
    
    info!("  ✅ Zero-Trust manager initialized");
    info!("  🎯 Default trust score: 0.5");
    info!("  ⏱️  Session timeout: 12 hours");
    info!("  🔍 Behavioral analytics: enabled");
    info!("  🛡️  Threat intelligence: enabled");
    
    Ok(zero_trust)
}

/// Demonstrate user authentication with different scenarios
async fn demo_user_authentication(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("👤 Demo: User Authentication Scenarios");
    
    let user_scenarios = vec![
        ("alice", "trusted_location", "corporate_device", VerificationLevel::Strong),
        ("bob", "home_location", "personal_device", VerificationLevel::Enhanced),
        ("charlie", "unknown_location", "new_device", VerificationLevel::Basic),
        ("admin", "secure_location", "admin_device", VerificationLevel::Maximum),
    ];
    
    for (username, location, device, expected_level) in user_scenarios {
        info!("  🔐 Authenticating user: {}", username);
        
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        credentials.insert("password".to_string(), format!("{}_secure_password", username));
        credentials.insert("verification_level".to_string(), format!("{:?}", expected_level));
        
        let mut request_context = HashMap::new();
        request_context.insert("location".to_string(), location.to_string());
        request_context.insert("device_id".to_string(), device.to_string());
        request_context.insert("ip_address".to_string(), "192.168.1.100".to_string());
        request_context.insert("user_agent".to_string(), "RustCI-Web/1.0".to_string());
        
        match zero_trust.authenticate(credentials, request_context).await {
            Ok(security_context) => {
                info!("    ✅ Authentication successful");
                info!("    🎯 Trust score: {:.2}", security_context.trust_score);
                info!("    ⚠️  Risk level: {:?}", security_context.risk_level);
                info!("    🔒 Identity type: {:?}", security_context.identity.identity_type);
                info!("    ⏰ Expires at: {:?}", security_context.expires_at);
            }
            Err(e) => {
                error!("    ❌ Authentication failed: {}", e);
            }
        }
    }
    
    Ok(())
}/// 
Demonstrate service authentication for automated systems
async fn demo_service_authentication(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("🤖 Demo: Service Authentication");
    
    let service_scenarios = vec![
        ("ci-pipeline-service", IdentityType::Service, "pipeline execution"),
        ("monitoring-agent", IdentityType::Application, "metrics collection"),
        ("backup-service", IdentityType::Workload, "data backup"),
        ("iot-sensor", IdentityType::Device, "sensor data"),
    ];
    
    for (service_name, identity_type, purpose) in service_scenarios {
        info!("  🔧 Authenticating service: {} ({})", service_name, purpose);
        
        let mut credentials = HashMap::new();
        credentials.insert("service_name".to_string(), service_name.to_string());
        credentials.insert("service_token".to_string(), format!("{}_token_12345", service_name));
        credentials.insert("identity_type".to_string(), format!("{:?}", identity_type));
        
        let mut request_context = HashMap::new();
        request_context.insert("service_purpose".to_string(), purpose.to_string());
        request_context.insert("environment".to_string(), "production".to_string());
        request_context.insert("cluster".to_string(), "main-cluster".to_string());
        
        match zero_trust.authenticate(credentials, request_context).await {
            Ok(security_context) => {
                info!("    ✅ Service authenticated");
                info!("    🎯 Trust score: {:.2}", security_context.trust_score);
                info!("    🔒 Identity: {}", security_context.identity.principal);
                
                // Services typically get higher trust scores
                if matches!(identity_type, IdentityType::Service) {
                    assert!(security_context.trust_score > 0.6, "Services should have high trust");
                }
            }
            Err(e) => {
                error!("    ❌ Service authentication failed: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Demonstrate policy enforcement with different access scenarios
async fn demo_policy_enforcement(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("📋 Demo: Policy Enforcement");
    
    // First authenticate a user
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "policy_test_user".to_string());
    credentials.insert("password".to_string(), "secure_password".to_string());
    
    let request_context = HashMap::new();
    let security_context = zero_trust.authenticate(credentials, request_context).await?;
    
    info!("  👤 Testing policies for user: {}", security_context.identity.principal);
    
    // Test different resource access scenarios
    let access_scenarios = vec![
        ("public_api", "read", "Public API access", true),
        ("user_profile", "read", "Own profile access", true),
        ("user_profile", "write", "Profile modification", true),
        ("pipeline_config", "read", "Pipeline configuration", true),
        ("pipeline_config", "write", "Pipeline modification", false), // Might require higher auth
        ("admin_panel", "access", "Admin panel access", false),
        ("system_config", "write", "System configuration", false),
        ("audit_logs", "read", "Audit log access", false),
    ];
    
    for (resource, action, description, expected_basic_access) in access_scenarios {
        info!("    🔍 Testing: {} ({})", description, format!("{}.{}", resource, action));
        
        let mut request_data = HashMap::new();
        request_data.insert("resource_type".to_string(), resource.to_string());
        request_data.insert("action_type".to_string(), action.to_string());
        request_data.insert("description".to_string(), description.to_string());
        
        match zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await {
            Ok(auth_result) => {
                if auth_result.allowed {
                    info!("      ✅ Access granted");
                } else {
                    info!("      ❌ Access denied: {}", auth_result.reason);
                    if !auth_result.required_actions.is_empty() {
                        info!("      🔧 Required actions: {:?}", auth_result.required_actions);
                    }
                }
                
                if !auth_result.policy_violations.is_empty() {
                    warn!("      ⚠️  Policy violations detected: {}", auth_result.policy_violations.len());
                }
            }
            Err(e) => {
                error!("      💥 Authorization error: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Demonstrate risk-based access control
async fn demo_risk_based_access(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("⚠️  Demo: Risk-Based Access Control");
    
    let risk_scenarios = vec![
        // Low risk scenario
        (
            "low_risk_user",
            vec![
                ("location", "US"),
                ("device_id", "trusted_device_123"),
                ("network", "corporate"),
                ("time_of_day", "14:30"), // Business hours
            ],
            "Low risk scenario",
        ),
        // Medium risk scenario
        (
            "medium_risk_user",
            vec![
                ("location", "CA"),
                ("device_id", "new_device_456"),
                ("network", "home"),
                ("time_of_day", "22:15"), // After hours
            ],
            "Medium risk scenario",
        ),
        // High risk scenario
        (
            "high_risk_user",
            vec![
                ("location", "XX"), // High-risk country
                ("device_id", "unknown_device"),
                ("network", "public_wifi"),
                ("time_of_day", "03:00"), // Unusual time
                ("failed_attempts", "2"), // Previous failures
            ],
            "High risk scenario",
        ),
    ];
    
    for (username, risk_factors, scenario_desc) in risk_scenarios {
        info!("  🎯 Testing: {}", scenario_desc);
        
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        credentials.insert("password".to_string(), "password123".to_string());
        
        let mut request_context = HashMap::new();
        for (key, value) in risk_factors {
            request_context.insert(key.to_string(), value.to_string());
        }
        
        match zero_trust.authenticate(credentials, request_context).await {
            Ok(security_context) => {
                info!("    ✅ Authentication successful");
                info!("    🎯 Trust score: {:.2}", security_context.trust_score);
                info!("    ⚠️  Risk level: {:?}", security_context.risk_level);
                
                // Test sensitive resource access based on risk
                let mut request_data = HashMap::new();
                request_data.insert("sensitivity".to_string(), "high".to_string());
                
                match zero_trust.authorize(
                    security_context.context_id,
                    "sensitive_data",
                    "access",
                    &request_data,
                ).await {
                    Ok(auth_result) => {
                        if auth_result.allowed {
                            info!("    ✅ Sensitive access granted");
                        } else {
                            info!("    ❌ Sensitive access denied due to risk");
                            if !auth_result.required_actions.is_empty() {
                                info!("    🔧 Additional verification required: {:?}", 
                                      auth_result.required_actions);
                            }
                        }
                    }
                    Err(e) => {
                        error!("    💥 Authorization failed: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("    ❌ Authentication failed: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Demonstrate continuous monitoring and behavioral analysis
async fn demo_continuous_monitoring(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Demo: Continuous Monitoring & Behavioral Analysis");
    
    // Authenticate user for monitoring
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "monitored_user".to_string());
    credentials.insert("password".to_string(), "secure_password".to_string());
    
    let request_context = HashMap::new();
    let security_context = zero_trust.authenticate(credentials, request_context).await?;
    let initial_trust_score = security_context.trust_score;
    
    info!("  👤 Monitoring user: {}", security_context.identity.principal);
    info!("  🎯 Initial trust score: {:.2}", initial_trust_score);
    
    // Simulate normal behavior pattern
    info!("  📈 Simulating normal behavior pattern...");
    let normal_activities = vec![
        ("dashboard", "view", "Normal dashboard access"),
        ("profile", "read", "Profile viewing"),
        ("projects", "list", "Project listing"),
        ("pipeline", "view", "Pipeline monitoring"),
        ("logs", "read", "Log viewing"),
    ];
    
    for (resource, action, description) in normal_activities {
        let mut request_data = HashMap::new();
        request_data.insert("behavior_type".to_string(), "normal".to_string());
        request_data.insert("description".to_string(), description.to_string());
        
        match zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await {
            Ok(auth_result) => {
                if auth_result.allowed {
                    info!("    ✅ {}: Access granted", description);
                } else {
                    warn!("    ⚠️  {}: Access restricted", description);
                }
            }
            Err(e) => {
                error!("    💥 {}: Authorization error: {}", description, e);
            }
        }
        
        // Small delay to simulate real usage
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Simulate suspicious behavior pattern
    info!("  🚨 Simulating suspicious behavior pattern...");
    let suspicious_activities = vec![
        ("admin_panel", "access", "Unusual admin access"),
        ("user_data", "export", "Mass data export"),
        ("system_config", "read", "System configuration access"),
        ("audit_logs", "access", "Audit log access"),
        ("security_settings", "modify", "Security modification"),
    ];
    
    for (resource, action, description) in suspicious_activities {
        let mut request_data = HashMap::new();
        request_data.insert("behavior_type".to_string(), "suspicious".to_string());
        request_data.insert("rapid_succession".to_string(), "true".to_string());
        request_data.insert("unusual_pattern".to_string(), "true".to_string());
        
        match zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await {
            Ok(auth_result) => {
                if auth_result.allowed {
                    warn!("    ⚠️  {}: Access granted despite suspicion", description);
                } else {
                    info!("    🛡️  {}: Access blocked due to suspicious behavior", description);
                    if !auth_result.required_actions.is_empty() {
                        info!("      🔧 Required actions: {:?}", auth_result.required_actions);
                    }
                }
            }
            Err(e) => {
                error!("    💥 {}: Authorization error: {}", description, e);
            }
        }
        
        // Rapid succession to trigger behavioral analysis
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    Ok(())
}

/// Demonstrate threat detection and response
async fn demo_threat_detection(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("🛡️  Demo: Threat Detection & Response");
    
    // Simulate different threat scenarios
    let threat_scenarios = vec![
        (
            "brute_force_user",
            vec![
                ("attack_type", "brute_force"),
                ("failed_attempts", "5"),
                ("time_window", "60"), // seconds
                ("source_ip", "192.168.1.200"),
            ],
            "Brute force attack simulation",
        ),
        (
            "credential_stuffing_user",
            vec![
                ("attack_type", "credential_stuffing"),
                ("multiple_accounts", "true"),
                ("automated_behavior", "true"),
                ("source_ip", "10.0.0.100"),
            ],
            "Credential stuffing simulation",
        ),
        (
            "insider_threat_user",
            vec![
                ("attack_type", "insider_threat"),
                ("privilege_escalation", "attempted"),
                ("data_exfiltration", "suspected"),
                ("behavior_change", "significant"),
            ],
            "Insider threat simulation",
        ),
    ];
    
    for (username, threat_indicators, scenario_desc) in threat_scenarios {
        info!("  🚨 Testing: {}", scenario_desc);
        
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        credentials.insert("password".to_string(), "password123".to_string());
        
        let mut request_context = HashMap::new();
        for (key, value) in threat_indicators {
            request_context.insert(key.to_string(), value.to_string());
        }
        
        match zero_trust.authenticate(credentials, request_context).await {
            Ok(security_context) => {
                info!("    ⚠️  Authentication succeeded despite threat indicators");
                info!("    🎯 Trust score: {:.2} (likely reduced)", security_context.trust_score);
                
                // Test if threat detection affects authorization
                let mut request_data = HashMap::new();
                request_data.insert("threat_context".to_string(), "active".to_string());
                
                match zero_trust.authorize(
                    security_context.context_id,
                    "sensitive_resource",
                    "access",
                    &request_data,
                ).await {
                    Ok(auth_result) => {
                        if auth_result.allowed {
                            warn!("    ⚠️  Access granted despite threat indicators");
                        } else {
                            info!("    🛡️  Access blocked due to threat detection");
                            info!("    📝 Reason: {}", auth_result.reason);
                        }
                    }
                    Err(e) => {
                        info!("    🛡️  Authorization blocked: {}", e);
                    }
                }
            }
            Err(e) => {
                info!("    🛡️  Authentication blocked due to threat: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Demonstrate session management and lifecycle
async fn demo_session_management(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Demo: Session Management & Lifecycle");
    
    // Create multiple sessions
    let users = vec!["session_user_1", "session_user_2", "session_user_3"];
    let mut contexts = Vec::new();
    
    info!("  📝 Creating multiple user sessions...");
    for username in &users {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        credentials.insert("password".to_string(), "secure_password".to_string());
        
        let request_context = HashMap::new();
        match zero_trust.authenticate(credentials, request_context).await {
            Ok(context) => {
                info!("    ✅ Session created for: {}", username);
                contexts.push(context);
            }
            Err(e) => {
                error!("    ❌ Failed to create session for {}: {}", username, e);
            }
        }
    }
    
    // List active sessions
    let active_contexts = zero_trust.list_active_contexts().await;
    info!("  📊 Active sessions: {}", active_contexts.len());
    
    // Test session usage
    info!("  🔄 Testing session usage...");
    for (i, context) in contexts.iter().enumerate() {
        let mut request_data = HashMap::new();
        request_data.insert("session_test".to_string(), format!("test_{}", i));
        
        match zero_trust.authorize(
            context.context_id,
            "test_resource",
            "read",
            &request_data,
        ).await {
            Ok(auth_result) => {
                if auth_result.allowed {
                    info!("    ✅ Session {} active and authorized", i + 1);
                } else {
                    warn!("    ⚠️  Session {} authorization failed", i + 1);
                }
            }
            Err(e) => {
                error!("    ❌ Session {} error: {}", i + 1, e);
            }
        }
    }
    
    // Revoke a session
    if !contexts.is_empty() {
        let context_to_revoke = contexts[0].context_id;
        info!("  🗑️  Revoking session: {}", context_to_revoke);
        
        match zero_trust.revoke_context(context_to_revoke).await {
            Ok(_) => {
                info!("    ✅ Session revoked successfully");
                
                // Try to use revoked session
                let mut request_data = HashMap::new();
                request_data.insert("revoked_test".to_string(), "true".to_string());
                
                match zero_trust.authorize(
                    context_to_revoke,
                    "test_resource",
                    "read",
                    &request_data,
                ).await {
                    Ok(_) => {
                        error!("    ❌ Revoked session should not be authorized!");
                    }
                    Err(_) => {
                        info!("    ✅ Revoked session correctly rejected");
                    }
                }
            }
            Err(e) => {
                error!("    ❌ Failed to revoke session: {}", e);
            }
        }
    }
    
    // Check final active sessions
    let final_active_contexts = zero_trust.list_active_contexts().await;
    info!("  📊 Final active sessions: {}", final_active_contexts.len());
    
    Ok(())
}

/// Show comprehensive security metrics
async fn show_security_metrics(zero_trust: &ZeroTrustManager) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Final Zero-Trust Security Metrics");
    
    let metrics = zero_trust.metrics().await;
    
    info!("  🔐 Authentication Metrics:");
    info!("    Total attempts: {}", metrics.total_auth_attempts);
    info!("    Successful: {}", metrics.successful_auths);
    info!("    Failed: {}", metrics.failed_auths);
    info!("    Success rate: {:.1}%", 
          if metrics.total_auth_attempts > 0 {
              (metrics.successful_auths as f64 / metrics.total_auth_attempts as f64) * 100.0
          } else { 0.0 });
    
    info!("  📋 Authorization Metrics:");
    info!("    Policy evaluations: {}", metrics.policy_evaluations);
    info!("    Policy violations: {}", metrics.policy_violations);
    info!("    Violation rate: {:.1}%",
          if metrics.policy_evaluations > 0 {
              (metrics.policy_violations as f64 / metrics.policy_evaluations as f64) * 100.0
          } else { 0.0 });
    
    info!("  🎯 Trust & Risk Metrics:");
    info!("    Average trust score: {:.2}", metrics.avg_trust_score);
    info!("    Risk assessments: {}", metrics.risk_assessments);
    info!("    Active sessions: {}", metrics.active_sessions);
    
    info!("  🛡️  Security Metrics:");
    info!("    Threat detections: {}", metrics.threat_detections);
    info!("    Security incidents: {}", metrics.security_incidents);
    
    // Calculate overall security posture
    let security_score = calculate_security_posture(&metrics);
    info!("  🏆 Overall Security Posture: {:.1}/10", security_score);
    
    match security_score {
        s if s >= 9.0 => info!("    ✅ Excellent security posture"),
        s if s >= 7.0 => info!("    ✅ Good security posture"),
        s if s >= 5.0 => info!("    ⚠️  Moderate security posture"),
        s if s >= 3.0 => info!("    ⚠️  Poor security posture"),
        _ => info!("    ❌ Critical security issues detected"),
    }
    
    Ok(())
}

/// Calculate overall security posture score
fn calculate_security_posture(metrics: &ZeroTrustMetrics) -> f64 {
    let mut score = 10.0;
    
    // Reduce score based on failure rates
    if metrics.total_auth_attempts > 0 {
        let auth_failure_rate = metrics.failed_auths as f64 / metrics.total_auth_attempts as f64;
        score -= auth_failure_rate * 3.0; // Max 3 points deduction
    }
    
    if metrics.policy_evaluations > 0 {
        let violation_rate = metrics.policy_violations as f64 / metrics.policy_evaluations as f64;
        score -= violation_rate * 2.0; // Max 2 points deduction
    }
    
    // Reduce score for security incidents
    if metrics.security_incidents > 0 {
        score -= (metrics.security_incidents as f64 * 0.5).min(2.0); // Max 2 points deduction
    }
    
    // Adjust based on trust score
    if metrics.avg_trust_score < 0.5 {
        score -= (0.5 - metrics.avg_trust_score) * 2.0; // Max 1 point deduction
    }
    
    score.max(0.0).min(10.0)
}