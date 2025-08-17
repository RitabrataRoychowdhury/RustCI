//! Load Balancer for Service Registry
//! 
//! Implements intelligent load balancing with multiple strategies,
//! health-aware routing, and performance optimization.

use std::collections::HashMap;
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use dashmap::DashMap;
use crate::error::AppError;
use tracing::info;

use super::{ServiceEntry, ServiceEndpoint, EndpointId, HealthStatus};
use crate::error::{Result, ValkyrieError};

/// Load balancer with multiple strategies
pub struct LoadBalancer {
    /// Current load balancing strategy
    strategy: LoadBalancingStrategy,
    /// Strategy implementations
    strategies: Arc<DashMap<LoadBalancingStrategy, Box<dyn LoadBalancingAlgorithm>>>,
    /// Endpoint state tracking
    endpoint_state: Arc<DashMap<EndpointId, EndpointState>>,
    /// Load balancing metrics
    metrics: Arc<RwLock<LoadBalancingMetrics>>,
    /// Configuration
    config: LoadBalancerConfig,
    /// Round-robin counter
    round_robin_counter: std::sync::atomic::AtomicUsize,
}

// Use the canonical LoadBalancingStrategy from types
pub use crate::core::networking::valkyrie::types::LoadBalancingStrategy;

/// Load balancing algorithm trait
pub trait LoadBalancingAlgorithm: Send + Sync {
    /// Select best endpoint from available services
    fn select_endpoint(&self, services: &[ServiceEntry], state: &DashMap<EndpointId, EndpointState>) -> Result<ServiceEndpoint>;
    
    /// Algorithm name
    fn name(&self) -> &str;
    
    /// Update algorithm state after request
    fn update_state(&self, endpoint_id: EndpointId, response_time: Duration, success: bool, state: &DashMap<EndpointId, EndpointState>);
    
    /// Algorithm priority (lower = higher priority)
    fn priority(&self) -> u8;
}

/// Endpoint state for load balancing
#[derive(Debug, Clone)]
pub struct EndpointState {
    /// Endpoint ID
    pub endpoint_id: EndpointId,
    /// Current active connections
    pub active_connections: u32,
    /// Total requests served
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Average response time
    pub avg_response_time: Duration,
    /// Last request timestamp
    pub last_request: Instant,
    /// Current weight (can be adjusted dynamically)
    pub current_weight: u32,
    /// Health score (0.0 - 1.0)
    pub health_score: f64,
    /// Resource utilization
    pub resource_utilization: ResourceUtilization,
    /// Round-robin position
    pub rr_position: u32,
}

/// Resource utilization metrics
#[derive(Debug, Clone)]
pub struct ResourceUtilization {
    /// CPU utilization (0.0 - 1.0)
    pub cpu: f64,
    /// Memory utilization (0.0 - 1.0)
    pub memory: f64,
    /// Network utilization (0.0 - 1.0)
    pub network: f64,
    /// Disk utilization (0.0 - 1.0)
    pub disk: f64,
}

/// Load balancer configuration
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    /// Enable health-aware routing
    pub health_aware: bool,
    /// Minimum health score for routing
    pub min_health_score: f64,
    /// Enable sticky sessions
    pub sticky_sessions: bool,
    /// Session timeout
    pub session_timeout: Duration,
    /// Enable circuit breaker integration
    pub circuit_breaker_integration: bool,
    /// Response time weight in scoring
    pub response_time_weight: f64,
    /// Connection count weight in scoring
    pub connection_weight: f64,
    /// Health score weight in scoring
    pub health_weight: f64,
    /// Resource utilization weight in scoring
    pub resource_weight: f64,
}

/// Load balancing metrics
#[derive(Debug, Clone)]
pub struct LoadBalancingMetrics {
    /// Total load balancing decisions
    pub total_decisions: u64,
    /// Decisions by strategy
    pub decisions_by_strategy: HashMap<LoadBalancingStrategy, u64>,
    /// Average decision latency
    pub avg_decision_latency: Duration,
    /// Endpoint selection distribution
    pub endpoint_distribution: HashMap<EndpointId, u64>,
    /// Failed selections (no healthy endpoints)
    pub failed_selections: u64,
    /// Strategy switches
    pub strategy_switches: u64,
}

/// Round-robin load balancing algorithm
pub struct RoundRobinAlgorithm {
    current_index: Arc<RwLock<usize>>,
}

/// Weighted round-robin algorithm
pub struct WeightedRoundRobinAlgorithm {
    current_weights: Arc<RwLock<HashMap<EndpointId, i32>>>,
}

/// Least connections algorithm
pub struct LeastConnectionsAlgorithm;

/// Weighted least connections algorithm
pub struct WeightedLeastConnectionsAlgorithm;

/// Random selection algorithm
pub struct RandomAlgorithm;

/// Weighted random algorithm
pub struct WeightedRandomAlgorithm;

/// Response time based algorithm
pub struct ResponseTimeAlgorithm;

/// Resource-based algorithm
pub struct ResourceBasedAlgorithm;

/// Consistent hashing algorithm
pub struct ConsistentHashAlgorithm {
    hash_ring: Arc<RwLock<HashMap<u64, EndpointId>>>,
}

/// Adaptive algorithm using ML
pub struct AdaptiveAlgorithm {
    model_weights: Arc<RwLock<HashMap<String, f64>>>,
    learning_rate: f64,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            health_aware: true,
            min_health_score: 0.5,
            sticky_sessions: false,
            session_timeout: Duration::from_secs(300),
            circuit_breaker_integration: true,
            response_time_weight: 0.3,
            connection_weight: 0.2,
            health_weight: 0.3,
            resource_weight: 0.2,
        }
    }
}

impl LoadBalancer {
    /// Create new load balancer
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self::with_config(strategy, LoadBalancerConfig::default())
    }

    /// Create load balancer with custom configuration
    pub fn with_config(strategy: LoadBalancingStrategy, config: LoadBalancerConfig) -> Self {
        let mut load_balancer = Self {
            strategy: strategy.clone(),
            strategies: Arc::new(DashMap::new()),
            endpoint_state: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(LoadBalancingMetrics::default())),
            config,
            round_robin_counter: std::sync::atomic::AtomicUsize::new(0),
        };

        // Initialize strategies
        load_balancer.initialize_strategies();
        load_balancer
    }

    /// Initialize load balancing strategies
    fn initialize_strategies(&mut self) {
        // This is a simplified approach - in practice we'd need proper async initialization
        info!("Initialized load balancing strategies");
    }

    /// Select best endpoint from services
    pub async fn select_endpoint(&self, services: &[ServiceEntry]) -> Result<ServiceEndpoint> {
        let start_time = Instant::now();

        if services.is_empty() {
            return Err(ValkyrieError::NoHealthyEndpoints("No services available".to_string()));
        }

        // Filter healthy services if health-aware routing is enabled
        let available_services = if self.config.health_aware {
            services.iter()
                .filter(|s| matches!(s.health_status, HealthStatus::Healthy) || 
                           matches!(s.health_status, HealthStatus::Degraded { .. }))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            services.to_vec()
        };

        if available_services.is_empty() {
            self.update_failed_selection_metrics().await;
            return Err(ValkyrieError::NoHealthyEndpoints("No healthy services available".to_string()));
        }

        // Get all healthy endpoints
        let mut healthy_endpoints = Vec::new();
        for service in &available_services {
            for endpoint in &service.endpoints {
                if self.is_endpoint_healthy(endpoint).await {
                    healthy_endpoints.push(endpoint.clone());
                }
            }
        }

        if healthy_endpoints.is_empty() {
            self.update_failed_selection_metrics().await;
            return Err(ValkyrieError::NoHealthyEndpoints("No healthy endpoints available".to_string()));
        }

        // Select endpoint using current strategy
        let selected_endpoint = self.select_using_strategy(&healthy_endpoints).await?;

        // Update metrics
        self.update_selection_metrics(&selected_endpoint, start_time.elapsed()).await;

        Ok(selected_endpoint)
    }

    /// Select endpoint using current strategy
    async fn select_using_strategy(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.round_robin_select(endpoints).await
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                self.weighted_round_robin_select(endpoints).await
            }
            LoadBalancingStrategy::LeastConnections => {
                self.least_connections_select(endpoints).await
            }
            LoadBalancingStrategy::WeightedLeastConnections => {
                self.weighted_least_connections_select(endpoints).await
            }
            LoadBalancingStrategy::Random => {
                self.random_select(endpoints).await
            }
            LoadBalancingStrategy::WeightedRandom => {
                self.weighted_random_select(endpoints).await
            }
            LoadBalancingStrategy::ResponseTime => {
                self.response_time_select(endpoints).await
            }
            LoadBalancingStrategy::ResourceBased => {
                self.resource_based_select(endpoints).await
            }
            LoadBalancingStrategy::ConsistentHashing => {
                self.consistent_hash_select(endpoints).await
            }
            LoadBalancingStrategy::Adaptive => {
                self.adaptive_select(endpoints).await
            }
            LoadBalancingStrategy::LatencyBased => {
                // Select endpoint with lowest latency
                endpoints.iter()
                    .min_by_key(|e| e.latency.unwrap_or(Duration::from_millis(1000)))
                    .cloned()
                    .ok_or_else(|| AppError::LoadBalancingError("No endpoint selected".to_string()))
            },
            LoadBalancingStrategy::Custom(_) => {
                // Use round-robin as fallback for custom strategies
                let index = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % endpoints.len();
                Ok(endpoints[index].clone())
            },
        }
    }

    /// Round-robin endpoint selection
    async fn round_robin_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        static mut COUNTER: usize = 0;
        
        unsafe {
            let index = COUNTER % endpoints.len();
            COUNTER = (COUNTER + 1) % endpoints.len();
            Ok(endpoints[index].clone())
        }
    }

    /// Weighted round-robin endpoint selection
    async fn weighted_round_robin_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut best_score = -1i32;

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            let current_weight = state.current_weight as i32;
            
            if current_weight > best_score {
                best_score = current_weight;
                best_endpoint = Some(endpoint.clone());
            }
        }

        // Decrease weight of selected endpoint and increase others
        if let Some(ref selected) = best_endpoint {
            let mut selected_state = self.get_or_create_endpoint_state(selected.endpoint_id).await;
            selected_state.current_weight = selected_state.current_weight.saturating_sub(1);
            self.endpoint_state.insert(selected.endpoint_id, selected_state);

            // Increase weights of other endpoints
            for endpoint in endpoints {
                if endpoint.endpoint_id != selected.endpoint_id {
                    let mut state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
                    state.current_weight += endpoint.weight;
                    self.endpoint_state.insert(endpoint.endpoint_id, state);
                }
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Least connections endpoint selection
    async fn least_connections_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut min_connections = u32::MAX;

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            if state.active_connections < min_connections {
                min_connections = state.active_connections;
                best_endpoint = Some(endpoint.clone());
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Weighted least connections endpoint selection
    async fn weighted_least_connections_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut best_ratio = f64::MAX;

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            let ratio = if endpoint.weight > 0 {
                state.active_connections as f64 / endpoint.weight as f64
            } else {
                f64::MAX
            };

            if ratio < best_ratio {
                best_ratio = ratio;
                best_endpoint = Some(endpoint.clone());
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Random endpoint selection
    async fn random_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..endpoints.len());
        Ok(endpoints[index].clone())
    }

    /// Weighted random endpoint selection
    async fn weighted_random_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        use rand::Rng;
        
        let total_weight: u32 = endpoints.iter().map(|e| e.weight).sum();
        if total_weight == 0 {
            return self.random_select(endpoints).await;
        }

        let mut rng = rand::thread_rng();
        let mut random_weight = rng.gen_range(0..total_weight);

        for endpoint in endpoints {
            if random_weight < endpoint.weight {
                return Ok(endpoint.clone());
            }
            random_weight -= endpoint.weight;
        }

        // Fallback to last endpoint
        Ok(endpoints[endpoints.len() - 1].clone())
    }

    /// Response time based endpoint selection
    async fn response_time_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut best_response_time = Duration::from_secs(u64::MAX);

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            if state.avg_response_time < best_response_time {
                best_response_time = state.avg_response_time;
                best_endpoint = Some(endpoint.clone());
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Resource-based endpoint selection
    async fn resource_based_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut best_score = f64::MAX;

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            let resource_score = self.calculate_resource_score(&state.resource_utilization);
            
            if resource_score < best_score {
                best_score = resource_score;
                best_endpoint = Some(endpoint.clone());
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Consistent hash endpoint selection
    async fn consistent_hash_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        // Simplified consistent hashing - in practice would use proper hash ring
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        let hash = hasher.finish();
        
        let index = (hash as usize) % endpoints.len();
        Ok(endpoints[index].clone())
    }

    /// Adaptive endpoint selection using ML
    async fn adaptive_select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint> {
        let mut best_endpoint = None;
        let mut best_score = f64::MIN;

        for endpoint in endpoints {
            let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
            let score = self.calculate_adaptive_score(endpoint, &state).await;
            
            if score > best_score {
                best_score = score;
                best_endpoint = Some(endpoint.clone());
            }
        }

        best_endpoint.ok_or_else(|| ValkyrieError::LoadBalancingError("No endpoint selected".to_string()))
    }

    /// Calculate resource utilization score
    fn calculate_resource_score(&self, utilization: &ResourceUtilization) -> f64 {
        // Weighted average of resource utilization
        (utilization.cpu * 0.4) + 
        (utilization.memory * 0.3) + 
        (utilization.network * 0.2) + 
        (utilization.disk * 0.1)
    }

    /// Calculate adaptive score using ML
    async fn calculate_adaptive_score(&self, endpoint: &ServiceEndpoint, state: &EndpointState) -> f64 {
        // Simplified ML scoring - in practice would use trained models
        let response_time_score = 1.0 / (state.avg_response_time.as_millis() as f64 + 1.0);
        let connection_score = 1.0 / (state.active_connections as f64 + 1.0);
        let health_score = state.health_score;
        let resource_score = 1.0 - self.calculate_resource_score(&state.resource_utilization);

        // Weighted combination
        (response_time_score * self.config.response_time_weight) +
        (connection_score * self.config.connection_weight) +
        (health_score * self.config.health_weight) +
        (resource_score * self.config.resource_weight)
    }

    /// Check if endpoint is healthy
    async fn is_endpoint_healthy(&self, endpoint: &ServiceEndpoint) -> bool {
        if !self.config.health_aware {
            return true;
        }

        match endpoint.health_status {
            HealthStatus::Healthy => true,
            HealthStatus::Degraded { .. } => {
                let state = self.get_or_create_endpoint_state(endpoint.endpoint_id).await;
                state.health_score >= self.config.min_health_score
            }
            _ => false,
        }
    }

    /// Get or create endpoint state
    async fn get_or_create_endpoint_state(&self, endpoint_id: EndpointId) -> EndpointState {
        if let Some(state) = self.endpoint_state.get(&endpoint_id) {
            state.clone()
        } else {
            let new_state = EndpointState {
                endpoint_id,
                active_connections: 0,
                total_requests: 0,
                successful_requests: 0,
                avg_response_time: Duration::from_millis(50), // Default
                last_request: Instant::now(),
                current_weight: 100, // Default weight
                health_score: 1.0,
                resource_utilization: ResourceUtilization::default(),
                rr_position: 0,
            };
            self.endpoint_state.insert(endpoint_id, new_state.clone());
            new_state
        }
    }

    /// Update endpoint state after request
    pub async fn update_endpoint_state(
        &self,
        endpoint_id: EndpointId,
        response_time: Duration,
        success: bool,
    ) {
        if let Some(mut state) = self.endpoint_state.get_mut(&endpoint_id) {
            state.total_requests += 1;
            if success {
                state.successful_requests += 1;
            }

            // Update average response time
            let total_time = state.avg_response_time.as_nanos() * (state.total_requests - 1) as u128
                + response_time.as_nanos();
            state.avg_response_time = Duration::from_nanos((total_time / state.total_requests as u128) as u64);

            // Update health score based on success rate
            state.health_score = state.successful_requests as f64 / state.total_requests as f64;

            state.last_request = Instant::now();
        }
    }

    /// Increment active connections for endpoint
    pub async fn increment_connections(&self, endpoint_id: EndpointId) {
        if let Some(mut state) = self.endpoint_state.get_mut(&endpoint_id) {
            state.active_connections += 1;
        }
    }

    /// Decrement active connections for endpoint
    pub async fn decrement_connections(&self, endpoint_id: EndpointId) {
        if let Some(mut state) = self.endpoint_state.get_mut(&endpoint_id) {
            state.active_connections = state.active_connections.saturating_sub(1);
        }
    }

    /// Update selection metrics
    async fn update_selection_metrics(&self, endpoint: &ServiceEndpoint, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_decisions += 1;
        
        *metrics.decisions_by_strategy.entry(self.strategy.clone()).or_insert(0) += 1;
        *metrics.endpoint_distribution.entry(endpoint.endpoint_id).or_insert(0) += 1;

        // Update average decision latency
        let total_latency = metrics.avg_decision_latency.as_nanos() * (metrics.total_decisions - 1) as u128
            + latency.as_nanos();
        metrics.avg_decision_latency = Duration::from_nanos((total_latency / metrics.total_decisions as u128) as u64);
    }

    /// Update failed selection metrics
    async fn update_failed_selection_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.failed_selections += 1;
    }

    /// Change load balancing strategy
    pub async fn change_strategy(&mut self, new_strategy: LoadBalancingStrategy) {
        if self.strategy != new_strategy {
            self.strategy = new_strategy;
            
            let mut metrics = self.metrics.write().await;
            metrics.strategy_switches += 1;
            
            info!("Changed load balancing strategy to: {:?}", self.strategy);
        }
    }

    /// Get load balancing metrics
    pub async fn metrics(&self) -> LoadBalancingMetrics {
        self.metrics.read().await.clone()
    }

    /// Get endpoint states
    pub async fn endpoint_states(&self) -> Vec<EndpointState> {
        self.endpoint_state.iter().map(|entry| entry.clone()).collect()
    }
}

impl Default for ResourceUtilization {
    fn default() -> Self {
        Self {
            cpu: 0.5,
            memory: 0.5,
            network: 0.3,
            disk: 0.2,
        }
    }
}

impl Default for LoadBalancingMetrics {
    fn default() -> Self {
        Self {
            total_decisions: 0,
            decisions_by_strategy: HashMap::new(),
            avg_decision_latency: Duration::from_nanos(0),
            endpoint_distribution: HashMap::new(),
            failed_selections: 0,
            strategy_switches: 0,
        }
    }
}

impl std::fmt::Display for LoadBalancingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadBalancingStrategy::RoundRobin => write!(f, "RoundRobin"),
            LoadBalancingStrategy::WeightedRoundRobin => write!(f, "WeightedRoundRobin"),
            LoadBalancingStrategy::LeastConnections => write!(f, "LeastConnections"),
            LoadBalancingStrategy::WeightedLeastConnections => write!(f, "WeightedLeastConnections"),
            LoadBalancingStrategy::Random => write!(f, "Random"),
            LoadBalancingStrategy::WeightedRandom => write!(f, "WeightedRandom"),
            LoadBalancingStrategy::ResponseTime => write!(f, "ResponseTime"),
            LoadBalancingStrategy::ResourceBased => write!(f, "ResourceBased"),
            LoadBalancingStrategy::ConsistentHashing => write!(f, "ConsistentHash"),
            LoadBalancingStrategy::Adaptive => write!(f, "Adaptive"),
        }
    }
}