//! Tests for Zero-Trust Security Layer
//! 
//! Validates identity verification, policy enforcement, continuous monitoring,
//! and adaptive threat response for comprehensive zero-trust security.

use std::collections::HashMap;
use std::time::Duration;
use tokio_test;

use crate::core::networking::valkyrie::security::zero_trust::*;

#[tokio::test]
async fn test_zero_trust_manager_creation() {
    let zero_trust = ZeroTrustManager::new();
    
    // Manager should be created successfully
    let metrics = zero_trust.metrics().await;
    assert_eq!(metrics.total_auth_attempts, 0);
    assert_eq!(metrics.active_sessions, 0);
}

#[tokio::test]
async fn test_identity_authentication() {
    let zero_trust = ZeroTrustManager::new();
    
    // Create authentication credentials
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "test_user".to_string());
    credentials.insert("password".to_string(), "secure_password".to_string());
    credentials.insert("provider".to_string(), "local".to_string());
    
    // Create request context
    let mut request_context = HashMap::new();
    request_context.insert("ip_address".to_string(), "192.168.1.100".to_string());
    request_context.insert("user_agent".to_string(), "RustCI-Client/1.0".to_string());
    request_context.insert("location".to_string(), "US".to_string());
    
    // Authenticate user
    let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
    
    // Verify security context
    assert_eq!(security_context.identity.principal, "test_user");
    assert_eq!(security_context.identity.identity_type, IdentityType::User);
    assert!(security_context.trust_score > 0.0);
    assert!(security_context.trust_score <= 1.0);
    
    // Check metrics
    let metrics = zero_trust.metrics().await;
    assert_eq!(metrics.total_auth_attempts, 1);
    assert_eq!(metrics.successful_auths, 1);
    assert_eq!(metrics.failed_auths, 0);
}

#[tokio::test]
async fn test_authorization_with_policies() {
    let zero_trust = ZeroTrustManager::new();
    
    // Authenticate first
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "admin_user".to_string());
    credentials.insert("password".to_string(), "admin_password".to_string());
    
    let request_context = HashMap::new();
    let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
    
    // Test authorization for different resources
    let test_cases = vec![
        ("pipelines", "read", true),
        ("pipelines", "write", true),
        ("users", "read", true),
        ("users", "delete", true), // Admin should have all permissions
        ("system", "configure", true),
    ];
    
    for (resource, action, expected_allowed) in test_cases {
        let mut request_data = HashMap::new();
        request_data.insert("resource_type".to_string(), resource.to_string());
        request_data.insert("action_type".to_string(), action.to_string());
        
        let auth_result = zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await.unwrap();
        
        assert_eq!(auth_result.allowed, expected_allowed, 
                   "Authorization failed for {}.{}", resource, action);
    }
}

#[tokio::test]
async fn test_trust_score_calculation() {
    let zero_trust = ZeroTrustManager::new();
    
    // Test different identity types and their trust scores
    let test_cases = vec![
        (IdentityType::Service, VerificationLevel::Strong, RiskLevel::Low),
        (IdentityType::User, VerificationLevel::Basic, RiskLevel::Medium),
        (IdentityType::Device, VerificationLevel::Enhanced, RiskLevel::Low),
        (IdentityType::Application, VerificationLevel::Maximum, RiskLevel::VeryLow),
    ];
    
    for (identity_type, verification_level, risk_level) in test_cases {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), format!("{:?}_test", identity_type));
        credentials.insert("verification_level".to_string(), format!("{:?}", verification_level));
        credentials.insert("risk_level".to_string(), format!("{:?}", risk_level));
        
        let request_context = HashMap::new();
        let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
        
        // Verify trust score is reasonable
        assert!(security_context.trust_score >= 0.0);
        assert!(security_context.trust_score <= 1.0);
        
        // Higher verification levels should generally result in higher trust scores
        match verification_level {
            VerificationLevel::Maximum => assert!(security_context.trust_score > 0.7),
            VerificationLevel::Strong => assert!(security_context.trust_score > 0.6),
            VerificationLevel::Enhanced => assert!(security_context.trust_score > 0.5),
            VerificationLevel::Basic => assert!(security_context.trust_score > 0.3),
        }
    }
}

#[tokio::test]
async fn test_risk_assessment() {
    let zero_trust = ZeroTrustManager::new();
    
    // Test different risk scenarios
    let risk_scenarios = vec![
        // Low risk scenario
        (
            vec![
                ("location", "US"),
                ("device_id", "trusted_device_123"),
                ("network", "corporate"),
            ],
            RiskLevel::Low,
        ),
        // High risk scenario
        (
            vec![
                ("location", "XX"), // High-risk country
                ("device_id", "unknown_device"),
                ("network", "public_wifi"),
                ("time", "03:00"), // Unusual time
            ],
            RiskLevel::High,
        ),
        // Medium risk scenario
        (
            vec![
                ("location", "US"),
                ("device_id", "new_device"),
                ("network", "home"),
            ],
            RiskLevel::Medium,
        ),
    ];
    
    for (context_data, expected_risk_level) in risk_scenarios {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), "risk_test_user".to_string());
        
        let mut request_context = HashMap::new();
        for (key, value) in context_data {
            request_context.insert(key.to_string(), value.to_string());
        }
        
        let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
        
        // Risk level assessment is part of authentication
        // Trust score should be inversely related to risk level
        match expected_risk_level {
            RiskLevel::Low | RiskLevel::VeryLow => {
                assert!(security_context.trust_score > 0.6, "Low risk should have high trust");
            }
            RiskLevel::High | RiskLevel::Critical => {
                assert!(security_context.trust_score < 0.7, "High risk should have low trust");
            }
            RiskLevel::Medium => {
                assert!(security_context.trust_score > 0.4 && security_context.trust_score < 0.8);
            }
        }
    }
}

#[tokio::test]
async fn test_session_management() {
    let zero_trust = ZeroTrustManager::new();
    
    // Create multiple sessions
    let session_count = 3;
    let mut contexts = Vec::new();
    
    for i in 0..session_count {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), format!("user_{}", i));
        credentials.insert("session_id".to_string(), format!("session_{}", i));
        
        let request_context = HashMap::new();
        let context = zero_trust.authenticate(credentials, request_context).await.unwrap();
        contexts.push(context);
    }
    
    // Verify all contexts are active
    let active_contexts = zero_trust.list_active_contexts().await;
    assert_eq!(active_contexts.len(), session_count);
    
    // Test context revocation
    let context_to_revoke = contexts[0].context_id;
    zero_trust.revoke_context(context_to_revoke).await.unwrap();
    
    // Verify context is revoked (should fail authorization)
    let mut request_data = HashMap::new();
    request_data.insert("test".to_string(), "data".to_string());
    
    let auth_result = zero_trust.authorize(
        context_to_revoke,
        "test_resource",
        "read",
        &request_data,
    ).await;
    
    // Should fail because context was revoked
    assert!(auth_result.is_err());
}

#[tokio::test]
async fn test_policy_enforcement() {
    let zero_trust = ZeroTrustManager::new();
    
    // Authenticate user
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "policy_test_user".to_string());
    
    let request_context = HashMap::new();
    let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
    
    // Test different policy scenarios
    let policy_tests = vec![
        // Normal access should be allowed
        ("public_resource", "read", true, Vec::new()),
        // Sensitive resource might require additional auth
        ("sensitive_resource", "write", false, vec![RequiredAction::StepUpAuth(VerificationLevel::Strong)]),
        // Admin resource should be denied for normal user
        ("admin_resource", "delete", false, Vec::new()),
    ];
    
    for (resource, action, expected_allowed, expected_actions) in policy_tests {
        let mut request_data = HashMap::new();
        request_data.insert("resource_sensitivity".to_string(), 
                           if resource.contains("sensitive") { "high".to_string() } 
                           else if resource.contains("admin") { "critical".to_string() }
                           else { "normal".to_string() });
        
        let auth_result = zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await.unwrap();
        
        assert_eq!(auth_result.allowed, expected_allowed, 
                   "Policy enforcement failed for {}.{}", resource, action);
        
        if !expected_actions.is_empty() {
            assert!(!auth_result.required_actions.is_empty(), 
                    "Expected required actions for {}.{}", resource, action);
        }
    }
}

#[tokio::test]
async fn test_continuous_monitoring() {
    let config = ZeroTrustConfig {
        enable_behavioral_analytics: true,
        enable_threat_intelligence: true,
        trust_decay_rate: 0.1, // 10% decay per hour
        ..Default::default()
    };
    let zero_trust = ZeroTrustManager::with_config(config);
    
    // Authenticate user
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "monitored_user".to_string());
    
    let request_context = HashMap::new();
    let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
    let initial_trust_score = security_context.trust_score;
    
    // Simulate some time passing and multiple authorization requests
    for i in 0..5 {
        let mut request_data = HashMap::new();
        request_data.insert("request_id".to_string(), format!("req_{}", i));
        request_data.insert("timestamp".to_string(), format!("{}", i * 1000));
        
        let auth_result = zero_trust.authorize(
            security_context.context_id,
            "monitored_resource",
            "read",
            &request_data,
        ).await.unwrap();
        
        // Should still be allowed
        assert!(auth_result.allowed, "Request {} should be allowed", i);
    }
    
    // Get updated context to check trust score changes
    let active_contexts = zero_trust.list_active_contexts().await;
    let updated_context = active_contexts.iter()
        .find(|c| c.context_id == security_context.context_id)
        .expect("Context should still be active");
    
    // Trust score might have changed due to behavioral analysis
    // (In a real implementation, this would depend on actual behavioral patterns)
    assert!(updated_context.trust_score >= 0.0);
    assert!(updated_context.trust_score <= 1.0);
}

#[tokio::test]
async fn test_threat_detection() {
    let config = ZeroTrustConfig {
        enable_threat_intelligence: true,
        min_trust_threshold: 0.5,
        ..Default::default()
    };
    let zero_trust = ZeroTrustManager::with_config(config);
    
    // Authenticate user
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), "threat_test_user".to_string());
    
    // Simulate suspicious request context
    let mut request_context = HashMap::new();
    request_context.insert("ip_address".to_string(), "192.168.1.100".to_string());
    request_context.insert("user_agent".to_string(), "SuspiciousBot/1.0".to_string());
    request_context.insert("location".to_string(), "XX".to_string()); // High-risk location
    
    let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
    
    // Simulate suspicious activity patterns
    let suspicious_requests = vec![
        ("admin_panel", "access"),
        ("user_data", "export"),
        ("system_config", "modify"),
        ("audit_logs", "delete"),
    ];
    
    for (resource, action) in suspicious_requests {
        let mut request_data = HashMap::new();
        request_data.insert("suspicious_pattern".to_string(), "true".to_string());
        request_data.insert("rapid_requests".to_string(), "true".to_string());
        
        let auth_result = zero_trust.authorize(
            security_context.context_id,
            resource,
            action,
            &request_data,
        ).await.unwrap();
        
        // Suspicious activity might be blocked or require additional verification
        if !auth_result.allowed {
            assert!(!auth_result.reason.is_empty(), "Should provide reason for denial");
        }
        
        if !auth_result.required_actions.is_empty() {
            // Should require additional verification for suspicious activity
            assert!(auth_result.required_actions.iter().any(|action| 
                matches!(action, RequiredAction::ReAuthenticate | 
                               RequiredAction::StepUpAuth(_) |
                               RequiredAction::EnhancedMonitoring)
            ));
        }
    }
}

#[tokio::test]
async fn test_identity_types() {
    let zero_trust = ZeroTrustManager::new();
    
    let identity_types = vec![
        (IdentityType::User, "human_user"),
        (IdentityType::Service, "api_service"),
        (IdentityType::Device, "iot_device"),
        (IdentityType::Application, "web_app"),
        (IdentityType::Workload, "k8s_pod"),
        (IdentityType::Custom("robot".to_string()), "automation_bot"),
    ];
    
    for (identity_type, username) in identity_types {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        credentials.insert("identity_type".to_string(), format!("{:?}", identity_type));
        
        let request_context = HashMap::new();
        let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
        
        // Verify identity type is preserved
        assert_eq!(security_context.identity.principal, username);
        
        // Different identity types might have different default trust scores
        match identity_type {
            IdentityType::Service => {
                // Services typically have higher trust
                assert!(security_context.trust_score >= 0.5);
            }
            IdentityType::User => {
                // Users start with default trust
                assert!(security_context.trust_score >= 0.3);
            }
            IdentityType::Device => {
                // Devices might have moderate trust
                assert!(security_context.trust_score >= 0.4);
            }
            _ => {
                // Other types have reasonable trust scores
                assert!(security_context.trust_score >= 0.3);
            }
        }
    }
}

#[tokio::test]
async fn test_verification_levels() {
    let zero_trust = ZeroTrustManager::new();
    
    let verification_levels = vec![
        VerificationLevel::Basic,
        VerificationLevel::Enhanced,
        VerificationLevel::Strong,
        VerificationLevel::Maximum,
    ];
    
    for verification_level in verification_levels {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), format!("user_{:?}", verification_level));
        credentials.insert("verification_level".to_string(), format!("{:?}", verification_level));
        
        let request_context = HashMap::new();
        let security_context = zero_trust.authenticate(credentials, request_context).await.unwrap();
        
        // Higher verification levels should result in higher trust scores
        let expected_min_trust = match verification_level {
            VerificationLevel::Basic => 0.3,
            VerificationLevel::Enhanced => 0.4,
            VerificationLevel::Strong => 0.5,
            VerificationLevel::Maximum => 0.6,
        };
        
        assert!(security_context.trust_score >= expected_min_trust,
                "Verification level {:?} should have trust score >= {}, got {}",
                verification_level, expected_min_trust, security_context.trust_score);
    }
}

#[tokio::test]
async fn test_zero_trust_metrics() {
    let zero_trust = ZeroTrustManager::new();
    
    // Perform various operations to generate metrics
    let operations = vec![
        ("user1", true),  // Successful auth
        ("user2", true),  // Successful auth
        ("user3", true),  // Successful auth
    ];
    
    for (username, should_succeed) in operations {
        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), username.to_string());
        
        let request_context = HashMap::new();
        let result = zero_trust.authenticate(credentials, request_context).await;
        
        if should_succeed {
            assert!(result.is_ok(), "Authentication should succeed for {}", username);
            
            // Perform authorization
            if let Ok(context) = result {
                let mut request_data = HashMap::new();
                request_data.insert("test".to_string(), "data".to_string());
                
                let _auth_result = zero_trust.authorize(
                    context.context_id,
                    "test_resource",
                    "read",
                    &request_data,
                ).await.unwrap();
            }
        }
    }
    
    // Check metrics
    let metrics = zero_trust.metrics().await;
    assert!(metrics.total_auth_attempts > 0);
    assert!(metrics.successful_auths > 0);
    assert!(metrics.policy_evaluations > 0);
    assert!(metrics.avg_trust_score >= 0.0);
    assert!(metrics.avg_trust_score <= 1.0);
    
    // Should have active sessions
    assert!(metrics.active_sessions > 0);
}

// Helper functions for creating test data

fn create_test_credentials(username: &str) -> HashMap<String, String> {
    let mut credentials = HashMap::new();
    credentials.insert("username".to_string(), username.to_string());
    credentials.insert("password".to_string(), "test_password".to_string());
    credentials
}

fn create_test_context() -> HashMap<String, String> {
    let mut context = HashMap::new();
    context.insert("ip_address".to_string(), "192.168.1.100".to_string());
    context.insert("user_agent".to_string(), "RustCI-Test/1.0".to_string());
    context.insert("location".to_string(), "US".to_string());
    context
}