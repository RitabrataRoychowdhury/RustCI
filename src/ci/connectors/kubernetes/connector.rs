//! Kubernetes connector implementation
//! 
//! This connector handles Kubernetes-based step execution including:
//! - Job creation and lifecycle management
//! - YAML generation and validation
//! - Log collection and monitoring
//! - Resource cleanup and error handling

use crate::ci::{config::Step, workspace::Workspace};
use crate::error::{AppError, Result};

use super::super::traits::{Connector, ConnectorType, ExecutionResult, KubernetesConfig};
use super::{
    job_manager::KubernetesJobManager, 
    yaml_generator::KubernetesYamlGenerator, 
    validation::KubernetesValidator,
    lifecycle_hooks::LifecycleHookManager
};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Kubernetes connector for executing steps as Kubernetes Jobs
pub struct KubernetesConnector {
    /// Unique identifier for this connector instance
    id: String,
}

impl KubernetesConnector {
    /// Create a new Kubernetes connector instance
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
        }
    }

    /// Create Kubernetes configuration from step
    fn create_kubernetes_config(&self, step: &Step) -> KubernetesConfig {
        KubernetesConfig::from_step(step)
    }

    /// Execute step with retry logic
    async fn execute_with_retry(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
        max_retries: u32,
    ) -> Result<ExecutionResult> {
        let mut last_error = None;
        
        for attempt in 1..=max_retries {
            info!("🔄 Kubernetes execution attempt {} of {}", attempt, max_retries);
            
            match self.execute_single_attempt(step, workspace, env).await {
                Ok(result) => {
                    if result.is_success() {
                        info!("✅ Kubernetes execution succeeded on attempt {}", attempt);
                        return Ok(result);
                    } else {
                        warn!("⚠️ Kubernetes execution failed on attempt {} (exit_code: {})", attempt, result.exit_code);
                        last_error = Some(AppError::KubernetesError(
                            format!("Job failed with exit code {}: {}", result.exit_code, result.stderr)
                        ));
                    }
                }
                Err(e) => {
                    error!("❌ Kubernetes execution error on attempt {}: {}", attempt, e);
                    last_error = Some(e);
                }
            }

            if attempt < max_retries {
                let delay = std::time::Duration::from_secs(2_u64.pow(attempt - 1)); // Exponential backoff
                info!("⏳ Waiting {:?} before retry", delay);
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::KubernetesError(
            "All retry attempts failed".to_string()
        )))
    }

    /// Execute a single attempt with enhanced lifecycle hooks and PVC support
    async fn execute_single_attempt(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        info!("☸️ Executing Kubernetes step: {}", step.name);

        // Create Kubernetes configuration with enhanced features
        let mut k8s_config = self.create_kubernetes_config(step);
        
        // Add default lifecycle hooks if none are configured
        if k8s_config.pre_hooks.is_empty() && k8s_config.post_hooks.is_empty() {
            k8s_config.pre_hooks.push(LifecycleHookManager::create_execution_tracking_hook(
                crate::ci::connectors::traits::LifecycleHookType::PreExecution
            ));
            k8s_config.post_hooks.push(LifecycleHookManager::create_execution_tracking_hook(
                crate::ci::connectors::traits::LifecycleHookType::PostExecution
            ));
            k8s_config.post_hooks.push(LifecycleHookManager::create_metrics_hook());
            k8s_config.post_hooks.push(LifecycleHookManager::create_failure_tracking_hook());
        }

        debug!("🔧 Kubernetes config: namespace={}, timeout={}s, use_pvc={}, hooks={}", 
               k8s_config.namespace, k8s_config.timeout_seconds, k8s_config.use_pvc,
               k8s_config.pre_hooks.len() + k8s_config.post_hooks.len());

        // Validate configuration and cluster connectivity
        KubernetesValidator::validate_kubernetes_config(&k8s_config).await?;

        // Create job manager with enhanced features
        let job_manager = KubernetesJobManager::new(k8s_config.clone());
        
        // Validate resource permissions and quotas
        job_manager.validate_resource_permissions().await?;

        // Ensure PVC exists if PVC mode is enabled
        job_manager.ensure_pvc(&workspace.id).await?;

        // Generate Job YAML with enhanced features
        let job_yaml = KubernetesYamlGenerator::generate_job_yaml(step, workspace, env, &k8s_config)?;
        
        // Validate generated YAML
        KubernetesYamlGenerator::validate_yaml_syntax(&job_yaml)?;

        // Execute job with lifecycle hooks (if database manager is available)
        // For now, we'll execute without lifecycle hooks since we don't have DB manager in this context
        // In a full implementation, the DB manager would be injected via dependency injection
        let job_name = job_manager.submit_job(&job_yaml).await?;
        info!("🚀 Kubernetes job submitted: {}", job_name);

        // Wait for completion
        let result = job_manager.wait_for_completion(&job_name).await?;

        // Collect additional metrics including PVC and resource usage
        let mut metrics = job_manager.get_job_metrics(&job_name).await.unwrap_or_default();
        
        // Add configuration metadata
        metrics.insert("use_pvc".to_string(), k8s_config.use_pvc.to_string());
        metrics.insert("storage_size".to_string(), k8s_config.storage_size.unwrap_or_else(|| "N/A".to_string()));
        if let Some(storage_class) = &k8s_config.storage_class {
            metrics.insert("storage_class".to_string(), storage_class.clone());
        }
        if let Some(service_account) = &k8s_config.service_account {
            metrics.insert("service_account".to_string(), service_account.clone());
        }

        let mut final_result = result;
        for (key, value) in metrics {
            final_result = final_result.with_metadata(key, value);
        }

        if final_result.is_success() {
            info!("✅ Kubernetes step '{}' completed successfully", step.name);
        } else {
            error!("❌ Kubernetes step '{}' failed", step.name);
        }

        Ok(final_result)
    }

    /// Handle emergency cleanup for stuck jobs
    async fn emergency_cleanup(&self, step: &Step) -> Result<()> {
        warn!("🚨 Performing emergency cleanup for step: {}", step.name);
        
        let k8s_config = self.create_kubernetes_config(step);
        let _job_manager = KubernetesJobManager::new(k8s_config);
        
        // Try to find and cleanup any jobs that might be stuck
        // This is a best-effort cleanup
        let job_pattern = format!("ci-{}", step.name.to_lowercase().replace(' ', "-"));
        
        // Note: In a real implementation, you might want to list jobs and filter by labels
        // For now, we'll just log the cleanup attempt
        warn!("🧹 Emergency cleanup pattern: {}", job_pattern);
        
        Ok(())
    }
}

impl Default for KubernetesConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for KubernetesConnector {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        info!("☸️ Executing Kubernetes step: {}", step.name);

        // Determine retry count (default to 1, no retries)
        let max_retries = 1; // Could be configurable in the future

        // Execute with retry logic
        let result = self.execute_with_retry(step, workspace, env, max_retries).await;

        // Handle cleanup on failure
        if let Err(ref e) = result {
            error!("❌ Kubernetes step failed: {}", e);
            if let Err(cleanup_err) = self.emergency_cleanup(step).await {
                warn!("⚠️ Emergency cleanup also failed: {}", cleanup_err);
            }
        }

        result
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Kubernetes
    }

    fn name(&self) -> &str {
        "kubernetes"
    }

    fn validate_config(&self, step: &Step) -> Result<()> {
        debug!("🔍 Validating Kubernetes step config: {}", step.name);

        // Use the validator for comprehensive validation
        KubernetesValidator::validate_step_config(step)?;

        debug!("✅ Kubernetes step config validation passed");
        Ok(())
    }

    async fn pre_execute(&self, step: &Step) -> Result<()> {
        debug!("🚀 Kubernetes pre-execution hook for step: {}", step.name);
        
        // Validate kubectl availability
        KubernetesValidator::validate_kubectl_available().await?;
        
        // Validate cluster connectivity
        KubernetesValidator::validate_cluster_connectivity().await?;
        
        // Create and validate Kubernetes configuration
        let k8s_config = self.create_kubernetes_config(step);
        
        // Validate namespace
        KubernetesValidator::validate_namespace(&k8s_config.namespace).await?;
        
        info!("☸️ Kubernetes connector ready for step: {}", step.name);
        Ok(())
    }

    async fn post_execute(&self, step: &Step, result: &ExecutionResult) -> Result<()> {
        debug!("🏁 Kubernetes post-execution hook for step: {} (exit_code: {})", 
               step.name, result.exit_code);
        
        // Log execution summary with metrics
        if result.is_success() {
            info!("✅ Kubernetes step '{}' completed successfully", step.name);
            
            // Log useful metrics if available
            if let Some(duration) = result.metadata.get("duration_seconds") {
                info!("⏱️ Execution duration: {}s", duration);
            }
            if let Some(job_name) = result.metadata.get("job_name") {
                debug!("📋 Job name: {}", job_name);
            }
        } else {
            warn!("⚠️ Kubernetes step '{}' failed with exit code: {}", step.name, result.exit_code);
            
            // Log failure details
            if let Some(failure_reason) = result.metadata.get("failure_reason") {
                error!("💥 Failure reason: {}", failure_reason);
            }
        }

        // Additional cleanup if needed
        if let Some(job_name) = result.metadata.get("job_name") {
            debug!("🧹 Job {} should have been cleaned up automatically", job_name);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{StepConfig, StepType};
    use std::path::PathBuf;

    fn create_test_step() -> Step {
        Step {
            name: "test-kubernetes-step".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig {
                image: Some("ubuntu:latest".to_string()),
                command: Some("echo 'Hello from Kubernetes'".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(60),
        }
    }

    fn create_test_workspace() -> Workspace {
        use uuid::Uuid;
        Workspace {
            id: Uuid::new_v4(),
            path: PathBuf::from("/tmp/test"),
            execution_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn test_connector_creation() {
        let connector = KubernetesConnector::new();
        assert_eq!(connector.name(), "kubernetes");
        assert_eq!(connector.connector_type(), ConnectorType::Kubernetes);
    }

    #[test]
    fn test_config_validation() {
        let connector = KubernetesConnector::new();
        let step = create_test_step();
        
        // This should pass basic validation
        let result = connector.validate_config(&step);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation_missing_image() {
        let connector = KubernetesConnector::new();
        let mut step = create_test_step();
        step.config.image = None;
        
        // This should fail validation
        let result = connector.validate_config(&step);
        assert!(result.is_err());
    }

    #[test]
    fn test_kubernetes_config_creation() {
        let connector = KubernetesConnector::new();
        let step = create_test_step();
        
        let config = connector.create_kubernetes_config(&step);
        assert_eq!(config.namespace, "default");
        assert_eq!(config.timeout_seconds, 60);
    }
}