//! Retry policies and circuit breaker patterns

use crate::{Result, ValkyrieError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Initial delay between retries
    pub initial_delay_ms: u64,
    /// Maximum delay between retries
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Enable jitter to avoid thundering herd
    pub enable_jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            enable_jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with no retries
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            backoff_multiplier: 1.0,
            enable_jitter: false,
        }
    }
    
    /// Create a new retry policy with fixed delay
    pub fn fixed_delay(attempts: usize, delay_ms: u64) -> Self {
        Self {
            max_attempts: attempts,
            initial_delay_ms: delay_ms,
            max_delay_ms: delay_ms,
            backoff_multiplier: 1.0,
            enable_jitter: false,
        }
    }
    
    /// Create a new retry policy with exponential backoff
    pub fn exponential_backoff(attempts: usize, initial_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_attempts: attempts,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier: 2.0,
            enable_jitter: true,
        }
    }
    
    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.max_attempts {
            debug!("Executing operation, attempt {}/{}", attempt, self.max_attempts);
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < self.max_attempts {
                        let delay = self.calculate_delay(attempt);
                        debug!("Operation failed, retrying in {}ms", delay.as_millis());
                        tokio::time::sleep(delay).await;
                    } else {
                        warn!("Operation failed after {} attempts", self.max_attempts);
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| ValkyrieError::protocol("All retry attempts failed")))
    }
    
    /// Calculate delay for a given attempt
    fn calculate_delay(&self, attempt: usize) -> Duration {
        let base_delay = self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32 - 1);
        let delay_ms = base_delay.min(self.max_delay_ms as f64) as u64;
        
        if self.enable_jitter {
            // Add up to 25% jitter
            let jitter = (delay_ms as f64 * 0.25 * rand::random::<f64>()) as u64;
            Duration::from_millis(delay_ms + jitter)
        } else {
            Duration::from_millis(delay_ms)
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed, requests are allowed
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service is recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open the circuit
    pub failure_threshold: usize,
    /// Success threshold to close the circuit from half-open
    pub success_threshold: usize,
    /// Timeout before transitioning from open to half-open
    pub timeout_ms: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 60000, // 1 minute
        }
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitState,
    failure_count: usize,
    success_count: usize,
    last_failure_time: Option<std::time::Instant>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
        }
    }
    
    /// Execute a function with circuit breaker protection
    pub async fn execute<F, Fut, T>(&mut self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit should transition from open to half-open
        if self.state == CircuitState::Open {
            if let Some(last_failure) = self.last_failure_time {
                if last_failure.elapsed().as_millis() as u64 > self.config.timeout_ms {
                    debug!("Circuit breaker transitioning from Open to HalfOpen");
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                }
            }
        }
        
        // Reject requests if circuit is open
        if self.state == CircuitState::Open {
            return Err(ValkyrieError::service_unavailable("Circuit breaker is open"));
        }
        
        // Execute the operation
        match operation().await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(e)
            }
        }
    }
    
    /// Handle successful operation
    fn on_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    debug!("Circuit breaker transitioning from HalfOpen to Closed");
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen
            }
        }
    }
    
    /// Handle failed operation
    fn on_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(std::time::Instant::now());
        
        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.config.failure_threshold {
                    debug!("Circuit breaker transitioning from Closed to Open");
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                debug!("Circuit breaker transitioning from HalfOpen to Open");
                self.state = CircuitState::Open;
                self.success_count = 0;
            }
            CircuitState::Open => {
                // Already open
            }
        }
    }
    
    /// Get current circuit state
    pub fn state(&self) -> &CircuitState {
        &self.state
    }
    
    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.failure_count
    }
    
    /// Get success count (only relevant in half-open state)
    pub fn success_count(&self) -> usize {
        self.success_count
    }
}