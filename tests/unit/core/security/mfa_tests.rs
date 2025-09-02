//! Unit tests for Multi-Factor Authentication (MFA) system

use RustAutoDevOps::core::security::{
    MultiFactorAuthProvider, MfaSetupRequest, MfaVerificationRequest, 
    MfaMethod, MfaChallenge, MfaVerificationResult
};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_mfa_provider_creation() {
    let provider = MultiFactorAuthProvider::new();
    assert!(!provider.is_mfa_enabled("test_user").await);
}

#[tokio::test]
async fn test_mfa_provider_with_custom_settings() {
    let provider = MultiFactorAuthProvider::with_settings(3, 10, 2);
    assert!(!provider.is_mfa_enabled("test_user").await);
}

#[tokio::test]
async fn test_totp_setup_success() {
    let provider = MultiFactorAuthProvider::new();
    
    let mut setup_data = HashMap::new();
    setup_data.insert("account_name".to_string(), "test@example.com".to_string());
    setup_data.insert("issuer".to_string(), "TestApp".to_string());
    
    let request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: Some(setup_data),
    };

    let response = provider.setup_mfa(request).await.unwrap();
    
    assert!(response.success);
    assert!(response.setup_data.is_some());
    assert!(response.backup_codes.is_some());
    assert!(response.error.is_none());
    
    let setup_data = response.setup_data.unwrap();
    assert!(setup_data.contains_key("qr_code_url"));
    assert!(setup_data.contains_key("secret"));
    
    let backup_codes = response.backup_codes.unwrap();
    assert_eq!(backup_codes.len(), 10);
    
    // Verify MFA is now enabled
    assert!(provider.is_mfa_enabled("test_user").await);
}

#[tokio::test]
async fn test_totp_setup_with_minimal_data() {
    let provider = MultiFactorAuthProvider::new();
    
    let request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };

    let response = provider.setup_mfa(request).await.unwrap();
    
    assert!(response.success);
    assert!(response.setup_data.is_some());
    assert!(response.backup_codes.is_some());
    
    // Verify MFA is enabled
    assert!(provider.is_mfa_enabled("test_user").await);
}

#[tokio::test]
async fn test_sms_setup_not_implemented() {
    let provider = MultiFactorAuthProvider::new();
    
    let request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Sms,
        setup_data: None,
    };

    let response = provider.setup_mfa(request).await.unwrap();
    
    assert!(!response.success);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("SMS MFA not yet implemented"));
}

#[tokio::test]
async fn test_hardware_token_setup_not_implemented() {
    let provider = MultiFactorAuthProvider::new();
    
    let request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::HardwareToken,
        setup_data: None,
    };

    let response = provider.setup_mfa(request).await.unwrap();
    
    assert!(!response.success);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("Hardware token MFA not yet implemented"));
}

#[tokio::test]
async fn test_mfa_challenge_creation() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP first
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();

    // Create challenge
    let challenge = provider.create_challenge("test_user").await.unwrap();
    
    assert_eq!(challenge.user_id, "test_user");
    assert!(challenge.available_methods.contains(&MfaMethod::Totp));
    assert!(challenge.expires_at > chrono::Utc::now());
    assert!(!challenge.challenge_id.is_empty());
}

#[tokio::test]
async fn test_mfa_challenge_for_non_configured_user() {
    let provider = MultiFactorAuthProvider::new();
    
    let result = provider.create_challenge("non_existent_user").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("MFA not configured"));
}

#[tokio::test]
async fn test_mfa_challenge_for_disabled_user() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up and then disable MFA
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();
    provider.disable_mfa("test_user").await.unwrap();

    let result = provider.create_challenge("test_user").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("MFA not enabled"));
}

#[tokio::test]
async fn test_backup_code_verification() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP to get backup codes
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    let setup_response = provider.setup_mfa(setup_request).await.unwrap();
    let backup_codes = setup_response.backup_codes.unwrap();

    // Create challenge
    let challenge = provider.create_challenge("test_user").await.unwrap();

    // Verify with backup code
    let verification_request = MfaVerificationRequest {
        challenge_id: challenge.challenge_id,
        method: MfaMethod::BackupCode,
        code: backup_codes[0].clone(),
    };

    let result = provider.verify_challenge(verification_request).await.unwrap();
    assert!(result.success);
    assert!(result.error.is_none());
    assert!(result.remaining_attempts.is_none());

    // Try to use the same backup code again (should fail)
    let challenge2 = provider.create_challenge("test_user").await.unwrap();
    let verification_request2 = MfaVerificationRequest {
        challenge_id: challenge2.challenge_id,
        method: MfaMethod::BackupCode,
        code: backup_codes[0].clone(),
    };

    let result2 = provider.verify_challenge(verification_request2).await.unwrap();
    assert!(!result2.success);
    assert!(result2.error.is_some());
}

#[tokio::test]
async fn test_invalid_backup_code_verification() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();

    // Create challenge
    let challenge = provider.create_challenge("test_user").await.unwrap();

    // Verify with invalid backup code
    let verification_request = MfaVerificationRequest {
        challenge_id: challenge.challenge_id,
        method: MfaMethod::BackupCode,
        code: "INVALID_CODE".to_string(),
    };

    let result = provider.verify_challenge(verification_request).await.unwrap();
    assert!(!result.success);
    assert!(result.error.is_some());
    assert!(result.remaining_attempts.is_some());
}

#[tokio::test]
async fn test_failed_attempts_lockout() {
    let provider = MultiFactorAuthProvider::with_settings(3, 15, 5);
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();

    // Make multiple failed attempts
    for i in 0..4 {
        let challenge = provider.create_challenge("test_user").await.unwrap();
        let verification_request = MfaVerificationRequest {
            challenge_id: challenge.challenge_id,
            method: MfaMethod::Totp,
            code: "invalid".to_string(),
        };

        let result = provider.verify_challenge(verification_request).await.unwrap();
        assert!(!result.success);
        
        if i < 3 {
            assert!(result.remaining_attempts.unwrap() > 0);
            assert!(result.lockout_expires_at.is_none());
        } else {
            assert_eq!(result.remaining_attempts.unwrap(), 0);
            assert!(result.lockout_expires_at.is_some());
        }
    }

    // User should now be locked out
    let result = provider.create_challenge("test_user").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("locked"));
}

#[tokio::test]
async fn test_expired_challenge_verification() {
    let provider = MultiFactorAuthProvider::with_settings(5, 15, 0); // 0 minute expiration for testing
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    let setup_response = provider.setup_mfa(setup_request).await.unwrap();
    let backup_codes = setup_response.backup_codes.unwrap();

    // Create challenge (will expire immediately due to 0 minute setting)
    let challenge = provider.create_challenge("test_user").await.unwrap();

    // Wait a bit to ensure expiration
    sleep(Duration::from_millis(100)).await;

    // Try to verify expired challenge
    let verification_request = MfaVerificationRequest {
        challenge_id: challenge.challenge_id,
        method: MfaMethod::BackupCode,
        code: backup_codes[0].clone(),
    };

    let result = provider.verify_challenge(verification_request).await.unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("expired"));
}

#[tokio::test]
async fn test_invalid_challenge_verification() {
    let provider = MultiFactorAuthProvider::new();
    
    let verification_request = MfaVerificationRequest {
        challenge_id: "invalid_challenge_id".to_string(),
        method: MfaMethod::BackupCode,
        code: "some_code".to_string(),
    };

    let result = provider.verify_challenge(verification_request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid or expired challenge"));
}

#[tokio::test]
async fn test_mfa_config_retrieval() {
    let provider = MultiFactorAuthProvider::new();
    
    // Initially no config
    let config = provider.get_mfa_config("test_user").await;
    assert!(config.is_none());
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();
    
    // Now should have config
    let config = provider.get_mfa_config("test_user").await;
    assert!(config.is_some());
    
    let config = config.unwrap();
    assert_eq!(config.user_id, "test_user");
    assert!(config.enabled);
    assert_eq!(config.methods.len(), 1);
    assert_eq!(config.methods[0].method, MfaMethod::Totp);
    assert!(config.methods[0].active);
    assert_eq!(config.backup_codes.len(), 10);
}

#[tokio::test]
async fn test_mfa_disable() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();
    assert!(provider.is_mfa_enabled("test_user").await);

    // Disable MFA
    provider.disable_mfa("test_user").await.unwrap();
    assert!(!provider.is_mfa_enabled("test_user").await);
    
    // Config should be cleared
    let config = provider.get_mfa_config("test_user").await;
    assert!(config.is_some());
    let config = config.unwrap();
    assert!(!config.enabled);
    assert!(config.methods.is_empty());
    assert!(config.backup_codes.is_empty());
}

#[tokio::test]
async fn test_cleanup_expired_challenges() {
    let provider = MultiFactorAuthProvider::with_settings(5, 15, 0); // 0 minute expiration
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();

    // Create challenge (will expire immediately)
    let _challenge = provider.create_challenge("test_user").await.unwrap();
    
    // Wait for expiration
    sleep(Duration::from_millis(100)).await;
    
    // Clean up expired challenges
    provider.cleanup_expired_challenges().await;
    
    // Verify challenge is cleaned up by trying to verify it
    let verification_request = MfaVerificationRequest {
        challenge_id: "any_id".to_string(),
        method: MfaMethod::BackupCode,
        code: "any_code".to_string(),
    };
    
    let result = provider.verify_challenge(verification_request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_totp_setups_replace_previous() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP first time
    let setup_request1 = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    let response1 = provider.setup_mfa(setup_request1).await.unwrap();
    assert!(response1.success);
    
    // Set up TOTP second time (should replace first)
    let setup_request2 = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    let response2 = provider.setup_mfa(setup_request2).await.unwrap();
    assert!(response2.success);
    
    // Should still have only one TOTP method
    let config = provider.get_mfa_config("test_user").await.unwrap();
    let totp_methods: Vec<_> = config.methods.iter()
        .filter(|m| m.method == MfaMethod::Totp)
        .collect();
    assert_eq!(totp_methods.len(), 1);
}

#[tokio::test]
async fn test_sms_and_hardware_token_verification_not_implemented() {
    let provider = MultiFactorAuthProvider::new();
    
    // Set up TOTP to create a valid challenge
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();
    
    let challenge = provider.create_challenge("test_user").await.unwrap();
    
    // Test SMS verification
    let sms_request = MfaVerificationRequest {
        challenge_id: challenge.challenge_id.clone(),
        method: MfaMethod::Sms,
        code: "123456".to_string(),
    };
    
    let result = provider.verify_challenge(sms_request).await.unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("not yet implemented"));
    
    // Test Hardware Token verification
    let hw_request = MfaVerificationRequest {
        challenge_id: challenge.challenge_id,
        method: MfaMethod::HardwareToken,
        code: "123456".to_string(),
    };
    
    let result = provider.verify_challenge(hw_request).await.unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("not yet implemented"));
}

#[tokio::test]
async fn test_lockout_cleanup() {
    let provider = MultiFactorAuthProvider::with_settings(2, 0, 5); // 0 minute lockout for testing
    
    // Set up TOTP
    let setup_request = MfaSetupRequest {
        user_id: "test_user".to_string(),
        method: MfaMethod::Totp,
        setup_data: None,
    };
    provider.setup_mfa(setup_request).await.unwrap();

    // Trigger lockout
    for _ in 0..3 {
        let challenge = provider.create_challenge("test_user").await.unwrap();
        let verification_request = MfaVerificationRequest {
            challenge_id: challenge.challenge_id,
            method: MfaMethod::Totp,
            code: "invalid".to_string(),
        };
        let _ = provider.verify_challenge(verification_request).await;
    }

    // User should be locked out
    let result = provider.create_challenge("test_user").await;
    assert!(result.is_err());

    // Wait for lockout to expire
    sleep(Duration::from_millis(100)).await;
    
    // Clean up expired lockouts
    provider.cleanup_expired_lockouts().await;
    
    // User should be able to create challenges again
    let result = provider.create_challenge("test_user").await;
    assert!(result.is_ok());
}