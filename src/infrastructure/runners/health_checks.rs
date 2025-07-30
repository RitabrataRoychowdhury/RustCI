//! Health check utilities for runners
//!
//! This module provides health check functions for Docker and Kubernetes
//! connectivity to ensure runners can operate properly.

use bollard::Docker;
use kube::Client;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Docker health check configuration
#[derive(Debug, Clone)]
pub struct DockerHealthConfig {
    /// Docker daemon endpoint
    pub endpoint: String,
    /// Connection timeout
    pub timeout_seconds: u64,
}

impl Default for DockerHealthConfig {
    fn default() -> Self {
        Self {
            endpoint: "unix:///var/run/docker.sock".to_string(),
            timeout_seconds: 10,
        }
    }
}

/// Kubernetes health check configuration
#[derive(Debug, Clone)]
pub struct KubernetesHealthConfig {
    /// Connection timeout
    pub timeout_seconds: u64,
    /// Namespace to check
    pub namespace: String,
}

impl Default for KubernetesHealthConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
            namespace: "default".to_string(),
        }
    }
}

/// Docker health check result
#[derive(Debug, Clone)]
pub struct DockerHealthResult {
    /// Whether Docker is accessible
    pub is_accessible: bool,
    /// Docker version information
    pub version: Option<String>,
    /// Error message if not accessible
    pub error_message: Option<String>,
    /// Response time in milliseconds
    pub response_time_ms: u64,
}

/// Kubernetes health check result
#[derive(Debug, Clone)]
pub struct KubernetesHealthResult {
    /// Whether Kubernetes is accessible
    pub is_accessible: bool,
    /// Kubernetes version information
    pub version: Option<String>,
    /// Error message if not accessible
    pub error_message: Option<String>,
    /// Response time in milliseconds
    pub response_time_ms: u64,
}

/// Perform Docker daemon health check
pub async fn check_docker_health(config: DockerHealthConfig) -> DockerHealthResult {
    let start_time = std::time::Instant::now();

    let docker_result = if config.endpoint.starts_with("unix://") {
        Docker::connect_with_socket(&config.endpoint, 120, bollard::API_DEFAULT_VERSION)
    } else {
        Docker::connect_with_http(&config.endpoint, 120, bollard::API_DEFAULT_VERSION)
    };

    let docker = match docker_result {
        Ok(docker) => docker,
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            return DockerHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!("Failed to connect to Docker: {}", e)),
                response_time_ms: response_time,
            };
        }
    };

    // Perform ping with timeout
    let ping_result = timeout(Duration::from_secs(config.timeout_seconds), docker.ping()).await;

    let response_time = start_time.elapsed().as_millis() as u64;

    match ping_result {
        Ok(Ok(_)) => {
            // Get version information
            let version = match docker.version().await {
                Ok(version_info) => Some(version_info.version.unwrap_or_default()),
                Err(e) => {
                    warn!("Failed to get Docker version: {}", e);
                    None
                }
            };

            debug!("Docker health check passed in {}ms", response_time);
            DockerHealthResult {
                is_accessible: true,
                version,
                error_message: None,
                response_time_ms: response_time,
            }
        }
        Ok(Err(e)) => {
            warn!("Docker ping failed: {}", e);
            DockerHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!("Docker ping failed: {}", e)),
                response_time_ms: response_time,
            }
        }
        Err(_) => {
            warn!(
                "Docker health check timed out after {}s",
                config.timeout_seconds
            );
            DockerHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!(
                    "Health check timed out after {}s",
                    config.timeout_seconds
                )),
                response_time_ms: response_time,
            }
        }
    }
}

/// Perform Kubernetes cluster health check
pub async fn check_kubernetes_health(config: KubernetesHealthConfig) -> KubernetesHealthResult {
    let start_time = std::time::Instant::now();

    // Create Kubernetes client
    let client = match Client::try_default().await {
        Ok(client) => client,
        Err(e) => {
            let response_time = start_time.elapsed().as_millis() as u64;
            return KubernetesHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!("Failed to create Kubernetes client: {}", e)),
                response_time_ms: response_time,
            };
        }
    };

    // Check API server version with timeout
    let version_result = timeout(
        Duration::from_secs(config.timeout_seconds),
        client.apiserver_version(),
    )
    .await;

    let response_time = start_time.elapsed().as_millis() as u64;

    match version_result {
        Ok(Ok(version_info)) => {
            debug!("Kubernetes health check passed in {}ms", response_time);
            KubernetesHealthResult {
                is_accessible: true,
                version: Some(version_info.git_version),
                error_message: None,
                response_time_ms: response_time,
            }
        }
        Ok(Err(e)) => {
            warn!("Kubernetes API server check failed: {}", e);
            KubernetesHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!("API server check failed: {}", e)),
                response_time_ms: response_time,
            }
        }
        Err(_) => {
            warn!(
                "Kubernetes health check timed out after {}s",
                config.timeout_seconds
            );
            KubernetesHealthResult {
                is_accessible: false,
                version: None,
                error_message: Some(format!(
                    "Health check timed out after {}s",
                    config.timeout_seconds
                )),
                response_time_ms: response_time,
            }
        }
    }
}

/// Check if Docker is available for testing
pub async fn is_docker_available() -> bool {
    let config = DockerHealthConfig::default();
    let result = check_docker_health(config).await;
    result.is_accessible
}

/// Check if Kubernetes is available for testing
pub async fn is_kubernetes_available() -> bool {
    let config = KubernetesHealthConfig::default();
    let result = check_kubernetes_health(config).await;
    result.is_accessible
}

/// Comprehensive runner health check
pub async fn check_all_runner_health() -> RunnerHealthSummary {
    let docker_check =
        tokio::spawn(async { check_docker_health(DockerHealthConfig::default()).await });

    let k8s_check =
        tokio::spawn(async { check_kubernetes_health(KubernetesHealthConfig::default()).await });

    let (docker_result, k8s_result) = tokio::join!(docker_check, k8s_check);

    RunnerHealthSummary {
        docker: docker_result.unwrap_or_else(|_| DockerHealthResult {
            is_accessible: false,
            version: None,
            error_message: Some("Health check task failed".to_string()),
            response_time_ms: 0,
        }),
        kubernetes: k8s_result.unwrap_or_else(|_| KubernetesHealthResult {
            is_accessible: false,
            version: None,
            error_message: Some("Health check task failed".to_string()),
            response_time_ms: 0,
        }),
    }
}

/// Summary of all runner health checks
#[derive(Debug, Clone)]
pub struct RunnerHealthSummary {
    /// Docker health check result
    pub docker: DockerHealthResult,
    /// Kubernetes health check result
    pub kubernetes: KubernetesHealthResult,
}

impl RunnerHealthSummary {
    /// Check if any runners are available
    pub fn has_available_runners(&self) -> bool {
        self.docker.is_accessible || self.kubernetes.is_accessible
    }

    /// Get list of available runner types
    pub fn available_runner_types(&self) -> Vec<String> {
        let mut types = Vec::new();

        if self.docker.is_accessible {
            types.push("docker".to_string());
        }

        if self.kubernetes.is_accessible {
            types.push("kubernetes".to_string());
        }

        // Local runner is always available
        types.push("local".to_string());

        types
    }

    /// Get health status message
    pub fn status_message(&self) -> String {
        let mut messages = Vec::new();

        if self.docker.is_accessible {
            messages.push(format!("Docker: OK ({}ms)", self.docker.response_time_ms));
        } else {
            messages.push(format!(
                "Docker: FAILED - {}",
                self.docker
                    .error_message
                    .as_deref()
                    .unwrap_or("Unknown error")
            ));
        }

        if self.kubernetes.is_accessible {
            messages.push(format!(
                "Kubernetes: OK ({}ms)",
                self.kubernetes.response_time_ms
            ));
        } else {
            messages.push(format!(
                "Kubernetes: FAILED - {}",
                self.kubernetes
                    .error_message
                    .as_deref()
                    .unwrap_or("Unknown error")
            ));
        }

        messages.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_health_config_default() {
        let config = DockerHealthConfig::default();
        assert_eq!(config.endpoint, "unix:///var/run/docker.sock");
        assert_eq!(config.timeout_seconds, 10);
    }

    #[tokio::test]
    async fn test_kubernetes_health_config_default() {
        let config = KubernetesHealthConfig::default();
        assert_eq!(config.timeout_seconds, 10);
        assert_eq!(config.namespace, "default");
    }

    #[tokio::test]
    async fn test_runner_health_summary() {
        let docker_result = DockerHealthResult {
            is_accessible: true,
            version: Some("20.10.0".to_string()),
            error_message: None,
            response_time_ms: 50,
        };

        let k8s_result = KubernetesHealthResult {
            is_accessible: false,
            version: None,
            error_message: Some("Connection refused".to_string()),
            response_time_ms: 1000,
        };

        let summary = RunnerHealthSummary {
            docker: docker_result,
            kubernetes: k8s_result,
        };

        assert!(summary.has_available_runners());
        let available_types = summary.available_runner_types();
        assert!(available_types.contains(&"docker".to_string()));
        assert!(available_types.contains(&"local".to_string()));
        assert!(!available_types.contains(&"kubernetes".to_string()));

        let status = summary.status_message();
        assert!(status.contains("Docker: OK"));
        assert!(status.contains("Kubernetes: FAILED"));
    }

    #[tokio::test]
    async fn test_docker_health_check_with_invalid_endpoint() {
        let config = DockerHealthConfig {
            endpoint: "unix:///invalid/socket".to_string(),
            timeout_seconds: 1,
        };

        let result = check_docker_health(config).await;
        assert!(!result.is_accessible);
        assert!(result.error_message.is_some());
    }

    #[tokio::test]
    async fn test_is_docker_available() {
        // This test will pass or fail based on whether Docker is actually available
        let available = is_docker_available().await;
        // We can't assert a specific value since it depends on the environment
        println!("Docker available: {}", available);
    }

    #[tokio::test]
    async fn test_is_kubernetes_available() {
        // This test will pass or fail based on whether Kubernetes is actually available
        let available = is_kubernetes_available().await;
        // We can't assert a specific value since it depends on the environment
        println!("Kubernetes available: {}", available);
    }

    #[tokio::test]
    async fn test_check_all_runner_health() {
        let summary = check_all_runner_health().await;

        // Should always have local runner available
        let available_types = summary.available_runner_types();
        assert!(available_types.contains(&"local".to_string()));

        println!("Health summary: {}", summary.status_message());
    }
}
