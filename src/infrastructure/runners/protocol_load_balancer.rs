// Protocol-Aware Load Balancer
// Task 3.3: Unified Runner System - Load balancing with performance optimization

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};
use super::capability_detector::{PreferredProtocol, PerformanceTier};
use super::unified_registry::{RegisteredRunner, RunnerStatus, UnifiedRunnerRegistry};
use super::unified_interface::{JobExecutionContext, PerformanceRequirements, JobPriority};

/// Protocol-aware load balancer with performance optimization
pub struct ProtocolLoadBalancer {
    /// Registry for runner information
    registry: Arc<UnifiedRunnerRegistry>,
    
    /// Load balancing algorithms
    algorithms: HashMap<String, Box<dyn LoadBalancingAlgorithm + Send + Sync>>,
    
    /// Runner load tracking
    runner_loads: Arc<RwLock<HashMap<Uuid, RunnerLoad>>>,
    
    /// Protocol performance tracking
    protocol_performance: Arc<RwLock<HashMap<PreferredProtocol, ProtocolMetrics>>>,
    
    /// Configuration
    config: LoadBalancerConfig,
    
    /// Round-robin state
    round_robin_state: Arc<Mutex<RoundRobinState>>,
}

/// Load balancer configuration
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    /// Default load balancing algorithm
    pub default_algorithm: String,
    
    /// Maximum load per runner (0.0 - 1.0)
    pub max_runner_load: f64,
    
    /// Load calculation window
    pub load_window: Duration,
    
    /// Health check interval
    pub health_check_interval: Duration,
    
    /// Performance monitoring interval
    pub performance_monitoring_interval: Duration,
    
    /// Enable adaptive load balancing
    pub enable_adaptive_balancing: bool,
    
    /// Protocol preference weights
    pub protocol_weights: HashMap<String, f64>,
    
    /// Geographic load balancing
    pub enable_geographic_balancing: bool,
    
    /// Sticky session support
    pub enable_sticky_sessions: bool,
}

/// Runner load information
#[derive(Debug, Clone)]
pub struct RunnerLoad {
    /// Runner ID
    pub runner_id: Uuid,
    
    /// Current load (0.0 - 1.0)
    pub current_load: f64,
    
    /// Active jobs
    pub active_jobs: u32,
    
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    
    /// Average response time
    pub avg_response_time: Duration,
    
    /// Success rate
    pub success_rate: f64,
    
    /// Last updated
    pub last_updated: Instant,
    
    /// Load history for trend analysis
    pub load_history: VecDeque<LoadSample>,
}

/// Load sample for historical tracking
#[derive(Debug, Clone)]
pub struct LoadSample {
    pub timestamp: Instant,
    pub load: f64,
    pub response_time: Duration,
    pub success_rate: f64,
}

/// Protocol performance metrics
#[derive(Debug, Clone)]
pub struct ProtocolMetrics {
    /// Protocol type
    pub protocol: PreferredProtocol,
    
    /// Average latency
    pub avg_latency: Duration,
    
    /// Throughput (requests per second)
    pub throughput: f64,
    
    /// Success rate
    pub success_rate: f64,
    
    /// Active connections
    pub active_connections: u32,
    
    /// Total requests processed
    pub total_requests: u64,
    
    /// Failed requests
    pub failed_requests: u64,
    
    /// Performance tier
    pub performance_tier: PerformanceTier,
    
    /// Last updated
    pub last_updated: Instant,
}

/// Round-robin state
#[derive(Debug, Default)]
pub struct RoundRobinState {
    /// Current index for each protocol
    pub protocol_indices: HashMap<PreferredProtocol, usize>,
    
    /// Current index for general round-robin
    pub general_index: usize,
}

/// Load balancing algorithm trait
pub trait LoadBalancingAlgorithm {
    /// Select optimal runner for job execution
    fn select_runner(
        &self,
        available_runners: &[RegisteredRunner],
        runner_loads: &HashMap<Uuid, RunnerLoad>,
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)>;
    
    /// Get algorithm name
    fn name(&self) -> &str;
    
    /// Get algorithm description
    fn description(&self) -> &str;
}

/// Round-robin load balancing algorithm
pub struct RoundRobinAlgorithm {
    state: Arc<Mutex<RoundRobinState>>,
}

/// Least connections load balancing algorithm
pub struct LeastConnectionsAlgorithm;

/// Weighted round-robin algorithm
pub struct WeightedRoundRobinAlgorithm {
    weights: HashMap<Uuid, f64>,
}

/// Performance-based algorithm
pub struct PerformanceBasedAlgorithm {
    performance_weight: f64,
    latency_weight: f64,
    throughput_weight: f64,
}

/// Adaptive algorithm that switches based on conditions
pub struct AdaptiveAlgorithm {
    algorithms: HashMap<String, Box<dyn LoadBalancingAlgorithm + Send + Sync>>,
    current_algorithm: String,
    adaptation_threshold: f64,
}

/// Geographic load balancing algorithm
pub struct GeographicAlgorithm {
    region_preferences: HashMap<String, f64>,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        let mut protocol_weights = HashMap::new();
        protocol_weights.insert("valkyrie".to_string(), 1.2);
        protocol_weights.insert("http".to_string(), 1.0);
        protocol_weights.insert("websocket".to_string(), 0.9);
        protocol_weights.insert("grpc".to_string(), 1.1);
        
        Self {
            default_algorithm: "adaptive".to_string(),
            max_runner_load: 0.8,
            load_window: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(30),
            performance_monitoring_interval: Duration::from_secs(10),
            enable_adaptive_balancing: true,
            protocol_weights,
            enable_geographic_balancing: false,
            enable_sticky_sessions: false,
        }
    }
}

impl ProtocolLoadBalancer {
    /// Create a new protocol-aware load balancer
    pub async fn new(
        registry: Arc<UnifiedRunnerRegistry>,
        config: LoadBalancerConfig,
    ) -> Result<Self> {
        let mut algorithms: HashMap<String, Box<dyn LoadBalancingAlgorithm + Send + Sync>> = HashMap::new();
        
        // Initialize algorithms
        let round_robin_state = Arc::new(Mutex::new(RoundRobinState::default()));
        algorithms.insert(
            "round_robin".to_string(),
            Box::new(RoundRobinAlgorithm {
                state: round_robin_state.clone(),
            }),
        );
        
        algorithms.insert(
            "least_connections".to_string(),
            Box::new(LeastConnectionsAlgorithm),
        );
        
        algorithms.insert(
            "performance_based".to_string(),
            Box::new(PerformanceBasedAlgorithm {
                performance_weight: 0.4,
                latency_weight: 0.3,
                throughput_weight: 0.3,
            }),
        );
        
        algorithms.insert(
            "geographic".to_string(),
            Box::new(GeographicAlgorithm {
                region_preferences: HashMap::new(),
            }),
        );
        
        let load_balancer = Self {
            registry,
            algorithms,
            runner_loads: Arc::new(RwLock::new(HashMap::new())),
            protocol_performance: Arc::new(RwLock::new(HashMap::new())),
            config,
            round_robin_state,
        };
        
        // Start background tasks
        load_balancer.start_background_tasks().await;
        
        Ok(load_balancer)
    }
    
    /// Select optimal runner using configured algorithm
    pub async fn select_runner(
        &self,
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        // Get available runners
        let available_runners = self.registry.list_runners_by_status(&RunnerStatus::Online).await;
        
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // Filter runners based on load limits
        let viable_runners = self.filter_viable_runners(&available_runners).await;
        
        if viable_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No viable runners (all overloaded)".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // Select algorithm based on job requirements
        let algorithm_name = self.select_algorithm(context).await;
        
        // Get runner loads
        let runner_loads = self.runner_loads.read().await;
        
        // Apply load balancing algorithm
        let algorithm = self.algorithms.get(&algorithm_name)
            .ok_or_else(|| AppError::InternalError {
                message: format!("Unknown algorithm: {}", algorithm_name),
                component: "protocol_load_balancer".to_string(),
            })?;
        
        let (runner_id, protocol) = algorithm.select_runner(&viable_runners, &runner_loads, context)?;
        
        // Update runner load
        self.update_runner_load(runner_id, 1).await;
        
        debug!(
            "Selected runner {} with protocol {:?} using {} algorithm",
            runner_id, protocol, algorithm_name
        );
        
        Ok((runner_id, protocol))
    }
    
    /// Filter runners based on load and health
    async fn filter_viable_runners(&self, runners: &[RegisteredRunner]) -> Vec<RegisteredRunner> {
        let runner_loads = self.runner_loads.read().await;
        
        runners.iter()
            .filter(|runner| {
                // Check if runner is healthy
                if runner.status != RunnerStatus::Online {
                    return false;
                }
                
                // Check load limits
                if let Some(load) = runner_loads.get(&runner.id) {
                    load.current_load < self.config.max_runner_load
                } else {
                    true // No load data means runner is available
                }
            })
            .cloned()
            .collect()
    }
    
    /// Select appropriate algorithm based on job context
    async fn select_algorithm(&self, context: &JobExecutionContext) -> String {
        if !self.config.enable_adaptive_balancing {
            return self.config.default_algorithm.clone();
        }
        
        // Adaptive algorithm selection based on job characteristics
        match context.performance_requirements.priority {
            JobPriority::Critical => "performance_based".to_string(),
            JobPriority::High => "least_connections".to_string(),
            JobPriority::Normal => "round_robin".to_string(),
            JobPriority::Low => "round_robin".to_string(),
        }
    }
    
    /// Update runner load information
    pub async fn update_runner_load(&self, runner_id: Uuid, load_delta: i32) {
        let mut runner_loads = self.runner_loads.write().await;
        
        let load = runner_loads.entry(runner_id).or_insert_with(|| RunnerLoad {
            runner_id,
            current_load: 0.0,
            active_jobs: 0,
            max_concurrent_jobs: 10, // Default value
            avg_response_time: Duration::from_millis(100),
            success_rate: 1.0,
            last_updated: Instant::now(),
            load_history: VecDeque::new(),
        });
        
        // Update active jobs
        if load_delta > 0 {
            load.active_jobs += load_delta as u32;
        } else if load_delta < 0 && load.active_jobs > 0 {
            load.active_jobs = load.active_jobs.saturating_sub((-load_delta) as u32);
        }
        
        // Calculate current load
        load.current_load = load.active_jobs as f64 / load.max_concurrent_jobs as f64;
        load.last_updated = Instant::now();
        
        // Add to history
        load.load_history.push_back(LoadSample {
            timestamp: Instant::now(),
            load: load.current_load,
            response_time: load.avg_response_time,
            success_rate: load.success_rate,
        });
        
        // Limit history size
        while load.load_history.len() > 100 {
            load.load_history.pop_front();
        }
    }
    
    /// Record job completion for load balancing metrics
    pub async fn record_job_completion(
        &self,
        runner_id: Uuid,
        protocol: PreferredProtocol,
        execution_time: Duration,
        success: bool,
    ) {
        // Update runner load
        self.update_runner_load(runner_id, -1).await;
        
        // Update runner performance metrics
        {
            let mut runner_loads = self.runner_loads.write().await;
            if let Some(load) = runner_loads.get_mut(&runner_id) {
                // Update average response time (exponential moving average)
                let alpha = 0.1;
                let new_time = execution_time.as_millis() as f64;
                let old_time = load.avg_response_time.as_millis() as f64;
                let updated_time = alpha * new_time + (1.0 - alpha) * old_time;
                load.avg_response_time = Duration::from_millis(updated_time as u64);
                
                // Update success rate
                let total_samples = load.load_history.len() as f64;
                if total_samples > 0.0 {
                    let successful_samples = load.load_history.iter()
                        .filter(|sample| sample.success_rate > 0.5)
                        .count() as f64;
                    load.success_rate = successful_samples / total_samples;
                }
            }
        }
        
        // Update protocol performance metrics
        {
            let mut protocol_performance = self.protocol_performance.write().await;
            let metrics = protocol_performance.entry(protocol.clone()).or_insert_with(|| ProtocolMetrics {
                protocol: protocol.clone(),
                avg_latency: Duration::ZERO,
                throughput: 0.0,
                success_rate: 1.0,
                active_connections: 0,
                total_requests: 0,
                failed_requests: 0,
                performance_tier: PerformanceTier::Basic,
                last_updated: Instant::now(),
            });
            
            metrics.total_requests += 1;
            if !success {
                metrics.failed_requests += 1;
            }
            
            // Update average latency
            let alpha = 0.1;
            let new_latency = execution_time.as_nanos() as f64;
            let old_latency = metrics.avg_latency.as_nanos() as f64;
            let updated_latency = alpha * new_latency + (1.0 - alpha) * old_latency;
            metrics.avg_latency = Duration::from_nanos(updated_latency as u64);
            
            // Update success rate
            metrics.success_rate = (metrics.total_requests - metrics.failed_requests) as f64 / metrics.total_requests as f64;
            
            // Update performance tier
            metrics.performance_tier = match metrics.avg_latency.as_micros() {
                0..=10 => PerformanceTier::Ultra,
                11..=100 => PerformanceTier::High,
                101..=1000 => PerformanceTier::Standard,
                _ => PerformanceTier::Basic,
            };
            
            metrics.last_updated = Instant::now();
        }
    }
    
    /// Get load balancing statistics
    pub async fn get_statistics(&self) -> LoadBalancerStatistics {
        let runner_loads = self.runner_loads.read().await;
        let protocol_performance = self.protocol_performance.read().await;
        
        let total_runners = runner_loads.len();
        let active_runners = runner_loads.values()
            .filter(|load| load.active_jobs > 0)
            .count();
        
        let total_active_jobs: u32 = runner_loads.values()
            .map(|load| load.active_jobs)
            .sum();
        
        let avg_load = if total_runners > 0 {
            runner_loads.values()
                .map(|load| load.current_load)
                .sum::<f64>() / total_runners as f64
        } else {
            0.0
        };
        
        let protocol_stats: HashMap<String, ProtocolStats> = protocol_performance.iter()
            .map(|(protocol, metrics)| {
                let protocol_name = format!("{:?}", protocol);
                let stats = ProtocolStats {
                    total_requests: metrics.total_requests,
                    failed_requests: metrics.failed_requests,
                    success_rate: metrics.success_rate,
                    avg_latency: metrics.avg_latency,
                    throughput: metrics.throughput,
                    performance_tier: metrics.performance_tier.clone(),
                };
                (protocol_name, stats)
            })
            .collect();
        
        LoadBalancerStatistics {
            total_runners,
            active_runners,
            total_active_jobs,
            average_load: avg_load,
            protocol_statistics: protocol_stats,
            current_algorithm: self.config.default_algorithm.clone(),
        }
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Load monitoring task
        let runner_loads_clone = self.runner_loads.clone();
        let load_window = self.config.load_window;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                
                // Clean up old load data
                let mut runner_loads = runner_loads_clone.write().await;
                let cutoff = Instant::now() - load_window;
                
                for load in runner_loads.values_mut() {
                    load.load_history.retain(|sample| sample.timestamp > cutoff);
                }
            }
        });
        
        // Performance monitoring task
        let protocol_performance_clone = self.protocol_performance.clone();
        let monitoring_interval = self.config.performance_monitoring_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitoring_interval);
            loop {
                interval.tick().await;
                
                // Update throughput calculations
                let mut protocol_performance = protocol_performance_clone.write().await;
                for metrics in protocol_performance.values_mut() {
                    let elapsed = metrics.last_updated.elapsed().as_secs_f64();
                    if elapsed > 0.0 {
                        metrics.throughput = metrics.total_requests as f64 / elapsed;
                    }
                }
            }
        });
    }
}

// Algorithm implementations

impl LoadBalancingAlgorithm for RoundRobinAlgorithm {
    fn select_runner(
        &self,
        available_runners: &[RegisteredRunner],
        _runner_loads: &HashMap<Uuid, RunnerLoad>,
        _context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // Simple round-robin selection
        let index = (Instant::now().elapsed().as_secs() as usize) % available_runners.len();
        let runner = &available_runners[index];
        
        let protocol = self.determine_protocol(runner);
        Ok((runner.id, protocol))
    }
    
    fn name(&self) -> &str {
        "round_robin"
    }
    
    fn description(&self) -> &str {
        "Round-robin load balancing with equal distribution"
    }
}

impl RoundRobinAlgorithm {
    fn determine_protocol(&self, runner: &RegisteredRunner) -> PreferredProtocol {
        match &runner.runner_type {
            super::unified_registry::RunnerType::Valkyrie { .. } => PreferredProtocol::Valkyrie,
            super::unified_registry::RunnerType::Http { .. } => PreferredProtocol::Http,
            super::unified_registry::RunnerType::Hybrid { primary_protocol, .. } => primary_protocol.clone(),
            super::unified_registry::RunnerType::Unknown => PreferredProtocol::Auto,
        }
    }
}

impl LoadBalancingAlgorithm for LeastConnectionsAlgorithm {
    fn select_runner(
        &self,
        available_runners: &[RegisteredRunner],
        runner_loads: &HashMap<Uuid, RunnerLoad>,
        _context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // Find runner with least connections
        let mut best_runner = &available_runners[0];
        let mut min_connections = u32::MAX;
        
        for runner in available_runners {
            let connections = runner_loads.get(&runner.id)
                .map(|load| load.active_jobs)
                .unwrap_or(0);
            
            if connections < min_connections {
                min_connections = connections;
                best_runner = runner;
            }
        }
        
        let protocol = self.determine_protocol(best_runner);
        Ok((best_runner.id, protocol))
    }
    
    fn name(&self) -> &str {
        "least_connections"
    }
    
    fn description(&self) -> &str {
        "Least connections load balancing"
    }
}

impl LeastConnectionsAlgorithm {
    fn determine_protocol(&self, runner: &RegisteredRunner) -> PreferredProtocol {
        match &runner.runner_type {
            super::unified_registry::RunnerType::Valkyrie { .. } => PreferredProtocol::Valkyrie,
            super::unified_registry::RunnerType::Http { .. } => PreferredProtocol::Http,
            super::unified_registry::RunnerType::Hybrid { primary_protocol, .. } => primary_protocol.clone(),
            super::unified_registry::RunnerType::Unknown => PreferredProtocol::Auto,
        }
    }
}

impl LoadBalancingAlgorithm for PerformanceBasedAlgorithm {
    fn select_runner(
        &self,
        available_runners: &[RegisteredRunner],
        runner_loads: &HashMap<Uuid, RunnerLoad>,
        context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // Calculate performance scores
        let mut best_runner = &available_runners[0];
        let mut best_score = f64::NEG_INFINITY;
        
        for runner in available_runners {
            let score = self.calculate_performance_score(runner, runner_loads, context);
            if score > best_score {
                best_score = score;
                best_runner = runner;
            }
        }
        
        let protocol = self.determine_protocol(best_runner);
        Ok((best_runner.id, protocol))
    }
    
    fn name(&self) -> &str {
        "performance_based"
    }
    
    fn description(&self) -> &str {
        "Performance-based load balancing with weighted scoring"
    }
}

impl PerformanceBasedAlgorithm {
    fn calculate_performance_score(
        &self,
        runner: &RegisteredRunner,
        runner_loads: &HashMap<Uuid, RunnerLoad>,
        _context: &JobExecutionContext,
    ) -> f64 {
        let load_info = runner_loads.get(&runner.id);
        
        // Performance score based on success rate
        let performance_score = runner.performance_metrics.success_rate;
        
        // Latency score (inverse of response time)
        let latency_score = if let Some(load) = load_info {
            1.0 / (load.avg_response_time.as_millis() as f64 + 1.0)
        } else {
            1.0
        };
        
        // Throughput score (inverse of current load)
        let throughput_score = if let Some(load) = load_info {
            1.0 - load.current_load
        } else {
            1.0
        };
        
        // Weighted combination
        performance_score * self.performance_weight +
        latency_score * self.latency_weight +
        throughput_score * self.throughput_weight
    }
    
    fn determine_protocol(&self, runner: &RegisteredRunner) -> PreferredProtocol {
        match &runner.runner_type {
            super::unified_registry::RunnerType::Valkyrie { .. } => PreferredProtocol::Valkyrie,
            super::unified_registry::RunnerType::Http { .. } => PreferredProtocol::Http,
            super::unified_registry::RunnerType::Hybrid { primary_protocol, .. } => primary_protocol.clone(),
            super::unified_registry::RunnerType::Unknown => PreferredProtocol::Auto,
        }
    }
}

impl LoadBalancingAlgorithm for GeographicAlgorithm {
    fn select_runner(
        &self,
        available_runners: &[RegisteredRunner],
        _runner_loads: &HashMap<Uuid, RunnerLoad>,
        _context: &JobExecutionContext,
    ) -> Result<(Uuid, PreferredProtocol)> {
        if available_runners.is_empty() {
            return Err(AppError::InternalError {
                message: "No available runners".to_string(),
                component: "protocol_load_balancer".to_string(),
            });
        }
        
        // For now, just select the first available runner
        // In a real implementation, this would consider geographic preferences
        let runner = &available_runners[0];
        let protocol = self.determine_protocol(runner);
        Ok((runner.id, protocol))
    }
    
    fn name(&self) -> &str {
        "geographic"
    }
    
    fn description(&self) -> &str {
        "Geographic load balancing based on region preferences"
    }
}

impl GeographicAlgorithm {
    fn determine_protocol(&self, runner: &RegisteredRunner) -> PreferredProtocol {
        match &runner.runner_type {
            super::unified_registry::RunnerType::Valkyrie { .. } => PreferredProtocol::Valkyrie,
            super::unified_registry::RunnerType::Http { .. } => PreferredProtocol::Http,
            super::unified_registry::RunnerType::Hybrid { primary_protocol, .. } => primary_protocol.clone(),
            super::unified_registry::RunnerType::Unknown => PreferredProtocol::Auto,
        }
    }
}

/// Load balancer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStatistics {
    pub total_runners: usize,
    pub active_runners: usize,
    pub total_active_jobs: u32,
    pub average_load: f64,
    pub protocol_statistics: HashMap<String, ProtocolStats>,
    pub current_algorithm: String,
}

/// Protocol-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub avg_latency: Duration,
    pub throughput: f64,
    pub performance_tier: PerformanceTier,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Job, JobStatus};
    
    #[test]
    fn test_round_robin_algorithm() {
        let algorithm = RoundRobinAlgorithm {
            state: Arc::new(Mutex::new(RoundRobinState::default())),
        };
        
        assert_eq!(algorithm.name(), "round_robin");
        assert!(!algorithm.description().is_empty());
    }
    
    #[test]
    fn test_least_connections_algorithm() {
        let algorithm = LeastConnectionsAlgorithm;
        
        assert_eq!(algorithm.name(), "least_connections");
        assert!(!algorithm.description().is_empty());
    }
    
    #[test]
    fn test_performance_based_algorithm() {
        let algorithm = PerformanceBasedAlgorithm {
            performance_weight: 0.4,
            latency_weight: 0.3,
            throughput_weight: 0.3,
        };
        
        assert_eq!(algorithm.name(), "performance_based");
        assert!(!algorithm.description().is_empty());
    }
}