//! Simple test for MFA functionality

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_basic_mfa_functionality() {
        let provider = MultiFactorAuthProvider::new();
        
        // Test initial state
        assert!(!provider.is_mfa_enabled("test_user").await);
        
        // Test TOTP setup
        let request = MfaSetupRequest {
            user_id: "test_user".to_string(),
            method: MfaMethod::Totp,
            setup_data: None,
        };
        
        let response = provider.setup_mfa(request).await.unwrap();
        assert!(response.success);
        assert!(response.setup_data.is_some());
        assert!(response.backup_codes.is_some());
        
        // Test MFA is now enabled
        assert!(provider.is_mfa_enabled("test_user").await);
        
        println!("✅ Basic MFA functionality test passed!");
    }
}