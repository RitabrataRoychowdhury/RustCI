use crate::error::{AppError, Result};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Distributed lock manager for preventing race conditions
pub struct DistributedLockManager {
    locks: Arc<DashMap<String, LockInfo>>,
    metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LockInfo {
    pub holder_id: Uuid,
    #[serde(skip)]
    pub acquired_at: Instant,
    #[serde(skip)]
    pub expires_at: Instant,
    pub lock_type: LockType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum LockType {
    Exclusive,
    Shared,
}

impl DistributedLockManager {
    pub fn new(metrics_collector: Option<Arc<super::metrics::MetricsCollector>>) -> Self {
        Self {
            locks: Arc::new(DashMap::new()),
            metrics_collector,
        }
    }

    /// Acquire an exclusive lock
    pub async fn acquire_exclusive_lock(
        &self,
        resource_id: &str,
        holder_id: Uuid,
        timeout: Duration,
        metadata: HashMap<String, String>,
    ) -> Result<LockGuard> {
        let start_time = Instant::now();
        let deadline = start_time + timeout;

        loop {
            // Try to acquire the lock
            if let Some(guard) =
                self.try_acquire_exclusive(resource_id, holder_id, metadata.clone())?
            {
                // Record metrics
                if let Some(metrics) = &self.metrics_collector {
                    let mut labels = HashMap::new();
                    labels.insert("resource_id".to_string(), resource_id.to_string());
                    labels.insert("lock_type".to_string(), "exclusive".to_string());

                    metrics.record_timing(
                        "lock_acquisition_duration_seconds",
                        start_time.elapsed(),
                        labels.clone(),
                    );
                    metrics.increment_counter("locks_acquired_total", labels);
                }

                debug!(
                    resource_id = resource_id,
                    holder_id = %holder_id,
                    duration_ms = start_time.elapsed().as_millis(),
                    "Acquired exclusive lock"
                );

                return Ok(guard);
            }

            // Check timeout
            if Instant::now() >= deadline {
                if let Some(metrics) = &self.metrics_collector {
                    let mut labels = HashMap::new();
                    labels.insert("resource_id".to_string(), resource_id.to_string());
                    labels.insert("reason".to_string(), "timeout".to_string());
                    metrics.increment_counter("lock_acquisition_failures_total", labels);
                }

                return Err(AppError::TimeoutError(format!(
                    "Failed to acquire exclusive lock for resource '{}' within {:?}",
                    resource_id, timeout
                )));
            }

            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Try to acquire an exclusive lock without waiting
    fn try_acquire_exclusive(
        &self,
        resource_id: &str,
        holder_id: Uuid,
        metadata: HashMap<String, String>,
    ) -> Result<Option<LockGuard>> {
        let now = Instant::now();
        let expires_at = now + Duration::from_secs(300); // 5 minutes default

        // Check if resource is already locked
        if let Some(existing_lock) = self.locks.get(resource_id) {
            if existing_lock.expires_at > now {
                // Lock is still valid
                return Ok(None);
            }
            // Lock has expired, we can proceed
        }

        // Acquire the lock
        let lock_info = LockInfo {
            holder_id,
            acquired_at: now,
            expires_at,
            lock_type: LockType::Exclusive,
            metadata,
        };

        self.locks.insert(resource_id.to_string(), lock_info);

        Ok(Some(LockGuard {
            resource_id: resource_id.to_string(),
            holder_id,
            lock_manager: self.clone(),
        }))
    }

    /// Release a lock
    pub fn release_lock(&self, resource_id: &str, holder_id: Uuid) -> Result<()> {
        if let Some((_, lock_info)) = self.locks.remove(resource_id) {
            if lock_info.holder_id == holder_id {
                debug!(
                    resource_id = resource_id,
                    holder_id = %holder_id,
                    duration_ms = lock_info.acquired_at.elapsed().as_millis(),
                    "Released lock"
                );

                // Record metrics
                if let Some(metrics) = &self.metrics_collector {
                    let mut labels = HashMap::new();
                    labels.insert("resource_id".to_string(), resource_id.to_string());
                    labels.insert(
                        "lock_type".to_string(),
                        format!("{:?}", lock_info.lock_type).to_lowercase(),
                    );

                    metrics.record_timing(
                        "lock_hold_duration_seconds",
                        lock_info.acquired_at.elapsed(),
                        labels.clone(),
                    );
                    metrics.increment_counter("locks_released_total", labels);
                }

                Ok(())
            } else {
                // Put the lock back if holder doesn't match
                self.locks.insert(resource_id.to_string(), lock_info);
                Err(AppError::AuthorizationError(format!(
                    "Cannot release lock for resource '{}': holder mismatch",
                    resource_id
                )))
            }
        } else {
            Err(AppError::NotFound(format!(
                "Lock not found for resource '{}'",
                resource_id
            )))
        }
    }

    /// Get information about active locks
    pub fn get_active_locks(&self) -> Vec<(String, LockInfo)> {
        let now = Instant::now();
        self.locks
            .iter()
            .filter(|entry| entry.value().expires_at > now)
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Clean up expired locks
    pub async fn cleanup_expired_locks(&self) {
        let now = Instant::now();
        let expired_keys: Vec<String> = self
            .locks
            .iter()
            .filter(|entry| entry.value().expires_at <= now)
            .map(|entry| entry.key().clone())
            .collect();

        let expired_count = expired_keys.len();
        for key in expired_keys {
            self.locks.remove(&key);
        }

        if expired_count > 0 {
            debug!(expired_count = expired_count, "Cleaned up expired locks");
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let lock_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;
                lock_manager.cleanup_expired_locks().await;
            }
        })
    }
}

impl Clone for DistributedLockManager {
    fn clone(&self) -> Self {
        Self {
            locks: Arc::clone(&self.locks),
            metrics_collector: self.metrics_collector.clone(),
        }
    }
}

/// Lock guard that automatically releases the lock when dropped
pub struct LockGuard {
    resource_id: String,
    holder_id: Uuid,
    lock_manager: DistributedLockManager,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Err(e) = self
            .lock_manager
            .release_lock(&self.resource_id, self.holder_id)
        {
            warn!(
                resource_id = %self.resource_id,
                holder_id = %self.holder_id,
                error = %e,
                "Failed to release lock in drop"
            );
        }
    }
}

/// Rate limiter for preventing resource exhaustion
pub struct RateLimiter {
    buckets: Arc<DashMap<String, TokenBucket>>,
    metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
}

#[derive(Debug)]
pub struct TokenBucket {
    capacity: u32,
    tokens: u32,
    refill_rate: u32, // tokens per second
    last_refill: Instant,
    window_start: Instant,
    requests_in_window: u32,
}

impl TokenBucket {
    pub fn new(capacity: u32, refill_rate: u32) -> Self {
        let now = Instant::now();
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            last_refill: now,
            window_start: now,
            requests_in_window: 0,
        }
    }

    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            self.requests_in_window += 1;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate as f64) as u32;

        if tokens_to_add > 0 {
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }

        // Reset window every minute
        if now.duration_since(self.window_start) >= Duration::from_secs(60) {
            self.window_start = now;
            self.requests_in_window = 0;
        }
    }

    pub fn get_requests_per_minute(&self) -> u32 {
        self.requests_in_window
    }
}

impl RateLimiter {
    pub fn new(metrics_collector: Option<Arc<super::metrics::MetricsCollector>>) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            metrics_collector,
        }
    }

    /// Check if a request is allowed
    pub async fn is_allowed(
        &self,
        key: &str,
        capacity: u32,
        refill_rate: u32,
        tokens_requested: u32,
    ) -> bool {
        let mut bucket = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(capacity, refill_rate));

        let allowed = bucket.try_consume(tokens_requested);

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("key".to_string(), key.to_string());
            labels.insert("allowed".to_string(), allowed.to_string());

            metrics.increment_counter("rate_limit_checks_total", labels.clone());

            if !allowed {
                metrics.increment_counter("rate_limit_rejections_total", labels.clone());
            }

            // Record current rate
            let rpm = bucket.get_requests_per_minute();
            let mut rate_labels = labels;
            rate_labels.insert("window".to_string(), "1m".to_string());
            metrics.set_gauge("rate_limit_requests_per_minute", rpm as f64, rate_labels);
        }

        if !allowed {
            debug!(
                key = key,
                tokens_requested = tokens_requested,
                available_tokens = bucket.tokens,
                "Rate limit exceeded"
            );
        }

        allowed
    }

    /// Get rate limiting statistics
    pub fn get_stats(&self) -> HashMap<String, RateLimitStats> {
        self.buckets
            .iter()
            .map(|entry| {
                let bucket = entry.value();
                let stats = RateLimitStats {
                    capacity: bucket.capacity,
                    available_tokens: bucket.tokens,
                    refill_rate: bucket.refill_rate,
                    requests_per_minute: bucket.get_requests_per_minute(),
                };
                (entry.key().clone(), stats)
            })
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RateLimitStats {
    pub capacity: u32,
    pub available_tokens: u32,
    pub refill_rate: u32,
    pub requests_per_minute: u32,
}

/// Async-safe data structures and utilities
pub struct AsyncSafeCounter {
    value: Arc<RwLock<u64>>,
    metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
    name: String,
}

impl AsyncSafeCounter {
    pub fn new(
        name: String,
        metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
    ) -> Self {
        Self {
            value: Arc::new(RwLock::new(0)),
            metrics_collector,
            name,
        }
    }

    pub async fn increment(&self) -> u64 {
        let mut value = self.value.write().await;
        *value += 1;
        let new_value = *value;

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("counter_name".to_string(), self.name.clone());
            metrics.set_gauge("async_counter_value", new_value as f64, labels);
        }

        new_value
    }

    pub async fn decrement(&self) -> u64 {
        let mut value = self.value.write().await;
        *value = value.saturating_sub(1);
        let new_value = *value;

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("counter_name".to_string(), self.name.clone());
            metrics.set_gauge("async_counter_value", new_value as f64, labels);
        }

        new_value
    }

    pub async fn get(&self) -> u64 {
        *self.value.read().await
    }

    pub async fn set(&self, new_value: u64) {
        let mut value = self.value.write().await;
        *value = new_value;

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("counter_name".to_string(), self.name.clone());
            metrics.set_gauge("async_counter_value", new_value as f64, labels);
        }
    }
}

/// Deadlock detector for identifying potential deadlocks
pub struct DeadlockDetector {
    wait_graph: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    active_threads: Arc<RwLock<HashMap<Uuid, ThreadInfo>>>,
    metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
}

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub thread_id: Uuid,
    pub waiting_for: Option<Uuid>,
    pub holding_locks: Vec<String>,
    pub started_at: Instant,
}

impl DeadlockDetector {
    pub fn new(metrics_collector: Option<Arc<super::metrics::MetricsCollector>>) -> Self {
        Self {
            wait_graph: Arc::new(RwLock::new(HashMap::new())),
            active_threads: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
        }
    }

    /// Register a thread as waiting for another thread
    pub async fn register_wait(&self, waiter: Uuid, waiting_for: Uuid) {
        let mut graph = self.wait_graph.write().await;
        graph
            .entry(waiter)
            .or_insert_with(Vec::new)
            .push(waiting_for);

        let mut threads = self.active_threads.write().await;
        if let Some(thread_info) = threads.get_mut(&waiter) {
            thread_info.waiting_for = Some(waiting_for);
        }

        debug!(
            waiter = %waiter,
            waiting_for = %waiting_for,
            "Registered thread wait relationship"
        );
    }

    /// Remove wait relationship
    pub async fn remove_wait(&self, waiter: Uuid, waiting_for: Uuid) {
        let mut graph = self.wait_graph.write().await;
        if let Some(waiters) = graph.get_mut(&waiter) {
            waiters.retain(|&id| id != waiting_for);
            if waiters.is_empty() {
                graph.remove(&waiter);
            }
        }

        let mut threads = self.active_threads.write().await;
        if let Some(thread_info) = threads.get_mut(&waiter) {
            thread_info.waiting_for = None;
        }
    }

    /// Detect potential deadlocks using cycle detection
    pub async fn detect_deadlocks(&self) -> Vec<Vec<Uuid>> {
        let graph = self.wait_graph.read().await;
        let mut deadlocks = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();

        for &node in graph.keys() {
            if !visited.contains(&node) {
                if let Some(cycle) =
                    self.dfs_cycle_detection(&graph, node, &mut visited, &mut rec_stack)
                {
                    deadlocks.push(cycle);
                }
            }
        }

        if !deadlocks.is_empty() {
            error!(
                deadlock_count = deadlocks.len(),
                "Detected potential deadlocks"
            );

            // Record metrics
            if let Some(metrics) = &self.metrics_collector {
                let labels = HashMap::new();
                metrics.set_gauge("deadlocks_detected_total", deadlocks.len() as f64, labels);
            }
        }

        deadlocks
    }

    fn dfs_cycle_detection(
        &self,
        graph: &HashMap<Uuid, Vec<Uuid>>,
        node: Uuid,
        visited: &mut std::collections::HashSet<Uuid>,
        rec_stack: &mut std::collections::HashSet<Uuid>,
    ) -> Option<Vec<Uuid>> {
        visited.insert(node);
        rec_stack.insert(node);

        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    if let Some(cycle) =
                        self.dfs_cycle_detection(graph, neighbor, visited, rec_stack)
                    {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(&neighbor) {
                    // Found a cycle
                    return Some(vec![node, neighbor]);
                }
            }
        }

        rec_stack.remove(&node);
        None
    }

    /// Start background deadlock detection
    pub fn start_detection_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let detector = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;
                let deadlocks = detector.detect_deadlocks().await;

                if !deadlocks.is_empty() {
                    warn!(
                        deadlock_count = deadlocks.len(),
                        "Deadlocks detected during background check"
                    );
                }
            }
        })
    }
}

impl Clone for DeadlockDetector {
    fn clone(&self) -> Self {
        Self {
            wait_graph: Arc::clone(&self.wait_graph),
            active_threads: Arc::clone(&self.active_threads),
            metrics_collector: self.metrics_collector.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_lock_manager() {
        let lock_manager = DistributedLockManager::new(None);
        let holder_id = Uuid::new_v4();
        let metadata = HashMap::new();

        let lock_guard = lock_manager
            .acquire_exclusive_lock("test_resource", holder_id, Duration::from_secs(1), metadata)
            .await
            .unwrap();

        // Verify lock is held
        let active_locks = lock_manager.get_active_locks();
        assert_eq!(active_locks.len(), 1);
        assert_eq!(active_locks[0].1.holder_id, holder_id);

        drop(lock_guard);

        // Verify lock is released
        let active_locks = lock_manager.get_active_locks();
        assert_eq!(active_locks.len(), 0);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(None);

        // Should allow first request
        assert!(rate_limiter.is_allowed("test_key", 10, 1, 1).await);

        // Should allow up to capacity
        for _ in 0..9 {
            assert!(rate_limiter.is_allowed("test_key", 10, 1, 1).await);
        }

        // Should reject when capacity exceeded
        assert!(!rate_limiter.is_allowed("test_key", 10, 1, 1).await);
    }

    #[tokio::test]
    async fn test_async_safe_counter() {
        let counter = AsyncSafeCounter::new("test_counter".to_string(), None);

        assert_eq!(counter.get().await, 0);
        assert_eq!(counter.increment().await, 1);
        assert_eq!(counter.increment().await, 2);
        assert_eq!(counter.decrement().await, 1);
        assert_eq!(counter.get().await, 1);

        counter.set(10).await;
        assert_eq!(counter.get().await, 10);
    }

    #[tokio::test]
    async fn test_deadlock_detector() {
        let detector = DeadlockDetector::new(None);
        let thread1 = Uuid::new_v4();
        let thread2 = Uuid::new_v4();

        // Create a circular wait
        detector.register_wait(thread1, thread2).await;
        detector.register_wait(thread2, thread1).await;

        let deadlocks = detector.detect_deadlocks().await;
        assert!(!deadlocks.is_empty());
    }
}
