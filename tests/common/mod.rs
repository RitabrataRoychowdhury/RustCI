use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// Test utilities and common setup
pub mod assertions;
pub mod builders;
pub mod containers;
pub mod database;
pub mod fixtures;
pub mod mocks;

pub use assertions::*;
pub use builders::*;
pub use containers::*;
pub use database::*;
pub use fixtures::*;
pub use mocks::*;

/// Test environment setup
pub struct TestEnvironment {
    pub database: TestDatabase,
    pub message_bus: TestMessageBus,
    pub external_services: MockExternalServices,
    pub config: TestConfig,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        Self {
            database: TestDatabase::new().await,
            message_bus: TestMessageBus::new(),
            external_services: MockExternalServices::new(),
            config: TestConfig::default(),
        }
    }

    pub async fn cleanup(&self) {
        self.database.cleanup().await;
        self.message_bus.cleanup().await;
    }
}

/// Test configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub github_token: String,
    pub test_timeout: std::time::Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: "mongodb://localhost:27017/rustci_test".to_string(),
            jwt_secret: "test_secret_key".to_string(),
            github_token: "test_github_token".to_string(),
            test_timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// Test result wrapper
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Common test macros
#[macro_export]
macro_rules! assert_error_type {
    ($result:expr, $error_type:pat) => {
        match $result {
            Err($error_type) => {}
            other => panic!("Expected error type, got: {:?}", other),
        }
    };
}

#[macro_export]
macro_rules! test_async {
    ($test_name:ident, $test_body:expr) => {
        #[tokio::test]
        async fn $test_name() -> TestResult {
            let env = TestEnvironment::new().await;
            let result = $test_body(&env).await;
            env.cleanup().await;
            result
        }
    };
}
