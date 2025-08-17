// Load Balancing Implementation
// Task 3.1.2: Load Balancing Engine

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use dashmap::DashMap;

/// Service endpoint for load balancing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id: EndpointId,
    pub address: std::net::SocketAddr,
    pub weight: u32,
    pub metadata: EndpointMetadata,
    pub health_status: EndpointHealthStatus,
    pub performance_metrics: EndpointPerformanceMetrics,
}

/// Endpoint metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EndpointMetadata {
    pub service_name: String,
    pub version: String,
    pub region: RegionId,
    pub zone: ZoneId,
    pub capabilities: Vec<String>,
    pub tags: HashMap<String, String>,
}

/// Health status of an endpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Performance metrics for an endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPerformanceMetrics {
    pub response_time: Duration,
    pub throughput: u64,
    pub error_rate: f64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub connection_count: u32,
    #[serde(skip)]
    pub last_updated: Instant,
}

impl Default for EndpointPerformanceMetrics {
    fn default() -> Self {
        Self {
            response_time: Duration::from_millis(0),
            throughput: 0,
            error_rate: 0.0,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            connection_count: 0,
            last_updated: Instant::now(),
        }
    }
}

/// Request context for load balancing decisions
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub service_name: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub source_ip: std::net::IpAddr,
    pub created_at: Instant,
}

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    WeightedLeastConnections,
    ConsistentHashing,
    Random,
    WeightedRandom,
    ResponseTime,
    ResourceBased,
}

/// Health status aggregation
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub endpoint_health: HashMap<EndpointId, EndpointHealth>,
    pub last_updated: Instant,
    pub global_health_score: f64,
}

/// Detailed health information for an endpoint
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub status: EndpointHealthStatus,
    pub response_time: Duration,
    pub error_rate: f64,
    pub throughput: u64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub connection_count: u32,
    pub consecutive_failures: u32,
    pub last_check: Instant,
}

/// Core trait for load balancing algorithms
#[async_trait::async_trait]
pub trait LoadBalancingAlgorithm: Send + Sync {
    async fn select_endpoint(
        &self,
        endpoints: &[ServiceEndpoint],
        request_context: &RequestContext,
        health_status: &HealthStatus,
    ) -> Result<ServiceEndpoint, LoadBalancingError>;

    fn get_strategy_type(&self) -> LoadBalancingStrategy;
    fn supports_session_affinity(&self) -> bool;
    fn supports_weighted_selection(&self) -> bool;
}

/// Load balancing errors
#[derive(Debug, thiserror::Error)]
pub enum LoadBalancingError {
    #[error("No healthy endpoints available")]
    NoHealthyEndpoints,
    
    #[error("No endpoints available for service {service}")]
    NoEndpointsAvailable { service: String },
    
    #[error("Session affinity failed for session {session_id}")]
    SessionAffinityFailed { session_id: String },
    
    #[error("Consistent hashing failed: {reason}")]
    ConsistentHashingFailed { reason: String },
    
    #[error("Algorithm error: {algorithm:?} - {error}")]
    AlgorithmError { algorithm: LoadBalancingStrategy, error: String },
    
    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Load balancer manager
pub struct LoadBalancerManager {
    strategies: HashMap<LoadBalancingStrategy, Arc<dyn LoadBalancingAlgorithm>>,
    health_monitor: Arc<HealthMonitor>,
    metrics_collector: Arc<LoadBalancingMetricsCollector>,
    session_manager: Arc<SessionManager>,
    circuit_breaker: Arc<CircuitBreakerManager>,
}

/// Health monitoring for endpoints
pub struct HealthMonitor {
    health_checks: Arc<RwLock<HashMap<EndpointId, HealthCheck>>>,
    health_status: Arc<RwLock<HealthStatus>>,
    check_interval: Duration,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub endpoint_id: EndpointId,
    pub check_type: HealthCheckType,
    pub interval: Duration,
    pub timeout: Duration,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
    pub last_check: Option<Instant>,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
}

/// Types of health checks
#[derive(Debug, Clone)]
pub enum HealthCheckType {
    Http { path: String, expected_status: u16 },
    Tcp,
    Custom { command: String },
}

/// Session management for sticky sessions
pub struct SessionManager {
    session_mappings: Arc<DashMap<String, EndpointId>>,
    session_ttl: Duration,
}

/// Circuit breaker for failing endpoints
pub struct CircuitBreakerManager {
    circuit_breakers: Arc<DashMap<EndpointId, CircuitBreaker>>,
}

/// Circuit breaker state
#[derive(Debug)]
pub struct CircuitBreaker {
    pub state: CircuitBreakerState,
    pub failure_count: AtomicU64,
    pub success_count: AtomicU64,
    pub last_failure_time: Option<Instant>,
    pub failure_threshold: u64,
    pub recovery_timeout: Duration,
}

impl Clone for CircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            failure_count: AtomicU64::new(self.failure_count.load(std::sync::atomic::Ordering::Relaxed)),
            success_count: AtomicU64::new(self.success_count.load(std::sync::atomic::Ordering::Relaxed)),
            last_failure_time: self.last_failure_time,
            failure_threshold: self.failure_threshold,
            recovery_timeout: self.recovery_timeout,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing, rejecting requests
    HalfOpen, // Testing if service recovered
}

/// Metrics collection for load balancing
pub struct LoadBalancingMetricsCollector {
    pub total_decisions: AtomicU64,
    pub average_decision_time: AtomicU64,
    pub endpoint_selections: Arc<DashMap<EndpointId, AtomicU64>>,
    pub strategy_usage: Arc<DashMap<LoadBalancingStrategy, AtomicU64>>,
    pub health_check_failures: AtomicU64,
    pub circuit_breaker_trips: AtomicU64,
}

// Round Robin Strategy Implementation
pub struct RoundRobinStrategy {
    counter: AtomicUsize,
}

impl RoundRobinStrategy {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LoadBalancingAlgorithm for RoundRobinStrategy {
    async fn select_endpoint(
        &self,
        endpoints: &[ServiceEndpoint],
        _request_context: &RequestContext,
        health_status: &HealthStatus,
    ) -> Result<ServiceEndpoint, LoadBalancingError> {
        // Filter healthy endpoints
        let healthy_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|endpoint| {
                health_status
                    .endpoint_health
                    .get(&endpoint.id)
                    .map(|health| health.status == EndpointHealthStatus::Healthy)
                    .unwrap_or(false)
            })
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(LoadBalancingError::NoHealthyEndpoints);
        }

        // Round robin selection
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % healthy_endpoints.len();
        Ok(healthy_endpoints[index].clone())
    }

    fn get_strategy_type(&self) -> LoadBalancingStrategy {
        LoadBalancingStrategy::RoundRobin
    }

    fn supports_session_affinity(&self) -> bool {
        false
    }

    fn supports_weighted_selection(&self) -> bool {
        false
    }
}

// Weighted Round Robin Strategy Implementation
pub struct WeightedRoundRobinStrategy {
    current_weights: Arc<DashMap<EndpointId, AtomicU64>>,
    effective_weights: Arc<DashMap<EndpointId, AtomicU64>>,
    total_weight: AtomicU64,
}

impl WeightedRoundRobinStrategy {
    pub fn new() -> Self {
        Self {
            current_weights: Arc::new(DashMap::new()),
            effective_weights: Arc::new(DashMap::new()),
            total_weight: AtomicU64::new(0),
        }
    }

    fn update_weights(&self, endpoints: &[ServiceEndpoint]) {
        let mut total = 0u64;
        
        for endpoint in endpoints {
            let weight = endpoint.weight as u64;
            self.current_weights.insert(endpoint.id, AtomicU64::new(weight));
            self.effective_weights.insert(endpoint.id, AtomicU64::new(weight));
            total += weight;
        }
        
        self.total_weight.store(total, Ordering::Relaxed);
    }
}

#[async_trait::async_trait]
impl LoadBalancingAlgorithm for WeightedRoundRobinStrategy {
    async fn select_endpoint(
        &self,
        endpoints: &[ServiceEndpoint],
        _request_context: &RequestContext,
        health_status: &HealthStatus,
    ) -> Result<ServiceEndpoint, LoadBalancingError> {
        // Filter healthy endpoints
        let healthy_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|endpoint| {
                health_status
                    .endpoint_health
                    .get(&endpoint.id)
                    .map(|health| health.status == EndpointHealthStatus::Healthy)
                    .unwrap_or(false)
            })
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(LoadBalancingError::NoHealthyEndpoints);
        }

        // Update weights if needed
        self.update_weights(&healthy_endpoints.iter().map(|e| (*e).clone()).collect::<Vec<_>>());

        // Weighted round robin selection
        let mut selected_endpoint: Option<&ServiceEndpoint> = None;
        let mut max_current_weight = 0i64;

        for endpoint in &healthy_endpoints {
            let current_weight = self.current_weights
                .get(&endpoint.id)
                .map(|w| w.fetch_add(endpoint.weight as u64, Ordering::Relaxed) as i64)
                .unwrap_or(0);

            if current_weight > max_current_weight {
                max_current_weight = current_weight;
                selected_endpoint = Some(endpoint);
            }
        }

        if let Some(endpoint) = selected_endpoint {
            // Decrease the selected endpoint's current weight
            if let Some(weight_ref) = self.current_weights.get(&endpoint.id) {
                let total = self.total_weight.load(Ordering::Relaxed) as i64;
                weight_ref.store((max_current_weight - total) as u64, Ordering::Relaxed);
            }
            
            Ok((*endpoint).clone())
        } else {
            Err(LoadBalancingError::NoHealthyEndpoints)
        }
    }

    fn get_strategy_type(&self) -> LoadBalancingStrategy {
        LoadBalancingStrategy::WeightedRoundRobin
    }

    fn supports_session_affinity(&self) -> bool {
        false
    }

    fn supports_weighted_selection(&self) -> bool {
        true
    }
}

// Consistent Hashing Strategy Implementation
pub struct ConsistentHashingStrategy {
    hash_ring: Arc<RwLock<HashRing>>,
    virtual_nodes: u32,
    hash_function: HashFunction,
}

/// Hash ring for consistent hashing
#[derive(Debug, Clone)]
pub struct HashRing {
    ring: std::collections::BTreeMap<u64, EndpointId>,
    endpoints: HashMap<EndpointId, ServiceEndpoint>,
}

/// Hash function types
#[derive(Debug, Clone, Copy)]
pub enum HashFunction {
    Sha256,
    Fnv1a,
    CityHash,
}

impl ConsistentHashingStrategy {
    pub fn new(virtual_nodes: u32, hash_function: HashFunction) -> Self {
        Self {
            hash_ring: Arc::new(RwLock::new(HashRing::new())),
            virtual_nodes,
            hash_function,
        }
    }

    async fn update_ring(&self, endpoints: &[ServiceEndpoint]) {
        let mut ring = self.hash_ring.write().await;
        ring.update_endpoints(endpoints, self.virtual_nodes, self.hash_function);
    }

    fn hash_request(&self, request_context: &RequestContext) -> u64 {
        // Use session ID if available, otherwise use source IP
        if let Some(session_id) = &request_context.session_id {
            self.hash_function.hash(session_id.as_bytes())
        } else {
            let ip_string = request_context.source_ip.to_string();
            self.hash_function.hash(ip_string.as_bytes())
        }
    }
}

#[async_trait::async_trait]
impl LoadBalancingAlgorithm for ConsistentHashingStrategy {
    async fn select_endpoint(
        &self,
        endpoints: &[ServiceEndpoint],
        request_context: &RequestContext,
        health_status: &HealthStatus,
    ) -> Result<ServiceEndpoint, LoadBalancingError> {
        // Update ring with current endpoints
        self.update_ring(endpoints).await;

        // Generate hash for the request
        let request_hash = self.hash_request(request_context);

        // Find the endpoint in the ring
        let ring = self.hash_ring.read().await;
        let endpoint_id = ring.get_endpoint_for_hash(request_hash)
            .ok_or_else(|| LoadBalancingError::ConsistentHashingFailed {
                reason: "No endpoints in hash ring".to_string(),
            })?;

        // Check if the endpoint is healthy
        let is_healthy = health_status
            .endpoint_health
            .get(&endpoint_id)
            .map(|health| health.status == EndpointHealthStatus::Healthy)
            .unwrap_or(false);

        if !is_healthy {
            // Find next healthy endpoint in the ring
            if let Some(healthy_endpoint_id) = ring.get_next_healthy_endpoint(request_hash, health_status) {
                let endpoint = ring.endpoints.get(&healthy_endpoint_id)
                    .ok_or_else(|| LoadBalancingError::Internal {
                        message: "Endpoint not found in ring".to_string(),
                    })?;
                return Ok(endpoint.clone());
            } else {
                return Err(LoadBalancingError::NoHealthyEndpoints);
            }
        }

        let endpoint = ring.endpoints.get(&endpoint_id)
            .ok_or_else(|| LoadBalancingError::Internal {
                message: "Endpoint not found in ring".to_string(),
            })?;

        Ok(endpoint.clone())
    }

    fn get_strategy_type(&self) -> LoadBalancingStrategy {
        LoadBalancingStrategy::ConsistentHashing
    }

    fn supports_session_affinity(&self) -> bool {
        true
    }

    fn supports_weighted_selection(&self) -> bool {
        false
    }
}

impl HashRing {
    pub fn new() -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            endpoints: HashMap::new(),
        }
    }

    pub fn update_endpoints(
        &mut self,
        endpoints: &[ServiceEndpoint],
        virtual_nodes: u32,
        hash_function: HashFunction,
    ) {
        self.ring.clear();
        self.endpoints.clear();

        for endpoint in endpoints {
            self.endpoints.insert(endpoint.id, endpoint.clone());
            
            // Add virtual nodes
            for i in 0..virtual_nodes {
                let key = format!("{}:{}", endpoint.id, i);
                let hash = hash_function.hash(key.as_bytes());
                self.ring.insert(hash, endpoint.id);
            }
        }
    }

    pub fn get_endpoint_for_hash(&self, hash: u64) -> Option<EndpointId> {
        // Find the first endpoint with hash >= request_hash
        self.ring.range(hash..).next()
            .or_else(|| self.ring.iter().next()) // Wrap around to first endpoint
            .map(|(_, endpoint_id)| *endpoint_id)
    }

    pub fn get_next_healthy_endpoint(
        &self,
        start_hash: u64,
        health_status: &HealthStatus,
    ) -> Option<EndpointId> {
        // Try endpoints starting from start_hash
        for (_, endpoint_id) in self.ring.range(start_hash..) {
            if health_status
                .endpoint_health
                .get(endpoint_id)
                .map(|health| health.status == EndpointHealthStatus::Healthy)
                .unwrap_or(false)
            {
                return Some(*endpoint_id);
            }
        }

        // Wrap around and try from the beginning
        for (_, endpoint_id) in self.ring.iter() {
            if health_status
                .endpoint_health
                .get(endpoint_id)
                .map(|health| health.status == EndpointHealthStatus::Healthy)
                .unwrap_or(false)
            {
                return Some(*endpoint_id);
            }
        }

        None
    }
}

impl HashFunction {
    pub fn hash(&self, data: &[u8]) -> u64 {
        match self {
            HashFunction::Sha256 => {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(data);
                let result = hasher.finalize();
                u64::from_be_bytes([
                    result[0], result[1], result[2], result[3],
                    result[4], result[5], result[6], result[7],
                ])
            }
            HashFunction::Fnv1a => {
                let mut hash = 0xcbf29ce484222325u64;
                for &byte in data {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hash
            }
            HashFunction::CityHash => {
                // Simplified CityHash implementation
                let mut hash = 0u64;
                for &byte in data {
                    hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
                }
                hash
            }
        }
    }
}

// Health-Aware Load Balancer Implementation
pub struct HealthAwareLoadBalancer {
    base_strategy: Arc<dyn LoadBalancingAlgorithm>,
    health_monitor: Arc<HealthMonitor>,
    circuit_breaker: Arc<CircuitBreakerManager>,
    failure_detector: Arc<FailureDetector>,
}

/// Failure detection for endpoints
pub struct FailureDetector {
    failure_history: Arc<DashMap<EndpointId, Vec<Instant>>>,
    failure_window: Duration,
    failure_threshold: u32,
}

impl HealthAwareLoadBalancer {
    pub fn new(
        base_strategy: Arc<dyn LoadBalancingAlgorithm>,
        health_monitor: Arc<HealthMonitor>,
        circuit_breaker: Arc<CircuitBreakerManager>,
        failure_detector: Arc<FailureDetector>,
    ) -> Self {
        Self {
            base_strategy,
            health_monitor,
            circuit_breaker,
            failure_detector,
        }
    }
}

#[async_trait::async_trait]
impl LoadBalancingAlgorithm for HealthAwareLoadBalancer {
    async fn select_endpoint(
        &self,
        endpoints: &[ServiceEndpoint],
        request_context: &RequestContext,
        health_status: &HealthStatus,
    ) -> Result<ServiceEndpoint, LoadBalancingError> {
        // Filter out endpoints with open circuit breakers
        let available_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|endpoint| {
                self.circuit_breaker
                    .is_endpoint_available(&endpoint.id)
            })
            .cloned()
            .collect();

        if available_endpoints.is_empty() {
            return Err(LoadBalancingError::NoHealthyEndpoints);
        }

        // Use base strategy to select from available endpoints
        self.base_strategy
            .select_endpoint(&available_endpoints, request_context, health_status)
            .await
    }

    fn get_strategy_type(&self) -> LoadBalancingStrategy {
        self.base_strategy.get_strategy_type()
    }

    fn supports_session_affinity(&self) -> bool {
        self.base_strategy.supports_session_affinity()
    }

    fn supports_weighted_selection(&self) -> bool {
        self.base_strategy.supports_weighted_selection()
    }
}

impl LoadBalancerManager {
    pub fn new(
        health_monitor: Arc<HealthMonitor>,
        metrics_collector: Arc<LoadBalancingMetricsCollector>,
        session_manager: Arc<SessionManager>,
        circuit_breaker: Arc<CircuitBreakerManager>,
    ) -> Self {
        let mut strategies: HashMap<LoadBalancingStrategy, Arc<dyn LoadBalancingAlgorithm>> = HashMap::new();
        
        // Register default strategies
        strategies.insert(
            LoadBalancingStrategy::RoundRobin,
            Arc::new(RoundRobinStrategy::new()),
        );
        strategies.insert(
            LoadBalancingStrategy::WeightedRoundRobin,
            Arc::new(WeightedRoundRobinStrategy::new()),
        );
        strategies.insert(
            LoadBalancingStrategy::ConsistentHashing,
            Arc::new(ConsistentHashingStrategy::new(150, HashFunction::Fnv1a)),
        );

        Self {
            strategies,
            health_monitor,
            metrics_collector,
            session_manager,
            circuit_breaker,
        }
    }

    pub async fn select_endpoint(
        &self,
        strategy: LoadBalancingStrategy,
        endpoints: &[ServiceEndpoint],
        request_context: &RequestContext,
    ) -> Result<ServiceEndpoint, LoadBalancingError> {
        let start_time = Instant::now();

        // Get health status
        let health_status = self.health_monitor.get_health_status().await;

        // Get strategy
        let algorithm = self.strategies.get(&strategy)
            .ok_or_else(|| LoadBalancingError::AlgorithmError {
                algorithm: strategy,
                error: "Strategy not found".to_string(),
            })?;

        // Select endpoint
        let result = algorithm
            .select_endpoint(endpoints, request_context, &health_status)
            .await;

        // Record metrics
        let duration = start_time.elapsed();
        self.metrics_collector.record_decision(strategy, duration, &result).await;

        result
    }

    pub fn register_strategy(
        &mut self,
        strategy: LoadBalancingStrategy,
        algorithm: Arc<dyn LoadBalancingAlgorithm>,
    ) {
        self.strategies.insert(strategy, algorithm);
    }

    pub async fn get_metrics(&self) -> LoadBalancingMetrics {
        self.metrics_collector.get_metrics().await
    }
}

/// Load balancing metrics
#[derive(Debug, Clone)]
pub struct LoadBalancingMetrics {
    pub total_decisions: u64,
    pub average_decision_time: Duration,
    pub endpoint_selections: HashMap<EndpointId, u64>,
    pub strategy_usage: HashMap<LoadBalancingStrategy, u64>,
    pub health_check_failures: u64,
    pub circuit_breaker_trips: u64,
}

impl LoadBalancingMetricsCollector {
    pub fn new() -> Self {
        Self {
            total_decisions: AtomicU64::new(0),
            average_decision_time: AtomicU64::new(0),
            endpoint_selections: Arc::new(DashMap::new()),
            strategy_usage: Arc::new(DashMap::new()),
            health_check_failures: AtomicU64::new(0),
            circuit_breaker_trips: AtomicU64::new(0),
        }
    }

    pub async fn record_decision(
        &self,
        strategy: LoadBalancingStrategy,
        duration: Duration,
        result: &Result<ServiceEndpoint, LoadBalancingError>,
    ) {
        self.total_decisions.fetch_add(1, Ordering::Relaxed);
        
        // Update average decision time
        let duration_nanos = duration.as_nanos() as u64;
        let current_avg = self.average_decision_time.load(Ordering::Relaxed);
        let total = self.total_decisions.load(Ordering::Relaxed);
        let new_avg = (current_avg * (total - 1) + duration_nanos) / total;
        self.average_decision_time.store(new_avg, Ordering::Relaxed);

        // Record strategy usage
        self.strategy_usage
            .entry(strategy)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);

        // Record endpoint selection
        if let Ok(endpoint) = result {
            self.endpoint_selections
                .entry(endpoint.id)
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub async fn get_metrics(&self) -> LoadBalancingMetrics {
        let endpoint_selections = self.endpoint_selections
            .iter()
            .map(|entry| (*entry.key(), entry.value().load(Ordering::Relaxed)))
            .collect();

        let strategy_usage = self.strategy_usage
            .iter()
            .map(|entry| (*entry.key(), entry.value().load(Ordering::Relaxed)))
            .collect();

        LoadBalancingMetrics {
            total_decisions: self.total_decisions.load(Ordering::Relaxed),
            average_decision_time: Duration::from_nanos(
                self.average_decision_time.load(Ordering::Relaxed)
            ),
            endpoint_selections,
            strategy_usage,
            health_check_failures: self.health_check_failures.load(Ordering::Relaxed),
            circuit_breaker_trips: self.circuit_breaker_trips.load(Ordering::Relaxed),
        }
    }
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            circuit_breakers: Arc::new(DashMap::new()),
        }
    }

    pub fn is_endpoint_available(&self, endpoint_id: &EndpointId) -> bool {
        if let Some(cb) = self.circuit_breakers.get(endpoint_id) {
            match cb.state {
                CircuitBreakerState::Closed => true,
                CircuitBreakerState::Open => {
                    // Check if recovery timeout has passed
                    if let Some(last_failure) = cb.last_failure_time {
                        Instant::now().duration_since(last_failure) > cb.recovery_timeout
                    } else {
                        false
                    }
                }
                CircuitBreakerState::HalfOpen => true,
            }
        } else {
            true // No circuit breaker, assume available
        }
    }

    pub fn record_success(&self, endpoint_id: &EndpointId) {
        if let Some(mut cb) = self.circuit_breakers.get_mut(endpoint_id) {
            cb.success_count.fetch_add(1, Ordering::Relaxed);
            
            match cb.state {
                CircuitBreakerState::HalfOpen => {
                    // Transition back to closed
                    cb.state = CircuitBreakerState::Closed;
                    cb.failure_count.store(0, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }

    pub fn record_failure(&self, endpoint_id: &EndpointId) {
        let mut cb = self.circuit_breakers
            .entry(*endpoint_id)
            .or_insert_with(|| CircuitBreaker {
                state: CircuitBreakerState::Closed,
                failure_count: AtomicU64::new(0),
                success_count: AtomicU64::new(0),
                last_failure_time: None,
                failure_threshold: 5,
                recovery_timeout: Duration::from_secs(30),
            });

        let failures = cb.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        cb.last_failure_time = Some(Instant::now());

        if failures >= cb.failure_threshold && cb.state == CircuitBreakerState::Closed {
            cb.state = CircuitBreakerState::Open;
        }
    }
}

impl HealthMonitor {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            health_checks: Arc::new(RwLock::new(HashMap::new())),
            health_status: Arc::new(RwLock::new(HealthStatus {
                endpoint_health: HashMap::new(),
                last_updated: Instant::now(),
                global_health_score: 1.0,
            })),
            check_interval,
        }
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_status.read().await.clone()
    }

    pub async fn update_endpoint_health(&self, endpoint_id: EndpointId, health: EndpointHealth) {
        let mut status = self.health_status.write().await;
        status.endpoint_health.insert(endpoint_id, health);
        status.last_updated = Instant::now();
        
        // Recalculate global health score
        let healthy_count = status.endpoint_health.values()
            .filter(|h| h.status == EndpointHealthStatus::Healthy)
            .count();
        let total_count = status.endpoint_health.len();
        
        status.global_health_score = if total_count > 0 {
            healthy_count as f64 / total_count as f64
        } else {
            0.0
        };
    }
}

impl Default for EndpointHealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}