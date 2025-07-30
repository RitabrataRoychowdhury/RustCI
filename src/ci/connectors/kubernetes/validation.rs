//! Kubernetes validation utilities
//!
//! This module provides validation functions for Kubernetes configurations,
//! cluster connectivity, and resource availability.

use super::super::traits::KubernetesConfig;
use crate::ci::config::Step;
use crate::error::{AppError, Result};
use std::collections::HashMap;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Validate Kubernetes cluster connectivity and configuration
pub struct KubernetesValidator;

impl KubernetesValidator {
    /// Validate that kubectl is available and configured
    pub async fn validate_kubectl_available() -> Result<()> {
        debug!("🔍 Validating kubectl availability");

        let output = Command::new("kubectl")
            .args(["version", "--client", "--short"])
            .output()
            .await
            .map_err(|e| {
                AppError::ConnectorConfigError(format!("kubectl not found on system: {}", e))
            })?;

        if !output.status.success() {
            return Err(AppError::ConnectorConfigError(
                "kubectl is not properly configured".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        debug!("✅ kubectl available: {}", version.trim());
        Ok(())
    }

    /// Validate cluster connectivity
    pub async fn validate_cluster_connectivity() -> Result<()> {
        debug!("🔍 Validating Kubernetes cluster connectivity");

        let output = Command::new("kubectl")
            .args(["cluster-info"])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!("Failed to check cluster info: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::KubernetesError(format!(
                "Cannot connect to Kubernetes cluster: {}",
                stderr
            )));
        }

        info!("✅ Kubernetes cluster connectivity validated");
        Ok(())
    }

    /// Validate namespace exists and is accessible
    pub async fn validate_namespace(namespace: &str) -> Result<()> {
        debug!("🔍 Validating namespace: {}", namespace);

        let output = Command::new("kubectl")
            .args(["get", "namespace", namespace])
            .output()
            .await
            .map_err(|e| AppError::KubernetesError(format!("Failed to check namespace: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not found") {
                return Err(AppError::KubernetesError(format!(
                    "Namespace '{}' does not exist",
                    namespace
                )));
            } else {
                return Err(AppError::KubernetesError(format!(
                    "Cannot access namespace '{}': {}",
                    namespace, stderr
                )));
            }
        }

        debug!("✅ Namespace '{}' is accessible", namespace);
        Ok(())
    }

    /// Validate resource quotas and limits
    pub async fn validate_resource_availability(
        namespace: &str,
        resource_requests: &Option<HashMap<String, String>>,
        resource_limits: &Option<HashMap<String, String>>,
    ) -> Result<()> {
        debug!(
            "🔍 Validating resource availability in namespace: {}",
            namespace
        );

        // Check resource quotas if they exist
        let output = Command::new("kubectl")
            .args(["get", "resourcequota", "-n", namespace, "-o", "json"])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!("Failed to check resource quotas: {}", e))
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("\"items\":[]") {
                debug!("📊 Resource quotas found in namespace, checking availability");
                // TODO: Parse JSON and validate against requested resources
                // For now, we'll just log a warning
                warn!("⚠️ Resource quotas exist but detailed validation not implemented yet");
            }
        }

        // Validate resource request/limit format
        if let Some(requests) = resource_requests {
            Self::validate_resource_format(requests, "requests")?;
        }

        if let Some(limits) = resource_limits {
            Self::validate_resource_format(limits, "limits")?;
        }

        debug!("✅ Resource availability validation completed");
        Ok(())
    }

    /// Validate resource format (CPU, memory, etc.)
    fn validate_resource_format(
        resources: &HashMap<String, String>,
        resource_type: &str,
    ) -> Result<()> {
        for (key, value) in resources {
            match key.as_str() {
                "cpu" => {
                    if !Self::is_valid_cpu_format(value) {
                        return Err(AppError::ValidationError(format!(
                            "Invalid CPU {} format: {}",
                            resource_type, value
                        )));
                    }
                }
                "memory" => {
                    if !Self::is_valid_memory_format(value) {
                        return Err(AppError::ValidationError(format!(
                            "Invalid memory {} format: {}",
                            resource_type, value
                        )));
                    }
                }
                _ => {
                    debug!("🔍 Unknown resource type '{}', skipping validation", key);
                }
            }
        }
        Ok(())
    }

    /// Validate CPU format (e.g., "100m", "0.1", "1")
    fn is_valid_cpu_format(cpu: &str) -> bool {
        if let Some(cpu_value) = cpu.strip_suffix('m') {
            cpu_value.parse::<u32>().is_ok()
        } else {
            cpu.parse::<f64>().is_ok()
        }
    }

    /// Validate memory format (e.g., "128Mi", "1Gi", "512M")
    fn is_valid_memory_format(memory: &str) -> bool {
        let suffixes = [
            "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "K", "M", "G", "T", "P", "E",
        ];

        for suffix in &suffixes {
            if let Some(memory_value) = memory.strip_suffix(suffix) {
                return memory_value.parse::<u64>().is_ok();
            }
        }

        // Check if it's just a number (bytes)
        memory.parse::<u64>().is_ok()
    }

    /// Validate step configuration for Kubernetes
    pub fn validate_step_config(step: &Step) -> Result<()> {
        debug!("🔍 Validating Kubernetes step config: {}", step.name);

        // Basic step validation
        if step.name.is_empty() {
            return Err(AppError::ValidationError(
                "Step name cannot be empty".to_string(),
            ));
        }

        // Validate required fields for Kubernetes execution
        if step.config.command.is_none() && step.config.script.is_none() {
            return Err(AppError::ValidationError(
                "Kubernetes step requires either 'command' or 'script'".to_string(),
            ));
        }

        // Validate image is specified (required for Kubernetes jobs)
        if step.config.image.is_none() {
            return Err(AppError::ValidationError(
                "Kubernetes step requires 'image' to be specified".to_string(),
            ));
        }

        // Validate namespace format if specified
        if let Some(namespace) = &step.config.namespace {
            if namespace.is_empty() {
                return Err(AppError::ValidationError(
                    "Kubernetes namespace cannot be empty".to_string(),
                ));
            }

            // Basic namespace name validation (RFC 1123)
            if !namespace
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(AppError::ValidationError(
                    "Kubernetes namespace must contain only lowercase letters, numbers, and hyphens".to_string()
                ));
            }

            if namespace.starts_with('-') || namespace.ends_with('-') {
                return Err(AppError::ValidationError(
                    "Kubernetes namespace cannot start or end with a hyphen".to_string(),
                ));
            }
        }

        debug!("✅ Kubernetes step config validation passed");
        Ok(())
    }

    /// Validate Kubernetes configuration object
    pub async fn validate_kubernetes_config(config: &KubernetesConfig) -> Result<()> {
        debug!("🔍 Validating Kubernetes configuration");

        // Validate basic config
        config.validate()?;

        // Validate cluster connectivity
        Self::validate_kubectl_available().await?;
        Self::validate_cluster_connectivity().await?;

        // Validate namespace
        Self::validate_namespace(&config.namespace).await?;

        // Validate resource availability
        Self::validate_resource_availability(
            &config.namespace,
            &config.resource_requests,
            &config.resource_limits,
        )
        .await?;

        info!("✅ Kubernetes configuration validation completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_format_validation() {
        assert!(KubernetesValidator::is_valid_cpu_format("100m"));
        assert!(KubernetesValidator::is_valid_cpu_format("0.1"));
        assert!(KubernetesValidator::is_valid_cpu_format("1"));
        assert!(KubernetesValidator::is_valid_cpu_format("2.5"));
        assert!(!KubernetesValidator::is_valid_cpu_format("invalid"));
        assert!(!KubernetesValidator::is_valid_cpu_format("100x"));
    }

    #[test]
    fn test_memory_format_validation() {
        assert!(KubernetesValidator::is_valid_memory_format("128Mi"));
        assert!(KubernetesValidator::is_valid_memory_format("1Gi"));
        assert!(KubernetesValidator::is_valid_memory_format("512M"));
        assert!(KubernetesValidator::is_valid_memory_format("1024"));
        assert!(!KubernetesValidator::is_valid_memory_format("invalid"));
        assert!(!KubernetesValidator::is_valid_memory_format("128Xx"));
    }
}
