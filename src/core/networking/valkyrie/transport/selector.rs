use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::core::networking::node_communication::ProtocolError;
use crate::core::networking::transport::{
    NetworkConditions, Transport, TransportEndpoint, TransportType,
};
use crate::core::networking::valkyrie::transport::{
    BackoffStrategy, FailoverConfig, TransportSelectionStrategy,
};
use crate::error::Result;

/// Transport selector with failover capabilities
pub struct TransportSelector {
    /// Available transports
    transports: HashMap<TransportType, Arc<dyn Transport>>,
    /// Selection strategy
    strategy: TransportSelectionStrategy,
    /// Failover configuration
    failover_config: FailoverConfig,
    /// Transport health status
    health_status: Arc<RwLock<HashMap<TransportType, TransportHealth>>>,
    /// Selection state for round-robin and weighted strategies
    selection_state: Arc<RwLock<SelectionState>>,
}

/// Transport health information
#[derive(Debug, Clone)]
pub struct TransportHealth {
    /// Is transport healthy
    pub is_healthy: bool,
    /// Last health check time
    pub last_check: Instant,
    /// Consecutive failure count
    pub failure_count: u32,
    /// Consecutive success count
    pub success_count: u32,
    /// Average response time
    pub avg_response_time: Duration,
    /// Last error message
    pub last_error: Option<String>,
}

/// Selection state for stateful strategies
#[derive(Debug, Default)]
pub struct SelectionState {
    /// Round-robin counter
    pub round_robin_index: usize,
    /// Weighted selection state
    pub weighted_state: HashMap<TransportType, f64>,
    /// Last selection time
    pub last_selection: Option<Instant>,
}

impl TransportSelector {
    /// Create a new transport selector
    pub fn new(strategy: TransportSelectionStrategy, failover_config: FailoverConfig) -> Self {
        Self {
            transports: HashMap::new(),
            strategy,
            failover_config,
            health_status: Arc::new(RwLock::new(HashMap::new())),
            selection_state: Arc::new(RwLock::new(SelectionState::default())),
        }
    }

    /// Register a transport
    pub fn register_transport(&mut self, transport: Arc<dyn Transport>) {
        let transport_type = transport.transport_type();
        self.transports.insert(transport_type.clone(), transport);

        // Initialize health status
        let health = TransportHealth {
            is_healthy: true,
            last_check: Instant::now(),
            failure_count: 0,
            success_count: 0,
            avg_response_time: Duration::from_millis(0),
            last_error: None,
        };

        tokio::spawn({
            let health_status = self.health_status.clone();
            async move {
                let mut health_map = health_status.write().await;
                health_map.insert(transport_type, health);
            }
        });
    }

    /// Select optimal transport for endpoint
    pub async fn select_transport(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // Get healthy transports that support the endpoint
        let candidates = self.get_healthy_candidates(endpoint).await;

        if candidates.is_empty() {
            return Err(ProtocolError::ConnectionError {
                message: "No healthy transports available".to_string(),
            }
            .into());
        }

        // Apply selection strategy
        let selected = match &self.strategy {
            TransportSelectionStrategy::FirstAvailable => self.select_first_available(&candidates),
            TransportSelectionStrategy::LatencyOptimized => {
                self.select_latency_optimized(&candidates, conditions).await
            }
            TransportSelectionStrategy::ThroughputOptimized => {
                self.select_throughput_optimized(&candidates, conditions)
                    .await
            }
            TransportSelectionStrategy::ReliabilityOptimized => {
                self.select_reliability_optimized(&candidates, conditions)
                    .await
            }
            TransportSelectionStrategy::RoundRobin => self.select_round_robin(&candidates).await,
            TransportSelectionStrategy::Weighted(weights) => {
                self.select_weighted(&candidates, weights).await
            }
            TransportSelectionStrategy::Custom(_) => {
                // Custom selection logic would be implemented here
                self.select_first_available(&candidates)
            }
        };

        selected
    }

    /// Attempt connection with failover
    pub async fn connect_with_failover(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<(
        Arc<dyn Transport>,
        Box<dyn crate::core::networking::transport::Connection>,
    )> {
        if !self.failover_config.enabled {
            let transport = self.select_transport(endpoint, conditions).await?;
            let connection = transport.connect(endpoint).await?;
            return Ok((transport, connection));
        }

        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.failover_config.max_attempts {
            match self.select_transport(endpoint, conditions).await {
                Ok(transport) => {
                    match transport.connect(endpoint).await {
                        Ok(connection) => {
                            // Mark transport as healthy
                            self.mark_transport_healthy(transport.transport_type())
                                .await;
                            return Ok((transport, connection));
                        }
                        Err(e) => {
                            // Mark transport as unhealthy
                            self.mark_transport_unhealthy(
                                transport.transport_type(),
                                &e.to_string(),
                            )
                            .await;
                            last_error = Some(e);
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }

            attempts += 1;

            if attempts < self.failover_config.max_attempts {
                // Apply backoff strategy
                let delay = self.calculate_backoff_delay(attempts);
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProtocolError::ConnectionError {
                message: "All failover attempts exhausted".to_string(),
            }
            .into()
        }))
    }

    /// Get healthy transport candidates
    async fn get_healthy_candidates(
        &self,
        endpoint: &TransportEndpoint,
    ) -> Vec<(TransportType, Arc<dyn Transport>)> {
        let health_status = self.health_status.read().await;
        let mut candidates = Vec::new();

        for (transport_type, transport) in &self.transports {
            // Check if transport supports the endpoint
            if !transport.supports_endpoint(endpoint) {
                continue;
            }

            // Check health status
            if let Some(health) = health_status.get(transport_type) {
                if health.is_healthy {
                    candidates.push((transport_type.clone(), transport.clone()));
                }
            } else {
                // No health info, assume healthy
                candidates.push((transport_type.clone(), transport.clone()));
            }
        }

        candidates
    }

    /// Select first available transport
    fn select_first_available(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
    ) -> Result<Arc<dyn Transport>> {
        candidates
            .first()
            .map(|(_, transport)| transport.clone())
            .ok_or_else(|| {
                ProtocolError::ConnectionError {
                    message: "No candidates available".to_string(),
                }
                .into()
            })
    }

    /// Select transport optimized for latency
    async fn select_latency_optimized(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // Prefer transports with lower latency characteristics
        let preferred_order = if conditions.latency_ms < 10.0 {
            vec![
                TransportType::Quic,
                TransportType::Tcp,
                TransportType::WebSocket,
            ]
        } else {
            vec![
                TransportType::Tcp,
                TransportType::Quic,
                TransportType::WebSocket,
            ]
        };

        for preferred_type in preferred_order {
            if let Some((_, transport)) = candidates.iter().find(|(t, _)| *t == preferred_type) {
                return Ok(transport.clone());
            }
        }

        self.select_first_available(candidates)
    }

    /// Select transport optimized for throughput
    async fn select_throughput_optimized(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // Prefer transports with multiplexing for high throughput
        let preferred_order = if conditions.bandwidth_mbps > 100.0 {
            vec![
                TransportType::Quic,
                TransportType::SecureTcp,
                TransportType::Tcp,
            ]
        } else {
            vec![
                TransportType::Tcp,
                TransportType::Quic,
                TransportType::WebSocket,
            ]
        };

        for preferred_type in preferred_order {
            if let Some((_, transport)) = candidates.iter().find(|(t, _)| *t == preferred_type) {
                return Ok(transport.clone());
            }
        }

        self.select_first_available(candidates)
    }

    /// Select transport optimized for reliability
    async fn select_reliability_optimized(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
        _conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        let health_status = self.health_status.read().await;

        // Sort by reliability metrics
        let mut scored_candidates: Vec<_> = candidates
            .iter()
            .map(|(transport_type, transport)| {
                let reliability_score = if let Some(health) = health_status.get(transport_type) {
                    // Calculate reliability score based on success rate and response time
                    let success_rate = if health.success_count + health.failure_count > 0 {
                        health.success_count as f64
                            / (health.success_count + health.failure_count) as f64
                    } else {
                        1.0
                    };

                    let response_time_score =
                        1.0 / (1.0 + health.avg_response_time.as_millis() as f64 / 1000.0);

                    success_rate * 0.7 + response_time_score * 0.3
                } else {
                    0.5 // Default score for unknown health
                };

                (transport_type, transport, reliability_score)
            })
            .collect();

        scored_candidates
            .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        scored_candidates
            .first()
            .map(|(_, transport, _)| transport.clone().clone())
            .ok_or_else(|| {
                ProtocolError::ConnectionError {
                    message: "No reliable candidates available".to_string(),
                }
                .into()
            })
    }

    /// Select transport using round-robin
    async fn select_round_robin(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
    ) -> Result<Arc<dyn Transport>> {
        let mut state = self.selection_state.write().await;
        let index = state.round_robin_index % candidates.len();
        state.round_robin_index = (state.round_robin_index + 1) % candidates.len();

        Ok(candidates[index].1.clone())
    }

    /// Select transport using weighted selection
    async fn select_weighted(
        &self,
        candidates: &[(TransportType, Arc<dyn Transport>)],
        weights: &HashMap<TransportType, f64>,
    ) -> Result<Arc<dyn Transport>> {
        // Calculate weighted selection
        let total_weight: f64 = candidates
            .iter()
            .map(|(transport_type, _)| weights.get(transport_type).unwrap_or(&1.0))
            .sum();

        if total_weight <= 0.0 {
            return self.select_first_available(candidates);
        }

        let random_value = fastrand::f64() * total_weight;
        let mut cumulative_weight = 0.0;

        for (transport_type, transport) in candidates {
            cumulative_weight += weights.get(transport_type).unwrap_or(&1.0);
            if random_value <= cumulative_weight {
                return Ok(transport.clone());
            }
        }

        // Fallback to first available
        self.select_first_available(candidates)
    }

    /// Mark transport as healthy
    async fn mark_transport_healthy(&self, transport_type: TransportType) {
        let mut health_status = self.health_status.write().await;
        if let Some(health) = health_status.get_mut(&transport_type) {
            health.success_count += 1;
            health.failure_count = 0;
            health.is_healthy = true;
            health.last_check = Instant::now();
            health.last_error = None;
        }
    }

    /// Mark transport as unhealthy
    async fn mark_transport_unhealthy(&self, transport_type: TransportType, error: &str) {
        let mut health_status = self.health_status.write().await;
        if let Some(health) = health_status.get_mut(&transport_type) {
            health.failure_count += 1;
            health.success_count = 0;
            health.last_check = Instant::now();
            health.last_error = Some(error.to_string());

            // Mark as unhealthy if failure threshold is reached
            if health.failure_count >= self.failover_config.health_check.failure_threshold {
                health.is_healthy = false;
            }
        }
    }

    /// Calculate backoff delay for failover attempts
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        match &self.failover_config.backoff_strategy {
            BackoffStrategy::Fixed(delay) => *delay,
            BackoffStrategy::Exponential {
                initial_delay,
                max_delay,
                multiplier,
            } => {
                let delay = Duration::from_millis(
                    (initial_delay.as_millis() as f64 * multiplier.powi(attempt as i32 - 1)) as u64,
                );
                std::cmp::min(delay, *max_delay)
            }
            BackoffStrategy::Linear {
                initial_delay,
                increment,
            } => Duration::from_millis(
                initial_delay.as_millis() as u64
                    + (increment.as_millis() as u64 * (attempt - 1) as u64),
            ),
        }
    }

    /// Perform health checks on all transports
    pub async fn perform_health_checks(&self) {
        if !self.failover_config.health_check.enabled {
            return;
        }

        let health_check_tasks: Vec<_> = self
            .transports
            .iter()
            .map(|(transport_type, transport)| {
                let transport_type = transport_type.clone();
                let transport = transport.clone();
                let health_status = self.health_status.clone();
                let health_config = self.failover_config.health_check.clone();

                tokio::spawn(async move {
                    let start_time = Instant::now();

                    // Perform health check (simplified - in reality, this would ping the transport)
                    let health_result = transport.get_metrics().await;
                    let response_time = start_time.elapsed();

                    // Update health status
                    let mut health_map = health_status.write().await;
                    if let Some(health) = health_map.get_mut(&transport_type) {
                        health.last_check = Instant::now();
                        health.avg_response_time = (health.avg_response_time + response_time) / 2;

                        // Simple health check based on metrics availability
                        if health_result.connection_errors > 0 {
                            health.failure_count += 1;
                            if health.failure_count >= health_config.failure_threshold {
                                health.is_healthy = false;
                            }
                        } else {
                            health.success_count += 1;
                            if health.success_count >= health_config.success_threshold {
                                health.is_healthy = true;
                                health.failure_count = 0;
                            }
                        }
                    }
                })
            })
            .collect();

        // Wait for all health checks to complete
        for task in health_check_tasks {
            let _ = task.await;
        }
    }

    /// Get current health status of all transports
    pub async fn get_health_status(&self) -> HashMap<TransportType, TransportHealth> {
        self.health_status.read().await.clone()
    }
}

impl Default for TransportHealth {
    fn default() -> Self {
        Self {
            is_healthy: true,
            last_check: Instant::now(),
            failure_count: 0,
            success_count: 0,
            avg_response_time: Duration::from_millis(0),
            last_error: None,
        }
    }
}
