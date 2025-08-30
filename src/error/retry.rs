
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};
use super::context::ErrorContext;

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries (in milliseconds)
    pub base_delay_ms: u64,
    /// Maximum delay between retries (in milliseconds)
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays
    pub use_jitter: bool,
    /// Maximum total time to spend on retries (in milliseconds)
    pub max_total_time_ms: Option<u64>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000, // 1 second
            max_delay_ms: 30000, // 30 seconds
            backoff_multiplier: 2.0,
            use_jitter: true,
            max_total_time_ms: Some(300000), // 5 minutes
        }
    }
}

/// Error recovery strategy enumeration
#[derive(Debug, Clone)]
pub enum ErrorRecoveryStrategy {
    /// Retry with exponential backoff
    Retry {
        policy: RetryPolicy,
    },
    /// Use a fallback operation
    Fallback {
        fallback_fn: String, // Function name for logging
    },
    /// Apply circuit breaker pattern
    CircuitBreaker {
        failure_threshold: u32,
        timeout_ms: u64,
        half_open_max_calls: u32,
    },
    /// Graceful degradation
    Graceful {
        degraded_mode: String,
    },
    /// No recovery - fail immediately
    None,
}

/// Retry manager for handling retry logic with configurable policies
#[derive(Debug)]
pub struct RetryManager {
    /// Default retry policy
    default_policy: RetryPolicy,
    /// Error-specific retry policies
    error_policies: HashMap<String, RetryPolicy>,
}

impl RetryManager {
    /// Create a new retry manager with default policy
    pub fn new() -> Self {
        Self {
            default_policy: RetryPolicy::default(),
            error_policies: HashMap::new(),
        }
    }

    /// Create a retry manager with custom default policy
    pub fn with_default_policy(policy: RetryPolicy) -> Self {
        Self {
            default_policy: policy,
            error_policies: HashMap::new(),
        }
    }

    /// Add a retry policy for a specific error type
    pub fn add_error_policy(&mut self, error_type: impl Into<String>, policy: RetryPolicy) {
        self.error_policies.insert(error_type.into(), policy);
    }

    /// Get the retry policy for a specific error type
    pub fn get_policy(&self, error_type: &str) -> &RetryPolicy {
        self.error_policies.get(error_type).unwrap_or(&self.default_policy)
    }

    /// Execute an operation with retry logic
    pub async fn execute_with_retry<T, F, Fut>(
        &self,
        mut operation: F,
        context: ErrorContext,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start_time = Instant::now();
        let mut attempt = 1;

        loop {
            debug!(
                correlation_id = %context.correlation_id,
                attempt = attempt,
                "Executing operation with retry"
            );

            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!(
                            correlation_id = %context.correlation_id,
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }
                    return Ok(result);
                }
                Err(error) => {
                    let error_type = self.get_error_type(&error);
                    let policy = self.get_policy(&error_type);

                    // Check if we should retry
                    if attempt >= policy.max_attempts {
                        warn!(
                            correlation_id = %context.correlation_id,
                            attempt = attempt,
                            max_attempts = policy.max_attempts,
                            "Maximum retry attempts reached"
                        );
                        return Err(error);
                    }

                    // Check total time limit
                    if let Some(max_total_time) = policy.max_total_time_ms {
                        if start_time.elapsed().as_millis() as u64 > max_total_time {
                            warn!(
                                correlation_id = %context.correlation_id,
                                elapsed_ms = start_time.elapsed().as_millis(),
                                max_total_time_ms = max_total_time,
                                "Maximum total retry time exceeded"
                            );
                            return Err(error);
                        }
                    }

                    // Check if error is retryable
                    if !self.is_retryable_error(&error) {
                        debug!(
                            correlation_id = %context.correlation_id,
                            error_type = error_type,
                            "Error is not retryable"
                        );
                        return Err(error);
                    }

                    // Calculate delay
                    let delay = self.calculate_delay(policy, attempt);
                    
                    warn!(
                        correlation_id = %context.correlation_id,
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "Operation failed, retrying after delay"
                    );

                    sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Calculate delay for retry attempt
    fn calculate_delay(&self, policy: &RetryPolicy, attempt: u32) -> Duration {
        let base_delay = Duration::from_millis(policy.base_delay_ms);
        let exponential_delay = base_delay.as_millis() as f64 
            * policy.backoff_multiplier.powi((attempt - 1) as i32);
        
        let mut delay_ms = exponential_delay as u64;
        
        // Apply maximum delay limit
        delay_ms = delay_ms.min(policy.max_delay_ms);
        
        // Add jitter if enabled
        if policy.use_jitter {
            let jitter = (rand::random::<f64>() * 0.1 + 0.95) * delay_ms as f64; // ±5% jitter
            delay_ms = jitter as u64;
        }
        
        Duration::from_millis(delay_ms)
    }

    /// Check if an error is retryable
    fn is_retryable_error(&self, error: &AppError) -> bool {
        matches!(error,
            AppError::DatabaseError(_) |
            AppError::ExternalServiceError(_) |
            AppError::TimeoutError(_) |
            AppError::Timeout(_) |
            AppError::ServiceUnavailable(_) |
            AppError::RateLimitExceeded { .. } |
            AppError::RateLimitExceededSimple(_) |
            AppError::IoError(_) |
            AppError::Transport(_) |
            AppError::RoutingError(_)
        )
    }

    /// Get error type string for an AppError
    fn get_error_type(&self, error: &AppError) -> String {
        match error {
            AppError::DatabaseError(_) => "DatabaseError".to_string(),
            AppError::ExternalServiceError(_) => "ExternalServiceError".to_string(),
            AppError::TimeoutError(_) | AppError::Timeout(_) => "TimeoutError".to_string(),
            AppError::ServiceUnavailable(_) => "ServiceUnavailable".to_string(),
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => "RateLimitExceeded".to_string(),
            AppError::IoError(_) => "IoError".to_string(),
            AppError::Transport(_) => "Transport".to_string(),
            AppError::RoutingError(_) => "RoutingError".to_string(),
            _ => "Other".to_string(),
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests are allowed through
    Closed,
    /// Circuit is open, requests are rejected immediately
    Open,
    /// Circuit is half-open, limited requests are allowed to test recovery
    HalfOpen,
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Total number of requests
    pub total_requests: u64,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
    /// Current failure rate (0.0 to 1.0)
    pub failure_rate: f64,
    /// Last state change timestamp
    pub last_state_change: Instant,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures required to open the circuit
    pub failure_threshold: u32,
    /// Minimum number of requests before calculating failure rate
    pub minimum_requests: u32,
    /// Failure rate threshold (0.0 to 1.0) to open the circuit
    pub failure_rate_threshold: f64,
    /// Time to wait before transitioning from Open to HalfOpen (in milliseconds)
    pub timeout_ms: u64,
    /// Maximum number of requests allowed in HalfOpen state
    pub half_open_max_calls: u32,
    /// Time window for calculating failure rate (in milliseconds)
    pub time_window_ms: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            minimum_requests: 10,
            failure_rate_threshold: 0.5, // 50%
            timeout_ms: 60000, // 1 minute
            half_open_max_calls: 3,
            time_window_ms: 60000, // 1 minute
        }
    }
}

/// Circuit breaker implementation to prevent cascade failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Circuit breaker name for identification
    name: String,
    /// Current state of the circuit breaker
    state: Arc<RwLock<CircuitBreakerState>>,
    /// Configuration
    config: CircuitBreakerConfig,
    /// Statistics
    stats: Arc<RwLock<CircuitBreakerStats>>,
    /// Recent request results (for sliding window)
    recent_results: Arc<RwLock<Vec<(Instant, bool)>>>,
    /// Number of requests in half-open state
    half_open_requests: Arc<RwLock<u32>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            config,
            stats: Arc::new(RwLock::new(CircuitBreakerStats {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                failure_rate: 0.0,
                last_state_change: Instant::now(),
            })),
            recent_results: Arc::new(RwLock::new(Vec::new())),
            half_open_requests: Arc::new(RwLock::new(0)),
        }
    }

    /// Execute an operation through the circuit breaker
    pub async fn execute<T, F, Fut>(&self, operation: F, context: ErrorContext) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if request should be allowed
        if !self.should_allow_request().await {
            return Err(AppError::ServiceUnavailable(format!(
                "Circuit breaker '{}' is open", self.name
            )));
        }

        let start_time = Instant::now();
        
        match operation().await {
            Ok(result) => {
                self.record_success().await;
                debug!(
                    correlation_id = %context.correlation_id,
                    circuit_breaker = %self.name,
                    duration_ms = start_time.elapsed().as_millis(),
                    "Circuit breaker operation succeeded"
                );
                Ok(result)
            }
            Err(error) => {
                self.record_failure().await;
                warn!(
                    correlation_id = %context.correlation_id,
                    circuit_breaker = %self.name,
                    duration_ms = start_time.elapsed().as_millis(),
                    error = %error,
                    "Circuit breaker operation failed"
                );
                Err(error)
            }
        }
    }

    /// Check if a request should be allowed through the circuit breaker
    async fn should_allow_request(&self) -> bool {
        let state = self.state.read().await;
        
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if timeout has elapsed
                let stats = self.stats.read().await;
                let timeout_duration = Duration::from_millis(self.config.timeout_ms);
                
                if stats.last_state_change.elapsed() >= timeout_duration {
                    drop(stats);
                    drop(state);
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => {
                let half_open_requests = self.half_open_requests.read().await;
                *half_open_requests < self.config.half_open_max_calls
            }
        }
    }

    /// Record a successful operation
    async fn record_success(&self) -> () {
        let mut stats = self.stats.write().await;
        let mut recent_results = self.recent_results.write().await;
        let now = Instant::now();
        
        stats.total_requests += 1;
        stats.successful_requests += 1;
        recent_results.push((now, true));
        
        // Clean old results
        self.clean_old_results(&mut recent_results, now).await;
        
        // Update failure rate
        self.update_failure_rate(&mut stats, &recent_results).await;
        
        let current_state = *self.state.read().await;
        
        match current_state {
            CircuitBreakerState::HalfOpen => {
                let mut half_open_requests = self.half_open_requests.write().await;
                *half_open_requests += 1;
                
                // If we've had enough successful requests in half-open, close the circuit
                if *half_open_requests >= self.config.half_open_max_calls {
                    drop(half_open_requests);
                    drop(stats);
                    drop(recent_results);
                    self.transition_to_closed().await;
                    return;
                }
            }
            _ => {}
        }
    }

    /// Record a failed operation
    async fn record_failure(&self) -> () {
        let mut stats = self.stats.write().await;
        let mut recent_results = self.recent_results.write().await;
        let now = Instant::now();
        
        stats.total_requests += 1;
        stats.failed_requests += 1;
        recent_results.push((now, false));
        
        // Clean old results
        self.clean_old_results(&mut recent_results, now).await;
        
        // Update failure rate
        self.update_failure_rate(&mut stats, &recent_results).await;
        
        let current_state = *self.state.read().await;
        
        // Check if we should open the circuit
        if current_state != CircuitBreakerState::Open {
            if self.should_open_circuit(&stats, &recent_results).await {
                drop(stats);
                drop(recent_results);
                self.transition_to_open().await;
                return;
            }
        }
        
        // If in half-open state and we get a failure, go back to open
        if current_state == CircuitBreakerState::HalfOpen {
            drop(stats);
            drop(recent_results);
            self.transition_to_open().await;
        }
    }

    /// Check if the circuit should be opened
    async fn should_open_circuit(&self, stats: &CircuitBreakerStats, recent_results: &[(Instant, bool)]) -> bool {
        // Check minimum requests threshold
        if recent_results.len() < self.config.minimum_requests as usize {
            return false;
        }
        
        // Check failure rate threshold
        stats.failure_rate >= self.config.failure_rate_threshold
    }

    /// Update the failure rate based on recent results
    async fn update_failure_rate(&self, stats: &mut CircuitBreakerStats, recent_results: &[(Instant, bool)]) -> () {
        if recent_results.is_empty() {
            stats.failure_rate = 0.0;
            return;
        }
        
        let failures = recent_results.iter().filter(|(_, success)| !success).count();
        stats.failure_rate = failures as f64 / recent_results.len() as f64;
    }

    /// Clean old results outside the time window
    async fn clean_old_results(&self, recent_results: &mut Vec<(Instant, bool)>, now: Instant) -> () {
        let time_window = Duration::from_millis(self.config.time_window_ms);
        recent_results.retain(|(timestamp, _)| now.duration_since(*timestamp) <= time_window);
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) -> () {
        let mut state = self.state.write().await;
        let mut stats = self.stats.write().await;
        let mut half_open_requests = self.half_open_requests.write().await;
        
        *state = CircuitBreakerState::Closed;
        stats.last_state_change = Instant::now();
        *half_open_requests = 0;
        
        info!(
            circuit_breaker = %self.name,
            "Circuit breaker transitioned to CLOSED"
        );
    }

    /// Transition to open state
    async fn transition_to_open(&self) -> () {
        let mut state = self.state.write().await;
        let mut stats = self.stats.write().await;
        let mut half_open_requests = self.half_open_requests.write().await;
        
        *state = CircuitBreakerState::Open;
        stats.last_state_change = Instant::now();
        *half_open_requests = 0;
        
        warn!(
            circuit_breaker = %self.name,
            failure_rate = stats.failure_rate,
            "Circuit breaker transitioned to OPEN"
        );
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) -> () {
        let mut state = self.state.write().await;
        let mut stats = self.stats.write().await;
        let mut half_open_requests = self.half_open_requests.write().await;
        
        *state = CircuitBreakerState::HalfOpen;
        stats.last_state_change = Instant::now();
        *half_open_requests = 0;
        
        info!(
            circuit_breaker = %self.name,
            "Circuit breaker transitioned to HALF_OPEN"
        );
    }

    /// Get current circuit breaker state
    pub async fn get_state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    /// Get current circuit breaker statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        self.stats.read().await.clone()
    }

    /// Get circuit breaker name
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

/// Circuit breaker manager for managing multiple circuit breakers
#[derive(Debug)]
pub struct CircuitBreakerManager {
    /// Map of circuit breakers by name
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    /// Default configuration for new circuit breakers
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new() -> Self {
        Self {
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config: CircuitBreakerConfig::default(),
        }
    }

    /// Create a circuit breaker manager with custom default configuration
    pub fn with_default_config(config: CircuitBreakerConfig) -> Self {
        Self {
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
        }
    }

    /// Get or create a circuit breaker
    pub async fn get_circuit_breaker(&self, name: impl Into<String>) -> Arc<CircuitBreaker> {
        let name = name.into();
        let circuit_breakers = self.circuit_breakers.read().await;
        
        if let Some(cb) = circuit_breakers.get(&name) {
            cb.clone()
        } else {
            drop(circuit_breakers);
            
            let mut circuit_breakers = self.circuit_breakers.write().await;
            // Double-check in case another thread created it
            if let Some(cb) = circuit_breakers.get(&name) {
                cb.clone()
            } else {
                let cb = Arc::new(CircuitBreaker::with_config(name.clone(), self.default_config.clone()));
                circuit_breakers.insert(name, cb.clone());
                cb
            }
        }
    }

    /// Execute an operation through a named circuit breaker
    pub async fn execute<T, F, Fut>(
        &self,
        circuit_breaker_name: impl Into<String>,
        operation: F,
        context: ErrorContext,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let cb = self.get_circuit_breaker(circuit_breaker_name).await;
        cb.execute(operation, context).await
    }

    /// Get all circuit breaker names
    pub async fn get_circuit_breaker_names(&self) -> Vec<String> {
        self.circuit_breakers.read().await.keys().cloned().collect()
    }

    /// Get statistics for all circuit breakers
    pub async fn get_all_stats(&self) -> HashMap<String, CircuitBreakerStats> {
        let circuit_breakers = self.circuit_breakers.read().await;
        let mut stats = HashMap::new();
        
        for (name, cb) in circuit_breakers.iter() {
            stats.insert(name.clone(), cb.get_stats().await);
        }
        
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_retry_manager_success() {
        let retry_manager = RetryManager::new();
        let context = ErrorContext::new("test", "operation");
        
        let result = retry_manager.execute_with_retry(
            || async { Ok::<i32, AppError>(42) },
            context,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_manager_eventual_success() {
        let retry_manager = RetryManager::new();
        let context = ErrorContext::new("test", "operation");
        let mut attempt_count = 0;
        
        let result = retry_manager.execute_with_retry(
            || {
                attempt_count += 1;
                async move {
                    if attempt_count < 3 {
                        Err(AppError::DatabaseError("Connection failed".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            },
            context,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count, 3);
    }

    #[tokio::test]
    async fn test_retry_manager_max_attempts() {
        let mut retry_manager = RetryManager::new();
        retry_manager.add_error_policy("DatabaseError", RetryPolicy {
            max_attempts: 2,
            base_delay_ms: 10, // Short delay for testing
            ..RetryPolicy::default()
        });
        
        let context = ErrorContext::new("test", "operation");
        let mut attempt_count = 0;
        
        let result = retry_manager.execute_with_retry(
            || {
                attempt_count += 1;
                async move {
                    Err::<i32, AppError>(AppError::DatabaseError("Always fails".to_string()))
                }
            },
            context,
        ).await;
        
        assert!(result.is_err());
        assert_eq!(attempt_count, 2);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::new("test");
        let context = ErrorContext::new("test", "operation");
        
        let result = cb.execute(
            || async { Ok::<i32, AppError>(42) },
            context,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            minimum_requests: 2,
            failure_rate_threshold: 0.5,
            timeout_ms: 1000,
            half_open_max_calls: 1,
            time_window_ms: 10000,
        };
        
        let cb = CircuitBreaker::with_config("test", config);
        let context = ErrorContext::new("test", "operation");
        
        // First failure
        let result1 = cb.execute(
            || async { Err::<i32, AppError>(AppError::DatabaseError("Failure 1".to_string())) },
            context.clone(),
        ).await;
        assert!(result1.is_err());
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
        
        // Second failure - should open the circuit
        let result2 = cb.execute(
            || async { Err::<i32, AppError>(AppError::DatabaseError("Failure 2".to_string())) },
            context.clone(),
        ).await;
        assert!(result2.is_err());
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        
        // Third request should be rejected immediately
        let result3 = cb.execute(
            || async { Ok::<i32, AppError>(42) },
            context,
        ).await;
        assert!(result3.is_err());
        assert!(result3.unwrap_err().to_string().contains("Circuit breaker"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_manager() {
        let manager = CircuitBreakerManager::new();
        let context = ErrorContext::new("test", "operation");
        
        let result = manager.execute(
            "test-service",
            || async { Ok::<i32, AppError>(42) },
            context,
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        let names = manager.get_circuit_breaker_names().await;
        assert!(names.contains(&"test-service".to_string()));
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.base_delay_ms, 1000);
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert!(policy.use_jitter);
    }

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.failure_rate_threshold, 0.5);
        assert_eq!(config.timeout_ms, 60000);
        assert_eq!(config.half_open_max_calls, 3);
    }
}