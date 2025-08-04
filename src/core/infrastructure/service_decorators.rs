use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Base service trait that all services must implement
#[async_trait]
pub trait Service: Send + Sync {
    fn service_name(&self) -> &'static str;
}

/// Service context for passing request-specific information
#[derive(Debug, Clone)]
pub struct ServiceContext {
    pub correlation_id: Uuid,
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub operation: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub started_at: Instant,
}

impl ServiceContext {
    pub fn new(operation: String) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            request_id: None,
            user_id: None,
            operation,
            metadata: std::collections::HashMap::new(),
            started_at: Instant::now(),
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn add_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn duration(&self) -> Duration {
        self.started_at.elapsed()
    }
}

/// Service result wrapper
#[derive(Debug)]
pub struct ServiceResult<T> {
    pub data: T,
    pub duration: Duration,
    pub metadata: std::collections::HashMap<String, String>,
}

impl<T> ServiceResult<T> {
    pub fn new(data: T, duration: Duration) -> Self {
        Self {
            data,
            duration,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Service middleware trait for decorating services
#[async_trait]
pub trait ServiceMiddleware: Send + Sync {
    async fn before_call(&self, context: &ServiceContext) -> Result<()>;
    async fn after_call(&self, context: &ServiceContext, result: &ServiceResult<()>) -> Result<()>;
    async fn on_error(&self, context: &ServiceContext, error: &AppError) -> Result<()>;
    fn middleware_name(&self) -> &'static str;
}

/// Logging middleware
pub struct LoggingMiddleware {
    log_requests: bool,
    log_responses: bool,
    log_errors: bool,
}

impl LoggingMiddleware {
    pub fn new() -> Self {
        Self {
            log_requests: true,
            log_responses: true,
            log_errors: true,
        }
    }

    pub fn with_requests(mut self, enabled: bool) -> Self {
        self.log_requests = enabled;
        self
    }

    pub fn with_responses(mut self, enabled: bool) -> Self {
        self.log_responses = enabled;
        self
    }

    pub fn with_errors(mut self, enabled: bool) -> Self {
        self.log_errors = enabled;
        self
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceMiddleware for LoggingMiddleware {
    #[instrument(skip(self, context))]
    async fn before_call(&self, context: &ServiceContext) -> Result<()> {
        if self.log_requests {
            info!(
                correlation_id = %context.correlation_id,
                request_id = ?context.request_id,
                user_id = ?context.user_id,
                operation = %context.operation,
                "Service call started"
            );
        }
        Ok(())
    }

    #[instrument(skip(self, context, result))]
    async fn after_call(&self, context: &ServiceContext, result: &ServiceResult<()>) -> Result<()> {
        if self.log_responses {
            info!(
                correlation_id = %context.correlation_id,
                request_id = ?context.request_id,
                user_id = ?context.user_id,
                operation = %context.operation,
                duration_ms = result.duration.as_millis(),
                "Service call completed"
            );
        }
        Ok(())
    }

    #[instrument(skip(self, context, error))]
    async fn on_error(&self, context: &ServiceContext, error: &AppError) -> Result<()> {
        if self.log_errors {
            error!(
                correlation_id = %context.correlation_id,
                request_id = ?context.request_id,
                user_id = ?context.user_id,
                operation = %context.operation,
                duration_ms = context.duration().as_millis(),
                error = %error,
                "Service call failed"
            );
        }
        Ok(())
    }

    fn middleware_name(&self) -> &'static str {
        "LoggingMiddleware"
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Retry middleware with exponential backoff
pub struct RetryMiddleware {
    config: RetryConfig,
}

impl RetryMiddleware {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.config.max_attempts = max_attempts;
        self
    }

    pub fn with_base_delay(mut self, base_delay: Duration) -> Self {
        self.config.base_delay = base_delay;
        self
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.config.base_delay.as_millis() as f64
            * self.config.backoff_multiplier.powi(attempt as i32);

        let delay = delay.min(self.config.max_delay.as_millis() as f64);

        let delay = if self.config.jitter {
            let jitter = fastrand::f64() * 0.1; // 10% jitter
            delay * (1.0 + jitter)
        } else {
            delay
        };

        Duration::from_millis(delay as u64)
    }

    pub async fn execute_with_retry<F, T, Fut>(
        &self,
        context: &ServiceContext,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..self.config.max_attempts {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            correlation_id = %context.correlation_id,
                            operation = %context.operation,
                            attempt = attempt + 1,
                            "Operation succeeded after retry"
                        );
                    }
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error);

                    if attempt < self.config.max_attempts - 1 {
                        let delay = self.calculate_delay(attempt);
                        warn!(
                            correlation_id = %context.correlation_id,
                            operation = %context.operation,
                            attempt = attempt + 1,
                            delay_ms = delay.as_millis(),
                            error = %last_error.as_ref().unwrap(),
                            "Operation failed, retrying"
                        );
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::InternalServerError("Retry operation failed without error".to_string())
        }))
    }
}

#[async_trait]
impl ServiceMiddleware for RetryMiddleware {
    async fn before_call(&self, _context: &ServiceContext) -> Result<()> {
        Ok(())
    }

    async fn after_call(
        &self,
        _context: &ServiceContext,
        _result: &ServiceResult<()>,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_error(&self, context: &ServiceContext, error: &AppError) -> Result<()> {
        debug!(
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            error = %error,
            "Retry middleware handling error"
        );
        Ok(())
    }

    fn middleware_name(&self) -> &'static str {
        "RetryMiddleware"
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub reset_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(10),
            reset_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker middleware
pub struct CircuitBreakerMiddleware {
    config: CircuitBreakerConfig,
    state: Arc<tokio::sync::RwLock<CircuitBreakerState>>,
    failure_count: Arc<tokio::sync::RwLock<u32>>,
    success_count: Arc<tokio::sync::RwLock<u32>>,
    last_failure_time: Arc<tokio::sync::RwLock<Option<Instant>>>,
}

impl CircuitBreakerMiddleware {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(tokio::sync::RwLock::new(CircuitBreakerState::Closed)),
            failure_count: Arc::new(tokio::sync::RwLock::new(0)),
            success_count: Arc::new(tokio::sync::RwLock::new(0)),
            last_failure_time: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    pub async fn execute_with_circuit_breaker<F, T, Fut>(
        &self,
        context: &ServiceContext,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit breaker should transition from Open to HalfOpen
        self.check_reset_timeout().await;

        let current_state = *self.state.read().await;

        match current_state {
            CircuitBreakerState::Open => {
                warn!(
                    correlation_id = %context.correlation_id,
                    operation = %context.operation,
                    "Circuit breaker is OPEN, rejecting request"
                );
                Err(AppError::ExternalServiceError(
                    "Circuit breaker is open".to_string(),
                ))
            }
            CircuitBreakerState::Closed | CircuitBreakerState::HalfOpen => {
                match timeout(self.config.timeout, operation()).await {
                    Ok(Ok(result)) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Ok(Err(error)) => {
                        self.on_failure().await;
                        Err(error)
                    }
                    Err(_) => {
                        let timeout_error = AppError::ExternalServiceError(format!(
                            "Operation timed out after {:?}",
                            self.config.timeout
                        ));
                        self.on_failure().await;
                        Err(timeout_error)
                    }
                }
            }
        }
    }

    async fn check_reset_timeout(&self) {
        let current_state = *self.state.read().await;
        if current_state == CircuitBreakerState::Open {
            let last_failure = *self.last_failure_time.read().await;
            if let Some(last_failure) = last_failure {
                if last_failure.elapsed() >= self.config.reset_timeout {
                    info!("Circuit breaker transitioning from OPEN to HALF_OPEN");
                    *self.state.write().await = CircuitBreakerState::HalfOpen;
                    *self.success_count.write().await = 0;
                }
            }
        }
    }

    async fn on_success(&self) {
        let current_state = *self.state.read().await;

        match current_state {
            CircuitBreakerState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                if *success_count >= self.config.success_threshold {
                    info!("Circuit breaker transitioning from HALF_OPEN to CLOSED");
                    *self.state.write().await = CircuitBreakerState::Closed;
                    *self.failure_count.write().await = 0;
                    *success_count = 0;
                }
            }
            CircuitBreakerState::Closed => {
                *self.failure_count.write().await = 0;
            }
            CircuitBreakerState::Open => {
                // Should not happen, but reset failure count just in case
                *self.failure_count.write().await = 0;
            }
        }
    }

    async fn on_failure(&self) {
        let current_state = *self.state.read().await;

        match current_state {
            CircuitBreakerState::Closed | CircuitBreakerState::HalfOpen => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                if *failure_count >= self.config.failure_threshold {
                    warn!("Circuit breaker transitioning to OPEN due to failures");
                    *self.state.write().await = CircuitBreakerState::Open;
                    *self.last_failure_time.write().await = Some(Instant::now());
                }
            }
            CircuitBreakerState::Open => {
                *self.last_failure_time.write().await = Some(Instant::now());
            }
        }
    }

    pub async fn get_state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    pub async fn get_failure_count(&self) -> u32 {
        *self.failure_count.read().await
    }
}

#[async_trait]
impl ServiceMiddleware for CircuitBreakerMiddleware {
    async fn before_call(&self, context: &ServiceContext) -> Result<()> {
        let state = self.get_state().await;
        debug!(
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            circuit_breaker_state = ?state,
            "Circuit breaker check"
        );
        Ok(())
    }

    async fn after_call(
        &self,
        _context: &ServiceContext,
        _result: &ServiceResult<()>,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_error(&self, context: &ServiceContext, error: &AppError) -> Result<()> {
        debug!(
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            error = %error,
            "Circuit breaker handling error"
        );
        Ok(())
    }

    fn middleware_name(&self) -> &'static str {
        "CircuitBreakerMiddleware"
    }
}

/// Metrics middleware for collecting service metrics
pub struct MetricsMiddleware {
    service_name: String,
}

impl MetricsMiddleware {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }
}

#[async_trait]
impl ServiceMiddleware for MetricsMiddleware {
    async fn before_call(&self, context: &ServiceContext) -> Result<()> {
        debug!(
            service = %self.service_name,
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            "Recording service call start metric"
        );
        // Here you would integrate with your metrics system (Prometheus, etc.)
        Ok(())
    }

    async fn after_call(&self, context: &ServiceContext, result: &ServiceResult<()>) -> Result<()> {
        info!(
            service = %self.service_name,
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            duration_ms = result.duration.as_millis(),
            "Recording service call success metric"
        );
        // Here you would record success metrics
        Ok(())
    }

    async fn on_error(&self, context: &ServiceContext, error: &AppError) -> Result<()> {
        warn!(
            service = %self.service_name,
            correlation_id = %context.correlation_id,
            operation = %context.operation,
            duration_ms = context.duration().as_millis(),
            error = %error,
            "Recording service call error metric"
        );
        // Here you would record error metrics
        Ok(())
    }

    fn middleware_name(&self) -> &'static str {
        "MetricsMiddleware"
    }
}

/// Service decorator that applies middleware to a service
pub struct ServiceDecorator<T: Service> {
    inner: Arc<T>,
    middlewares: Vec<Arc<dyn ServiceMiddleware>>,
}

impl<T: Service> ServiceDecorator<T> {
    pub fn new(service: Arc<T>) -> Self {
        Self {
            inner: service,
            middlewares: Vec::new(),
        }
    }

    pub fn with_middleware(mut self, middleware: Arc<dyn ServiceMiddleware>) -> Self {
        self.middlewares.push(middleware);
        self
    }

    pub fn with_logging(self) -> Self {
        self.with_middleware(Arc::new(LoggingMiddleware::default()))
    }

    pub fn with_retry(self, config: RetryConfig) -> Self {
        self.with_middleware(Arc::new(RetryMiddleware::new(config)))
    }

    pub fn with_circuit_breaker(self, config: CircuitBreakerConfig) -> Self {
        self.with_middleware(Arc::new(CircuitBreakerMiddleware::new(config)))
    }

    pub fn with_metrics(self, service_name: String) -> Self {
        self.with_middleware(Arc::new(MetricsMiddleware::new(service_name)))
    }

    pub async fn execute_with_context<F, R, Fut>(
        &self,
        context: ServiceContext,
        operation: F,
    ) -> Result<ServiceResult<R>>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        // Execute before_call for all middlewares
        for middleware in &self.middlewares {
            middleware.before_call(&context).await?;
        }

        let start_time = Instant::now();
        let result = operation().await;
        let duration = start_time.elapsed();

        match result {
            Ok(data) => {
                let service_result = ServiceResult::new(data, duration);

                // Execute after_call for all middlewares
                for middleware in &self.middlewares {
                    let dummy_result = ServiceResult::new((), duration);
                    middleware.after_call(&context, &dummy_result).await?;
                }

                Ok(service_result)
            }
            Err(error) => {
                // Execute on_error for all middlewares
                for middleware in &self.middlewares {
                    middleware.on_error(&context, &error).await?;
                }

                Err(error)
            }
        }
    }

    pub fn inner(&self) -> &Arc<T> {
        &self.inner
    }
}

#[async_trait]
impl<T: Service> Service for ServiceDecorator<T> {
    fn service_name(&self) -> &'static str {
        self.inner.service_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestService {
        call_count: Arc<AtomicUsize>,
        should_fail: bool,
    }

    impl TestService {
        fn new(should_fail: bool) -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                should_fail,
            }
        }

        async fn test_operation(&self) -> Result<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                Err(AppError::ExternalServiceError("Test failure".to_string()))
            } else {
                Ok("Success".to_string())
            }
        }
    }

    #[async_trait]
    impl Service for TestService {
        fn service_name(&self) -> &'static str {
            "TestService"
        }
    }

    #[tokio::test]
    async fn test_service_decorator_with_logging() {
        let service = Arc::new(TestService::new(false));
        let decorator = ServiceDecorator::new(service.clone()).with_logging();

        let context = ServiceContext::new("test_operation".to_string());
        let result = decorator
            .execute_with_context(context, || async { service.test_operation().await })
            .await;

        assert!(result.is_ok());
        assert_eq!(service.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_middleware() {
        let service = Arc::new(TestService::new(true));
        let retry_config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        };

        let retry_middleware = RetryMiddleware::new(retry_config);
        let context = ServiceContext::new("test_operation".to_string());

        let result = retry_middleware
            .execute_with_retry(&context, || async { service.test_operation().await })
            .await;

        assert!(result.is_err());
        assert_eq!(service.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_middleware() {
        let service = Arc::new(TestService::new(true));
        let cb_config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(1),
            ..Default::default()
        };

        let circuit_breaker = CircuitBreakerMiddleware::new(cb_config);
        let context = ServiceContext::new("test_operation".to_string());

        // First two calls should fail and open the circuit
        for _ in 0..2 {
            let _ = circuit_breaker
                .execute_with_circuit_breaker(&context, || async { service.test_operation().await })
                .await;
        }

        // Third call should be rejected by circuit breaker
        let result = circuit_breaker
            .execute_with_circuit_breaker(&context, || async { service.test_operation().await })
            .await;

        assert!(result.is_err());
        // Should be 2 calls, not 3, because circuit breaker rejected the third
        assert_eq!(service.call_count.load(Ordering::SeqCst), 2);
    }
}
