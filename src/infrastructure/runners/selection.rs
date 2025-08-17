// Intelligent Runner Selection with Load Balancing and Health-Based Routing
// Part of Task 3.1: High-Performance Runner Adapter

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::core::networking::valkyrie::routing::{
    RoutingContext, Route, RoutingError, QoSRequirements,
};
use crate::core::networking::valkyrie::{
    MessagePriority,
};

use super::valkyrie_adapter::{
    ValkyrieJob, JobId, RunnerId, JobRequirements, RoutingHints,
    RunnerCapabilities, RunnerStatus, HealthStatus, GeographicLocation,
    QoSGuarantees, ValkyrieAdapterError, PerformanceTier,
};

/// Intelligent runner selector with advanced routing capabilities
pub struct IntelligentRunnerSelector {
    // Valkyrie routing integration
    // routing_client: Arc<RoutingClient>, // TODO: Implement when available
    // load_balancer: Arc<LoadBalancerManager>, // TODO: Implement when available
    // qos_router: Arc<QoSRouter>, // TODO: Implement when available
    
    // Runner registry and tracking
    runner_registry: Arc<RwLock<RunnerRegistry>>,
    runner_health: Arc<RwLock<HashMap<RunnerId, RunnerHealthInfo>>>,
    runner_performance: Arc<RwLock<HashMap<RunnerId, RunnerPerformanceHistory>>>,
    
    // Selection algorithms
    selection_algorithms: Arc<SelectionAlgorithms>,
    
    // Configuration
    config: RunnerSelectorConfig,
    
    // Metrics
    metrics: Arc<SelectionMetrics>,
}

/// Configuration for runner selection
#[derive(Debug, Clone)]
pub struct RunnerSelectorConfig {
    pub health_check_interval: Duration,
    pub performance_window: Duration,
    pub max_load_threshold: f64,
    pub min_health_score: f64,
    pub geographic_preference_weight: f64,
    pub performance_weight: f64,
    pub load_weight: f64,
    pub affinity_weight: f64,
    pub enable_predictive_selection: bool,
    pub selection_timeout: Duration,
}

impl Default for RunnerSelectorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            performance_window: Duration::from_secs(300), // 5 minutes
            max_load_threshold: 0.8,
            min_health_score: 0.7,
            geographic_preference_weight: 0.2,
            performance_weight: 0.3,
            load_weight: 0.3,
            affinity_weight: 0.2,
            enable_predictive_selection: true,
            selection_timeout: Duration::from_micros(50), // <50μs target
        }
    }
}

/// Runner registry for tracking available runners
#[derive(Debug, Default)]
pub struct RunnerRegistry {
    pub runners: HashMap<RunnerId, RegisteredRunner>,
    pub node_to_runner: HashMap<Uuid, RunnerId>,
    pub capability_index: CapabilityIndex,
    pub geographic_index: GeographicIndex,
}

/// Registered runner information
#[derive(Debug, Clone)]
pub struct RegisteredRunner {
    pub id: RunnerId,
    pub node_id: Uuid,
    pub capabilities: RunnerCapabilities,
    pub endpoint: String,
    pub geographic_location: Option<GeographicLocation>,
    pub registered_at: Instant,
    pub last_seen: Instant,
    pub metadata: HashMap<String, String>,
}

/// Capability indexing for fast matching
#[derive(Debug, Default)]
pub struct CapabilityIndex {
    pub by_job_type: HashMap<String, Vec<RunnerId>>,
    pub by_language: HashMap<String, Vec<RunnerId>>,
    pub by_framework: HashMap<String, Vec<RunnerId>>,
    pub by_performance_tier: HashMap<PerformanceTier, Vec<RunnerId>>,
    pub by_resource_range: ResourceRangeIndex,
}

/// Resource range indexing for efficient filtering
#[derive(Debug, Default)]
pub struct ResourceRangeIndex {
    pub cpu_ranges: Vec<(u32, u32, Vec<RunnerId>)>, // (min, max, runners)
    pub memory_ranges: Vec<(u32, u32, Vec<RunnerId>)>,
    pub storage_ranges: Vec<(u32, u32, Vec<RunnerId>)>,
    pub gpu_ranges: Vec<(u32, u32, Vec<RunnerId>)>,
}

/// Geographic indexing for location-aware selection
#[derive(Debug, Default)]
pub struct GeographicIndex {
    pub by_region: HashMap<String, Vec<RunnerId>>,
    pub by_zone: HashMap<String, Vec<RunnerId>>,
    pub by_datacenter: HashMap<String, Vec<RunnerId>>,
    pub distance_cache: HashMap<(String, String), f64>, // (from, to) -> distance
}

/// Runner health information
#[derive(Debug, Clone)]
pub struct RunnerHealthInfo {
    pub runner_id: RunnerId,
    pub health_status: HealthStatus,
    pub health_score: f64, // 0.0 to 1.0
    pub last_health_check: Instant,
    pub consecutive_failures: u32,
    pub response_time: Duration,
    pub error_rate: f64,
    pub availability: f64,
}

/// Runner performance history for predictive selection
#[derive(Debug, Clone)]
pub struct RunnerPerformanceHistory {
    pub runner_id: RunnerId,
    pub job_completion_times: Vec<(Instant, Duration)>,
    pub success_rate: f64,
    pub average_response_time: Duration,
    pub throughput: f64, // jobs per second
    pub resource_efficiency: f64,
    pub load_history: Vec<(Instant, f64)>,
    pub performance_trend: PerformanceTrend,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
    Unknown,
}

/// Selection algorithms for different strategies
pub struct SelectionAlgorithms {
    pub weighted_scoring: WeightedScoringAlgorithm,
    pub machine_learning: MLSelectionAlgorithm,
    pub round_robin: RoundRobinAlgorithm,
    pub least_loaded: LeastLoadedAlgorithm,
    pub geographic_aware: GeographicAwareAlgorithm,
}

/// Weighted scoring algorithm
pub struct WeightedScoringAlgorithm {
    config: RunnerSelectorConfig,
}

/// Machine learning-based selection algorithm
pub struct MLSelectionAlgorithm {
    model: Option<SelectionModel>,
    training_data: Vec<SelectionTrainingData>,
}

/// Round robin algorithm for load distribution
pub struct RoundRobinAlgorithm {
    current_index: AtomicU64,
}

/// Least loaded algorithm
pub struct LeastLoadedAlgorithm {
    load_cache: Arc<RwLock<HashMap<RunnerId, f64>>>,
}

/// Geographic-aware selection algorithm
pub struct GeographicAwareAlgorithm {
    distance_calculator: DistanceCalculator,
}

/// Selection model for ML-based selection
#[derive(Debug, Clone)]
pub struct SelectionModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub accuracy: f64,
    pub last_trained: Instant,
}

/// Training data for ML model
#[derive(Debug, Clone)]
pub struct SelectionTrainingData {
    pub job_features: JobFeatures,
    pub runner_features: RunnerFeatures,
    pub selection_outcome: SelectionOutcome,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct JobFeatures {
    pub job_type_encoded: Vec<f64>,
    pub priority: f64,
    pub resource_requirements: Vec<f64>,
    pub deadline_urgency: f64,
    pub payload_size: f64,
}

#[derive(Debug, Clone)]
pub struct RunnerFeatures {
    pub performance_score: f64,
    pub current_load: f64,
    pub health_score: f64,
    pub geographic_distance: f64,
    pub capability_match: f64,
}

#[derive(Debug, Clone)]
pub struct SelectionOutcome {
    pub success: bool,
    pub completion_time: Duration,
    pub resource_efficiency: f64,
    pub user_satisfaction: f64,
}

/// Distance calculator for geographic routing
pub struct DistanceCalculator;

impl DistanceCalculator {
    pub fn calculate_distance(
        &self,
        from: &GeographicLocation,
        to: &GeographicLocation,
    ) -> f64 {
        // Simplified distance calculation
        // In production, this would use proper geographic distance algorithms
        if from.region == to.region {
            if from.zone == to.zone {
                0.1 // Same zone
            } else {
                0.5 // Same region, different zone
            }
        } else {
            1.0 // Different region
        }
    }
}

/// Runner selection result
#[derive(Debug, Clone)]
pub struct RunnerSelection {
    pub runner_id: RunnerId,
    pub runner_node_id: Uuid,
    pub route: Route,
    pub estimated_duration: Duration,
    pub qos_guarantees: QoSGuarantees,
    pub selection_score: f64,
    pub selection_algorithm: String,
    pub selection_latency: Duration,
}

/// Selection metrics for monitoring
#[derive(Debug, Default)]
pub struct SelectionMetrics {
    pub total_selections: AtomicU64,
    pub successful_selections: AtomicU64,
    pub average_selection_time: AtomicU64, // microseconds
    pub algorithm_usage: [AtomicU32; 5], // Usage count for each algorithm
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

impl IntelligentRunnerSelector {
    /// Create a new intelligent runner selector
    pub async fn new(
        // routing_client: Arc<RoutingClient>, // TODO: Add when available
        // load_balancer: Arc<LoadBalancerManager>, // TODO: Add when available
        // qos_router: Arc<QoSRouter>, // TODO: Add when available
        config: RunnerSelectorConfig,
    ) -> Result<Self, ValkyrieAdapterError> {
        let selection_algorithms = Arc::new(SelectionAlgorithms {
            weighted_scoring: WeightedScoringAlgorithm { config: config.clone() },
            machine_learning: MLSelectionAlgorithm {
                model: None,
                training_data: Vec::new(),
            },
            round_robin: RoundRobinAlgorithm {
                current_index: AtomicU64::new(0),
            },
            least_loaded: LeastLoadedAlgorithm {
                load_cache: Arc::new(RwLock::new(HashMap::new())),
            },
            geographic_aware: GeographicAwareAlgorithm {
                distance_calculator: DistanceCalculator,
            },
        });
        
        Ok(Self {
            // routing_client, // TODO: Add when available
            // load_balancer, // TODO: Add when available
            // qos_router, // TODO: Add when available
            runner_registry: Arc::new(RwLock::new(RunnerRegistry::default())),
            runner_health: Arc::new(RwLock::new(HashMap::new())),
            runner_performance: Arc::new(RwLock::new(HashMap::new())),
            selection_algorithms,
            config,
            metrics: Arc::new(SelectionMetrics::default()),
        })
    }
    
    /// Select optimal runner for a job
    pub async fn select_optimal_runner(
        &self,
        job: &ValkyrieJob,
    ) -> Result<RunnerSelection, ValkyrieAdapterError> {
        let start_time = Instant::now();
        
        // Find candidate runners based on capabilities
        let candidates = self.find_candidate_runners(job).await?;
        
        if candidates.is_empty() {
            return Err(ValkyrieAdapterError::Internal(
                "No suitable runners found".to_string()
            ));
        }
        
        // Apply health filtering
        let healthy_candidates = self.filter_healthy_runners(candidates).await?;
        
        if healthy_candidates.is_empty() {
            return Err(ValkyrieAdapterError::Internal(
                "No healthy runners available".to_string()
            ));
        }
        
        // Select best runner using configured algorithm
        let selection = self.select_best_runner(job, healthy_candidates).await?;
        
        // Create Valkyrie route
        let route = self.create_route(job, &selection).await?;
        
        // Calculate QoS guarantees
        let qos_guarantees = self.calculate_qos_guarantees(job, &selection).await?;
        
        let selection_latency = start_time.elapsed();
        
        // Record metrics
        self.metrics.total_selections.fetch_add(1, Ordering::Relaxed);
        if selection_latency <= self.config.selection_timeout {
            self.metrics.successful_selections.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update average selection time
        let current_avg = self.metrics.average_selection_time.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            selection_latency.as_micros() as u64
        } else {
            (current_avg * 9 + selection_latency.as_micros() as u64) / 10
        };
        self.metrics.average_selection_time.store(new_avg, Ordering::Relaxed);
        
        Ok(RunnerSelection {
            runner_id: selection.runner_id,
            runner_node_id: selection.node_id,
            route,
            estimated_duration: selection.estimated_duration,
            qos_guarantees,
            selection_score: selection.score,
            selection_algorithm: selection.algorithm.clone(),
            selection_latency,
        })
    }
    
    /// Find candidate runners based on job requirements
    async fn find_candidate_runners(
        &self,
        job: &ValkyrieJob,
    ) -> Result<Vec<RunnerId>, ValkyrieAdapterError> {
        let registry = self.runner_registry.read().await;
        let mut candidates = Vec::new();
        
        // Start with all runners if no specific requirements
        if job.requirements.required_capabilities.is_empty() {
            candidates.extend(registry.runners.keys().cloned());
        } else {
            // Find runners with required capabilities
            for capability in &job.requirements.required_capabilities {
                if let Some(runner_ids) = registry.capability_index.by_language.get(capability) {
                    candidates.extend(runner_ids.iter().cloned());
                }
                if let Some(runner_ids) = registry.capability_index.by_framework.get(capability) {
                    candidates.extend(runner_ids.iter().cloned());
                }
            }
        }
        
        // Filter by resource requirements
        candidates = self.filter_by_resources(&candidates, &job.requirements, &registry).await;
        
        // Filter by geographic preferences
        if !job.requirements.preferred_regions.is_empty() {
            candidates = self.filter_by_geography(&candidates, &job.requirements, &registry).await;
        }
        
        // Apply routing hints
        if let Some(routing_hints) = &job.routing_hints {
            candidates = self.apply_routing_hints(&candidates, routing_hints).await;
        }
        
        // Remove duplicates
        candidates.sort();
        candidates.dedup();
        
        Ok(candidates)
    }
    
    /// Filter runners by resource requirements
    async fn filter_by_resources(
        &self,
        candidates: &[RunnerId],
        requirements: &JobRequirements,
        registry: &RunnerRegistry,
    ) -> Vec<RunnerId> {
        candidates
            .iter()
            .filter(|&runner_id| {
                if let Some(runner) = registry.runners.get(runner_id) {
                    // Check CPU requirements
                    if let Some(required_cpu) = requirements.cpu_cores {
                        if runner.capabilities.cpu_cores < required_cpu {
                            return false;
                        }
                    }
                    
                    // Check memory requirements
                    if let Some(required_memory) = requirements.memory_mb {
                        if runner.capabilities.memory_gb * 1024 < required_memory {
                            return false;
                        }
                    }
                    
                    // Check storage requirements
                    if let Some(required_storage) = requirements.storage_gb {
                        if runner.capabilities.storage_gb < required_storage {
                            return false;
                        }
                    }
                    
                    // Check GPU requirements
                    if let Some(required_gpu) = requirements.gpu_count {
                        if runner.capabilities.gpu_count < required_gpu {
                            return false;
                        }
                    }
                    
                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }
    
    /// Filter runners by geographic preferences
    async fn filter_by_geography(
        &self,
        candidates: &[RunnerId],
        requirements: &JobRequirements,
        registry: &RunnerRegistry,
    ) -> Vec<RunnerId> {
        if requirements.preferred_regions.is_empty() {
            return candidates.to_vec();
        }
        
        candidates
            .iter()
            .filter(|&runner_id| {
                if let Some(runner) = registry.runners.get(runner_id) {
                    if let Some(location) = &runner.geographic_location {
                        requirements.preferred_regions.contains(&location.region)
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }
    
    /// Apply routing hints to filter candidates
    async fn apply_routing_hints(
        &self,
        candidates: &[RunnerId],
        routing_hints: &RoutingHints,
    ) -> Vec<RunnerId> {
        let mut filtered = candidates.to_vec();
        
        // Apply preferred runners
        if !routing_hints.preferred_runners.is_empty() {
            let preferred: Vec<RunnerId> = filtered.clone()
                .into_iter()
                .filter(|id| routing_hints.preferred_runners.contains(id))
                .collect();
            
            if !preferred.is_empty() {
                filtered = preferred;
            }
        }
        
        // Remove excluded runners
        filtered.retain(|id| !routing_hints.excluded_runners.contains(id));
        
        filtered
    }
    
    /// Filter runners by health status
    async fn filter_healthy_runners(
        &self,
        candidates: Vec<RunnerId>,
    ) -> Result<Vec<RunnerId>, ValkyrieAdapterError> {
        let health_map = self.runner_health.read().await;
        
        let healthy: Vec<RunnerId> = candidates
            .into_iter()
            .filter(|runner_id| {
                if let Some(health_info) = health_map.get(runner_id) {
                    health_info.health_status == HealthStatus::Healthy
                        && health_info.health_score >= self.config.min_health_score
                } else {
                    // Assume healthy if no health info available
                    true
                }
            })
            .collect();
        
        Ok(healthy)
    }
    
    /// Select the best runner from healthy candidates
    async fn select_best_runner(
        &self,
        job: &ValkyrieJob,
        candidates: Vec<RunnerId>,
    ) -> Result<BestRunnerSelection, ValkyrieAdapterError> {
        // Use weighted scoring algorithm by default
        self.selection_algorithms
            .weighted_scoring
            .select_best(job, candidates, self)
            .await
    }
    
    /// Create Valkyrie route for selected runner
    async fn create_route(
        &self,
        job: &ValkyrieJob,
        selection: &BestRunnerSelection,
    ) -> Result<Route, ValkyrieAdapterError> {
        let routing_context = RoutingContext {
            message_id: job.id,
            source: uuid::Uuid::new_v4(), // TODO: Get actual local node ID
            destination: selection.node_id,
            qos_requirements: job.qos_requirements.clone().unwrap_or_default(),
            security_context: Default::default(),
            routing_hints: crate::core::networking::valkyrie::routing::RoutingHints {
                preferred_nodes: job.routing_hints.as_ref().map(|h| h.preferred_runners.clone()).unwrap_or_default(),
                excluded_nodes: job.routing_hints.as_ref().map(|h| h.excluded_runners.clone()).unwrap_or_default(),
                geographic_preference: job.routing_hints.as_ref().and_then(|h| h.geographic_preference.clone()),
                performance_requirements: None,
            },
            deadline: job.deadline,
            created_at: SystemTime::now(),
        };
        
        // TODO: Implement routing client integration
        // TODO: Implement routing calculation
        Ok(())
            .await
            .map_err(ValkyrieAdapterError::Routing)
    }
    
    /// Calculate QoS guarantees for the selection
    async fn calculate_qos_guarantees(
        &self,
        job: &ValkyrieJob,
        selection: &BestRunnerSelection,
    ) -> Result<QoSGuarantees, ValkyrieAdapterError> {
        // Calculate based on runner performance history and current load
        let performance_map = self.runner_performance.read().await;
        
        let guarantees = if let Some(perf_history) = performance_map.get(&selection.runner_id) {
            QoSGuarantees {
                max_latency: Some(perf_history.average_response_time * 2), // Conservative estimate
                min_throughput: Some((perf_history.throughput * 0.8) as u64), // 80% of average
                reliability_guarantee: perf_history.success_rate,
                priority_level: Some(job.priority.into()),
            }
        } else {
            QoSGuarantees::default()
        };
        
        Ok(guarantees)
    }
    
    /// Register a new runner
    pub async fn register_runner(
        &self,
        runner_id: RunnerId,
        node_id: Uuid,
        capabilities: RunnerCapabilities,
        endpoint: String,
        geographic_location: Option<GeographicLocation>,
        metadata: HashMap<String, String>,
    ) -> Result<(), ValkyrieAdapterError> {
        let registered_runner = RegisteredRunner {
            id: runner_id,
            node_id,
            capabilities: capabilities.clone(),
            endpoint,
            geographic_location: geographic_location.clone(),
            registered_at: Instant::now(),
            last_seen: Instant::now(),
            metadata,
        };
        
        let mut registry = self.runner_registry.write().await;
        
        // Add to main registry
        registry.runners.insert(runner_id, registered_runner);
        registry.node_to_runner.insert(node_id, runner_id);
        
        // Update capability index
        self.update_capability_index(&mut registry.capability_index, runner_id, &capabilities).await;
        
        // Update geographic index
        if let Some(location) = geographic_location {
            self.update_geographic_index(&mut registry.geographic_index, runner_id, &location).await;
        }
        
        // Initialize health info
        let health_info = RunnerHealthInfo {
            runner_id,
            health_status: HealthStatus::Healthy,
            health_score: 1.0,
            last_health_check: Instant::now(),
            consecutive_failures: 0,
            response_time: Duration::from_millis(100),
            error_rate: 0.0,
            availability: 1.0,
        };
        
        let mut health_map = self.runner_health.write().await;
        health_map.insert(runner_id, health_info);
        
        // Initialize performance history
        let performance_history = RunnerPerformanceHistory {
            runner_id,
            job_completion_times: Vec::new(),
            success_rate: 1.0,
            average_response_time: Duration::from_millis(100),
            throughput: 1.0,
            resource_efficiency: 1.0,
            load_history: Vec::new(),
            performance_trend: PerformanceTrend::Unknown,
        };
        
        let mut performance_map = self.runner_performance.write().await;
        performance_map.insert(runner_id, performance_history);
        
        Ok(())
    }
    
    /// Update capability index
    async fn update_capability_index(
        &self,
        index: &mut CapabilityIndex,
        runner_id: RunnerId,
        capabilities: &RunnerCapabilities,
    ) {
        // Index by supported languages
        for language in &capabilities.supported_languages {
            index.by_language
                .entry(language.clone())
                .or_insert_with(Vec::new)
                .push(runner_id);
        }
        
        // Index by supported frameworks
        for framework in &capabilities.supported_frameworks {
            index.by_framework
                .entry(framework.clone())
                .or_insert_with(Vec::new)
                .push(runner_id);
        }
        
        // Index by performance tier
        index.by_performance_tier
            .entry(capabilities.performance_tier.clone())
            .or_insert_with(Vec::new)
            .push(runner_id);
    }
    
    /// Update geographic index
    async fn update_geographic_index(
        &self,
        index: &mut GeographicIndex,
        runner_id: RunnerId,
        location: &GeographicLocation,
    ) {
        // Index by region
        index.by_region
            .entry(location.region.clone())
            .or_insert_with(Vec::new)
            .push(runner_id);
        
        // Index by zone
        if let Some(zone) = &location.zone {
            index.by_zone
                .entry(zone.clone())
                .or_insert_with(Vec::new)
                .push(runner_id);
        }
        
        // Index by datacenter
        if let Some(datacenter) = &location.datacenter {
            index.by_datacenter
                .entry(datacenter.clone())
                .or_insert_with(Vec::new)
                .push(runner_id);
        }
    }
    
    /// Get selection metrics
    pub fn get_metrics(&self) -> SelectionMetricsSnapshot {
        SelectionMetricsSnapshot {
            total_selections: self.metrics.total_selections.load(Ordering::Relaxed),
            successful_selections: self.metrics.successful_selections.load(Ordering::Relaxed),
            average_selection_time: Duration::from_micros(
                self.metrics.average_selection_time.load(Ordering::Relaxed)
            ),
            cache_hits: self.metrics.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.metrics.cache_misses.load(Ordering::Relaxed),
        }
    }
}

/// Best runner selection result
#[derive(Debug, Clone)]
pub struct BestRunnerSelection {
    pub runner_id: RunnerId,
    pub node_id: Uuid,
    pub score: f64,
    pub estimated_duration: Duration,
    pub algorithm: String,
}

/// Selection metrics snapshot
#[derive(Debug, Clone)]
pub struct SelectionMetricsSnapshot {
    pub total_selections: u64,
    pub successful_selections: u64,
    pub average_selection_time: Duration,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl WeightedScoringAlgorithm {
    /// Select best runner using weighted scoring
    pub async fn select_best(
        &self,
        job: &ValkyrieJob,
        candidates: Vec<RunnerId>,
        selector: &IntelligentRunnerSelector,
    ) -> Result<BestRunnerSelection, ValkyrieAdapterError> {
        let mut best_runner = None;
        let mut best_score = f64::NEG_INFINITY;
        
        let registry = selector.runner_registry.read().await;
        let health_map = selector.runner_health.read().await;
        let performance_map = selector.runner_performance.read().await;
        
        for runner_id in candidates {
            if let Some(runner) = registry.runners.get(&runner_id) {
                let score = self.calculate_score(
                    job,
                    runner,
                    health_map.get(&runner_id),
                    performance_map.get(&runner_id),
                ).await;
                
                if score > best_score {
                    best_score = score;
                    best_runner = Some((runner_id, runner.node_id));
                }
            }
        }
        
        if let Some((runner_id, node_id)) = best_runner {
            // Estimate duration based on performance history
            let estimated_duration = if let Some(perf_history) = performance_map.get(&runner_id) {
                perf_history.average_response_time
            } else {
                Duration::from_secs(60) // Default estimate
            };
            
            Ok(BestRunnerSelection {
                runner_id,
                node_id,
                score: best_score,
                estimated_duration,
                algorithm: "weighted_scoring".to_string(),
            })
        } else {
            Err(ValkyrieAdapterError::Internal(
                "No suitable runner found in candidates".to_string()
            ))
        }
    }
    
    /// Calculate weighted score for a runner
    async fn calculate_score(
        &self,
        job: &ValkyrieJob,
        runner: &RegisteredRunner,
        health_info: Option<&RunnerHealthInfo>,
        performance_history: Option<&RunnerPerformanceHistory>,
    ) -> f64 {
        let mut score = 0.0;
        
        // Health score component
        if let Some(health) = health_info {
            score += health.health_score * 0.3;
        } else {
            score += 0.5; // Neutral score for unknown health
        }
        
        // Performance score component
        if let Some(performance) = performance_history {
            let performance_score = (performance.success_rate + 
                                   (1.0 - performance.average_response_time.as_secs_f64() / 300.0).max(0.0)) / 2.0;
            score += performance_score * self.config.performance_weight;
        } else {
            score += 0.5 * self.config.performance_weight;
        }
        
        // Load score component (prefer less loaded runners)
        if let Some(performance) = performance_history {
            let current_load = performance.load_history
                .last()
                .map(|(_, load)| *load)
                .unwrap_or(0.5);
            let load_score = 1.0 - current_load;
            score += load_score * self.config.load_weight;
        } else {
            score += 0.5 * self.config.load_weight;
        }
        
        // Geographic preference score
        if let (Some(job_regions), Some(runner_location)) = (
            job.requirements.preferred_regions.first(),
            &runner.geographic_location
        ) {
            let geographic_score = if job_regions == &runner_location.region {
                1.0
            } else {
                0.3
            };
            score += geographic_score * self.config.geographic_preference_weight;
        } else {
            score += 0.5 * self.config.geographic_preference_weight;
        }
        
        // Capability match score
        let capability_score = self.calculate_capability_match(job, &runner.capabilities).await;
        score += capability_score * 0.2;
        
        score.max(0.0).min(1.0) // Clamp to [0, 1]
    }
    
    /// Calculate how well runner capabilities match job requirements
    async fn calculate_capability_match(
        &self,
        job: &ValkyrieJob,
        capabilities: &RunnerCapabilities,
    ) -> f64 {
        if job.requirements.required_capabilities.is_empty() {
            return 1.0; // Perfect match if no specific requirements
        }
        
        let mut matches = 0;
        let total_requirements = job.requirements.required_capabilities.len();
        
        for requirement in &job.requirements.required_capabilities {
            if capabilities.supported_languages.contains(requirement) ||
               capabilities.supported_frameworks.contains(requirement) ||
               capabilities.custom_capabilities.contains_key(requirement) {
                matches += 1;
            }
        }
        
        matches as f64 / total_requirements as f64
    }
}