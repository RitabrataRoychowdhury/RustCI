use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, mpsc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::core::performance::{
    CriticalPathOptimizer, OptimizerConfig, BatchOperation, OperationType
};
use super::valkyrie_adapter::{ValkyrieJob, JobId, JobPriority};

/// Performance optimizer specifically for Valkyrie adapter operations
pub struct ValkyriePerformanceOptimizer {
    config: ValkyrieOptimizerConfig,
    critical_path_optimizer: Arc<CriticalPathOptimizer>,
    job_dispatcher: Arc<OptimizedJobDispatcher>,
    connection_pool: Arc<ValkyrieConnectionPool>,
    metrics: Arc<ValkyriePerformanceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieOptimizerConfig {
    pub max_concurrent_dispatches: usize,
    pub dispatch_batch_size: usize,
    pub dispatch_timeout: Duration,
    pub connection_pool_size: usize,
    pub connection_timeout: Duration,
    pub enable_zero_copy: bool,
    pub enable_simd_processing: bool,
    pub enable_adaptive_batching: bool,
    pub performance_monitoring: bool,
}

impl Default for ValkyrieOptimizerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_dispatches: 1000,
            dispatch_batch_size: 50,
            dispatch_timeout: Duration::from_micros(100),
            connection_pool_size: 100,
            connection_timeout: Duration::from_millis(5),
            enable_zero_copy: true,
            enable_simd_processing: true,
            enable_adaptive_batching: true,
            performance_monitoring: true,
        }
    }
}

/// Performance metrics specific to Valkyrie operations
#[derive(Debug, Default)]
pub struct ValkyriePerformanceMetrics {
    pub total_jobs_processed: AtomicU64,
    pub total_dispatch_time: AtomicU64, // microseconds
    pub average_dispatch_latency: AtomicU64, // microseconds
    pub zero_copy_operations: AtomicU64,
    pub simd_operations: AtomicU64,
    pub batch_optimizations: AtomicU64,
    pub connection_pool_hits: AtomicU64,
    pub connection_pool_misses: AtomicU64,
    pub peak_concurrent_jobs: AtomicU32,
    pub current_active_jobs: AtomicU32,
}

impl ValkyriePerformanceMetrics {
    pub fn record_job_dispatch(&self, dispatch_time: Duration, used_zero_copy: bool, used_simd: bool) {
        self.total_jobs_processed.fetch_add(1, Ordering::Relaxed);
        
        let dispatch_micros = dispatch_time.as_micros() as u64;
        self.total_dispatch_time.fetch_add(dispatch_micros, Ordering::Relaxed);
        
        // Update average dispatch latency using exponential moving average
        let current_avg = self.average_dispatch_latency.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            dispatch_micros
        } else {
            (current_avg * 9 + dispatch_micros) / 10
        };
        self.average_dispatch_latency.store(new_avg, Ordering::Relaxed);
        
        if used_zero_copy {
            self.zero_copy_operations.fetch_add(1, Ordering::Relaxed);
        }
        
        if used_simd {
            self.simd_operations.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update concurrent job tracking
        let current_active = self.current_active_jobs.fetch_add(1, Ordering::Relaxed) + 1;
        let peak = self.peak_concurrent_jobs.load(Ordering::Relaxed);
        if current_active > peak {
            self.peak_concurrent_jobs.store(current_active, Ordering::Relaxed);
        }
    }
    
    pub fn record_job_completion(&self) {
        self.current_active_jobs.fetch_sub(1, Ordering::Relaxed);
    }
    
    pub fn record_batch_optimization(&self) {
        self.batch_optimizations.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_connection_pool_hit(&self) {
        self.connection_pool_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_connection_pool_miss(&self) {
        self.connection_pool_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        let total_jobs = self.total_jobs_processed.load(Ordering::Relaxed);
        let total_time = self.total_dispatch_time.load(Ordering::Relaxed);
        let zero_copy_ops = self.zero_copy_operations.load(Ordering::Relaxed);
        let simd_ops = self.simd_operations.load(Ordering::Relaxed);
        let pool_hits = self.connection_pool_hits.load(Ordering::Relaxed);
        let pool_misses = self.connection_pool_misses.load(Ordering::Relaxed);
        
        PerformanceSummary {
            total_jobs_processed: total_jobs,
            average_dispatch_latency: Duration::from_micros(
                self.average_dispatch_latency.load(Ordering::Relaxed)
            ),
            zero_copy_utilization: if total_jobs > 0 {
                zero_copy_ops as f64 / total_jobs as f64
            } else {
                0.0
            },
            simd_utilization: if total_jobs > 0 {
                simd_ops as f64 / total_jobs as f64
            } else {
                0.0
            },
            connection_pool_hit_rate: if pool_hits + pool_misses > 0 {
                pool_hits as f64 / (pool_hits + pool_misses) as f64
            } else {
                0.0
            },
            peak_concurrent_jobs: self.peak_concurrent_jobs.load(Ordering::Relaxed),
            current_active_jobs: self.current_active_jobs.load(Ordering::Relaxed),
            batch_optimizations: self.batch_optimizations.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_jobs_processed: u64,
    pub average_dispatch_latency: Duration,
    pub zero_copy_utilization: f64,
    pub simd_utilization: f64,
    pub connection_pool_hit_rate: f64,
    pub peak_concurrent_jobs: u32,
    pub current_active_jobs: u32,
    pub batch_optimizations: u64,
}

/// Optimized job dispatcher with advanced performance features
pub struct OptimizedJobDispatcher {
    config: ValkyrieOptimizerConfig,
    dispatch_semaphore: Arc<Semaphore>,
    pending_jobs: Arc<RwLock<Vec<OptimizedJob>>>,
    batch_sender: mpsc::UnboundedSender<Vec<OptimizedJob>>,
    metrics: Arc<ValkyriePerformanceMetrics>,
}

#[derive(Debug, Clone)]
pub struct OptimizedJob {
    pub job: ValkyrieJob,
    pub dispatch_started: Instant,
    pub optimization_flags: OptimizationFlags,
    pub priority_boost: u32,
}

#[derive(Debug, Clone, Default)]
pub struct OptimizationFlags {
    pub use_zero_copy: bool,
    pub use_simd: bool,
    pub use_compression: bool,
    pub use_batching: bool,
}

impl OptimizedJobDispatcher {
    pub fn new(
        config: ValkyrieOptimizerConfig,
        metrics: Arc<ValkyriePerformanceMetrics>,
    ) -> Self {
        let dispatch_semaphore = Arc::new(Semaphore::new(config.max_concurrent_dispatches));
        let (batch_sender, mut batch_receiver) = mpsc::unbounded_channel();
        
        // Start batch processing task
        let batch_config = config.clone();
        let batch_metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            while let Some(batch) = batch_receiver.recv().await {
                if let Err(e) = Self::process_job_batch(batch, &batch_config, &batch_metrics).await {
                    tracing::error!("Batch processing failed: {}", e);
                }
            }
        });
        
        Self {
            config,
            dispatch_semaphore,
            pending_jobs: Arc::new(RwLock::new(Vec::new())),
            batch_sender,
            metrics,
        }
    }
    
    /// Dispatch a job with optimizations
    pub async fn dispatch_job(&self, job: ValkyrieJob) -> Result<(), AppError> {
        let optimization_flags = self.determine_optimizations(&job).await;
        
        let optimized_job = OptimizedJob {
            job,
            dispatch_started: Instant::now(),
            optimization_flags,
            priority_boost: 0,
        };
        
        if self.config.enable_adaptive_batching {
            self.add_to_batch(optimized_job).await?;
        } else {
            self.dispatch_single_job(optimized_job).await?;
        }
        
        Ok(())
    }
    
    /// Add job to batch for optimized processing
    async fn add_to_batch(&self, job: OptimizedJob) -> Result<(), AppError> {
        let mut pending = self.pending_jobs.write().await;
        pending.push(job);
        
        // Check if we should process the batch
        if pending.len() >= self.config.dispatch_batch_size {
            let batch = pending.drain(..).collect();
            drop(pending);
            
            self.batch_sender.send(batch)
                .map_err(|_| AppError::Internal("Batch sender closed".to_string()))?;
            
            self.metrics.record_batch_optimization();
        }
        
        Ok(())
    }
    
    /// Process a batch of jobs
    async fn process_job_batch(
        batch: Vec<OptimizedJob>,
        config: &ValkyrieOptimizerConfig,
        metrics: &ValkyriePerformanceMetrics,
    ) -> Result<(), AppError> {
        let batch_start = Instant::now();
        
        // Group jobs by optimization flags for efficient processing
        let mut grouped_jobs: HashMap<String, Vec<OptimizedJob>> = HashMap::new();
        for job in batch {
            let key = format!(
                "zc:{}_simd:{}_comp:{}",
                job.optimization_flags.use_zero_copy,
                job.optimization_flags.use_simd,
                job.optimization_flags.use_compression
            );
            grouped_jobs.entry(key).or_default().push(job);
        }
        
        // Process each group in parallel
        let mut handles = Vec::new();
        for (_, jobs) in grouped_jobs {
            let config_clone = config.clone();
            let metrics_clone = Arc::clone(metrics);
            
            let handle = tokio::spawn(async move {
                Self::process_job_group(jobs, &config_clone, &metrics_clone).await
            });
            handles.push(handle);
        }
        
        // Wait for all groups to complete
        for handle in handles {
            handle.await
                .map_err(|e| AppError::Internal(format!("Job group processing failed: {}", e)))??;
        }
        
        let batch_time = batch_start.elapsed();
        tracing::debug!("Processed job batch in {:?}", batch_time);
        
        Ok(())
    }
    
    /// Process a group of jobs with similar optimization flags
    async fn process_job_group(
        jobs: Vec<OptimizedJob>,
        config: &ValkyrieOptimizerConfig,
        metrics: &ValkyriePerformanceMetrics,
    ) -> Result<(), AppError> {
        for job in jobs {
            let dispatch_time = job.dispatch_started.elapsed();
            
            // Simulate optimized job processing
            if config.enable_zero_copy && job.optimization_flags.use_zero_copy {
                Self::process_with_zero_copy(&job).await?;
            }
            
            if config.enable_simd_processing && job.optimization_flags.use_simd {
                Self::process_with_simd(&job).await?;
            }
            
            // Record metrics
            metrics.record_job_dispatch(
                dispatch_time,
                job.optimization_flags.use_zero_copy,
                job.optimization_flags.use_simd,
            );
            
            // Simulate job completion
            tokio::time::sleep(Duration::from_micros(50)).await;
            metrics.record_job_completion();
        }
        
        Ok(())
    }
    
    /// Process job with zero-copy optimizations
    async fn process_with_zero_copy(job: &OptimizedJob) -> Result<(), AppError> {
        // Simulate zero-copy processing
        tracing::debug!("Processing job {} with zero-copy optimization", job.job.id);
        
        // In a real implementation, this would:
        // 1. Use memory-mapped files for large payloads
        // 2. Implement zero-copy network transfers
        // 3. Use shared memory for inter-process communication
        
        tokio::time::sleep(Duration::from_micros(10)).await;
        Ok(())
    }
    
    /// Process job with SIMD optimizations
    async fn process_with_simd(job: &OptimizedJob) -> Result<(), AppError> {
        // Simulate SIMD processing
        tracing::debug!("Processing job {} with SIMD optimization", job.job.id);
        
        // In a real implementation, this would:
        // 1. Use SIMD instructions for data processing
        // 2. Vectorize operations on job payloads
        // 3. Optimize serialization/deserialization
        
        tokio::time::sleep(Duration::from_micros(5)).await;
        Ok(())
    }
    
    /// Dispatch a single job without batching
    async fn dispatch_single_job(&self, job: OptimizedJob) -> Result<(), AppError> {
        let _permit = self.dispatch_semaphore.acquire().await
            .map_err(|_| AppError::Internal("Failed to acquire dispatch permit".to_string()))?;
        
        let dispatch_time = job.dispatch_started.elapsed();
        
        // Process the job with optimizations
        if self.config.enable_zero_copy && job.optimization_flags.use_zero_copy {
            Self::process_with_zero_copy(&job).await?;
        }
        
        if self.config.enable_simd_processing && job.optimization_flags.use_simd {
            Self::process_with_simd(&job).await?;
        }
        
        // Record metrics
        self.metrics.record_job_dispatch(
            dispatch_time,
            job.optimization_flags.use_zero_copy,
            job.optimization_flags.use_simd,
        );
        
        // Simulate job completion
        tokio::time::sleep(Duration::from_micros(50)).await;
        self.metrics.record_job_completion();
        
        Ok(())
    }
    
    /// Determine optimal optimization flags for a job
    async fn determine_optimizations(&self, job: &ValkyrieJob) -> OptimizationFlags {
        let mut flags = OptimizationFlags::default();
        
        // Enable zero-copy for large payloads
        if let Some(payload_size) = self.estimate_payload_size(job) {
            flags.use_zero_copy = payload_size > 1024; // 1KB threshold
        }
        
        // Enable SIMD for data-intensive operations
        flags.use_simd = matches!(job.job_type, super::valkyrie_adapter::JobType::Analysis);
        
        // Enable compression for network transfers
        flags.use_compression = job.priority <= JobPriority::Normal;
        
        // Enable batching for non-critical jobs
        flags.use_batching = job.priority >= JobPriority::Normal;
        
        flags
    }
    
    /// Estimate payload size for optimization decisions
    fn estimate_payload_size(&self, job: &ValkyrieJob) -> Option<usize> {
        match &job.payload {
            super::valkyrie_adapter::JobPayload::Small(data) => Some(data.len()),
            super::valkyrie_adapter::JobPayload::Large(large) => Some(large.size as usize),
            super::valkyrie_adapter::JobPayload::Stream(_) => Some(8192), // Estimate for streams
        }
    }
    
    /// Flush all pending jobs
    pub async fn flush(&self) -> Result<(), AppError> {
        let mut pending = self.pending_jobs.write().await;
        if !pending.is_empty() {
            let batch = pending.drain(..).collect();
            drop(pending);
            
            self.batch_sender.send(batch)
                .map_err(|_| AppError::Internal("Batch sender closed".to_string()))?;
        }
        Ok(())
    }
}

/// Optimized connection pool for Valkyrie connections
pub struct ValkyrieConnectionPool {
    config: ValkyrieOptimizerConfig,
    available_connections: Arc<RwLock<Vec<ValkyrieConnection>>>,
    active_connections: Arc<RwLock<HashMap<Uuid, ValkyrieConnection>>>,
    metrics: Arc<ValkyriePerformanceMetrics>,
}

#[derive(Debug, Clone)]
pub struct ValkyrieConnection {
    pub id: Uuid,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: u32,
    pub is_healthy: bool,
}

impl ValkyrieConnectionPool {
    pub fn new(
        config: ValkyrieOptimizerConfig,
        metrics: Arc<ValkyriePerformanceMetrics>,
    ) -> Self {
        Self {
            config,
            available_connections: Arc::new(RwLock::new(Vec::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            metrics,
        }
    }
    
    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<ValkyrieConnection, AppError> {
        // Try to get an available connection
        {
            let mut available = self.available_connections.write().await;
            if let Some(mut connection) = available.pop() {
                connection.last_used = Instant::now();
                connection.use_count += 1;
                
                let connection_id = connection.id;
                let mut active = self.active_connections.write().await;
                active.insert(connection_id, connection.clone());
                
                self.metrics.record_connection_pool_hit();
                return Ok(connection);
            }
        }
        
        // No available connections, create a new one
        let connection = ValkyrieConnection {
            id: Uuid::new_v4(),
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 1,
            is_healthy: true,
        };
        
        let connection_id = connection.id;
        let mut active = self.active_connections.write().await;
        active.insert(connection_id, connection.clone());
        
        self.metrics.record_connection_pool_miss();
        Ok(connection)
    }
    
    /// Return a connection to the pool
    pub async fn return_connection(&self, connection: ValkyrieConnection) -> Result<(), AppError> {
        let mut active = self.active_connections.write().await;
        active.remove(&connection.id);
        
        // Check if connection is still healthy and within limits
        if connection.is_healthy && connection.use_count < 1000 {
            let mut available = self.available_connections.write().await;
            if available.len() < self.config.connection_pool_size {
                available.push(connection);
            }
        }
        
        Ok(())
    }
    
    /// Cleanup expired connections
    pub async fn cleanup_expired(&self) {
        let mut available = self.available_connections.write().await;
        let now = Instant::now();
        
        available.retain(|conn| {
            now.duration_since(conn.last_used) < Duration::from_secs(300) // 5 minutes
        });
    }
}

impl ValkyriePerformanceOptimizer {
    pub fn new(config: ValkyrieOptimizerConfig) -> Self {
        let metrics = Arc::new(ValkyriePerformanceMetrics::default());
        
        let optimizer_config = OptimizerConfig {
            batch_size: config.dispatch_batch_size,
            batch_timeout: config.dispatch_timeout,
            parallel_workers: config.max_concurrent_dispatches,
            ..Default::default()
        };
        
        let critical_path_optimizer = Arc::new(CriticalPathOptimizer::new(optimizer_config));
        let job_dispatcher = Arc::new(OptimizedJobDispatcher::new(config.clone(), Arc::clone(&metrics)));
        let connection_pool = Arc::new(ValkyrieConnectionPool::new(config.clone(), Arc::clone(&metrics)));
        
        Self {
            config,
            critical_path_optimizer,
            job_dispatcher,
            connection_pool,
            metrics,
        }
    }
    
    /// Dispatch a job with full optimization
    pub async fn dispatch_optimized_job(&self, job: ValkyrieJob) -> Result<(), AppError> {
        // Create batch operation for critical path optimization
        let batch_operation = BatchOperation {
            id: job.id,
            operation_type: OperationType::ApiRequest,
            data: bincode::serialize(&job)
                .map_err(|e| AppError::Internal(format!("Job serialization failed: {}", e)))?,
            created_at: Instant::now(),
            priority: job.priority as u32,
        };
        
        // Add to critical path optimizer
        self.critical_path_optimizer.optimize_batch_operation(batch_operation).await?;
        
        // Dispatch through optimized dispatcher
        self.job_dispatcher.dispatch_job(job).await?;
        
        Ok(())
    }
    
    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceSummary {
        self.metrics.get_performance_summary()
    }
    
    /// Flush all pending operations
    pub async fn flush_all(&self) -> Result<(), AppError> {
        self.critical_path_optimizer.flush_all().await?;
        self.job_dispatcher.flush().await?;
        Ok(())
    }
    
    /// Cleanup resources
    pub async fn cleanup(&self) {
        self.critical_path_optimizer.cleanup().await;
        self.connection_pool.cleanup_expired().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::valkyrie_adapter::{ValkyrieJob, JobType, JobPriority, JobPayload};
    
    #[tokio::test]
    async fn test_valkyrie_performance_optimizer() {
        let config = ValkyrieOptimizerConfig::default();
        let optimizer = ValkyriePerformanceOptimizer::new(config);
        
        let job = ValkyrieJob {
            id: Uuid::new_v4(),
            job_type: JobType::Build,
            priority: JobPriority::Normal,
            payload: JobPayload::Small(b"test_payload".to_vec()),
            ..Default::default()
        };
        
        // Dispatch job with optimizations
        optimizer.dispatch_optimized_job(job).await.unwrap();
        
        // Flush all operations
        optimizer.flush_all().await.unwrap();
        
        // Get performance metrics
        let metrics = optimizer.get_performance_metrics();
        assert!(metrics.total_jobs_processed > 0);
    }
    
    #[tokio::test]
    async fn test_connection_pool() {
        let config = ValkyrieOptimizerConfig::default();
        let metrics = Arc::new(ValkyriePerformanceMetrics::default());
        let pool = ValkyrieConnectionPool::new(config, metrics);
        
        // Get a connection
        let conn1 = pool.get_connection().await.unwrap();
        assert!(conn1.is_healthy);
        
        // Return the connection
        pool.return_connection(conn1).await.unwrap();
        
        // Get another connection (should reuse the returned one)
        let conn2 = pool.get_connection().await.unwrap();
        assert!(conn2.is_healthy);
    }
    
    #[tokio::test]
    async fn test_job_dispatcher_batching() {
        let config = ValkyrieOptimizerConfig {
            dispatch_batch_size: 3,
            enable_adaptive_batching: true,
            ..Default::default()
        };
        let metrics = Arc::new(ValkyriePerformanceMetrics::default());
        let dispatcher = OptimizedJobDispatcher::new(config, metrics);
        
        // Dispatch multiple jobs to trigger batching
        for i in 0..5 {
            let job = ValkyrieJob {
                id: Uuid::new_v4(),
                job_type: JobType::Test,
                priority: JobPriority::Normal,
                payload: JobPayload::Small(format!("payload_{}", i).into_bytes()),
                ..Default::default()
            };
            
            dispatcher.dispatch_job(job).await.unwrap();
        }
        
        // Flush remaining jobs
        dispatcher.flush().await.unwrap();
        
        // Allow time for processing
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}