//! RustCI API client for automated testing integration

use crate::ci::testing::{
    DeploymentTestError, DeploymentTestResult, ExecutionResponse, ExecutionStatus, HealthResponse,
    PipelineResponse,
};
use reqwest::{multipart, Client, Response};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// RustCI API client for automated testing
#[derive(Debug, Clone)]
pub struct RustCIApiClient {
    /// Base URL for the RustCI server
    base_url: String,
    /// HTTP client for making requests
    client: Client,
    /// Optional authentication token
    auth_token: Option<String>,
    /// Retry configuration
    retry_config: RetryConfig,
    /// Request timeout
    timeout: Duration,
}

impl RustCIApiClient {
    /// Create a new API client
    pub fn new<S: Into<String>>(base_url: S) -> DeploymentTestResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(DeploymentTestError::from)?;

        Ok(Self {
            base_url: base_url.into(),
            client,
            auth_token: None,
            retry_config: RetryConfig::default(),
            timeout: Duration::from_secs(30),
        })
    }

    /// Create a new API client with custom configuration
    pub fn with_config<S: Into<String>>(
        base_url: S,
        retry_config: RetryConfig,
        timeout: Duration,
    ) -> DeploymentTestResult<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(DeploymentTestError::from)?;

        Ok(Self {
            base_url: base_url.into(),
            client,
            auth_token: None,
            retry_config,
            timeout,
        })
    }

    /// Set authentication token
    pub fn with_auth_token<S: Into<String>>(mut self, token: S) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Check if RustCI server is healthy and accessible
    pub async fn health_check(&self) -> DeploymentTestResult<HealthResponse> {
        info!("Performing health check on RustCI server");

        let url = format!("{}/health", self.base_url);
        let response = self
            .execute_with_retry(|| async { self.client.get(&url).send().await })
            .await?;

        let health: HealthResponse = response.json().await?;
        info!("Health check successful: status={}", health.status);
        Ok(health)
    }

    /// Upload a pipeline YAML to RustCI
    pub async fn upload_pipeline(
        &self,
        yaml_content: &str,
        name: &str,
    ) -> DeploymentTestResult<PipelineResponse> {
        info!("Uploading pipeline: {}", name);

        let url = format!("{}/api/ci/pipelines/upload", self.base_url);

        // Create multipart form with pipeline YAML
        let form = multipart::Form::new().text("name", name.to_string()).part(
            "pipeline",
            multipart::Part::text(yaml_content.to_string())
                .file_name("pipeline.yaml")
                .mime_str("application/x-yaml")
                .map_err(|e| {
                    DeploymentTestError::configuration(format!("Invalid MIME type: {}", e))
                })?,
        );

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.post(&url).multipart(form.try_clone().unwrap());

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::pipeline_upload(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let pipeline: PipelineResponse = response.json().await?;
        info!("Pipeline uploaded successfully: id={}", pipeline.id);
        Ok(pipeline)
    }

    /// Trigger pipeline execution
    pub async fn trigger_pipeline(
        &self,
        pipeline_id: &str,
        branch: Option<&str>,
        environment: Option<&std::collections::HashMap<String, String>>,
    ) -> DeploymentTestResult<ExecutionResponse> {
        info!("Triggering pipeline execution: {}", pipeline_id);

        let url = format!("{}/api/ci/pipelines/{}/trigger", self.base_url, pipeline_id);

        let mut trigger_data = json!({
            "trigger_type": "manual",
            "branch": branch.unwrap_or("main"),
            "repository": "danawoodman/docker-node-hello-world"
        });

        if let Some(env) = environment {
            trigger_data["environment"] = json!(env);
        }

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.post(&url).json(&trigger_data);

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::pipeline_execution(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let execution: ExecutionResponse = response.json().await?;
        info!(
            "Pipeline triggered successfully: execution_id={}",
            execution.execution_id
        );
        Ok(execution)
    }

    /// Monitor pipeline execution status
    pub async fn get_execution_status(
        &self,
        execution_id: &str,
    ) -> DeploymentTestResult<ExecutionStatus> {
        debug!("Getting execution status: {}", execution_id);

        let url = format!("{}/api/ci/executions/{}", self.base_url, execution_id);

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.get(&url);

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::http_status(status, error_text));
        }

        let execution_status: ExecutionStatus = response.json().await?;
        debug!(
            "Execution status retrieved: status={}",
            execution_status.status
        );
        Ok(execution_status)
    }

    /// Wait for pipeline execution to complete
    pub async fn wait_for_completion(
        &self,
        execution_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> DeploymentTestResult<ExecutionStatus> {
        info!(
            "Waiting for execution completion: {} (timeout: {}s)",
            execution_id,
            timeout.as_secs()
        );

        let start_time = Instant::now();

        loop {
            if start_time.elapsed() > timeout {
                return Err(DeploymentTestError::timeout(
                    timeout.as_secs(),
                    format!("waiting for execution {}", execution_id),
                ));
            }

            let status = self.get_execution_status(execution_id).await?;

            match status.status.as_str() {
                "completed" | "success" | "failed" | "cancelled" | "error" => {
                    info!("Execution completed with status: {}", status.status);
                    return Ok(status);
                }
                "running" | "pending" | "queued" => {
                    debug!("Execution still in progress: {}", status.status);
                    sleep(poll_interval).await;
                }
                _ => {
                    warn!("Unknown execution status: {}", status.status);
                    sleep(poll_interval).await;
                }
            }
        }
    }

    /// Get execution logs
    pub async fn get_execution_logs(
        &self,
        execution_id: &str,
    ) -> DeploymentTestResult<Vec<String>> {
        debug!("Getting execution logs: {}", execution_id);

        let url = format!("{}/api/ci/executions/{}/logs", self.base_url, execution_id);

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.get(&url);

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::http_status(status, error_text));
        }

        let logs_response: Value = response.json().await?;
        let logs: Vec<String> = logs_response["logs"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        debug!("Retrieved {} log entries", logs.len());
        Ok(logs)
    }

    /// List all pipelines
    pub async fn list_pipelines(&self) -> DeploymentTestResult<Vec<PipelineResponse>> {
        debug!("Listing all pipelines");

        let url = format!("{}/api/ci/pipelines", self.base_url);

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.get(&url);

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::http_status(status, error_text));
        }

        let pipelines: Vec<PipelineResponse> = response.json().await?;
        debug!("Retrieved {} pipelines", pipelines.len());
        Ok(pipelines)
    }

    /// Delete a pipeline
    pub async fn delete_pipeline(&self, pipeline_id: &str) -> DeploymentTestResult<()> {
        info!("Deleting pipeline: {}", pipeline_id);

        let url = format!("{}/api/ci/pipelines/{}", self.base_url, pipeline_id);

        let response = self
            .execute_with_retry(|| async {
                let mut request = self.client.delete(&url);

                if let Some(ref token) = self.auth_token {
                    request = request.bearer_auth(token);
                }

                request.send().await
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(DeploymentTestError::http_status(status, error_text));
        }

        info!("Pipeline deleted successfully: {}", pipeline_id);
        Ok(())
    }

    /// Execute HTTP request with retry logic
    async fn execute_with_retry<F, Fut>(&self, request_fn: F) -> DeploymentTestResult<Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<Response, reqwest::Error>>,
    {
        let mut delay = self.retry_config.initial_delay;
        let mut last_error = None;

        for attempt in 1..=self.retry_config.max_attempts {
            match request_fn().await {
                Ok(response) => {
                    if response.status().is_success() || response.status().is_client_error() {
                        // Don't retry client errors (4xx)
                        return Ok(response);
                    }

                    // Server errors (5xx) should be retried
                    if attempt == self.retry_config.max_attempts {
                        let status = response.status().as_u16();
                        let error_text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());
                        return Err(DeploymentTestError::http_status(status, error_text));
                    }

                    warn!(
                        "HTTP request failed with status {}, retrying in {}ms (attempt {}/{})",
                        response.status(),
                        delay.as_millis(),
                        attempt,
                        self.retry_config.max_attempts
                    );
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt == self.retry_config.max_attempts {
                        return Err(DeploymentTestError::retry_limit_exceeded(
                            self.retry_config.max_attempts,
                            "HTTP request",
                        ));
                    }

                    warn!(
                        "HTTP request failed: {}, retrying in {}ms (attempt {}/{})",
                        last_error.as_ref().unwrap(),
                        delay.as_millis(),
                        attempt,
                        self.retry_config.max_attempts
                    );
                }
            }

            // Wait before retrying
            sleep(delay).await;

            // Exponential backoff
            delay = std::cmp::min(
                Duration::from_millis(
                    (delay.as_millis() as f64 * self.retry_config.backoff_multiplier) as u64,
                ),
                self.retry_config.max_delay,
            );
        }

        // This should never be reached due to the loop logic above
        Err(last_error
            .map(DeploymentTestError::from)
            .unwrap_or_else(|| {
                DeploymentTestError::retry_limit_exceeded(
                    self.retry_config.max_attempts,
                    "HTTP request",
                )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_api_client_creation() {
        let client = RustCIApiClient::new("http://localhost:8080").unwrap();
        assert_eq!(client.base_url, "http://localhost:8080");
        assert!(client.auth_token.is_none());
    }

    #[tokio::test]
    async fn test_api_client_with_auth() {
        let client = RustCIApiClient::new("http://localhost:8080")
            .unwrap()
            .with_auth_token("test-token");
        assert_eq!(client.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
    }
}
