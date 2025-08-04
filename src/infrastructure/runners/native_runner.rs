//! Native Process Runner Implementation
//!
//! This module provides a self-contained native runner that executes jobs without
//! Docker or Kubernetes dependencies. It uses OS-level process isolation mechanisms
//! and integrates with the existing runner pool and job scheduling infrastructure.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{timeout, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::infrastructure::event_loop::{Event, EventDemultiplexer, EventHandler, EventPayload, EventType};
use crate::core::jobs::job_queue::{InMemoryJobQueue, JobQueue};
use crate::domain::entities::{
    HealthStatus, Job, JobId, JobResult, JobStatus, Runner, RunnerCapacity, RunnerId,
    RunnerMetadata, RunnerType, StepResult,
};
use crate::error::{AppError, Result};

/// Process isolation levels for native runner
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IsolationLevel {
    /// No isolation - run processes directly
    None,
    /// Basic process isolation using process groups
    ProcessGroup,
    /// Namespace isolation (Linux only)
    #[cfg(target_os = "linux")]
    Namespace {
        /// Use PID namespace
        pid: bool,
        /// Use network namespace
        network: bool,
        /// Use mount namespace
        mount: bool,
        /// Use user namespace
        user: bool,
    },
    /// Container-like isolation using cgroups (Linux only)
    #[cfg(target_os = "linux")]
    Cgroup {
        /// CPU limit in millicores
        cpu_limit: Option<u32>,
        /// Memory limit in MB
        memory_limit: Option<u32>,
        /// Enable process isolation
        process_isolation: bool,
    },
}

impl Default for IsolationLevel {
    fn default() -> Self {
        #[cfg(target_os = "linux")]
        {
            IsolationLevel::ProcessGroup
        }
        #[cfg(not(target_os = "linux"))]
        {
            IsolationLevel::ProcessGroup
        }
    }
}

/// Resource limits for native processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU usage (0.0 to 1.0)
    pub max_cpu: Option<f64>,
    /// Maximum memory usage in MB
    pub max_memory: Option<u64>,
    /// Maximum execution time
    pub max_execution_time: Option<Duration>,
    /// Maximum number of processes
    pub max_processes: Option<u32>,
    /// Maximum file descriptors
    pub max_file_descriptors: Option<u32>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu: Some(1.0), // 100% of one CPU core
            max_memory: Some(1024), // 1GB
            max_execution_time: Some(Duration::from_secs(3600)), // 1 hour
            max_processes: Some(100),
            max_file_descriptors: Some(1024),
        }
    }
}

/// Native runner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeRunnerConfig {
    /// Runner name
    pub name: String,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Working directory for job execution
    pub working_directory: PathBuf,
    /// Environment variables to set for all jobs
    pub environment: HashMap<String, String>,
    /// Default job timeout in seconds
    pub default_timeout_seconds: u64,
    /// Process isolation level
    pub isolation_level: IsolationLevel,
    /// Resource limits
    pub resource_limits: ResourceLimits,
    /// Enable job output streaming
    pub enable_output_streaming: bool,
    /// Maximum output buffer size
    pub max_output_buffer_size: usize,
    /// Shell to use for command execution
    pub shell: String,
    /// Enable enhanced lifecycle management
    pub enhanced_lifecycle: bool,
    /// Control plane coordination settings
    pub control_plane_coordination: ControlPlaneCoordinationConfig,
}

/// Control plane coordination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneCoordinationConfig {
    /// Enable control plane integration
    pub enabled: bool,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Enable automatic registration
    pub auto_registration: bool,
    /// Runner tags for job matching
    pub tags: Vec<String>,
    /// Node affinity labels
    pub node_affinity: HashMap<String, String>,
}

impl Default for NativeRunnerConfig {
    fn default() -> Self {
        Self {
            name: "native-runner".to_string(),
            max_concurrent_jobs: 4,
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
            environment: HashMap::new(),
            default_timeout_seconds: 3600, // 1 hour
            isolation_level: IsolationLevel::default(),
            resource_limits: ResourceLimits::default(),
            enable_output_streaming: true,
            max_output_buffer_size: 1024 * 1024, // 1MB
            shell: if cfg!(windows) {
                "cmd".to_string()
            } else {
                "bash".to_string()
            },
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig::default(),
        }
    }
}

impl Default for ControlPlaneCoordinationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            heartbeat_interval: 30,
            health_check_interval: 60,
            auto_registration: true,
            tags: vec!["native".to_string()],
            node_affinity: HashMap::new(),
        }
    }
}

/// Information about a currently running job
#[derive(Debug, Clone)]
struct RunningJob {
    job: Job,
    started_at: DateTime<Utc>,
    process_id: Option<u32>,
    current_step: usize,
    output_buffer: String,
    error_buffer: String,
    isolation_context: Option<IsolationContext>,
}

/// Process isolation context
#[derive(Debug, Clone)]
struct IsolationContext {
    /// Process group ID (if using process group isolation)
    process_group_id: Option<u32>,
    /// Namespace information (Linux only)
    #[cfg(target_os = "linux")]
    namespace_info: Option<NamespaceInfo>,
    /// Cgroup information (Linux only)
    #[cfg(target_os = "linux")]
    cgroup_info: Option<CgroupInfo>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
struct NamespaceInfo {
    pid_namespace: Option<String>,
    network_namespace: Option<String>,
    mount_namespace: Option<String>,
    user_namespace: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
struct CgroupInfo {
    cgroup_path: String,
    cpu_limit: Option<u32>,
    memory_limit: Option<u32>,
}

/// Native runner statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NativeRunnerStats {
    pub total_jobs_executed: u64,
    pub successful_jobs: u64,
    pub failed_jobs: u64,
    pub avg_execution_time: Duration,
    pub current_load: f64,
    pub uptime: Duration,
    pub last_job_completed: Option<DateTime<Utc>>,
    pub isolation_failures: u64,
    pub resource_limit_violations: u64,
    pub control_plane_heartbeats: u64,
}

/// Native Process Runner implementation
pub struct NativeProcessRunner {
    /// Runner ID
    id: RunnerId,
    /// Runner configuration
    config: NativeRunnerConfig,
    /// Currently running jobs
    running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    /// Job queue
    job_queue: Arc<dyn JobQueue>,
    /// Event demultiplexer for event handling
    event_demux: Arc<EventDemultiplexer>,
    /// Runner statistics
    stats: Arc<RwLock<NativeRunnerStats>>,
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
    /// Runner start time
    start_time: DateTime<Utc>,
}

impl NativeProcessRunner {
    /// Create a new native process runner
    pub async fn new(
        config: NativeRunnerConfig,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<Self> {
        let id = Uuid::new_v4();
        let job_queue = Arc::new(InMemoryJobQueue::with_default_config());
        let running_jobs = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(NativeRunnerStats::default()));

        // Register job completion handler
        let completion_handler = Arc::new(NativeJobCompletionHandler {
            runner_id: id,
            running_jobs: running_jobs.clone(),
            stats: stats.clone(),
        });

        event_demux.register_handler(completion_handler).await?;

        // Ensure working directory exists
        if !config.working_directory.exists() {
            tokio::fs::create_dir_all(&config.working_directory)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!(
                        "Failed to create working directory: {}",
                        e
                    ))
                })?;
        }

        // Initialize isolation environment if needed
        Self::initialize_isolation_environment(&config).await?;

        info!(
            "Created native process runner: {} with working directory: {:?}",
            config.name, config.working_directory
        );

        Ok(Self {
            id,
            config,
            running_jobs,
            job_queue,
            event_demux,
            stats,
            shutdown: Arc::new(Mutex::new(false)),
            start_time: Utc::now(),
        })
    }

    /// Initialize isolation environment
    async fn initialize_isolation_environment(config: &NativeRunnerConfig) -> Result<()> {
        match &config.isolation_level {
            IsolationLevel::None | IsolationLevel::ProcessGroup => {
                // No special initialization needed
                Ok(())
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Namespace { .. } => {
                // Initialize namespace support
                Self::initialize_namespace_support().await
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Cgroup { .. } => {
                // Initialize cgroup support
                Self::initialize_cgroup_support().await
            }
        }
    }

    #[cfg(target_os = "linux")]
    async fn initialize_namespace_support() -> Result<()> {
        // Check if namespace support is available
        if !std::path::Path::new("/proc/self/ns").exists() {
            return Err(AppError::InternalServerError(
                "Namespace support not available on this system".to_string(),
            ));
        }

        info!("Namespace isolation support initialized");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn initialize_cgroup_support() -> Result<()> {
        // Check if cgroup support is available
        if !std::path::Path::new("/sys/fs/cgroup").exists() {
            return Err(AppError::InternalServerError(
                "Cgroup support not available on this system".to_string(),
            ));
        }

        info!("Cgroup isolation support initialized");
        Ok(())
    }

    /// Start the runner event loop with enhanced lifecycle management
    pub async fn start(&self) -> Result<()> {
        info!("Starting native process runner: {}", self.config.name);

        // Start control plane coordination if enabled
        if self.config.control_plane_coordination.enabled {
            self.start_control_plane_coordination().await?;
        }

        // Start enhanced lifecycle management tasks
        if self.config.enhanced_lifecycle {
            self.start_enhanced_lifecycle_management().await?;
        }

        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            // Check for shutdown signal
            {
                let shutdown = self.shutdown.lock().await;
                if *shutdown {
                    info!("Native process runner shutting down");
                    break;
                }
            }

            // Process queued jobs if we have capacity
            if self.has_capacity().await? {
                if let Some(job) = self.job_queue.dequeue().await? {
                    self.spawn_job_execution(job).await?;
                }
            }

            // Clean up completed jobs
            self.cleanup_completed_jobs().await?;

            // Update statistics
            self.update_stats().await?;
        }

        Ok(())
    }

    /// Start control plane coordination
    async fn start_control_plane_coordination(&self) -> Result<()> {
        if !self.config.control_plane_coordination.enabled {
            return Ok(());
        }

        // Start heartbeat task
        let heartbeat_interval = Duration::from_secs(
            self.config.control_plane_coordination.heartbeat_interval
        );
        self.start_heartbeat(heartbeat_interval).await?;

        // Start health check task
        let health_check_interval = Duration::from_secs(
            self.config.control_plane_coordination.health_check_interval
        );
        self.start_health_monitoring(health_check_interval).await?;

        // Auto-register with control plane if enabled
        if self.config.control_plane_coordination.auto_registration {
            self.register_with_control_plane().await?;
        }

        info!("Control plane coordination started for runner: {}", self.id);
        Ok(())
    }

    /// Start enhanced lifecycle management
    async fn start_enhanced_lifecycle_management(&self) -> Result<()> {
        info!("Starting enhanced lifecycle management for runner: {}", self.id);

        // Start resource monitoring task
        self.start_resource_monitoring().await?;

        // Start job timeout monitoring
        self.start_job_timeout_monitoring().await?;

        // Start isolation cleanup task
        self.start_isolation_cleanup().await?;

        // Start capacity management task
        self.start_capacity_management().await?;

        info!("Enhanced lifecycle management started for runner: {}", self.id);
        Ok(())
    }

    /// Register with control plane
    async fn register_with_control_plane(&self) -> Result<()> {
        let registration_event = Event::new(
            EventType::Custom("RunnerRegistration".to_string()),
            EventPayload::Custom {
                data: serde_json::json!({
                    "runner_id": self.id,
                    "runner_name": self.config.name,
                    "runner_type": "native",
                    "capabilities": {
                        "max_concurrent_jobs": self.config.max_concurrent_jobs,
                        "isolation_level": self.config.isolation_level,
                        "resource_limits": self.config.resource_limits,
                        "tags": self.config.control_plane_coordination.tags,
                        "node_affinity": self.config.control_plane_coordination.node_affinity
                    },
                    "working_directory": self.config.working_directory,
                    "enhanced_lifecycle": self.config.enhanced_lifecycle
                }),
            },
        );

        self.event_demux.submit_event(registration_event).await?;
        info!("Registered runner {} with control plane", self.id);
        Ok(())
    }

    /// Start resource monitoring task
    async fn start_resource_monitoring(&self) -> Result<()> {
        let runner_id = self.id;
        let event_demux = self.event_demux.clone();
        let running_jobs = self.running_jobs.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Monitor system resources
                let resource_usage = Self::collect_resource_usage(&running_jobs, &config).await;

                // Send resource usage event
                let resource_event = Event::new(
                    EventType::Custom("ResourceUsage".to_string()),
                    EventPayload::Custom {
                        data: serde_json::json!({
                            "runner_id": runner_id,
                            "resource_usage": resource_usage,
                            "timestamp": Utc::now()
                        }),
                    },
                );

                if let Err(e) = event_demux.submit_event(resource_event).await {
                    error!("Failed to submit resource usage event: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start job timeout monitoring
    async fn start_job_timeout_monitoring(&self) -> Result<()> {
        let runner_id = self.id;
        let running_jobs = self.running_jobs.clone();
        let event_demux = self.event_demux.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let now = Utc::now();
                let mut timed_out_jobs = Vec::new();

                // Check for timed out jobs
                {
                    let running_jobs_guard = running_jobs.read().await;
                    for (job_id, running_job) in running_jobs_guard.iter() {
                        let elapsed = now - running_job.started_at;
                        if elapsed.to_std().unwrap_or_default() > running_job.job.timeout {
                            timed_out_jobs.push(*job_id);
                        }
                    }
                }

                // Handle timed out jobs
                for job_id in timed_out_jobs {
                    warn!("Job {} timed out, terminating", job_id);
                    
                    let timeout_event = Event::new(
                        EventType::Custom("JobTimeout".to_string()),
                        EventPayload::Custom {
                            data: serde_json::json!({
                                "runner_id": runner_id,
                                "job_id": job_id,
                                "timestamp": now
                            }),
                        },
                    );

                    if let Err(e) = event_demux.submit_event(timeout_event).await {
                        error!("Failed to submit job timeout event: {}", e);
                    }

                    // Remove the timed out job
                    {
                        let mut running_jobs_guard = running_jobs.write().await;
                        running_jobs_guard.remove(&job_id);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start isolation cleanup task
    async fn start_isolation_cleanup(&self) -> Result<()> {
        let runner_id = self.id;
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                // Clean up orphaned isolation contexts
                if let Err(e) = Self::cleanup_orphaned_isolation(&config).await {
                    error!("Failed to cleanup orphaned isolation for runner {}: {}", runner_id, e);
                }
            }
        });

        Ok(())
    }

    /// Start capacity management task
    async fn start_capacity_management(&self) -> Result<()> {
        let runner_id = self.id;
        let running_jobs = self.running_jobs.clone();
        let config = self.config.clone();
        let event_demux = self.event_demux.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                // Monitor capacity and send updates
                let running_jobs_guard = running_jobs.read().await;
                let current_jobs = running_jobs_guard.len() as u32;
                let max_jobs = config.max_concurrent_jobs;
                let available_slots = max_jobs.saturating_sub(current_jobs);
                
                let capacity_event = Event::new(
                    EventType::Custom("CapacityUpdate".to_string()),
                    EventPayload::Custom {
                        data: serde_json::json!({
                            "runner_id": runner_id,
                            "max_concurrent_jobs": max_jobs,
                            "current_jobs": current_jobs,
                            "available_slots": available_slots,
                            "utilization": current_jobs as f64 / max_jobs as f64,
                            "timestamp": Utc::now()
                        }),
                    },
                );

                if let Err(e) = event_demux.submit_event(capacity_event).await {
                    error!("Failed to submit capacity update event: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start health monitoring task
    async fn start_health_monitoring(&self, interval: Duration) -> Result<()> {
        let runner_id = self.id;
        let event_demux = self.event_demux.clone();
        let running_jobs = self.running_jobs.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                // Perform health check
                let health_status = Self::perform_health_check(
                    &running_jobs,
                    &config,
                ).await;

                // Send health status event
                let health_event = Event::new(
                    EventType::RunnerHealthCheck,
                    EventPayload::RunnerHealthCheck {
                        runner_id,
                        health_status,
                    },
                );

                if let Err(e) = event_demux.submit_event(health_event).await {
                    error!("Failed to submit health check event: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Perform health check
    async fn perform_health_check(
        running_jobs: &Arc<RwLock<HashMap<JobId, RunningJob>>>,
        config: &NativeRunnerConfig,
    ) -> HealthStatus {
        // Check if working directory is accessible
        if !config.working_directory.exists() {
            return HealthStatus::Unhealthy {
                reason: "Working directory does not exist".to_string(),
            };
        }

        // Check current load
        let running_jobs_guard = running_jobs.read().await;
        let load = running_jobs_guard.len() as f64 / config.max_concurrent_jobs as f64;

        if load >= 1.0 {
            HealthStatus::Degraded {
                reason: "Runner at full capacity".to_string(),
            }
        } else {
            HealthStatus::Healthy
        }
    }

    /// Check if runner has capacity for more jobs
    async fn has_capacity(&self) -> Result<bool> {
        let running_jobs = self.running_jobs.read().await;
        Ok(running_jobs.len() < self.config.max_concurrent_jobs as usize)
    }

    /// Spawn job execution with enhanced isolation
    async fn spawn_job_execution(&self, job: Job) -> Result<()> {
        let job_id = job.id;
        
        // Create isolation context
        let isolation_context = self.create_isolation_context(&job).await?;
        
        let running_job = RunningJob {
            job: job.clone(),
            started_at: Utc::now(),
            process_id: None,
            current_step: 0,
            output_buffer: String::new(),
            error_buffer: String::new(),
            isolation_context,
        };

        // Add to running jobs
        {
            let mut running_jobs = self.running_jobs.write().await;
            running_jobs.insert(job_id, running_job);
        }

        // Spawn execution task
        let _runner_id = self.id;
        let config = self.config.clone();
        let running_jobs = self.running_jobs.clone();
        let event_demux = self.event_demux.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let result = Self::execute_job_with_isolation(
                job.clone(),
                config,
                running_jobs.clone(),
            ).await;

            // Update statistics
            {
                let mut stats = stats.write().await;
                match &result {
                    Ok(job_result) => {
                        if job_result.is_success() {
                            stats.successful_jobs += 1;
                        } else {
                            stats.failed_jobs += 1;
                        }
                    }
                    Err(_) => {
                        stats.failed_jobs += 1;
                    }
                }
                stats.total_jobs_executed += 1;
                stats.last_job_completed = Some(Utc::now());
            }

            // Send completion event
            let completion_event = Event::new(
                EventType::JobCompletion,
                EventPayload::JobCompletion {
                    job_id,
                    result: match result {
                        Ok(job_result) => crate::core::infrastructure::event_loop::JobCompletionResult {
                            success: job_result.is_success(),
                            exit_code: job_result.exit_code,
                            output: job_result.stdout,
                            error: job_result.error_message,
                            duration: job_result.duration.unwrap_or_default(),
                        },
                        Err(e) => crate::core::infrastructure::event_loop::JobCompletionResult {
                            success: false,
                            exit_code: Some(-1),
                            output: String::new(),
                            error: Some(e.to_string()),
                            duration: Duration::default(),
                        },
                    },
                },
            );

            if let Err(e) = event_demux.submit_event(completion_event).await {
                error!("Failed to submit job completion event: {}", e);
            }
        });

        debug!("Spawned execution for job: {} with isolation", job_id);
        Ok(())
    }

    /// Create isolation context for a job
    async fn create_isolation_context(&self, _job: &Job) -> Result<Option<IsolationContext>> {
        match &self.config.isolation_level {
            IsolationLevel::None => Ok(None),
            IsolationLevel::ProcessGroup => {
                Ok(Some(IsolationContext {
                    process_group_id: None, // Will be set during execution
                    #[cfg(target_os = "linux")]
                    namespace_info: None,
                    #[cfg(target_os = "linux")]
                    cgroup_info: None,
                }))
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Namespace { .. } => {
                let namespace_info = self.create_namespace_context().await?;
                Ok(Some(IsolationContext {
                    process_group_id: None,
                    namespace_info: Some(namespace_info),
                    cgroup_info: None,
                }))
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Cgroup { cpu_limit, memory_limit, .. } => {
                let cgroup_info = self.create_cgroup_context(*cpu_limit, *memory_limit).await?;
                Ok(Some(IsolationContext {
                    process_group_id: None,
                    namespace_info: None,
                    cgroup_info: Some(cgroup_info),
                }))
            }
        }
    }

    #[cfg(target_os = "linux")]
    async fn create_namespace_context(&self) -> Result<NamespaceInfo> {
        // In a real implementation, this would create actual namespaces
        // For now, we'll create a placeholder
        Ok(NamespaceInfo {
            pid_namespace: Some(format!("rustci-{}", self.id)),
            network_namespace: None,
            mount_namespace: None,
            user_namespace: None,
        })
    }

    #[cfg(target_os = "linux")]
    async fn create_cgroup_context(&self, cpu_limit: Option<u32>, memory_limit: Option<u32>) -> Result<CgroupInfo> {
        // In a real implementation, this would create actual cgroups
        // For now, we'll create a placeholder
        Ok(CgroupInfo {
            cgroup_path: format!("/sys/fs/cgroup/rustci/{}", self.id),
            cpu_limit,
            memory_limit,
        })
    }

    /// Execute job with isolation
    async fn execute_job_with_isolation(
        job: Job,
        config: NativeRunnerConfig,
        running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    ) -> Result<JobResult> {
        let job_id = job.id;
        let mut result = JobResult::new(job_id);
        result.start();

        debug!("Executing job with isolation: {} with {} steps", job_id, job.steps.len());

        let mut step_results = Vec::new();
        let mut overall_success = true;

        for (step_index, step) in job.steps.iter().enumerate() {
            // Update current step
            {
                let mut running_jobs = running_jobs.write().await;
                if let Some(running_job) = running_jobs.get_mut(&job_id) {
                    running_job.current_step = step_index;
                }
            }

            let step_start = Instant::now();
            debug!("Executing step: {} for job: {}", step.name, job_id);

            let step_result = Self::execute_step_with_isolation(step, &config).await;
            let step_duration = step_start.elapsed();

            let step_success = step_result.is_ok();
            let (exit_code, stdout, stderr, error_message) = match step_result {
                Ok((code, out, err)) => (Some(code), out, err, None),
                Err(e) => (Some(-1), String::new(), String::new(), Some(e.to_string())),
            };

            let step_result = StepResult {
                name: step.name.clone(),
                status: if step_success {
                    JobStatus::Success
                } else {
                    JobStatus::Failed
                },
                exit_code,
                stdout: stdout.clone(),
                stderr: stderr.clone(),
                duration: step_duration,
                error_message,
            };

            step_results.push(step_result);

            // Update output buffers
            {
                let mut running_jobs = running_jobs.write().await;
                if let Some(running_job) = running_jobs.get_mut(&job_id) {
                    running_job.output_buffer.push_str(&stdout);
                    running_job.error_buffer.push_str(&stderr);
                }
            }

            if !step_success {
                overall_success = false;
                if !step.continue_on_error {
                    warn!(
                        "Step {} failed for job {}, stopping execution",
                        step.name, job_id
                    );
                    break;
                }
            }
        }

        // Complete the job result
        let final_status = if overall_success {
            JobStatus::Success
        } else {
            JobStatus::Failed
        };
        let exit_code = if overall_success { Some(0) } else { Some(1) };
        result.complete(final_status.clone(), exit_code);
        result.step_results = step_results;

        // Set final output
        {
            let running_jobs = running_jobs.read().await;
            if let Some(running_job) = running_jobs.get(&job_id) {
                result.stdout = running_job.output_buffer.clone();
                result.stderr = running_job.error_buffer.clone();
            }
        }

        info!("Job {} completed with status: {:?}", job_id, final_status);
        Ok(result)
    }

    /// Execute a single job step with isolation
    async fn execute_step_with_isolation(
        step: &crate::domain::entities::JobStep,
        config: &NativeRunnerConfig,
    ) -> Result<(i32, String, String)> {
        let working_dir = step
            .working_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| config.working_directory.clone());

        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &step.command]);
            cmd
        } else {
            let mut cmd = Command::new(&config.shell);
            cmd.args(["-c", &format!("{} {}", step.command, step.args.join(" "))]);
            cmd
        };

        // Set working directory
        cmd.current_dir(&working_dir);

        // Set environment variables
        for (key, value) in &config.environment {
            cmd.env(key, value);
        }
        for (key, value) in &step.environment {
            cmd.env(key, value);
        }

        // Apply isolation settings
        Self::apply_isolation_to_command(&mut cmd, &config.isolation_level)?;

        // Configure stdio
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Apply timeout
        let timeout_duration = step
            .timeout
            .unwrap_or_else(|| Duration::from_secs(config.default_timeout_seconds));

        let execution_result = timeout(timeout_duration, async {
            let mut child = cmd.spawn().map_err(|e| {
                AppError::InternalServerError(format!("Failed to spawn process: {}", e))
            })?;

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let mut stdout_lines = stdout_reader.lines();
            let mut stderr_lines = stderr_reader.lines();

            let mut stdout_output = String::new();
            let mut stderr_output = String::new();

            // Read output streams
            loop {
                tokio::select! {
                    line = stdout_lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                stdout_output.push_str(&line);
                                stdout_output.push('\n');
                                if config.enable_output_streaming {
                                    debug!("STDOUT: {}", line);
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                error!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                    line = stderr_lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                stderr_output.push_str(&line);
                                stderr_output.push('\n');
                                if config.enable_output_streaming {
                                    debug!("STDERR: {}", line);
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                error!("Error reading stderr: {}", e);
                                break;
                            }
                        }
                    }
                    status = child.wait() => {
                        match status {
                            Ok(exit_status) => {
                                let exit_code = exit_status.code().unwrap_or(-1);
                                return Ok((exit_code, stdout_output, stderr_output));
                            }
                            Err(e) => {
                                return Err(AppError::InternalServerError(
                                    format!("Process execution failed: {}", e)
                                ));
                            }
                        }
                    }
                }
            }

            // Wait for process to complete
            let exit_status = child.wait().await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to wait for process: {}", e))
            })?;

            let exit_code = exit_status.code().unwrap_or(-1);
            Ok((exit_code, stdout_output, stderr_output))
        })
        .await;

        match execution_result {
            Ok(result) => result,
            Err(_) => Err(AppError::TimeoutError(format!(
                "Step '{}' timed out after {:?}",
                step.name, timeout_duration
            ))),
        }
    }

    /// Apply isolation settings to command
    fn apply_isolation_to_command(
        cmd: &mut Command,
        isolation_level: &IsolationLevel,
    ) -> Result<()> {
        match isolation_level {
            IsolationLevel::None => {
                // No isolation
                Ok(())
            }
            IsolationLevel::ProcessGroup => {
                // Create new process group
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    cmd.process_group(0);
                }
                Ok(())
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Namespace { .. } => {
                // Apply namespace isolation
                // In a real implementation, this would use unshare() or clone()
                warn!("Namespace isolation not fully implemented");
                Ok(())
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Cgroup { .. } => {
                // Apply cgroup isolation
                // In a real implementation, this would configure cgroups
                warn!("Cgroup isolation not fully implemented");
                Ok(())
            }
        }
    }

    /// Clean up completed jobs
    async fn cleanup_completed_jobs(&self) -> Result<()> {
        // This would be more sophisticated in a real implementation
        // For now, we rely on the event handler to clean up
        Ok(())
    }

    /// Update runner statistics
    async fn update_stats(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        let running_jobs = self.running_jobs.read().await;
        
        stats.current_load = running_jobs.len() as f64 / self.config.max_concurrent_jobs as f64;
        stats.uptime = Duration::from_millis(
            (Utc::now() - self.start_time).num_milliseconds() as u64
        );

        Ok(())
    }

    /// Get runner statistics
    pub async fn get_stats(&self) -> NativeRunnerStats {
        self.stats.read().await.clone()
    }

    /// Get currently running jobs
    pub async fn get_running_jobs(&self) -> Vec<Job> {
        let running_jobs = self.running_jobs.read().await;
        running_jobs.values().map(|rj| rj.job.clone()).collect()
    }

    /// Collect resource usage information
    async fn collect_resource_usage(
        running_jobs: &Arc<RwLock<HashMap<JobId, RunningJob>>>,
        config: &NativeRunnerConfig,
    ) -> serde_json::Value {
        let running_jobs_guard = running_jobs.read().await;
        let job_count = running_jobs_guard.len();
        let max_jobs = config.max_concurrent_jobs as usize;
        
        // In a real implementation, this would collect actual system metrics
        // For now, we'll provide estimated values based on job count
        let cpu_usage = (job_count as f64 / max_jobs as f64) * 100.0;
        let memory_usage = (job_count as f64 / max_jobs as f64) * 80.0; // Assume 80% max memory usage
        
        serde_json::json!({
            "cpu_usage_percent": cpu_usage.min(100.0),
            "memory_usage_percent": memory_usage.min(100.0),
            "disk_usage_percent": 25.0, // Placeholder
            "active_jobs": job_count,
            "max_jobs": max_jobs,
            "load_average": cpu_usage / 100.0,
            "working_directory": config.working_directory,
            "isolation_level": config.isolation_level
        })
    }

    /// Clean up orphaned isolation contexts
    async fn cleanup_orphaned_isolation(config: &NativeRunnerConfig) -> Result<()> {
        match &config.isolation_level {
            IsolationLevel::None => {
                // No cleanup needed
                Ok(())
            }
            IsolationLevel::ProcessGroup => {
                // Clean up orphaned process groups
                Self::cleanup_orphaned_process_groups().await
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Namespace { .. } => {
                // Clean up orphaned namespaces
                Self::cleanup_orphaned_namespaces().await
            }
            #[cfg(target_os = "linux")]
            IsolationLevel::Cgroup { .. } => {
                // Clean up orphaned cgroups
                Self::cleanup_orphaned_cgroups().await
            }
        }
    }

    /// Clean up orphaned process groups
    async fn cleanup_orphaned_process_groups() -> Result<()> {
        // In a real implementation, this would identify and clean up
        // orphaned process groups created by this runner
        debug!("Cleaning up orphaned process groups");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn cleanup_orphaned_namespaces() -> Result<()> {
        // In a real implementation, this would clean up orphaned namespaces
        debug!("Cleaning up orphaned namespaces");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn cleanup_orphaned_cgroups() -> Result<()> {
        // In a real implementation, this would clean up orphaned cgroups
        debug!("Cleaning up orphaned cgroups");
        Ok(())
    }

    /// Graceful shutdown with job completion waiting
    pub async fn graceful_shutdown(&self, timeout: Duration) -> Result<()> {
        info!("Starting graceful shutdown for runner: {}", self.config.name);

        // Set shutdown flag
        {
            let mut shutdown = self.shutdown.lock().await;
            *shutdown = true;
        }

        // Send shutdown notification to control plane
        if self.config.control_plane_coordination.enabled {
            let shutdown_event = Event::new(
                EventType::Custom("RunnerShutdown".to_string()),
                EventPayload::Custom {
                    data: serde_json::json!({
                        "runner_id": self.id,
                        "reason": "graceful_shutdown",
                        "timestamp": Utc::now()
                    }),
                },
            );

            if let Err(e) = self.event_demux.submit_event(shutdown_event).await {
                warn!("Failed to send shutdown notification: {}", e);
            }
        }

        // Wait for running jobs to complete with timeout
        let start = Instant::now();
        while start.elapsed() < timeout {
            let running_jobs = self.running_jobs.read().await;
            if running_jobs.is_empty() {
                break;
            }
            
            let job_count = running_jobs.len();
            drop(running_jobs);
            
            info!("Waiting for {} jobs to complete...", job_count);
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        // Force cleanup remaining jobs if timeout exceeded
        {
            let mut running_jobs = self.running_jobs.write().await;
            if !running_jobs.is_empty() {
                warn!("Force terminating {} remaining jobs due to timeout", running_jobs.len());
                running_jobs.clear();
            }
        }

        // Clean up isolation contexts
        if let Err(e) = Self::cleanup_orphaned_isolation(&self.config).await {
            warn!("Failed to cleanup isolation contexts during shutdown: {}", e);
        }

        info!("Graceful shutdown completed for runner: {}", self.config.name);
        Ok(())
    }

    /// Handle runner migration to another node
    pub async fn migrate_to_node(&self, target_node_id: Uuid) -> Result<()> {
        info!("Starting migration of runner {} to node {}", self.id, target_node_id);

        // Send migration start event
        let migration_event = Event::new(
            EventType::Custom("RunnerMigration".to_string()),
            EventPayload::Custom {
                data: serde_json::json!({
                    "runner_id": self.id,
                    "target_node_id": target_node_id,
                    "phase": "start",
                    "timestamp": Utc::now()
                }),
            },
        );

        self.event_demux.submit_event(migration_event).await?;

        // Wait for running jobs to complete
        self.graceful_shutdown(Duration::from_secs(300)).await?;

        // Send migration complete event
        let migration_complete_event = Event::new(
            EventType::Custom("RunnerMigration".to_string()),
            EventPayload::Custom {
                data: serde_json::json!({
                    "runner_id": self.id,
                    "target_node_id": target_node_id,
                    "phase": "complete",
                    "timestamp": Utc::now()
                }),
            },
        );

        self.event_demux.submit_event(migration_complete_event).await?;

        info!("Migration completed for runner {} to node {}", self.id, target_node_id);
        Ok(())
    }

    /// Send a heartbeat event
    pub async fn send_heartbeat(&self) -> Result<()> {
        let capacity = self.get_capacity().await?;
        let status = if capacity.available_slots > 0 {
            crate::domain::entities::RunnerStatus::Idle
        } else {
            crate::domain::entities::RunnerStatus::Busy
        };

        let heartbeat_event = Event::new(
            EventType::RunnerHeartbeat,
            EventPayload::RunnerHeartbeat {
                runner_id: self.id,
                status,
            },
        );

        self.event_demux.submit_event(heartbeat_event).await?;
        
        // Update heartbeat statistics
        {
            let mut stats = self.stats.write().await;
            stats.control_plane_heartbeats += 1;
        }
        
        Ok(())
    }

    /// Start heartbeat timer
    pub async fn start_heartbeat(&self, interval: Duration) -> Result<()> {
        let runner_id = self.id;
        let event_demux = self.event_demux.clone();
        let stats = self.stats.clone();

        let callback = Box::new(move || {
            let event_demux = event_demux.clone();
            let runner_id = runner_id;
            let stats = stats.clone();

            tokio::spawn(async move {
                let heartbeat_event = Event::new(
                    EventType::RunnerHeartbeat,
                    EventPayload::RunnerHeartbeat {
                        runner_id,
                        status: crate::domain::entities::RunnerStatus::Active,
                    },
                );

                if let Err(e) = event_demux.submit_event(heartbeat_event).await {
                    error!("Failed to send heartbeat: {}", e);
                } else {
                    // Update heartbeat statistics
                    let mut stats = stats.write().await;
                    stats.control_plane_heartbeats += 1;
                }
            });

            Ok(())
        });

        self.event_demux
            .schedule_recurring_timer(format!("heartbeat-{}", self.id), interval, callback)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl Runner for NativeProcessRunner {
    async fn execute(&self, job: Job) -> Result<JobResult> {
        debug!("Queuing job for execution: {}", job.id);

        // Add job to queue
        self.job_queue.enqueue(job.clone()).await?;

        // For this implementation, we return a placeholder result
        // In a real system, this would be handled asynchronously
        let mut result = JobResult::new(job.id);
        result.status = JobStatus::Queued;
        Ok(result)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Self::perform_health_check(&self.running_jobs, &self.config).await;
        
        // Check if working directory is accessible
        if !self.config.working_directory.exists() {
            return Ok(HealthStatus::Unhealthy {
                reason: "Working directory does not exist".to_string(),
            });
        }

        // Check current load
        let running_jobs = self.running_jobs.read().await;
        let load = running_jobs.len() as f64 / self.config.max_concurrent_jobs as f64;

        if load >= 1.0 {
            Ok(HealthStatus::Degraded {
                reason: "Runner at full capacity".to_string(),
            })
        } else {
            Ok(HealthStatus::Healthy)
        }
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        let running_jobs = self.running_jobs.read().await;
        let current_jobs = running_jobs.len() as u32;
        let max_jobs = self.config.max_concurrent_jobs;

        Ok(RunnerCapacity {
            max_concurrent_jobs: max_jobs,
            current_jobs,
            available_slots: max_jobs.saturating_sub(current_jobs),
            cpu_usage: 0.0,    // Would be implemented with system monitoring
            memory_usage: 0.0, // Would be implemented with system monitoring
            disk_usage: 0.0,   // Would be implemented with system monitoring
        })
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down native process runner: {}", self.config.name);

        // Set shutdown flag
        {
            let mut shutdown = self.shutdown.lock().await;
            *shutdown = true;
        }

        // Wait for running jobs to complete (with timeout)
        let timeout_duration = Duration::from_secs(30);
        let start = Instant::now();

        while start.elapsed() < timeout_duration {
            let running_jobs = self.running_jobs.read().await;
            if running_jobs.is_empty() {
                break;
            }
            drop(running_jobs);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Force cleanup remaining jobs
        {
            let mut running_jobs = self.running_jobs.write().await;
            running_jobs.clear();
        }

        info!("Native process runner shutdown complete");
        Ok(())
    }

    fn get_metadata(&self) -> RunnerMetadata {
        RunnerMetadata {
            id: self.id,
            name: self.config.name.clone(),
            runner_type: RunnerType::Local {
                max_concurrent_jobs: self.config.max_concurrent_jobs,
                working_directory: self.config.working_directory.to_string_lossy().to_string(),
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_job_types: vec![
                "shell".to_string(),
                "script".to_string(),
                "native".to_string(),
            ],
        }
    }

    async fn can_handle_job(&self, job: &Job) -> Result<bool> {
        // Check if we have capacity
        if !self.has_capacity().await? {
            return Ok(false);
        }

        // Check if job requirements match our capabilities
        // Native runner can handle most basic jobs
        
        // Check for native runner tag requirement
        if job.requirements.required_tags.contains(&"native".to_string()) {
            return Ok(true);
        }

        // Check if any excluded tags match our tags
        for excluded_tag in &job.requirements.excluded_tags {
            if self.config.control_plane_coordination.tags.contains(excluded_tag) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// Job completion event handler for native runner
pub struct NativeJobCompletionHandler {
    runner_id: RunnerId,
    running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    stats: Arc<RwLock<NativeRunnerStats>>,
}

#[async_trait]
impl EventHandler for NativeJobCompletionHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::JobCompletion { job_id, result } = event.payload {
            debug!("Handling job completion for: {}", job_id);

            // Remove from running jobs
            {
                let mut running_jobs = self.running_jobs.write().await;
                running_jobs.remove(&job_id);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_jobs_executed += 1;
                stats.last_job_completed = Some(Utc::now());

                if result.success {
                    stats.successful_jobs += 1;
                } else {
                    stats.failed_jobs += 1;
                }

                // Update average execution time
                let total_time = stats.avg_execution_time.as_nanos() as u64
                    * (stats.total_jobs_executed - 1)
                    + result.duration.as_nanos() as u64;
                stats.avg_execution_time =
                    Duration::from_nanos(total_time / stats.total_jobs_executed);
            }

            info!("Job {} completed successfully: {}", job_id, result.success);
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::JobCompletion]
    }

    fn name(&self) -> &str {
        "native-job-completion-handler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{BackoffStrategy, JobRequirements, JobStep, RetryPolicy};
    use std::collections::HashMap;

    fn create_test_job() -> Job {
        Job {
            id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
            name: "test-job".to_string(),
            steps: vec![JobStep {
                name: "echo-step".to_string(),
                command: "echo".to_string(),
                args: vec!["hello world".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(10)),
                continue_on_error: false,
            }],
            requirements: JobRequirements::default(),
            priority: crate::domain::entities::JobPriority::Normal,
            timeout: Duration::from_secs(300),
            retry_policy: RetryPolicy {
                max_retries: 3,
                retry_delay: Duration::from_secs(30),
                backoff_strategy: BackoffStrategy::Fixed,
            },
            metadata: HashMap::new(),
            created_at: Utc::now(),
            scheduled_at: None,
        }
    }

    #[tokio::test]
    async fn test_native_runner_creation() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        let metadata = runner.get_metadata();

        assert_eq!(metadata.name, "native-runner");
        assert!(matches!(metadata.runner_type, RunnerType::Local { .. }));
        assert!(metadata.supported_job_types.contains(&"native".to_string()));
    }

    #[tokio::test]
    async fn test_native_runner_health_check() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        let health = runner.health_check().await.unwrap();

        assert_eq!(health, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_native_runner_capacity() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        let capacity = runner.get_capacity().await.unwrap();

        assert_eq!(capacity.max_concurrent_jobs, 4);
        assert_eq!(capacity.current_jobs, 0);
        assert_eq!(capacity.available_slots, 4);
    }

    #[tokio::test]
    async fn test_native_runner_job_execution() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        let job = create_test_job();

        let result = runner.execute(job).await.unwrap();
        assert_eq!(result.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_native_runner_can_handle_job() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        let mut job = create_test_job();

        // Test with native tag requirement
        job.requirements.required_tags = vec!["native".to_string()];
        let can_handle = runner.can_handle_job(&job).await.unwrap();
        assert!(can_handle);

        // Test with excluded tag
        job.requirements.required_tags = vec![];
        job.requirements.excluded_tags = vec!["native".to_string()];
        let can_handle = runner.can_handle_job(&job).await.unwrap();
        assert!(!can_handle);
    }

    #[tokio::test]
    async fn test_isolation_level_default() {
        let isolation = IsolationLevel::default();
        assert_eq!(isolation, IsolationLevel::ProcessGroup);
    }

    #[tokio::test]
    async fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_cpu, Some(1.0));
        assert_eq!(limits.max_memory, Some(1024));
        assert!(limits.max_execution_time.is_some());
    }

    #[tokio::test]
    async fn test_control_plane_coordination_config() {
        let config = ControlPlaneCoordinationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.heartbeat_interval, 30);
        assert!(config.tags.contains(&"native".to_string()));
    }

    #[tokio::test]
    async fn test_enhanced_lifecycle_management() {
        let mut config = NativeRunnerConfig::default();
        config.enhanced_lifecycle = true;
        config.control_plane_coordination.enabled = true;
        
        let event_demux = Arc::new(EventDemultiplexer::new());
        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        
        // Test that the runner can be created with enhanced lifecycle
        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "native-runner");
        
        // Test graceful shutdown
        let shutdown_result = runner.graceful_shutdown(Duration::from_secs(1)).await;
        assert!(shutdown_result.is_ok());
    }

    #[tokio::test]
    async fn test_resource_usage_collection() {
        let config = NativeRunnerConfig::default();
        let running_jobs = Arc::new(RwLock::new(HashMap::new()));
        
        let resource_usage = NativeProcessRunner::collect_resource_usage(&running_jobs, &config).await;
        
        // Verify resource usage structure
        assert!(resource_usage.get("cpu_usage_percent").is_some());
        assert!(resource_usage.get("memory_usage_percent").is_some());
        assert!(resource_usage.get("active_jobs").is_some());
        assert!(resource_usage.get("max_jobs").is_some());
    }

    #[tokio::test]
    async fn test_migration_functionality() {
        let config = NativeRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());
        let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();
        
        let target_node_id = Uuid::new_v4();
        let migration_result = runner.migrate_to_node(target_node_id).await;
        
        // Migration should complete successfully
        assert!(migration_result.is_ok());
    }
}