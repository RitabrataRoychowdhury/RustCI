use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use uuid::Uuid;
use anyhow::{Result, anyhow};

use crate::testing::{TestCategory, CategoryResults, FailedTestDetail, TestSuiteConfig};

/// Test orchestrator for managing parallel test execution and coordination
#[derive(Debug, Clone)]
pub struct TestOrchestrator {
    config: TestSuiteConfig,
    semaphore: Arc<Semaphore>,
    test_registry: Arc<Mutex<TestRegistry>>,
}

#[derive(Debug)]
struct TestRegistry {
    registered_tests: HashMap<TestCategory, Vec<TestDefinition>>,
    test_results: HashMap<Uuid, TestExecutionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDefinition {
    pub id: Uuid,
    pub name: String,
    pub category: TestCategory,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub environment_variables: HashMap<String, String>,
    pub timeout: Duration,
    pub retry_count: u32,
    pub dependencies: Vec<Uuid>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionResult {
    pub test_id: Uuid,
    pub test_name: String,
    pub category: TestCategory,
    pub status: TestStatus,
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
pub enum TestStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
    Timeout,
    Cancelled,
}

#[async_trait]
pub trait TestOrchestratorInterface {
    async fn register_test(&self, test_definition: TestDefinition) -> Result<()>;
    async fn run_test_category(&self, category: TestCategory) -> Result<CategoryResults>;
    async fn run_single_test(&self, test_id: Uuid) -> Result<TestExecutionResult>;
    async fn cancel_test(&self, test_id: Uuid) -> Result<()>;
    async fn get_test_results(&self, category: Option<TestCategory>) -> Result<Vec<TestExecutionResult>>;
    async fn cleanup_test_artifacts(&self) -> Result<()>;
}

impl TestOrchestrator {
    pub fn new(config: TestSuiteConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.parallel_test_limit));
        let test_registry = Arc::new(Mutex::new(TestRegistry {
            registered_tests: HashMap::new(),
            test_results: HashMap::new(),
        }));

        Self {
            config,
            semaphore,
            test_registry,
        }
    }

    async fn discover_tests(&self) -> Result<()> {
        tracing::info!("Discovering tests in the codebase");

        // Discover unit tests
        self.discover_unit_tests().await?;
        
        // Discover integration tests
        self.discover_integration_tests().await?;
        
        // Discover performance tests
        self.discover_performance_tests().await?;
        
        // Discover security tests
        self.discover_security_tests().await?;

        tracing::info!("Test discovery completed");
        Ok(())
    }

    async fn discover_unit_tests(&self) -> Result<()> {
        let unit_tests = vec![
            TestDefinition {
                id: Uuid::new_v4(),
                name: "cargo_test_lib".to_string(),
                category: TestCategory::Unit,
                command: "cargo".to_string(),
                args: vec!["test".to_string(), "--lib".to_string()],
                working_directory: None,
                environment_variables: HashMap::new(),
                timeout: Duration::from_secs(300),
                retry_count: 2,
                dependencies: Vec::new(),
                tags: vec!["unit".to_string(), "fast".to_string()],
            },
            TestDefinition {
                id: Uuid::new_v4(),
                name: "cargo_test_bins".to_string(),
                category: TestCategory::Unit,
                command: "cargo".to_string(),
                args: vec!["test".to_string(), "--bins".to_string()],
                working_directory: None,
                environment_variables: HashMap::new(),
                timeout: Duration::from_secs(300),
                retry_count: 2,
                dependencies: Vec::new(),
                tags: vec!["unit".to_string(), "binary".to_string()],
            },
        ];

        let mut registry = self.test_registry.lock().await;
        registry.registered_tests.insert(TestCategory::Unit, unit_tests);
        Ok(())
    }

    async fn discover_integration_tests(&self) -> Result<()> {
        let integration_tests = vec![
            TestDefinition {
                id: Uuid::new_v4(),
                name: "integration_tests".to_string(),
                category: TestCategory::Integration,
                command: "cargo".to_string(),
                args: vec!["test".to_string(), "--test".to_string(), "*".to_string()],
                working_directory: None,
                environment_variables: HashMap::new(),
                timeout: Duration::from_secs(600),
                retry_count: 1,
                dependencies: Vec::new(),
                tags: vec!["integration".to_string(), "slow".to_string()],
            },
        ];

        let mut registry = self.test_registry.lock().await;
        registry.registered_tests.insert(TestCategory::Integration, integration_tests);
        Ok(())
    }

    async fn discover_performance_tests(&self) -> Result<()> {
        let performance_tests = vec![
            TestDefinition {
                id: Uuid::new_v4(),
                name: "benchmark_tests".to_string(),
                category: TestCategory::Performance,
                command: "cargo".to_string(),
                args: vec!["bench".to_string()],
                working_directory: None,
                environment_variables: HashMap::new(),
                timeout: Duration::from_secs(1800),
                retry_count: 0,
                dependencies: Vec::new(),
                tags: vec!["performance".to_string(), "benchmark".to_string()],
            },
        ];

        let mut registry = self.test_registry.lock().await;
        registry.registered_tests.insert(TestCategory::Performance, performance_tests);
        Ok(())
    }

    async fn discover_security_tests(&self) -> Result<()> {
        let security_tests = vec![
            TestDefinition {
                id: Uuid::new_v4(),
                name: "security_audit".to_string(),
                category: TestCategory::Security,
                command: "cargo".to_string(),
                args: vec!["audit".to_string()],
                working_directory: None,
                environment_variables: HashMap::new(),
                timeout: Duration::from_secs(300),
                retry_count: 1,
                dependencies: Vec::new(),
                tags: vec!["security".to_string(), "audit".to_string()],
            },
        ];

        let mut registry = self.test_registry.lock().await;
        registry.registered_tests.insert(TestCategory::Security, security_tests);
        Ok(())
    }

    async fn execute_test(&self, test_definition: TestDefinition) -> Result<TestExecutionResult> {
        let _permit = self.semaphore.acquire().await?;
        
        let started_at = Utc::now();
        let mut result = TestExecutionResult {
            test_id: test_definition.id,
            test_name: test_definition.name.clone(),
            category: test_definition.category.clone(),
            status: TestStatus::Running,
            started_at,
            completed_at: None,
            duration: None,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            retry_attempts: 0,
            error_message: None,
        };

        tracing::info!("Executing test: {}", test_definition.name);

        let start_time = Instant::now();
        
        for attempt in 0..=test_definition.retry_count {
            result.retry_attempts = attempt;
            
            match self.run_test_command(&test_definition).await {
                Ok((exit_code, stdout, stderr)) => {
                    result.exit_code = Some(exit_code);
                    result.stdout = stdout;
                    result.stderr = stderr;
                    
                    if exit_code == 0 {
                        result.status = TestStatus::Passed;
                        break;
                    } else {
                        result.status = TestStatus::Failed;
                        result.error_message = Some(format!("Test failed with exit code: {}", exit_code));
                        
                        if attempt < test_definition.retry_count {
                            tracing::warn!("Test {} failed, retrying (attempt {}/{})", 
                                test_definition.name, attempt + 1, test_definition.retry_count);
                            tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1) as u64)).await;
                        }
                    }
                }
                Err(e) => {
                    result.status = TestStatus::Failed;
                    result.error_message = Some(e.to_string());
                    
                    if attempt < test_definition.retry_count {
                        tracing::warn!("Test {} failed with error, retrying (attempt {}/{}): {}", 
                            test_definition.name, attempt + 1, test_definition.retry_count, e);
                        tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1) as u64)).await;
                    }
                }
            }
        }

        result.duration = Some(start_time.elapsed());
        result.completed_at = Some(Utc::now());

        tracing::info!("Test {} completed with status: {:?}", 
            test_definition.name, result.status);

        Ok(result)
    }

    async fn run_test_command(&self, test_definition: &TestDefinition) -> Result<(i32, String, String)> {
        let mut command = Command::new(&test_definition.command);
        command.args(&test_definition.args);

        if let Some(working_dir) = &test_definition.working_directory {
            command.current_dir(working_dir);
        }

        for (key, value) in &test_definition.environment_variables {
            command.env(key, value);
        }

        let join_result = tokio::time::timeout(
            test_definition.timeout,
            tokio::task::spawn_blocking(move || command.output())
        ).await?;
        
        let command_result = join_result?;
        let output = command_result?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((exit_code, stdout, stderr))
    }

    async fn aggregate_category_results(&self, category: TestCategory) -> Result<CategoryResults> {
        let registry = self.test_registry.lock().await;
        let results: Vec<&TestExecutionResult> = registry.test_results
            .values()
            .filter(|result| result.category == category)
            .collect();

        let total = results.len() as u32;
        let passed = results.iter().filter(|r| r.status == TestStatus::Passed).count() as u32;
        let failed = results.iter().filter(|r| r.status == TestStatus::Failed).count() as u32;
        let skipped = results.iter().filter(|r| r.status == TestStatus::Skipped).count() as u32;

        let total_duration = results.iter()
            .filter_map(|r| r.duration)
            .fold(Duration::ZERO, |acc, d| acc + d);

        let failed_test_details: Vec<FailedTestDetail> = results.iter()
            .filter(|r| r.status == TestStatus::Failed)
            .map(|r| FailedTestDetail {
                test_name: r.test_name.clone(),
                error_message: r.error_message.clone().unwrap_or_default(),
                stack_trace: Some(r.stderr.clone()),
                retry_count: r.retry_attempts,
                category: r.category.clone(),
            })
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
}

#[async_trait]
impl TestOrchestratorInterface for TestOrchestrator {
    async fn register_test(&self, test_definition: TestDefinition) -> Result<()> {
        let mut registry = self.test_registry.lock().await;
        let category = test_definition.category.clone();
        
        registry.registered_tests
            .entry(category)
            .or_insert_with(Vec::new)
            .push(test_definition);
        
        Ok(())
    }

    async fn run_test_category(&self, category: TestCategory) -> Result<CategoryResults> {
        // Discover tests if not already done
        self.discover_tests().await?;

        let tests = {
            let registry = self.test_registry.lock().await;
            registry.registered_tests
                .get(&category)
                .cloned()
                .unwrap_or_default()
        };

        if tests.is_empty() {
            tracing::warn!("No tests found for category: {:?}", category);
            return Ok(CategoryResults {
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
                duration: Duration::ZERO,
                failed_test_details: Vec::new(),
            });
        }

        tracing::info!("Running {} tests for category: {:?}", tests.len(), category);

        let mut handles: Vec<JoinHandle<Result<TestExecutionResult>>> = Vec::new();

        for test in tests {
            let orchestrator = self.clone();
            let handle = tokio::spawn(async move {
                orchestrator.execute_test(test).await
            });
            handles.push(handle);
        }

        // Wait for all tests to complete
        let mut results = Vec::new();
        for handle in handles {
            match handle.await? {
                Ok(result) => {
                    let mut registry = self.test_registry.lock().await;
                    registry.test_results.insert(result.test_id, result.clone());
                    results.push(result);
                }
                Err(e) => {
                    tracing::error!("Test execution failed: {}", e);
                    return Err(e);
                }
            }
        }

        self.aggregate_category_results(category).await
    }

    async fn run_single_test(&self, test_id: Uuid) -> Result<TestExecutionResult> {
        let test_definition = {
            let registry = self.test_registry.lock().await;
            registry.registered_tests
                .values()
                .flatten()
                .find(|test| test.id == test_id)
                .cloned()
                .ok_or_else(|| anyhow!("Test not found: {}", test_id))?
        };

        let result = self.execute_test(test_definition).await?;
        
        {
            let mut registry = self.test_registry.lock().await;
            registry.test_results.insert(test_id, result.clone());
        }

        Ok(result)
    }

    async fn cancel_test(&self, test_id: Uuid) -> Result<()> {
        // Implementation would involve tracking running processes and killing them
        // For now, we'll just mark the test as cancelled in the registry
        let mut registry = self.test_registry.lock().await;
        if let Some(result) = registry.test_results.get_mut(&test_id) {
            if result.status == TestStatus::Running {
                result.status = TestStatus::Cancelled;
                result.completed_at = Some(Utc::now());
            }
        }
        Ok(())
    }

    async fn get_test_results(&self, category: Option<TestCategory>) -> Result<Vec<TestExecutionResult>> {
        let registry = self.test_registry.lock().await;
        let results = if let Some(cat) = category {
            registry.test_results
                .values()
                .filter(|result| result.category == cat)
                .cloned()
                .collect()
        } else {
            registry.test_results.values().cloned().collect()
        };
        Ok(results)
    }

    async fn cleanup_test_artifacts(&self) -> Result<()> {
        tracing::info!("Cleaning up test artifacts");
        
        // Clean up temporary files, test databases, etc.
        let cleanup_commands = vec![
            ("rm", vec!["-rf", "target/tmp"]),
            ("rm", vec!["-rf", "test_output"]),
        ];

        for (cmd, args) in cleanup_commands {
            let output = Command::new(cmd)
                .args(&args)
                .output();
                
            match output {
                Ok(_) => tracing::debug!("Cleanup command succeeded: {} {:?}", cmd, args),
                Err(e) => tracing::warn!("Cleanup command failed: {} {:?} - {}", cmd, args, e),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let config = TestSuiteConfig::default();
        let orchestrator = TestOrchestrator::new(config);
        
        assert_eq!(orchestrator.config.parallel_test_limit, num_cpus::get());
    }

    #[tokio::test]
    async fn test_register_test() {
        let config = TestSuiteConfig::default();
        let orchestrator = TestOrchestrator::new(config);
        
        let test_def = TestDefinition {
            id: Uuid::new_v4(),
            name: "test_example".to_string(),
            category: TestCategory::Unit,
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            working_directory: None,
            environment_variables: HashMap::new(),
            timeout: Duration::from_secs(30),
            retry_count: 0,
            dependencies: Vec::new(),
            tags: vec!["example".to_string()],
        };

        let result = orchestrator.register_test(test_def).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_single_test() {
        let config = TestSuiteConfig::default();
        let orchestrator = TestOrchestrator::new(config);
        
        let test_def = TestDefinition {
            id: Uuid::new_v4(),
            name: "echo_test".to_string(),
            category: TestCategory::Unit,
            command: "echo".to_string(),
            args: vec!["test_output".to_string()],
            working_directory: None,
            environment_variables: HashMap::new(),
            timeout: Duration::from_secs(30),
            retry_count: 0,
            dependencies: Vec::new(),
            tags: vec!["simple".to_string()],
        };

        orchestrator.register_test(test_def.clone()).await.unwrap();
        let result = orchestrator.run_single_test(test_def.id).await.unwrap();
        
        assert_eq!(result.status, TestStatus::Passed);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("test_output"));
    }
}