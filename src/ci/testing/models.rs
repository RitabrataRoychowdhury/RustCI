//! Data models for test execution and results tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Test environment types supported by the testing infrastructure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestEnvironment {
    /// Docker-in-Docker environment
    DockerInDocker,
    /// K3D Kubernetes cluster environment
    K3D,
    /// Local native environment
    Local,
}

impl std::fmt::Display for TestEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestEnvironment::DockerInDocker => write!(f, "docker-in-docker"),
            TestEnvironment::K3D => write!(f, "k3d"),
            TestEnvironment::Local => write!(f, "local"),
        }
    }
}

/// Pipeline types supported for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineType {
    Minimal,
    Simple,
    Standard,
    Advanced,
}

impl std::fmt::Display for PipelineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineType::Minimal => write!(f, "minimal"),
            PipelineType::Simple => write!(f, "simple"),
            PipelineType::Standard => write!(f, "standard"),
            PipelineType::Advanced => write!(f, "advanced"),
        }
    }
}

/// Status of a test execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    /// Test is pending execution
    Pending,
    /// Test is currently running
    Running,
    /// Test completed successfully
    Success,
    /// Test failed
    Failed,
    /// Test was cancelled
    Cancelled,
    /// Test timed out
    TimedOut,
}

impl std::fmt::Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestStatus::Pending => write!(f, "pending"),
            TestStatus::Running => write!(f, "running"),
            TestStatus::Success => write!(f, "success"),
            TestStatus::Failed => write!(f, "failed"),
            TestStatus::Cancelled => write!(f, "cancelled"),
            TestStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// Individual test execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecution {
    /// Unique identifier for this test execution
    pub id: Uuid,
    /// Test name for identification
    pub name: String,
    /// Environment being tested
    pub environment: TestEnvironment,
    /// Pipeline type being tested
    pub pipeline_type: PipelineType,
    /// Current status of the test
    pub status: TestStatus,
    /// When the test started
    pub start_time: DateTime<Utc>,
    /// When the test ended (if completed)
    pub end_time: Option<DateTime<Utc>>,
    /// Test execution logs
    pub logs: Vec<String>,
    /// Validation results from HTTP checks
    pub validation_results: Vec<ValidationResult>,
    /// Pipeline ID in RustCI (if created)
    pub pipeline_id: Option<String>,
    /// Execution ID in RustCI (if triggered)
    pub execution_id: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl TestExecution {
    /// Create a new test execution
    pub fn new(name: String, environment: TestEnvironment, pipeline_type: PipelineType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            environment,
            pipeline_type,
            status: TestStatus::Pending,
            start_time: Utc::now(),
            end_time: None,
            logs: Vec::new(),
            validation_results: Vec::new(),
            pipeline_id: None,
            execution_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a log entry
    pub fn add_log<S: Into<String>>(&mut self, message: S) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        self.logs
            .push(format!("[{}] {}", timestamp, message.into()));
    }

    /// Mark test as completed with status
    pub fn complete(&mut self, status: TestStatus) {
        self.status = status;
        self.end_time = Some(Utc::now());
    }

    /// Get test duration if completed
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.end_time.map(|end| end - self.start_time)
    }

    /// Check if test is completed
    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            TestStatus::Success | TestStatus::Failed | TestStatus::Cancelled | TestStatus::TimedOut
        )
    }
}

/// Result of HTTP validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Endpoint that was tested
    pub endpoint: String,
    /// HTTP status code received
    pub status_code: Option<u16>,
    /// Response body content
    pub response_body: Option<String>,
    /// Response time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Whether the validation was successful
    pub success: bool,
    /// Error message if validation failed
    pub error_message: Option<String>,
    /// Timestamp of the validation attempt
    pub timestamp: DateTime<Utc>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success(
        endpoint: String,
        status_code: u16,
        response_body: String,
        response_time_ms: u64,
    ) -> Self {
        Self {
            endpoint,
            status_code: Some(status_code),
            response_body: Some(response_body),
            response_time_ms: Some(response_time_ms),
            success: true,
            error_message: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a failed validation result
    pub fn failure<S: Into<String>>(endpoint: String, error_message: S) -> Self {
        Self {
            endpoint,
            status_code: None,
            response_body: None,
            response_time_ms: None,
            success: false,
            error_message: Some(error_message.into()),
            timestamp: Utc::now(),
        }
    }
}

/// Test suite configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentTestSuite {
    /// Name of the test suite
    pub name: String,
    /// Description of what this test suite validates
    pub description: String,
    /// Target repository URL for testing
    pub target_repository: String,
    /// List of test cases to execute
    pub test_cases: Vec<TestCase>,
    /// Global timeout for the entire suite
    pub global_timeout_seconds: u64,
    /// Cleanup policy after tests complete
    pub cleanup_policy: CleanupPolicy,
}

/// Individual test case configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Name of the test case
    pub name: String,
    /// Environment to test in
    pub environment: TestEnvironment,
    /// Pipeline type to test
    pub pipeline_type: PipelineType,
    /// Expected port for the deployed application
    pub expected_port: u16,
    /// Health check endpoint path
    pub health_endpoint: String,
    /// Additional validation commands to run
    pub validation_commands: Vec<String>,
    /// Timeout for this specific test case
    pub timeout_seconds: u64,
}

/// Cleanup policy for test resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupPolicy {
    /// Always cleanup resources
    Always,
    /// Only cleanup on success
    OnSuccess,
    /// Only cleanup on failure
    OnFailure,
    /// Never cleanup (for debugging)
    Never,
}

/// API response models for RustCI integration

/// Response from pipeline upload API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResponse {
    /// Pipeline ID assigned by RustCI
    pub id: String,
    /// Pipeline name
    pub name: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Pipeline status
    pub status: String,
}

/// Response from pipeline trigger API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    /// Execution ID assigned by RustCI
    pub execution_id: String,
    /// Pipeline ID being executed
    pub pipeline_id: String,
    /// Execution status
    pub status: String,
    /// Trigger timestamp
    pub triggered_at: DateTime<Utc>,
}

/// Execution status from monitoring API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatus {
    /// Execution ID
    pub id: String,
    /// Pipeline ID
    pub pipeline_id: String,
    /// Current status
    pub status: String,
    /// Start time
    pub started_at: Option<DateTime<Utc>>,
    /// End time (if completed)
    pub completed_at: Option<DateTime<Utc>>,
    /// Execution logs
    pub logs: Option<Vec<String>>,
    /// Error message (if failed)
    pub error_message: Option<String>,
}

/// Health check response from RustCI server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Server status
    pub status: String,
    /// Server version
    pub version: Option<String>,
    /// Uptime in seconds
    pub uptime: Option<u64>,
}

/// Test execution summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionSummary {
    /// Total number of tests executed
    pub total_tests: usize,
    /// Number of successful tests
    pub successful_tests: usize,
    /// Number of failed tests
    pub failed_tests: usize,
    /// Number of cancelled tests
    pub cancelled_tests: usize,
    /// Number of timed out tests
    pub timed_out_tests: usize,
    /// Total execution time
    pub total_duration_seconds: u64,
    /// Individual test results
    pub test_results: Vec<TestExecution>,
}

impl TestExecutionSummary {
    /// Create a new summary from test executions
    pub fn from_executions(executions: Vec<TestExecution>) -> Self {
        let total_tests = executions.len();
        let successful_tests = executions
            .iter()
            .filter(|e| e.status == TestStatus::Success)
            .count();
        let failed_tests = executions
            .iter()
            .filter(|e| e.status == TestStatus::Failed)
            .count();
        let cancelled_tests = executions
            .iter()
            .filter(|e| e.status == TestStatus::Cancelled)
            .count();
        let timed_out_tests = executions
            .iter()
            .filter(|e| e.status == TestStatus::TimedOut)
            .count();

        let total_duration_seconds = executions
            .iter()
            .filter_map(|e| e.duration())
            .map(|d| d.num_seconds() as u64)
            .sum();

        Self {
            total_tests,
            successful_tests,
            failed_tests,
            cancelled_tests,
            timed_out_tests,
            total_duration_seconds,
            test_results: executions,
        }
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            0.0
        } else {
            (self.successful_tests as f64 / self.total_tests as f64) * 100.0
        }
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.successful_tests == self.total_tests
    }
}
