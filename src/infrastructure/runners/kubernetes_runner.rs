//! Kubernetes runner implementation for executing jobs in pods
//!
//! This module provides a Kubernetes-based runner that executes jobs in
//! isolated pods using the kube client library.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use k8s_openapi::api::batch::v1::{Job as K8sJob, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, EmptyDirVolumeSource, EnvVar, Pod, PodSpec, ResourceRequirements, SecurityContext,
    Volume, VolumeMount as K8sVolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{DeleteParams, ListParams, LogParams, PostParams},
    Api, Client,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::event_loop::{Event, EventDemultiplexer, EventHandler, EventPayload, EventType};
use crate::domain::entities::{
    HealthStatus, Job, JobId, JobResult, ResourceLimits, RunnerCapacity, RunnerId, RunnerMetadata,
    RunnerType, StepResult, VolumeMount,
};
use crate::error::{AppError, Result};

/// Kubernetes runner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesRunnerConfig {
    /// Runner name
    pub name: String,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Kubernetes namespace
    pub namespace: String,
    /// Default image for job pods
    pub default_image: String,
    /// Service account for job pods
    pub service_account: Option<String>,
    /// Resource limits for job pods
    pub resource_limits: ResourceLimits,
    /// Volume mounts for job pods
    pub volume_mounts: Vec<VolumeMount>,
    /// Default environment variables
    pub environment: HashMap<String, String>,
    /// Pod cleanup settings
    pub cleanup_settings: KubernetesCleanupSettings,
    /// Image pull policy
    pub image_pull_policy: String,
    /// Pod security context
    pub security_context: Option<KubernetesPodSecurityContext>,
    /// Node selector for pod scheduling
    pub node_selector: HashMap<String, String>,
    /// Tolerations for pod scheduling
    pub tolerations: Vec<KubernetesToleration>,
}

/// Kubernetes cleanup settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesCleanupSettings {
    /// Remove jobs after completion
    pub remove_jobs: bool,
    /// Remove pods after completion
    pub remove_pods: bool,
    /// Cleanup timeout in seconds
    pub cleanup_timeout_seconds: u64,
    /// TTL for completed jobs (seconds)
    pub ttl_seconds_after_finished: Option<i32>,
}

/// Pod security context configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesPodSecurityContext {
    /// Run as user ID
    pub run_as_user: Option<i64>,
    /// Run as group ID
    pub run_as_group: Option<i64>,
    /// Run as non-root
    pub run_as_non_root: Option<bool>,
    /// FS group
    pub fs_group: Option<i64>,
}

/// Kubernetes toleration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesToleration {
    /// Key
    pub key: Option<String>,
    /// Operator
    pub operator: Option<String>,
    /// Value
    pub value: Option<String>,
    /// Effect
    pub effect: Option<String>,
    /// Toleration seconds
    pub toleration_seconds: Option<i64>,
}

impl Default for KubernetesRunnerConfig {
    fn default() -> Self {
        Self {
            name: "kubernetes-runner".to_string(),
            max_concurrent_jobs: 10,
            namespace: "default".to_string(),
            default_image: "ubuntu:22.04".to_string(),
            service_account: None,
            resource_limits: ResourceLimits {
                cpu_limit: 1000,           // 1 CPU
                memory_limit: 512,         // 512MB
                storage_limit: Some(1024), // 1GB
            },
            volume_mounts: Vec::new(),
            environment: HashMap::new(),
            cleanup_settings: KubernetesCleanupSettings::default(),
            image_pull_policy: "IfNotPresent".to_string(),
            security_context: None,
            node_selector: HashMap::new(),
            tolerations: Vec::new(),
        }
    }
}

impl Default for KubernetesCleanupSettings {
    fn default() -> Self {
        Self {
            remove_jobs: true,
            remove_pods: true,
            cleanup_timeout_seconds: 60,
            ttl_seconds_after_finished: Some(300), // 5 minutes
        }
    }
}

/// Kubernetes runner implementation
pub struct KubernetesRunner {
    /// Runner ID
    id: RunnerId,
    /// Runner configuration
    config: KubernetesRunnerConfig,
    /// Kubernetes client
    client: Client,
    /// Jobs API
    jobs_api: Api<K8sJob>,
    /// Pods API
    pods_api: Api<Pod>,
    /// Currently running jobs
    running_jobs: Arc<RwLock<HashMap<JobId, RunningK8sJob>>>,
    /// Event demultiplexer
    event_demux: Arc<EventDemultiplexer>,
    /// Runner statistics
    stats: Arc<RwLock<KubernetesRunnerStats>>,
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

/// Information about a running Kubernetes job
#[derive(Debug, Clone)]
struct RunningK8sJob {
    job: Job,
    k8s_job_name: String,
    pod_name: Option<String>,
    started_at: DateTime<Utc>,
    current_step: usize,
    image: String,
}

/// Kubernetes runner statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KubernetesRunnerStats {
    pub total_jobs_created: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_execution_time: Duration,
    pub pods_created: u64,
    pub pods_cleaned_up: u64,
    pub current_load: f64,
    pub uptime: Duration,
    pub last_job_completed: Option<DateTime<Utc>>,
}

/// Kubernetes job completion event handler
pub struct KubernetesJobCompletionHandler {
    runner_id: RunnerId,
    running_jobs: Arc<RwLock<HashMap<JobId, RunningK8sJob>>>,
    stats: Arc<RwLock<KubernetesRunnerStats>>,
    jobs_api: Api<K8sJob>,
    pods_api: Api<Pod>,
    cleanup_settings: KubernetesCleanupSettings,
}

impl KubernetesRunner {
    /// Create a new Kubernetes runner
    pub async fn new(
        config: KubernetesRunnerConfig,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<Self> {
        let id = Uuid::new_v4();

        // Initialize Kubernetes client
        let client = Client::try_default().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create Kubernetes client: {}", e))
        })?;

        let jobs_api: Api<K8sJob> = Api::namespaced(client.clone(), &config.namespace);
        let pods_api: Api<Pod> = Api::namespaced(client.clone(), &config.namespace);

        let running_jobs = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(KubernetesRunnerStats::default()));

        // Register job completion handler
        let completion_handler = Arc::new(KubernetesJobCompletionHandler {
            runner_id: id,
            running_jobs: running_jobs.clone(),
            stats: stats.clone(),
            jobs_api: jobs_api.clone(),
            pods_api: pods_api.clone(),
            cleanup_settings: config.cleanup_settings.clone(),
        });

        event_demux.register_handler(completion_handler).await?;

        info!(
            "Created Kubernetes runner: {} in namespace: {}",
            config.name, config.namespace
        );

        Ok(Self {
            id,
            config,
            client,
            jobs_api,
            pods_api,
            running_jobs,
            event_demux,
            stats,
            shutdown: Arc::new(Mutex::new(false)),
        })
    }

    /// Check Kubernetes cluster connectivity
    pub async fn check_cluster_connectivity(&self) -> Result<bool> {
        match self.client.apiserver_version().await {
            Ok(version) => {
                debug!(
                    "Kubernetes cluster is accessible, version: {}",
                    version.git_version
                );
                Ok(true)
            }
            Err(e) => {
                warn!("Kubernetes cluster connectivity check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Create Kubernetes job for execution
    async fn create_k8s_job(&self, job: &Job) -> Result<String> {
        let job_name = format!("rustci-job-{}", job.id.to_string().replace('_', "-"));
        let image = job
            .metadata
            .get("image")
            .unwrap_or(&self.config.default_image)
            .clone();

        // Prepare environment variables
        let mut env_vars = Vec::new();
        for (key, value) in &self.config.environment {
            env_vars.push(EnvVar {
                name: key.clone(),
                value: Some(value.clone()),
                value_from: None,
            });
        }
        for (key, value) in &job.metadata {
            if let Some(env_key) = key.strip_prefix("env.") {
                // Remove "env." prefix
                env_vars.push(EnvVar {
                    name: env_key.to_string(),
                    value: Some(value.clone()),
                    value_from: None,
                });
            }
        }

        // Prepare volume mounts
        let mut volume_mounts = Vec::new();
        let mut volumes = Vec::new();

        for (i, mount) in self.config.volume_mounts.iter().enumerate() {
            let volume_name = format!("volume-{}", i);

            volume_mounts.push(K8sVolumeMount {
                name: volume_name.clone(),
                mount_path: mount.container_path.clone(),
                read_only: Some(mount.read_only),
                ..Default::default()
            });

            // For simplicity, create empty dir volumes
            // In production, you'd want to support different volume types
            volumes.push(Volume {
                name: volume_name,
                empty_dir: Some(EmptyDirVolumeSource::default()),
                ..Default::default()
            });
        }

        // Prepare resource requirements
        let mut resource_requests = BTreeMap::new();
        let mut resource_limits = BTreeMap::new();

        resource_requests.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", self.config.resource_limits.cpu_limit)),
        );
        resource_requests.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", self.config.resource_limits.memory_limit)),
        );

        resource_limits.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", self.config.resource_limits.cpu_limit)),
        );
        resource_limits.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", self.config.resource_limits.memory_limit)),
        );

        if let Some(storage_limit) = self.config.resource_limits.storage_limit {
            resource_limits.insert(
                "ephemeral-storage".to_string(),
                Quantity(format!("{}Mi", storage_limit)),
            );
        }

        // Build command for all job steps
        let mut commands = Vec::new();
        for step in &job.steps {
            let command = if step.args.is_empty() {
                step.command.clone()
            } else {
                format!("{} {}", step.command, step.args.join(" "))
            };
            commands.push(command);
        }

        let full_command = commands.join(" && ");

        // Create container spec
        let container = Container {
            name: "job-container".to_string(),
            image: Some(image.clone()),
            command: Some(vec!["sh".to_string(), "-c".to_string(), full_command]),
            env: Some(env_vars),
            volume_mounts: Some(volume_mounts),
            resources: Some(ResourceRequirements {
                requests: Some(resource_requests),
                limits: Some(resource_limits),
                ..Default::default()
            }),
            image_pull_policy: Some(self.config.image_pull_policy.clone()),
            security_context: self
                .config
                .security_context
                .as_ref()
                .map(|sc| SecurityContext {
                    run_as_user: sc.run_as_user,
                    run_as_group: sc.run_as_group,
                    run_as_non_root: sc.run_as_non_root,
                    ..Default::default()
                }),
            ..Default::default()
        };

        // Create pod template
        let pod_template = k8s_openapi::api::core::v1::PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(
                    [
                        ("app".to_string(), "rustci".to_string()),
                        ("job-id".to_string(), job.id.to_string()),
                        ("runner-id".to_string(), self.id.to_string()),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                ),
                ..Default::default()
            }),
            spec: Some(PodSpec {
                containers: vec![container],
                restart_policy: Some("Never".to_string()),
                service_account_name: self.config.service_account.clone(),
                volumes: Some(volumes),
                node_selector: if self.config.node_selector.is_empty() {
                    None
                } else {
                    Some(
                        self.config
                            .node_selector
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    )
                },
                tolerations: self
                    .config
                    .tolerations
                    .iter()
                    .map(|t| k8s_openapi::api::core::v1::Toleration {
                        key: t.key.clone(),
                        operator: t.operator.clone(),
                        value: t.value.clone(),
                        effect: t.effect.clone(),
                        toleration_seconds: t.toleration_seconds,
                    })
                    .collect::<Vec<_>>()
                    .into(),
                ..Default::default()
            }),
        };

        // Create job spec
        let job_spec = JobSpec {
            template: pod_template,
            backoff_limit: Some(0), // No retries at K8s level
            ttl_seconds_after_finished: self.config.cleanup_settings.ttl_seconds_after_finished,
            ..Default::default()
        };

        // Create Kubernetes job
        let k8s_job = K8sJob {
            metadata: ObjectMeta {
                name: Some(job_name.clone()),
                namespace: Some(self.config.namespace.clone()),
                labels: Some(
                    [
                        ("app".to_string(), "rustci".to_string()),
                        ("job-id".to_string(), job.id.to_string()),
                        ("runner-id".to_string(), self.id.to_string()),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                ),
                ..Default::default()
            },
            spec: Some(job_spec),
            ..Default::default()
        };

        // Submit job to Kubernetes
        self.jobs_api
            .create(&PostParams::default(), &k8s_job)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create Kubernetes job: {}", e))
            })?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_jobs_created += 1;
        }

        info!("Created Kubernetes job: {} for job: {}", job_name, job.id);
        Ok(job_name)
    }

    /// Wait for job completion and collect results
    async fn wait_for_job_completion(&self, job: Job, k8s_job_name: String) -> Result<JobResult> {
        let job_id = job.id;
        let mut result = JobResult::new(job_id);
        result.start();

        debug!("Waiting for Kubernetes job completion: {}", k8s_job_name);

        let timeout_duration = job.timeout;
        let wait_result = timeout(timeout_duration, async {
            // Simple polling approach for job completion
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;

                match self.jobs_api.get(&k8s_job_name).await {
                    Ok(k8s_job) => {
                        if let Some(status) = &k8s_job.status {
                            if let Some(conditions) = &status.conditions {
                                for condition in conditions {
                                    if condition.type_ == "Complete" && condition.status == "True" {
                                        return self
                                            .collect_job_results(&job, &k8s_job_name, true)
                                            .await;
                                    } else if condition.type_ == "Failed"
                                        && condition.status == "True"
                                    {
                                        return self
                                            .collect_job_results(&job, &k8s_job_name, false)
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get job status: {}", e);
                        break;
                    }
                }
            }

            Err(AppError::InternalServerError(
                "Job polling ended without completion".to_string(),
            ))
        })
        .await;

        match wait_result {
            Ok(job_result) => job_result,
            Err(_) => {
                result.complete(crate::domain::entities::JobStatus::TimedOut, Some(-1));
                result.error_message = Some(format!("Job timed out after {:?}", timeout_duration));
                Ok(result)
            }
        }
    }

    /// Collect job results from completed Kubernetes job
    async fn collect_job_results(
        &self,
        job: &Job,
        k8s_job_name: &str,
        success: bool,
    ) -> Result<JobResult> {
        let mut result = JobResult::new(job.id);
        result.start();

        // Find the pod associated with this job
        let pod_list = self
            .pods_api
            .list(&ListParams::default().labels(&format!("job-name={}", k8s_job_name)))
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to list pods for job: {}", e))
            })?;

        if let Some(pod) = pod_list.items.first() {
            if let Some(pod_name) = &pod.metadata.name {
                // Get pod logs
                let log_params = LogParams {
                    container: Some("job-container".to_string()),
                    ..Default::default()
                };

                match self.pods_api.logs(pod_name, &log_params).await {
                    Ok(logs) => {
                        result.stdout = logs;
                    }
                    Err(e) => {
                        warn!("Failed to get pod logs: {}", e);
                        result.stderr = format!("Failed to retrieve logs: {}", e);
                    }
                }

                // Update running job with pod name
                {
                    let mut running_jobs = self.running_jobs.write().await;
                    if let Some(running_job) = running_jobs.get_mut(&job.id) {
                        running_job.pod_name = Some(pod_name.clone());
                    }
                }
            }
        }

        // Create step results (simplified - treating entire job as one step)
        let step_result = StepResult {
            name: "kubernetes-job".to_string(),
            status: if success {
                crate::domain::entities::JobStatus::Success
            } else {
                crate::domain::entities::JobStatus::Failed
            },
            exit_code: if success { Some(0) } else { Some(1) },
            stdout: result.stdout.clone(),
            stderr: result.stderr.clone(),
            duration: Duration::from_secs(0), // Would calculate from pod status
            error_message: if success {
                None
            } else {
                Some("Job failed".to_string())
            },
        };

        result.step_results = vec![step_result];

        let final_status = if success {
            crate::domain::entities::JobStatus::Success
        } else {
            crate::domain::entities::JobStatus::Failed
        };
        let exit_code = if success { Some(0) } else { Some(1) };
        result.complete(final_status, exit_code);

        info!(
            "Collected results for Kubernetes job: {} (success: {})",
            k8s_job_name, success
        );
        Ok(result)
    }

    /// Clean up Kubernetes resources
    async fn cleanup_k8s_resources(
        &self,
        k8s_job_name: &str,
        pod_name: Option<&str>,
    ) -> Result<()> {
        let timeout_duration =
            Duration::from_secs(self.config.cleanup_settings.cleanup_timeout_seconds);

        let cleanup_result = timeout(timeout_duration, async {
            // Clean up job
            if self.config.cleanup_settings.remove_jobs {
                if let Err(e) = self
                    .jobs_api
                    .delete(k8s_job_name, &DeleteParams::default())
                    .await
                {
                    warn!("Failed to delete Kubernetes job {}: {}", k8s_job_name, e);
                }
            }

            // Clean up pod
            if self.config.cleanup_settings.remove_pods {
                if let Some(pod_name) = pod_name {
                    if let Err(e) = self
                        .pods_api
                        .delete(pod_name, &DeleteParams::default())
                        .await
                    {
                        warn!("Failed to delete pod {}: {}", pod_name, e);
                    }
                }
            }
        })
        .await;

        match cleanup_result {
            Ok(_) => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.pods_cleaned_up += 1;
                }
                debug!(
                    "Successfully cleaned up Kubernetes resources for job: {}",
                    k8s_job_name
                );
                Ok(())
            }
            Err(_) => {
                warn!(
                    "Kubernetes resource cleanup timed out for job: {}",
                    k8s_job_name
                );
                Ok(()) // Don't fail the job if cleanup times out
            }
        }
    }

    /// Get runner statistics
    pub async fn get_stats(&self) -> KubernetesRunnerStats {
        self.stats.read().await.clone()
    }

    /// Get currently running jobs
    pub async fn get_running_jobs(&self) -> Vec<RunningK8sJob> {
        let jobs = self.running_jobs.read().await;
        jobs.values().cloned().collect()
    }
}

#[async_trait]
impl crate::domain::entities::runner::Runner for KubernetesRunner {
    async fn execute(&self, job: Job) -> Result<JobResult> {
        let job_id = job.id;
        debug!("Starting Kubernetes execution for job: {}", job_id);

        // Create Kubernetes job
        let k8s_job_name = self.create_k8s_job(&job).await?;

        // Add to running jobs
        {
            let mut jobs = self.running_jobs.write().await;
            jobs.insert(
                job_id,
                RunningK8sJob {
                    job: job.clone(),
                    k8s_job_name: k8s_job_name.clone(),
                    pod_name: None,
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

        // Wait for completion
        let result = self
            .wait_for_job_completion(job, k8s_job_name.clone())
            .await;

        // Get pod name for cleanup
        let pod_name = {
            let jobs = self.running_jobs.read().await;
            jobs.get(&job_id).and_then(|j| j.pod_name.clone())
        };

        // Clean up resources
        if let Err(e) = self
            .cleanup_k8s_resources(&k8s_job_name, pod_name.as_deref())
            .await
        {
            warn!("Failed to cleanup Kubernetes resources: {}", e);
        }

        // Remove from running jobs
        {
            let mut jobs = self.running_jobs.write().await;
            jobs.remove(&job_id);
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
        // Check Kubernetes cluster connectivity
        if !self.check_cluster_connectivity().await? {
            return Ok(HealthStatus::Unhealthy {
                reason: "Kubernetes cluster is not accessible".to_string(),
            });
        }

        // Check current load
        let jobs = self.running_jobs.read().await;
        let load = jobs.len() as f64 / self.config.max_concurrent_jobs as f64;

        if load >= 1.0 {
            Ok(HealthStatus::Degraded {
                reason: "Runner at full capacity".to_string(),
            })
        } else {
            Ok(HealthStatus::Healthy)
        }
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        let jobs = self.running_jobs.read().await;
        let current_jobs = jobs.len() as u32;
        let max_jobs = self.config.max_concurrent_jobs;

        Ok(RunnerCapacity {
            max_concurrent_jobs: max_jobs,
            current_jobs,
            available_slots: max_jobs.saturating_sub(current_jobs),
            cpu_usage: 0.0,    // Would be implemented with Kubernetes metrics API
            memory_usage: 0.0, // Would be implemented with Kubernetes metrics API
            disk_usage: 0.0,   // Would be implemented with Kubernetes metrics API
        })
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Kubernetes runner: {}", self.config.name);

        // Set shutdown flag
        {
            let mut shutdown = self.shutdown.lock().await;
            *shutdown = true;
        }

        // Clean up all running jobs
        let jobs: Vec<_> = {
            let jobs = self.running_jobs.read().await;
            jobs.values().cloned().collect()
        };

        for job in jobs {
            if let Err(e) = self
                .cleanup_k8s_resources(&job.k8s_job_name, job.pod_name.as_deref())
                .await
            {
                warn!("Failed to cleanup job during shutdown: {}", e);
            }
        }

        // Clear running jobs
        {
            let mut jobs = self.running_jobs.write().await;
            jobs.clear();
        }

        info!("Kubernetes runner shutdown complete");
        Ok(())
    }

    fn get_metadata(&self) -> RunnerMetadata {
        RunnerMetadata {
            id: self.id,
            name: self.config.name.clone(),
            runner_type: RunnerType::Kubernetes {
                namespace: self.config.namespace.clone(),
                resource_limits: self.config.resource_limits.clone(),
                max_concurrent_jobs: self.config.max_concurrent_jobs,
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_job_types: vec![
                "shell".to_string(),
                "script".to_string(),
                "kubernetes".to_string(),
            ],
        }
    }

    async fn can_handle_job(&self, _job: &Job) -> Result<bool> {
        // Check if we have capacity
        let capacity = self.get_capacity().await?;
        if capacity.available_slots == 0 {
            return Ok(false);
        }

        // Check if Kubernetes cluster is accessible
        if !self.check_cluster_connectivity().await? {
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventHandler for KubernetesJobCompletionHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::JobCompletion { job_id, result } = event.payload {
            debug!("Handling Kubernetes job completion for: {}", job_id);

            // Get job info before removing
            let (k8s_job_name, pod_name) = {
                let jobs = self.running_jobs.read().await;
                jobs.get(&job_id)
                    .map(|j| (j.k8s_job_name.clone(), j.pod_name.clone()))
                    .unwrap_or_default()
            };

            // Clean up Kubernetes resources
            if !k8s_job_name.is_empty() {
                if let Err(e) = self
                    .cleanup_k8s_resources(&k8s_job_name, pod_name.as_deref())
                    .await
                {
                    warn!("Failed to cleanup Kubernetes resources: {}", e);
                }
            }

            // Remove from running jobs
            {
                let mut jobs = self.running_jobs.write().await;
                jobs.remove(&job_id);
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
                "Kubernetes job {} completed successfully: {}",
                job_id, result.success
            );
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::JobCompletion]
    }

    fn name(&self) -> &str {
        "kubernetes-job-completion-handler"
    }
}

impl KubernetesJobCompletionHandler {
    async fn cleanup_k8s_resources(
        &self,
        k8s_job_name: &str,
        pod_name: Option<&str>,
    ) -> Result<()> {
        let timeout_duration = Duration::from_secs(self.cleanup_settings.cleanup_timeout_seconds);

        let cleanup_result = timeout(timeout_duration, async {
            // Clean up job
            if self.cleanup_settings.remove_jobs {
                if let Err(e) = self
                    .jobs_api
                    .delete(k8s_job_name, &DeleteParams::default())
                    .await
                {
                    warn!("Failed to delete Kubernetes job {}: {}", k8s_job_name, e);
                }
            }

            // Clean up pod
            if self.cleanup_settings.remove_pods {
                if let Some(pod_name) = pod_name {
                    if let Err(e) = self
                        .pods_api
                        .delete(pod_name, &DeleteParams::default())
                        .await
                    {
                        warn!("Failed to delete pod {}: {}", pod_name, e);
                    }
                }
            }
        })
        .await;

        match cleanup_result {
            Ok(_) => {
                debug!(
                    "Successfully cleaned up Kubernetes resources for job: {}",
                    k8s_job_name
                );
                Ok::<(), crate::error::AppError>(())
            }
            Err(_) => {
                warn!(
                    "Kubernetes resource cleanup timed out for job: {}",
                    k8s_job_name
                );
                Ok::<(), crate::error::AppError>(()) // Don't fail if cleanup times out
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
            name: "test-k8s-job".to_string(),
            steps: vec![JobStep {
                name: "echo-step".to_string(),
                command: "echo".to_string(),
                args: vec!["hello from kubernetes".to_string()],
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
    async fn test_kubernetes_runner_config_defaults() {
        let config = KubernetesRunnerConfig::default();
        assert_eq!(config.name, "kubernetes-runner");
        assert_eq!(config.max_concurrent_jobs, 10);
        assert_eq!(config.namespace, "default");
        assert_eq!(config.default_image, "ubuntu:22.04");
        assert!(config.cleanup_settings.remove_jobs);
    }

    #[test]
    fn test_kubernetes_resource_limits() {
        let limits = ResourceLimits {
            cpu_limit: 2000,
            memory_limit: 1024,
            storage_limit: Some(2048),
        };

        assert_eq!(limits.cpu_limit, 2000);
        assert_eq!(limits.memory_limit, 1024);
        assert_eq!(limits.storage_limit, Some(2048));
    }

    #[test]
    fn test_kubernetes_cleanup_settings_default() {
        let settings = KubernetesCleanupSettings::default();
        assert!(settings.remove_jobs);
        assert!(settings.remove_pods);
        assert_eq!(settings.cleanup_timeout_seconds, 60);
        assert_eq!(settings.ttl_seconds_after_finished, Some(300));
    }

    #[test]
    fn test_kubernetes_toleration() {
        let toleration = KubernetesToleration {
            key: Some("node-type".to_string()),
            operator: Some("Equal".to_string()),
            value: Some("gpu".to_string()),
            effect: Some("NoSchedule".to_string()),
            toleration_seconds: Some(300),
        };

        assert_eq!(toleration.key, Some("node-type".to_string()));
        assert_eq!(toleration.effect, Some("NoSchedule".to_string()));
    }

    #[test]
    fn test_pod_security_context() {
        let security_context = KubernetesPodSecurityContext {
            run_as_user: Some(1000),
            run_as_group: Some(1000),
            run_as_non_root: Some(true),
            fs_group: Some(1000),
        };

        assert_eq!(security_context.run_as_user, Some(1000));
        assert_eq!(security_context.run_as_non_root, Some(true));
    }
}
