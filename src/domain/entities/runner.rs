//! Runner domain entities and types
//!
//! This module defines the core runner entities, types, and related data structures
//! for the RustCI platform's runner architecture.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::Result;

/// Unique identifier for a runner instance
pub type RunnerId = Uuid;

/// Unique identifier for a job
pub type JobId = Uuid;

/// Unique identifier for a pipeline
pub type PipelineId = Uuid;

/// Core Runner trait defining async execution capabilities
#[async_trait]
pub trait Runner: Send + Sync {
    /// Execute a job asynchronously
    async fn execute(&self, job: Job) -> Result<JobResult>;

    /// Perform health check on the runner
    async fn health_check(&self) -> Result<HealthStatus>;

    /// Get current capacity information
    async fn get_capacity(&self) -> Result<RunnerCapacity>;

    /// Shutdown the runner gracefully
    async fn shutdown(&self) -> Result<()>;

    /// Get runner metadata
    fn get_metadata(&self) -> RunnerMetadata;

    /// Check if runner can handle a specific job
    async fn can_handle_job(&self, job: &Job) -> Result<bool>;
}

/// Runner types supported by the platform
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum RunnerType {
    /// Local runner executing on the same machine
    Local {
        max_concurrent_jobs: u32,
        working_directory: String,
    },
    /// Docker-based runner using containers
    Docker {
        max_concurrent_jobs: u32,
        docker_config: DockerConfig,
    },
    /// Kubernetes-based runner using pods
    Kubernetes {
        namespace: String,
        resource_limits: ResourceLimits,
        max_concurrent_jobs: u32,
    },
}

/// Docker configuration for Docker runners
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockerConfig {
    /// Docker daemon endpoint
    pub endpoint: String,
    /// Default image to use for jobs
    pub default_image: String,
    /// Network configuration
    pub network: Option<String>,
    /// Volume mounts
    pub volumes: Vec<VolumeMount>,
    /// Environment variables
    pub environment: HashMap<String, String>,
}

/// Volume mount configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

/// Resource limits for Kubernetes runners
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    /// CPU limit (in millicores)
    pub cpu_limit: u32,
    /// Memory limit (in MB)
    pub memory_limit: u32,
    /// Storage limit (in MB)
    pub storage_limit: Option<u32>,
}

// Runner trait is defined above - keeping it as a trait for actual implementation

/// Runner entity representing a registered runner
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunnerEntity {
    /// Unique identifier
    pub id: RunnerId,
    /// Human-readable name
    pub name: String,
    /// Runner type and configuration
    pub runner_type: RunnerType,
    /// Current status
    pub status: RunnerStatus,
    /// Tags for job matching
    pub tags: Vec<String>,
    /// Node ID if part of a cluster
    pub node_id: Option<Uuid>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
    /// Runner metadata
    pub metadata: HashMap<String, String>,
}

/// Runner status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum RunnerStatus {
    /// Runner is active and ready to accept jobs
    Active,
    /// Runner is busy executing jobs
    Busy,
    /// Runner is idle but available
    Idle,
    /// Runner is offline or unreachable
    Offline,
    /// Runner is in maintenance mode
    Maintenance,
    /// Runner has failed and needs attention
    Failed,
}

/// Runner capacity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerCapacity {
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Currently running jobs
    pub current_jobs: u32,
    /// Available job slots
    pub available_slots: u32,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f64,
    /// Memory usage percentage (0-100)
    pub memory_usage: f64,
    /// Disk usage percentage (0-100)
    pub disk_usage: f64,
}

/// Runner metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerMetadata {
    /// Runner ID
    pub id: RunnerId,
    /// Runner name
    pub name: String,
    /// Runner type
    pub runner_type: RunnerType,
    /// Version information
    pub version: String,
    /// Supported job types
    pub supported_job_types: Vec<String>,
}

/// Health status for runners
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    /// Runner is healthy and operational
    Healthy,
    /// Runner is degraded but functional
    Degraded { reason: String },
    /// Runner is unhealthy and not operational
    Unhealthy { reason: String },
}

/// Job entity representing work to be executed
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    /// Unique job identifier
    pub id: JobId,
    /// Associated pipeline ID
    pub pipeline_id: PipelineId,
    /// Job name
    pub name: String,
    /// Job steps to execute
    pub steps: Vec<JobStep>,
    /// Job requirements and constraints
    pub requirements: JobRequirements,
    /// Job priority
    pub priority: JobPriority,
    /// Execution timeout
    pub timeout: Duration,
    /// Retry policy
    pub retry_policy: RetryPolicy,
    /// Job metadata
    pub metadata: HashMap<String, String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Scheduled execution time
    pub scheduled_at: Option<DateTime<Utc>>,
}

/// Individual job step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    /// Step name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Working directory
    pub working_directory: Option<String>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Step timeout
    pub timeout: Option<Duration>,
    /// Whether to continue on failure
    pub continue_on_error: bool,
}

/// Job requirements and constraints
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobRequirements {
    /// Required runner tags
    pub required_tags: Vec<String>,
    /// Excluded runner tags
    pub excluded_tags: Vec<String>,
    /// Minimum resource requirements
    pub min_resources: Option<ResourceRequirements>,
    /// Required runner type
    pub runner_type: Option<RunnerType>,
    /// Node affinity rules
    pub node_affinity: Vec<String>,
}

/// Resource requirements for jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Minimum CPU (in millicores)
    pub min_cpu: u32,
    /// Minimum memory (in MB)
    pub min_memory: u32,
    /// Minimum disk space (in MB)
    pub min_disk: Option<u32>,
}

/// Job priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub enum JobPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl Default for JobPriority {
    fn default() -> Self {
        JobPriority::Normal
    }
}

/// Retry policy for failed jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Delay between retries
    pub retry_delay: Duration,
    /// Backoff strategy
    pub backoff_strategy: BackoffStrategy,
}

/// Backoff strategies for retries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed,
    /// Exponential backoff
    Exponential { multiplier: f64 },
    /// Linear backoff
    Linear { increment: Duration },
}

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// Job ID
    pub job_id: JobId,
    /// Execution status
    pub status: JobStatus,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution start time
    pub started_at: DateTime<Utc>,
    /// Execution end time
    pub finished_at: Option<DateTime<Utc>>,
    /// Execution duration
    pub duration: Option<Duration>,
    /// Step results
    pub step_results: Vec<StepResult>,
    /// Error message if failed
    pub error_message: Option<String>,
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum JobStatus {
    /// Job is queued for execution
    Queued,
    /// Job is currently running
    Running,
    /// Job completed successfully
    Success,
    /// Job failed
    Failed,
    /// Job was cancelled
    Cancelled,
    /// Job timed out
    TimedOut,
}

/// Individual step execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step name
    pub name: String,
    /// Step status
    pub status: JobStatus,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution duration
    pub duration: Duration,
    /// Error message if failed
    pub error_message: Option<String>,
}

impl Default for RunnerCapacity {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 1,
            current_jobs: 0,
            available_slots: 1,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
        }
    }
}



impl RunnerEntity {
    /// Create a new runner entity
    pub fn new(name: String, runner_type: RunnerType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            runner_type,
            status: RunnerStatus::Idle,
            tags: Vec::new(),
            node_id: None,
            created_at: now,
            last_heartbeat: now,
            metadata: HashMap::new(),
        }
    }

    /// Update the last heartbeat timestamp
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    /// Check if the runner is available for new jobs
    pub fn is_available(&self) -> bool {
        matches!(self.status, RunnerStatus::Active | RunnerStatus::Idle)
    }

    /// Get the maximum concurrent jobs for this runner
    pub fn max_concurrent_jobs(&self) -> u32 {
        match &self.runner_type {
            RunnerType::Local {
                max_concurrent_jobs,
                ..
            } => *max_concurrent_jobs,
            RunnerType::Docker {
                max_concurrent_jobs,
                ..
            } => *max_concurrent_jobs,
            RunnerType::Kubernetes {
                max_concurrent_jobs,
                ..
            } => *max_concurrent_jobs,
        }
    }
}

impl Job {
    /// Create a new job
    pub fn new(name: String, pipeline_id: PipelineId, steps: Vec<JobStep>) -> Self {
        Self {
            id: Uuid::new_v4(),
            pipeline_id,
            name,
            steps,
            requirements: JobRequirements::default(),
            priority: JobPriority::Normal,
            timeout: Duration::from_secs(3600), // 1 hour default
            retry_policy: RetryPolicy::default(),
            metadata: HashMap::new(),
            created_at: Utc::now(),
            scheduled_at: None,
        }
    }

    /// Check if this job matches the given runner
    pub fn matches_runner(&self, runner: &RunnerEntity) -> bool {
        // Check required tags
        for required_tag in &self.requirements.required_tags {
            if !runner.tags.contains(required_tag) {
                return false;
            }
        }

        // Check excluded tags
        for excluded_tag in &self.requirements.excluded_tags {
            if runner.tags.contains(excluded_tag) {
                return false;
            }
        }

        // Check runner type if specified
        if let Some(required_type) = &self.requirements.runner_type {
            if std::mem::discriminant(&runner.runner_type) != std::mem::discriminant(required_type)
            {
                return false;
            }
        }

        true
    }
}

impl JobResult {
    /// Create a new job result
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            status: JobStatus::Queued,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            started_at: Utc::now(),
            finished_at: None,
            duration: None,
            step_results: Vec::new(),
            error_message: None,
        }
    }

    /// Mark the job as started
    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Utc::now();
    }

    /// Mark the job as completed with the given status
    pub fn complete(&mut self, status: JobStatus, exit_code: Option<i32>) {
        self.status = status;
        self.exit_code = exit_code;
        self.finished_at = Some(Utc::now());
        if let Some(finished) = self.finished_at {
            self.duration = Some(Duration::from_millis(
                (finished - self.started_at).num_milliseconds() as u64,
            ));
        }
    }

    /// Check if the job was successful
    pub fn is_success(&self) -> bool {
        self.status == JobStatus::Success
    }

    /// Check if the job failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, JobStatus::Failed | JobStatus::TimedOut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_entity_creation() {
        let runner = RunnerEntity::new(
            "test-runner".to_string(),
            RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            },
        );

        assert_eq!(runner.name, "test-runner");
        assert_eq!(runner.status, RunnerStatus::Idle);
        assert!(runner.is_available());
        assert_eq!(runner.max_concurrent_jobs(), 4);
    }

    #[test]
    fn test_job_creation() {
        let pipeline_id = Uuid::new_v4();
        let steps = vec![JobStep {
            name: "build".to_string(),
            command: "cargo".to_string(),
            args: vec!["build".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: None,
            continue_on_error: false,
        }];

        let job = Job::new("test-job".to_string(), pipeline_id, steps);

        assert_eq!(job.name, "test-job");
        assert_eq!(job.pipeline_id, pipeline_id);
        assert_eq!(job.priority, JobPriority::Normal);
        assert_eq!(job.steps.len(), 1);
    }

    #[test]
    fn test_job_runner_matching() {
        let mut runner = RunnerEntity::new(
            "test-runner".to_string(),
            RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            },
        );
        runner.tags = vec!["rust".to_string(), "linux".to_string()];

        let mut job = Job::new("test-job".to_string(), Uuid::new_v4(), vec![]);
        job.requirements.required_tags = vec!["rust".to_string()];

        assert!(job.matches_runner(&runner));

        job.requirements.required_tags = vec!["python".to_string()];
        assert!(!job.matches_runner(&runner));

        job.requirements.required_tags = vec!["rust".to_string()];
        job.requirements.excluded_tags = vec!["linux".to_string()];
        assert!(!job.matches_runner(&runner));
    }

    #[test]
    fn test_job_result_lifecycle() {
        let job_id = Uuid::new_v4();
        let mut result = JobResult::new(job_id);

        assert_eq!(result.status, JobStatus::Queued);
        assert!(!result.is_success());
        assert!(!result.is_failed());

        result.start();
        assert_eq!(result.status, JobStatus::Running);

        result.complete(JobStatus::Success, Some(0));
        assert_eq!(result.status, JobStatus::Success);
        assert!(result.is_success());
        assert!(!result.is_failed());
        assert!(result.finished_at.is_some());
        assert!(result.duration.is_some());
    }

    #[test]
    fn test_runner_capacity_default() {
        let capacity = RunnerCapacity::default();
        assert_eq!(capacity.max_concurrent_jobs, 1);
        assert_eq!(capacity.current_jobs, 0);
        assert_eq!(capacity.available_slots, 1);
    }

    #[test]
    fn test_health_status_variants() {
        let healthy = HealthStatus::Healthy;
        let degraded = HealthStatus::Degraded {
            reason: "High load".to_string(),
        };
        let unhealthy = HealthStatus::Unhealthy {
            reason: "Connection failed".to_string(),
        };

        assert_eq!(healthy, HealthStatus::Healthy);
        assert_ne!(healthy, degraded);
        assert_ne!(degraded, unhealthy);
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Critical > JobPriority::High);
        assert!(JobPriority::High > JobPriority::Normal);
        assert!(JobPriority::Normal > JobPriority::Low);
    }
}
