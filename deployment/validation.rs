use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Deployment validation framework
pub struct DeploymentValidator {
    validators: Vec<Box<dyn Validator>>,
    config: ValidationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
    pub parallel_validation: bool,
    pub fail_fast: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300), // 5 minutes
            retry_attempts: 3,
            retry_delay: Duration::from_secs(5),
            parallel_validation: true,
            fail_fast: true,
        }
    }
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub validator_name: String,
    pub success: bool,
    pub duration: Duration,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// Validation summary
#[derive(Debug)]
pub struct ValidationSummary {
    pub total_validators: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_duration: Duration,
    pub results: Vec<ValidationResult>,
}

/// Validator trait
#[async_trait::async_trait]
pub trait Validator: Send + Sync {
    async fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, ValidationError>;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn dependencies(&self) -> Vec<String> { Vec::new() }
    fn is_critical(&self) -> bool { true }
}

/// Validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub deployment_id: Uuid,
    pub environment: String,
    pub service_url: String,
    pub database_url: String,
    pub metadata: HashMap<String, String>,
}

impl DeploymentValidator {
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            validators: Vec::new(),
            config,
        }
    }

    pub fn add_validator(&mut self, validator: Box<dyn Validator>) {
        self.validators.push(validator);
    }

    /// Run all validations
    pub async fn validate(&self, context: &ValidationContext) -> Result<ValidationSummary, ValidationError> {
        println!("🔍 Starting deployment validation...");
        println!("📋 Running {} validators", self.validators.len());

        let start_time = std::time::Instant::now();
        let mut results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;

        if self.config.parallel_validation {
            results = self.run_parallel_validation(context).await?;
        } else {
            results = self.run_sequential_validation(context).await?;
        }

        // Count results
        for result in &results {
            if result.success {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        let summary = ValidationSummary {
            total_validators: self.validators.len(),
            passed,
            failed,
            skipped,
            total_duration: start_time.elapsed(),
            results,
        };

        self.print_summary(&summary);
        Ok(summary)
    }

    async fn run_parallel_validation(&self, context: &ValidationContext) -> Result<Vec<ValidationResult>, ValidationError> {
        use futures::future::join_all;

        let mut handles = Vec::new();
        
        for validator in &self.validators {
            let validator_ref = validator.as_ref();
            let context_clone = context.clone();
            let config = self.config.clone();
            
            let handle = tokio::spawn(async move {
                Self::run_single_validator(validator_ref, &context_clone, &config).await
            });
            
            handles.push(handle);
        }

        let results = join_all(handles).await;
        let mut validation_results = Vec::new();

        for result in results {
            match result {
                Ok(validation_result) => validation_results.push(validation_result),
                Err(e) => {
                    return Err(ValidationError::ExecutionFailed(format!("Task join error: {}", e)));
                }
            }
        }

        Ok(validation_results)
    }

    async fn run_sequential_validation(&self, context: &ValidationContext) -> Result<Vec<ValidationResult>, ValidationError> {
        let mut results = Vec::new();

        for validator in &self.validators {
            let result = Self::run_single_validator(validator.as_ref(), context, &self.config).await;
            
            if !result.success && self.config.fail_fast && validator.is_critical() {
                println!("❌ Critical validation failed, stopping validation");
                results.push(result);
                break;
            }
            
            results.push(result);
        }

        Ok(results)
    }

    async fn run_single_validator(
        validator: &dyn Validator,
        context: &ValidationContext,
        config: &ValidationConfig,
    ) -> ValidationResult {
        println!("🔍 Running validator: {}", validator.name());
        
        let start_time = std::time::Instant::now();
        let mut attempts = 0;
        
        loop {
            attempts += 1;
            
            match tokio::time::timeout(config.timeout, validator.validate(context)).await {
                Ok(Ok(result)) => {
                    if result.success {
                        println!("✅ {} passed", validator.name());
                    } else {
                        println!("❌ {} failed: {}", validator.name(), result.message);
                    }
                    return result;
                }
                Ok(Err(e)) => {
                    if attempts >= config.retry_attempts {
                        println!("❌ {} failed after {} attempts: {}", validator.name(), attempts, e);
                        return ValidationResult {
                            validator_name: validator.name().to_string(),
                            success: false,
                            duration: start_time.elapsed(),
                            message: format!("Failed after {} attempts: {}", attempts, e),
                            details: None,
                            timestamp: Utc::now(),
                        };
                    }
                    
                    println!("⚠️ {} failed (attempt {}), retrying in {:?}...", 
                        validator.name(), attempts, config.retry_delay);
                    tokio::time::sleep(config.retry_delay).await;
                }
                Err(_) => {
                    if attempts >= config.retry_attempts {
                        println!("❌ {} timed out after {} attempts", validator.name(), attempts);
                        return ValidationResult {
                            validator_name: validator.name().to_string(),
                            success: false,
                            duration: start_time.elapsed(),
                            message: format!("Timed out after {} attempts", attempts),
                            details: None,
                            timestamp: Utc::now(),
                        };
                    }
                    
                    println!("⚠️ {} timed out (attempt {}), retrying...", validator.name(), attempts);
                    tokio::time::sleep(config.retry_delay).await;
                }
            }
        }
    }

    fn print_summary(&self, summary: &ValidationSummary) {
        println!("\n🔍 Validation Summary");
        println!("{'=':<50}");
        println!("📊 Total Validators: {}", summary.total_validators);
        println!("✅ Passed: {}", summary.passed);
        println!("❌ Failed: {}", summary.failed);
        println!("⏭️ Skipped: {}", summary.skipped);
        println!("⏱️ Total Duration: {:?}", summary.total_duration);
        
        let success_rate = (summary.passed as f64 / summary.total_validators as f64) * 100.0;
        println!("📈 Success Rate: {:.1}%", success_rate);
        
        if summary.failed > 0 {
            println!("\n❌ Failed Validations:");
            for result in &summary.results {
                if !result.success {
                    println!("   - {}: {}", result.validator_name, result.message);
                }
            }
        }
        
        println!("{'=':<50}");
    }
}

// Built-in validators

/// Health check validator
pub struct HealthCheckValidator {
    endpoint: String,
    expected_status: u16,
}

impl HealthCheckValidator {
    pub fn new(endpoint: &str, expected_status: u16) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            expected_status,
        }
    }
}

#[async_trait::async_trait]
impl Validator for HealthCheckValidator {
    async fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, ValidationError> {
        let start_time = std::time::Instant::now();
        let url = format!("{}{}", context.service_url, self.endpoint);
        
        // Simulate HTTP request
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate response
        let status_code = if url.contains("/health") { 200 } else { 404 };
        let success = status_code == self.expected_status;
        
        Ok(ValidationResult {
            validator_name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            message: if success {
                format!("Health check passed (status: {})", status_code)
            } else {
                format!("Expected status {}, got {}", self.expected_status, status_code)
            },
            details: Some(serde_json::json!({
                "url": url,
                "expected_status": self.expected_status,
                "actual_status": status_code
            })),
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "health_check"
    }

    fn description(&self) -> &str {
        "Validates service health endpoint"
    }
}

/// Database connectivity validator
pub struct DatabaseValidator {
    connection_string: String,
}

impl DatabaseValidator {
    pub fn new(connection_string: &str) -> Self {
        Self {
            connection_string: connection_string.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Validator for DatabaseValidator {
    async fn validate(&self, _context: &ValidationContext) -> Result<ValidationResult, ValidationError> {
        let start_time = std::time::Instant::now();
        
        // Simulate database connection test
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Simulate connection result
        let success = !self.connection_string.contains("invalid");
        
        Ok(ValidationResult {
            validator_name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            message: if success {
                "Database connection successful".to_string()
            } else {
                "Database connection failed".to_string()
            },
            details: Some(serde_json::json!({
                "connection_string": self.connection_string
            })),
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "database_connectivity"
    }

    fn description(&self) -> &str {
        "Validates database connectivity"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

/// API endpoint validator
pub struct ApiEndpointValidator {
    endpoints: Vec<EndpointTest>,
}

#[derive(Debug, Clone)]
pub struct EndpointTest {
    pub path: String,
    pub method: String,
    pub expected_status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl ApiEndpointValidator {
    pub fn new(endpoints: Vec<EndpointTest>) -> Self {
        Self { endpoints }
    }
}

#[async_trait::async_trait]
impl Validator for ApiEndpointValidator {
    async fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, ValidationError> {
        let start_time = std::time::Instant::now();
        let mut failed_endpoints = Vec::new();
        
        for endpoint in &self.endpoints {
            let url = format!("{}{}", context.service_url, endpoint.path);
            
            // Simulate API request
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Simulate response based on endpoint
            let status_code = match endpoint.path.as_str() {
                "/api/health" => 200,
                "/api/users/me" => if endpoint.headers.contains_key("Authorization") { 200 } else { 401 },
                "/api/workspaces" => 200,
                _ => 404,
            };
            
            if status_code != endpoint.expected_status {
                failed_endpoints.push(format!("{} {} (expected: {}, got: {})", 
                    endpoint.method, endpoint.path, endpoint.expected_status, status_code));
            }
        }
        
        let success = failed_endpoints.is_empty();
        
        Ok(ValidationResult {
            validator_name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            message: if success {
                format!("All {} API endpoints validated successfully", self.endpoints.len())
            } else {
                format!("Failed endpoints: {}", failed_endpoints.join(", "))
            },
            details: Some(serde_json::json!({
                "total_endpoints": self.endpoints.len(),
                "failed_endpoints": failed_endpoints
            })),
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "api_endpoints"
    }

    fn description(&self) -> &str {
        "Validates API endpoint responses"
    }
}

/// Performance validator
pub struct PerformanceValidator {
    max_response_time: Duration,
    endpoint: String,
    concurrent_requests: usize,
}

impl PerformanceValidator {
    pub fn new(endpoint: &str, max_response_time: Duration, concurrent_requests: usize) -> Self {
        Self {
            max_response_time,
            endpoint: endpoint.to_string(),
            concurrent_requests,
        }
    }
}

#[async_trait::async_trait]
impl Validator for PerformanceValidator {
    async fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, ValidationError> {
        let start_time = std::time::Instant::now();
        let url = format!("{}{}", context.service_url, self.endpoint);
        
        // Simulate concurrent requests
        let mut handles = Vec::new();
        for _ in 0..self.concurrent_requests {
            let handle = tokio::spawn(async {
                let request_start = std::time::Instant::now();
                tokio::time::sleep(Duration::from_millis(rand::random::<u64>() % 100 + 50)).await;
                request_start.elapsed()
            });
            handles.push(handle);
        }
        
        let results = futures::future::join_all(handles).await;
        let response_times: Vec<Duration> = results.into_iter()
            .filter_map(|r| r.ok())
            .collect();
        
        let avg_response_time = response_times.iter().sum::<Duration>() / response_times.len() as u32;
        let max_response_time = response_times.iter().max().copied().unwrap_or(Duration::ZERO);
        
        let success = avg_response_time <= self.max_response_time;
        
        Ok(ValidationResult {
            validator_name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            message: if success {
                format!("Performance test passed (avg: {:?}, max: {:?})", avg_response_time, max_response_time)
            } else {
                format!("Performance test failed (avg: {:?} > threshold: {:?})", avg_response_time, self.max_response_time)
            },
            details: Some(serde_json::json!({
                "url": url,
                "concurrent_requests": self.concurrent_requests,
                "average_response_time_ms": avg_response_time.as_millis(),
                "max_response_time_ms": max_response_time.as_millis(),
                "threshold_ms": self.max_response_time.as_millis()
            })),
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "performance"
    }

    fn description(&self) -> &str {
        "Validates service performance under load"
    }

    fn is_critical(&self) -> bool {
        false // Performance issues are warnings, not critical failures
    }
}

/// Security validator
pub struct SecurityValidator {
    security_headers: Vec<String>,
}

impl SecurityValidator {
    pub fn new() -> Self {
        Self {
            security_headers: vec![
                "X-Content-Type-Options".to_string(),
                "X-Frame-Options".to_string(),
                "X-XSS-Protection".to_string(),
                "Strict-Transport-Security".to_string(),
            ],
        }
    }
}

#[async_trait::async_trait]
impl Validator for SecurityValidator {
    async fn validate(&self, context: &ValidationContext) -> Result<ValidationResult, ValidationError> {
        let start_time = std::time::Instant::now();
        
        // Simulate security header check
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate missing headers (for demonstration)
        let missing_headers = vec!["X-Frame-Options"];
        let success = missing_headers.is_empty();
        
        Ok(ValidationResult {
            validator_name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            message: if success {
                "All security headers present".to_string()
            } else {
                format!("Missing security headers: {}", missing_headers.join(", "))
            },
            details: Some(serde_json::json!({
                "required_headers": self.security_headers,
                "missing_headers": missing_headers
            })),
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "security_headers"
    }

    fn description(&self) -> &str {
        "Validates security headers are present"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Validation execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Validation timeout: {0}")]
    Timeout(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Validation builder for easy setup
pub struct ValidationBuilder {
    validator: DeploymentValidator,
}

impl ValidationBuilder {
    pub fn new() -> Self {
        Self {
            validator: DeploymentValidator::new(ValidationConfig::default()),
        }
    }

    pub fn with_config(mut self, config: ValidationConfig) -> Self {
        self.validator.config = config;
        self
    }

    pub fn add_health_check(mut self, endpoint: &str) -> Self {
        self.validator.add_validator(Box::new(HealthCheckValidator::new(endpoint, 200)));
        self
    }

    pub fn add_database_check(mut self, connection_string: &str) -> Self {
        self.validator.add_validator(Box::new(DatabaseValidator::new(connection_string)));
        self
    }

    pub fn add_api_endpoints(mut self, endpoints: Vec<EndpointTest>) -> Self {
        self.validator.add_validator(Box::new(ApiEndpointValidator::new(endpoints)));
        self
    }

    pub fn add_performance_check(mut self, endpoint: &str, max_response_time: Duration) -> Self {
        self.validator.add_validator(Box::new(PerformanceValidator::new(endpoint, max_response_time, 10)));
        self
    }

    pub fn add_security_check(mut self) -> Self {
        self.validator.add_validator(Box::new(SecurityValidator::new()));
        self
    }

    pub fn build(self) -> DeploymentValidator {
        self.validator
    }
}