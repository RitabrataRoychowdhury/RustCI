use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    pub algorithm: LoadBalancingAlgorithm,
    pub health_check_interval: Duration,
    pub health_check_timeout: Duration,
    pub max_failures: u32,
    pub recovery_time: Duration,
    pub sticky_sessions: bool,
    pub session_timeout: Duration,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            algorithm: LoadBalancingAlgorithm::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            max_failures: 3,
            recovery_time: Duration::from_secs(60),
            sticky_sessions: false,
            session_timeout: Duration::from_secs(1800), // 30 minutes
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    WeightedLeastConnections,
    IpHash,
    Random,
    HealthAware,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id: Uuid,
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub health_status: HealthStatus,
    pub current_connections: u32,
    pub max_connections: u32,
    pub response_time: Duration,
    pub failure_count: u32,
    #[serde(skip, default = "Instant::now")]
    pub last_health_check: Instant,
    #[serde(skip)]
    pub last_failure: Option<Instant>,
}

impl ServiceEndpoint {
    pub fn new(address: String, port: u16) -> Self {
        Self {
            id: Uuid::new_v4(),
            address,
            port,
            weight: 100,
            health_status: HealthStatus::Unknown,
            current_connections: 0,
            max_connections: 1000,
            response_time: Duration::from_millis(0),
            failure_count: 0,
            last_health_check: Instant::now(),
            last_failure: None,
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self.health_status, HealthStatus::Healthy)
    }

    pub fn is_available(&self) -> bool {
        self.is_healthy() && self.current_connections < self.max_connections
    }

    pub fn connection_utilization(&self) -> f64 {
        if self.max_connections == 0 {
            0.0
        } else {
            self.current_connections as f64 / self.max_connections as f64
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Degraded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time: Duration,
    pub active_connections: u32,
    pub healthy_endpoints: u32,
    pub total_endpoints: u32,
    pub requests_per_endpoint: HashMap<Uuid, u64>,
}

#[derive(Debug, Clone)]
pub struct RoutingContext {
    pub client_ip: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Uuid,
    pub headers: HashMap<String, String>,
    pub path: String,
    pub method: String,
}

impl RoutingContext {
    pub fn new(path: String, method: String) -> Self {
        Self {
            client_ip: None,
            session_id: None,
            request_id: Uuid::new_v4(),
            headers: HashMap::new(),
            path,
            method,
        }
    }
}

pub trait LoadBalancer: Send + Sync {
    fn select_endpoint(&self, context: &RoutingContext) -> impl std::future::Future<Output = Result<ServiceEndpoint, AppError>> + Send;
    fn add_endpoint(&self, endpoint: ServiceEndpoint) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn remove_endpoint(&self, endpoint_id: Uuid) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn update_endpoint_health(&self, endpoint_id: Uuid, status: HealthStatus) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn record_request_start(&self, endpoint_id: Uuid) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn record_request_end(&self, endpoint_id: Uuid, success: bool, response_time: Duration) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn get_stats(&self) -> impl std::future::Future<Output = LoadBalancerStats> + Send;
    fn get_healthy_endpoints(&self) -> impl std::future::Future<Output = Vec<ServiceEndpoint>> + Send;
}

pub struct ProductionLoadBalancer {
    config: LoadBalancerConfig,
    endpoints: Arc<RwLock<HashMap<Uuid, ServiceEndpoint>>>,
    stats: Arc<RwLock<LoadBalancerStats>>,
    round_robin_index: Arc<RwLock<usize>>,
    session_affinity: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl ProductionLoadBalancer {
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(LoadBalancerStats {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                average_response_time: Duration::from_millis(0),
                active_connections: 0,
                healthy_endpoints: 0,
                total_endpoints: 0,
                requests_per_endpoint: HashMap::new(),
            })),
            round_robin_index: Arc::new(RwLock::new(0)),
            session_affinity: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_health_checks(&self) {
        let endpoints = Arc::clone(&self.endpoints);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            
            loop {
                interval.tick().await;
                
                let endpoints_map = endpoints.read().await;
                let endpoint_ids: Vec<Uuid> = endpoints_map.keys().cloned().collect();
                drop(endpoints_map);
                
                for endpoint_id in endpoint_ids {
                    let endpoints_clone = Arc::clone(&endpoints);
                    let config_clone = config.clone();
                    
                    tokio::spawn(async move {
                        if let Some(endpoint) = {
                            let endpoints = endpoints_clone.read().await;
                            endpoints.get(&endpoint_id).cloned()
                        } {
                            let health_status = Self::check_endpoint_health(&endpoint, &config_clone).await;
                            
                            let mut endpoints = endpoints_clone.write().await;
                            if let Some(endpoint) = endpoints.get_mut(&endpoint_id) {
                                endpoint.health_status = health_status;
                                endpoint.last_health_check = Instant::now();
                                
                                if health_status == HealthStatus::Unhealthy {
                                    endpoint.failure_count += 1;
                                    endpoint.last_failure = Some(Instant::now());
                                } else if health_status == HealthStatus::Healthy {
                                    endpoint.failure_count = 0;
                                    endpoint.last_failure = None;
                                }
                            }
                        }
                    });
                }
            }
        });
    }

    async fn check_endpoint_health(endpoint: &ServiceEndpoint, config: &LoadBalancerConfig) -> HealthStatus {
        // Simple TCP connection check
        match tokio::time::timeout(
            config.health_check_timeout,
            tokio::net::TcpStream::connect(format!("{}:{}", endpoint.address, endpoint.port))
        ).await {
            Ok(Ok(_)) => {
                // Additional checks could be added here (HTTP health endpoint, etc.)
                HealthStatus::Healthy
            }
            Ok(Err(_)) | Err(_) => {
                if endpoint.failure_count >= config.max_failures {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Degraded
                }
            }
        }
    }

    async fn select_round_robin(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        let mut index = self.round_robin_index.write().await;
        let selected = healthy_endpoints[*index % healthy_endpoints.len()];
        *index = (*index + 1) % healthy_endpoints.len();

        Ok(selected.clone())
    }

    async fn select_weighted_round_robin(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        // Create weighted list
        let mut weighted_endpoints = Vec::new();
        for endpoint in &healthy_endpoints {
            for _ in 0..endpoint.weight {
                weighted_endpoints.push(*endpoint);
            }
        }

        if weighted_endpoints.is_empty() {
            return Ok(healthy_endpoints[0].clone());
        }

        let mut index = self.round_robin_index.write().await;
        let selected = weighted_endpoints[*index % weighted_endpoints.len()];
        *index = (*index + 1) % weighted_endpoints.len();

        Ok(selected.clone())
    }

    async fn select_least_connections(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        let selected = healthy_endpoints
            .iter()
            .min_by_key(|e| e.current_connections)
            .unwrap();

        Ok((*selected).clone())
    }

    async fn select_weighted_least_connections(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        let selected = healthy_endpoints
            .iter()
            .min_by(|a, b| {
                let a_ratio = a.current_connections as f64 / a.weight as f64;
                let b_ratio = b.current_connections as f64 / b.weight as f64;
                a_ratio.partial_cmp(&b_ratio).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        Ok((*selected).clone())
    }

    async fn select_ip_hash(&self, context: &RoutingContext) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        let hash = if let Some(client_ip) = &context.client_ip {
            self.hash_string(client_ip)
        } else {
            self.hash_string(&context.request_id.to_string())
        };

        let index = hash % healthy_endpoints.len();
        Ok(healthy_endpoints[index].clone())
    }

    async fn select_random(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..healthy_endpoints.len());
        Ok(healthy_endpoints[index].clone())
    }

    async fn select_health_aware(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoints = self.endpoints.read().await;
        let healthy_endpoints: Vec<&ServiceEndpoint> = endpoints
            .values()
            .filter(|e| e.is_available())
            .collect();

        if healthy_endpoints.is_empty() {
            return Err(AppError::ServiceUnavailable("No healthy endpoints available".to_string()));
        }

        // Score endpoints based on health metrics
        let selected = healthy_endpoints
            .iter()
            .max_by(|a, b| {
                let a_score = self.calculate_health_score(a);
                let b_score = self.calculate_health_score(b);
                a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        Ok((*selected).clone())
    }

    fn calculate_health_score(&self, endpoint: &ServiceEndpoint) -> f64 {
        let mut score = 100.0;

        // Penalize high connection utilization
        score -= endpoint.connection_utilization() * 50.0;

        // Penalize slow response times
        let response_time_penalty = endpoint.response_time.as_millis() as f64 / 1000.0;
        score -= response_time_penalty;

        // Penalize recent failures
        if let Some(last_failure) = endpoint.last_failure {
            let time_since_failure = last_failure.elapsed().as_secs() as f64;
            if time_since_failure < 300.0 { // 5 minutes
                score -= (300.0 - time_since_failure) / 10.0;
            }
        }

        // Bonus for higher weight
        score += (endpoint.weight as f64 / 100.0) * 10.0;

        score.max(0.0)
    }

    fn hash_string(&self, s: &str) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish() as usize
    }

    async fn handle_session_affinity(&self, context: &RoutingContext) -> Option<ServiceEndpoint> {
        if !self.config.sticky_sessions {
            return None;
        }

        if let Some(session_id) = &context.session_id {
            let affinity = self.session_affinity.read().await;
            if let Some(endpoint_id) = affinity.get(session_id) {
                let endpoints = self.endpoints.read().await;
                if let Some(endpoint) = endpoints.get(endpoint_id) {
                    if endpoint.is_available() {
                        return Some(endpoint.clone());
                    }
                }
            }
        }

        None
    }

    async fn update_session_affinity(&self, context: &RoutingContext, endpoint: &ServiceEndpoint) {
        if !self.config.sticky_sessions {
            return;
        }

        if let Some(session_id) = &context.session_id {
            let mut affinity = self.session_affinity.write().await;
            affinity.insert(session_id.clone(), endpoint.id);
        }
    }

    async fn cleanup_expired_sessions(&self) {
        if !self.config.sticky_sessions {
            return;
        }

        // This is a simplified cleanup - in production, you'd track session timestamps
        let mut affinity = self.session_affinity.write().await;
        if affinity.len() > 10000 { // Arbitrary limit
            affinity.clear();
        }
    }
}

impl LoadBalancer for ProductionLoadBalancer {
    async fn select_endpoint(&self, context: &RoutingContext) -> Result<ServiceEndpoint, AppError> {
        // Check session affinity first
        if let Some(endpoint) = self.handle_session_affinity(context).await {
            return Ok(endpoint);
        }

        let endpoint = match self.config.algorithm {
            LoadBalancingAlgorithm::RoundRobin => self.select_round_robin().await?,
            LoadBalancingAlgorithm::WeightedRoundRobin => self.select_weighted_round_robin().await?,
            LoadBalancingAlgorithm::LeastConnections => self.select_least_connections().await?,
            LoadBalancingAlgorithm::WeightedLeastConnections => self.select_weighted_least_connections().await?,
            LoadBalancingAlgorithm::IpHash => self.select_ip_hash(context).await?,
            LoadBalancingAlgorithm::Random => self.select_random().await?,
            LoadBalancingAlgorithm::HealthAware => self.select_health_aware().await?,
        };

        // Update session affinity
        self.update_session_affinity(context, &endpoint).await;

        Ok(endpoint)
    }

    async fn add_endpoint(&self, endpoint: ServiceEndpoint) -> Result<(), AppError> {
        let mut endpoints = self.endpoints.write().await;
        endpoints.insert(endpoint.id, endpoint);
        
        let mut stats = self.stats.write().await;
        stats.total_endpoints = endpoints.len() as u32;
        
        Ok(())
    }

    async fn remove_endpoint(&self, endpoint_id: Uuid) -> Result<(), AppError> {
        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(&endpoint_id);
        
        let mut stats = self.stats.write().await;
        stats.total_endpoints = endpoints.len() as u32;
        
        Ok(())
    }

    async fn update_endpoint_health(&self, endpoint_id: Uuid, status: HealthStatus) -> Result<(), AppError> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(&endpoint_id) {
            endpoint.health_status = status;
            endpoint.last_health_check = Instant::now();
        }
        
        Ok(())
    }

    async fn record_request_start(&self, endpoint_id: Uuid) -> Result<(), AppError> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(&endpoint_id) {
            endpoint.current_connections += 1;
        }
        
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        stats.active_connections += 1;
        
        Ok(())
    }

    async fn record_request_end(&self, endpoint_id: Uuid, success: bool, response_time: Duration) -> Result<(), AppError> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(&endpoint_id) {
            endpoint.current_connections = endpoint.current_connections.saturating_sub(1);
            endpoint.response_time = response_time;
        }
        
        let mut stats = self.stats.write().await;
        stats.active_connections = stats.active_connections.saturating_sub(1);
        
        if success {
            stats.successful_requests += 1;
        } else {
            stats.failed_requests += 1;
        }
        
        // Update average response time (simple moving average)
        let total_completed = stats.successful_requests + stats.failed_requests;
        if total_completed > 0 {
            let current_avg = stats.average_response_time.as_millis() as u64;
            let new_avg = (current_avg * (total_completed - 1) + response_time.as_millis() as u64) / total_completed;
            stats.average_response_time = Duration::from_millis(new_avg);
        }
        
        // Update per-endpoint stats
        *stats.requests_per_endpoint.entry(endpoint_id).or_insert(0) += 1;
        
        Ok(())
    }

    async fn get_stats(&self) -> LoadBalancerStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update healthy endpoints count
        let endpoints = self.endpoints.read().await;
        stats.healthy_endpoints = endpoints.values().filter(|e| e.is_healthy()).count() as u32;
        stats.total_endpoints = endpoints.len() as u32;
        
        stats
    }

    async fn get_healthy_endpoints(&self) -> Vec<ServiceEndpoint> {
        let endpoints = self.endpoints.read().await;
        endpoints.values().filter(|e| e.is_healthy()).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_balancer_basic_operations() {
        let config = LoadBalancerConfig::default();
        let lb = ProductionLoadBalancer::new(config);

        // Add endpoints
        let endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
        
        lb.add_endpoint(endpoint1.clone()).await.unwrap();
        lb.add_endpoint(endpoint2.clone()).await.unwrap();

        let stats = lb.get_stats().await;
        assert_eq!(stats.total_endpoints, 2);
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        let mut config = LoadBalancerConfig::default();
        config.algorithm = LoadBalancingAlgorithm::RoundRobin;
        let lb = ProductionLoadBalancer::new(config);

        // Add healthy endpoints
        let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
        endpoint1.health_status = HealthStatus::Healthy;
        endpoint2.health_status = HealthStatus::Healthy;
        
        lb.add_endpoint(endpoint1.clone()).await.unwrap();
        lb.add_endpoint(endpoint2.clone()).await.unwrap();

        let context = RoutingContext::new("/test".to_string(), "GET".to_string());
        
        // Should alternate between endpoints
        let selected1 = lb.select_endpoint(&context).await.unwrap();
        let selected2 = lb.select_endpoint(&context).await.unwrap();
        
        assert_ne!(selected1.id, selected2.id);
    }

    #[tokio::test]
    async fn test_health_aware_selection() {
        let mut config = LoadBalancerConfig::default();
        config.algorithm = LoadBalancingAlgorithm::HealthAware;
        let lb = ProductionLoadBalancer::new(config);

        // Add endpoints with different health scores
        let mut endpoint1 = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let mut endpoint2 = ServiceEndpoint::new("127.0.0.1".to_string(), 8081);
        
        endpoint1.health_status = HealthStatus::Healthy;
        endpoint1.current_connections = 10;
        endpoint1.response_time = Duration::from_millis(100);
        
        endpoint2.health_status = HealthStatus::Healthy;
        endpoint2.current_connections = 5;
        endpoint2.response_time = Duration::from_millis(50);
        
        lb.add_endpoint(endpoint1).await.unwrap();
        lb.add_endpoint(endpoint2.clone()).await.unwrap();

        let context = RoutingContext::new("/test".to_string(), "GET".to_string());
        let selected = lb.select_endpoint(&context).await.unwrap();
        
        // Should select endpoint2 (better health score)
        assert_eq!(selected.id, endpoint2.id);
    }

    #[tokio::test]
    async fn test_request_tracking() {
        let config = LoadBalancerConfig::default();
        let lb = ProductionLoadBalancer::new(config);

        let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let endpoint_id = endpoint.id;
        
        lb.add_endpoint(endpoint).await.unwrap();

        // Record request lifecycle
        lb.record_request_start(endpoint_id).await.unwrap();
        lb.record_request_end(endpoint_id, true, Duration::from_millis(100)).await.unwrap();

        let stats = lb.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_no_healthy_endpoints() {
        let config = LoadBalancerConfig::default();
        let lb = ProductionLoadBalancer::new(config);

        // Add unhealthy endpoint
        let mut endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        endpoint.health_status = HealthStatus::Unhealthy;
        
        lb.add_endpoint(endpoint).await.unwrap();

        let context = RoutingContext::new("/test".to_string(), "GET".to_string());
        let result = lb.select_endpoint(&context).await;
        
        assert!(result.is_err());
    }
}