use crate::ci::{
    builder::{BuildConfig, ProjectBuilder},
    config::{Stage, Step, StepType},
    connectors::ConnectorManager,
    deployment::{DeploymentConfig, DeploymentType, LocalDeploymentManager},
    pipeline::{ExecutionStatus, LogLevel, PipelineExecution},
    repository::{RepositoryConfig, RepositoryManager},
    workspace::Workspace,
};
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[allow(dead_code)] // Will be used when CI engine is fully implemented
#[derive(Clone, Debug)]
pub struct PipelineExecutor {
    pub connector_manager: Arc<tokio::sync::Mutex<ConnectorManager>>,
    pub repository_manager: Arc<RepositoryManager>,
    pub project_builder: Arc<ProjectBuilder>,
    pub deployment_manager: Arc<tokio::sync::Mutex<LocalDeploymentManager>>,
}

impl PipelineExecutor {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new(
        connector_manager: ConnectorManager,
        cache_directory: std::path::PathBuf,
        deployment_directory: std::path::PathBuf,
    ) -> Self {
        Self {
            connector_manager: Arc::new(tokio::sync::Mutex::new(connector_manager)),
            repository_manager: Arc::new(RepositoryManager::new()),
            project_builder: Arc::new(ProjectBuilder::new(cache_directory)),
            deployment_manager: Arc::new(tokio::sync::Mutex::new(LocalDeploymentManager::new(
                deployment_directory,
            ))),
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
            self.execute_steps_parallel(execution, stage, workspace, &env)
                .await
        } else {
            self.execute_steps_sequential(execution, stage, workspace, &env)
                .await
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
            self.execute_step(execution.clone(), &stage.name, step, workspace, env)
                .await?;
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
                executor_clone
                    .execute_step(
                        execution_clone,
                        &stage_name,
                        &step_clone,
                        &workspace_clone,
                        &env_clone,
                    )
                    .await
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
            Err(AppError::InternalServerError(
                "One or more parallel steps failed".to_string(),
            ))
        }
    }

    pub async fn execute_step(
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

        // Handle special step types for CI/CD operations based on step type, not just name
        let result = match &step.step_type {
            StepType::Shell => {
                // Check if deployment detection is explicitly disabled
                if step.config.disable_deployment_detection.unwrap_or(false) {
                    // Skip all special handling and just execute as shell
                    self.execute_shell_step(step, workspace, &step_env).await
                } else {
                    // For shell steps, prioritize actual command/script over name-based detection
                    if step.config.command.is_some() || step.config.script.is_some() {
                        // If step has a command or script, execute it as shell regardless of name
                        info!("🔧 EXECUTOR DEBUG - Shell step has command/script, executing as shell step");
                        self.execute_shell_step(step, workspace, &step_env).await
                    } else {
                        // Only use name-based detection if no command/script is provided
                        match step.name.as_str() {
                            name if name.starts_with("clone") || name.starts_with("checkout") => {
                                info!("🔧 EXECUTOR DEBUG - Detected repository step by name: {}", name);
                                self.execute_repository_step(step, workspace, &step_env)
                                    .await
                            }
                            name if name.starts_with("build") || name.starts_with("compile") => {
                                info!("🔧 EXECUTOR DEBUG - Detected build step by name: {}", name);
                                self.execute_build_step(step, workspace, &step_env).await
                            }
                            _ => {
                                // Regular shell step - don't trigger deployment logic
                                info!("🔧 EXECUTOR DEBUG - Executing as regular shell step");
                                self.execute_shell_step(step, workspace, &step_env).await
                            }
                        }
                    }
                }
            }
            StepType::Custom => {
                // Only trigger deployment for custom steps that explicitly request it
                match step.name.as_str() {
                    name if name.starts_with("deploy") || name.starts_with("release") => {
                        self.execute_deployment_step(execution.clone(), step, workspace, &step_env)
                            .await
                    }
                    _ => self.execute_custom_step(step, workspace, &step_env).await,
                }
            }
            StepType::Docker => self.execute_docker_step(step, workspace, &step_env).await,
            StepType::Kubernetes => {
                self.execute_kubernetes_step(step, workspace, &step_env)
                    .await
            }
            StepType::AWS => self.execute_aws_step(step, workspace, &step_env).await,
            StepType::Azure => self.execute_azure_step(step, workspace, &step_env).await,
            StepType::GCP => self.execute_gcp_step(step, workspace, &step_env).await,
            StepType::GitHub => self.execute_github_step(step, workspace, &step_env).await,
            StepType::GitLab => self.execute_gitlab_step(step, workspace, &step_env).await,
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
                if matches!(status, ExecutionStatus::Success) {
                    LogLevel::Info
                } else {
                    LogLevel::Error
                },
                format!("Step finished: {} with status: {:?}", step.name, status),
                Some(stage_name.to_string()),
                Some(step.name.clone()),
            );
        }

        if matches!(status, ExecutionStatus::Failed) && !step.continue_on_error.unwrap_or(false) {
            return Err(AppError::InternalServerError(format!(
                "Step failed: {}",
                step.name
            )));
        }

        Ok(())
    }

    fn get_step_command(&self, step: &Step) -> Option<String> {
        match &step.config.command {
            Some(cmd) => Some(cmd.clone()),
            None => step.config.script.clone(),
        }
    }

    async fn execute_shell_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let raw_command = step
            .config
            .command
            .as_ref()
            .or(step.config.script.as_ref())
            .ok_or_else(|| {
                AppError::ValidationError("Shell step requires command or script".to_string())
            })?;

        // Apply environment variable substitution to the command
        let mut command = raw_command.clone();
        info!("🔧 EXECUTOR DEBUG - Raw command: {}", raw_command);
        info!("🔧 EXECUTOR DEBUG - Available environment variables: {:?}", env);
        
        for (key, value) in env {
            let placeholder = format!("${{{}}}", key);
            command = command.replace(&placeholder, value);
        }

        info!("🔧 EXECUTOR DEBUG - Final command after substitution: {}", command);

        // For Git clone operations, use workspace root; for other operations, use SOURCE_DIR if available
        let working_dir = if command.contains("git clone") {
            // Git clone should run from workspace root, not from the target directory
            workspace.path.clone()
        } else if let Some(source_dir) = env.get("SOURCE_DIR") {
            std::path::PathBuf::from(source_dir)
        } else {
            step.config
                .working_directory
                .as_ref()
                .map(|d| workspace.path.join(d))
                .unwrap_or_else(|| workspace.path.clone())
        };

        info!(
            "🐚 EXECUTOR DEBUG - Executing enhanced shell command: {} in directory: {:?}",
            command, working_dir
        );

        // Ensure working directory exists before executing command
        if !working_dir.exists() {
            tokio::fs::create_dir_all(&working_dir).await.map_err(|e| {
                AppError::InternalServerError(format!(
                    "Failed to create working directory {}: {}", 
                    working_dir.display(), e
                ))
            })?;
            info!("📁 EXECUTOR DEBUG - Created working directory: {:?}", working_dir);
        }

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &command]);
            c
        };

        cmd.current_dir(&working_dir);

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        info!("🚀 EXECUTOR DEBUG - About to execute command with tokio::process::Command");
        let start_time = std::time::Instant::now();
        
        let output = cmd.output().await.map_err(|e| {
            error!("❌ EXECUTOR DEBUG - Failed to execute command: {}", e);
            AppError::InternalServerError(format!("Failed to execute command: {}", e))
        })?;

        let execution_duration = start_time.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        info!("⏱️ EXECUTOR DEBUG - Command execution completed in {:?}", execution_duration);
        info!("📤 EXECUTOR DEBUG - Exit code: {}", exit_code);
        info!("📤 EXECUTOR DEBUG - Stdout length: {} bytes", stdout.len());
        info!("📤 EXECUTOR DEBUG - Stderr length: {} bytes", stderr.len());
        
        if !stdout.is_empty() {
            info!("📤 EXECUTOR DEBUG - Stdout: {}", stdout);
        }
        if !stderr.is_empty() {
            info!("📤 EXECUTOR DEBUG - Stderr: {}", stderr);
        }

        if !output.status.success() {
            error!("❌ EXECUTOR DEBUG - Command failed with exit code {}: {}", exit_code, stderr);
            return Err(AppError::InternalServerError(format!(
                "Command failed with exit code {}: {}",
                exit_code, stderr
            )));
        }

        info!("✅ EXECUTOR DEBUG - Command executed successfully");
        Ok((exit_code, stdout, stderr))
    }

    async fn execute_repository_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        info!("📥 Executing repository step: {}", step.name);

        let repo_url = step.config.repository_url.as_ref().ok_or_else(|| {
            AppError::ValidationError("Repository step requires repository_url".to_string())
        })?;

        let access_token = env
            .get("GITHUB_TOKEN")
            .or_else(|| env.get("ACCESS_TOKEN"))
            .cloned();

        let repo_config = RepositoryConfig {
            url: repo_url.clone(),
            branch: step.config.branch.clone(),
            commit: step.config.commit.clone(),
            access_token,
            ssh_key: None,
            clone_depth: Some(1), // Shallow clone by default
            submodules: false,
        };

        let clone_result = self
            .repository_manager
            .clone_repository(&repo_config, &workspace.path)
            .await?;

        let output = format!(
            "Repository cloned successfully:\n- Commit: {}\n- Branch: {}\n- Size: {} bytes",
            clone_result.commit_hash, clone_result.branch, clone_result.size_bytes
        );

        Ok((0, output, String::new()))
    }

    async fn execute_build_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        info!("🔨 Executing build step: {}", step.name);

        // Create build config from step configuration
        let build_config = if step.config.command.is_some() || step.config.script.is_some() {
            // Custom build commands
            let commands = if let Some(cmd) = &step.config.command {
                vec![cmd.clone()]
            } else if let Some(script) = &step.config.script {
                script.lines().map(|line| line.to_string()).collect()
            } else {
                Vec::new()
            };

            Some(BuildConfig {
                build_type: crate::ci::builder::BuildType::Custom,
                build_commands: commands,
                environment_variables: env.clone(),
                build_directory: step.config.working_directory.clone(),
                output_directory: None,
                cache_enabled: true,
                parallel_jobs: None,
            })
        } else {
            None // Auto-detect
        };

        let build_result = self
            .project_builder
            .build_project(&workspace.path, build_config)
            .await?;

        if build_result.success {
            let output = format!(
                "Build completed successfully:\n- Type: {:?}\n- Duration: {}s\n- Artifacts: {}",
                build_result.build_type,
                build_result.duration_seconds,
                build_result.artifacts.len()
            );
            Ok((0, output, build_result.logs.join("\n")))
        } else {
            Err(AppError::InternalServerError(format!(
                "Build failed: {}",
                build_result.logs.join("\n")
            )))
        }
    }

    async fn execute_deployment_step(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        info!("🚀 Executing deployment step: {}", step.name);

        let execution_id = {
            let exec = execution.read().await;
            exec.id
        };

        // Create deployment config from step configuration
        let deployment_config = self.create_deployment_config(step, env)?;

        let mut deployment_manager = self.deployment_manager.lock().await;
        let deployment_result = deployment_manager
            .deploy(execution_id, &workspace.path, &deployment_config)
            .await?;

        let output = format!(
            "Deployment completed:\n- ID: {}\n- Type: {:?}\n- Status: {:?}\n- Services: {}\n- Artifacts: {}",
            deployment_result.deployment_id,
            deployment_result.deployment_type,
            deployment_result.status,
            deployment_result.services.len(),
            deployment_result.artifacts.len()
        );

        Ok((0, output, deployment_result.logs.join("\n")))
    }

    fn create_deployment_config(
        &self,
        step: &Step,
        env: &HashMap<String, String>,
    ) -> Result<DeploymentConfig> {
        // Determine deployment type from step configuration
        let deployment_type = if step.config.image.is_some() || step.config.dockerfile.is_some() {
            DeploymentType::DockerContainer
        } else if step.name.contains("local") {
            DeploymentType::LocalDirectory
        } else {
            DeploymentType::Hybrid // Default to both directory and container
        };

        // Parse port mappings from environment or step config
        let port_mappings = env
            .get("PORTS")
            .map(|ports| {
                ports
                    .split(',')
                    .filter_map(|port_str| {
                        if let Some((host, container)) = port_str.split_once(':') {
                            if let (Ok(host_port), Ok(container_port)) =
                                (host.parse(), container.parse())
                            {
                                return Some(crate::ci::deployment::PortMapping {
                                    host_port,
                                    container_port,
                                    protocol: "tcp".to_string(),
                                });
                            }
                        }
                        None
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![crate::ci::deployment::PortMapping {
                    host_port: 0, // Auto-allocate
                    container_port: 8000,
                    protocol: "tcp".to_string(),
                }]
            });

        let docker_config = if matches!(
            deployment_type,
            DeploymentType::DockerContainer | DeploymentType::Hybrid
        ) {
            Some(crate::ci::deployment::DockerDeploymentConfig {
                image_name: step
                    .config
                    .image
                    .clone()
                    .unwrap_or_else(|| format!("ci-app-{}", uuid::Uuid::new_v4())),
                dockerfile_path: step.config.dockerfile.clone(),
                build_context: step.config.build_context.clone(),
                base_image: None,
                distroless: env.get("DISTROLESS").map(|v| v == "true").unwrap_or(false),
                registry: step.config.registry.clone(),
                tags: step
                    .config
                    .tags
                    .clone()
                    .unwrap_or_else(|| vec!["latest".to_string()]),
            })
        } else {
            None
        };

        Ok(DeploymentConfig {
            deployment_type,
            target_directory: None, // Will be auto-generated
            docker_config,
            port_mappings,
            environment_variables: env.clone(),
            health_check: None,        // TODO: Add health check configuration
            manual_project_type: None, // Auto-detect by default
        })
    }

    async fn execute_docker_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_kubernetes_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_aws_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_azure_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_gcp_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_github_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_gitlab_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }

    async fn execute_custom_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        let mut connector_manager = self.connector_manager.lock().await;
        let result = connector_manager.execute_step(step, workspace, env).await?;
        Ok((result.exit_code, result.stdout, result.stderr))
    }
}
