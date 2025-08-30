use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Critical path optimizer for high-performance operations
pub struct CriticalPathOptimizer {
    config: OptimizerConfig,
    metrics: Arc<OptimizationMetrics>,
    batch_processor: Arc<BatchProcessor>,
    cache_optimizer: Arc<CacheOptimizer>,
    compression_engine: Arc<CompressionEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub batch_size: usize,
    pub batch_timeout: Duration,
    pub cache_size: usize,
    pub cache_ttl: Duration,
    pub compression_threshold: usize,
    pub compression_level: u32,
    pub parallel_workers: usize,
    pub metrics_enabled: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout: Duration::from_millis(10),
            cache_size: 10000,
            cache_ttl: Duration::from_secs(300),
            compression_threshold: 1024, // 1KB
            compression_level: 6,
            parallel_workers: num_cpus::get(),
            metrics_enabled: true,
        }
    }
}

/// Optimization metrics for monitoring performance improvements
#[derive(Debug, Default)]
pub struct OptimizationMetrics {
    pub batch_operations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub compression_savings: AtomicU64,
    pub average_batch_size: AtomicU32,
    pub average_processing_time: AtomicU64, // microseconds
    pub total_optimized_operations: AtomicU64,
}

impl OptimizationMetrics {
    pub fn record_batch_operation(&self, batch_size: usize, processing_time: Duration) {
        self.batch_operations.fetch_add(1, Ordering::Relaxed);
        self.total_optimized_operations.fetch_add(batch_size as u64, Ordering::Relaxed);
        
        // Update average batch size
        let current_avg = self.average_batch_size.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            batch_size as u32
        } else {
            (current_avg * 9 + batch_size as u32) / 10
        };
        self.average_batch_size.store(new_avg, Ordering::Relaxed);
        
        // Update average processing time
        let processing_micros = processing_time.as_micros() as u64;
        let current_time_avg = self.average_processing_time.load(Ordering::Relaxed);
        let new_time_avg = if current_time_avg == 0 {
            processing_micros
        } else {
            (current_time_avg * 9 + processing_micros) / 10
        };
        self.average_processing_time.store(new_time_avg, Ordering::Relaxed);
    }
    
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_compression_savings(&self, savings: u64) {
        self.compression_savings.fetch_add(savings, Ordering::Relaxed);
    }
    
    pub fn get_cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

/// Batch processor for optimizing bulk operations
pub struct BatchProcessor {
    config: OptimizerConfig,
    pending_operations: Arc<RwLock<Vec<BatchOperation>>>,
    semaphore: Arc<Semaphore>,
}

#[derive(Debug, Clone)]
pub struct BatchOperation {
    pub id: Uuid,
    pub operation_type: OperationType,
    pub data: Vec<u8>,
    pub created_at: Instant,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OperationType {
    DatabaseWrite,
    DatabaseRead,
    ApiRequest,
    FileOperation,
    NetworkRequest,
}

impl BatchProcessor {
    pub fn new(config: OptimizerConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.parallel_workers));
        
        Self {
            config,
            pending_operations: Arc::new(RwLock::new(Vec::new())),
            semaphore,
        }
    }
    
    /// Add operation to batch
    pub async fn add_operation(&self, operation: BatchOperation) -> Result<(), AppError> {
        let mut pending = self.pending_operations.write().await;
        pending.push(operation);
        
        // Check if we should process the batch
        if pending.len() >= self.config.batch_size {
            let batch = pending.drain(..).collect();
            drop(pending);
            
            // Process batch asynchronously
            let processor = self.clone();
            tokio::spawn(async move {
                if let Err(e) = processor.process_batch(batch).await {
                    tracing::error!("Batch processing failed: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// Process a batch of operations
    async fn process_batch(&self, operations: Vec<BatchOperation>) -> Result<(), AppError> {
        let _permit = self.semaphore.acquire().await
            .map_err(|_| AppError::Internal("Failed to acquire semaphore".to_string()))?;
        
        let start_time = Instant::now();
        
        // Group operations by type for optimal processing
        let mut grouped_ops: HashMap<OperationType, Vec<BatchOperation>> = HashMap::new();
        for op in operations {
            grouped_ops.entry(op.operation_type.clone()).or_default().push(op);
        }
        
        // Process each group in parallel
        let mut handles = Vec::new();
        for (op_type, ops) in grouped_ops {
            let handle = tokio::spawn(async move {
                Self::process_operation_group(op_type, ops).await
            });
            handles.push(handle);
        }
        
        // Wait for all groups to complete
        for handle in handles {
            handle.await
                .map_err(|e| AppError::Internal(format!("Batch processing task failed: {}", e)))??;
        }
        
        let processing_time = start_time.elapsed();
        tracing::debug!("Batch processed in {:?}", processing_time);
        
        Ok(())
    }
    
    /// Process a group of operations of the same type
    async fn process_operation_group(
        op_type: OperationType,
        operations: Vec<BatchOperation>,
    ) -> Result<(), AppError> {
        match op_type {
            OperationType::DatabaseWrite => {
                Self::process_database_writes(operations).await
            }
            OperationType::DatabaseRead => {
                Self::process_database_reads(operations).await
            }
            OperationType::ApiRequest => {
                Self::process_api_requests(operations).await
            }
            OperationType::FileOperation => {
                Self::process_file_operations(operations).await
            }
            OperationType::NetworkRequest => {
                Self::process_network_requests(operations).await
            }
        }
    }
    
    async fn process_database_writes(operations: Vec<BatchOperation>) -> Result<(), AppError> {
        // Simulate batch database write optimization
        tracing::debug!("Processing {} database writes in batch", operations.len());
        
        // In a real implementation, this would:
        // 1. Prepare a batch SQL statement
        // 2. Execute all writes in a single transaction
        // 3. Handle errors and rollback if necessary
        
        tokio::time::sleep(Duration::from_micros(100)).await; // Simulate processing
        Ok(())
    }
    
    async fn process_database_reads(operations: Vec<BatchOperation>) -> Result<(), AppError> {
        tracing::debug!("Processing {} database reads in batch", operations.len());
        
        // In a real implementation, this would:
        // 1. Combine multiple SELECT queries
        // 2. Use prepared statements for efficiency
        // 3. Return results in the correct order
        
        tokio::time::sleep(Duration::from_micros(50)).await; // Simulate processing
        Ok(())
    }
    
    async fn process_api_requests(operations: Vec<BatchOperation>) -> Result<(), AppError> {
        tracing::debug!("Processing {} API requests in batch", operations.len());
        
        // In a real implementation, this would:
        // 1. Group requests by endpoint
        // 2. Use HTTP/2 multiplexing
        // 3. Implement connection pooling
        
        tokio::time::sleep(Duration::from_micros(200)).await; // Simulate processing
        Ok(())
    }
    
    async fn process_file_operations(operations: Vec<BatchOperation>) -> Result<(), AppError> {
        tracing::debug!("Processing {} file operations in batch", operations.len());
        
        // In a real implementation, this would:
        // 1. Group operations by directory
        // 2. Use async I/O for parallel processing
        // 3. Optimize disk access patterns
        
        tokio::time::sleep(Duration::from_micros(300)).await; // Simulate processing
        Ok(())
    }
    
    async fn process_network_requests(operations: Vec<BatchOperation>) -> Result<(), AppError> {
        tracing::debug!("Processing {} network requests in batch", operations.len());
        
        // In a real implementation, this would:
        // 1. Use connection pooling
        // 2. Implement request pipelining
        // 3. Handle retries and timeouts
        
        tokio::time::sleep(Duration::from_micros(150)).await; // Simulate processing
        Ok(())
    }
    
    /// Force process all pending operations
    pub async fn flush(&self) -> Result<(), AppError> {
        let mut pending = self.pending_operations.write().await;
        if !pending.is_empty() {
            let batch = pending.drain(..).collect();
            drop(pending);
            self.process_batch(batch).await?;
        }
        Ok(())
    }
}

impl Clone for BatchProcessor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pending_operations: Arc::clone(&self.pending_operations),
            semaphore: Arc::clone(&self.semaphore),
        }
    }
}

/// Cache optimizer for frequently accessed data
pub struct CacheOptimizer {
    config: OptimizerConfig,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    metrics: Arc<OptimizationMetrics>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    data: Vec<u8>,
    created_at: Instant,
    access_count: u32,
    last_accessed: Instant,
}

impl CacheOptimizer {
    pub fn new(config: OptimizerConfig, metrics: Arc<OptimizationMetrics>) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            metrics,
        }
    }
    
    /// Get data from cache
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.write().await;
        
        if let Some(entry) = cache.get_mut(key) {
            // Check if entry is still valid
            if entry.created_at.elapsed() < self.config.cache_ttl {
                entry.access_count += 1;
                entry.last_accessed = Instant::now();
                self.metrics.record_cache_hit();
                return Some(entry.data.clone());
            } else {
                // Entry expired, remove it
                cache.remove(key);
            }
        }
        
        self.metrics.record_cache_miss();
        None
    }
    
    /// Store data in cache
    pub async fn put(&self, key: String, data: Vec<u8>) -> Result<(), AppError> {
        let mut cache = self.cache.write().await;
        
        // Check cache size limit
        if cache.len() >= self.config.cache_size {
            self.evict_lru(&mut cache).await;
        }
        
        let entry = CacheEntry {
            data,
            created_at: Instant::now(),
            access_count: 1,
            last_accessed: Instant::now(),
        };
        
        cache.insert(key, entry);
        Ok(())
    }
    
    /// Evict least recently used entries
    async fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry>) {
        // Find the least recently used entry
        if let Some((lru_key, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            cache.remove(&lru_key);
        }
    }
    
    /// Clear expired entries
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        
        cache.retain(|_, entry| {
            now.duration_since(entry.created_at) < self.config.cache_ttl
        });
    }
    
    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total_entries = cache.len();
        let total_size: usize = cache.values().map(|entry| entry.data.len()).sum();
        
        CacheStats {
            total_entries,
            total_size,
            hit_rate: self.metrics.get_cache_hit_rate(),
            average_entry_size: if total_entries > 0 {
                total_size / total_entries
            } else {
                0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size: usize,
    pub hit_rate: f64,
    pub average_entry_size: usize,
}

/// Compression engine for reducing data transfer sizes
pub struct CompressionEngine {
    config: OptimizerConfig,
    metrics: Arc<OptimizationMetrics>,
}

impl CompressionEngine {
    pub fn new(config: OptimizerConfig, metrics: Arc<OptimizationMetrics>) -> Self {
        Self { config, metrics }
    }
    
    /// Compress data if it exceeds the threshold
    pub async fn compress(&self, data: &[u8]) -> Result<CompressedData, AppError> {
        if data.len() < self.config.compression_threshold {
            return Ok(CompressedData {
                data: data.to_vec(),
                compressed: false,
                original_size: data.len(),
                compressed_size: data.len(),
                compression_ratio: 1.0,
            });
        }
        
        let start_time = Instant::now();
        
        // Use LZ4 for fast compression
        let compressed = lz4_flex::compress_prepend_size(data);
        
        let compression_time = start_time.elapsed();
        let compression_ratio = data.len() as f64 / compressed.len() as f64;
        let savings = data.len().saturating_sub(compressed.len()) as u64;
        
        self.metrics.record_compression_savings(savings);
        
        tracing::debug!(
            "Compressed {} bytes to {} bytes (ratio: {:.2}) in {:?}",
            data.len(),
            compressed.len(),
            compression_ratio,
            compression_time
        );
        
        let compressed_size = compressed.len();
        Ok(CompressedData {
            data: compressed,
            compressed: true,
            original_size: data.len(),
            compressed_size,
            compression_ratio,
        })
    }
    
    /// Decompress data
    pub async fn decompress(&self, compressed_data: &CompressedData) -> Result<Vec<u8>, AppError> {
        if !compressed_data.compressed {
            return Ok(compressed_data.data.clone());
        }
        
        let start_time = Instant::now();
        
        let decompressed = lz4_flex::decompress_size_prepended(&compressed_data.data)
            .map_err(|e| AppError::Internal(format!("Decompression failed: {}", e)))?;
        
        let decompression_time = start_time.elapsed();
        
        tracing::debug!(
            "Decompressed {} bytes to {} bytes in {:?}",
            compressed_data.compressed_size,
            decompressed.len(),
            decompression_time
        );
        
        Ok(decompressed)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressedData {
    pub data: Vec<u8>,
    pub compressed: bool,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
}

impl CriticalPathOptimizer {
    pub fn new(config: OptimizerConfig) -> Self {
        let metrics = Arc::new(OptimizationMetrics::default());
        let batch_processor = Arc::new(BatchProcessor::new(config.clone()));
        let cache_optimizer = Arc::new(CacheOptimizer::new(config.clone(), Arc::clone(&metrics)));
        let compression_engine = Arc::new(CompressionEngine::new(config.clone(), Arc::clone(&metrics)));
        
        Self {
            config,
            metrics,
            batch_processor,
            cache_optimizer,
            compression_engine,
        }
    }
    
    /// Optimize a batch operation
    pub async fn optimize_batch_operation(
        &self,
        operation: BatchOperation,
    ) -> Result<(), AppError> {
        self.batch_processor.add_operation(operation).await
    }
    
    /// Get cached data with compression support
    pub async fn get_cached_data(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(compressed_data) = self.cache_optimizer.get(key).await {
            // Try to deserialize as CompressedData
            if let Ok(compressed) = bincode::deserialize::<CompressedData>(&compressed_data) {
                if let Ok(decompressed) = self.compression_engine.decompress(&compressed).await {
                    return Some(decompressed);
                }
            }
            // Fallback to raw data
            return Some(compressed_data);
        }
        None
    }
    
    /// Store data in cache with compression
    pub async fn cache_data(&self, key: String, data: Vec<u8>) -> Result<(), AppError> {
        let compressed = self.compression_engine.compress(&data).await?;
        let serialized = bincode::serialize(&compressed)
            .map_err(|e| AppError::Internal(format!("Serialization failed: {}", e)))?;
        
        self.cache_optimizer.put(key, serialized).await
    }
    
    /// Flush all pending operations
    pub async fn flush_all(&self) -> Result<(), AppError> {
        self.batch_processor.flush().await
    }
    
    /// Get optimization metrics
    pub fn get_metrics(&self) -> &OptimizationMetrics {
        &self.metrics
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        self.cache_optimizer.get_stats().await
    }
    
    /// Cleanup expired cache entries
    pub async fn cleanup(&self) {
        self.cache_optimizer.cleanup_expired().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_batch_processor() {
        let config = OptimizerConfig {
            batch_size: 3,
            ..Default::default()
        };
        let processor = BatchProcessor::new(config);
        
        // Add operations to trigger batch processing
        for i in 0..5 {
            let operation = BatchOperation {
                id: Uuid::new_v4(),
                operation_type: OperationType::DatabaseWrite,
                data: format!("data_{}", i).into_bytes(),
                created_at: Instant::now(),
                priority: 1,
            };
            
            processor.add_operation(operation).await.unwrap();
        }
        
        // Allow time for batch processing
        sleep(Duration::from_millis(100)).await;
        
        // Flush remaining operations
        processor.flush().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_cache_optimizer() {
        let config = OptimizerConfig::default();
        let metrics = Arc::new(OptimizationMetrics::default());
        let cache = CacheOptimizer::new(config, metrics);
        
        let key = "test_key".to_string();
        let data = b"test_data".to_vec();
        
        // Store data
        cache.put(key.clone(), data.clone()).await.unwrap();
        
        // Retrieve data
        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved, Some(data));
        
        // Test cache miss
        let missing = cache.get("missing_key").await;
        assert_eq!(missing, None);
    }
    
    #[tokio::test]
    async fn test_compression_engine() {
        let config = OptimizerConfig {
            compression_threshold: 10,
            ..Default::default()
        };
        let metrics = Arc::new(OptimizationMetrics::default());
        let engine = CompressionEngine::new(config, metrics);
        
        let data = b"This is a test string that should be compressed because it's longer than the threshold";
        
        // Compress data
        let compressed = engine.compress(data).await.unwrap();
        assert!(compressed.compressed);
        assert!(compressed.compressed_size < compressed.original_size);
        
        // Decompress data
        let decompressed = engine.decompress(&compressed).await.unwrap();
        assert_eq!(decompressed, data);
    }
    
    #[tokio::test]
    async fn test_critical_path_optimizer() {
        let config = OptimizerConfig::default();
        let optimizer = CriticalPathOptimizer::new(config);
        
        let key = "test_key".to_string();
        let data = b"This is test data for the critical path optimizer".to_vec();
        
        // Cache data with compression
        optimizer.cache_data(key.clone(), data.clone()).await.unwrap();
        
        // Retrieve cached data
        let retrieved = optimizer.get_cached_data(&key).await;
        assert_eq!(retrieved, Some(data));
        
        // Test batch operation
        let operation = BatchOperation {
            id: Uuid::new_v4(),
            operation_type: OperationType::ApiRequest,
            data: b"batch_data".to_vec(),
            created_at: Instant::now(),
            priority: 1,
        };
        
        optimizer.optimize_batch_operation(operation).await.unwrap();
        optimizer.flush_all().await.unwrap();
    }
}