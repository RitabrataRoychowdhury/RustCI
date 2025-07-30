//! Docker runner implementation for executing jobs in containers
//!
//! This module provides a Docker-based runner that executes jobs in isolated
//! containers using the bollard Docker client library.

use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, RemoveContainerOptions, StartContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::{CreateImageOptions, ListImagesOptions};
use bollard::models::{ContainerCreateResponse, HostConfig, Mount, MountTypeEnum};
use bollard::Docker;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{timeout, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::event_loop::{Event, EventDemultiplexer, EventHandler, EventPayload, EventType};
use crate::domain::entities::{
    DockerConfig, HealthStatus, Job, JobId, JobResult, JobStatus, RunnerCapacity, RunnerId,
    RunnerMetadata, RunnerType, StepResult, VolumeMount,
};
use crate::domain::entities::runner::Runner;
use crate::error::{AppError, Result};

/// Docker runner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRunnerConfig {
    /// Runner name
    pub name: String,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Docker daemon endpoint
    pub docker_endpoint: String,
    /// Default image for jobs
    pub default_image: String,
    /// Network to use for containers
    pub network: Option<String>,
    /// Default volume mounts
    pub volume_mounts: Vec<VolumeMount>,
    /// Default environment variables
    pub environment: HashMap<String, String>,
    /// Container resource limits
    pub resource_limits: DockerResourceLimits,
    /// Container cleanup settings
    pub cleanup_settings: DockerCleanupSettings,
    /// Image pull policy
    pub image_pull_policy: ImagePullPolicy,
}

/// Docker resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerResourceLimits {
    /// Memory limit in bytes
    pub memory_limit: Option<i64>,
    /// CPU limit (nano CPUs)
    pub cpu_limit: Option<i64>,
    /// CPU shares (relative weight)
    pub cpu_shares: Option<i64>,
    /// Disk space limit in bytes
    pub disk_limit: Option<i64>,
}

/// Docker cleanup settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerCleanupSettings {
    /// Remove containers after execution
    pub remove_containers: bool,
    /// Remove volumes after execution
    pub remove_volumes: bool,
    /// Timeout for container removal
    pub removal_timeout_seconds: u64,
}

/// Image pull policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImagePullPolicy {
    /// Always pull the image
    Always,
    /// Pull if not present locally
    IfNotPresent,
    /// Never pull, use local image only
    Never,
}

impl Default for DockerRunnerConfig {
    fn default() -> Self {
        Self {
            name: "docker-runner".to_string(),
            max_concurrent_jobs: 4,
            docker_endpoint: "unix:///var/run/docker.sock".to_string(),
            default_image: "ubuntu:22.04".to_string(),
            network: None,
            volume_mounts: Vec::new(),
            environment: HashMap::new(),
            resource_limits: DockerResourceLimits::default(),
            cleanup_settings: DockerCleanupSettings::default(),
            image_pull_policy: ImagePullPolicy::IfNotPresent,
        }
    }
}

impl Default for DockerResourceLimits {
    fn default() -> Self {
        Self {
            memory_limit: Some(512 * 1024 * 1024), // 512MB
            cpu_limit: Some(1_000_000_000),        // 1 CPU
            cpu_shares: Some(1024),
            disk_limit: Some(1024 * 1024 * 1024), // 1GB
        }
    }
}

impl Default for DockerCleanupSettings {
    fn default() -> Self {
        Self {
            remove_containers: true,
            remove_volumes: true,
            removal_timeout_seconds: 30,
        }
    }
}

/// Docker runner implementation
pub struct DockerRunner {
    /// Runner ID
    id: RunnerId,
    /// Runner configuration
    config: DockerRunnerConfig,
    /// Docker client
    docker: Docker,
    /// Currently running containers
    running_containers: Arc<RwLock<HashMap<JobId, RunningContainer>>>,
    /// Event demultiplexer
    event_demux: Arc<EventDemultiplexer>,
    /// Runner statistics
    stats: Arc<RwLock<DockerRunnerStats>>,
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

/// Information about a running container
#[derive(Debug, Clone)]
pub struct RunningContainer {
    job: Job,
    container_id: String,
    container_name: String,
    started_at: DateTime<Utc>,
    current_step: usize,
    image: String,
}

/// Docker runner statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerRunnerStats {
    pub total_containers_created: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_execution_time: Duration,
    pub total_images_pulled: u64,
    pub containers_cleaned_up: u64,
    pub current_load: f64,
    pub uptime: Duration,
    pub last_job_completed: Option<DateTime<Utc>>,
}

/// Docker job completion event handler
pub struct DockerJobCompletionHandler {
    runner_id: RunnerId,
    running_containers: Arc<RwLock<HashMap<JobId, RunningContainer>>>,
    stats: Arc<RwLock<DockerRunnerStats>>,
    docker: Docker,
    cleanup_settings: DockerCleanupSettings,
}

impl DockerRunner {
    /// Create a new Docker runner
    pub async fn new(
        config: DockerRunnerConfig,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<Self> {
        let id = Uuid::new_v4();

        // Initialize Docker client
        let docker = if config.docker_endpoint.starts_with("unix://") {
            Docker::connect_with_socket(&config.docker_endpoint, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| {
                    AppError::InternalServerError(format!(
                        "Failed to connect to Docker daemon: {}",
                        e
                    ))
                })?
        } else {
            Docker::connect_with_http(&config.docker_endpoint, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| {
                    AppError::InternalServerError(format!(
                        "Failed to connect to Docker daemon: {}",
                        e
                    ))
                })?
        };

        let running_containers = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(DockerRunnerStats::default()));

        // Register job completion handler
        let completion_handler = Arc::new(DockerJobCompletionHandler {
            runner_id: id,
            running_containers: running_containers.clone(),
            stats: stats.clone(),
            docker: docker.clone(),
            cleanup_settings: config.cleanup_settings.clone(),
        });

        event_demux.register_handler(completion_handler).await?;

        info!(
            "Created Docker runner: {} connected to: {}",
            config.name, config.docker_endpoint
        );

        Ok(Self {
            id,
            config,
            docker,
            running_containers,
            event_demux,
            stats,
            shutdown: Arc::new(Mutex::new(false)),
        })
    }

    /// Check Docker daemon connectivity
    pub async fn check_docker_connectivity(&self) -> Result<bool> {
        match self.docker.ping().await {
            Ok(_) => {
                debug!("Docker daemon is accessible");
                Ok(true)
            }
            Err(e) => {
                warn!("Docker daemon connectivity check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Pull image if needed based on pull policy
    async fn ensure_image_available(&self, image: &str) -> Result<()> {
        match self.config.image_pull_policy {
            ImagePullPolicy::Never => {
                // Check if image exists locally
                let images = self
                    .docker
                    .list_images(Some(ListImagesOptions::<String> {
                        all: false,
                        filters: [("reference".to_string(), vec![image.to_string()])]
                            .iter()
                            .cloned()
                            .collect(),
                        ..Default::default()
                    }))
                    .await
                    .map_err(|e| {
                        AppError::InternalServerError(format!("Failed to list images: {}", e))
                    })?;

                if images.is_empty() {
                    return Err(AppError::InternalServerError(format!(
                        "Image {} not found locally and pull policy is Never",
                        image
                    )));
                }
            }
            ImagePullPolicy::IfNotPresent => {
                // Check if image exists locally first
                let images = self
                    .docker
                    .list_images(Some(ListImagesOptions::<String> {
                        all: false,
                        filters: [("reference".to_string(), vec![image.to_string()])]
                            .iter()
                            .cloned()
                            .collect(),
                        ..Default::default()
                    }))
                    .await
                    .map_err(|e| {
                        AppError::InternalServerError(format!("Failed to list images: {}", e))
                    })?;

                if images.is_empty() {
                    self.pull_image(image).await?;
                }
            }
            ImagePullPolicy::Always => {
                self.pull_image(image).await?;
            }
        }

        Ok(())
    }

    /// Pull Docker image
    async fn pull_image(&self, image: &str) -> Result<()> {
        info!("Pulling Docker image: {}", image);

        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        debug!("Image pull status: {}", status);
                    }
                    if let Some(error) = info.error {
                        return Err(AppError::InternalServerError(format!(
                            "Failed to pull image {}: {}",
                            image, error
                        )));
                    }
                }
                Err(e) => {
                    return Err(AppError::InternalServerError(format!(
                        "Failed to pull image {}: {}",
                        image, e
                    )));
                }
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_images_pulled += 1;
        }

        info!("Successfully pulled image: {}", image);
        Ok(())
    }

    /// Create and start container for job execution
    async fn create_container(&self, job: &Job) -> Result<String> {
        let container_name = format!("rustci-job-{}", job.id);
        let image = job
            .metadata
            .get("image")
            .unwrap_or(&self.config.default_image)
            .clone();

        // Ensure image is available
        self.ensure_image_available(&image).await?;

        // Prepare mounts
        let mut mounts = Vec::new();
        for volume_mount in &self.config.volume_mounts {
            mounts.push(Mount {
                target: Some(volume_mount.container_path.clone()),
                source: Some(volume_mount.host_path.clone()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(volume_mount.read_only),
                ..Default::default()
            });
        }

        // Prepare environment variables
        let mut env_vars = Vec::new();
        for (key, value) in &self.config.environment {
            env_vars.push(format!("{}={}", key, value));
        }
        for (key, value) in &job.metadata {
            if let Some(env_key) = key.strip_prefix("env.") {
                // Remove "env." prefix
                env_vars.push(format!("{}={}", env_key, value));
            }
        }

        // Create host config with resource limits
        let host_config = HostConfig {
            memory: self.config.resource_limits.memory_limit,
            nano_cpus: self.config.resource_limits.cpu_limit,
            cpu_shares: self.config.resource_limits.cpu_shares,
            mounts: Some(mounts),
            network_mode: self.config.network.clone(),
            auto_remove: Some(self.config.cleanup_settings.remove_containers),
            ..Default::default()
        };

        // Create container config
        let config = Config {
            image: Some(image.clone()),
            env: Some(env_vars),
            working_dir: Some("/workspace".to_string()),
            host_config: Some(host_config),
            ..Default::default()
        };

        // Create container
        let response: ContainerCreateResponse = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.clone(),
                    platform: None,
                }),
                config,
            )
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create container: {}", e))
            })?;

        let container_id = response.id;

        // Start container
        self.docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to start container: {}", e))
            })?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_containers_created += 1;
        }

        info!(
            "Created and started container: {} ({})",
            container_name, container_id
        );
        Ok(container_id)
    }

    /// Execute job steps in the container
    async fn execute_job_in_container(&self, job: Job, container_id: String) -> Result<JobResult> {
        let job_id = job.id;
        let mut result = JobResult::new(job_id);
        result.start();

        debug!("Executing job {} in container {}", job_id, container_id);

        let mut step_results = Vec::new();
        let mut overall_success = true;

        for (step_index, step) in job.steps.iter().enumerate() {
            // Update current step
            {
                let mut containers = self.running_containers.write().await;
                if let Some(container) = containers.get_mut(&job_id) {
                    container.current_step = step_index;
                }
            }

            let step_start = Instant::now();
            debug!(
                "Executing step: {} in container {}",
                step.name, container_id
            );

            let step_result = self.execute_step_in_container(&container_id, step).await;
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

            // Append to overall output
            result.stdout.push_str(&stdout);
            result.stderr.push_str(&stderr);

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

        info!(
            "Job {} completed in container with status: {:?}",
            job_id, final_status
        );
        Ok(result)
    }

    /// Execute a single step in the container
    async fn execute_step_in_container(
        &self,
        container_id: &str,
        step: &crate::domain::entities::JobStep,
    ) -> Result<(i32, String, String)> {
        // Build command
        let command = if step.args.is_empty() {
            step.command.clone()
        } else {
            format!("{} {}", step.command, step.args.join(" "))
        };

        // Create exec instance
        let env_vars: Vec<String> = step
            .environment
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let env_refs: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();

        let exec_config = CreateExecOptions {
            cmd: Some(vec!["sh", "-c", &command]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            working_dir: step.working_directory.as_deref(),
            env: Some(env_refs),
            ..Default::default()
        };

        let exec_result = self
            .docker
            .create_exec(container_id, exec_config)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to create exec: {}", e)))?;

        // Start exec and collect output
        let timeout_duration = step.timeout.unwrap_or_else(|| Duration::from_secs(3600));

        let execution_result = timeout(timeout_duration, async {
            let start_exec_result = self
                .docker
                .start_exec(&exec_result.id, None)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to start exec: {}", e))
                })?;

            let mut stdout = String::new();
            let mut stderr = String::new();

            if let StartExecResults::Attached { mut output, .. } = start_exec_result {
                while let Some(msg) = output.next().await {
                    match msg {
                        Ok(LogOutput::StdOut { message }) => {
                            let text = String::from_utf8_lossy(&message);
                            stdout.push_str(&text);
                            debug!("STDOUT: {}", text.trim());
                        }
                        Ok(LogOutput::StdErr { message }) => {
                            let text = String::from_utf8_lossy(&message);
                            stderr.push_str(&text);
                            debug!("STDERR: {}", text.trim());
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error reading exec output: {}", e);
                            break;
                        }
                    }
                }
            }

            // Get exit code
            let inspect_result = self
                .docker
                .inspect_exec(&exec_result.id)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to inspect exec: {}", e))
                })?;

            let exit_code = inspect_result.exit_code.unwrap_or(-1) as i32;
            Ok((exit_code, stdout, stderr))
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

    /// Clean up container
    async fn cleanup_container(&self, container_id: &str) -> Result<()> {
        if !self.config.cleanup_settings.remove_containers {
            return Ok(());
        }

        debug!("Cleaning up container: {}", container_id);

        let timeout_duration =
            Duration::from_secs(self.config.cleanup_settings.removal_timeout_seconds);

        let cleanup_result = timeout(timeout_duration, async {
            // Stop container first
            if let Err(e) = self.docker.stop_container(container_id, None).await {
                warn!("Failed to stop container {}: {}", container_id, e);
            }

            // Remove container
            self.docker
                .remove_container(
                    container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        v: self.config.cleanup_settings.remove_volumes,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to remove container: {}", e))
                })
        })
        .await;

        match cleanup_result {
            Ok(result) => {
                result?;
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.containers_cleaned_up += 1;
                }
                debug!("Successfully cleaned up container: {}", container_id);
                Ok(())
            }
            Err(_) => {
                warn!("Container cleanup timed out for: {}", container_id);
                Ok(()) // Don't fail the job if cleanup times out
            }
        }
    }

    /// Get runner statistics
    pub async fn get_stats(&self) -> DockerRunnerStats {
        self.stats.read().await.clone()
    }

    /// Get currently running containers
    pub async fn get_running_containers(&self) -> Vec<RunningContainer> {
        let containers = self.running_containers.read().await;
        containers.values().cloned().collect()
    }
}

#[async_trait]
impl crate::domain::entities::runner::Runner for DockerRunner {
    async fn execute(&self, job: Job) -> Result<JobResult> {
        let job_id = job.id;
        debug!("Starting Docker execution for job: {}", job_id);

        // Create and start container
        let container_id = self.create_container(&job).await?;
        let container_name = format!("rustci-job-{}", job_id);

        // Add to running containers
        {
            let mut containers = self.running_containers.write().await;
            containers.insert(
                job_id,
                RunningContainer {
                    job: job.clone(),
                    container_id: container_id.clone(),
                    container_name,
                    started_at: Utc::now(),
                    current_step: 0,
                    image: job
                        .metadata
                        .get("image")
                        .unwrap_or(&self.config.default_image)
                        .clone(),
                },
            );
        }

        // Execute job in container
        let result = self
            .execute_job_in_container(job, container_id.clone())
            .await;

        // Clean up container
        if let Err(e) = self.cleanup_container(&container_id).await {
            warn!("Failed to cleanup container {}: {}", container_id, e);
        }

        // Remove from running containers
        {
            let mut containers = self.running_containers.write().await;
            containers.remove(&job_id);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            match &result {
                Ok(job_result) => {
                    if job_result.is_success() {
                        stats.successful_executions += 1;
                    } else {
                        stats.failed_executions += 1;
                    }

                    if let Some(duration) = job_result.duration {
                        let total_time = stats.avg_execution_time.as_nanos() as u64
                            * (stats.successful_executions + stats.failed_executions - 1)
                            + duration.as_nanos() as u64;
                        stats.avg_execution_time = Duration::from_nanos(
                            total_time / (stats.successful_executions + stats.failed_executions),
                        );
                    }

                    stats.last_job_completed = Some(Utc::now());
                }
                Err(_) => {
                    stats.failed_executions += 1;
                }
            }
        }

        result
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Check Docker daemon connectivity
        if !self.check_docker_connectivity().await? {
            return Ok(HealthStatus::Unhealthy {
                reason: "Docker daemon is not accessible".to_string(),
            });
        }

        // Check current load
        let containers = self.running_containers.read().await;
        let load = containers.len() as f64 / self.config.max_concurrent_jobs as f64;

        if load >= 1.0 {
            Ok(HealthStatus::Degraded {
                reason: "Runner at full capacity".to_string(),
            })
        } else {
            Ok(HealthStatus::Healthy)
        }
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        let containers = self.running_containers.read().await;
        let current_jobs = containers.len() as u32;
        let max_jobs = self.config.max_concurrent_jobs;

        Ok(RunnerCapacity {
            max_concurrent_jobs: max_jobs,
            current_jobs,
            available_slots: max_jobs.saturating_sub(current_jobs),
            cpu_usage: 0.0,    // Would be implemented with Docker stats API
            memory_usage: 0.0, // Would be implemented with Docker stats API
            disk_usage: 0.0,   // Would be implemented with Docker stats API
        })
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Docker runner: {}", self.config.name);

        // Set shutdown flag
        {
            let mut shutdown = self.shutdown.lock().await;
            *shutdown = true;
        }

        // Stop all running containers
        let containers: Vec<_> = {
            let containers = self.running_containers.read().await;
            containers.values().cloned().collect()
        };

        for container in containers {
            if let Err(e) = self.cleanup_container(&container.container_id).await {
                warn!("Failed to cleanup container during shutdown: {}", e);
            }
        }

        // Clear running containers
        {
            let mut containers = self.running_containers.write().await;
            containers.clear();
        }

        info!("Docker runner shutdown complete");
        Ok(())
    }

    fn get_metadata(&self) -> RunnerMetadata {
        RunnerMetadata {
            id: self.id,
            name: self.config.name.clone(),
            runner_type: RunnerType::Docker {
                max_concurrent_jobs: self.config.max_concurrent_jobs,
                docker_config: DockerConfig {
                    endpoint: self.config.docker_endpoint.clone(),
                    default_image: self.config.default_image.clone(),
                    network: self.config.network.clone(),
                    volumes: self.config.volume_mounts.clone(),
                    environment: self.config.environment.clone(),
                },
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_job_types: vec![
                "shell".to_string(),
                "script".to_string(),
                "docker".to_string(),
            ],
        }
    }

    async fn can_handle_job(&self, job: &Job) -> Result<bool> {
        // Check if we have capacity
        let capacity = self.get_capacity().await?;
        if capacity.available_slots == 0 {
            return Ok(false);
        }

        // Check if Docker daemon is accessible
        if !self.check_docker_connectivity().await? {
            return Ok(false);
        }

        // Check if required image is available or can be pulled
        let image = job
            .metadata
            .get("image")
            .unwrap_or(&self.config.default_image);

        match self.ensure_image_available(image).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[async_trait]
impl EventHandler for DockerJobCompletionHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::JobCompletion { job_id, result } = event.payload {
            debug!("Handling Docker job completion for: {}", job_id);

            // Get container info before removing
            let container_id = {
                let containers = self.running_containers.read().await;
                containers.get(&job_id).map(|c| c.container_id.clone())
            };

            // Clean up container if it exists
            if let Some(container_id) = container_id {
                if let Err(e) = self.cleanup_container(&container_id).await {
                    warn!("Failed to cleanup container {}: {}", container_id, e);
                }
            }

            // Remove from running containers
            {
                let mut containers = self.running_containers.write().await;
                containers.remove(&job_id);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                if result.success {
                    stats.successful_executions += 1;
                } else {
                    stats.failed_executions += 1;
                }
                stats.last_job_completed = Some(Utc::now());
            }

            info!(
                "Docker job {} completed successfully: {}",
                job_id, result.success
            );
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::JobCompletion]
    }

    fn name(&self) -> &str {
        "docker-job-completion-handler"
    }
}

impl DockerJobCompletionHandler {
    async fn cleanup_container(&self, container_id: &str) -> Result<()> {
        if !self.cleanup_settings.remove_containers {
            return Ok(());
        }

        debug!("Cleaning up container: {}", container_id);

        let timeout_duration = Duration::from_secs(self.cleanup_settings.removal_timeout_seconds);

        let cleanup_result = timeout(timeout_duration, async {
            // Stop container first
            if let Err(e) = self.docker.stop_container(container_id, None).await {
                warn!("Failed to stop container {}: {}", container_id, e);
            }

            // Remove container
            self.docker
                .remove_container(
                    container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        v: self.cleanup_settings.remove_volumes,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to remove container: {}", e))
                })
        })
        .await;

        match cleanup_result {
            Ok(result) => {
                result?;
                debug!("Successfully cleaned up container: {}", container_id);
                Ok(())
            }
            Err(_) => {
                warn!("Container cleanup timed out for: {}", container_id);
                Ok(()) // Don't fail if cleanup times out
            }
        }
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
            name: "test-docker-job".to_string(),
            steps: vec![JobStep {
                name: "echo-step".to_string(),
                command: "echo".to_string(),
                args: vec!["hello from docker".to_string()],
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
    async fn test_docker_runner_creation() {
        let config = DockerRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        // This test might fail if Docker is not available
        if let Ok(runner) = DockerRunner::new(config, event_demux).await {
            let metadata = runner.get_metadata();
            assert_eq!(metadata.name, "docker-runner");
            assert!(matches!(metadata.runner_type, RunnerType::Docker { .. }));
        }
    }

    #[tokio::test]
    async fn test_docker_runner_config_defaults() {
        let config = DockerRunnerConfig::default();
        assert_eq!(config.name, "docker-runner");
        assert_eq!(config.max_concurrent_jobs, 4);
        assert_eq!(config.default_image, "ubuntu:22.04");
        assert!(config.cleanup_settings.remove_containers);
    }

    #[tokio::test]
    async fn test_image_pull_policy() {
        let policy = ImagePullPolicy::IfNotPresent;
        assert!(matches!(policy, ImagePullPolicy::IfNotPresent));

        let policy = ImagePullPolicy::Always;
        assert!(matches!(policy, ImagePullPolicy::Always));

        let policy = ImagePullPolicy::Never;
        assert!(matches!(policy, ImagePullPolicy::Never));
    }

    #[test]
    fn test_docker_resource_limits_default() {
        let limits = DockerResourceLimits::default();
        assert_eq!(limits.memory_limit, Some(512 * 1024 * 1024));
        assert_eq!(limits.cpu_limit, Some(1_000_000_000));
        assert_eq!(limits.cpu_shares, Some(1024));
    }

    #[test]
    fn test_docker_cleanup_settings_default() {
        let settings = DockerCleanupSettings::default();
        assert!(settings.remove_containers);
        assert!(settings.remove_volumes);
        assert_eq!(settings.removal_timeout_seconds, 30);
    }
}
