//! Load balancing implementations for runner selection
//! 
//! This module provides various load balancing strategies for distributing
//! jobs across available runners in the pool.

use async_trait::async_trait;
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::runners::runner_pool::{LoadBalancer, SchedulingContext, RunnerRegistration};
use crate::domain::entities::{RunnerId, RunnerCapacity};
use crate::error::Result;

/// Round-robin load balancer
pub struct RoundRobinLoadBalancer {
    counter: AtomicUsize,
}

impl Default for RoundRobinLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl RoundRobinLoadBalancer {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl LoadBalancer for RoundRobinLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        _runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        if context.available_runners.is_empty() {
            return Ok(None);
        }
        
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % context.available_runners.len();
        Ok(Some(context.available_runners[index]))
    }
    
    fn strategy_name(&self) -> &'static str {
        "RoundRobin"
    }
}

/// Least loaded load balancer - selects runner with lowest current load
pub struct LeastLoadedLoadBalancer;

impl Default for LeastLoadedLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LeastLoadedLoadBalancer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LoadBalancer for LeastLoadedLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        _runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        if context.available_runners.is_empty() {
            return Ok(None);
        }
        
        let mut best_runner = None;
        let mut lowest_load = f64::MAX;
        
        for &runner_id in &context.available_runners {
            if let Some(capacity) = context.runner_capacities.get(&runner_id) {
                // Calculate load as percentage of used capacity
                let load = if capacity.max_concurrent_jobs > 0 {
                    (capacity.current_jobs as f64 / capacity.max_concurrent_jobs as f64) * 100.0
                } else {
                    0.0
                };
                
                // Also consider CPU and memory usage
                let resource_load = (capacity.cpu_usage + capacity.memory_usage) / 2.0;
                let combined_load = (load + resource_load) / 2.0;
                
                if combined_load < lowest_load {
                    lowest_load = combined_load;
                    best_runner = Some(runner_id);
                }
            }
        }
        
        Ok(best_runner)
    }
    
    fn strategy_name(&self) -> &'static str {
        "LeastLoaded"
    }
}

/// Random load balancer - selects a random available runner
pub struct RandomLoadBalancer;

impl Default for RandomLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomLoadBalancer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LoadBalancer for RandomLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        _runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        if context.available_runners.is_empty() {
            return Ok(None);
        }
        
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..context.available_runners.len());
        Ok(Some(context.available_runners[index]))
    }
    
    fn strategy_name(&self) -> &'static str {
        "Random"
    }
}

/// Weighted load balancer - selects runner based on capacity weights
pub struct WeightedLoadBalancer;

impl Default for WeightedLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightedLoadBalancer {
    pub fn new() -> Self {
        Self
    }
    
    /// Calculate weight for a runner based on its capacity and current load
    fn calculate_weight(&self, capacity: &RunnerCapacity) -> f64 {
        if capacity.max_concurrent_jobs == 0 {
            return 0.0;
        }
        
        // Base weight on available slots
        let slot_weight = capacity.available_slots as f64 / capacity.max_concurrent_jobs as f64;
        
        // Adjust for resource availability
        let cpu_weight = (100.0 - capacity.cpu_usage) / 100.0;
        let memory_weight = (100.0 - capacity.memory_usage) / 100.0;
        
        // Combined weight (higher is better)
        (slot_weight + cpu_weight + memory_weight) / 3.0
    }
}

#[async_trait]
impl LoadBalancer for WeightedLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        _runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        if context.available_runners.is_empty() {
            return Ok(None);
        }
        
        // Calculate weights for all available runners
        let mut weighted_runners = Vec::new();
        let mut total_weight = 0.0;
        
        for &runner_id in &context.available_runners {
            if let Some(capacity) = context.runner_capacities.get(&runner_id) {
                let weight = self.calculate_weight(capacity);
                if weight > 0.0 {
                    weighted_runners.push((runner_id, weight));
                    total_weight += weight;
                }
            }
        }
        
        if weighted_runners.is_empty() || total_weight == 0.0 {
            // Fallback to first available if no weights
            return Ok(context.available_runners.first().copied());
        }
        
        // Select runner using weighted random selection
        let mut rng = rand::thread_rng();
        let mut random_value = rng.gen::<f64>() * total_weight;
        
        for (runner_id, weight) in &weighted_runners {
            random_value -= weight;
            if random_value <= 0.0 {
                return Ok(Some(*runner_id));
            }
        }
        
        // Fallback to last runner if rounding errors occur
        Ok(weighted_runners.last().map(|(id, _)| *id))
    }
    
    fn strategy_name(&self) -> &'static str {
        "Weighted"
    }
}

/// First available load balancer - selects the first available runner
pub struct FirstAvailableLoadBalancer;

impl Default for FirstAvailableLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl FirstAvailableLoadBalancer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LoadBalancer for FirstAvailableLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        _runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        Ok(context.available_runners.first().copied())
    }
    
    fn strategy_name(&self) -> &'static str {
        "FirstAvailable"
    }
}

/// Affinity-aware load balancer - considers job requirements and runner tags
pub struct AffinityLoadBalancer {
    fallback_strategy: Arc<dyn LoadBalancer>,
}

impl AffinityLoadBalancer {
    pub fn new(fallback_strategy: Arc<dyn LoadBalancer>) -> Self {
        Self {
            fallback_strategy,
        }
    }
    
    /// Calculate affinity score between job and runner
    fn calculate_affinity_score(
        &self,
        context: &SchedulingContext,
        runner_id: RunnerId,
        runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> f64 {
        let Some(registration) = runners.get(&runner_id) else {
            return 0.0;
        };
        
        let mut score = 0.0;
        
        // Check required tags
        for required_tag in &context.constraints.required_tags {
            if registration.entity.tags.contains(required_tag) {
                score += 10.0;
            } else {
                return 0.0; // Must have all required tags
            }
        }
        
        // Check excluded tags
        for excluded_tag in &context.constraints.excluded_tags {
            if registration.entity.tags.contains(excluded_tag) {
                return 0.0; // Cannot have excluded tags
            }
        }
        
        // Check node affinity
        if !context.constraints.node_affinity.is_empty() {
            if let Some(node_id) = registration.entity.node_id {
                let node_id_str = node_id.to_string();
                if context.constraints.node_affinity.contains(&node_id_str) {
                    score += 5.0;
                }
            }
        }
        
        // Bonus for exact runner type match
        if let Some(required_type) = &context.job.requirements.runner_type {
            if std::mem::discriminant(&registration.entity.runner_type) == std::mem::discriminant(required_type) {
                score += 3.0;
            }
        }
        
        score
    }
}

#[async_trait]
impl LoadBalancer for AffinityLoadBalancer {
    async fn select_runner(
        &self,
        context: &SchedulingContext,
        runners: &HashMap<RunnerId, RunnerRegistration>,
    ) -> Result<Option<RunnerId>> {
        if context.available_runners.is_empty() {
            return Ok(None);
        }
        
        // Calculate affinity scores for all available runners
        let mut scored_runners = Vec::new();
        
        for &runner_id in &context.available_runners {
            let score = self.calculate_affinity_score(context, runner_id, runners);
            if score > 0.0 {
                scored_runners.push((runner_id, score));
            }
        }
        
        if scored_runners.is_empty() {
            // No runners match affinity requirements, use fallback
            return self.fallback_strategy.select_runner(context, runners).await;
        }
        
        // Sort by score (highest first)
        scored_runners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Select the highest scoring runner
        Ok(scored_runners.first().map(|(id, _)| *id))
    }
    
    fn strategy_name(&self) -> &'static str {
        "Affinity"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{RunnerEntity, RunnerType, Job, JobRequirements};
    use crate::core::runners::runner_pool::{LoadBalancingStrategy, SchedulingConstraints};
    use std::collections::HashMap;
    use uuid::Uuid;
    
    fn create_test_capacity(max_jobs: u32, current_jobs: u32, cpu_usage: f64, memory_usage: f64) -> RunnerCapacity {
        RunnerCapacity {
            max_concurrent_jobs: max_jobs,
            current_jobs,
            available_slots: max_jobs.saturating_sub(current_jobs),
            cpu_usage,
            memory_usage,
            disk_usage: 50.0,
        }
    }
    
    fn create_test_context(available_runners: Vec<RunnerId>, capacities: HashMap<RunnerId, RunnerCapacity>) -> SchedulingContext {
        SchedulingContext {
            job: Job::new("test-job".to_string(), Uuid::new_v4(), vec![]),
            available_runners,
            runner_capacities: capacities,
            strategy: LoadBalancingStrategy::LeastLoaded,
            constraints: SchedulingConstraints::default(),
        }
    }
    
    #[tokio::test]
    async fn test_round_robin_load_balancer() {
        let balancer = RoundRobinLoadBalancer::new();
        let runner1 = Uuid::new_v4();
        let runner2 = Uuid::new_v4();
        let runner3 = Uuid::new_v4();
        
        let available_runners = vec![runner1, runner2, runner3];
        let capacities = HashMap::new();
        let context = create_test_context(available_runners.clone(), capacities);
        let runners = HashMap::new();
        
        // Test round-robin selection
        let selected1 = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        let selected2 = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        let selected3 = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        let selected4 = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        
        assert_eq!(selected1, runner1);
        assert_eq!(selected2, runner2);
        assert_eq!(selected3, runner3);
        assert_eq!(selected4, runner1); // Should wrap around
    }
    
    #[tokio::test]
    async fn test_least_loaded_load_balancer() {
        let balancer = LeastLoadedLoadBalancer::new();
        let runner1 = Uuid::new_v4();
        let runner2 = Uuid::new_v4();
        let runner3 = Uuid::new_v4();
        
        let available_runners = vec![runner1, runner2, runner3];
        let mut capacities = HashMap::new();
        
        // Runner1: 50% job load, 30% CPU, 40% memory = 40% combined
        capacities.insert(runner1, create_test_capacity(4, 2, 30.0, 40.0));
        // Runner2: 25% job load, 20% CPU, 30% memory = 25% combined  
        capacities.insert(runner2, create_test_capacity(4, 1, 20.0, 30.0));
        // Runner3: 75% job load, 60% CPU, 70% memory = 67.5% combined
        capacities.insert(runner3, create_test_capacity(4, 3, 60.0, 70.0));
        
        let context = create_test_context(available_runners, capacities);
        let runners = HashMap::new();
        
        let selected = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        assert_eq!(selected, runner2); // Should select least loaded
    }
    
    #[tokio::test]
    async fn test_random_load_balancer() {
        let balancer = RandomLoadBalancer::new();
        let runner1 = Uuid::new_v4();
        let runner2 = Uuid::new_v4();
        
        let available_runners = vec![runner1, runner2];
        let capacities = HashMap::new();
        let context = create_test_context(available_runners.clone(), capacities);
        let runners = HashMap::new();
        
        let selected = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        assert!(available_runners.contains(&selected));
    }
    
    #[tokio::test]
    async fn test_weighted_load_balancer() {
        let balancer = WeightedLoadBalancer::new();
        let runner1 = Uuid::new_v4();
        let runner2 = Uuid::new_v4();
        
        let available_runners = vec![runner1, runner2];
        let mut capacities = HashMap::new();
        
        // Runner1: High availability
        capacities.insert(runner1, create_test_capacity(4, 1, 10.0, 20.0));
        // Runner2: Low availability
        capacities.insert(runner2, create_test_capacity(4, 3, 80.0, 90.0));
        
        let context = create_test_context(available_runners, capacities);
        let runners = HashMap::new();
        
        // Run multiple times to test weighted selection
        let mut runner1_count = 0;
        let mut runner2_count = 0;
        
        for _ in 0..100 {
            let selected = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
            if selected == runner1 {
                runner1_count += 1;
            } else {
                runner2_count += 1;
            }
        }
        
        // Runner1 should be selected more often due to higher weight
        assert!(runner1_count > runner2_count);
    }
    
    #[tokio::test]
    async fn test_first_available_load_balancer() {
        let balancer = FirstAvailableLoadBalancer::new();
        let runner1 = Uuid::new_v4();
        let runner2 = Uuid::new_v4();
        let runner3 = Uuid::new_v4();
        
        let available_runners = vec![runner1, runner2, runner3];
        let capacities = HashMap::new();
        let context = create_test_context(available_runners, capacities);
        let runners = HashMap::new();
        
        let selected = balancer.select_runner(&context, &runners).await.unwrap().unwrap();
        assert_eq!(selected, runner1); // Should always select first
    }
    
    #[tokio::test]
    async fn test_empty_runners() {
        let balancer = LeastLoadedLoadBalancer::new();
        let context = create_test_context(vec![], HashMap::new());
        let runners = HashMap::new();
        
        let selected = balancer.select_runner(&context, &runners).await.unwrap();
        assert!(selected.is_none());
    }
}