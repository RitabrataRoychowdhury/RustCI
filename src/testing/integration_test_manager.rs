use async_trait::async_trait;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::{CategoryResults, TestCategory, FailedTestDetail};

/// Integration test manager for automated integration testing
#[derive(Debug, Clone)]
pub struct IntegrationTestManager {
    config: IntegrationTestConfig,
    test_database: Arc<Mutex<Option<TestDatabase>>>,
    mock_services: Arc<Mutex<Vec<MockService>>>,
    test_environments: Arc<Mutex<HashMap<String, TestEnvironment>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTestConfig {
    pub database_url: String,
    pub test_database_name: String,
    pub mock_service_port_range: (u16, u16),
    pub test_timeout: Duration,
    pub cleanup_after_tests: bool,
    pub parallel_test_limit: usize,
    pub retry_failed_tests: bool,
    pub max_retries: u32,
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self {
            database_url: "mongodb://localhost:27017".to_string(),
            test_database_name: "rustci_integration_test".to_string(),
            mock_service_port_range: (8000, 8100),
            test_timeout: Duration::from_secs(300),
            cleanup_after_tests: true,
            parallel_test_limit: 4,
            retry_failed_tests: true,
            max_retries: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestDatabase {
    pub connection_string: String,
    pub database_name: String,
    pub collections: Vec<String>,
    pub is_initialized: bool,
}

#[derive(Debug, Clone)]
pub struct MockService {
    pub service_id: Uuid,
    pub name: String,
    pub port: u16,
    pub endpoints: Vec<MockEndpoint>,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEndpoint {
    pub path: String,
    pub method: String,
    pub response_body: String,
    pub response_status: u16,
    pub response_headers: HashMap<String, String>,
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TestEnvironment {
    pub environment_id: Uuid,
    pub name: String,
    pub database: Option<TestDatabase>,
    pub mock_services: Vec<MockService>,
    pub environment_variables: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTestSuite {
    pub suite_id: Uuid,
    pub name: String,
    pub tests: Vec<IntegrationTest>,
    pub setup_scripts: Vec<String>,
    pub teardown_scripts: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTest {
    pub test_id: Uuid,
    pub name: String,
    pub description: String,
    pub test_type: IntegrationTestType,
    pub command: String,
    pub args: Vec<String>,
    pub environment_variables: HashMap<String, String>,
    pub expected_exit_code: i32,
    pub timeout: Duration,
    pub retry_count: u32,
    pub dependencies: Vec<Uuid>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationTestType {
    ApiTest,
    DatabaseTest,
    ServiceIntegration,
    EndToEnd,
    ContractTest,
    PerformanceIntegration,
}

#[async_trait]
pub trait IntegrationTestManagerInterface {
    async fn setup_test_database(&self) -> Result<()>;
    async fn setup_mock_services(&self) -> Result<()>;
    async fn cleanup_test_data(&self) -> Result<()>;
    async fn run_all_integration_tests(&self) -> Result<CategoryResults>;
    async fn run_test_suite(&self, suite: &IntegrationTestSuite) -> Result<CategoryResults>;
    async fn create_test_environment(&self, name: &str) -> Result<Uuid>;
    async fn destroy_test_environment(&self, environment_id: Uuid) -> Result<()>;
}

impl IntegrationTestManager {
    pub fn new() -> Self {
        Self::with_config(IntegrationTestConfig::default())
    }

    pub fn with_config(config: IntegrationTestConfig) -> Self {
        Self {
            config,
            test_database: Arc::new(Mutex::new(None)),
            mock_services: Arc::new(Mutex::new(Vec::new())),
            test_environments: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn initialize_test_database(&self) -> Result<TestDatabase> {
        tracing::info!("Initializing test database: {}", self.config.test_database_name);

        let connection_string = format!("{}/{}", 
            self.config.database_url, 
            self.config.test_database_name
        );

        // Create test database collections
        let collections = vec![
            "test_users".to_string(),
            "test_jobs".to_string(),
            "test_pipelines".to_string(),
            "test_executions".to_string(),
            "test_runners".to_string(),
        ];

        let test_db = TestDatabase {
            connection_string: connection_string.clone(),
            database_name: self.config.test_database_name.clone(),
            collections: collections.clone(),
            is_initialized: true,
        };

        // Initialize collections with test data
        self.seed_test_data(&test_db).await?;

        tracing::info!("Test database initialized successfully");
        Ok(test_db)
    }

    async fn seed_test_data(&self, _test_db: &TestDatabase) -> Result<()> {
        tracing::info!("Seeding test database with initial data");

        // In a real implementation, this would:
        // 1. Connect to the test database
        // 2. Create necessary collections
        // 3. Insert test data fixtures
        // 4. Set up indexes and constraints

        // For now, we'll simulate this process
        tokio::time::sleep(Duration::from_millis(100)).await;

        tracing::info!("Test data seeding completed");
        Ok(())
    }

    async fn start_mock_service(&self, service_name: &str, port: u16) -> Result<MockService> {
        tracing::info!("Starting mock service '{}' on port {}", service_name, port);

        let endpoints = match service_name {
            "github_api" => vec![
                MockEndpoint {
                    path: "/user".to_string(),
                    method: "GET".to_string(),
                    response_body: r#"{"login": "testuser", "id": 12345}"#.to_string(),
                    response_status: 200,
                    response_headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers
                    },
                    delay_ms: Some(50),
                },
                MockEndpoint {
                    path: "/repos/testuser/testrepo/hooks".to_string(),
                    method: "POST".to_string(),
                    response_body: r#"{"id": 67890, "url": "http://localhost/webhook"}"#.to_string(),
                    response_status: 201,
                    response_headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers
                    },
                    delay_ms: Some(100),
                },
            ],
            "docker_registry" => vec![
                MockEndpoint {
                    path: "/v2/_catalog".to_string(),
                    method: "GET".to_string(),
                    response_body: r#"{"repositories": ["rust", "alpine"]}"#.to_string(),
                    response_status: 200,
                    response_headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers
                    },
                    delay_ms: Some(25),
                },
            ],
            _ => vec![
                MockEndpoint {
                    path: "/health".to_string(),
                    method: "GET".to_string(),
                    response_body: r#"{"status": "ok"}"#.to_string(),
                    response_status: 200,
                    response_headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers
                    },
                    delay_ms: None,
                },
            ],
        };

        let mock_service = MockService {
            service_id: Uuid::new_v4(),
            name: service_name.to_string(),
            port,
            endpoints,
            is_running: true,
        };

        // In a real implementation, this would start an actual HTTP server
        // For now, we'll simulate the service startup
        tokio::time::sleep(Duration::from_millis(50)).await;

        tracing::info!("Mock service '{}' started successfully", service_name);
        Ok(mock_service)
    }

    async fn stop_mock_service(&self, service: &MockService) -> Result<()> {
        tracing::info!("Stopping mock service '{}'", service.name);

        // In a real implementation, this would stop the HTTP server
        tokio::time::sleep(Duration::from_millis(25)).await;

        tracing::info!("Mock service '{}' stopped", service.name);
        Ok(())
    }

    async fn discover_integration_tests(&self) -> Result<Vec<IntegrationTestSuite>> {
        tracing::info!("Discovering integration test suites");

        let suites = vec![
            IntegrationTestSuite {
                suite_id: Uuid::new_v4(),
                name: "API Integration Tests".to_string(),
                tests: vec![
                    IntegrationTest {
                        test_id: Uuid::new_v4(),
                        name: "test_user_authentication".to_string(),
                        description: "Test user authentication flow".to_string(),
                        test_type: IntegrationTestType::ApiTest,
                        command: "cargo".to_string(),
                        args: vec!["test".to_string(), "test_user_authentication".to_string(), "--".to_string(), "--nocapture".to_string()],
                        environment_variables: {
                            let mut env = HashMap::new();
                            env.insert("TEST_MODE".to_string(), "integration".to_string());
                            env.insert("DATABASE_URL".to_string(), self.config.database_url.clone());
                            env
                        },
                        expected_exit_code: 0,
                        timeout: Duration::from_secs(60),
                        retry_count: 1,
                        dependencies: Vec::new(),
                        tags: vec!["api".to_string(), "auth".to_string()],
                    },
                    IntegrationTest {
                        test_id: Uuid::new_v4(),
                        name: "test_pipeline_execution".to_string(),
                        description: "Test complete pipeline execution flow".to_string(),
                        test_type: IntegrationTestType::EndToEnd,
                        command: "cargo".to_string(),
                        args: vec!["test".to_string(), "test_pipeline_execution".to_string(), "--".to_string(), "--nocapture".to_string()],
                        environment_variables: {
                            let mut env = HashMap::new();
                            env.insert("TEST_MODE".to_string(), "integration".to_string());
                            env.insert("DATABASE_URL".to_string(), self.config.database_url.clone());
                            env
                        },
                        expected_exit_code: 0,
                        timeout: Duration::from_secs(120),
                        retry_count: 2,
                        dependencies: Vec::new(),
                        tags: vec!["pipeline".to_string(), "e2e".to_string()],
                    },
                ],
                setup_scripts: vec![
                    "echo 'Setting up API test environment'".to_string(),
                ],
                teardown_scripts: vec![
                    "echo 'Cleaning up API test environment'".to_string(),
                ],
                dependencies: vec!["test_database".to_string(), "mock_services".to_string()],
            },
            IntegrationTestSuite {
                suite_id: Uuid::new_v4(),
                name: "Database Integration Tests".to_string(),
                tests: vec![
                    IntegrationTest {
                        test_id: Uuid::new_v4(),
                        name: "test_database_operations".to_string(),
                        description: "Test database CRUD operations".to_string(),
                        test_type: IntegrationTestType::DatabaseTest,
                        command: "cargo".to_string(),
                        args: vec!["test".to_string(), "test_database_operations".to_string(), "--".to_string(), "--nocapture".to_string()],
                        environment_variables: {
                            let mut env = HashMap::new();
                            env.insert("TEST_MODE".to_string(), "integration".to_string());
                            env.insert("DATABASE_URL".to_string(), self.config.database_url.clone());
                            env
                        },
                        expected_exit_code: 0,
                        timeout: Duration::from_secs(90),
                        retry_count: 1,
                        dependencies: Vec::new(),
                        tags: vec!["database".to_string(), "crud".to_string()],
                    },
                ],
                setup_scripts: vec![
                    "echo 'Setting up database test environment'".to_string(),
                ],
                teardown_scripts: vec![
                    "echo 'Cleaning up database test environment'".to_string(),
                ],
                dependencies: vec!["test_database".to_string()],
            },
        ];

        tracing::info!("Discovered {} integration test suites", suites.len());
        Ok(suites)
    }

    async fn execute_integration_test(&self, test: &IntegrationTest) -> Result<IntegrationTestResult> {
        tracing::info!("Executing integration test: {}", test.name);

        let start_time = Instant::now();
        let mut result = IntegrationTestResult {
            test_id: test.test_id,
            test_name: test.name.clone(),
            test_type: test.test_type.clone(),
            status: IntegrationTestStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            duration: None,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            retry_attempts: 0,
            error_message: None,
        };

        for attempt in 0..=test.retry_count {
            result.retry_attempts = attempt;

            match self.run_test_command(test).await {
                Ok((exit_code, stdout, stderr)) => {
                    result.exit_code = Some(exit_code);
                    result.stdout = stdout;
                    result.stderr = stderr;

                    if exit_code == test.expected_exit_code {
                        result.status = IntegrationTestStatus::Passed;
                        break;
                    } else {
                        result.status = IntegrationTestStatus::Failed;
                        result.error_message = Some(format!(
                            "Test failed with exit code: {} (expected: {})",
                            exit_code, test.expected_exit_code
                        ));

                        if attempt < test.retry_count {
                            tracing::warn!(
                                "Integration test {} failed, retrying (attempt {}/{})",
                                test.name, attempt + 1, test.retry_count
                            );
                            tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1) as u64)).await;
                        }
                    }
                }
                Err(e) => {
                    result.status = IntegrationTestStatus::Failed;
                    result.error_message = Some(e.to_string());

                    if attempt < test.retry_count {
                        tracing::warn!(
                            "Integration test {} failed with error, retrying (attempt {}/{}): {}",
                            test.name, attempt + 1, test.retry_count, e
                        );
                        tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1) as u64)).await;
                    }
                }
            }
        }

        result.duration = Some(start_time.elapsed());
        result.completed_at = Some(Utc::now());

        tracing::info!(
            "Integration test {} completed with status: {:?}",
            test.name, result.status
        );

        Ok(result)
    }

    async fn run_test_command(&self, test: &IntegrationTest) -> Result<(i32, String, String)> {
        let mut command = Command::new(&test.command);
        command.args(&test.args);

        for (key, value) in &test.environment_variables {
            command.env(key, value);
        }

        let join_result = tokio::time::timeout(
            test.timeout,
            tokio::task::spawn_blocking(move || command.output())
        ).await?;

        let command_result = join_result?;
        let output = command_result?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((exit_code, stdout, stderr))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTestResult {
    pub test_id: Uuid,
    pub test_name: String,
    pub test_type: IntegrationTestType,
    pub status: IntegrationTestStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub retry_attempts: u32,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntegrationTestStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
    Timeout,
}

#[async_trait]
impl IntegrationTestManagerInterface for IntegrationTestManager {
    async fn setup_test_database(&self) -> Result<()> {
        let test_db = self.initialize_test_database().await?;
        let mut db_guard = self.test_database.lock().await;
        *db_guard = Some(test_db);
        Ok(())
    }

    async fn setup_mock_services(&self) -> Result<()> {
        let services_to_start = vec![
            ("github_api", 8001),
            ("docker_registry", 8002),
            ("notification_service", 8003),
        ];

        let mut mock_services = self.mock_services.lock().await;
        
        for (service_name, port) in services_to_start {
            let service = self.start_mock_service(service_name, port).await?;
            mock_services.push(service);
        }

        tracing::info!("All mock services started successfully");
        Ok(())
    }

    async fn cleanup_test_data(&self) -> Result<()> {
        if !self.config.cleanup_after_tests {
            tracing::info!("Test data cleanup disabled, skipping");
            return Ok(());
        }

        tracing::info!("Cleaning up test data");

        // Stop mock services
        let mock_services = self.mock_services.lock().await;
        for service in mock_services.iter() {
            self.stop_mock_service(service).await?;
        }
        drop(mock_services);

        // Clear mock services
        self.mock_services.lock().await.clear();

        // Clean up test database
        let mut db_guard = self.test_database.lock().await;
        if let Some(test_db) = db_guard.take() {
            tracing::info!("Dropping test database: {}", test_db.database_name);
            // In a real implementation, this would drop the test database
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Clean up test environments
        self.test_environments.lock().await.clear();

        tracing::info!("Test data cleanup completed");
        Ok(())
    }

    async fn run_all_integration_tests(&self) -> Result<CategoryResults> {
        tracing::info!("Running all integration tests");

        let test_suites = self.discover_integration_tests().await?;
        let mut all_results = Vec::new();

        for suite in &test_suites {
            let suite_results = self.run_test_suite(suite).await?;
            all_results.push(suite_results);
        }

        // Aggregate results from all suites
        let total = all_results.iter().map(|r| r.total).sum();
        let passed = all_results.iter().map(|r| r.passed).sum();
        let failed = all_results.iter().map(|r| r.failed).sum();
        let skipped = all_results.iter().map(|r| r.skipped).sum();

        let total_duration = all_results.iter()
            .map(|r| r.duration)
            .fold(Duration::ZERO, |acc, d| acc + d);

        let failed_test_details: Vec<FailedTestDetail> = all_results.iter()
            .flat_map(|r| r.failed_test_details.clone())
            .collect();

        Ok(CategoryResults {
            total,
            passed,
            failed,
            skipped,
            duration: total_duration,
            failed_test_details,
        })
    }

    async fn run_test_suite(&self, suite: &IntegrationTestSuite) -> Result<CategoryResults> {
        tracing::info!("Running integration test suite: {}", suite.name);

        // Run setup scripts
        for script in &suite.setup_scripts {
            tracing::debug!("Running setup script: {}", script);
            // In a real implementation, this would execute the script
        }

        let mut test_results = Vec::new();

        // Execute tests
        for test in &suite.tests {
            let result = self.execute_integration_test(test).await?;
            test_results.push(result);
        }

        // Run teardown scripts
        for script in &suite.teardown_scripts {
            tracing::debug!("Running teardown script: {}", script);
            // In a real implementation, this would execute the script
        }

        // Aggregate results
        let total = test_results.len() as u32;
        let passed = test_results.iter()
            .filter(|r| r.status == IntegrationTestStatus::Passed)
            .count() as u32;
        let failed = test_results.iter()
            .filter(|r| r.status == IntegrationTestStatus::Failed)
            .count() as u32;
        let skipped = test_results.iter()
            .filter(|r| r.status == IntegrationTestStatus::Skipped)
            .count() as u32;

        let total_duration = test_results.iter()
            .filter_map(|r| r.duration)
            .fold(Duration::ZERO, |acc, d| acc + d);

        let failed_test_details: Vec<FailedTestDetail> = test_results.iter()
            .filter(|r| r.status == IntegrationTestStatus::Failed)
            .map(|r| FailedTestDetail {
                test_name: r.test_name.clone(),
                error_message: r.error_message.clone().unwrap_or_default(),
                stack_trace: Some(r.stderr.clone()),
                retry_count: r.retry_attempts,
                category: TestCategory::Integration,
            })
            .collect();

        tracing::info!(
            "Integration test suite '{}' completed: {} passed, {} failed, {} skipped",
            suite.name, passed, failed, skipped
        );

        Ok(CategoryResults {
            total,
            passed,
            failed,
            skipped,
            duration: total_duration,
            failed_test_details,
        })
    }

    async fn create_test_environment(&self, name: &str) -> Result<Uuid> {
        let environment_id = Uuid::new_v4();
        
        let environment = TestEnvironment {
            environment_id,
            name: name.to_string(),
            database: None,
            mock_services: Vec::new(),
            environment_variables: HashMap::new(),
            created_at: Utc::now(),
        };

        self.test_environments.lock().await.insert(
            environment_id.to_string(),
            environment
        );

        tracing::info!("Created test environment '{}' with ID: {}", name, environment_id);
        Ok(environment_id)
    }

    async fn destroy_test_environment(&self, environment_id: Uuid) -> Result<()> {
        let mut environments = self.test_environments.lock().await;
        
        if let Some(environment) = environments.remove(&environment_id.to_string()) {
            tracing::info!("Destroyed test environment '{}' (ID: {})", environment.name, environment_id);
            Ok(())
        } else {
            Err(anyhow!("Test environment not found: {}", environment_id))
        }
    }
}

impl Default for IntegrationTestManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_test_manager_creation() {
        let manager = IntegrationTestManager::new();
        assert_eq!(manager.config.test_database_name, "rustci_integration_test");
    }

    #[tokio::test]
    async fn test_create_test_environment() {
        let manager = IntegrationTestManager::new();
        let env_id = manager.create_test_environment("test_env").await.unwrap();
        
        let environments = manager.test_environments.lock().await;
        assert!(environments.contains_key(&env_id.to_string()));
    }

    #[tokio::test]
    async fn test_discover_integration_tests() {
        let manager = IntegrationTestManager::new();
        let suites = manager.discover_integration_tests().await.unwrap();
        
        assert!(!suites.is_empty());
        assert!(suites.iter().any(|s| s.name == "API Integration Tests"));
    }

    #[test]
    fn test_integration_test_config_default() {
        let config = IntegrationTestConfig::default();
        assert_eq!(config.database_url, "mongodb://localhost:27017");
        assert_eq!(config.parallel_test_limit, 4);
        assert!(config.cleanup_after_tests);
    }
}