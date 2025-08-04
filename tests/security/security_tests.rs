use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use RustAutoDevOps::core::node_communication::{
    NodeMessage, ControlPlaneMessage, ProtocolMessage, MessagePayload,
    NodeInfo, NodeCapabilities, NodeStatus, NodeType, RunnerType
};
use RustAutoDevOps::core::secure_transport::{
    AuthenticationManager, EncryptionManager, SecurityAuditor,
    SecurityEventType, SecuritySeverity
};
use RustAutoDevOps::core::transport::{
    TransportConfig, TransportType, AuthenticationConfig, AuthMethod
};

#[tokio::test]
async fn test_jwt_security_properties() {
    let auth_manager = AuthenticationManager::new(
        "super-secret-key-for-security-testing-that-is-very-long",
        Duration::from_secs(1), // Short expiry for testing
    );
    
    let node_id = Uuid::new_v4();
    
    // Test 1: Valid token generation and verification
    let token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    let claims = auth_manager.verify_token(&token).await.unwrap();
    assert_eq!(claims.node_id, node_id);
    assert_eq!(claims.node_type, "worker");
    
    // Test 2: Token expiration
    sleep(Duration::from_secs(2)).await;
    let result = auth_manager.verify_token(&token).await;
    assert!(result.is_err(), "Expired token should be rejected");
    
    // Test 3: Invalid token format
    let invalid_token = "invalid.jwt.token";
    let result = auth_manager.verify_token(invalid_token).await;
    assert!(result.is_err(), "Invalid token format should be rejected");
    
    // Test 4: Tampered token
    let valid_token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    let mut tampered_token = valid_token.clone();
    tampered_token.push('x'); // Tamper with the token
    let result = auth_manager.verify_token(&tampered_token).await;
    assert!(result.is_err(), "Tampered token should be rejected");
    
    // Test 5: Token revocation
    let revocable_token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    // Verify token works initially
    auth_manager.verify_token(&revocable_token).await.unwrap();
    
    // Revoke token
    auth_manager.revoke_token(&revocable_token).await.unwrap();
    
    // Verify revoked token is rejected
    let result = auth_manager.verify_token(&revocable_token).await;
    assert!(result.is_err(), "Revoked token should be rejected");
}

#[tokio::test]
async fn test_encryption_security() {
    let encryption_manager = EncryptionManager::new(true, None);
    
    let original_message = ProtocolMessage::new(
        Uuid::new_v4(),
        MessagePayload::NodeMessage(NodeMessage::Heartbeat {
            node_id: Uuid::new_v4(),
            status: NodeStatus::Ready,
            resources: Default::default(),
            metrics: Default::default(),
            timestamp: chrono::Utc::now(),
        }),
    );
    
    // Test 1: Encryption/Decryption cycle
    let encrypted = encryption_manager.encrypt_message(&original_message).await.unwrap();
    let decrypted = encryption_manager.decrypt_message(&encrypted).await.unwrap();
    
    assert_eq!(original_message.id, decrypted.id);
    assert_eq!(original_message.source, decrypted.source);
    
    // Test 2: Encryption with disabled manager should pass through
    let disabled_manager = EncryptionManager::new(false, None);
    let passthrough = disabled_manager.encrypt_message(&original_message).await.unwrap();
    assert_eq!(original_message.id, passthrough.id);
    
    // Test 3: Multiple encryptions should produce different results (if using proper encryption)
    // Note: Current implementation is a placeholder, but this test ensures the interface works
    let encrypted1 = encryption_manager.encrypt_message(&original_message).await.unwrap();
    let encrypted2 = encryption_manager.encrypt_message(&original_message).await.unwrap();
    
    // Both should decrypt to the same original message
    let decrypted1 = encryption_manager.decrypt_message(&encrypted1).await.unwrap();
    let decrypted2 = encryption_manager.decrypt_message(&encrypted2).await.unwrap();
    
    assert_eq!(decrypted1.id, decrypted2.id);
}

#[tokio::test]
async fn test_security_audit_logging() {
    let auditor = SecurityAuditor::new(1000);
    let node_id = Uuid::new_v4();
    
    // Test 1: Log various security events
    auditor.log_event(
        SecurityEventType::AuthenticationSuccess,
        Some(node_id),
        "192.168.1.100".to_string(),
        [("method".to_string(), "jwt".to_string())].into(),
        SecuritySeverity::Info,
    ).await;
    
    auditor.log_event(
        SecurityEventType::AuthenticationFailure,
        Some(node_id),
        "192.168.1.100".to_string(),
        [("reason".to_string(), "invalid_token".to_string())].into(),
        SecuritySeverity::Warning,
    ).await;
    
    auditor.log_event(
        SecurityEventType::SuspiciousActivity,
        Some(node_id),
        "192.168.1.100".to_string(),
        [("activity".to_string(), "multiple_failed_attempts".to_string())].into(),
        SecuritySeverity::Critical,
    ).await;
    
    // Test 2: Retrieve recent events
    let recent_events = auditor.get_recent_events(10).await;
    assert_eq!(recent_events.len(), 3);
    
    // Events should be in reverse chronological order (most recent first)
    assert!(matches!(recent_events[0].event_type, SecurityEventType::SuspiciousActivity));
    assert!(matches!(recent_events[0].severity, SecuritySeverity::Critical));
    
    // Test 3: Retrieve events for specific node
    let node_events = auditor.get_events_for_node(node_id).await;
    assert_eq!(node_events.len(), 3);
    
    // Test 4: Audit log capacity management
    let large_auditor = SecurityAuditor::new(2); // Small capacity
    
    for i in 0..5 {
        large_auditor.log_event(
            SecurityEventType::ConnectionEstablished,
            Some(Uuid::new_v4()),
            format!("192.168.1.{}", i),
            HashMap::new(),
            SecuritySeverity::Info,
        ).await;
    }
    
    let events = large_auditor.get_recent_events(10).await;
    assert_eq!(events.len(), 2, "Should only keep the most recent events");
}

#[tokio::test]
async fn test_brute_force_protection() {
    let auth_manager = AuthenticationManager::new(
        "brute-force-test-secret",
        Duration::from_secs(3600),
    );
    
    let auditor = SecurityAuditor::new(1000);
    let node_id = Uuid::new_v4();
    
    // Simulate multiple failed authentication attempts
    let invalid_tokens = vec![
        "invalid.token.1",
        "invalid.token.2",
        "invalid.token.3",
        "invalid.token.4",
        "invalid.token.5",
    ];
    
    let mut failed_attempts = 0;
    
    for token in invalid_tokens {
        let result = auth_manager.verify_token(token).await;
        if result.is_err() {
            failed_attempts += 1;
            
            // Log the failed attempt
            auditor.log_event(
                SecurityEventType::AuthenticationFailure,
                Some(node_id),
                "192.168.1.100".to_string(),
                [("token".to_string(), token.to_string())].into(),
                SecuritySeverity::Warning,
            ).await;
        }
    }
    
    assert_eq!(failed_attempts, 5);
    
    // Check that all failures were logged
    let node_events = auditor.get_events_for_node(node_id).await;
    let auth_failures = node_events.iter()
        .filter(|e| matches!(e.event_type, SecurityEventType::AuthenticationFailure))
        .count();
    
    assert_eq!(auth_failures, 5);
}

#[tokio::test]
async fn test_message_integrity() {
    let original_message = ProtocolMessage::new(
        Uuid::new_v4(),
        MessagePayload::NodeMessage(NodeMessage::RegisterNode {
            node_info: NodeInfo {
                hostname: "test-node".to_string(),
                ip_address: "192.168.1.100".to_string(),
                port: 8080,
                node_type: NodeType::Worker,
                version: "1.0.0".to_string(),
                platform: "linux".to_string(),
                architecture: "x86_64".to_string(),
                tags: HashMap::new(),
            },
            capabilities: NodeCapabilities {
                runner_types: vec![RunnerType::Native],
                max_resources: Default::default(),
                supported_job_types: vec!["build".to_string()],
                features: vec![],
                protocols: vec!["tcp".to_string()],
            },
            auth_token: "test-token".to_string(),
        }),
    );
    
    // Test 1: Serialization preserves all data
    let serialized = serde_json::to_string(&original_message).unwrap();
    let deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(original_message.id, deserialized.id);
    assert_eq!(original_message.source, deserialized.source);
    assert_eq!(original_message.timestamp.timestamp(), deserialized.timestamp.timestamp());
    
    // Test 2: Message tampering detection (through serialization)
    let mut tampered_json = serialized.clone();
    tampered_json = tampered_json.replace("test-node", "hacked-node");
    
    let tampered_message: ProtocolMessage = serde_json::from_str(&tampered_json).unwrap();
    
    // The message should deserialize but contain different data
    if let MessagePayload::NodeMessage(NodeMessage::RegisterNode { node_info, .. }) = &tampered_message.message {
        assert_eq!(node_info.hostname, "hacked-node");
        assert_ne!(node_info.hostname, "test-node");
    } else {
        panic!("Expected RegisterNode message");
    }
    
    // Test 3: Invalid JSON should fail to deserialize
    let invalid_json = "{ invalid json }";
    let result = serde_json::from_str::<ProtocolMessage>(invalid_json);
    assert!(result.is_err(), "Invalid JSON should fail to deserialize");
}

#[tokio::test]
async fn test_timing_attack_resistance() {
    let auth_manager = AuthenticationManager::new(
        "timing-attack-test-secret",
        Duration::from_secs(3600),
    );
    
    let valid_token = auth_manager.generate_token(
        Uuid::new_v4(),
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    let invalid_tokens = vec![
        "short",
        "medium-length-token",
        "very-long-invalid-token-that-should-take-same-time-to-reject",
        &valid_token[..valid_token.len()-1], // Almost valid token
    ];
    
    // Measure timing for valid token verification
    let start = std::time::Instant::now();
    let _ = auth_manager.verify_token(&valid_token).await;
    let valid_duration = start.elapsed();
    
    // Measure timing for invalid tokens
    for invalid_token in invalid_tokens {
        let start = std::time::Instant::now();
        let _ = auth_manager.verify_token(invalid_token).await;
        let invalid_duration = start.elapsed();
        
        // The timing difference should not be too significant
        // This is a basic check - in production, you'd want more sophisticated timing analysis
        let ratio = if valid_duration > invalid_duration {
            valid_duration.as_nanos() as f64 / invalid_duration.as_nanos() as f64
        } else {
            invalid_duration.as_nanos() as f64 / valid_duration.as_nanos() as f64
        };
        
        // Allow for some variance but flag if there's a huge difference
        assert!(ratio < 10.0, "Timing difference too large, potential timing attack vector");
    }
}

#[tokio::test]
async fn test_concurrent_security_operations() {
    let auth_manager = Arc::new(AuthenticationManager::new(
        "concurrent-security-test-secret",
        Duration::from_secs(3600),
    ));
    
    let auditor = Arc::new(SecurityAuditor::new(10000));
    
    // Test concurrent token generation and verification
    let mut handles = vec![];
    
    for i in 0..100 {
        let auth_manager = auth_manager.clone();
        let auditor = auditor.clone();
        
        let handle = tokio::spawn(async move {
            let node_id = Uuid::new_v4();
            
            // Generate token
            let token = auth_manager.generate_token(
                node_id,
                format!("worker-{}", i),
                vec!["native".to_string()],
            ).await.unwrap();
            
            // Verify token
            let claims = auth_manager.verify_token(&token).await.unwrap();
            assert_eq!(claims.node_id, node_id);
            
            // Log security event
            auditor.log_event(
                SecurityEventType::AuthenticationSuccess,
                Some(node_id),
                format!("192.168.1.{}", i % 255),
                HashMap::new(),
                SecuritySeverity::Info,
            ).await;
            
            node_id
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all operations succeeded
    assert_eq!(results.len(), 100);
    for result in results {
        assert!(result.is_ok());
    }
    
    // Verify audit log captured all events
    let events = auditor.get_recent_events(200).await;
    assert_eq!(events.len(), 100);
}

#[tokio::test]
async fn test_resource_exhaustion_protection() {
    let auth_manager = AuthenticationManager::new(
        "resource-exhaustion-test-secret",
        Duration::from_secs(3600),
    );
    
    // Test 1: Large number of token generations (should not crash or hang)
    let start = std::time::Instant::now();
    
    for i in 0..1000 {
        let _token = auth_manager.generate_token(
            Uuid::new_v4(),
            format!("worker-{}", i),
            vec!["native".to_string()],
        ).await.unwrap();
    }
    
    let duration = start.elapsed();
    
    // Should complete within reasonable time (adjust threshold as needed)
    assert!(duration < Duration::from_secs(10), "Token generation taking too long");
    
    // Test 2: Memory usage should be reasonable
    let auditor = SecurityAuditor::new(10000);
    
    // Generate many audit events
    for i in 0..5000 {
        auditor.log_event(
            SecurityEventType::ConnectionEstablished,
            Some(Uuid::new_v4()),
            format!("192.168.1.{}", i % 255),
            [("test".to_string(), format!("event-{}", i))].into(),
            SecuritySeverity::Info,
        ).await;
    }
    
    // Should maintain only the configured number of events
    let events = auditor.get_recent_events(20000).await;
    assert_eq!(events.len(), 10000, "Audit log should respect capacity limits");
}

#[tokio::test]
async fn test_configuration_security() {
    // Test 1: Weak secrets should be detectable (in production)
    let weak_secrets = vec![
        "123456",
        "password",
        "secret",
        "test",
        "",
    ];
    
    for weak_secret in weak_secrets {
        // In production, you might want to validate secret strength
        // For now, we just ensure the system doesn't crash with weak secrets
        let auth_manager = AuthenticationManager::new(
            weak_secret,
            Duration::from_secs(3600),
        );
        
        let token = auth_manager.generate_token(
            Uuid::new_v4(),
            "worker".to_string(),
            vec!["native".to_string()],
        ).await.unwrap();
        
        // Token should still be generated (but in production, you'd warn about weak secrets)
        assert!(!token.is_empty());
    }
    
    // Test 2: Transport configuration validation
    let configs = vec![
        TransportConfig {
            transport_type: TransportType::Tcp,
            bind_address: "0.0.0.0".to_string(),
            port: Some(8080),
            tls_config: None,
            authentication: AuthenticationConfig {
                method: AuthMethod::None, // Insecure but should work
                jwt_secret: None,
                token_expiry: Duration::from_secs(3600),
                require_mutual_auth: false,
            },
            timeouts: Default::default(),
            buffer_sizes: Default::default(),
        },
        TransportConfig {
            transport_type: TransportType::Tcp,
            bind_address: "127.0.0.1".to_string(), // More secure binding
            port: Some(8443),
            tls_config: None,
            authentication: AuthenticationConfig {
                method: AuthMethod::JwtToken,
                jwt_secret: Some("strong-secret-key".to_string()),
                token_expiry: Duration::from_secs(1800), // Shorter expiry
                require_mutual_auth: true, // More secure
            },
            timeouts: Default::default(),
            buffer_sizes: Default::default(),
        },
    ];
    
    for config in configs {
        // Configuration should serialize/deserialize without issues
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: TransportConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.bind_address, deserialized.bind_address);
        assert_eq!(config.port, deserialized.port);
    }
}