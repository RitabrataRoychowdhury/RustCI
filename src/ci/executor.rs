use crate::ci::{
    config::{Stage, Step, StepType},
    pipeline::{PipelineExecution, ExecutionStatus, LogLevel},
    workspace::Workspace,
    connectors::{ConnectorManager, ConnectorType},
};
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use tracing::{info, error, debug, warn};

#[allow(dead_code)] // Will be used when CI engine is fully implemented
#[derive(Clone)]
pub struct PipelineExecutor {
    pub connector_manager: Arc<ConnectorManager>,
}

impl PipelineExecutor {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new(connector_manager: Arc<ConnectorManager>) -> Self {
        Self {
            connector_manager,
        }
    }

    pub async fn execute_stage(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        stage: &Stage,
        workspace: &Workspace,
        global_env: &HashMap<String, String>,
    ) -> Result<()> {
        info!("🔄 Executing stage: {}", stage.name);

        // Merge environment variables
        let mut env = global_env.clone();
        if let Some(stage_env) = &stage.environment {
            env.extend(stage_env.clone());
        }

        // Execute steps
        if stage.parallel.unwrap_or(false) {
            self.execute_steps_parallel(execution, stage, workspace, &env).await
        } else {
            self.execute_steps_sequential(execution, stage, workspace, &env).await
        }
    }

    async fn execute_steps_sequential(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        stage: &Stage,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<()> {
        for step in &stage.steps {
            self.execute_step(execution.clone(), &stage.name, step, workspace, env).await?;
        }
        Ok(())
    }

    async fn execute_steps_parallel(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        stage: &Stage,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<()> {
        let mut handles = Vec::new();

        for step in &stage.steps {
            let execution_clone = execution.clone();
            let stage_name = stage.name.clone();
            let step_clone = step.clone();
            let workspace_clone = workspace.clone();
            let env_clone = env.clone();
            let executor_clone = self.clone();

            let handle = tokio::spawn(async move {
                executor_clone.execute_step(execution_clone, &stage_name, &step_clone, &workspace_clone, &env_clone).await
            });

            handles.push(handle);
        }

        // Wait for all steps to complete
        let mut overall_success = true;
        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => {
                    debug!("✅ Parallel step completed successfully");
                }
                Ok(Err(e)) => {
                    error!("❌ Parallel step failed: {}", e);
                    overall_success = false;
                }
                Err(e) => {
                    error!("❌ Parallel step panicked: {}", e);
                    overall_success = false;
                }
            }
        }

        if overall_success {
            Ok(())
        } else {
            Err(AppError::InternalServerError("One or more parallel steps failed".to_string()))
        }
    }

    async fn execute_step(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        stage_name: &str,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<()> {
        info!("🔄 Executing step: {} in stage: {}", step.name, stage_name);

        // Merge step environment
        let mut step_env = env.clone();
        if let Some(step_env_config) = &step.config.environment {
            step_env.extend(step_env_config.clone());
        }

        // Update step status to running
        {
            let mut exec = execution.write().await;
            if let Some(step_exec) = exec.get_step_mut(stage_name, &step.name) {
                step_exec.start(self.get_step_command(step));
            }
            exec.add_log(
                LogLevel::Info,
                format!("Starting step: {}", step.name),
                Some(stage_name.to_string()),
                Some(step.name.clone()),
            );
        }

        // Execute step based on type
        let result = match &step.step_type {
            StepType::Shell => self.execute_shell_step(step, workspace, &step_env).await,
            StepType::Docker => self.execute_docker_step(step, workspace, &step_env).await,
            StepType::Kubernetes => self.execute_kubernetes_step(step, workspace, &step_env).await,
            StepType::AWS => self.execute_aws_step(step, workspace, &step_env).await,
            StepType::Azure => self.execute_azure_step(step, workspace, &step_env).await,
            StepType::GCP => self.execute_gcp_step(step, workspace, &step_env).await,
            StepType::GitHub => self.execute_github_step(step, workspace, &step_env).await,
            StepType::GitLab => self.execute_gitlab_step(step, workspace, &step_env).await,
            StepType::Custom => self.execute_custom_step(step, workspace, &step_env).await,
        };

        // Update step status
        let (status, exit_code, stdout, stderr) = match result {
            Ok((code, out, err)) => {
                info!("✅ Step completed successfully: {}", step.name);
                (ExecutionStatus::Success, Some(code), Some(out), Some(err))
            }
            Err(e) => {
                error!("❌ Step failed: {} - {}", step.name, e);
                if step.continue_on_error.unwrap_or(false) {
                    warn!("⚠️ Continuing despite step failure due to continue_on_error flag");
                    (ExecutionStatus::Success, Some(1), None, Some(e.to_string()))
                } else {
                    (ExecutionStatus::Failed, Some(1), None, Some(e.to_string()))
                }
            }
        };

        {
            let mut exec = execution.write().await;
            if let Some(step_exec) = exec.get_step_mut(stage_name, &step.name) {
                step_exec.finish(status.clone(), exit_code, stdout.clone(), stderr.clone());
            }
            exec.add_log(
                if matches!(status, ExecutionStatus::Success) { LogLevel::Info } else { LogLevel::Error },
                format!("Step finished: {} with status: {:?}", step.name, status),
                Some(stage_name.to_string()),
                Some(step.name.clone()),
            );
        }

        if matches!(status, ExecutionStatus::Failed) && !step.continue_on_error.unwrap_or(false) {
            return Err(AppError::InternalServerError(format!("Step failed: {}", step.name)));
        }

        Ok(())
    }

    fn get_step_command(&self, step: &Step) -> Option<String> {
        match &step.config.command {
            Some(cmd) => Some(cmd.clone()),
            None => match &step.config.script {
                Some(script) => Some(script.clone()),
                None => None,
            }
        }
    }

    async fn execute_shell_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let command = step.config.command.as_ref()
            .or(step.config.script.as_ref())
            .ok_or_else(|| AppError::ValidationError("Shell step requires command or script".to_string()))?;

        let working_dir = step.config.working_directory.as_ref()
            .map(|d| workspace.path.join(d))
            .unwrap_or_else(|| workspace.path.clone());

        debug!("🐚 Executing shell command: {} in directory: {:?}", command, working_dir);

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(&working_dir);
        
        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let output = cmd.output().await
            .map_err(|e| AppError::InternalServerError(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        if !output.status.success() {
            return Err(AppError::InternalServerError(format!("Command failed with exit code {}: {}", exit_code, stderr)));
        }

        Ok((exit_code, stdout, stderr))
    }

    async fn execute_docker_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::Docker).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_kubernetes_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::Kubernetes).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_aws_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::AWS).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_azure_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::Azure).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_gcp_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::GCP).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_github_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::GitHub).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_gitlab_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let connector = self.connector_manager.get_connector(ConnectorType::GitLab).await?;
        connector.execute_step(step, workspace, env).await
    }

    async fn execute_custom_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let plugin_name = step.config.plugin_name.as_ref()
            .ok_or_else(|| AppError::ValidationError("Custom step requires plugin_name".to_string()))?;

        let connector = self.connector_manager.get_custom_connector(plugin_name).await?;
        connector.execute_step(step, workspace, env).await
    }
}