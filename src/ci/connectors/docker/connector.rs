//! Docker connector implementation
//!
//! This connector handles Docker-based step execution including:
//! - Container execution with custom images
//! - Docker image building from Dockerfiles
//! - Volume mounting and environment variable injection
//! - Proper cleanup and error handling

use super::super::traits::{Connector, ConnectorType, ExecutionResult};
use crate::ci::{config::Step, workspace::Workspace};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use std::collections::HashMap;

use tokio::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Docker connector for executing steps in Docker containers
pub struct DockerConnector {
    /// Unique identifier for this connector instance
    id: String,
}

impl DockerConnector {
    /// Create a new Docker connector instance
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
        }
    }

    /// Validate Docker is available on the system
    async fn validate_docker_available(&self) -> Result<()> {
        debug!("🔍 Validating Docker availability");

        let output = Command::new("docker")
            .args(["--version"])
            .output()
            .await
            .map_err(|e| {
                AppError::ConnectorConfigError(format!("Docker not found on system: {}", e))
            })?;

        if !output.status.success() {
            return Err(AppError::ConnectorConfigError(
                "Docker is not running or not accessible".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        debug!("✅ Docker available: {}", version.trim());
        Ok(())
    }

    /// Build Docker image if Dockerfile is specified
    async fn build_image_if_needed(&self, step: &Step, workspace: &Workspace) -> Result<String> {
        if let Some(dockerfile) = &step.config.dockerfile {
            let image_name = step
                .config
                .image
                .clone()
                .unwrap_or_else(|| format!("ci-step-{}", Uuid::new_v4()));

            info!(
                "🔨 Building Docker image: {} from {}",
                image_name, dockerfile
            );

            let dockerfile_path = workspace.path.join(dockerfile);
            if !dockerfile_path.exists() {
                return Err(AppError::ValidationError(format!(
                    "Dockerfile not found: {}",
                    dockerfile_path.display()
                )));
            }

            let build_context = step
                .config
                .build_context
                .as_ref()
                .map(|ctx| workspace.path.join(ctx))
                .unwrap_or_else(|| workspace.path.clone());

            let output = Command::new("docker")
                .args([
                    "build",
                    "-t",
                    &image_name,
                    "-f",
                    dockerfile_path.to_str().unwrap(),
                    build_context.to_str().unwrap(),
                ])
                .output()
                .await
                .map_err(|e| {
                    AppError::ExternalServiceError(format!("Failed to execute docker build: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("❌ Docker build failed: {}", stderr);
                return Err(AppError::ExternalServiceError(format!(
                    "Docker build failed: {}",
                    stderr
                )));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            debug!("✅ Docker build completed: {}", stdout);

            Ok(image_name)
        } else {
            // Use specified image or default
            Ok(step
                .config
                .image
                .clone()
                .unwrap_or_else(|| "ubuntu:latest".to_string()))
        }
    }

    /// Execute command in Docker container
    async fn execute_in_container(
        &self,
        image: &str,
        command: &str,
        workspace: &Workspace,
        env: &HashMap<String, String>,
        working_dir: Option<&str>,
    ) -> Result<ExecutionResult> {
        let container_name = format!("ci-exec-{}", Uuid::new_v4());

        info!(
            "🐳 Running container: {} with image: {}",
            container_name, image
        );
        debug!("📝 Command: {}", command);

        let mut docker_cmd = Command::new("docker");
        docker_cmd.args(["run", "--rm", "--name", &container_name]);

        // Mount workspace
        let workspace_mount = format!("{}:/workspace", workspace.path.display());
        docker_cmd.args(["-v", &workspace_mount]);

        // Set working directory
        let work_dir = working_dir.unwrap_or("/workspace");
        docker_cmd.args(["-w", work_dir]);

        // Add environment variables
        for (key, value) in env {
            docker_cmd.args(["-e", &format!("{}={}", key, value)]);
        }

        // Add image and command
        docker_cmd.args([image, "sh", "-c", command]);

        debug!("🚀 Executing Docker command: {:?}", docker_cmd);

        let output = docker_cmd.output().await.map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to execute docker run: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        if output.status.success() {
            info!("✅ Container execution completed successfully");
            debug!("📤 Output: {}", stdout);
        } else {
            warn!(
                "⚠️ Container execution failed with exit code: {}",
                exit_code
            );
            debug!("📤 Error: {}", stderr);
        }

        Ok(ExecutionResult::new(exit_code, stdout, stderr)
            .with_metadata("container_name".to_string(), container_name)
            .with_metadata("image".to_string(), image.to_string()))
    }

    /// Cleanup any leftover containers or images if needed
    async fn cleanup(&self, container_name: &str) -> Result<()> {
        debug!("🧹 Cleaning up container: {}", container_name);

        // Try to stop and remove container (ignore errors as it might already be gone)
        let _ = Command::new("docker")
            .args(["stop", container_name])
            .output()
            .await;

        let _ = Command::new("docker")
            .args(["rm", container_name])
            .output()
            .await;

        Ok(())
    }
}

impl Default for DockerConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Connector for DockerConnector {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        info!("🐳 Executing Docker step: {}", step.name);

        // Validate Docker is available
        self.validate_docker_available().await?;

        // Get or build the Docker image
        let image = self.build_image_if_needed(step, workspace).await?;

        // Determine the command to execute
        let command = step
            .config
            .command
            .as_ref()
            .or(step.config.script.as_ref())
            .ok_or_else(|| {
                AppError::ValidationError("Docker step requires command or script".to_string())
            })?;

        // Execute in container
        let result = self
            .execute_in_container(
                &image,
                command,
                workspace,
                env,
                step.config.working_directory.as_deref(),
            )
            .await?;

        if result.is_success() {
            info!("✅ Docker step '{}' completed successfully", step.name);
        } else {
            error!(
                "❌ Docker step '{}' failed with exit code: {}",
                step.name, result.exit_code
            );
        }

        Ok(result)
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Docker
    }

    fn name(&self) -> &str {
        "docker"
    }

    fn validate_config(&self, step: &Step) -> Result<()> {
        debug!("🔍 Validating Docker step config: {}", step.name);

        // Call parent validation
        if step.name.is_empty() {
            return Err(AppError::ValidationError(
                "Step name cannot be empty".to_string(),
            ));
        }

        // Validate Docker-specific configuration
        if step.config.command.is_none() && step.config.script.is_none() {
            return Err(AppError::ValidationError(
                "Docker step requires either 'command' or 'script'".to_string(),
            ));
        }

        // If dockerfile is specified, validate it exists (will be checked during execution)
        if let Some(dockerfile) = &step.config.dockerfile {
            if dockerfile.is_empty() {
                return Err(AppError::ValidationError(
                    "Dockerfile path cannot be empty".to_string(),
                ));
            }
        }

        // Validate image name format if provided
        if let Some(image) = &step.config.image {
            if image.is_empty() {
                return Err(AppError::ValidationError(
                    "Docker image name cannot be empty".to_string(),
                ));
            }

            // Basic image name validation (could be more comprehensive)
            if image.contains(' ') {
                return Err(AppError::ValidationError(
                    "Docker image name cannot contain spaces".to_string(),
                ));
            }
        }

        debug!("✅ Docker step config validation passed");
        Ok(())
    }

    async fn pre_execute(&self, step: &Step) -> Result<()> {
        debug!("🚀 Docker pre-execution hook for step: {}", step.name);

        // Validate Docker is available before execution
        self.validate_docker_available().await?;

        info!("🐳 Docker connector ready for step: {}", step.name);
        Ok(())
    }

    async fn post_execute(&self, step: &Step, result: &ExecutionResult) -> Result<()> {
        debug!(
            "🏁 Docker post-execution hook for step: {} (exit_code: {})",
            step.name, result.exit_code
        );

        // Log execution summary
        if result.is_success() {
            info!("✅ Docker step '{}' completed successfully", step.name);
        } else {
            warn!(
                "⚠️ Docker step '{}' failed with exit code: {}",
                step.name, result.exit_code
            );
        }

        // Cleanup if container name is in metadata
        if let Some(container_name) = result.metadata.get("container_name") {
            if let Err(e) = self.cleanup(container_name).await {
                warn!("⚠️ Failed to cleanup container {}: {}", container_name, e);
            }
        }

        Ok(())
    }
}
