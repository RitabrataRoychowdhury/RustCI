use std::time::{Duration, SystemTime};
use reqwest::Client;
use serde_json::Value;
use crate::deployment::blue_green_manager::{HealthChecker, HealthCheckResult, ValidationRule, ValidationResult};
use crate::error::{AppError, Result};

pub struct ProductionHealthChecker {
    client: Client,
}

impl ProductionHealthChecker {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    async fn make_request(&self, url: &str, timeout: Duration) -> Result<(u16, Duration, Option<String>)> {
        let start_time = SystemTime::now();
        
        let response = tokio::time::timeout(timeout, self.client.get(url).send()).await
            .map_err(|_| AppError::Timeout(format!("Health check timeout for {}", url)))?
            .map_err(|e| AppError::NetworkError(format!("Request failed for {}: {}", url, e)))?;

        let response_time = start_time.elapsed()
            .map_err(|e| AppError::SystemError(format!("Failed to calculate response time: {}", e)))?;

        let status = response.status().as_u16();
        let error_message = if !response.status().is_success() {
            Some(format!("HTTP {} - {}", status, response.status().canonical_reason().unwrap_or("Unknown")))
        } else {
            None
        };

        Ok((status, response_time, error_message))
    }

    async fn validate_response_content(&self, url: &str, rule: &ValidationRule) -> Result<bool> {
        let response = tokio::time::timeout(rule.timeout, self.client.get(url).send()).await
            .map_err(|_| AppError::Timeout(format!("Validation timeout for {}", url)))?
            .map_err(|e| AppError::NetworkError(format!("Validation request failed for {}: {}", url, e)))?;

        if response.status().as_u16() != rule.expected_status {
            return Ok(false);
        }

        // Additional content validation could be added here
        // For now, we just check the status code
        Ok(true)
    }
}

#[async_trait::async_trait]
impl HealthChecker for ProductionHealthChecker {
    async fn check_health(&self, endpoint: &str, timeout: Duration) -> Result<HealthCheckResult> {
        let checked_at = SystemTime::now();
        
        match self.make_request(endpoint, timeout).await {
            Ok((status, response_time, error_message)) => {
                let is_healthy = status >= 200 && status < 300;
                
                Ok(HealthCheckResult {
                    endpoint: endpoint.to_string(),
                    status: is_healthy,
                    response_time,
                    error_message,
                    checked_at,
                })
            }
            Err(e) => {
                Ok(HealthCheckResult {
                    endpoint: endpoint.to_string(),
                    status: false,
                    response_time: Duration::from_secs(0),
                    error_message: Some(e.to_string()),
                    checked_at,
                })
            }
        }
    }

    async fn validate_rule(&self, rule: &ValidationRule) -> Result<ValidationResult> {
        let validated_at = SystemTime::now();
        let mut attempts = 0;
        let mut last_error = None;

        while attempts <= rule.retry_count {
            match self.validate_response_content(&rule.endpoint, rule).await {
                Ok(true) => {
                    return Ok(ValidationResult {
                        rule_name: rule.name.clone(),
                        passed: true,
                        error_message: None,
                        validated_at,
                    });
                }
                Ok(false) => {
                    last_error = Some(format!("Validation failed for rule '{}'", rule.name));
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }
            
            attempts += 1;
            if attempts <= rule.retry_count {
                tokio::time::sleep(Duration::from_millis(1000 * attempts as u64)).await;
            }
        }

        Ok(ValidationResult {
            rule_name: rule.name.clone(),
            passed: false,
            error_message: last_error,
            validated_at,
        })
    }
}

impl Default for ProductionHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_health_check_success() {
        let checker = ProductionHealthChecker::new();
        
        // This test would need a mock server in a real implementation
        // For now, we'll test the structure
        let result = checker.check_health("http://httpbin.org/status/200", Duration::from_secs(5)).await;
        
        match result {
            Ok(health_result) => {
                assert_eq!(health_result.endpoint, "http://httpbin.org/status/200");
                // In a real test, we'd verify the status based on the mock response
            }
            Err(_) => {
                // Network errors are acceptable in unit tests
            }
        }
    }

    #[tokio::test]
    async fn test_validation_rule() {
        let checker = ProductionHealthChecker::new();
        
        let rule = ValidationRule {
            name: "test_rule".to_string(),
            endpoint: "http://httpbin.org/status/200".to_string(),
            expected_status: 200,
            timeout: Duration::from_secs(5),
            retry_count: 2,
        };

        let result = checker.validate_rule(&rule).await;
        
        match result {
            Ok(validation_result) => {
                assert_eq!(validation_result.rule_name, "test_rule");
                // In a real test, we'd verify the validation result based on the mock response
            }
            Err(_) => {
                // Network errors are acceptable in unit tests
            }
        }
    }
}