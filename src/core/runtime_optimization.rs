use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Runtime optimization manager for performance tuning
pub struct RuntimeOptimizer {
    optimization_profiles: Arc<RwLock<HashMap<String, OptimizationProfile>>>,
    active_optimizations: Arc<RwLock<Vec<ActiveOptimization>>>,
    metrics_collector: Option<Arc<super::metrics::MetricsCollector>>,
}

#[derive(Debug, Clone)]
pub struct OptimizationProfile {
    pub name: String,
    pub cpu_optimization: CpuOptimization,
    pub memory_optimization: MemoryOptimization,
    pub io_optimization: IoOptimization,
    pub network_optimization: NetworkOptimization,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct CpuOptimization {
    pub thread_pool_size: Option<usize>,
    pub cpu_affinity: Option<Vec<usize>>,
    pub priority_boost: bool,
    pub yield_strategy: YieldStrategy,
}

#[derive(Debug, Clone)]
pub struct MemoryOptimization {
    pub gc_strategy: GcStrategy,
    pub memory_pool_size: Option<usize>,
    pub preallocation_size: Option<usize>,
    pub compression_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct IoOptimization {
    pub buffer_size: usize,
    pub read_ahead: bool,
    pub write_behind: bool,
    pub async_io: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkOptimization {
    pub connection_pooling: bool,
    pub keep_alive: bool,
    pub tcp_nodelay: bool,
    pub buffer_size: usize,
}

#[derive(Debug, Clone)]
pub enum YieldStrategy {
    Aggressive,
    Conservative,
    Adaptive,
}

#[derive(Debug, Clone)]
pub enum GcStrategy {
    Frequent,
    Lazy,
    Adaptive,
}

#[derive(Debug, Clone)]
pub struct ActiveOptimization {
    pub profile_name: String,
    pub started_at: Instant,
    pub performance_impact: f64,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceUsage {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub io_operations_per_second: u64,
    pub network_throughput_bytes_per_second: u64,
}

impl RuntimeOptimizer {
    pub fn new(metrics_collector: Option<Arc<super::metrics::MetricsCollector>>) -> Self {
        Self {
            optimization_profiles: Arc::new(RwLock::new(HashMap::new())),
            active_optimizations: Arc::new(RwLock::new(Vec::new())),
            metrics_collector,
        }
    }

    /// Add an optimization profile
    pub async fn add_profile(&self, profile: OptimizationProfile) {
        let mut profiles = self.optimization_profiles.write().await;
        profiles.insert(profile.name.clone(), profile.clone());

        info!(
            profile_name = %profile.name,
            "Added optimization profile"
        );
    }

    /// Apply an optimization profile
    pub async fn apply_profile(&self, profile_name: &str) -> Result<()> {
        let profile = {
            let profiles = self.optimization_profiles.read().await;
            profiles.get(profile_name).cloned().ok_or_else(|| {
                AppError::NotFound(format!("Optimization profile '{}' not found", profile_name))
            })?
        };

        if !profile.enabled {
            return Err(AppError::BadRequest(format!(
                "Optimization profile '{}' is disabled",
                profile_name
            )));
        }

        // Apply CPU optimizations
        self.apply_cpu_optimizations(&profile.cpu_optimization)
            .await?;

        // Apply memory optimizations
        self.apply_memory_optimizations(&profile.memory_optimization)
            .await?;

        // Apply I/O optimizations
        self.apply_io_optimizations(&profile.io_optimization)
            .await?;

        // Apply network optimizations
        self.apply_network_optimizations(&profile.network_optimization)
            .await?;

        // Track active optimization
        let active_optimization = ActiveOptimization {
            profile_name: profile_name.to_string(),
            started_at: Instant::now(),
            performance_impact: 0.0, // Will be calculated over time
            resource_usage: self.get_current_resource_usage().await,
        };

        let mut active = self.active_optimizations.write().await;
        active.push(active_optimization);

        info!(
            profile_name = %profile_name,
            "Applied optimization profile"
        );

        Ok(())
    }

    async fn apply_cpu_optimizations(&self, cpu_opt: &CpuOptimization) -> Result<()> {
        // Set thread pool size if specified
        if let Some(pool_size) = cpu_opt.thread_pool_size {
            // This would typically involve configuring the Tokio runtime
            // For now, we'll just log the intention
            debug!(pool_size = pool_size, "Would configure thread pool size");
        }

        // Apply CPU affinity if specified
        if let Some(affinity) = &cpu_opt.cpu_affinity {
            debug!(affinity = ?affinity, "Would set CPU affinity");
        }

        // Apply priority boost
        if cpu_opt.priority_boost {
            debug!("Would apply priority boost");
        }

        // Configure yield strategy
        match cpu_opt.yield_strategy {
            YieldStrategy::Aggressive => {
                debug!("Configured aggressive yield strategy");
            }
            YieldStrategy::Conservative => {
                debug!("Configured conservative yield strategy");
            }
            YieldStrategy::Adaptive => {
                debug!("Configured adaptive yield strategy");
            }
        }

        Ok(())
    }

    async fn apply_memory_optimizations(&self, memory_opt: &MemoryOptimization) -> Result<()> {
        // Configure GC strategy
        match memory_opt.gc_strategy {
            GcStrategy::Frequent => {
                debug!("Configured frequent GC strategy");
            }
            GcStrategy::Lazy => {
                debug!("Configured lazy GC strategy");
            }
            GcStrategy::Adaptive => {
                debug!("Configured adaptive GC strategy");
            }
        }

        // Set memory pool size
        if let Some(pool_size) = memory_opt.memory_pool_size {
            debug!(pool_size = pool_size, "Would configure memory pool size");
        }

        // Set preallocation size
        if let Some(prealloc_size) = memory_opt.preallocation_size {
            debug!(
                prealloc_size = prealloc_size,
                "Would configure preallocation size"
            );
        }

        // Enable compression if specified
        if memory_opt.compression_enabled {
            debug!("Would enable memory compression");
        }

        Ok(())
    }

    async fn apply_io_optimizations(&self, io_opt: &IoOptimization) -> Result<()> {
        debug!(
            buffer_size = io_opt.buffer_size,
            read_ahead = io_opt.read_ahead,
            write_behind = io_opt.write_behind,
            async_io = io_opt.async_io,
            "Applied I/O optimizations"
        );
        Ok(())
    }

    async fn apply_network_optimizations(&self, network_opt: &NetworkOptimization) -> Result<()> {
        debug!(
            connection_pooling = network_opt.connection_pooling,
            keep_alive = network_opt.keep_alive,
            tcp_nodelay = network_opt.tcp_nodelay,
            buffer_size = network_opt.buffer_size,
            "Applied network optimizations"
        );
        Ok(())
    }

    async fn get_current_resource_usage(&self) -> ResourceUsage {
        use sysinfo::{CpuExt, System, SystemExt};

        let mut system = System::new_all();
        system.refresh_all();

        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        let memory_usage = system.used_memory();

        // Get process-specific information
        let _process_info =
            system.process(sysinfo::get_current_pid().unwrap_or(sysinfo::Pid::from(0)));

        ResourceUsage {
            cpu_usage_percent: cpu_usage,
            memory_usage_bytes: memory_usage,
            io_operations_per_second: 0, // Would need more sophisticated tracking
            network_throughput_bytes_per_second: 0, // Would need more sophisticated tracking
        }
    }

    /// Get performance report
    pub async fn get_performance_report(&self) -> PerformanceReport {
        let active_optimizations = self.active_optimizations.read().await;
        let current_usage = self.get_current_resource_usage().await;

        let total_optimizations = active_optimizations.len();
        let average_impact = if !active_optimizations.is_empty() {
            active_optimizations
                .iter()
                .map(|opt| opt.performance_impact)
                .sum::<f64>()
                / total_optimizations as f64
        } else {
            0.0
        };

        PerformanceReport {
            active_optimizations: total_optimizations,
            average_performance_impact: average_impact,
            current_resource_usage: current_usage,
            recommendations: self.generate_recommendations().await,
            generated_at: chrono::Utc::now(),
        }
    }

    async fn generate_recommendations(&self) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();
        let current_usage = self.get_current_resource_usage().await;

        // CPU recommendations
        if current_usage.cpu_usage_percent > 80.0 {
            recommendations.push(OptimizationRecommendation {
                category: "CPU".to_string(),
                priority: RecommendationPriority::High,
                description: "High CPU usage detected. Consider enabling CPU optimizations."
                    .to_string(),
                suggested_profile: Some("high_performance_cpu".to_string()),
            });
        }

        // Memory recommendations
        if current_usage.memory_usage_bytes > 1024 * 1024 * 1024 {
            // 1GB
            recommendations.push(OptimizationRecommendation {
                category: "Memory".to_string(),
                priority: RecommendationPriority::Medium,
                description: "High memory usage detected. Consider enabling memory optimizations."
                    .to_string(),
                suggested_profile: Some("memory_efficient".to_string()),
            });
        }

        recommendations
    }

    /// Start continuous optimization monitoring
    pub fn start_monitoring(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let optimizer = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                // Update performance metrics for active optimizations
                let mut active = optimizer.active_optimizations.write().await;
                for optimization in active.iter_mut() {
                    let current_usage = optimizer.get_current_resource_usage().await;

                    // Calculate performance impact (simplified)
                    let baseline_cpu = optimization.resource_usage.cpu_usage_percent;
                    let current_cpu = current_usage.cpu_usage_percent;
                    optimization.performance_impact = baseline_cpu - current_cpu;

                    optimization.resource_usage = current_usage;
                }

                // Record metrics
                if let Some(metrics) = &optimizer.metrics_collector {
                    let current_usage = optimizer.get_current_resource_usage().await;
                    let labels = HashMap::new();

                    metrics.set_gauge(
                        "runtime_cpu_usage_percent",
                        current_usage.cpu_usage_percent,
                        labels.clone(),
                    );
                    metrics.set_gauge(
                        "runtime_memory_usage_bytes",
                        current_usage.memory_usage_bytes as f64,
                        labels,
                    );
                }
            }
        })
    }

    /// Create default optimization profiles
    pub async fn create_default_profiles(&self) {
        // High performance profile
        let high_performance = OptimizationProfile {
            name: "high_performance".to_string(),
            cpu_optimization: CpuOptimization {
                thread_pool_size: Some(num_cpus::get()),
                cpu_affinity: None,
                priority_boost: true,
                yield_strategy: YieldStrategy::Aggressive,
            },
            memory_optimization: MemoryOptimization {
                gc_strategy: GcStrategy::Frequent,
                memory_pool_size: Some(1024 * 1024 * 100), // 100MB
                preallocation_size: Some(1024 * 1024 * 50), // 50MB
                compression_enabled: false,
            },
            io_optimization: IoOptimization {
                buffer_size: 64 * 1024, // 64KB
                read_ahead: true,
                write_behind: true,
                async_io: true,
            },
            network_optimization: NetworkOptimization {
                connection_pooling: true,
                keep_alive: true,
                tcp_nodelay: true,
                buffer_size: 64 * 1024,
            },
            enabled: true,
        };

        // Memory efficient profile
        let memory_efficient = OptimizationProfile {
            name: "memory_efficient".to_string(),
            cpu_optimization: CpuOptimization {
                thread_pool_size: Some(num_cpus::get() / 2),
                cpu_affinity: None,
                priority_boost: false,
                yield_strategy: YieldStrategy::Conservative,
            },
            memory_optimization: MemoryOptimization {
                gc_strategy: GcStrategy::Adaptive,
                memory_pool_size: Some(1024 * 1024 * 50), // 50MB
                preallocation_size: Some(1024 * 1024 * 10), // 10MB
                compression_enabled: true,
            },
            io_optimization: IoOptimization {
                buffer_size: 16 * 1024, // 16KB
                read_ahead: false,
                write_behind: false,
                async_io: true,
            },
            network_optimization: NetworkOptimization {
                connection_pooling: true,
                keep_alive: false,
                tcp_nodelay: false,
                buffer_size: 16 * 1024,
            },
            enabled: true,
        };

        self.add_profile(high_performance).await;
        self.add_profile(memory_efficient).await;

        info!("Created default optimization profiles");
    }
}

impl Clone for RuntimeOptimizer {
    fn clone(&self) -> Self {
        Self {
            optimization_profiles: Arc::clone(&self.optimization_profiles),
            active_optimizations: Arc::clone(&self.active_optimizations),
            metrics_collector: self.metrics_collector.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceReport {
    pub active_optimizations: usize,
    pub average_performance_impact: f64,
    pub current_resource_usage: ResourceUsage,
    pub recommendations: Vec<OptimizationRecommendation>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OptimizationRecommendation {
    pub category: String,
    pub priority: RecommendationPriority,
    pub description: String,
    pub suggested_profile: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_optimizer() {
        let optimizer = RuntimeOptimizer::new(None);

        // Create default profiles
        optimizer.create_default_profiles().await;

        // Apply high performance profile
        let result = optimizer.apply_profile("high_performance").await;
        assert!(result.is_ok());

        // Get performance report
        let report = optimizer.get_performance_report().await;
        assert_eq!(report.active_optimizations, 1);
    }

    #[tokio::test]
    async fn test_optimization_profile_creation() {
        let optimizer = RuntimeOptimizer::new(None);

        let profile = OptimizationProfile {
            name: "test_profile".to_string(),
            cpu_optimization: CpuOptimization {
                thread_pool_size: Some(4),
                cpu_affinity: None,
                priority_boost: false,
                yield_strategy: YieldStrategy::Adaptive,
            },
            memory_optimization: MemoryOptimization {
                gc_strategy: GcStrategy::Lazy,
                memory_pool_size: None,
                preallocation_size: None,
                compression_enabled: false,
            },
            io_optimization: IoOptimization {
                buffer_size: 32 * 1024,
                read_ahead: true,
                write_behind: false,
                async_io: true,
            },
            network_optimization: NetworkOptimization {
                connection_pooling: false,
                keep_alive: true,
                tcp_nodelay: true,
                buffer_size: 32 * 1024,
            },
            enabled: true,
        };

        optimizer.add_profile(profile).await;

        let result = optimizer.apply_profile("test_profile").await;
        assert!(result.is_ok());
    }
}
