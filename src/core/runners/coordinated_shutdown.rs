//! Coordinated Shutdown and Cleanup System
//!
//! This module provides enhanced graceful shutdown for cluster coordination,
//! job completion waiting, resource cleanup across nodes, and cluster-wide
//! runner lifecycle event propagation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::runners::runner_migration::RunnerMigrationService;
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::domain::entities::{NodeId, RunnerId};
use crate::error::{AppError, Result};

/// Coordinated shutdown configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatedShutdownConfig {
    /// Maximum time to wait for jobs to complete
    pub job_completion_timeout: u64,
    /// Maximum time to wait for resource cleanup
    pub cleanup_timeout: u64,
    /// Shutdown phases and their timeouts
    pub shutdown_phases: Vec<ShutdownPhase>,
    /// Enable job migration during shutdown
    pub enable_job_migration: bool,
    /// Maximum concurrent shutdowns
    pub max_concurrent_shutdowns: u32,
    /// Event propagation timeout
    pub event_propagation_timeout: u64,
    /// Cleanup strategies
    pub cleanup_strategies: HashMap<String, CleanupStrategy>,
}

impl Default for CoordinatedShutdownConfig {
    fn default() -> Self {
        Self {
            job_completion_timeout: 300,
            cleanup_timeout: 120,
            shutdown_phases: vec![
                ShutdownPhase::new("prepare", 30, "Prepare for shutdown"),
                ShutdownPhase::new("drain", 180, "Drain running jobs"),
                ShutdownPhase::new("migrate", 120, "Migrate remaining jobs"),
                ShutdownPhase::new("cleanup", 60, "Cleanup resources"),
                ShutdownPhase::new("finalize", 30, "Finalize shutdown"),
            ],
            enable_job_migration: true,
            max_concurrent_shutdowns: 3,
            event_propagation_timeout: 30,
            cleanup_strategies: HashMap::new(),
        }
    }
}

/// Shutdown phase definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownPhase {
    /// Phase name
    pub name: String,
    /// Phase timeout in seconds
    pub timeout: u64,
    /// Phase description
    pub description: String,
    /// Phase dependencies (must complete before this phase)
    pub dependencies: Vec<String>,
    /// Phase actions
    pub actions: Vec<ShutdownAction>,
}

impl ShutdownPhase {
    /// Create a new shutdown phase
    pub fn new(name: &str, timeout: u64, description: &str) -> Self {
        Self {
            name: name.to_string(),
            timeout,
            description: description.to_string(),
            dependencies: Vec::new(),
            actions: Vec::new(),
        }
    }
}

/// Shutdown actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShutdownAction {
    /// Stop accepting new jobs
    StopAcceptingJobs,
    /// Wait for running jobs to complete
    WaitForJobCompletion,
    /// Migrate jobs to other runners
    MigrateJobs,
    /// Cleanup temporary resources
    CleanupResources,
    /// Send shutdown notifications
    SendNotifications,
    /// Custom action
    Custom {
        name: String,
        parameters: HashMap<String, String>,
    },
}

/// Cleanup strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupStrategy {
    /// Delete all temporary files
    DeleteTempFiles,
    /// Archive logs before cleanup
    ArchiveLogs,
    /// Graceful resource release
    GracefulRelease,
    /// Force cleanup
    ForceCleanup,
    /// Custom cleanup
    Custom { script: String },
}

/// Shutdown request
#[derive(Debug, Clone)]
pub struct ShutdownRequest {
    /// Shutdown ID
    pub shutdown_id: Uuid,
    /// Target type
    pub target: ShutdownTarget,
    /// Shutdown type
    pub shutdown_type: ShutdownType,
    /// Shutdown reason
    pub reason: String,
    /// Requested by
    pub requested_by: String,
    /// Request timestamp
    pub requested_at: DateTime<Utc>,
    /// Shutdown timeout
    pub timeout: Duration,
    /// Force shutdown if timeout exceeded
    pub force_on_timeout: bool,
}

/// Shutdown targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShutdownTarget {
    /// Single runner
    Runner { runner_id: RunnerId },
    /// Single node
    Node { node_id: NodeId },
    /// Multiple runners
    Runners { runner_ids: Vec<RunnerId> },
    /// Multiple nodes
    Nodes { node_ids: Vec<NodeId> },
    /// Entire cluster
    Cluster,
}

/// Shutdown types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShutdownType {
    /// Graceful shutdown with job completion
    Graceful,
    /// Fast shutdown with job migration
    Fast,
    /// Immediate shutdown (force)
    Immediate,
    /// Maintenance shutdown
    Maintenance,
}

/// Shutdown status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownStatus {
    /// Shutdown ID
    pub shutdown_id: Uuid,
    /// Current phase
    pub current_phase: String,
    /// Phase progress (0.0 to 1.0)
    pub phase_progress: f64,
    /// Overall progress (0.0 to 1.0)
    pub overall_progress: f64,
    /// Status
    pub status: ShutdownState,
    /// Running jobs count
    pub running_jobs: u32,
    /// Completed jobs count
    pub completed_jobs: u32,
    /// Migrated jobs count
    pub migrated_jobs: u32,
    /// Failed jobs count
    pub failed_jobs: u32,
    /// Resources cleaned up
    pub resources_cleaned: u32,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Error messages
    pub errors: Vec<String>,
}

/// Shutdown states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShutdownState {
    /// Shutdown requested
    Requested,
    /// Shutdown in progress
    InProgress,
    /// Shutdown completed successfully
    Completed,
    /// Shutdown failed
    Failed,
    /// Shutdown cancelled
    Cancelled,
    /// Shutdown timed out
    TimedOut,
}

/// Lifecycle event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LifecycleEvent {
    /// Runner started
    RunnerStarted {
        runner_id: RunnerId,
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// Runner stopped
    RunnerStopped {
        runner_id: RunnerId,
        node_id: NodeId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Runner failed
    RunnerFailed {
        runner_id: RunnerId,
        node_id: NodeId,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Node joined cluster
    NodeJoined {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// Node left cluster
    NodeLeft {
        node_id: NodeId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Shutdown initiated
    ShutdownInitiated {
        shutdown_id: Uuid,
        target: ShutdownTarget,
        timestamp: DateTime<Utc>,
    },
    /// Shutdown completed
    ShutdownCompleted {
        shutdown_id: Uuid,
        target: ShutdownTarget,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },
}

/// Resource cleanup information
#[derive(Debug, Clone)]
pub struct ResourceCleanup {
    /// Resource type
    pub resource_type: ResourceType,
    /// Resource identifier
    pub resource_id: String,
    /// Cleanup strategy
    pub strategy: CleanupStrategy,
    /// Cleanup priority (higher = more important)
    pub priority: u32,
    /// Estimated cleanup time
    pub estimated_time: Duration,
}

/// Resource types for cleanup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    /// Temporary files
    TempFiles { path: String },
    /// Log files
    LogFiles { path: String },
    /// Database connections
    DatabaseConnections { connection_id: String },
    /// Network connections
    NetworkConnections { connection_id: String },
    /// Memory allocations
    MemoryAllocations { allocation_id: String },
    /// Process handles
    ProcessHandles { process_id: u32 },
    /// Custom resource
    Custom { resource_name: String },
}

/// Coordinated shutdown service trait
#[async_trait]
pub trait CoordinatedShutdownService: Send + Sync {
    /// Start the shutdown service
    async fn start(&self) -> Result<()>;

    /// Stop the shutdown service
    async fn stop(&self) -> Result<()>;

    /// Request coordinated shutdown
    async fn request_shutdown(&self, request: ShutdownRequest) -> Result<Uuid>;

    /// Get shutdown status
    async fn get_shutdown_status(&self, shutdown_id: Uuid) -> Result<ShutdownStatus>;

    /// Cancel ongoing shutdown
    async fn cancel_shutdown(&self, shutdown_id: Uuid) -> Result<()>;

    /// Register lifecycle event listener
    async fn register_event_listener(
        &self,
        listener: Box<dyn LifecycleEventListener>,
    ) -> Result<()>;

    /// Propagate lifecycle event
    async fn propagate_event(&self, event: LifecycleEvent) -> Result<()>;

    /// Register resource for cleanup
    async fn register_cleanup_resource(&self, resource: ResourceCleanup) -> Result<()>;

    /// Get active shutdowns
    async fn get_active_shutdowns(&self) -> Result<Vec<ShutdownStatus>>;
}

/// Lifecycle event listener trait
#[async_trait]
pub trait LifecycleEventListener: Send + Sync {
    /// Handle lifecycle event
    async fn on_event(&self, event: &LifecycleEvent) -> Result<()>;

    /// Get listener name
    fn name(&self) -> &str;
}

/// Default implementation of coordinated shutdown service
pub struct DefaultCoordinatedShutdownService {
    /// Configuration
    config: CoordinatedShutdownConfig,
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Runner pool manager
    runner_pool: Arc<dyn RunnerPoolManager>,
    /// Migration service
    migration_service: Option<Arc<dyn RunnerMigrationService>>,
    /// Active shutdowns
    active_shutdowns: Arc<RwLock<HashMap<Uuid, ShutdownStatus>>>,
    /// Event listeners
    event_listeners: Arc<RwLock<Vec<Box<dyn LifecycleEventListener>>>>,
    /// Event broadcaster
    event_broadcaster: broadcast::Sender<LifecycleEvent>,
    /// Cleanup resources
    cleanup_resources: Arc<RwLock<Vec<ResourceCleanup>>>,
    /// Running state
    running: Arc<Mutex<bool>>,
}

impl DefaultCoordinatedShutdownService {
    /// Create a new coordinated shutdown service
    pub fn new(
        config: CoordinatedShutdownConfig,
        node_registry: Arc<NodeRegistry>,
        runner_pool: Arc<dyn RunnerPoolManager>,
        migration_service: Option<Arc<dyn RunnerMigrationService>>,
    ) -> Self {
        let (event_broadcaster, _) = broadcast::channel(1000);

        Self {
            config,
            node_registry,
            runner_pool,
            migration_service,
            active_shutdowns: Arc::new(RwLock::new(HashMap::new())),
            event_listeners: Arc::new(RwLock::new(Vec::new())),
            event_broadcaster,
            cleanup_resources: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start shutdown monitoring task
    async fn start_shutdown_monitoring(&self) -> Result<()> {
        let active_shutdowns = self.active_shutdowns.clone();
        let config = self.config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Check for timed out shutdowns
                let now = Utc::now();
                let mut shutdowns = active_shutdowns.write().await;
                let mut timed_out_shutdowns = Vec::new();

                for (shutdown_id, status) in shutdowns.iter_mut() {
                    if matches!(status.status, ShutdownState::InProgress) {
                        // Check if shutdown has timed out (simplified check)
                        let elapsed = now - status.last_updated;
                        if elapsed > chrono::Duration::seconds(config.job_completion_timeout as i64)
                        {
                            status.status = ShutdownState::TimedOut;
                            status.last_updated = now;
                            timed_out_shutdowns.push(*shutdown_id);
                        }
                    }
                }

                for shutdown_id in timed_out_shutdowns {
                    warn!("Shutdown {} timed out", shutdown_id);
                }
            }
        });

        Ok(())
    }

    /// Start event propagation task
    async fn start_event_propagation(&self) -> Result<()> {
        let mut event_receiver = self.event_broadcaster.subscribe();
        let event_listeners = self.event_listeners.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            loop {
                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Wait for events
                match event_receiver.recv().await {
                    Ok(event) => {
                        let listeners = event_listeners.read().await;

                        // Propagate event to all listeners
                        for listener in listeners.iter() {
                            if let Err(e) = listener.on_event(&event).await {
                                warn!("Event listener {} failed: {}", listener.name(), e);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Event receiver lagged, skipped {} events", skipped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("Event broadcaster closed");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Execute shutdown phases
    async fn execute_shutdown_phases(&self, request: &ShutdownRequest) -> Result<ShutdownStatus> {
        let mut status = ShutdownStatus {
            shutdown_id: request.shutdown_id,
            current_phase: "prepare".to_string(),
            phase_progress: 0.0,
            overall_progress: 0.0,
            status: ShutdownState::InProgress,
            running_jobs: 0,
            completed_jobs: 0,
            migrated_jobs: 0,
            failed_jobs: 0,
            resources_cleaned: 0,
            last_updated: Utc::now(),
            errors: Vec::new(),
        };

        let total_phases = self.config.shutdown_phases.len();

        for (phase_index, phase) in self.config.shutdown_phases.iter().enumerate() {
            status.current_phase = phase.name.clone();
            status.phase_progress = 0.0;
            status.overall_progress = phase_index as f64 / total_phases as f64;
            status.last_updated = Utc::now();

            // Update status
            {
                let mut shutdowns = self.active_shutdowns.write().await;
                shutdowns.insert(request.shutdown_id, status.clone());
            }

            info!(
                "Executing shutdown phase: {} for {}",
                phase.name, request.shutdown_id
            );

            // Execute phase with timeout
            let phase_result = timeout(
                Duration::from_secs(phase.timeout),
                self.execute_phase(phase, request, &mut status),
            )
            .await;

            match phase_result {
                Ok(Ok(())) => {
                    status.phase_progress = 1.0;
                    info!("Completed shutdown phase: {}", phase.name);
                }
                Ok(Err(e)) => {
                    error!("Shutdown phase {} failed: {}", phase.name, e);
                    status
                        .errors
                        .push(format!("Phase {} failed: {}", phase.name, e));
                    status.status = ShutdownState::Failed;
                    return Ok(status);
                }
                Err(_) => {
                    error!("Shutdown phase {} timed out", phase.name);
                    status
                        .errors
                        .push(format!("Phase {} timed out", phase.name));
                    status.status = ShutdownState::TimedOut;
                    return Ok(status);
                }
            }
        }

        status.overall_progress = 1.0;
        status.status = ShutdownState::Completed;
        status.last_updated = Utc::now();

        info!("Shutdown {} completed successfully", request.shutdown_id);
        Ok(status)
    }

    /// Execute a single shutdown phase
    async fn execute_phase(
        &self,
        phase: &ShutdownPhase,
        request: &ShutdownRequest,
        status: &mut ShutdownStatus,
    ) -> Result<()> {
        match phase.name.as_str() {
            "prepare" => self.execute_prepare_phase(request, status).await,
            "drain" => self.execute_drain_phase(request, status).await,
            "migrate" => self.execute_migrate_phase(request, status).await,
            "cleanup" => self.execute_cleanup_phase(request, status).await,
            "finalize" => self.execute_finalize_phase(request, status).await,
            _ => {
                warn!("Unknown shutdown phase: {}", phase.name);
                Ok(())
            }
        }
    }

    /// Execute prepare phase
    async fn execute_prepare_phase(
        &self,
        request: &ShutdownRequest,
        status: &mut ShutdownStatus,
    ) -> Result<()> {
        // Send shutdown notifications
        let event = LifecycleEvent::ShutdownInitiated {
            shutdown_id: request.shutdown_id,
            target: request.target.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to send shutdown initiated event: {}", e);
        }

        // Count running jobs
        match &request.target {
            ShutdownTarget::Runner { runner_id } => {
                if let Ok(Some(runner_registration)) = self.runner_pool.get_runner(*runner_id).await
                {
                    status.running_jobs = runner_registration.assigned_jobs.len() as u32;
                }
            }
            ShutdownTarget::Node { node_id } => {
                // Count jobs on all runners on this node
                let runners = self.runner_pool.list_runners().await?;
                status.running_jobs = runners
                    .iter()
                    .filter(|r| r.entity.node_id == Some(*node_id))
                    .map(|r| r.assigned_jobs.len() as u32)
                    .sum();
            }
            _ => {
                // For other targets, get total running jobs
                let pool_stats = self.runner_pool.get_stats().await?;
                status.running_jobs = pool_stats.busy_runners as u32;
            }
        }

        Ok(())
    }

    /// Execute drain phase
    async fn execute_drain_phase(
        &self,
        request: &ShutdownRequest,
        status: &mut ShutdownStatus,
    ) -> Result<()> {
        // Stop accepting new jobs (implementation would mark runners as draining)
        info!("Draining jobs for shutdown {}", request.shutdown_id);

        // Wait for running jobs to complete
        let start_time = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(self.config.job_completion_timeout);

        while start_time.elapsed() < timeout_duration {
            // Check remaining running jobs
            let remaining_jobs = self.count_running_jobs(&request.target).await?;

            if remaining_jobs == 0 {
                status.completed_jobs = status.running_jobs;
                status.running_jobs = 0;
                break;
            }

            status.running_jobs = remaining_jobs;
            status.completed_jobs = status.running_jobs.saturating_sub(remaining_jobs);
            status.phase_progress =
                status.completed_jobs as f64 / (status.completed_jobs + status.running_jobs) as f64;

            // Update status
            {
                let mut shutdowns = self.active_shutdowns.write().await;
                shutdowns.insert(request.shutdown_id, status.clone());
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        Ok(())
    }

    /// Execute migrate phase
    async fn execute_migrate_phase(
        &self,
        request: &ShutdownRequest,
        status: &mut ShutdownStatus,
    ) -> Result<()> {
        if !self.config.enable_job_migration {
            return Ok(());
        }

        if let Some(migration_service) = &self.migration_service {
            match &request.target {
                ShutdownTarget::Runner { runner_id } => {
                    // Migrate jobs from this runner
                    match migration_service
                        .migrate_jobs_from_failed_runner(*runner_id)
                        .await
                    {
                        Ok(migrated_jobs) => {
                            status.migrated_jobs = migrated_jobs.len() as u32;
                            info!(
                                "Migrated {} jobs from runner {}",
                                migrated_jobs.len(),
                                runner_id
                            );
                        }
                        Err(e) => {
                            warn!("Failed to migrate jobs from runner {}: {}", runner_id, e);
                            status.errors.push(format!("Job migration failed: {}", e));
                        }
                    }
                }
                _ => {
                    // For other targets, migration would be more complex
                    debug!(
                        "Job migration not implemented for target type: {:?}",
                        request.target
                    );
                }
            }
        }

        Ok(())
    }

    /// Execute cleanup phase
    async fn execute_cleanup_phase(
        &self,
        _request: &ShutdownRequest,
        status: &mut ShutdownStatus,
    ) -> Result<()> {
        let cleanup_resources = self.cleanup_resources.read().await;
        let mut cleaned_count = 0;

        // Sort resources by priority (highest first)
        let mut sorted_resources: Vec<_> = cleanup_resources.iter().collect();
        sorted_resources.sort_by(|a, b| b.priority.cmp(&a.priority));

        for resource in sorted_resources {
            match self.cleanup_resource(resource).await {
                Ok(()) => {
                    cleaned_count += 1;
                    debug!("Cleaned up resource: {}", resource.resource_id);
                }
                Err(e) => {
                    warn!("Failed to cleanup resource {}: {}", resource.resource_id, e);
                    status.errors.push(format!(
                        "Cleanup failed for {}: {}",
                        resource.resource_id, e
                    ));
                }
            }
        }

        status.resources_cleaned = cleaned_count;
        Ok(())
    }

    /// Execute finalize phase
    async fn execute_finalize_phase(
        &self,
        request: &ShutdownRequest,
        _status: &mut ShutdownStatus,
    ) -> Result<()> {
        // Send shutdown completed event
        let event = LifecycleEvent::ShutdownCompleted {
            shutdown_id: request.shutdown_id,
            target: request.target.clone(),
            duration: Duration::from_secs(0), // Would calculate actual duration
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to send shutdown completed event: {}", e);
        }

        // Final cleanup and status update
        info!("Finalized shutdown {}", request.shutdown_id);
        Ok(())
    }

    /// Count running jobs for a target
    async fn count_running_jobs(&self, target: &ShutdownTarget) -> Result<u32> {
        match target {
            ShutdownTarget::Runner { runner_id } => {
                if let Ok(Some(runner_registration)) = self.runner_pool.get_runner(*runner_id).await
                {
                    Ok(runner_registration.assigned_jobs.len() as u32)
                } else {
                    Ok(0)
                }
            }
            ShutdownTarget::Node { node_id } => {
                let runners = self.runner_pool.list_runners().await?;
                Ok(runners
                    .iter()
                    .filter(|r| r.entity.node_id == Some(*node_id))
                    .map(|r| r.assigned_jobs.len() as u32)
                    .sum())
            }
            _ => {
                let pool_stats = self.runner_pool.get_stats().await?;
                Ok(pool_stats.busy_runners as u32)
            }
        }
    }

    /// Cleanup a single resource
    async fn cleanup_resource(&self, resource: &ResourceCleanup) -> Result<()> {
        match &resource.resource_type {
            ResourceType::TempFiles { path } => {
                // Cleanup temporary files
                debug!("Cleaning up temp files at: {}", path);
                // Implementation would delete files
            }
            ResourceType::LogFiles { path } => {
                // Archive or cleanup log files
                debug!("Cleaning up log files at: {}", path);
                // Implementation would archive/delete logs
            }
            ResourceType::DatabaseConnections { connection_id } => {
                // Close database connections
                debug!("Closing database connection: {}", connection_id);
                // Implementation would close connections
            }
            ResourceType::NetworkConnections { connection_id } => {
                // Close network connections
                debug!("Closing network connection: {}", connection_id);
                // Implementation would close connections
            }
            ResourceType::MemoryAllocations { allocation_id } => {
                // Free memory allocations
                debug!("Freeing memory allocation: {}", allocation_id);
                // Implementation would free memory
            }
            ResourceType::ProcessHandles { process_id } => {
                // Close process handles
                debug!("Closing process handle: {}", process_id);
                // Implementation would close handles
            }
            ResourceType::Custom { resource_name } => {
                // Custom resource cleanup
                debug!("Cleaning up custom resource: {}", resource_name);
                // Implementation would call custom cleanup
            }
        }

        // Simulate cleanup time
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

#[async_trait]
impl CoordinatedShutdownService for DefaultCoordinatedShutdownService {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest(
                "Shutdown service is already running".to_string(),
            ));
        }

        *running = true;

        // Start monitoring tasks
        self.start_shutdown_monitoring().await?;
        self.start_event_propagation().await?;

        info!("Coordinated shutdown service started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }

        *running = false;

        info!("Coordinated shutdown service stopped");
        Ok(())
    }

    async fn request_shutdown(&self, request: ShutdownRequest) -> Result<Uuid> {
        let shutdown_id = request.shutdown_id;

        info!(
            "Received shutdown request: {} for {:?}",
            shutdown_id, request.target
        );

        // Check if we're at the concurrent shutdown limit
        {
            let shutdowns = self.active_shutdowns.read().await;
            let active_count = shutdowns
                .values()
                .filter(|s| matches!(s.status, ShutdownState::InProgress))
                .count();

            if active_count >= self.config.max_concurrent_shutdowns as usize {
                return Err(AppError::ResourceExhausted(
                    "Maximum concurrent shutdowns reached".to_string(),
                ));
            }
        }

        // Execute shutdown in background
        let service = DefaultCoordinatedShutdownService {
            config: self.config.clone(),
            node_registry: self.node_registry.clone(),
            runner_pool: self.runner_pool.clone(),
            migration_service: self.migration_service.clone(),
            active_shutdowns: self.active_shutdowns.clone(),
            event_listeners: self.event_listeners.clone(),
            event_broadcaster: self.event_broadcaster.clone(),
            cleanup_resources: self.cleanup_resources.clone(),
            running: self.running.clone(),
        };

        tokio::spawn(async move {
            match service.execute_shutdown_phases(&request).await {
                Ok(final_status) => {
                    let mut shutdowns = service.active_shutdowns.write().await;
                    shutdowns.insert(shutdown_id, final_status);
                }
                Err(e) => {
                    error!("Shutdown execution failed: {}", e);
                    let failed_status = ShutdownStatus {
                        shutdown_id,
                        current_phase: "failed".to_string(),
                        phase_progress: 0.0,
                        overall_progress: 0.0,
                        status: ShutdownState::Failed,
                        running_jobs: 0,
                        completed_jobs: 0,
                        migrated_jobs: 0,
                        failed_jobs: 0,
                        resources_cleaned: 0,
                        last_updated: Utc::now(),
                        errors: vec![e.to_string()],
                    };

                    let mut shutdowns = service.active_shutdowns.write().await;
                    shutdowns.insert(shutdown_id, failed_status);
                }
            }
        });

        Ok(shutdown_id)
    }

    async fn get_shutdown_status(&self, shutdown_id: Uuid) -> Result<ShutdownStatus> {
        let shutdowns = self.active_shutdowns.read().await;
        shutdowns
            .get(&shutdown_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Shutdown {} not found", shutdown_id)))
    }

    async fn cancel_shutdown(&self, shutdown_id: Uuid) -> Result<()> {
        let mut shutdowns = self.active_shutdowns.write().await;

        if let Some(status) = shutdowns.get_mut(&shutdown_id) {
            if matches!(status.status, ShutdownState::InProgress) {
                status.status = ShutdownState::Cancelled;
                status.last_updated = Utc::now();
                info!("Cancelled shutdown {}", shutdown_id);
                Ok(())
            } else {
                Err(AppError::BadRequest(format!(
                    "Shutdown {} cannot be cancelled (status: {:?})",
                    shutdown_id, status.status
                )))
            }
        } else {
            Err(AppError::NotFound(format!(
                "Shutdown {} not found",
                shutdown_id
            )))
        }
    }

    async fn register_event_listener(
        &self,
        listener: Box<dyn LifecycleEventListener>,
    ) -> Result<()> {
        let mut listeners = self.event_listeners.write().await;
        listeners.push(listener);
        Ok(())
    }

    async fn propagate_event(&self, event: LifecycleEvent) -> Result<()> {
        if let Err(e) = self.event_broadcaster.send(event) {
            warn!("Failed to propagate event: {}", e);
        }
        Ok(())
    }

    async fn register_cleanup_resource(&self, resource: ResourceCleanup) -> Result<()> {
        let mut resources = self.cleanup_resources.write().await;
        resources.push(resource);
        Ok(())
    }

    async fn get_active_shutdowns(&self) -> Result<Vec<ShutdownStatus>> {
        let shutdowns = self.active_shutdowns.read().await;
        Ok(shutdowns
            .values()
            .filter(|s| matches!(s.status, ShutdownState::InProgress))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cluster::node_registry::tests::create_test_registry;
    use crate::core::DefaultRunnerPoolManager;

    fn create_test_shutdown_service() -> DefaultCoordinatedShutdownService {
        let config = CoordinatedShutdownConfig::default();
        let node_registry = Arc::new(create_test_registry());
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());

        DefaultCoordinatedShutdownService::new(config, node_registry, runner_pool, None)
    }

    #[tokio::test]
    async fn test_shutdown_service_creation() {
        let service = create_test_shutdown_service();
        assert!(!*service.running.lock().await);
    }

    #[tokio::test]
    async fn test_shutdown_request() {
        let request = ShutdownRequest {
            shutdown_id: Uuid::new_v4(),
            target: ShutdownTarget::Runner {
                runner_id: Uuid::new_v4(),
            },
            shutdown_type: ShutdownType::Graceful,
            reason: "Test shutdown".to_string(),
            requested_by: "test".to_string(),
            requested_at: Utc::now(),
            timeout: Duration::from_secs(300),
            force_on_timeout: false,
        };

        assert!(matches!(request.shutdown_type, ShutdownType::Graceful));
        assert_eq!(request.reason, "Test shutdown");
        assert!(!request.force_on_timeout);
    }

    #[tokio::test]
    async fn test_shutdown_phases() {
        let config = CoordinatedShutdownConfig::default();
        assert_eq!(config.shutdown_phases.len(), 5);

        let prepare_phase = &config.shutdown_phases[0];
        assert_eq!(prepare_phase.name, "prepare");
        assert_eq!(prepare_phase.timeout, 30);

        let drain_phase = &config.shutdown_phases[1];
        assert_eq!(drain_phase.name, "drain");
        assert_eq!(drain_phase.timeout, 180);
    }

    #[tokio::test]
    async fn test_lifecycle_events() {
        let runner_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let event = LifecycleEvent::RunnerStarted {
            runner_id,
            node_id,
            timestamp: Utc::now(),
        };

        match event {
            LifecycleEvent::RunnerStarted {
                runner_id: r_id,
                node_id: n_id,
                ..
            } => {
                assert_eq!(r_id, runner_id);
                assert_eq!(n_id, node_id);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_resource_cleanup() {
        let resource = ResourceCleanup {
            resource_type: ResourceType::TempFiles {
                path: "/tmp/test".to_string(),
            },
            resource_id: "temp-files-1".to_string(),
            strategy: CleanupStrategy::DeleteTempFiles,
            priority: 100,
            estimated_time: Duration::from_secs(5),
        };

        assert_eq!(resource.resource_id, "temp-files-1");
        assert_eq!(resource.priority, 100);
        assert!(matches!(
            resource.strategy,
            CleanupStrategy::DeleteTempFiles
        ));
    }

    #[tokio::test]
    async fn test_shutdown_status() {
        let shutdown_id = Uuid::new_v4();
        let status = ShutdownStatus {
            shutdown_id,
            current_phase: "drain".to_string(),
            phase_progress: 0.5,
            overall_progress: 0.3,
            status: ShutdownState::InProgress,
            running_jobs: 5,
            completed_jobs: 3,
            migrated_jobs: 1,
            failed_jobs: 0,
            resources_cleaned: 2,
            last_updated: Utc::now(),
            errors: Vec::new(),
        };

        assert_eq!(status.shutdown_id, shutdown_id);
        assert_eq!(status.current_phase, "drain");
        assert_eq!(status.phase_progress, 0.5);
        assert!(matches!(status.status, ShutdownState::InProgress));
        assert_eq!(status.running_jobs, 5);
        assert_eq!(status.completed_jobs, 3);
    }
}
