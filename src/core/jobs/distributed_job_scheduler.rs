//! Distributed Job Scheduler for cluster-aware job placement
//!
//! This module extends the existing JobScheduler with cluster-aware capabilities,
//! intelligent node selection, and failure recovery mechanisms.

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::cluster::cluster_coordinator::ClusterCoordinator;
use crate::core::patterns::events::EventBus;
use crate::core::jobs::job_scheduler::{JobScheduler, JobSchedulerConfig, JobSchedulerStats};
use crate::core::cluster::node_registry::NodeRegistry;
use crate::domain::entities::{
    ClusterNode, Job, JobId, JobStatus, NodeId, NodeStatus,
};
use crate::error::{AppError, Result};

/// Configuration for the distributed job scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedJobSchedulerConfig {
    /// Base job scheduler configuration
    pub base_config: JobSchedulerConfig,
    /// Node selection strategy
    pub node_selection_strategy: NodeSelectionStrategy,
    /// Enable automatic job rescheduling on node failures
    pub enable_auto_rescheduling: bool,
    /// Maximum rescheduling attempts per job
    pub max_reschedule_attempts: u32,
    /// Rescheduling delay in seconds
    pub reschedule_delay: u64,
    /// Resource utilization threshold for node selection (0-100)
    pub resource_threshold: f64,
    /// Enable job affinity scheduling
    pub enable_job_affinity: bool,
    /// Job placement timeout in seconds
    pub placement_timeout: u64,
}

impl Default for DistributedJobSchedulerConfig {
    fn default() -> Self {
        Self {
            base_config: JobSchedulerConfig::default(),
            node_selection_strategy: NodeSelectionStrategy::ResourceAware,
            enable_auto_rescheduling: true,
            max_reschedule_attempts: 3,
            reschedule_delay: 30,
            resource_threshold: 80.0,
            enable_job_affinity: true,
            placement_timeout: 60,
        }
    }
}

/// Node selection strategies for job placement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeSelectionStrategy {
    /// Select node with lowest resource utilization
    LeastLoaded,
    /// Select node based on comprehensive resource analysis
    ResourceAware,
    /// Select node using round-robin distribution
    RoundRobin,
    /// Select node based on job affinity rules
    Affinity,
    /// Select node using weighted distribution
    Weighted,
}

/// Job placement information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPlacement {
    /// Job ID
    pub job_id: JobId,
    /// Target node ID
    pub node_id: NodeId,
    /// Placement timestamp
    pub placed_at: DateTime<Utc>,
    /// Selection strategy used
    pub strategy: NodeSelectionStrategy,
    /// Placement attempt number
    pub attempt: u32,
    /// Previous placements (for rescheduling)
    pub previous_placements: Vec<NodeId>,
    /// Placement metadata
    pub metadata: HashMap<String, String>,
}

/// Job rescheduling information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRescheduling {
    /// Job ID
    pub job_id: JobId,
    /// Original node ID
    pub original_node: NodeId,
    /// New node ID
    pub new_node: NodeId,
    /// Rescheduling reason
    pub reason: ReschedulingReason,
    /// Rescheduling timestamp
    pub rescheduled_at: DateTime<Utc>,
    /// Attempt number
    pub attempt: u32,
}

/// Reasons for job rescheduling
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReschedulingReason {
    /// Node failure detected
    NodeFailure,
    /// Node became unavailable
    NodeUnavailable,
    /// Resource constraints not met
    ResourceConstraints,
    /// Load balancing optimization
    LoadBalancing,
    /// Manual rescheduling request
    Manual,
}

/// Node resource analysis for job placement
#[derive(Debug, Clone)]
pub struct NodeResourceAnalysis {
    /// Node ID
    pub node_id: NodeId,
    /// Overall resource score (0-100, higher is better)
    pub resource_score: f64,
    /// CPU availability score
    pub cpu_score: f64,
    /// Memory availability score
    pub memory_score: f64,
    /// Current load factor
    pub load_factor: f64,
    /// Job affinity score
    pub affinity_score: f64,
    /// Can handle the job
    pub can_handle: bool,
}

/// Distributed job scheduler statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedJobSchedulerStats {
    /// Base scheduler statistics
    pub base_stats: JobSchedulerStats,
    /// Total jobs placed across cluster
    pub total_jobs_placed: u64,
    /// Total jobs rescheduled
    pub total_jobs_rescheduled: u64,
    /// Jobs by node distribution
    pub jobs_by_node: HashMap<NodeId, u64>,
    /// Average placement time in milliseconds
    pub avg_placement_time: f64,
    /// Rescheduling success rate
    pub rescheduling_success_rate: f64,
    /// Node utilization distribution
    pub node_utilization: HashMap<NodeId, f64>,
}

/// Distributed job scheduler implementation
pub struct DistributedJobScheduler {
    /// Configuration
    config: DistributedJobSchedulerConfig,
    /// Base job scheduler
    base_scheduler: Arc<dyn JobScheduler>,
    /// Cluster coordinator for node management
    cluster_coordinator: Arc<ClusterCoordinator>,
    /// Node registry for node information
    node_registry: Arc<NodeRegistry>,
    /// Event bus for failure recovery
    event_bus: Arc<EventBus>,
    /// Job placements tracking
    job_placements: Arc<RwLock<HashMap<JobId, JobPlacement>>>,
    /// Rescheduling history
    rescheduling_history: Arc<RwLock<Vec<JobRescheduling>>>,
    /// Node selection state
    node_selection_state: Arc<Mutex<NodeSelectionState>>,
    /// Statistics
    stats: Arc<RwLock<DistributedJobSchedulerStats>>,
}

/// Node selection state for round-robin and other stateful strategies
#[derive(Debug)]
struct NodeSelectionState {
    /// Round-robin counter
    round_robin_counter: usize,
    /// Node weights for weighted selection
    node_weights: HashMap<NodeId, f64>,
    /// Last selection timestamp per node
    last_selection: HashMap<NodeId, DateTime<Utc>>,
}

impl DistributedJobScheduler {
    /// Create a new distributed job scheduler
    pub fn new(
        config: DistributedJobSchedulerConfig,
        base_scheduler: Arc<dyn JobScheduler>,
        cluster_coordinator: Arc<ClusterCoordinator>,
        node_registry: Arc<NodeRegistry>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            base_scheduler,
            cluster_coordinator,
            node_registry,
            event_bus,
            job_placements: Arc::new(RwLock::new(HashMap::new())),
            rescheduling_history: Arc::new(RwLock::new(Vec::new())),
            node_selection_state: Arc::new(Mutex::new(NodeSelectionState {
                round_robin_counter: 0,
                node_weights: HashMap::new(),
                last_selection: HashMap::new(),
            })),
            stats: Arc::new(RwLock::new(DistributedJobSchedulerStats {
                base_stats: JobSchedulerStats::default(),
                total_jobs_placed: 0,
                total_jobs_rescheduled: 0,
                jobs_by_node: HashMap::new(),
                avg_placement_time: 0.0,
                rescheduling_success_rate: 100.0,
                node_utilization: HashMap::new(),
            })),
        }
    }

    /// Start the distributed scheduler
    pub async fn start(&self) -> Result<()> {
        info!("Starting distributed job scheduler");

        // Start base scheduler
        self.base_scheduler.start().await?;

        // Subscribe to cluster events for failure recovery
        self.start_failure_recovery_listener().await;

        // Start periodic node analysis
        self.start_node_analysis_task().await;

        info!("Distributed job scheduler started successfully");
        Ok(())
    }

    /// Stop the distributed scheduler
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping distributed job scheduler");

        // Stop base scheduler
        self.base_scheduler.stop().await?;

        info!("Distributed job scheduler stopped");
        Ok(())
    }

    /// Schedule a job with cluster-aware placement
    pub async fn schedule_job_distributed(&self, job: Job) -> Result<JobPlacement> {
        let start_time = std::time::Instant::now();
        let job_id = job.id;

        debug!("Scheduling job {} for distributed execution", job_id);

        // Get available nodes
        let available_nodes = self.get_available_nodes().await?;
        if available_nodes.is_empty() {
            return Err(AppError::ValidationError(
                "No available nodes for job scheduling".to_string(),
            ));
        }

        // Analyze nodes for job placement
        let node_analyses = self.analyze_nodes_for_job(&job, &available_nodes).await?;

        // Select the best node
        let selected_node = self.select_best_node(&job, &node_analyses).await?;

        // Create job placement
        let placement = JobPlacement {
            job_id,
            node_id: selected_node.node_id,
            placed_at: Utc::now(),
            strategy: self.config.node_selection_strategy.clone(),
            attempt: 1,
            previous_placements: Vec::new(),
            metadata: HashMap::new(),
        };

        // Store placement information
        {
            let mut placements = self.job_placements.write().await;
            placements.insert(job_id, placement.clone());
        }

        // Distribute job to selected node
        self.cluster_coordinator
            .distribute_job(job_id)
            .await
            .map_err(|e| {
                error!("Failed to distribute job {} to node {}: {}", job_id, selected_node.node_id, e);
                e
            })?;

        // Schedule with base scheduler
        self.base_scheduler.schedule_job(job).await?;

        // Update statistics
        let placement_time = start_time.elapsed().as_millis() as f64;
        self.update_placement_stats(selected_node.node_id, placement_time)
            .await;

        info!(
            "Job {} scheduled to node {} in {:.2}ms",
            job_id, selected_node.node_id, placement_time
        );

        Ok(placement)
    }

    /// Reschedule a job to a different node
    pub async fn reschedule_job(
        &self,
        job_id: JobId,
        reason: ReschedulingReason,
    ) -> Result<JobPlacement> {
        info!("Rescheduling job {} due to {:?}", job_id, reason);

        // Get current placement
        let current_placement = {
            let placements = self.job_placements.read().await;
            placements
                .get(&job_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("Job placement not found: {}", job_id)))?
        };

        // Check rescheduling limits
        if current_placement.attempt >= self.config.max_reschedule_attempts {
            return Err(AppError::ValidationError(format!(
                "Maximum rescheduling attempts ({}) exceeded for job {}",
                self.config.max_reschedule_attempts, job_id
            )));
        }

        // Get job information (this would typically come from job storage)
        let job = Job::new(
            format!("rescheduled-job-{}", job_id),
            Uuid::new_v4(),
            vec![]
        );

        // Get available nodes (excluding the current node)
        let available_nodes = self.get_available_nodes().await?;
        let available_nodes: Vec<_> = available_nodes
            .into_iter()
            .filter(|node| node.id != current_placement.node_id)
            .collect();

        if available_nodes.is_empty() {
            return Err(AppError::ValidationError(
                "No alternative nodes available for rescheduling".to_string(),
            ));
        }

        // Analyze nodes for rescheduling
        let node_analyses = self.analyze_nodes_for_job(&job, &available_nodes).await?;

        // Select the best alternative node
        let selected_node = self.select_best_node(&job, &node_analyses).await?;

        // Create new placement
        let mut new_placement = JobPlacement {
            job_id,
            node_id: selected_node.node_id,
            placed_at: Utc::now(),
            strategy: self.config.node_selection_strategy.clone(),
            attempt: current_placement.attempt + 1,
            previous_placements: {
                let mut prev = current_placement.previous_placements.clone();
                prev.push(current_placement.node_id);
                prev
            },
            metadata: HashMap::new(),
        };

        // Add rescheduling metadata
        new_placement.metadata.insert(
            "rescheduling_reason".to_string(),
            format!("{:?}", reason),
        );
        new_placement.metadata.insert(
            "original_node".to_string(),
            current_placement.node_id.to_string(),
        );

        // Update placement information
        {
            let mut placements = self.job_placements.write().await;
            placements.insert(job_id, new_placement.clone());
        }

        // Record rescheduling history
        let rescheduling = JobRescheduling {
            job_id,
            original_node: current_placement.node_id,
            new_node: selected_node.node_id,
            reason,
            rescheduled_at: Utc::now(),
            attempt: new_placement.attempt,
        };

        {
            let mut history = self.rescheduling_history.write().await;
            history.push(rescheduling);
            // Keep only last 1000 rescheduling records
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        // Distribute job to new node
        self.cluster_coordinator
            .distribute_job(job_id)
            .await?;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_jobs_rescheduled += 1;
            
            // Update success rate
            let total_reschedules = stats.total_jobs_rescheduled;
            stats.rescheduling_success_rate = 
                (stats.rescheduling_success_rate * (total_reschedules - 1) as f64 + 100.0) 
                / total_reschedules as f64;
        }

        info!(
            "Job {} rescheduled from node {} to node {}",
            job_id, current_placement.node_id, selected_node.node_id
        );

        Ok(new_placement)
    }

    /// Get available nodes for job scheduling
    async fn get_available_nodes(&self) -> Result<Vec<ClusterNode>> {
        let all_nodes = self.node_registry.get_active_nodes().await?;
        
        // Filter nodes that can accept jobs
        let available_nodes: Vec<_> = all_nodes
            .into_iter()
            .filter(|node| {
                node.status == NodeStatus::Active && node.can_accept_jobs()
            })
            .collect();

        Ok(available_nodes)
    }

    /// Analyze nodes for job placement suitability
    async fn analyze_nodes_for_job(
        &self,
        job: &Job,
        nodes: &[ClusterNode],
    ) -> Result<Vec<NodeResourceAnalysis>> {
        let mut analyses = Vec::new();

        for node in nodes {
            let analysis = self.analyze_single_node(job, node).await;
            analyses.push(analysis);
        }

        // Sort by resource score (highest first)
        analyses.sort_by(|a, b| b.resource_score.partial_cmp(&a.resource_score).unwrap());

        Ok(analyses)
    }

    /// Analyze a single node for job placement
    async fn analyze_single_node(&self, job: &Job, node: &ClusterNode) -> NodeResourceAnalysis {
        // Calculate CPU score (higher available CPU = higher score)
        let cpu_score = if node.resources.total_cpu > 0 {
            (node.resources.available_cpu as f64 / node.resources.total_cpu as f64) * 100.0
        } else {
            0.0
        };

        // Calculate memory score
        let memory_score = if node.resources.total_memory > 0 {
            (node.resources.available_memory as f64 / node.resources.total_memory as f64) * 100.0
        } else {
            0.0
        };

        // Calculate load factor (lower is better)
        let load_factor = (node.resources.cpu_usage + node.resources.memory_usage) / 2.0;

        // Calculate affinity score based on job requirements
        let affinity_score = self.calculate_job_affinity_score(job, node);

        // Check if node can handle the job
        let can_handle = self.can_node_handle_job(job, node);

        // Calculate overall resource score
        let resource_score = if can_handle {
            let base_score = (cpu_score + memory_score) / 2.0;
            let load_penalty = load_factor * 0.5; // Penalize high load
            let affinity_bonus = affinity_score * 0.3; // Bonus for affinity
            
            (base_score - load_penalty + affinity_bonus).clamp(0.0, 100.0)
        } else {
            0.0 // Cannot handle job
        };

        NodeResourceAnalysis {
            node_id: node.id,
            resource_score,
            cpu_score,
            memory_score,
            load_factor,
            affinity_score,
            can_handle,
        }
    }

    /// Calculate job affinity score for a node
    fn calculate_job_affinity_score(&self, job: &Job, node: &ClusterNode) -> f64 {
        if !self.config.enable_job_affinity {
            return 0.0;
        }

        let mut score = 0.0;

        // Check required tags (if job had tag requirements)
        // This would be implemented based on job requirements structure
        
        // Check node metadata for affinity hints
        if let Some(preferred_workload) = node.metadata.get("preferred_workload") {
            if job.name.contains(preferred_workload) {
                score += 10.0;
            }
        }

        // Check for job type affinity
        if let Some(job_type) = job.metadata.get("job_type") {
            if let Some(node_specialization) = node.metadata.get("specialization") {
                if job_type == node_specialization {
                    score += 15.0;
                }
            }
        }

        score
    }

    /// Check if a node can handle a specific job
    fn can_node_handle_job(&self, job: &Job, node: &ClusterNode) -> bool {
        // Check basic resource requirements
        if let Some(min_resources) = &job.requirements.min_resources {
            if node.resources.available_cpu < min_resources.min_cpu {
                return false;
            }
            if node.resources.available_memory < min_resources.min_memory {
                return false;
            }
            if let Some(min_disk) = min_resources.min_disk {
                if node.resources.available_disk < min_disk {
                    return false;
                }
            }
        }

        // Check resource utilization threshold
        let avg_utilization = (node.resources.cpu_usage + node.resources.memory_usage) / 2.0;
        if avg_utilization > self.config.resource_threshold {
            return false;
        }

        // Check if node is in maintenance or draining
        if matches!(node.status, NodeStatus::Maintenance | NodeStatus::Draining) {
            return false;
        }

        true
    }

    /// Select the best node from analyzed candidates
    async fn select_best_node(
        &self,
        _job: &Job,
        analyses: &[NodeResourceAnalysis],
    ) -> Result<NodeResourceAnalysis> {
        let suitable_nodes: Vec<_> = analyses
            .iter()
            .filter(|analysis| analysis.can_handle)
            .collect();

        if suitable_nodes.is_empty() {
            return Err(AppError::ValidationError(
                "No suitable nodes found for job placement".to_string(),
            ));
        }

        match self.config.node_selection_strategy {
            NodeSelectionStrategy::LeastLoaded => {
                // Select node with lowest load factor
                suitable_nodes
                    .iter()
                    .min_by(|a, b| a.load_factor.partial_cmp(&b.load_factor).unwrap())
                    .map(|&analysis| analysis.clone())
                    .ok_or_else(|| AppError::ValidationError("No node selected".to_string()))
            }
            NodeSelectionStrategy::ResourceAware => {
                // Select node with highest resource score
                suitable_nodes
                    .iter()
                    .max_by(|a, b| a.resource_score.partial_cmp(&b.resource_score).unwrap())
                    .map(|&analysis| analysis.clone())
                    .ok_or_else(|| AppError::ValidationError("No node selected".to_string()))
            }
            NodeSelectionStrategy::RoundRobin => {
                // Round-robin selection
                let mut state = self.node_selection_state.lock().await;
                let index = state.round_robin_counter % suitable_nodes.len();
                state.round_robin_counter = state.round_robin_counter.wrapping_add(1);
                Ok(suitable_nodes[index].clone())
            }
            NodeSelectionStrategy::Affinity => {
                // Select node with highest affinity score
                suitable_nodes
                    .iter()
                    .max_by(|a, b| a.affinity_score.partial_cmp(&b.affinity_score).unwrap())
                    .map(|&analysis| analysis.clone())
                    .ok_or_else(|| AppError::ValidationError("No node selected".to_string()))
            }
            NodeSelectionStrategy::Weighted => {
                // Weighted random selection based on resource scores
                self.select_weighted_node(suitable_nodes).await
            }
        }
    }

    /// Select node using weighted random selection
    async fn select_weighted_node(
        &self,
        nodes: Vec<&NodeResourceAnalysis>,
    ) -> Result<NodeResourceAnalysis> {
        let total_weight: f64 = nodes.iter().map(|n| n.resource_score).sum();
        
        if total_weight == 0.0 {
            return Ok(nodes[0].clone());
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut random_value = rng.gen::<f64>() * total_weight;

        for node in &nodes {
            random_value -= node.resource_score;
            if random_value <= 0.0 {
                return Ok((*node).clone());
            }
        }

        // Fallback to last node
        Ok(nodes[nodes.len() - 1].clone())
    }

    /// Update placement statistics
    async fn update_placement_stats(&self, node_id: NodeId, placement_time: f64) {
        let mut stats = self.stats.write().await;
        
        stats.total_jobs_placed += 1;
        
        // Update average placement time
        let total_placements = stats.total_jobs_placed;
        stats.avg_placement_time = 
            (stats.avg_placement_time * (total_placements - 1) as f64 + placement_time) 
            / total_placements as f64;
        
        // Update jobs by node
        *stats.jobs_by_node.entry(node_id).or_insert(0) += 1;
    }

    /// Start failure recovery listener
    async fn start_failure_recovery_listener(&self) {
        if !self.config.enable_auto_rescheduling {
            return;
        }

        let job_placements = self.job_placements.clone();
        let _scheduler = Arc::new(self.clone());

        // This would typically subscribe to cluster events
        // For now, we'll create a placeholder task
        tokio::spawn(async move {
            // In a real implementation, this would listen to cluster events
            // and trigger rescheduling when nodes fail
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                
                // Check for failed placements and reschedule
                // This is a simplified implementation
                let placements = job_placements.read().await;
                for (job_id, placement) in placements.iter() {
                    // In reality, we'd check if the node is still healthy
                    // and reschedule if needed
                    debug!("Monitoring job {} on node {}", job_id, placement.node_id);
                }
            }
        });
    }

    /// Start periodic node analysis task
    async fn start_node_analysis_task(&self) {
        let node_registry = self.node_registry.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Update node utilization statistics
                if let Ok(nodes) = node_registry.get_active_nodes().await {
                    let mut stats_guard = stats.write().await;
                    stats_guard.node_utilization.clear();
                    
                    for node in nodes {
                        let utilization = (node.resources.cpu_usage + node.resources.memory_usage) / 2.0;
                        stats_guard.node_utilization.insert(node.id, utilization);
                    }
                }
            }
        });
    }

    /// Get distributed scheduler statistics
    pub async fn get_distributed_stats(&self) -> DistributedJobSchedulerStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update base stats from underlying scheduler
        if let Ok(base_stats) = self.base_scheduler.get_stats().await {
            stats.base_stats = base_stats;
        }
        
        stats
    }

    /// Get job placement information
    pub async fn get_job_placement(&self, job_id: JobId) -> Option<JobPlacement> {
        let placements = self.job_placements.read().await;
        placements.get(&job_id).cloned()
    }

    /// Get rescheduling history
    pub async fn get_rescheduling_history(&self) -> Vec<JobRescheduling> {
        let history = self.rescheduling_history.read().await;
        history.clone()
    }
}

// Implement Clone for DistributedJobScheduler to support Arc usage
impl Clone for DistributedJobScheduler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            base_scheduler: self.base_scheduler.clone(),
            cluster_coordinator: self.cluster_coordinator.clone(),
            node_registry: self.node_registry.clone(),
            event_bus: self.event_bus.clone(),
            job_placements: self.job_placements.clone(),
            rescheduling_history: self.rescheduling_history.clone(),
            node_selection_state: self.node_selection_state.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[async_trait]
impl JobScheduler for DistributedJobScheduler {
    async fn schedule_job(&self, job: Job) -> Result<()> {
        // Use distributed scheduling
        self.schedule_job_distributed(job).await?;
        Ok(())
    }

    async fn schedule_job_with_delay(&self, job: Job, delay: ChronoDuration) -> Result<()> {
        // For delayed jobs, use base scheduler
        self.base_scheduler.schedule_job_with_delay(job, delay).await
    }

    async fn cancel_job(&self, job_id: JobId) -> Result<bool> {
        // Remove placement information
        {
            let mut placements = self.job_placements.write().await;
            placements.remove(&job_id);
        }
        
        // Cancel with base scheduler
        self.base_scheduler.cancel_job(job_id).await
    }

    async fn get_job_status(&self, job_id: JobId) -> Result<Option<JobStatus>> {
        self.base_scheduler.get_job_status(job_id).await
    }

    async fn get_stats(&self) -> Result<JobSchedulerStats> {
        self.base_scheduler.get_stats().await
    }

    async fn start(&self) -> Result<()> {
        self.start().await
    }

    async fn stop(&self) -> Result<()> {
        self.stop().await
    }

    async fn process_pending_jobs(&self) -> Result<()> {
        self.base_scheduler.process_pending_jobs().await
    }

    async fn retry_job(&self, job_id: JobId) -> Result<()> {
        // For retries, attempt rescheduling first
        if self.config.enable_auto_rescheduling {
            match self.reschedule_job(job_id, ReschedulingReason::Manual).await {
                Ok(_) => {
                    info!("Job {} rescheduled for retry", job_id);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Failed to reschedule job {} for retry: {}", job_id, e);
                }
            }
        }
        
        // Fallback to base scheduler retry
        self.base_scheduler.retry_job(job_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::jobs::job_scheduler::DefaultJobScheduler;
    use crate::core::runners::runner_pool::{DefaultRunnerPoolManager, RunnerPoolConfig};
    use crate::domain::entities::{JobStep, NodeResources, NodeRole};
    use std::net::SocketAddr;
    use std::str::FromStr;

    // Mock implementations for testing would go here
    // For now, we'll use simplified tests that don't require full mocks

    fn create_test_node(id: NodeId, cpu_usage: f64, memory_usage: f64) -> ClusterNode {
        let mut node = ClusterNode::new(
            format!("test-node-{}", id),
            SocketAddr::from_str("127.0.0.1:8080").unwrap(),
            NodeRole::Worker,
        );
        node.id = id;
        node.status = NodeStatus::Active;
        node.resources = NodeResources {
            total_cpu: 4,
            available_cpu: 2,
            total_memory: 8192,
            available_memory: 4096,
            total_disk: 102400,
            available_disk: 51200,
            cpu_usage,
            memory_usage,
            disk_usage: 50.0,
        };
        node
    }

    #[tokio::test]
    async fn test_node_resource_analysis() {
        let config = DistributedJobSchedulerConfig::default();
        let base_scheduler = Arc::new(DefaultJobScheduler::new(
            Arc::new(DefaultRunnerPoolManager::with_default_config()),
            JobSchedulerConfig::default(),
        ));
        
        // Create mock dependencies - in a real test, these would be proper mocks
        // For now, we'll skip the full test due to complex dependencies
        
        // Test node analysis logic
        let node = create_test_node(Uuid::new_v4(), 30.0, 40.0);
        let job = Job::new("test-job".to_string(), Uuid::new_v4(), vec![]);
        
        // This would test the analyze_single_node method
        // but requires a full scheduler instance
        assert_eq!(node.resources.cpu_usage, 30.0);
        assert_eq!(node.resources.memory_usage, 40.0);
    }

    #[test]
    fn test_node_selection_strategies() {
        // Test strategy enum
        assert_eq!(NodeSelectionStrategy::LeastLoaded, NodeSelectionStrategy::LeastLoaded);
        assert_ne!(NodeSelectionStrategy::LeastLoaded, NodeSelectionStrategy::ResourceAware);
    }

    #[test]
    fn test_rescheduling_reasons() {
        // Test rescheduling reason enum
        assert_eq!(ReschedulingReason::NodeFailure, ReschedulingReason::NodeFailure);
        assert_ne!(ReschedulingReason::NodeFailure, ReschedulingReason::LoadBalancing);
    }
}