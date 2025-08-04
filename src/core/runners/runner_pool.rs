//! Runner pool manager for managing multiple runners and job scheduling
//! 
//! This module provides a comprehensive runner pool management system with
//! load balancing, health monitoring, and job scheduling capabilities.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::jobs::job_queue::{JobQueue, InMemoryJobQueue};
use crate::domain::entities::{
    Runner, RunnerEntity, Job, RunnerId, 
    RunnerStatus, HealthStatus, RunnerCapacity
};
use crate::error::{AppError, Result};

/// Load balancing strategies for job distribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Least loaded runner first
    LeastLoaded,
    /// Random selection
    Random,
    /// Weighted distribution based on runner capacity
    Weighted,
    /// First available runner
    FirstAvailable,
}

/// Runner pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerPoolConfig {
    /// Maximum number of runners in the pool
    pub max_runners: usize,
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Runner timeout in seconds
    pub runner_timeout: u64,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Enable automatic failover
    pub enable_failover: bool,
    /// Maximum job retries
    pub max_job_retries: u32,
    /// Job retry delay in seconds
    pub job_retry_delay: u64,
}

impl Default for RunnerPoolConfig {
    fn default() -> Self {
        Self {
            max_runners: 50,
            health_check_interval: 30,
            runner_timeout: 300,
            load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
            enable_failover: true,
            max_job_retries: 3,
            job_retry_delay: 30,
        }
    }
}

/// Runner pool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerPoolStats {
    /// Total number of runners
    pub total_runners: usize,
    /// Number of active runners
    pub active_runners: usize,
    /// Number of idle runners
    pub idle_runners: usize,
    /// Number of busy runners
    pub busy_runners: usize,
    /// Number of failed runners
    pub failed_runners: usize,
    /// Total jobs processed
    pub total_jobs_processed: u64,
    /// Total jobs failed
    pub total_jobs_failed: u64,
    /// Average job execution time
    pub avg_job_execution_time: f64,
    /// Pool utilization percentage
    pub pool_utilization: f64,
    /// Jobs per minute throughput
    pub jobs_per_minute: f64,
}

/// Runner registration information
#[derive(Clone)]
pub struct RunnerRegistration {
    /// Runner instance
    pub runner: Arc<dyn Runner>,
    /// Runner entity metadata
    pub entity: RunnerEntity,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Last health check timestamp
    pub last_health_check: DateTime<Utc>,
    /// Current job assignments
    pub assigned_jobs: Vec<crate::domain::entities::JobId>,
    /// Runner statistics
    pub stats: RunnerStats,
}

/// Individual runner statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerStats {
    /// Jobs executed by this runner
    pub jobs_executed: u64,
    /// Jobs failed by this runner
    pub jobs_failed: u64,
    /// Average execution time for this runner
    pub avg_execution_time: f64,
    /// Last job execution timestamp
    pub last_job_execution: Option<DateTime<Utc>>,
    /// Runner uptime in seconds
    pub uptime_seconds: u64,
}

/// Job scheduling context
#[derive(Debug, Clone)]
pub struct SchedulingContext {
    /// Job to be scheduled
    pub job: Job,
    /// Available runners
    pub available_runners: Vec<RunnerId>,
    /// Runner capacities
    pub runner_capacities: HashMap<RunnerId, RunnerCapacity>,
    /// Load balancing strategy
    pub strategy: LoadBalancingStrategy,
    /// Scheduling constraints
    pub constraints: SchedulingConstraints,
}

/// Scheduling constraints
#[derive(Debug, Clone, Default)]
pub struct SchedulingConstraints {
    /// Required runner tags
    pub required_tags: Vec<String>,
    /// Excluded runner tags
    pub excluded_tags: Vec<String>,
    /// Minimum resource requirements
    pub min_cpu: Option<u32>,
    pub min_memory: Option<u32>,
    /// Node affinity
    pub node_affinity: Vec<String>,
}

/// Load balancer trait for different strategies
#[async_trait]
pub trait LoadBalancer: Send + Sync {
    /// Select the best runner for a job
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>>;
    
    /// Get strategy name
    fn strategy_name(&self) -> &'static str;
}

/// Runner pool manager trait
#[async_trait]
pub trait RunnerPoolManager: Send + Sync {
    /// Register a new runner
    async fn register_runner(&self, runner: Arc<dyn Runner>, entity: RunnerEntity) -> Result<()>;
    
    /// Deregister a runner
    async fn deregister_runner(&self, runner_id: RunnerId) -> Result<()>;
    
    /// Submit a job for execution
    async fn submit_job(&self, job: Job) -> Result<()>;
    
    /// Get pool statistics
    async fn get_stats(&self) -> Result<RunnerPoolStats>;
    
    /// Get runner by ID
    async fn get_runner(&self, runner_id: RunnerId) -> Result<Option<RunnerRegistration>>;
    
    /// List all runners
    async fn list_runners(&self) -> Result<Vec<RunnerRegistration>>;
    
    /// Perform health checks on all runners
    async fn health_check_all(&self) -> Result<()>;
    
    /// Start the pool manager
    async fn start(&self) -> Result<()>;
    
    /// Stop the pool manager
    async fn stop(&self) -> Result<()>;
}

/// Default runner pool manager implementation
pub struct DefaultRunnerPoolManager {
    /// Registered runners
    runners: Arc<RwLock<HashMap<RunnerId, RunnerRegistration>>>,
    /// Job queue
    job_queue: Arc<dyn JobQueue>,
    /// Load balancer
    load_balancer: Arc<dyn LoadBalancer>,
    /// Configuration
    config: RunnerPoolConfig,
    /// Pool statistics
    stats: Arc<RwLock<RunnerPoolStats>>,
    /// Running state
    running: Arc<Mutex<bool>>,
}

impl DefaultRunnerPoolManager {
    /// Create a new runner pool manager
    pub fn new(config: RunnerPoolConfig) -> Self {
        let load_balancer = create_load_balancer(&config.load_balancing_strategy);
        let job_queue = Arc::new(InMemoryJobQueue::with_default_config());
        
        Self {
            runners: Arc::new(RwLock::new(HashMap::new())),
            job_queue,
            load_balancer,
            config,
            stats: Arc::new(RwLock::new(RunnerPoolStats::default())),
            running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(RunnerPoolConfig::default())
    }
    
    /// Process jobs from the queue
    async fn process_jobs(&self) -> Result<()> {
        while let Some(job) = self.job_queue.dequeue().await? {
            if let Err(e) = self.schedule_job(job).await {
                error!("Failed to schedule job: {}", e);
            }
        }
        Ok(())
    }
    
    /// Schedule a job to an appropriate runner
    async fn schedule_job(&self, job: Job) -> Result<()> {
        let runners = self.runners.read().await;
        
        // Find available runners that can handle this job
        let mut available_runners = Vec::new();
        let mut runner_capacities = HashMap::new();
        
        for (runner_id, registration) in runners.iter() {
            if registration.entity.is_available() && job.matches_runner(&registration.entity) {
                if let Ok(capacity) = registration.runner.get_capacity().await {
                    if capacity.available_slots > 0 {
                        available_runners.push(*runner_id);
                        runner_capacities.insert(*runner_id, capacity);
                    }
                }
            }
        }
        
        if available_runners.is_empty() {
            warn!("No available runners for job {}", job.id);
            // Re-queue the job for later
            self.job_queue.enqueue(job).await?;
            return Ok(());
        }
        
        // Create scheduling context
        let context = SchedulingContext {
            job: job.clone(),
            available_runners,
            runner_capacities,
            strategy: self.config.load_balancing_strategy.clone(),
            constraints: SchedulingConstraints {
                required_tags: job.requirements.required_tags.clone(),
                excluded_tags: job.requirements.excluded_tags.clone(),
                min_cpu: job.requirements.min_resources.as_ref().map(|r| r.min_cpu),
                min_memory: job.requirements.min_resources.as_ref().map(|r| r.min_memory),
                node_affinity: job.requirements.node_affinity.clone(),
            },
        };
        
        // Select runner using load balancer
        if let Some(selected_runner_id) = self.load_balancer.select_runner(&context, &runners).await? {
            if let Some(registration) = runners.get(&selected_runner_id) {
                let runner = registration.runner.clone();
                let job_id = job.id;
                
                // Execute job asynchronously
                let stats = self.stats.clone();
                tokio::spawn(async move {
                    let start_time = Utc::now();
                    match runner.execute(job).await {
                        Ok(_result) => {
                            let execution_time = (Utc::now() - start_time).num_milliseconds() as f64 / 1000.0;
                            info!("Job {} completed successfully in {:.2}s", job_id, execution_time);
                            
                            // Update statistics
                            let mut stats = stats.write().await;
                            stats.total_jobs_processed += 1;
                            stats.avg_job_execution_time = 
                                (stats.avg_job_execution_time * (stats.total_jobs_processed - 1) as f64 + execution_time) 
                                / stats.total_jobs_processed as f64;
                        }
                        Err(e) => {
                            error!("Job {} failed: {}", job_id, e);
                            
                            // Update statistics
                            let mut stats = stats.write().await;
                            stats.total_jobs_failed += 1;
                        }
                    }
                });
                
                info!("Scheduled job {} to runner {}", job_id, selected_runner_id);
            }
        } else {
            warn!("Load balancer could not select a runner for job {}", job.id);
            // Re-queue the job for later
            self.job_queue.enqueue(job).await?;
        }
        
        Ok(())
    }
    
    /// Update pool statistics
    async fn update_stats(&self) -> Result<()> {
        let runners = self.runners.read().await;
        let mut stats = self.stats.write().await;
        
        stats.total_runners = runners.len();
        stats.active_runners = 0;
        stats.idle_runners = 0;
        stats.busy_runners = 0;
        stats.failed_runners = 0;
        
        for registration in runners.values() {
            match registration.entity.status {
                RunnerStatus::Active => stats.active_runners += 1,
                RunnerStatus::Idle => stats.idle_runners += 1,
                RunnerStatus::Busy => stats.busy_runners += 1,
                RunnerStatus::Failed => stats.failed_runners += 1,
                _ => {}
            }
        }
        
        // Calculate pool utilization
        if stats.total_runners > 0 {
            stats.pool_utilization = (stats.busy_runners as f64 / stats.total_runners as f64) * 100.0;
        }
        
        Ok(())
    }
}

#[async_trait]
impl RunnerPoolManager for DefaultRunnerPoolManager {
    async fn register_runner(&self, runner: Arc<dyn Runner>, entity: RunnerEntity) -> Result<()> {
        let mut runners = self.runners.write().await;
        
        if runners.len() >= self.config.max_runners {
            return Err(AppError::ResourceExhausted(
                format!("Runner pool is full (max: {})", self.config.max_runners)
            ));
        }
        
        let registration = RunnerRegistration {
            runner,
            entity: entity.clone(),
            registered_at: Utc::now(),
            last_health_check: Utc::now(),
            assigned_jobs: Vec::new(),
            stats: RunnerStats::default(),
        };
        
        runners.insert(entity.id, registration);
        
        info!("Registered runner: {} ({})", entity.name, entity.id);
        Ok(())
    }
    
    async fn deregister_runner(&self, runner_id: RunnerId) -> Result<()> {
        let mut runners = self.runners.write().await;
        
        if let Some(registration) = runners.remove(&runner_id) {
            // Gracefully shutdown the runner
            if let Err(e) = registration.runner.shutdown().await {
                warn!("Error shutting down runner {}: {}", runner_id, e);
            }
            
            info!("Deregistered runner: {}", runner_id);
            Ok(())
        } else {
            Err(AppError::NotFound(format!("Runner {} not found", runner_id)))
        }
    }
    
    async fn submit_job(&self, job: Job) -> Result<()> {
        self.job_queue.enqueue(job).await
    }
    
    async fn get_stats(&self) -> Result<RunnerPoolStats> {
        self.update_stats().await?;
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn get_runner(&self, runner_id: RunnerId) -> Result<Option<RunnerRegistration>> {
        let runners = self.runners.read().await;
        Ok(runners.get(&runner_id).cloned())
    }
    
    async fn list_runners(&self) -> Result<Vec<RunnerRegistration>> {
        let runners = self.runners.read().await;
        Ok(runners.values().cloned().collect())
    }
    
    async fn health_check_all(&self) -> Result<()> {
        let runners = self.runners.read().await;
        let mut health_check_tasks = Vec::new();
        
        for (runner_id, registration) in runners.iter() {
            let runner = registration.runner.clone();
            let runner_id = *runner_id;
            
            let task = tokio::spawn(async move {
                match runner.health_check().await {
                    Ok(health_status) => {
                        debug!("Runner {} health check: {:?}", runner_id, health_status);
                        (runner_id, health_status)
                    }
                    Err(e) => {
                        warn!("Runner {} health check failed: {}", runner_id, e);
                        (runner_id, HealthStatus::Unhealthy { reason: e.to_string() })
                    }
                }
            });
            
            health_check_tasks.push(task);
        }
        
        // Wait for all health checks to complete
        for task in health_check_tasks {
            if let Ok((runner_id, health_status)) = task.await {
                // Update runner status based on health check
                let mut runners = self.runners.write().await;
                if let Some(registration) = runners.get_mut(&runner_id) {
                    registration.last_health_check = Utc::now();
                    
                    match health_status {
                        HealthStatus::Healthy => {
                            if registration.entity.status == RunnerStatus::Failed {
                                registration.entity.status = RunnerStatus::Idle;
                            }
                        }
                        HealthStatus::Unhealthy { .. } => {
                            registration.entity.status = RunnerStatus::Failed;
                        }
                        HealthStatus::Degraded { .. } => {
                            // Keep current status but log warning
                            warn!("Runner {} is degraded: {:?}", runner_id, health_status);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest("Runner pool is already running".to_string()));
        }
        
        *running = true;
        
        // Start background tasks
        let _pool_manager = self;
        
        // Job processing task
        let runners = self.runners.clone();
        let job_queue = self.job_queue.clone();
        let load_balancer = self.load_balancer.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_millis(100)
            );
            
            loop {
                interval.tick().await;
                // Process jobs from queue
                while let Ok(Some(job)) = job_queue.dequeue().await {
                    // Schedule job logic here
                    let runners_read = runners.read().await;
                    
                    // Find available runners
                    let mut available_runners = Vec::new();
                    let mut runner_capacities = HashMap::new();
                    
                    for (runner_id, registration) in runners_read.iter() {
                        if registration.entity.is_available() {
                            if let Ok(capacity) = registration.runner.get_capacity().await {
                                if capacity.available_slots > 0 {
                                    available_runners.push(*runner_id);
                                    runner_capacities.insert(*runner_id, capacity);
                                }
                            }
                        }
                    }
                    
                    if !available_runners.is_empty() {
                        let context = SchedulingContext {
                            job: job.clone(),
                            available_runners,
                            runner_capacities,
                            strategy: config.load_balancing_strategy.clone(),
                            constraints: SchedulingConstraints::default(),
                        };
                        
                        if let Ok(Some(selected_runner_id)) = load_balancer.select_runner(&context, &runners_read).await {
                            if let Some(registration) = runners_read.get(&selected_runner_id) {
                                let runner = registration.runner.clone();
                                let job_id = job.id;
                                let stats_clone = stats.clone();
                                
                                tokio::spawn(async move {
                                    let start_time = Utc::now();
                                    match runner.execute(job).await {
                                        Ok(_result) => {
                                            let execution_time = (Utc::now() - start_time).num_milliseconds() as f64 / 1000.0;
                                            info!("Job {} completed successfully in {:.2}s", job_id, execution_time);
                                            
                                            let mut stats = stats_clone.write().await;
                                            stats.total_jobs_processed += 1;
                                            stats.avg_job_execution_time = 
                                                (stats.avg_job_execution_time * (stats.total_jobs_processed - 1) as f64 + execution_time) 
                                                / stats.total_jobs_processed as f64;
                                        }
                                        Err(e) => {
                                            error!("Job {} failed: {}", job_id, e);
                                            let mut stats = stats_clone.write().await;
                                            stats.total_jobs_failed += 1;
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        });
        
        // Health check task
        let runners_health = self.runners.clone();
        let health_interval = self.config.health_check_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(health_interval)
            );
            
            loop {
                interval.tick().await;
                let runners = runners_health.read().await;
                
                for (runner_id, registration) in runners.iter() {
                    let runner = registration.runner.clone();
                    let runner_id = *runner_id;
                    
                    tokio::spawn(async move {
                        match runner.health_check().await {
                            Ok(health_status) => {
                                debug!("Runner {} health check: {:?}", runner_id, health_status);
                            }
                            Err(e) => {
                                warn!("Runner {} health check failed: {}", runner_id, e);
                            }
                        }
                    });
                }
            }
        });
        
        info!("Runner pool manager started");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }
        
        *running = false;
        
        // Shutdown all runners
        let runners = self.runners.read().await;
        for (runner_id, registration) in runners.iter() {
            if let Err(e) = registration.runner.shutdown().await {
                warn!("Error shutting down runner {}: {}", runner_id, e);
            }
        }
        
        info!("Runner pool manager stopped");
        Ok(())
    }
}

/// Create a load balancer for the given strategy
fn create_load_balancer(strategy: &LoadBalancingStrategy) -> Arc<dyn LoadBalancer> {
    use crate::core::runners::load_balancer::*;
    
    match strategy {
        LoadBalancingStrategy::RoundRobin => Arc::new(RoundRobinLoadBalancer::new()),
        LoadBalancingStrategy::LeastLoaded => Arc::new(LeastLoadedLoadBalancer::new()),
        LoadBalancingStrategy::Random => Arc::new(RandomLoadBalancer::new()),
        LoadBalancingStrategy::Weighted => Arc::new(WeightedLoadBalancer::new()),
        LoadBalancingStrategy::FirstAvailable => Arc::new(FirstAvailableLoadBalancer::new()),
    }
}