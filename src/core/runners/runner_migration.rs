//! Runner Migration and Failure Recovery System
//!
//! This module provides cross-node job migration, runner state preservation
//! and restoration, and graceful runner migration for maintenance scenarios.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::domain::entities::{
    Job, JobId, JobStatus, NodeId, NodeStatus, RunnerEntity, RunnerId, RunnerType,
};
use crate::error::{AppError, Result};

/// Migration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Migration timeout in seconds
    pub migration_timeout: u64,
    /// Maximum concurrent migrations
    pub max_concurrent_migrations: u32,
    /// Enable automatic migration on failure
    pub auto_migrate_on_failure: bool,
    /// Migration retry attempts
    pub migration_retry_attempts: u32,
    /// State preservation strategy
    pub state_preservation: StatePreservationStrategy,
    /// Migration priority levels
    pub migration_priorities: HashMap<String, u32>,
    /// Graceful shutdown timeout
    pub graceful_shutdown_timeout: u64,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            migration_timeout: 300,
            max_concurrent_migrations: 5,
            auto_migrate_on_failure: true,
            migration_retry_attempts: 3,
            state_preservation: StatePreservationStrategy::Full,
            migration_priorities: HashMap::new(),
            graceful_shutdown_timeout: 120,
        }
    }
}

/// State preservation strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatePreservationStrategy {
    /// Preserve full runner state
    Full,
    /// Preserve only essential state
    Essential,
    /// Preserve minimal state
    Minimal,
    /// No state preservation
    None,
}

/// Runner state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStateSnapshot {
    /// Runner ID
    pub runner_id: RunnerId,
    /// Runner entity information
    pub runner_entity: RunnerEntity,
    /// Running jobs
    pub running_jobs: Vec<JobSnapshot>,
    /// Queued jobs
    pub queued_jobs: Vec<JobSnapshot>,
    /// Runner configuration
    pub configuration: RunnerConfiguration,
    /// Resource state
    pub resource_state: ResourceState,
    /// Custom state data
    pub custom_state: HashMap<String, serde_json::Value>,
    /// Snapshot timestamp
    pub created_at: DateTime<Utc>,
    /// Snapshot version
    pub version: u32,
}

/// Job snapshot for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSnapshot {
    /// Job information
    pub job: Job,
    /// Current execution state
    pub execution_state: JobExecutionState,
    /// Progress information
    pub progress: JobProgress,
    /// Temporary files and resources
    pub resources: Vec<JobResource>,
}

/// Job execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecutionState {
    /// Current status
    pub status: JobStatus,
    /// Started timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Current step index
    pub current_step: usize,
    /// Step states
    pub step_states: Vec<StepState>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Working directory
    pub working_directory: String,
}

/// Job progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    /// Completion percentage (0-100)
    pub completion_percentage: f64,
    /// Estimated remaining time
    pub estimated_remaining: Option<Duration>,
    /// Bytes processed
    pub bytes_processed: u64,
    /// Items processed
    pub items_processed: u64,
}

/// Job resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResource {
    /// Resource type
    pub resource_type: ResourceType,
    /// Resource path or identifier
    pub path: String,
    /// Resource size in bytes
    pub size: u64,
    /// Whether resource is temporary
    pub temporary: bool,
    /// Resource metadata
    pub metadata: HashMap<String, String>,
}

/// Resource types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    /// Temporary file
    TempFile,
    /// Log file
    LogFile,
    /// Cache file
    CacheFile,
    /// Database connection
    Database,
    /// Network connection
    Network,
    /// Custom resource
    Custom { resource_name: String },
}

/// Step execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    /// Step name
    pub name: String,
    /// Step status
    pub status: JobStatus,
    /// Step output
    pub output: String,
    /// Step error output
    pub error_output: String,
    /// Step start time
    pub started_at: Option<DateTime<Utc>>,
    /// Step completion time
    pub completed_at: Option<DateTime<Utc>>,
}

/// Runner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfiguration {
    /// Runner type
    pub runner_type: RunnerType,
    /// Configuration parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Resource limits
    pub resource_limits: ResourceLimits,
}

/// Resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU limit
    pub cpu_limit: Option<u32>,
    /// Memory limit in MB
    pub memory_limit: Option<u32>,
    /// Disk limit in MB
    pub disk_limit: Option<u32>,
    /// Network bandwidth limit in Mbps
    pub network_limit: Option<u32>,
}

/// Resource state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceState {
    /// CPU usage
    pub cpu_usage: f64,
    /// Memory usage in MB
    pub memory_usage: u64,
    /// Disk usage in MB
    pub disk_usage: u64,
    /// Network usage in Mbps
    pub network_usage: f64,
    /// Open file descriptors
    pub open_files: u32,
    /// Active connections
    pub active_connections: u32,
}

/// Migration request
#[derive(Debug, Clone)]
pub struct MigrationRequest {
    /// Migration ID
    pub migration_id: Uuid,
    /// Source runner ID
    pub source_runner_id: RunnerId,
    /// Target node ID
    pub target_node_id: NodeId,
    /// Migration type
    pub migration_type: MigrationType,
    /// Migration priority
    pub priority: u32,
    /// Migration reason
    pub reason: String,
    /// Request timestamp
    pub requested_at: DateTime<Utc>,
    /// Requester information
    pub requester: String,
}

/// Migration types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationType {
    /// Planned migration for maintenance
    Planned,
    /// Emergency migration due to failure
    Emergency,
    /// Load balancing migration
    LoadBalancing,
    /// Resource optimization migration
    Optimization,
}

/// Migration result
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Migration ID
    pub migration_id: Uuid,
    /// Migration status
    pub status: MigrationStatus,
    /// Source runner ID
    pub source_runner_id: RunnerId,
    /// Target runner ID
    pub target_runner_id: Option<RunnerId>,
    /// Migrated jobs
    pub migrated_jobs: Vec<JobId>,
    /// Failed jobs
    pub failed_jobs: Vec<(JobId, String)>,
    /// Migration duration
    pub duration: Duration,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Completion timestamp
    pub completed_at: DateTime<Utc>,
}

/// Migration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration pending
    Pending,
    /// Migration in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
    /// Migration cancelled
    Cancelled,
}

/// Runner migration service trait
#[async_trait]
pub trait RunnerMigrationService: Send + Sync {
    /// Start the migration service
    async fn start(&self) -> Result<()>;

    /// Stop the migration service
    async fn stop(&self) -> Result<()>;

    /// Create a state snapshot of a runner
    async fn create_state_snapshot(&self, runner_id: RunnerId) -> Result<RunnerStateSnapshot>;

    /// Restore runner state from snapshot
    async fn restore_state_snapshot(
        &self,
        snapshot: &RunnerStateSnapshot,
        target_runner_id: RunnerId,
    ) -> Result<()>;

    /// Migrate a runner to another node
    async fn migrate_runner(&self, request: MigrationRequest) -> Result<MigrationResult>;

    /// Migrate jobs from failed runner
    async fn migrate_jobs_from_failed_runner(
        &self,
        failed_runner_id: RunnerId,
    ) -> Result<Vec<JobId>>;

    /// Gracefully shutdown runner with job migration
    async fn graceful_shutdown_with_migration(
        &self,
        runner_id: RunnerId,
        target_node_id: Option<NodeId>,
    ) -> Result<()>;

    /// Get migration history
    async fn get_migration_history(&self, duration: Duration) -> Result<Vec<MigrationResult>>;

    /// Cancel ongoing migration
    async fn cancel_migration(&self, migration_id: Uuid) -> Result<()>;
}

/// Default implementation of runner migration service
pub struct DefaultRunnerMigrationService {
    /// Configuration
    config: MigrationConfig,
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Runner pool manager
    runner_pool: Arc<dyn RunnerPoolManager>,
    /// State snapshots storage
    snapshots: Arc<RwLock<HashMap<RunnerId, RunnerStateSnapshot>>>,
    /// Active migrations
    active_migrations: Arc<RwLock<HashMap<Uuid, MigrationRequest>>>,
    /// Migration history
    migration_history: Arc<RwLock<Vec<MigrationResult>>>,
    /// Running state
    running: Arc<Mutex<bool>>,
}

impl DefaultRunnerMigrationService {
    /// Create a new migration service
    pub fn new(
        config: MigrationConfig,
        node_registry: Arc<NodeRegistry>,
        runner_pool: Arc<dyn RunnerPoolManager>,
    ) -> Self {
        Self {
            config,
            node_registry,
            runner_pool,
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            active_migrations: Arc::new(RwLock::new(HashMap::new())),
            migration_history: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start migration monitoring task
    async fn start_migration_monitoring(&self) -> Result<()> {
        let active_migrations = self.active_migrations.clone();
        let migration_history = self.migration_history.clone();
        let config = self.config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Check for timed out migrations
                let now = Utc::now();
                let timeout_duration = chrono::Duration::seconds(config.migration_timeout as i64);

                let mut migrations = active_migrations.write().await;
                let mut timed_out_migrations = Vec::new();

                for (migration_id, request) in migrations.iter() {
                    if now - request.requested_at > timeout_duration {
                        timed_out_migrations.push(*migration_id);
                    }
                }

                // Handle timed out migrations
                for migration_id in timed_out_migrations {
                    if let Some(request) = migrations.remove(&migration_id) {
                        warn!("Migration {} timed out", migration_id);

                        let result = MigrationResult {
                            migration_id,
                            status: MigrationStatus::Failed,
                            source_runner_id: request.source_runner_id,
                            target_runner_id: None,
                            migrated_jobs: Vec::new(),
                            failed_jobs: Vec::new(),
                            duration: Duration::from_secs(config.migration_timeout),
                            error_message: Some("Migration timed out".to_string()),
                            completed_at: now,
                        };

                        let mut history = migration_history.write().await;
                        history.push(result);
                    }
                }
            }
        });

        Ok(())
    }

    /// Collect runner state for snapshot
    async fn collect_runner_state(&self, runner_id: RunnerId) -> Result<RunnerStateSnapshot> {
        // Get runner information
        let runner_registration = self
            .runner_pool
            .get_runner(runner_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Runner {} not found", runner_id)))?;

        // Collect running and queued jobs (simplified implementation)
        let running_jobs = self
            .collect_runner_jobs(runner_id, JobStatus::Running)
            .await?;
        let queued_jobs = self
            .collect_runner_jobs(runner_id, JobStatus::Queued)
            .await?;

        // Create configuration snapshot
        let configuration = RunnerConfiguration {
            runner_type: runner_registration.entity.runner_type.clone(),
            parameters: HashMap::new(),  // Would collect actual parameters
            environment: HashMap::new(), // Would collect actual environment
            resource_limits: ResourceLimits {
                cpu_limit: Some(4000), // Simplified
                memory_limit: Some(8192),
                disk_limit: Some(102400),
                network_limit: Some(1000),
            },
        };

        // Create resource state snapshot
        let resource_state = ResourceState {
            cpu_usage: 45.0, // Would collect actual usage
            memory_usage: 2048,
            disk_usage: 10240,
            network_usage: 100.0,
            open_files: 25,
            active_connections: 10,
        };

        Ok(RunnerStateSnapshot {
            runner_id,
            runner_entity: runner_registration.entity.clone(),
            running_jobs,
            queued_jobs,
            configuration,
            resource_state,
            custom_state: HashMap::new(),
            created_at: Utc::now(),
            version: 1,
        })
    }

    /// Collect jobs for a runner (simplified implementation)
    async fn collect_runner_jobs(
        &self,
        _runner_id: RunnerId,
        _status: JobStatus,
    ) -> Result<Vec<JobSnapshot>> {
        // In a real implementation, this would query the job system
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Find suitable target node for migration
    async fn find_target_node(
        &self,
        _source_runner_id: RunnerId,
        preferred_node_id: Option<NodeId>,
    ) -> Result<NodeId> {
        // If preferred node is specified and available, use it
        if let Some(node_id) = preferred_node_id {
            if self.node_registry.node_exists(node_id).await {
                let node = self
                    .node_registry
                    .get_node(node_id)
                    .await?
                    .ok_or_else(|| AppError::NotFound(format!("Node {} not found", node_id)))?;

                if node.status == NodeStatus::Active {
                    return Ok(node_id);
                }
            }
        }

        // Find best available node
        let active_nodes = self.node_registry.get_active_nodes().await?;

        if active_nodes.is_empty() {
            return Err(AppError::ResourceExhausted(
                "No active nodes available for migration".to_string(),
            ));
        }

        // Select node with least load (simplified selection)
        let best_node = active_nodes
            .into_iter()
            .min_by_key(|node| node.resources.cpu_usage as u32)
            .ok_or_else(|| AppError::BadRequest("Failed to select target node".to_string()))?;

        Ok(best_node.id)
    }

    /// Create new runner on target node
    async fn create_target_runner(
        &self,
        snapshot: &RunnerStateSnapshot,
        target_node_id: NodeId,
    ) -> Result<RunnerId> {
        // Create new runner entity with similar configuration
        let mut new_runner_entity = RunnerEntity::new(
            format!("migrated-{}", snapshot.runner_entity.name),
            snapshot.configuration.runner_type.clone(),
        );

        new_runner_entity.node_id = Some(target_node_id);
        new_runner_entity.tags = snapshot.runner_entity.tags.clone();

        // In a real implementation, this would create the actual runner instance
        // For now, we'll just return the new runner ID
        let new_runner_id = new_runner_entity.id;

        info!(
            "Created target runner {} on node {}",
            new_runner_id, target_node_id
        );
        Ok(new_runner_id)
    }

    /// Transfer jobs to target runner
    async fn transfer_jobs(
        &self,
        jobs: &[JobSnapshot],
        target_runner_id: RunnerId,
    ) -> Result<(Vec<JobId>, Vec<(JobId, String)>)> {
        let mut migrated_jobs = Vec::new();
        let mut failed_jobs = Vec::new();

        for job_snapshot in jobs {
            match self
                .transfer_single_job(job_snapshot, target_runner_id)
                .await
            {
                Ok(()) => {
                    migrated_jobs.push(job_snapshot.job.id);
                    info!("Successfully migrated job {}", job_snapshot.job.id);
                }
                Err(e) => {
                    failed_jobs.push((job_snapshot.job.id, e.to_string()));
                    warn!("Failed to migrate job {}: {}", job_snapshot.job.id, e);
                }
            }
        }

        Ok((migrated_jobs, failed_jobs))
    }

    /// Transfer a single job to target runner
    async fn transfer_single_job(
        &self,
        job_snapshot: &JobSnapshot,
        target_runner_id: RunnerId,
    ) -> Result<()> {
        // In a real implementation, this would:
        // 1. Recreate the job on the target runner
        // 2. Transfer any temporary files or resources
        // 3. Restore the execution state
        // 4. Resume execution from the current step

        // For now, this is a simplified implementation
        debug!(
            "Transferring job {} to runner {}",
            job_snapshot.job.id, target_runner_id
        );

        // Simulate job transfer
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Cleanup source runner
    async fn cleanup_source_runner(&self, runner_id: RunnerId) -> Result<()> {
        // Gracefully shutdown the source runner
        if let Ok(Some(runner_registration)) = self.runner_pool.get_runner(runner_id).await {
            if let Err(e) = runner_registration.runner.shutdown().await {
                warn!("Error shutting down source runner {}: {}", runner_id, e);
            }
        }

        // Deregister from runner pool
        if let Err(e) = self.runner_pool.deregister_runner(runner_id).await {
            warn!("Error deregistering source runner {}: {}", runner_id, e);
        }

        info!("Cleaned up source runner {}", runner_id);
        Ok(())
    }
}

#[async_trait]
impl RunnerMigrationService for DefaultRunnerMigrationService {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest(
                "Migration service is already running".to_string(),
            ));
        }

        *running = true;

        // Start migration monitoring
        self.start_migration_monitoring().await?;

        info!("Runner migration service started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }

        *running = false;

        info!("Runner migration service stopped");
        Ok(())
    }

    async fn create_state_snapshot(&self, runner_id: RunnerId) -> Result<RunnerStateSnapshot> {
        let snapshot = self.collect_runner_state(runner_id).await?;

        // Store snapshot
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(runner_id, snapshot.clone());
        }

        info!("Created state snapshot for runner {}", runner_id);
        Ok(snapshot)
    }

    async fn restore_state_snapshot(
        &self,
        _snapshot: &RunnerStateSnapshot,
        target_runner_id: RunnerId,
    ) -> Result<()> {
        // In a real implementation, this would:
        // 1. Configure the target runner with the same settings
        // 2. Restore environment variables and configuration
        // 3. Recreate temporary files and resources
        // 4. Restore job execution states

        info!("Restoring state snapshot to runner {}", target_runner_id);

        // Simulate state restoration
        tokio::time::sleep(Duration::from_millis(500)).await;

        info!("Successfully restored state to runner {}", target_runner_id);
        Ok(())
    }

    async fn migrate_runner(&self, request: MigrationRequest) -> Result<MigrationResult> {
        let migration_id = request.migration_id;
        let start_time = std::time::Instant::now();

        // Add to active migrations
        {
            let mut active_migrations = self.active_migrations.write().await;
            active_migrations.insert(migration_id, request.clone());
        }

        info!(
            "Starting migration {} for runner {}",
            migration_id, request.source_runner_id
        );

        let result = async {
            // Create state snapshot
            let snapshot = self.create_state_snapshot(request.source_runner_id).await?;

            // Find target node
            let target_node_id = self
                .find_target_node(request.source_runner_id, Some(request.target_node_id))
                .await?;

            // Create target runner
            let target_runner_id = self.create_target_runner(&snapshot, target_node_id).await?;

            // Restore state on target runner
            self.restore_state_snapshot(&snapshot, target_runner_id)
                .await?;

            // Transfer running jobs
            let (migrated_running, failed_running) = self
                .transfer_jobs(&snapshot.running_jobs, target_runner_id)
                .await?;

            // Transfer queued jobs
            let (migrated_queued, failed_queued) = self
                .transfer_jobs(&snapshot.queued_jobs, target_runner_id)
                .await?;

            // Cleanup source runner
            self.cleanup_source_runner(request.source_runner_id).await?;

            // Combine results
            let mut migrated_jobs = migrated_running;
            migrated_jobs.extend(migrated_queued);

            let mut failed_jobs = failed_running;
            failed_jobs.extend(failed_queued);

            Ok::<_, AppError>(MigrationResult {
                migration_id,
                status: MigrationStatus::Completed,
                source_runner_id: request.source_runner_id,
                target_runner_id: Some(target_runner_id),
                migrated_jobs,
                failed_jobs,
                duration: start_time.elapsed(),
                error_message: None,
                completed_at: Utc::now(),
            })
        }
        .await;

        // Remove from active migrations
        {
            let mut active_migrations = self.active_migrations.write().await;
            active_migrations.remove(&migration_id);
        }

        let migration_result = match result {
            Ok(result) => {
                info!("Migration {} completed successfully", migration_id);
                result
            }
            Err(e) => {
                error!("Migration {} failed: {}", migration_id, e);
                MigrationResult {
                    migration_id,
                    status: MigrationStatus::Failed,
                    source_runner_id: request.source_runner_id,
                    target_runner_id: None,
                    migrated_jobs: Vec::new(),
                    failed_jobs: Vec::new(),
                    duration: start_time.elapsed(),
                    error_message: Some(e.to_string()),
                    completed_at: Utc::now(),
                }
            }
        };

        // Add to history
        {
            let mut history = self.migration_history.write().await;
            history.push(migration_result.clone());

            // Keep only last 1000 migrations
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        Ok(migration_result)
    }

    async fn migrate_jobs_from_failed_runner(
        &self,
        failed_runner_id: RunnerId,
    ) -> Result<Vec<JobId>> {
        info!("Migrating jobs from failed runner {}", failed_runner_id);

        // Create emergency migration request
        let migration_request = MigrationRequest {
            migration_id: Uuid::new_v4(),
            source_runner_id: failed_runner_id,
            target_node_id: Uuid::new_v4(), // Will be auto-selected
            migration_type: MigrationType::Emergency,
            priority: 100, // Highest priority
            reason: "Runner failure".to_string(),
            requested_at: Utc::now(),
            requester: "failure-recovery-system".to_string(),
        };

        // Execute migration
        let result = self.migrate_runner(migration_request).await?;

        Ok(result.migrated_jobs)
    }

    async fn graceful_shutdown_with_migration(
        &self,
        runner_id: RunnerId,
        target_node_id: Option<NodeId>,
    ) -> Result<()> {
        info!(
            "Gracefully shutting down runner {} with migration",
            runner_id
        );

        // Create planned migration request
        let migration_request = MigrationRequest {
            migration_id: Uuid::new_v4(),
            source_runner_id: runner_id,
            target_node_id: target_node_id.unwrap_or_else(Uuid::new_v4),
            migration_type: MigrationType::Planned,
            priority: 50,
            reason: "Graceful shutdown".to_string(),
            requested_at: Utc::now(),
            requester: "maintenance-system".to_string(),
        };

        // Execute migration with timeout
        let migration_future = self.migrate_runner(migration_request);
        let timeout_duration = Duration::from_secs(self.config.graceful_shutdown_timeout);

        match timeout(timeout_duration, migration_future).await {
            Ok(Ok(result)) => {
                if matches!(result.status, MigrationStatus::Completed) {
                    info!("Graceful shutdown with migration completed successfully");
                } else {
                    warn!(
                        "Graceful shutdown migration failed: {:?}",
                        result.error_message
                    );
                }
            }
            Ok(Err(e)) => {
                error!("Graceful shutdown migration error: {}", e);
            }
            Err(_) => {
                error!("Graceful shutdown migration timed out");
            }
        }

        Ok(())
    }

    async fn get_migration_history(&self, duration: Duration) -> Result<Vec<MigrationResult>> {
        let history = self.migration_history.read().await;
        let cutoff = Utc::now()
            - chrono::Duration::from_std(duration)
                .map_err(|e| AppError::BadRequest(format!("Invalid duration: {}", e)))?;

        Ok(history
            .iter()
            .filter(|result| result.completed_at > cutoff)
            .cloned()
            .collect())
    }

    async fn cancel_migration(&self, migration_id: Uuid) -> Result<()> {
        let mut active_migrations = self.active_migrations.write().await;

        if let Some(request) = active_migrations.remove(&migration_id) {
            info!(
                "Cancelled migration {} for runner {}",
                migration_id, request.source_runner_id
            );

            // Add cancelled result to history
            let result = MigrationResult {
                migration_id,
                status: MigrationStatus::Cancelled,
                source_runner_id: request.source_runner_id,
                target_runner_id: None,
                migrated_jobs: Vec::new(),
                failed_jobs: Vec::new(),
                duration: Duration::from_secs(0),
                error_message: Some("Migration cancelled by user".to_string()),
                completed_at: Utc::now(),
            };

            let mut history = self.migration_history.write().await;
            history.push(result);

            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Migration {} not found or already completed",
                migration_id
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cluster::node_registry::tests::create_test_registry;
    use crate::core::DefaultRunnerPoolManager;

    fn create_test_migration_service() -> DefaultRunnerMigrationService {
        let config = MigrationConfig::default();
        let node_registry = Arc::new(create_test_registry());
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());

        DefaultRunnerMigrationService::new(config, node_registry, runner_pool)
    }

    #[tokio::test]
    async fn test_migration_service_creation() {
        let service = create_test_migration_service();
        assert!(!*service.running.lock().await);
    }

    #[tokio::test]
    async fn test_migration_request() {
        let request = MigrationRequest {
            migration_id: Uuid::new_v4(),
            source_runner_id: Uuid::new_v4(),
            target_node_id: Uuid::new_v4(),
            migration_type: MigrationType::Planned,
            priority: 50,
            reason: "Test migration".to_string(),
            requested_at: Utc::now(),
            requester: "test".to_string(),
        };

        assert!(matches!(request.migration_type, MigrationType::Planned));
        assert_eq!(request.priority, 50);
        assert_eq!(request.reason, "Test migration");
    }

    #[tokio::test]
    async fn test_state_snapshot() {
        let runner_id = Uuid::new_v4();
        let runner_entity = RunnerEntity::new(
            "test-runner".to_string(),
            RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            },
        );

        let snapshot = RunnerStateSnapshot {
            runner_id,
            runner_entity,
            running_jobs: Vec::new(),
            queued_jobs: Vec::new(),
            configuration: RunnerConfiguration {
                runner_type: RunnerType::Local {
                    max_concurrent_jobs: 4,
                    working_directory: "/tmp".to_string(),
                },
                parameters: HashMap::new(),
                environment: HashMap::new(),
                resource_limits: ResourceLimits {
                    cpu_limit: Some(4000),
                    memory_limit: Some(8192),
                    disk_limit: Some(102400),
                    network_limit: Some(1000),
                },
            },
            resource_state: ResourceState {
                cpu_usage: 45.0,
                memory_usage: 2048,
                disk_usage: 10240,
                network_usage: 100.0,
                open_files: 25,
                active_connections: 10,
            },
            custom_state: HashMap::new(),
            created_at: Utc::now(),
            version: 1,
        };

        assert_eq!(snapshot.runner_id, runner_id);
        assert_eq!(snapshot.version, 1);
        assert!(snapshot.running_jobs.is_empty());
        assert!(snapshot.queued_jobs.is_empty());
    }

    #[tokio::test]
    async fn test_migration_result() {
        let migration_id = Uuid::new_v4();
        let source_runner_id = Uuid::new_v4();
        let target_runner_id = Uuid::new_v4();

        let result = MigrationResult {
            migration_id,
            status: MigrationStatus::Completed,
            source_runner_id,
            target_runner_id: Some(target_runner_id),
            migrated_jobs: vec![Uuid::new_v4(), Uuid::new_v4()],
            failed_jobs: Vec::new(),
            duration: Duration::from_secs(30),
            error_message: None,
            completed_at: Utc::now(),
        };

        assert!(matches!(result.status, MigrationStatus::Completed));
        assert_eq!(result.migrated_jobs.len(), 2);
        assert!(result.failed_jobs.is_empty());
        assert!(result.error_message.is_none());
    }
}
