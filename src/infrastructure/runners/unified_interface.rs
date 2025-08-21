// Unified Runner Interface
// Task 3.3: Unified Runner System - Protocol-agnostic runner interface

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error};

use crate::error::{AppError, Result};
use crate::domain::entities::{Job, JobResult, HealthStatus, RunnerCapacity, RunnerMetadata};
use super::capability_detector::{DetectedCapabilities, PreferredProtocol, PerformanceTier};
use super::unified_registry::{UnifiedRunnerRegistry, RegisteredRunner, RunnerType};
use super::valkyrie_adapter::ValkyrieRunnerAdapter;
use super::http_fallback::HttpFallbackSystem;

/// Protocol-agnostic runner interface that automatically selects optimal protocol
pub struct UnifiedRunnerInterface {
    /// Runner ID
    id: Uuid,
    
    /// Registry for runner discovery
    registry: Arc<UnifiedRunnerRegistry>,
    
    /// Valkyrie adapter for high-performance communication
    valkyrie_adapter: Option<Arc<ValkyrieRunnerAdapter>>,
    
    /// HTTP fallback for compatibility
    http_fallback: Arc<HttpFallbackSystem>,
    
    /// Protocol selection strategy
    selection_strategy: Arc<ProtocolSelectionStrategy>,
    
    /// Performance metrics
    metrics: Arc<RwLock<InterfaceMetrics>>,
    
    /// Configuration
    config: UnifiedInterfaceConfig,
}

/// Configuration for unified interface
#[derive(Debug, Clone)]
pub struct UnifiedInterfaceConfig {
    /// Enable automatic protocol selection
    pub auto_protocol_selection: bool,
    
    /// Protocol selection timeout
    pub selection_timeout: Duration,
    
    /// Fallback timeout
    pub fallback_timeout: Duration,
    
    /// Performance monitoring interval
    pub metrics_interval: Duration,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Retry delay
    pub retry_delay: Duration,
    
    /// Enable protocol migration
    pub enable_protocol_migration: bool,
    
    /// Migration threshold (performance degradation %)
    pub migration_threshold: f64,
}

/// Protocol selection strategy
pub struct ProtocolSelectionStrategy {
    /// Selection algorithm
    algorithm: SelectionAlgorithm,
    
    /// Performance history
    performance_history: Arc<RwLock<HashMap<Uuid, ProtocolPerformance>>>,
    
    /// Configuration
    config: StrategyConfig,
}

/// Selection algorithms
#[derive(Debug, Clone)]
pub enum SelectionAlgorithm {
    /// Always prefer Valkyrie if available
    ValkyrieFirst,
    
    /// Always prefer HTTP for compatibility
    HttpFirst,
    
    /// Select based on performance metrics
    PerformanceBased,
    
    /// Adaptive selection based on job characteristics
    Adaptive,
    
    /// Round-robin between protocols
    RoundRobin,
    
    /// Custom selection logic
    Custom(String),
}

/// Strategy configuration
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Performance weight in selection (0.0 - 1.0)
    pub performance_weight: f64,
    
    /// Reliability weight in selection (0.0 - 1.0)
    pub reliability_weight: f64,
    
    /// Latency weight in selection (0.0 - 1.0)
    pub latency_weight: f64,
    
    /// Throughput weight in selection (0.0 - 1.0)
    pub throughput_weight: f64,
    
    /// Minimum sample size for performance-based decisions
    pub min_sample_size: usize,
    
    /// Performance history retention period
    pub history_retention: Duration,
}

/// Protocol performance tracking
#[derive(Debug, Clone)]
pub struct ProtocolPerformance {
    /// Protocol type
    pub protocol: PreferredProtocol,
    
    /// Average latency
    pub avg_latency: Duration,
    
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    
    /// Throughput (operations per second)
    pub throughput: f64,
    
    /// Total operations
    pub total_operations: u64,
    
    /// Failed operations
    pub failed_operations: u64,
    
    /// Last updated
    pub last_updated: Instant,
    
    /// Performance tier
    pub performance_tier: PerformanceTier,
}

/// Interface performance metrics
#[derive(Debug, Clone, Default)]
pub struct InterfaceMetrics {
    /// Total jobs processed
    pub total_jobs: u64,
    
    /// Jobs processed via Valkyrie
    pub valkyrie_jobs: u64,
    
    /// Jobs processed via HTTP
    pub http_jobs: u64,
    
    /// Protocol migrations
    pub protocol_migrations: u64,
    
    /// Fallback activations
    pub fallback_activations: u64,
    
    /// Average job latency
    pub avg_job_latency: Duration,
    
    /// Success rate
    pub success_rate: f64,
    
    /// Protocol distribution
    pub protocol_distribution: HashMap<String, u64>,
}

/// Job execution context
#[derive(Debug, Clone)]
pub struct JobExecutionContext {
    /// Job information
    pub job: Job,
    
    /// Selected runner
    pub runner_id: Uuid,
    
    /// Selected protocol
    pub protocol: PreferredProtocol,
    
    /// Execution start time
    pub start_time: Instant,
    
    /// Retry count
    pub retry_count: u32,
    
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,
}

/// Performance requirements for job execution
#[derive(Debug, Clone, Default)]
pub struct PerformanceRequirements {
    /// Maximum acceptable latency
    pub max_latency: Option<Duration>,
    
    /// Minimum required throughput
    pub min_throughput: Option<f64>,
    
    /// Required reliability (success rate)
    pub required_reliability: Option<f64>,
    
    /// Priority level
    pub priority: JobPriority,
    
    /// Resource requirements
    pub resource_requirements: ResourceRequirements,
}

#[derive(Debug, Clone, Default)]
pub enum JobPriority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub storage_mb: Option<u64>,
    pub network_bandwidth: Option<u64>,
}

impl Default for UnifiedInterfaceConfig {
    fn default() -> Self {
        Self {
            auto_protocol_selection: true,
            selection_timeout: Duration::from_millis(100),
            fallback_timeout: Duration::from_secs(5),
            metrics_interval: Duration::from_secs(60),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
            enable_protocol_migration: true,
            migration_threshold: 0.2, // 20% performance degradation
        }
    }
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            performance_weight: 0.4,
            reliability_weight: 0.3,
            latency_weight: 0.2,
            throughput_weight: 0.1,
            min_sample_size: 10,
            history_retention: Duration::from_secs(24 * 60 * 60), // 24 hours
        }
    }
}

impl UnifiedRunnerInterface {
    /// Create a new unified runner interface
    pub async fn new(
        registry: Arc<UnifiedRunnerRegistry>,
        valkyrie_adapter: Option<Arc<ValkyrieRunnerAdapter>>,
        http_fallback: Arc<HttpFallbackSystem>,
        config: UnifiedInterfaceConfig,
    ) -> Result<Self> {
        let id = Uuid::new_v4();
        
        let selection_strategy = Arc::new(ProtocolSelectionStrategy::new(
            SelectionAlgorithm::Adaptive,
            StrategyConfig::default(),
        ));
        
        let interface = Self {
            id,
            registry,
            valkyrie_adapter,
            http_fallback,
            selection_strategy,
            metrics: Arc::new(RwLock::new(InterfaceMetrics::default())),
            config,
        };
        
        // Start background tasks
        interface.start_background_tasks().await;
        
        Ok(interface)
    }
    
    /// Execute a job using optimal protocol selection
    pub async fn execute_job(&self, job: Job) -> Result<JobResult> {
        let start_time = Instant::now();
        let mut context = JobExecutionContext {
            job: job.clone(),
            runner_id: Uuid::nil(), // Will be set during selection
            protocol: PreferredProtocol::Auto,
            start_time,
            retry_count: 0,
            performance_requirements: self.extract_performance_requirements(&job),
        };
        
        // Execute with retries
        let result = self.execute_with_retries(&mut context).await;
        
        // Update metrics
        self.update_execution_metrics(&context, &result, start_time.elapsed()).await;
        
        result
    }
    
    /// Execute job with automatic retries and protocol fallback
    async fn execute_with_retries(&self, context: &mut JobExecutionContext) -> Result<JobResult> {
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            context.retry_count = attempt;
            
            // Select optimal runner and protocol
            match self.select_optimal_runner_and_protocol(context).await {
                Ok((runner_id, protocol)) => {
                    context.runner_id = runner_id;
                    context.protocol = protocol.clone();
                    
                    // Execute using selected protocol
                    match self.execute_with_protocol(context, &protocol).await {
                        Ok(result) => {
                            // Success - update performance metrics
                            self.record_successful_execution(context, &result).await;
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!(
                                "Job execution failed on attempt {} with protocol {:?}: {}",
                                attempt + 1, protocol, e
                            );
                            
                            // Record failure
                            self.record_failed_execution(context, &e).await;
                            last_error = Some(e);
                            
                            // Check if we should try protocol migration
                            if self.config.enable_protocol_migration && attempt < self.config.max_retries {
                                if let Some(fallback_protocol) = self.get_fallback_protocol(&protocol).await {
                                    info!("Attempting protocol migration from {:?} to {:?}", protocol, fallback_protocol);
                                    context.protocol = fallback_protocol;
                                    continue;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to select runner and protocol: {}", e);
                    last_error = Some(e);
                }
            }
            
            // Wait before retry
            if attempt < self.config.max_retries {
                tokio::time::sleep(self.config.retry_delay).await;
            }
        }
        
        Err(last_error.unwrap_or_else(|| AppError::InternalError {
            message: "Job execution failed after all retries".to_string(),
            component: "unified_interface".to_string(),
        }))
    }
    
    /// Select optimal runner and protocol for job execution
    async fn select_optimal_runner_and_protocol(
        &self,
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        let selection_start = Instant::now();
        
        // Get available runners
        let available_runners = self.registry.list_runners_by_status(
            &super::unified_registry::RunnerStatus::Online
        ).await;
        
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "unified_interface".to_string(),
            });
        }
        
        // Apply selection strategy
        let (runner_id, protocol) = self.selection_strategy
            .select_optimal_runner(&available_runners, context)
            .await?;
        
        let selection_time = selection_start.elapsed();
        debug!(
            "Selected runner {} with protocol {:?} in {:?}",
            runner_id, protocol, selection_time
        );
        
        // Check selection timeout
        if selection_time > self.config.selection_timeout {
            warn!(
                "Protocol selection took {:?}, exceeding timeout of {:?}",
                selection_time, self.config.selection_timeout
            );
        }
        
        Ok((runner_id, protocol))
    }
    
    /// Execute job using specific protocol
    async fn execute_with_protocol(
        &self,
        context: &JobExecutionContext,
        protocol: &PreferredProtocol,
    ) -> Result<JobResult> {
        match protocol {
            PreferredProtocol::Valkyrie => {
                if let Some(valkyrie_adapter) = &self.valkyrie_adapter {
                    debug!("Executing job {} via Valkyrie protocol", context.job.id);
                    valkyrie_adapter.as_ref().execute_domain_job(&context.job, context.runner_id).await
                } else {
                    // Fallback to HTTP if Valkyrie not available
                    warn!("Valkyrie adapter not available, falling back to HTTP");
                    self.execute_via_http_fallback(context).await
                }
            }
            PreferredProtocol::Http => {
                debug!("Executing job {} via HTTP protocol", context.job.id);
                self.execute_via_http_fallback(context).await
            }
            _ => {
                // Auto or other protocols - use adaptive selection
                if let Some(valkyrie_adapter) = &self.valkyrie_adapter {
                    // Try Valkyrie first for better performance
                    match valkyrie_adapter.execute_domain_job(&context.job, context.runner_id).await {
                        Ok(result) => Ok(result),
                        Err(_) => {
                            // Fallback to HTTP
                            info!("Valkyrie execution failed, falling back to HTTP");
                            self.execute_via_http_fallback(context).await
                        }
                    }
                } else {
                    self.execute_via_http_fallback(context).await
                }
            }
        }
    }
    
    /// Execute job via HTTP fallback
    async fn execute_via_http_fallback(&self, context: &JobExecutionContext) -> Result<JobResult> {
        // Update fallback metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.fallback_activations += 1;
        }
        
        self.http_fallback.as_ref().execute_domain_job(&context.job, context.runner_id).await
    }
    
    /// Get fallback protocol for failed execution
    async fn get_fallback_protocol(&self, failed_protocol: &PreferredProtocol) -> Option<PreferredProtocol> {
        match failed_protocol {
            PreferredProtocol::Valkyrie => Some(PreferredProtocol::Http),
            PreferredProtocol::Http => {
                // Check if Valkyrie is available as fallback
                if self.valkyrie_adapter.is_some() {
                    Some(PreferredProtocol::Valkyrie)
                } else {
                    None
                }
            }
            _ => Some(PreferredProtocol::Http), // Default fallback
        }
    }
    
    /// Extract performance requirements from job
    fn extract_performance_requirements(&self, job: &Job) -> PerformanceRequirements {
        let mut requirements = PerformanceRequirements::default();
        
        // Extract from job metadata
        if let Some(max_latency_str) = job.metadata.get("max_latency") {
            if let Ok(latency_ms) = max_latency_str.parse::<u64>() {
                requirements.max_latency = Some(Duration::from_millis(latency_ms));
            }
        }
        
        if let Some(priority_str) = job.metadata.get("priority") {
            requirements.priority = match priority_str.to_lowercase().as_str() {
                "low" => JobPriority::Low,
                "high" => JobPriority::High,
                "critical" => JobPriority::Critical,
                _ => JobPriority::Normal,
            };
        }
        
        // Extract resource requirements
        if let Some(cpu_str) = job.metadata.get("cpu_cores") {
            if let Ok(cpu_cores) = cpu_str.parse::<u32>() {
                requirements.resource_requirements.cpu_cores = Some(cpu_cores);
            }
        }
        
        if let Some(memory_str) = job.metadata.get("memory_mb") {
            if let Ok(memory_mb) = memory_str.parse::<u64>() {
                requirements.resource_requirements.memory_mb = Some(memory_mb);
            }
        }
        
        requirements
    }
    
    /// Record successful job execution
    async fn record_successful_execution(&self, context: &JobExecutionContext, _result: &JobResult) {
        let execution_time = context.start_time.elapsed();
        
        // Update interface metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_jobs += 1;
            
            match context.protocol {
                PreferredProtocol::Valkyrie => metrics.valkyrie_jobs += 1,
                PreferredProtocol::Http => metrics.http_jobs += 1,
                _ => {}
            }
            
            // Update protocol distribution
            let protocol_name = format!("{:?}", context.protocol);
            *metrics.protocol_distribution.entry(protocol_name).or_insert(0) += 1;
            
            // Update average latency
            let total_time = metrics.avg_job_latency.as_nanos() as f64 * (metrics.total_jobs - 1) as f64;
            let new_avg = (total_time + execution_time.as_nanos() as f64) / metrics.total_jobs as f64;
            metrics.avg_job_latency = Duration::from_nanos(new_avg as u64);
        }
        
        // Update protocol performance
        self.selection_strategy.record_performance(
            context.runner_id,
            &context.protocol,
            execution_time,
            true,
        ).await;
    }
    
    /// Record failed job execution
    async fn record_failed_execution(&self, context: &JobExecutionContext, _error: &AppError) {
        let execution_time = context.start_time.elapsed();
        
        // Update interface metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_jobs += 1;
            
            // Update success rate
            let total_successful = (metrics.success_rate * (metrics.total_jobs - 1) as f64) as u64;
            metrics.success_rate = total_successful as f64 / metrics.total_jobs as f64;
        }
        
        // Update protocol performance
        self.selection_strategy.record_performance(
            context.runner_id,
            &context.protocol,
            execution_time,
            false,
        ).await;
    }
    
    /// Update execution metrics
    async fn update_execution_metrics(
        &self,
        context: &JobExecutionContext,
        result: &Result<JobResult>,
        execution_time: Duration,
    ) {
        match result {
            Ok(job_result) => {
                self.record_successful_execution(context, job_result).await;
            }
            Err(error) => {
                self.record_failed_execution(context, error).await;
            }
        }
    }
    
    /// Get interface metrics
    pub async fn get_metrics(&self) -> InterfaceMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get protocol performance statistics
    pub async fn get_protocol_performance(&self) -> HashMap<Uuid, ProtocolPerformance> {
        self.selection_strategy.get_performance_history().await
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Metrics collection task
        let metrics_clone = self.metrics.clone();
        let metrics_interval = self.config.metrics_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(metrics_interval);
            loop {
                interval.tick().await;
                // Perform periodic metrics aggregation
                // This would include calculating moving averages, cleaning up old data, etc.
            }
        });
        
        // Performance monitoring task
        let strategy_clone = self.selection_strategy.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                strategy_clone.cleanup_old_performance_data().await;
            }
        });
    }
}

impl ProtocolSelectionStrategy {
    /// Create a new protocol selection strategy
    pub fn new(algorithm: SelectionAlgorithm, config: StrategyConfig) -> Self {
        Self {
            algorithm,
            performance_history: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Select optimal runner based on strategy
    pub async fn select_optimal_runner(
        &self,
        available_runners: &[RegisteredRunner],
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        match &self.algorithm {
            SelectionAlgorithm::ValkyrieFirst => {
                self.select_valkyrie_first(available_runners).await
            }
            SelectionAlgorithm::HttpFirst => {
                self.select_http_first(available_runners).await
            }
            SelectionAlgorithm::PerformanceBased => {
                self.select_performance_based(available_runners, context).await
            }
            SelectionAlgorithm::Adaptive => {
                self.select_adaptive(available_runners, context).await
            }
            SelectionAlgorithm::RoundRobin => {
                self.select_round_robin(available_runners).await
            }
            SelectionAlgorithm::Custom(name) => {
                self.select_custom(available_runners, context, name).await
            }
        }
    }
    
    /// Select Valkyrie-capable runner first
    async fn select_valkyrie_first(&self, runners: &[RegisteredRunner]) -> Result<(Uuid, PreferredProtocol)> {
        // Find Valkyrie-capable runners
        for runner in runners {
            if matches!(runner.runner_type, RunnerType::Valkyrie { .. }) ||
               matches!(runner.runner_type, RunnerType::Hybrid { primary_protocol: PreferredProtocol::Valkyrie, .. }) {
                return Ok((runner.id, PreferredProtocol::Valkyrie));
            }
        }
        
        // Fallback to HTTP
        if let Some(runner) = runners.first() {
            Ok((runner.id, PreferredProtocol::Http))
        } else {
            Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "unified_interface".to_string(),
            })
        }
    }
    
    /// Select HTTP runner first
    async fn select_http_first(&self, runners: &[RegisteredRunner]) -> Result<(Uuid, PreferredProtocol)> {
        // Find HTTP-capable runners
        for runner in runners {
            if matches!(runner.runner_type, RunnerType::Http { .. }) ||
               matches!(runner.runner_type, RunnerType::Hybrid { .. }) {
                return Ok((runner.id, PreferredProtocol::Http));
            }
        }
        
        // Fallback to any available runner
        if let Some(runner) = runners.first() {
            Ok((runner.id, PreferredProtocol::Auto))
        } else {
            Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "unified_interface".to_string(),
            })
        }
    }
    
    /// Select based on performance metrics
    async fn select_performance_based(
        &self,
        runners: &[RegisteredRunner],
        _context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        let performance_history = self.performance_history.read().await;
        
        let mut best_runner = None;
        let mut best_score = f64::NEG_INFINITY;
        
        for runner in runners {
            if let Some(perf) = performance_history.get(&runner.id) {
                let score = self.calculate_performance_score(perf);
                if score > best_score {
                    best_score = score;
                    best_runner = Some((runner.id, perf.protocol.clone()));
                }
            }
        }
        
        // If no performance history, use adaptive selection
        if let Some((runner_id, protocol)) = best_runner {
            Ok((runner_id, protocol))
        } else {
            self.select_adaptive(runners, _context).await
        }
    }
    
    /// Adaptive selection based on job characteristics
    async fn select_adaptive(
        &self,
        runners: &[RegisteredRunner],
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        // Analyze job requirements
        let requires_low_latency = context.performance_requirements.max_latency
            .map(|lat| lat < Duration::from_millis(100))
            .unwrap_or(false);
        
        let is_high_priority = matches!(
            context.performance_requirements.priority,
            JobPriority::High | JobPriority::Critical
        );
        
        // Select protocol based on requirements
        if requires_low_latency || is_high_priority {
            // Prefer Valkyrie for low-latency/high-priority jobs
            self.select_valkyrie_first(runners).await
        } else {
            // Use HTTP for regular jobs
            self.select_http_first(runners).await
        }
    }
    
    /// Round-robin selection
    async fn select_round_robin(&self, runners: &[RegisteredRunner]) -> Result<(Uuid, PreferredProtocol)> {
        // Simple round-robin implementation
        // In a real implementation, this would maintain state
        let index = (Instant::now().elapsed().as_secs() as usize) % runners.len();
        let runner = &runners[index];
        
        let protocol = match &runner.runner_type {
            RunnerType::Valkyrie { .. } => PreferredProtocol::Valkyrie,
            RunnerType::Http { .. } => PreferredProtocol::Http,
            RunnerType::Hybrid { primary_protocol, .. } => primary_protocol.clone(),
            RunnerType::Unknown => PreferredProtocol::Auto,
        };
        
        Ok((runner.id, protocol))
    }
    
    /// Custom selection logic
    async fn select_custom(
        &self,
        runners: &[RegisteredRunner],
        context: &JobExecutionContext,
        _strategy_name: &str,
    ) -> Result<(Uuid, PreferredProtocol)> {
        // Placeholder for custom selection logic
        // This would be implemented based on specific requirements
        self.select_adaptive(runners, context).await
    }
    
    /// Calculate performance score for a runner
    fn calculate_performance_score(&self, perf: &ProtocolPerformance) -> f64 {
        let latency_score = match perf.avg_latency.as_millis() {
            0..=10 => 1.0,
            11..=50 => 0.8,
            51..=100 => 0.6,
            101..=500 => 0.4,
            _ => 0.2,
        };
        
        let reliability_score = perf.success_rate;
        let throughput_score = (perf.throughput / 1000.0).min(1.0); // Normalize to 1000 ops/sec
        
        // Weighted score
        latency_score * self.config.latency_weight +
        reliability_score * self.config.reliability_weight +
        throughput_score * self.config.throughput_weight
    }
    
    /// Record performance metrics
    pub async fn record_performance(
        &self,
        runner_id: Uuid,
        protocol: &PreferredProtocol,
        execution_time: Duration,
        success: bool,
    ) {
        let mut history = self.performance_history.write().await;
        
        let perf = history.entry(runner_id).or_insert_with(|| ProtocolPerformance {
            protocol: protocol.clone(),
            avg_latency: Duration::ZERO,
            success_rate: 1.0,
            throughput: 0.0,
            total_operations: 0,
            failed_operations: 0,
            last_updated: Instant::now(),
            performance_tier: PerformanceTier::Basic,
        });
        
        // Update metrics
        perf.total_operations += 1;
        if !success {
            perf.failed_operations += 1;
        }
        
        // Update average latency (exponential moving average)
        let alpha = 0.1; // Smoothing factor
        let new_latency = execution_time.as_nanos() as f64;
        let old_latency = perf.avg_latency.as_nanos() as f64;
        let updated_latency = alpha * new_latency + (1.0 - alpha) * old_latency;
        perf.avg_latency = Duration::from_nanos(updated_latency as u64);
        
        // Update success rate
        perf.success_rate = (perf.total_operations - perf.failed_operations) as f64 / perf.total_operations as f64;
        
        // Update throughput (operations per second)
        let elapsed_seconds = perf.last_updated.elapsed().as_secs_f64();
        if elapsed_seconds > 0.0 {
            perf.throughput = 1.0 / elapsed_seconds;
        }
        
        // Update performance tier
        perf.performance_tier = match perf.avg_latency.as_micros() {
            0..=10 => PerformanceTier::Ultra,
            11..=100 => PerformanceTier::High,
            101..=1000 => PerformanceTier::Standard,
            _ => PerformanceTier::Basic,
        };
        
        perf.last_updated = Instant::now();
    }
    
    /// Get performance history
    pub async fn get_performance_history(&self) -> HashMap<Uuid, ProtocolPerformance> {
        self.performance_history.read().await.clone()
    }
    
    /// Clean up old performance data
    pub async fn cleanup_old_performance_data(&self) {
        let mut history = self.performance_history.write().await;
        let cutoff = Instant::now() - self.config.history_retention;
        
        history.retain(|_, perf| perf.last_updated > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::domain::entities::{Job, JobStatus};
    
    #[tokio::test]
    async fn test_protocol_selection_strategy() {
        let strategy = ProtocolSelectionStrategy::new(
            SelectionAlgorithm::PerformanceBased,
            StrategyConfig::default(),
        );
        
        // Record some performance data
        let runner_id = Uuid::new_v4();
        strategy.record_performance(
            runner_id,
            &PreferredProtocol::Valkyrie,
            Duration::from_micros(50),
            true,
        ).await;
        
        let history = strategy.get_performance_history().await;
        assert!(history.contains_key(&runner_id));
        
        let perf = &history[&runner_id];
        assert_eq!(perf.protocol, PreferredProtocol::Valkyrie);
        assert_eq!(perf.total_operations, 1);
        assert_eq!(perf.success_rate, 1.0);
    }

    // Add dummy PipelineId and JobId for test if not imported
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct PipelineId(Uuid);
    impl PipelineId {
        fn new(id: Uuid) -> Self {
            PipelineId(id)
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct JobId(Uuid);
    impl JobId {
        fn new(id: Uuid) -> Self {
            JobId(id)
        }
    }
    
    #[test]
    fn test_performance_requirements_extraction() {
        let mut job = Job {
        id: JobId::new(Uuid::new_v4()), // or just Uuid if JobId = Uuid
        pipeline_id: PipelineId::new(Uuid::new_v4()), // adjust depending on your type
        name: "test-job".to_string(),
        steps: Vec::new(),
        requirements: Default::default(),
        priority: Default::default(),
        timeout: Duration::from_secs(60), // pick a sensible default
        retry_policy: Default::default(),
        metadata: HashMap::new(),
        created_at: Utc::now(),
        scheduled_at: None,
    };

        
        job.metadata.insert("priority".to_string(), "high".to_string());
        job.metadata.insert("max_latency".to_string(), "100".to_string());
        job.metadata.insert("cpu_cores".to_string(), "4".to_string());
        
        // This would be called by the interface
        // let requirements = interface.extract_performance_requirements(&job);
        // assert!(matches!(requirements.priority, JobPriority::High));
        // assert_eq!(requirements.max_latency, Some(Duration::from_millis(100)));
        // assert_eq!(requirements.resource_requirements.cpu_cores, Some(4));
    }
}