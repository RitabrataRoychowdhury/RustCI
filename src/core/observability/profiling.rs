use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

fn serialize_duration<S>(duration: &Duration, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(duration.as_millis() as u64)
}

/// Performance profiler for tracking application performance
pub struct PerformanceProfiler {
    active_profiles: Arc<RwLock<HashMap<Uuid, ProfileSession>>>,
    completed_profiles: Arc<RwLock<Vec<ProfileResult>>>,
    metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
}

#[derive(Debug, Clone)]
pub struct ProfileSession {
    pub id: Uuid,
    pub name: String,
    pub start_time: Instant,
    pub checkpoints: Vec<ProfileCheckpoint>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileCheckpoint {
    pub name: String,
    #[serde(skip)]
    pub timestamp: Instant,
    #[serde(serialize_with = "serialize_duration")]
    pub duration_from_start: Duration,
    pub memory_usage: Option<u64>,
    pub cpu_usage: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileResult {
    pub id: Uuid,
    pub name: String,
    pub total_duration: Duration,
    pub checkpoints: Vec<ProfileCheckpoint>,
    pub metadata: HashMap<String, String>,
    pub peak_memory: Option<u64>,
    pub average_cpu: Option<f64>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

impl PerformanceProfiler {
    pub fn new(metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>) -> Self {
        Self {
            active_profiles: Arc::new(RwLock::new(HashMap::new())),
            completed_profiles: Arc::new(RwLock::new(Vec::new())),
            metrics_collector,
        }
    }

    /// Start a new profiling session
    pub async fn start_profile(&self, name: String, metadata: HashMap<String, String>) -> Uuid {
        let session = ProfileSession {
            id: Uuid::new_v4(),
            name: name.clone(),
            start_time: Instant::now(),
            checkpoints: Vec::new(),
            metadata,
        };

        let session_id = session.id;
        let mut profiles = self.active_profiles.write().await;
        profiles.insert(session_id, session);

        debug!(
            profile_id = %session_id,
            profile_name = %name,
            "Started performance profiling session"
        );

        session_id
    }

    /// Add a checkpoint to an active profiling session
    pub async fn add_checkpoint(&self, profile_id: Uuid, checkpoint_name: String) -> Result<()> {
        let mut profiles = self.active_profiles.write().await;

        if let Some(session) = profiles.get_mut(&profile_id) {
            let now = Instant::now();
            let checkpoint = ProfileCheckpoint {
                name: checkpoint_name.clone(),
                timestamp: now,
                duration_from_start: now.duration_since(session.start_time),
                memory_usage: self.get_current_memory_usage(),
                cpu_usage: self.get_current_cpu_usage(),
            };

            session.checkpoints.push(checkpoint);

            debug!(
                profile_id = %profile_id,
                checkpoint = %checkpoint_name,
                duration_ms = now.duration_since(session.start_time).as_millis(),
                "Added profiling checkpoint"
            );

            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Profile session {} not found",
                profile_id
            )))
        }
    }

    /// Finish a profiling session
    pub async fn finish_profile(&self, profile_id: Uuid) -> Result<ProfileResult> {
        let session = {
            let mut profiles = self.active_profiles.write().await;
            profiles.remove(&profile_id).ok_or_else(|| {
                AppError::NotFound(format!("Profile session {} not found", profile_id))
            })?
        };

        let total_duration = session.start_time.elapsed();
        let peak_memory = session
            .checkpoints
            .iter()
            .filter_map(|c| c.memory_usage)
            .max();
        let average_cpu = if !session.checkpoints.is_empty() {
            let sum: f64 = session.checkpoints.iter().filter_map(|c| c.cpu_usage).sum();
            let count = session
                .checkpoints
                .iter()
                .filter(|c| c.cpu_usage.is_some())
                .count();
            if count > 0 {
                Some(sum / count as f64)
            } else {
                None
            }
        } else {
            None
        };

        let checkpoints_len = session.checkpoints.len();
        let result = ProfileResult {
            id: session.id,
            name: session.name.clone(),
            total_duration,
            checkpoints: session.checkpoints,
            metadata: session.metadata,
            peak_memory,
            average_cpu,
            completed_at: chrono::Utc::now(),
        };

        // Store completed profile
        let mut completed = self.completed_profiles.write().await;
        completed.push(result.clone());

        // Keep only last 100 completed profiles
        if completed.len() > 100 {
            completed.remove(0);
        }

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("profile_name".to_string(), session.name.clone());

            metrics.record_timing("profile_duration_seconds", total_duration, labels.clone());
            metrics.increment_counter("profiles_completed_total", labels.clone());

            if let Some(peak_mem) = peak_memory {
                metrics.set_gauge("profile_peak_memory_bytes", peak_mem as f64, labels.clone());
            }

            if let Some(avg_cpu) = average_cpu {
                metrics.set_gauge("profile_average_cpu_percent", avg_cpu, labels);
            }
        }

        info!(
            profile_id = %profile_id,
            profile_name = %session.name,
            duration_ms = total_duration.as_millis(),
            checkpoints = checkpoints_len,
            peak_memory_mb = peak_memory.map(|m| m / 1024 / 1024),
            average_cpu = average_cpu,
            "Completed performance profiling session"
        );

        Ok(result)
    }

    /// Get active profiling sessions
    pub async fn get_active_profiles(&self) -> Vec<ProfileSession> {
        let profiles = self.active_profiles.read().await;
        profiles.values().cloned().collect()
    }

    /// Get completed profiling results
    pub async fn get_completed_profiles(&self) -> Vec<ProfileResult> {
        self.completed_profiles.read().await.clone()
    }

    /// Profile a future operation
    pub async fn profile_operation<F, T>(
        &self,
        name: String,
        metadata: HashMap<String, String>,
        operation: F,
    ) -> Result<(T, ProfileResult)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let profile_id = self.start_profile(name, metadata).await;

        self.add_checkpoint(profile_id, "operation_start".to_string())
            .await?;

        let result = operation.await;

        self.add_checkpoint(profile_id, "operation_end".to_string())
            .await?;

        let profile_result = self.finish_profile(profile_id).await?;

        match result {
            Ok(value) => Ok((value, profile_result)),
            Err(e) => {
                warn!(
                    profile_id = %profile_id,
                    error = %e,
                    "Profiled operation failed"
                );
                Err(e)
            }
        }
    }

    /// Get current memory usage (simplified implementation)
    fn get_current_memory_usage(&self) -> Option<u64> {
        use sysinfo::{ProcessExt, System, SystemExt};

        let mut system = System::new();
        system.refresh_processes();

        system.process(sysinfo::get_current_pid().ok()?).map(|process| process.memory())
    }

    /// Get current CPU usage (simplified implementation)
    fn get_current_cpu_usage(&self) -> Option<f64> {
        use sysinfo::{ProcessExt, System, SystemExt};

        let mut system = System::new();
        system.refresh_processes();

        system.process(sysinfo::get_current_pid().ok()?).map(|process| process.cpu_usage() as f64)
    }

    /// Generate performance report
    pub async fn generate_report(&self) -> PerformanceReport {
        let active_profiles = self.get_active_profiles().await;
        let completed_profiles = self.get_completed_profiles().await;

        let total_completed = completed_profiles.len();
        let average_duration = if !completed_profiles.is_empty() {
            let total_duration: Duration =
                completed_profiles.iter().map(|p| p.total_duration).sum();
            Some(total_duration / total_completed as u32)
        } else {
            None
        };

        let slowest_profile = completed_profiles
            .iter()
            .max_by_key(|p| p.total_duration)
            .cloned();

        let fastest_profile = completed_profiles
            .iter()
            .min_by_key(|p| p.total_duration)
            .cloned();

        PerformanceReport {
            active_sessions: active_profiles.len(),
            completed_sessions: total_completed,
            average_duration,
            slowest_profile,
            fastest_profile,
            generated_at: chrono::Utc::now(),
        }
    }
}

/// Performance report
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceReport {
    pub active_sessions: usize,
    pub completed_sessions: usize,
    pub average_duration: Option<Duration>,
    pub slowest_profile: Option<ProfileResult>,
    pub fastest_profile: Option<ProfileResult>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Memory profiler for tracking memory usage patterns
pub struct MemoryProfiler {
    snapshots: Arc<RwLock<Vec<MemorySnapshot>>>,
    metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemorySnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub total_memory: u64,
    pub used_memory: u64,
    pub available_memory: u64,
    pub process_memory: Option<u64>,
    pub heap_size: Option<u64>,
}

impl MemoryProfiler {
    pub fn new(metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>) -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(Vec::new())),
            metrics_collector,
        }
    }

    /// Take a memory snapshot
    pub async fn take_snapshot(&self) -> MemorySnapshot {
        use sysinfo::{ProcessExt, System, SystemExt};

        let mut system = System::new_all();
        system.refresh_all();

        let process_memory = system
            .process(sysinfo::get_current_pid().unwrap_or(sysinfo::Pid::from(0)))
            .map(|p| p.memory());

        let snapshot = MemorySnapshot {
            timestamp: chrono::Utc::now(),
            total_memory: system.total_memory(),
            used_memory: system.used_memory(),
            available_memory: system.available_memory(),
            process_memory,
            heap_size: None, // Would need more sophisticated heap tracking
        };

        // Store snapshot
        let mut snapshots = self.snapshots.write().await;
        snapshots.push(snapshot.clone());

        // Keep only last 1000 snapshots
        if snapshots.len() > 1000 {
            snapshots.remove(0);
        }

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let labels = HashMap::new();
            metrics.set_gauge(
                "memory_total_bytes",
                snapshot.total_memory as f64,
                labels.clone(),
            );
            metrics.set_gauge(
                "memory_used_bytes",
                snapshot.used_memory as f64,
                labels.clone(),
            );
            metrics.set_gauge(
                "memory_available_bytes",
                snapshot.available_memory as f64,
                labels.clone(),
            );

            if let Some(process_mem) = snapshot.process_memory {
                metrics.set_gauge("process_memory_bytes", process_mem as f64, labels);
            }
        }

        debug!(
            total_mb = snapshot.total_memory / 1024 / 1024,
            used_mb = snapshot.used_memory / 1024 / 1024,
            process_mb = snapshot.process_memory.map(|m| m / 1024 / 1024),
            "Memory snapshot taken"
        );

        snapshot
    }

    /// Get memory usage trend
    pub async fn get_memory_trend(&self, duration: Duration) -> Vec<MemorySnapshot> {
        let snapshots = self.snapshots.read().await;
        let cutoff_time =
            chrono::Utc::now() - chrono::Duration::from_std(duration).unwrap_or_default();

        snapshots
            .iter()
            .filter(|s| s.timestamp > cutoff_time)
            .cloned()
            .collect()
    }

    /// Start continuous memory monitoring
    pub fn start_monitoring(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let profiler = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;
                profiler.take_snapshot().await;
            }
        })
    }
}

impl Clone for MemoryProfiler {
    fn clone(&self) -> Self {
        Self {
            snapshots: Arc::clone(&self.snapshots),
            metrics_collector: self.metrics_collector.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_profiler() {
        let profiler = PerformanceProfiler::new(None);

        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        let profile_id = profiler
            .start_profile("test_operation".to_string(), metadata)
            .await;

        profiler
            .add_checkpoint(profile_id, "checkpoint1".to_string())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        profiler
            .add_checkpoint(profile_id, "checkpoint2".to_string())
            .await
            .unwrap();

        let result = profiler.finish_profile(profile_id).await.unwrap();

        assert_eq!(result.name, "test_operation");
        assert_eq!(result.checkpoints.len(), 2);
        assert!(result.total_duration > Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_memory_profiler() {
        let profiler = MemoryProfiler::new(None);

        let snapshot = profiler.take_snapshot().await;

        assert!(snapshot.total_memory > 0);
        assert!(snapshot.used_memory > 0);
        assert!(snapshot.available_memory > 0);
    }

    #[tokio::test]
    async fn test_profile_operation() {
        let profiler = PerformanceProfiler::new(None);

        let metadata = HashMap::new();
        let result = profiler
            .profile_operation("test_async_op".to_string(), metadata, async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok::<i32, AppError>(42)
            })
            .await;

        assert!(result.is_ok());
        let (value, profile_result) = result.unwrap();
        assert_eq!(value, 42);
        assert_eq!(profile_result.name, "test_async_op");
        assert!(profile_result.total_duration > Duration::from_millis(5));
    }
}
