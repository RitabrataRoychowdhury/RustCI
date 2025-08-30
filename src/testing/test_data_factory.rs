use async_trait::async_trait;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use rand::Rng;

/// Test data factory for generating realistic test data
#[derive(Debug, Clone)]
pub struct TestDataFactory {
    config: TestDataConfig,
    generated_data: Arc<Mutex<HashMap<String, Vec<serde_json::Value>>>>,
    database_seeder: Arc<Mutex<Option<DatabaseSeeder>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataConfig {
    pub cleanup_after_tests: bool,
    pub use_realistic_data: bool,
    pub data_size_limits: HashMap<String, usize>,
    pub database_url: Option<String>,
    pub seed_database: bool,
}

impl Default for TestDataConfig {
    fn default() -> Self {
        let mut data_size_limits = HashMap::new();
        data_size_limits.insert("users".to_string(), 1000);
        data_size_limits.insert("jobs".to_string(), 500);
        data_size_limits.insert("pipelines".to_string(), 100);
        data_size_limits.insert("executions".to_string(), 200);

        Self {
            cleanup_after_tests: true,
            use_realistic_data: true,
            data_size_limits,
            database_url: Some("mongodb://localhost:27017/rustci_test".to_string()),
            seed_database: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseSeeder {
    connection_string: String,
    collections: Vec<String>,
    is_connected: bool,
}

#[async_trait]
pub trait TestDataFactoryInterface {
    async fn generate_test_users(&self, count: usize) -> Result<Vec<TestUser>>;
    async fn generate_test_jobs(&self, count: usize) -> Result<Vec<TestJob>>;
    async fn generate_test_pipelines(&self, count: usize) -> Result<Vec<TestPipeline>>;
    async fn generate_test_executions(&self, count: usize) -> Result<Vec<TestExecution>>;
    async fn seed_database(&self) -> Result<()>;
    async fn cleanup_test_data(&self) -> Result<()>;
    async fn create_test_scenario(&self, scenario_name: &str) -> Result<TestScenario>;
}

impl TestDataFactory {
    pub fn new() -> Self {
        Self::with_config(TestDataConfig::default())
    }

    pub fn with_config(config: TestDataConfig) -> Self {
        Self {
            config,
            generated_data: Arc::new(Mutex::new(HashMap::new())),
            database_seeder: Arc::new(Mutex::new(None)),
        }
    }

    pub fn generate_test_user(&self) -> TestUser {
        let mut rng = rand::thread_rng();
        let user_id = rng.gen_range(1000..9999);
        
        TestUser {
            id: Uuid::new_v4(),
            username: format!("test_user_{}", user_id),
            email: format!("test_user_{}@example.com", user_id),
            github_id: Some(format!("github_{}", user_id)),
            roles: vec!["user".to_string()],
            created_at: Utc::now() - ChronoDuration::days(rng.gen_range(1..365)),
            last_login: if rng.gen_bool(0.8) {
                Some(Utc::now() - ChronoDuration::hours(rng.gen_range(1..72)))
            } else {
                None
            },
            is_active: rng.gen_bool(0.9),
        }
    }

    pub fn generate_test_data_set(&self, size: usize) -> Vec<TestUser> {
        (0..size).map(|_| self.generate_test_user()).collect()
    }

    fn generate_realistic_job_name(&self) -> String {
        let prefixes = ["build", "test", "deploy", "lint", "security", "performance"];
        let suffixes = ["backend", "frontend", "api", "database", "integration", "unit"];
        let mut rng = rand::thread_rng();
        
        format!("{}-{}-{}", 
            prefixes[rng.gen_range(0..prefixes.len())],
            suffixes[rng.gen_range(0..suffixes.len())],
            rng.gen_range(1..100)
        )
    }

    fn generate_realistic_pipeline_config(&self) -> serde_json::Value {
        serde_json::json!({
            "stages": [
                {
                    "name": "build",
                    "steps": [
                        {
                            "name": "compile",
                            "command": "cargo build --release",
                            "timeout": 300
                        }
                    ]
                },
                {
                    "name": "test",
                    "steps": [
                        {
                            "name": "unit-tests",
                            "command": "cargo test --lib",
                            "timeout": 600
                        },
                        {
                            "name": "integration-tests",
                            "command": "cargo test --test '*'",
                            "timeout": 900
                        }
                    ]
                }
            ],
            "environment": {
                "RUST_LOG": "info",
                "CARGO_TARGET_DIR": "target"
            }
        })
    }

    async fn initialize_database_seeder(&self) -> Result<()> {
        if let Some(db_url) = &self.config.database_url {
            let seeder = DatabaseSeeder {
                connection_string: db_url.clone(),
                collections: vec![
                    "test_users".to_string(),
                    "test_jobs".to_string(),
                    "test_pipelines".to_string(),
                    "test_executions".to_string(),
                ],
                is_connected: false,
            };

            let mut seeder_guard = self.database_seeder.lock().await;
            *seeder_guard = Some(seeder);
        }
        Ok(())
    }
}

#[async_trait]
impl TestDataFactoryInterface for TestDataFactory {
    async fn generate_test_users(&self, count: usize) -> Result<Vec<TestUser>> {
        let limit = self.config.data_size_limits.get("users").copied().unwrap_or(1000);
        let actual_count = count.min(limit);
        
        let mut users = Vec::with_capacity(actual_count);
        for _ in 0..actual_count {
            users.push(self.generate_test_user());
        }

        // Store generated data for cleanup
        let mut data_guard = self.generated_data.lock().await;
        let user_data: Vec<serde_json::Value> = users.iter()
            .map(|u| serde_json::to_value(u).unwrap())
            .collect();
        data_guard.insert("users".to_string(), user_data);

        tracing::info!("Generated {} test users", actual_count);
        Ok(users)
    }

    async fn generate_test_jobs(&self, count: usize) -> Result<Vec<TestJob>> {
        let limit = self.config.data_size_limits.get("jobs").copied().unwrap_or(500);
        let actual_count = count.min(limit);
        
        let mut jobs = Vec::with_capacity(actual_count);
        for i in 0..actual_count {
            let mut rng = rand::thread_rng();
            let job = TestJob {
                id: Uuid::new_v4(),
                name: self.generate_realistic_job_name(),
                status: match rng.gen_range(0..4) {
                    0 => JobStatus::Pending,
                    1 => JobStatus::Running,
                    2 => JobStatus::Completed,
                    _ => JobStatus::Failed,
                },
                created_at: Utc::now() - ChronoDuration::hours(rng.gen_range(1..168)),
                started_at: if rng.gen_bool(0.8) {
                    Some(Utc::now() - ChronoDuration::hours(rng.gen_range(1..72)))
                } else {
                    None
                },
                completed_at: if rng.gen_bool(0.6) {
                    Some(Utc::now() - ChronoDuration::minutes(rng.gen_range(1..120)))
                } else {
                    None
                },
                pipeline_id: Uuid::new_v4(),
                runner_id: Some(Uuid::new_v4()),
                exit_code: if rng.gen_bool(0.8) { Some(0) } else { Some(1) },
                logs: format!("Test job execution logs for job {}", rng.gen_range(1000..9999)),
            };
            jobs.push(job);
        }

        // Store generated data
        let mut data_guard = self.generated_data.lock().await;
        let job_data: Vec<serde_json::Value> = jobs.iter()
            .map(|j| serde_json::to_value(j).unwrap())
            .collect();
        data_guard.insert("jobs".to_string(), job_data);

        tracing::info!("Generated {} test jobs", actual_count);
        Ok(jobs)
    }

    async fn generate_test_pipelines(&self, count: usize) -> Result<Vec<TestPipeline>> {
        let limit = self.config.data_size_limits.get("pipelines").copied().unwrap_or(100);
        let actual_count = count.min(limit);
        
        let mut pipelines = Vec::with_capacity(actual_count);
        for i in 0..actual_count {
            let mut rng = rand::thread_rng();
            let pipeline = TestPipeline {
                id: Uuid::new_v4(),
                name: format!("test-pipeline-{}", i + 1),
                description: format!("Test pipeline for automated testing {}", i + 1),
                config: self.generate_realistic_pipeline_config(),
                created_at: Utc::now() - ChronoDuration::days(rng.gen_range(1..90)),
                updated_at: Utc::now() - ChronoDuration::hours(rng.gen_range(1..24)),
                owner_id: Uuid::new_v4(),
                is_active: rng.gen_bool(0.85),
                triggers: vec!["push".to_string(), "pull_request".to_string()],
            };
            pipelines.push(pipeline);
        }

        // Store generated data
        let mut data_guard = self.generated_data.lock().await;
        let pipeline_data: Vec<serde_json::Value> = pipelines.iter()
            .map(|p| serde_json::to_value(p).unwrap())
            .collect();
        data_guard.insert("pipelines".to_string(), pipeline_data);

        tracing::info!("Generated {} test pipelines", actual_count);
        Ok(pipelines)
    }

    async fn generate_test_executions(&self, count: usize) -> Result<Vec<TestExecution>> {
        let limit = self.config.data_size_limits.get("executions").copied().unwrap_or(200);
        let actual_count = count.min(limit);
        
        let mut executions = Vec::with_capacity(actual_count);
        for _ in 0..actual_count {
            let mut rng = rand::thread_rng();
            let execution = TestExecution {
                id: Uuid::new_v4(),
                pipeline_id: Uuid::new_v4(),
                status: match rng.gen_range(0..4) {
                    0 => ExecutionStatus::Running,
                    1 => ExecutionStatus::Completed,
                    2 => ExecutionStatus::Failed,
                    _ => ExecutionStatus::Cancelled,
                },
                started_at: Utc::now() - ChronoDuration::hours(rng.gen_range(1..48)),
                completed_at: if rng.gen_bool(0.7) {
                    Some(Utc::now() - ChronoDuration::minutes(rng.gen_range(5..180)))
                } else {
                    None
                },
                trigger: "push".to_string(),
                commit_sha: format!("abc123def456{:04x}", rng.gen_range(0..0xFFFF)),
                branch: "main".to_string(),
                user_id: Uuid::new_v4(),
                job_count: rng.gen_range(1..10),
                success_count: rng.gen_range(0..8),
                failure_count: rng.gen_range(0..3),
            };
            executions.push(execution);
        }

        // Store generated data
        let mut data_guard = self.generated_data.lock().await;
        let execution_data: Vec<serde_json::Value> = executions.iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();
        data_guard.insert("executions".to_string(), execution_data);

        tracing::info!("Generated {} test executions", actual_count);
        Ok(executions)
    }

    async fn seed_database(&self) -> Result<()> {
        if !self.config.seed_database {
            tracing::info!("Database seeding disabled");
            return Ok(());
        }

        tracing::info!("Seeding test database with generated data");

        // Initialize database seeder if not already done
        self.initialize_database_seeder().await?;

        // Generate test data
        let _users = self.generate_test_users(50).await?;
        let _jobs = self.generate_test_jobs(100).await?;
        let _pipelines = self.generate_test_pipelines(20).await?;
        let _executions = self.generate_test_executions(75).await?;

        // In a real implementation, this would insert data into the database
        // For now, we simulate the seeding process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        tracing::info!("Database seeding completed successfully");
        Ok(())
    }

    async fn cleanup_test_data(&self) -> Result<()> {
        if !self.config.cleanup_after_tests {
            tracing::info!("Test data cleanup disabled");
            return Ok(());
        }

        tracing::info!("Cleaning up test data");

        // Clear generated data from memory
        let mut data_guard = self.generated_data.lock().await;
        let total_records: usize = data_guard.values().map(|v| v.len()).sum();
        data_guard.clear();

        // In a real implementation, this would also clean up database records
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        tracing::info!("Cleaned up {} test data records", total_records);
        Ok(())
    }

    async fn create_test_scenario(&self, scenario_name: &str) -> Result<TestScenario> {
        tracing::info!("Creating test scenario: {}", scenario_name);

        let scenario = match scenario_name {
            "basic_pipeline_execution" => {
                let users = self.generate_test_users(5).await?;
                let pipelines = self.generate_test_pipelines(3).await?;
                let jobs = self.generate_test_jobs(10).await?;
                let executions = self.generate_test_executions(8).await?;

                TestScenario {
                    id: Uuid::new_v4(),
                    name: scenario_name.to_string(),
                    description: "Basic pipeline execution scenario with users, pipelines, jobs, and executions".to_string(),
                    users,
                    pipelines,
                    jobs,
                    executions,
                    created_at: Utc::now(),
                }
            }
            "high_load_scenario" => {
                let users = self.generate_test_users(100).await?;
                let pipelines = self.generate_test_pipelines(50).await?;
                let jobs = self.generate_test_jobs(500).await?;
                let executions = self.generate_test_executions(200).await?;

                TestScenario {
                    id: Uuid::new_v4(),
                    name: scenario_name.to_string(),
                    description: "High load scenario with many users, pipelines, jobs, and executions".to_string(),
                    users,
                    pipelines,
                    jobs,
                    executions,
                    created_at: Utc::now(),
                }
            }
            "failure_scenario" => {
                // Generate data with higher failure rates
                let users = self.generate_test_users(10).await?;
                let pipelines = self.generate_test_pipelines(5).await?;
                let mut jobs = self.generate_test_jobs(20).await?;
                let mut executions = self.generate_test_executions(15).await?;

                // Modify jobs and executions to have more failures
                for job in &mut jobs {
                    if rand::thread_rng().gen_bool(0.6) {
                        job.status = JobStatus::Failed;
                        job.exit_code = Some(1);
                    }
                }

                for execution in &mut executions {
                    if rand::thread_rng().gen_bool(0.5) {
                        execution.status = ExecutionStatus::Failed;
                        execution.failure_count = execution.job_count;
                        execution.success_count = 0;
                    }
                }

                TestScenario {
                    id: Uuid::new_v4(),
                    name: scenario_name.to_string(),
                    description: "Failure scenario with higher rates of job and execution failures".to_string(),
                    users,
                    pipelines,
                    jobs,
                    executions,
                    created_at: Utc::now(),
                }
            }
            _ => {
                return Err(anyhow!("Unknown test scenario: {}", scenario_name));
            }
        };

        tracing::info!("Created test scenario '{}' with {} users, {} pipelines, {} jobs, {} executions",
            scenario.name, scenario.users.len(), scenario.pipelines.len(), 
            scenario.jobs.len(), scenario.executions.len());

        Ok(scenario)
    }
}

// Data model definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub github_id: Option<String>,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestJob {
    pub id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub pipeline_id: Uuid,
    pub runner_id: Option<Uuid>,
    pub exit_code: Option<i32>,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPipeline {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_id: Uuid,
    pub is_active: bool,
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecution {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub status: ExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub trigger: String,
    pub commit_sha: String,
    pub branch: String,
    pub user_id: Uuid,
    pub job_count: u32,
    pub success_count: u32,
    pub failure_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub users: Vec<TestUser>,
    pub pipelines: Vec<TestPipeline>,
    pub jobs: Vec<TestJob>,
    pub executions: Vec<TestExecution>,
    pub created_at: DateTime<Utc>,
}

impl Default for TestDataFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_data_factory_creation() {
        let factory = TestDataFactory::new();
        assert!(factory.config.cleanup_after_tests);
        assert!(factory.config.use_realistic_data);
    }

    #[test]
    fn test_generate_test_user() {
        let factory = TestDataFactory::new();
        let user = factory.generate_test_user();
        
        assert!(!user.username.is_empty());
        assert!(user.email.contains("@example.com"));
        assert!(!user.roles.is_empty());
    }

    #[tokio::test]
    async fn test_generate_test_users() {
        let factory = TestDataFactory::new();
        let users = factory.generate_test_users(10).await.unwrap();
        
        assert_eq!(users.len(), 10);
        // Verify all users have unique IDs
        let mut ids = std::collections::HashSet::new();
        for user in &users {
            assert!(ids.insert(user.id));
        }
    }

    #[tokio::test]
    async fn test_generate_test_jobs() {
        let factory = TestDataFactory::new();
        let jobs = factory.generate_test_jobs(5).await.unwrap();
        
        assert_eq!(jobs.len(), 5);
        assert!(jobs.iter().all(|j| !j.name.is_empty()));
    }

    #[tokio::test]
    async fn test_generate_test_pipelines() {
        let factory = TestDataFactory::new();
        let pipelines = factory.generate_test_pipelines(3).await.unwrap();
        
        assert_eq!(pipelines.len(), 3);
        assert!(pipelines.iter().all(|p| !p.name.is_empty()));
        assert!(pipelines.iter().all(|p| !p.triggers.is_empty()));
    }

    #[tokio::test]
    async fn test_generate_test_executions() {
        let factory = TestDataFactory::new();
        let executions = factory.generate_test_executions(7).await.unwrap();
        
        assert_eq!(executions.len(), 7);
        assert!(executions.iter().all(|e| !e.commit_sha.is_empty()));
    }

    #[tokio::test]
    async fn test_create_test_scenario() {
        let factory = TestDataFactory::new();
        let scenario = factory.create_test_scenario("basic_pipeline_execution").await.unwrap();
        
        assert_eq!(scenario.name, "basic_pipeline_execution");
        assert!(!scenario.users.is_empty());
        assert!(!scenario.pipelines.is_empty());
        assert!(!scenario.jobs.is_empty());
        assert!(!scenario.executions.is_empty());
    }

    #[tokio::test]
    async fn test_create_unknown_scenario() {
        let factory = TestDataFactory::new();
        let result = factory.create_test_scenario("unknown_scenario").await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown test scenario"));
    }

    #[tokio::test]
    async fn test_seed_and_cleanup_database() {
        let factory = TestDataFactory::new();
        
        let seed_result = factory.seed_database().await;
        assert!(seed_result.is_ok());
        
        let cleanup_result = factory.cleanup_test_data().await;
        assert!(cleanup_result.is_ok());
    }

    #[tokio::test]
    async fn test_data_size_limits() {
        let mut config = TestDataConfig::default();
        config.data_size_limits.insert("users".to_string(), 5);
        
        let factory = TestDataFactory::with_config(config);
        let users = factory.generate_test_users(100).await.unwrap(); // Request 100, should get 5
        
        assert_eq!(users.len(), 5);
    }

    #[test]
    fn test_realistic_job_name_generation() {
        let factory = TestDataFactory::new();
        let job_name = factory.generate_realistic_job_name();
        
        assert!(job_name.contains("-"));
        assert!(!job_name.is_empty());
    }
}