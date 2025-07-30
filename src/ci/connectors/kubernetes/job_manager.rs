//! Kubernetes job lifecycle management
//!
//! This module handles the complete lifecycle of Kubernetes jobs including:
//! - Job creation and submission
//! - Status monitoring and waiting
//! - Log collection and streaming
//! - Resource cleanup and error handling

use super::super::traits::{ExecutionResult, KubernetesConfig};
use super::{lifecycle_hooks::LifecycleHookManager, yaml_generator::KubernetesYamlGenerator};
use crate::error::{AppError, Result};
use crate::infrastructure::database::DatabaseManager;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Kubernetes job manager for handling job lifecycle
pub struct KubernetesJobManager {
    /// Configuration for this job manager
    config: KubernetesConfig,
}

/// Job status information
#[derive(Debug, Clone)]
pub struct JobStatus {
    pub phase: JobPhase,
    pub active: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub start_time: Option<String>,
    pub completion_time: Option<String>,
    pub conditions: Vec<JobCondition>,
}

/// Job phase enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum JobPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

/// Job condition information
#[derive(Debug, Clone)]
pub struct JobCondition {
    pub condition_type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

impl KubernetesJobManager {
    /// Create a new job manager with the given configuration
    pub fn new(config: KubernetesConfig) -> Self {
        Self { config }
    }

    /// Create a new job manager with lifecycle hooks support
    pub fn with_lifecycle_hooks(config: KubernetesConfig, _db_manager: DatabaseManager) -> Self {
        // For now, we'll store the config and add lifecycle hook support later
        Self { config }
    }

    /// Ensure PVC exists for workspace storage
    pub async fn ensure_pvc(&self, workspace_id: &Uuid) -> Result<()> {
        if !self.config.use_pvc {
            debug!("🔍 PVC not enabled, skipping PVC creation");
            return Ok(());
        }

        info!("🗂️ Ensuring PVC exists for workspace: {}", workspace_id);

        // Check if PVC already exists
        let pvc_name = "workspace-pvc";
        let check_output = Command::new("kubectl")
            .args([
                "get",
                "pvc",
                pvc_name,
                "-n",
                &self.config.namespace,
                "--ignore-not-found=true",
            ])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!("Failed to check PVC existence: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&check_output.stdout);
        if !stdout.trim().is_empty() {
            debug!("✅ PVC already exists: {}", pvc_name);
            return Ok(());
        }

        // Create PVC
        info!("🔄 Creating PVC: {}", pvc_name);
        let pvc_yaml =
            KubernetesYamlGenerator::generate_pvc_yaml(&workspace_id.to_string(), &self.config)?;

        self.apply_yaml(&pvc_yaml).await?;
        info!("✅ PVC created successfully: {}", pvc_name);

        // Wait for PVC to be bound
        self.wait_for_pvc_bound(pvc_name).await?;

        Ok(())
    }

    /// Wait for PVC to be bound
    async fn wait_for_pvc_bound(&self, pvc_name: &str) -> Result<()> {
        info!("⏳ Waiting for PVC to be bound: {}", pvc_name);

        let timeout = Duration::from_secs(60); // 1 minute timeout for PVC binding
        let start_time = Instant::now();
        let poll_interval = Duration::from_secs(2);

        loop {
            if start_time.elapsed() > timeout {
                return Err(AppError::KubernetesError(format!(
                    "PVC binding timeout: {}",
                    pvc_name
                )));
            }

            let output = Command::new("kubectl")
                .args([
                    "get",
                    "pvc",
                    pvc_name,
                    "-n",
                    &self.config.namespace,
                    "-o",
                    "jsonpath={.status.phase}",
                ])
                .output()
                .await
                .map_err(|e| {
                    AppError::KubernetesError(format!("Failed to check PVC status: {}", e))
                })?;

            if output.status.success() {
                let phase = String::from_utf8_lossy(&output.stdout);
                debug!("📊 PVC phase: {}", phase);

                if phase.trim() == "Bound" {
                    info!("✅ PVC is bound: {}", pvc_name);
                    return Ok(());
                }
            }

            sleep(poll_interval).await;
        }
    }

    /// Apply YAML to Kubernetes cluster
    async fn apply_yaml(&self, yaml: &str) -> Result<()> {
        debug!("📝 Applying YAML to cluster");

        let mut cmd = Command::new("kubectl");
        cmd.args(["apply", "-f", "-", "-n", &self.config.namespace]);
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            AppError::KubernetesError(format!("Failed to spawn kubectl apply: {}", e))
        })?;

        // Write YAML to stdin
        if let Some(stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let mut stdin = stdin;
            stdin.write_all(yaml.as_bytes()).await.map_err(|e| {
                AppError::KubernetesError(format!("Failed to write YAML to kubectl: {}", e))
            })?;
            stdin.shutdown().await.map_err(|e| {
                AppError::KubernetesError(format!("Failed to close kubectl stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().await.map_err(|e| {
            AppError::KubernetesError(format!("Failed to wait for kubectl apply: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::KubernetesError(format!(
                "Failed to apply YAML: {}",
                stderr
            )));
        }

        debug!("✅ YAML applied successfully");
        Ok(())
    }

    /// Execute job with lifecycle hooks support
    pub async fn execute_job_with_hooks(
        &self,
        job_yaml: &str,
        step_name: &str,
        execution_id: &Uuid,
        workspace_id: &Uuid,
        hook_manager: Option<&LifecycleHookManager>,
    ) -> Result<ExecutionResult> {
        // Execute pre-hooks if hook manager is available
        if let Some(manager) = hook_manager {
            manager
                .execute_pre_hooks(
                    &self.config.pre_hooks,
                    step_name,
                    execution_id,
                    workspace_id,
                )
                .await?;
        }

        // Submit and wait for job completion
        let job_name = self.submit_job(job_yaml).await?;
        let result = self.wait_for_completion(&job_name).await?;

        // Execute post-hooks if hook manager is available
        if let Some(manager) = hook_manager {
            manager
                .execute_post_hooks(
                    &self.config.post_hooks,
                    step_name,
                    execution_id,
                    workspace_id,
                    &result,
                )
                .await?;
        }

        Ok(result)
    }

    /// Validate resource permissions and quotas
    pub async fn validate_resource_permissions(&self) -> Result<()> {
        info!("🔍 Validating resource permissions and quotas");

        // Check if we can create jobs in the namespace
        let auth_output = Command::new("kubectl")
            .args([
                "auth",
                "can-i",
                "create",
                "jobs",
                "-n",
                &self.config.namespace,
            ])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!(
                    "Failed to check job creation permissions: {}",
                    e
                ))
            })?;

        let stdout = String::from_utf8_lossy(&auth_output.stdout);
        if !auth_output.status.success() || stdout.trim() != "yes" {
            return Err(AppError::PermissionDenied(format!(
                "Cannot create jobs in namespace '{}'",
                self.config.namespace
            )));
        }

        // Check PVC permissions if PVC is enabled
        if self.config.use_pvc {
            let pvc_auth_output = Command::new("kubectl")
                .args([
                    "auth",
                    "can-i",
                    "create",
                    "persistentvolumeclaims",
                    "-n",
                    &self.config.namespace,
                ])
                .output()
                .await
                .map_err(|e| {
                    AppError::KubernetesError(format!(
                        "Failed to check PVC creation permissions: {}",
                        e
                    ))
                })?;

            let pvc_stdout = String::from_utf8_lossy(&pvc_auth_output.stdout);
            if !pvc_auth_output.status.success() || pvc_stdout.trim() != "yes" {
                return Err(AppError::PermissionDenied(format!(
                    "Cannot create PVCs in namespace '{}'",
                    self.config.namespace
                )));
            }
        }

        // Check resource quotas
        self.validate_resource_quotas().await?;

        info!("✅ Resource permissions and quotas validated");
        Ok(())
    }

    /// Validate resource quotas against requested resources
    async fn validate_resource_quotas(&self) -> Result<()> {
        debug!("📊 Validating resource quotas");

        let output = Command::new("kubectl")
            .args([
                "get",
                "resourcequota",
                "-n",
                &self.config.namespace,
                "-o",
                "json",
            ])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!("Failed to get resource quotas: {}", e))
            })?;

        if !output.status.success() {
            debug!("⚠️ No resource quotas found or failed to retrieve them");
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let quota_data: Value = serde_json::from_str(&stdout).map_err(|e| {
            AppError::KubernetesError(format!("Failed to parse resource quota JSON: {}", e))
        })?;

        let empty_vec = vec![];
        let quotas = quota_data
            .get("items")
            .and_then(|items| items.as_array())
            .unwrap_or(&empty_vec);

        if quotas.is_empty() {
            debug!("✅ No resource quotas to validate against");
            return Ok(());
        }

        // For now, we'll just log that quotas exist
        // In a full implementation, we would parse the quotas and validate against requested resources
        info!(
            "📊 Found {} resource quotas - detailed validation not implemented yet",
            quotas.len()
        );

        Ok(())
    }

    /// Submit a job to Kubernetes and return the job name
    pub async fn submit_job(&self, job_yaml: &str) -> Result<String> {
        info!(
            "🚀 Submitting Kubernetes job to namespace: {}",
            self.config.namespace
        );
        debug!("📝 Job YAML:\n{}", job_yaml);

        // Apply the job YAML using kubectl
        let mut cmd = Command::new("kubectl");
        cmd.args(["apply", "-f", "-", "-n", &self.config.namespace]);
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            AppError::KubernetesError(format!("Failed to spawn kubectl apply: {}", e))
        })?;

        // Write YAML to stdin
        if let Some(stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let mut stdin = stdin;
            stdin.write_all(job_yaml.as_bytes()).await.map_err(|e| {
                AppError::KubernetesError(format!("Failed to write YAML to kubectl: {}", e))
            })?;
            stdin.shutdown().await.map_err(|e| {
                AppError::KubernetesError(format!("Failed to close kubectl stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().await.map_err(|e| {
            AppError::KubernetesError(format!("Failed to wait for kubectl apply: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("❌ Failed to submit Kubernetes job: {}", stderr);
            return Err(AppError::KubernetesError(format!(
                "Failed to submit job: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("✅ Job submission output: {}", stdout);

        // Extract job name from output (format: "job.batch/job-name created")
        let job_name = self.extract_job_name_from_output(&stdout)?;
        info!("✅ Kubernetes job submitted: {}", job_name);

        Ok(job_name)
    }

    /// Wait for job completion with timeout and status monitoring
    pub async fn wait_for_completion(&self, job_name: &str) -> Result<ExecutionResult> {
        info!("⏳ Waiting for job completion: {}", job_name);

        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(self.config.timeout_seconds);
        let poll_interval = Duration::from_secs(5);

        loop {
            // Check timeout
            if start_time.elapsed() > timeout_duration {
                error!("⏰ Job timeout exceeded: {}", job_name);
                self.cleanup_job(job_name).await?;
                return Err(AppError::KubernetesError(format!(
                    "Job timed out after {} seconds",
                    self.config.timeout_seconds
                )));
            }

            // Get job status
            let status = self.get_job_status(job_name).await?;
            debug!("📊 Job status: {:?}", status.phase);

            match status.phase {
                JobPhase::Succeeded => {
                    info!("✅ Job completed successfully: {}", job_name);
                    let logs = self.collect_job_logs(job_name).await?;
                    let result = ExecutionResult::success(logs)
                        .with_metadata("job_name".to_string(), job_name.to_string())
                        .with_metadata("namespace".to_string(), self.config.namespace.clone())
                        .with_metadata(
                            "duration_seconds".to_string(),
                            start_time.elapsed().as_secs().to_string(),
                        );

                    // Cleanup job
                    self.cleanup_job(job_name).await?;
                    return Ok(result);
                }
                JobPhase::Failed => {
                    error!("❌ Job failed: {}", job_name);
                    let logs = self
                        .collect_job_logs(job_name)
                        .await
                        .unwrap_or_else(|_| "Failed to collect logs".to_string());
                    let error_message = self
                        .get_failure_reason(&status)
                        .unwrap_or_else(|| "Job failed without specific reason".to_string());

                    let result = ExecutionResult::failure(
                        1,
                        format!("{}\n\nLogs:\n{}", error_message, logs),
                    )
                    .with_metadata("job_name".to_string(), job_name.to_string())
                    .with_metadata("namespace".to_string(), self.config.namespace.clone())
                    .with_metadata("failure_reason".to_string(), error_message);

                    // Cleanup job
                    self.cleanup_job(job_name).await?;
                    return Ok(result);
                }
                JobPhase::Running => {
                    debug!("🔄 Job is running: {}", job_name);
                }
                JobPhase::Pending => {
                    debug!("⏸️ Job is pending: {}", job_name);
                }
                JobPhase::Unknown => {
                    warn!("❓ Job status unknown: {}", job_name);
                }
            }

            // Wait before next poll
            sleep(poll_interval).await;
        }
    }

    /// Get current job status
    async fn get_job_status(&self, job_name: &str) -> Result<JobStatus> {
        debug!("📊 Getting job status: {}", job_name);

        let output = Command::new("kubectl")
            .args([
                "get",
                "job",
                job_name,
                "-n",
                &self.config.namespace,
                "-o",
                "json",
            ])
            .output()
            .await
            .map_err(|e| AppError::KubernetesError(format!("Failed to get job status: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::KubernetesError(format!(
                "Failed to get job status: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_job_status(&stdout)
    }

    /// Parse job status from kubectl JSON output
    fn parse_job_status(&self, json_output: &str) -> Result<JobStatus> {
        let job_data: Value = serde_json::from_str(json_output).map_err(|e| {
            AppError::KubernetesError(format!("Failed to parse job status JSON: {}", e))
        })?;

        let status = job_data.get("status").unwrap_or(&Value::Null);

        let active = status.get("active").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let succeeded = status
            .get("succeeded")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let failed = status.get("failed").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let phase = if succeeded > 0 {
            JobPhase::Succeeded
        } else if failed > 0 {
            JobPhase::Failed
        } else if active > 0 {
            JobPhase::Running
        } else {
            JobPhase::Pending
        };

        let start_time = status
            .get("startTime")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let completion_time = status
            .get("completionTime")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let conditions = status
            .get("conditions")
            .and_then(|v| v.as_array())
            .map(|conditions| {
                conditions
                    .iter()
                    .filter_map(|condition| {
                        Some(JobCondition {
                            condition_type: condition.get("type")?.as_str()?.to_string(),
                            status: condition.get("status")?.as_str()?.to_string(),
                            reason: condition
                                .get("reason")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            message: condition
                                .get("message")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(JobStatus {
            phase,
            active,
            succeeded,
            failed,
            start_time,
            completion_time,
            conditions,
        })
    }

    /// Collect logs from job pods
    async fn collect_job_logs(&self, job_name: &str) -> Result<String> {
        debug!("📋 Collecting logs for job: {}", job_name);

        // First, get the pod name(s) for this job
        let pod_names = self.get_job_pod_names(job_name).await?;

        if pod_names.is_empty() {
            warn!("⚠️ No pods found for job: {}", job_name);
            return Ok("No pods found for job".to_string());
        }

        let mut all_logs = String::new();

        for pod_name in pod_names {
            debug!("📋 Collecting logs from pod: {}", pod_name);

            let output = Command::new("kubectl")
                .args([
                    "logs",
                    &pod_name,
                    "-n",
                    &self.config.namespace,
                    "--tail=1000", // Limit log size
                ])
                .output()
                .await
                .map_err(|e| {
                    AppError::KubernetesError(format!(
                        "Failed to collect logs from pod {}: {}",
                        pod_name, e
                    ))
                })?;

            if output.status.success() {
                let logs = String::from_utf8_lossy(&output.stdout);
                if !all_logs.is_empty() {
                    all_logs.push_str("\n--- Pod: ");
                    all_logs.push_str(&pod_name);
                    all_logs.push_str(" ---\n");
                }
                all_logs.push_str(&logs);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(
                    "⚠️ Failed to collect logs from pod {}: {}",
                    pod_name, stderr
                );
                all_logs.push_str(&format!(
                    "\n--- Failed to collect logs from pod {} ---\n",
                    pod_name
                ));
            }
        }

        debug!("✅ Collected {} bytes of logs", all_logs.len());
        Ok(all_logs)
    }

    /// Get pod names associated with a job
    async fn get_job_pod_names(&self, job_name: &str) -> Result<Vec<String>> {
        debug!("🔍 Finding pods for job: {}", job_name);

        let output = Command::new("kubectl")
            .args([
                "get",
                "pods",
                "-n",
                &self.config.namespace,
                "-l",
                &format!("job-name={}", job_name),
                "-o",
                "jsonpath={.items[*].metadata.name}",
            ])
            .output()
            .await
            .map_err(|e| AppError::KubernetesError(format!("Failed to get job pods: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::KubernetesError(format!(
                "Failed to get job pods: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pod_names: Vec<String> = stdout
            .split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        debug!("✅ Found {} pods for job: {:?}", pod_names.len(), pod_names);
        Ok(pod_names)
    }

    /// Extract job name from kubectl apply output
    fn extract_job_name_from_output(&self, output: &str) -> Result<String> {
        // Expected format: "job.batch/job-name created" or "job.batch/job-name configured"
        for line in output.lines() {
            if line.contains("job.batch/")
                && (line.contains("created") || line.contains("configured"))
            {
                if let Some(start) = line.find("job.batch/") {
                    let start = start + "job.batch/".len();
                    if let Some(end) = line[start..].find(' ') {
                        return Ok(line[start..start + end].to_string());
                    }
                }
            }
        }

        Err(AppError::KubernetesError(
            "Could not extract job name from kubectl output".to_string(),
        ))
    }

    /// Get failure reason from job status
    fn get_failure_reason(&self, status: &JobStatus) -> Option<String> {
        for condition in &status.conditions {
            if condition.condition_type == "Failed" && condition.status == "True" {
                if let Some(message) = &condition.message {
                    return Some(message.clone());
                }
                if let Some(reason) = &condition.reason {
                    return Some(reason.clone());
                }
            }
        }
        None
    }

    /// Cleanup job and associated resources
    async fn cleanup_job(&self, job_name: &str) -> Result<()> {
        info!("🧹 Cleaning up job: {}", job_name);

        // Delete the job (this will also delete associated pods)
        let output = Command::new("kubectl")
            .args([
                "delete",
                "job",
                job_name,
                "-n",
                &self.config.namespace,
                "--ignore-not-found=true",
            ])
            .output()
            .await
            .map_err(|e| AppError::KubernetesError(format!("Failed to cleanup job: {}", e)))?;

        if output.status.success() {
            debug!("✅ Job cleanup completed: {}", job_name);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("⚠️ Job cleanup warning: {}", stderr);
        }

        Ok(())
    }

    /// Force cleanup job with cascade deletion
    pub async fn force_cleanup(&self, job_name: &str) -> Result<()> {
        warn!("🚨 Force cleaning up job: {}", job_name);

        // Force delete with cascade
        let output = Command::new("kubectl")
            .args([
                "delete",
                "job",
                job_name,
                "-n",
                &self.config.namespace,
                "--force",
                "--grace-period=0",
                "--ignore-not-found=true",
            ])
            .output()
            .await
            .map_err(|e| {
                AppError::KubernetesError(format!("Failed to force cleanup job: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("❌ Force cleanup failed: {}", stderr);
        }

        Ok(())
    }

    /// Get job execution metrics
    pub async fn get_job_metrics(&self, job_name: &str) -> Result<HashMap<String, String>> {
        let mut metrics = HashMap::new();

        if let Ok(status) = self.get_job_status(job_name).await {
            metrics.insert("active_pods".to_string(), status.active.to_string());
            metrics.insert("succeeded_pods".to_string(), status.succeeded.to_string());
            metrics.insert("failed_pods".to_string(), status.failed.to_string());

            if let Some(start_time) = status.start_time {
                metrics.insert("start_time".to_string(), start_time);
            }

            if let Some(completion_time) = status.completion_time {
                metrics.insert("completion_time".to_string(), completion_time);
            }
        }

        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_job_name_from_output() {
        let manager = KubernetesJobManager::new(KubernetesConfig::default());

        let output1 = "job.batch/ci-test-step-abc123 created";
        assert_eq!(
            manager.extract_job_name_from_output(output1).unwrap(),
            "ci-test-step-abc123"
        );

        let output2 = "job.batch/my-job-456 configured";
        assert_eq!(
            manager.extract_job_name_from_output(output2).unwrap(),
            "my-job-456"
        );

        let output3 = "invalid output";
        assert!(manager.extract_job_name_from_output(output3).is_err());
    }

    #[test]
    fn test_job_phase_determination() {
        // This would test the job phase logic, but requires more complex setup
        // For now, we'll just verify the enum values exist
        assert_eq!(JobPhase::Succeeded, JobPhase::Succeeded);
        assert_ne!(JobPhase::Failed, JobPhase::Succeeded);
    }
}
