use std::time::Duration;
use tokio::time::timeout;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions};
use rustci::core::networking::valkyrie::security::{SecurityManager, AuthMethod, EncryptionMethod};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;

/// Security penetration testing and validation
#[cfg(test)]
mod security_tests {
    use super::*;

    #[tokio::test]
    async fn test_mutual_tls_authentication() {
        let config = TestFixtures::secure_config();
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        
        engine.start().await.unwrap();
        
        // Test mTLS connection establishment
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection_result = timeout(
            Duration::from_secs(10),
            engine.connect_with_mtls(bind_addr, "test-client-cert.pem", "test-client-key.pem")
        ).await;
        
        assert!(connection_result.is_ok(), "mTLS connection should be established");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_token_based_authentication() {
        let mut config = TestFixtures::minimal_config();
        config.security_enabled = true;
        config.auth_method = Some("token".to_string());
        config.auth_token = Some("test-secret-token-123".to_string());
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test valid token authentication
        let valid_connection = engine.connect_with_token(
            "127.0.0.1:8080".parse().unwrap(),
            "test-secret-token-123"
        ).await;
        
        assert!(valid_connection.is_ok(), "Valid token should authenticate");
        
        // Test invalid token authentication
        let invalid_connection = engine.connect_with_token(
            "127.0.0.1:8080".parse().unwrap(),
            "invalid-token"
        ).await;
        
        assert!(invalid_connection.is_err(), "Invalid token should be rejected");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_post_quantum_cryptography() {
        let mut config = TestFixtures::secure_config();
        config.encryption_method = Some("post-quantum".to_string());
        config.enable_kyber_key_exchange = true;
        config.enable_dilithium_signatures = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test post-quantum key exchange
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = timeout(
            Duration::from_secs(15), // Post-quantum crypto may be slower
            engine.connect(bind_addr)
        ).await.unwrap().unwrap();
        
        // Verify post-quantum encryption is active
        let encryption_info = connection.get_encryption_info().await.unwrap();
        assert_eq!(encryption_info.key_exchange, "kyber", "Should use Kyber key exchange");
        assert_eq!(encryption_info.signature, "dilithium", "Should use Dilithium signatures");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_intrusion_detection_system() {
        let mut config = TestFixtures::secure_config();
        config.enable_intrusion_detection = true;
        config.ids_sensitivity = "high".to_string();
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Simulate suspicious activity (rapid connection attempts)
        let suspicious_endpoint = "127.0.0.1:8080".parse().unwrap();
        
        for _ in 0..100 {
            let _ = engine.connect(suspicious_endpoint).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Check if IDS detected the suspicious activity
        let ids_alerts = engine.get_ids_alerts().await.unwrap();
        assert!(!ids_alerts.is_empty(), "IDS should detect suspicious activity");
        
        let alert = &ids_alerts[0];
        assert_eq!(alert.alert_type, "rapid_connections", "Should detect rapid connections");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_message_integrity_verification() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let mut message = TestUtils::create_test_message(
            rustci::core::networking::valkyrie::message::MessageType::JobRequest
        );
        
        // Add integrity signature
        message.signature = Some(engine.sign_message(&message).await.unwrap());
        
        // Test message verification
        let verification_result = engine.verify_message(&message).await;
        assert!(verification_result.is_ok(), "Valid signature should verify");
        
        // Test tampered message
        message.payload.push(0xFF); // Tamper with payload
        let tampered_verification = engine.verify_message(&message).await;
        assert!(tampered_verification.is_err(), "Tampered message should fail verification");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_rbac_authorization() {
        let mut config = TestFixtures::secure_config();
        config.enable_rbac = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Create test roles and permissions
        engine.create_role("admin", vec!["job:create", "job:read", "job:update", "job:delete"]).await.unwrap();
        engine.create_role("user", vec!["job:read"]).await.unwrap();
        
        // Test admin permissions
        let admin_token = engine.create_user_token("admin-user", "admin").await.unwrap();
        let admin_authorized = engine.check_permission(&admin_token, "job:create").await.unwrap();
        assert!(admin_authorized, "Admin should have create permission");
        
        // Test user permissions
        let user_token = engine.create_user_token("regular-user", "user").await.unwrap();
        let user_create_denied = engine.check_permission(&user_token, "job:create").await.unwrap();
        assert!(!user_create_denied, "User should not have create permission");
        
        let user_read_allowed = engine.check_permission(&user_token, "job:read").await.unwrap();
        assert!(user_read_allowed, "User should have read permission");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let mut config = TestFixtures::secure_config();
        config.enable_audit_logging = true;
        config.audit_log_path = Some("/tmp/valkyrie-audit.log".to_string());
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Perform auditable actions
        let connection = engine.connect("127.0.0.1:8080".parse().unwrap()).await.unwrap();
        let message = TestUtils::create_test_message(
            rustci::core::networking::valkyrie::message::MessageType::JobRequest
        );
        
        engine.send_message_via_connection(&connection, message).await.unwrap();
        
        // Wait for audit log to be written
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify audit log entries
        let audit_log = std::fs::read_to_string("/tmp/valkyrie-audit.log").unwrap();
        assert!(audit_log.contains("CONNECTION_ESTABLISHED"), "Should log connection");
        assert!(audit_log.contains("MESSAGE_SENT"), "Should log message sending");
        
        // Verify audit log integrity
        let audit_entries: Vec<serde_json::Value> = audit_log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        
        for entry in audit_entries {
            assert!(entry.get("timestamp").is_some(), "Audit entry should have timestamp");
            assert!(entry.get("event_type").is_some(), "Audit entry should have event type");
            assert!(entry.get("integrity_hash").is_some(), "Audit entry should have integrity hash");
        }
        
        engine.stop().await.unwrap();
        
        // Cleanup
        let _ = std::fs::remove_file("/tmp/valkyrie-audit.log");
    }

    #[tokio::test]
    async fn test_certificate_rotation() {
        let mut config = TestFixtures::secure_config();
        config.enable_auto_cert_rotation = true;
        config.cert_rotation_interval = Duration::from_secs(60);
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Get initial certificate fingerprint
        let initial_cert = engine.get_current_certificate().await.unwrap();
        let initial_fingerprint = initial_cert.fingerprint();
        
        // Trigger certificate rotation
        engine.rotate_certificates().await.unwrap();
        
        // Verify certificate has changed
        let new_cert = engine.get_current_certificate().await.unwrap();
        let new_fingerprint = new_cert.fingerprint();
        
        assert_ne!(
            initial_fingerprint,
            new_fingerprint,
            "Certificate should be rotated"
        );
        
        // Verify connections still work with new certificate
        let connection = engine.connect("127.0.0.1:8080".parse().unwrap()).await;
        assert!(connection.is_ok(), "Connection should work with rotated certificate");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_security_penetration_scenarios() {
        let mut config = TestFixtures::secure_config();
        config.enable_security_hardening = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test 1: Brute force attack simulation
        let mut failed_attempts = 0;
        for i in 0..50 {
            let result = engine.connect_with_token(
                "127.0.0.1:8080".parse().unwrap(),
                &format!("wrong-token-{}", i)
            ).await;
            
            if result.is_err() {
                failed_attempts += 1;
            }
        }
        
        assert_eq!(failed_attempts, 50, "All invalid tokens should be rejected");
        
        // Test 2: Rate limiting
        let rate_limit_exceeded = engine.is_rate_limited("127.0.0.1").await.unwrap();
        assert!(rate_limit_exceeded, "Rate limiting should be triggered");
        
        // Test 3: DDoS protection
        let ddos_protection_active = engine.is_ddos_protection_active().await.unwrap();
        assert!(ddos_protection_active, "DDoS protection should be active");
        
        // Test 4: Message size limits
        let oversized_message = TestUtils::generate_test_data(10 * 1024 * 1024); // 10MB
        let mut large_message = TestUtils::create_test_message(
            rustci::core::networking::valkyrie::message::MessageType::JobRequest
        );
        large_message.payload = oversized_message;
        
        let send_result = engine.send_message(large_message).await;
        assert!(send_result.is_err(), "Oversized message should be rejected");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_zero_trust_architecture() {
        let mut config = TestFixtures::secure_config();
        config.enable_zero_trust = true;
        config.default_deny_policy = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test default deny behavior
        let unauthorized_connection = engine.connect("127.0.0.1:8080".parse().unwrap()).await;
        assert!(unauthorized_connection.is_err(), "Unauthorized connection should be denied");
        
        // Add explicit allow policy
        engine.add_security_policy("allow", "127.0.0.1", vec!["connect", "send_message"]).await.unwrap();
        
        // Test authorized connection
        let authorized_connection = engine.connect("127.0.0.1:8080".parse().unwrap()).await;
        assert!(authorized_connection.is_ok(), "Authorized connection should succeed");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_encryption_performance() {
        let encryption_methods = vec![
            ("aes-256-gcm", false),
            ("chacha20-poly1305", false),
            ("post-quantum", true),
        ];
        
        for (method, is_pq) in encryption_methods {
            let mut config = TestFixtures::secure_config();
            config.encryption_method = Some(method.to_string());
            
            if is_pq {
                config.enable_kyber_key_exchange = true;
                config.enable_dilithium_signatures = true;
            }
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            
            let test_data = TestUtils::generate_test_data(1024 * 1024); // 1MB
            
            let (_, encryption_time) = TestUtils::measure_time(async {
                engine.encrypt_data(&test_data).await
            }).await;
            
            println!("Encryption method {}: time {:?}", method, encryption_time);
            
            // Performance thresholds (adjust based on requirements)
            let max_time = if is_pq {
                Duration::from_millis(100) // Post-quantum may be slower
            } else {
                Duration::from_millis(50)
            };
            
            TestAssertions::assert_latency_below(encryption_time, max_time);
            
            engine.stop().await.unwrap();
        }
    }
}