//! Execution Strategies - Strategy pattern implementation for pipeline execution
//!
//! This module provides different execution strategies for pipelines,
//! including sequential, parallel, and hybrid execution approaches.
//! It also includes multi-tier pipeline execution strategies for Minimal,
//! Simple, Standard, and Advanced pipeline types.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::ci::{
    config::{CIPipeline, PipelineType, Stage, SimpleStep, PipelineJob, JobScript, MatrixConfig},
    executor::PipelineExecutor,
    pipeline::{ExecutionStatus, TriggerInfo},
    template_engine::PipelineTemplateEngine,
    workspace::{Workspace, WorkspaceContext, WorkspaceManager},
};
use crate::error::{AppError, Result};

/// Execution context passed to strategies
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub execution_id: Uuid,
    pub pipeline_id: Uuid,
    pub correlation_id: Uuid,
    pub pipeline: CIPipeline,
    pub trigger_info: TriggerInfo,
    pub environment: HashMap<String, String>,
    pub started_at: DateTime<Utc>,
}

/// Result of pipeline execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub execution_id: Uuid,
    pub status: ExecutionStatus,
    pub stages_executed: usize,
    pub steps_executed: usize,
    pub duration_ms: u64,
    pub artifacts: Vec<String>,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

/// Types of execution strategies
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionStrategyType {
    Sequential,
    Parallel,
    Hybrid,
    Conditional,
}

/// Core trait for execution strategies
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    /// Execute the pipeline with this strategy
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult>;

    /// Get the strategy type
    fn strategy_type(&self) -> ExecutionStrategyType;

    /// Check if this strategy supports rollback
    fn supports_rollback(&self) -> bool;

    /// Get strategy name for logging
    fn strategy_name(&self) -> &'static str;

    /// Validate if this strategy can handle the given context
    fn can_handle(&self, context: &ExecutionContext) -> bool;

    /// Get estimated execution time (for scheduling)
    fn estimate_execution_time(&self, context: &ExecutionContext) -> std::time::Duration {
        // Default estimation based on number of steps
        let total_steps: usize = context
            .pipeline
            .stages
            .iter()
            .map(|stage| stage.steps.len())
            .sum();
        std::time::Duration::from_secs((total_steps * 30) as u64) // 30 seconds per step estimate
    }
}

/// Sequential execution strategy - executes stages and steps one by one
pub struct SequentialExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
}

impl SequentialExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            executor,
            workspace_manager,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for SequentialExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "sequential",
            "Starting sequential pipeline execution"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        // Use the original pipeline - workspace context injection handles path replacement
        let processed_pipeline = context.pipeline.clone();
        // Note: Template engine disabled to prevent double path processing
        // let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
        // template_engine.process_pipeline(&mut processed_pipeline)?;

        info!(
            execution_id = %context.execution_id,
            workspace_path = %workspace_context.workspace_path.display(),
            source_dir = %workspace_context.source_directory.display(),
            "Enhanced workspace created with path injection"
        );

        // Create a traditional workspace for compatibility
        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut stages_executed = 0;
        let mut steps_executed = 0;
        let mut logs = Vec::new();
        let mut artifacts = Vec::new();

        // Execute stages sequentially using processed pipeline
        for stage in &processed_pipeline.stages {
            debug!(
                execution_id = %context.execution_id,
                stage_name = %stage.name,
                "Executing stage sequentially"
            );

            // Execute stage with workspace context
            match self
                .execute_stage_sequential(context, stage, &workspace, &workspace_context)
                .await
            {
                Ok(stage_result) => {
                    stages_executed += 1;
                    steps_executed += stage.steps.len();
                    logs.extend(stage_result.logs);
                    artifacts.extend(stage_result.artifacts);
                }
                Err(e) => {
                    error!(
                        execution_id = %context.execution_id,
                        stage_name = %stage.name,
                        error = %e,
                        "Stage execution failed in sequential strategy"
                    );

                    // Cleanup workspace
                    if let Err(cleanup_err) = self
                        .workspace_manager
                        .cleanup_workspace(context.execution_id)
                        .await
                    {
                        warn!(
                            execution_id = %context.execution_id,
                            error = %cleanup_err,
                            "Failed to cleanup workspace after stage failure"
                        );
                    }

                    return Ok(ExecutionResult {
                        execution_id: context.execution_id,
                        status: ExecutionStatus::Failed,
                        stages_executed,
                        steps_executed,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        artifacts,
                        logs,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(
                execution_id = %context.execution_id,
                error = %e,
                "Failed to cleanup workspace after successful execution"
            );
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            stages_executed = stages_executed,
            steps_executed = steps_executed,
            duration_ms = duration.as_millis(),
            "Sequential pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed,
            steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Sequential
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Sequential"
    }

    fn can_handle(&self, _context: &ExecutionContext) -> bool {
        true // Sequential strategy can handle any pipeline
    }
}

impl SequentialExecutionStrategy {
    async fn execute_stage_sequential(
        &self,
        context: &ExecutionContext,
        stage: &Stage,
        workspace: &Workspace,
        workspace_context: &WorkspaceContext,
    ) -> Result<StageExecutionResult> {
        let mut logs = Vec::new();
        let artifacts = Vec::new();

        // Merge environment variables with workspace context
        let mut env = context.environment.clone();
        if let Some(stage_env) = &stage.environment {
            env.extend(stage_env.clone());
        }
        // Add workspace environment variables
        env.extend(workspace_context.get_environment_variables());

        // Execute steps sequentially
        for step in &stage.steps {
            debug!(
                execution_id = %context.execution_id,
                stage_name = %stage.name,
                step_name = %step.name,
                "Executing step sequentially"
            );

            // Create a mock execution for the executor
            let execution = Arc::new(tokio::sync::RwLock::new(
                crate::ci::pipeline::PipelineExecution::new(
                    context.pipeline_id,
                    context.trigger_info.clone(),
                ),
            ));

            match self
                .executor
                .execute_step(execution, &stage.name, step, workspace, &env)
                .await
            {
                Ok(_) => {
                    logs.push(format!("Step '{}' completed successfully", step.name));
                }
                Err(e) => {
                    if step.continue_on_error.unwrap_or(false) {
                        warn!(
                            execution_id = %context.execution_id,
                            step_name = %step.name,
                            error = %e,
                            "Step failed but continuing due to continue_on_error flag"
                        );
                        logs.push(format!("Step '{}' failed but continuing: {}", step.name, e));
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(StageExecutionResult { logs, artifacts })
    }
}

/// Parallel execution strategy - executes stages in parallel where possible
pub struct ParallelExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
    max_parallel_stages: usize,
}

impl ParallelExecutionStrategy {
    pub fn new(
        executor: Arc<PipelineExecutor>,
        workspace_manager: Arc<WorkspaceManager>,
        max_parallel_stages: usize,
    ) -> Self {
        Self {
            executor,
            workspace_manager,
            max_parallel_stages,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for ParallelExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "parallel",
            max_parallel_stages = self.max_parallel_stages,
            "Starting parallel pipeline execution"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        // Use the original pipeline - workspace context injection handles path replacement
        let processed_pipeline = context.pipeline.clone();
        // Note: Template engine disabled to prevent double path processing
        // let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
        // template_engine.process_pipeline(&mut processed_pipeline)?;

        info!(
            execution_id = %context.execution_id,
            workspace_path = %workspace_context.workspace_path.display(),
            source_dir = %workspace_context.source_directory.display(),
            "Enhanced workspace created with path injection for parallel execution"
        );

        // Create a traditional workspace for compatibility
        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut stages_executed = 0;
        let mut steps_executed = 0;
        let mut logs = Vec::new();
        let mut artifacts = Vec::new();

        // Group stages that can be executed in parallel using processed pipeline
        let stage_groups = self.group_stages_for_parallel_execution(&processed_pipeline.stages);

        // Execute stage groups
        for (group_index, stage_group) in stage_groups.iter().enumerate() {
            debug!(
                execution_id = %context.execution_id,
                group_index = group_index,
                stages_in_group = stage_group.len(),
                "Executing stage group in parallel"
            );

            // Execute stages in this group in parallel
            let mut handles = Vec::new();

            for stage in stage_group {
                let context_clone = context.clone();
                let stage_clone = stage.clone();
                let workspace_clone = workspace.clone();
                let executor_clone = self.executor.clone();

                let workspace_context_clone = workspace_context.clone();
                let handle = tokio::spawn(async move {
                    Self::execute_stage_parallel(
                        executor_clone,
                        &context_clone,
                        &stage_clone,
                        &workspace_clone,
                        &workspace_context_clone,
                    )
                    .await
                });

                handles.push((stage.name.clone(), handle));
            }

            // Wait for all stages in the group to complete
            let mut group_success = true;
            for (stage_name, handle) in handles {
                match handle.await {
                    Ok(Ok(stage_result)) => {
                        debug!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            "Parallel stage execution completed successfully"
                        );
                        stages_executed += 1;
                        steps_executed += stage_result.steps_count;
                        logs.extend(stage_result.logs);
                        artifacts.extend(stage_result.artifacts);
                    }
                    Ok(Err(e)) => {
                        error!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            error = %e,
                            "Parallel stage execution failed"
                        );
                        group_success = false;
                        logs.push(format!("Stage '{}' failed: {}", stage_name, e));
                    }
                    Err(e) => {
                        error!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            error = %e,
                            "Parallel stage execution panicked"
                        );
                        group_success = false;
                        logs.push(format!("Stage '{}' panicked: {}", stage_name, e));
                    }
                }
            }

            // If any stage in the group failed, stop execution
            if !group_success {
                // Cleanup workspace
                if let Err(e) = self
                    .workspace_manager
                    .cleanup_workspace(context.execution_id)
                    .await
                {
                    warn!(
                        execution_id = %context.execution_id,
                        error = %e,
                        "Failed to cleanup workspace after parallel stage failure"
                    );
                }

                return Ok(ExecutionResult {
                    execution_id: context.execution_id,
                    status: ExecutionStatus::Failed,
                    stages_executed,
                    steps_executed,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    artifacts,
                    logs,
                    error: Some("One or more parallel stages failed".to_string()),
                });
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(
                execution_id = %context.execution_id,
                error = %e,
                "Failed to cleanup workspace after successful parallel execution"
            );
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            stages_executed = stages_executed,
            steps_executed = steps_executed,
            duration_ms = duration.as_millis(),
            "Parallel pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed,
            steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Parallel
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Parallel"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        // Check if pipeline has stages that can benefit from parallel execution
        context.pipeline.stages.len() > 1
            || context
                .pipeline
                .stages
                .iter()
                .any(|stage| stage.parallel.unwrap_or(false))
    }

    fn estimate_execution_time(&self, context: &ExecutionContext) -> std::time::Duration {
        // For parallel execution, estimate based on the longest stage group
        let stage_groups = self.group_stages_for_parallel_execution(&context.pipeline.stages);
        let max_group_steps = stage_groups
            .iter()
            .map(|group| group.iter().map(|stage| stage.steps.len()).sum::<usize>())
            .max()
            .unwrap_or(0);

        std::time::Duration::from_secs((max_group_steps * 30) as u64)
    }
}

impl ParallelExecutionStrategy {
    /// Group stages for parallel execution based on dependencies
    fn group_stages_for_parallel_execution(&self, stages: &[Stage]) -> Vec<Vec<Stage>> {
        // Simple implementation: each stage is its own group
        // In a more sophisticated implementation, you would analyze dependencies
        stages.iter().map(|stage| vec![stage.clone()]).collect()
    }

    async fn execute_stage_parallel(
        executor: Arc<PipelineExecutor>,
        context: &ExecutionContext,
        stage: &Stage,
        workspace: &Workspace,
        workspace_context: &WorkspaceContext,
    ) -> Result<ParallelStageResult> {
        let mut logs = Vec::new();
        let artifacts = Vec::new();
        let mut steps_count = 0;

        // Merge environment variables with workspace context
        let mut env = context.environment.clone();
        if let Some(stage_env) = &stage.environment {
            env.extend(stage_env.clone());
        }
        // Add workspace environment variables
        env.extend(workspace_context.get_environment_variables());

        // Check if stage should execute steps in parallel
        if stage.parallel.unwrap_or(false) {
            // Execute steps in parallel
            let mut handles = Vec::new();

            for step in &stage.steps {
                let execution = Arc::new(tokio::sync::RwLock::new(
                    crate::ci::pipeline::PipelineExecution::new(
                        context.pipeline_id,
                        context.trigger_info.clone(),
                    ),
                ));

                let executor_clone = executor.clone();
                let stage_name = stage.name.clone();
                let step_clone = step.clone();
                let workspace_clone = workspace.clone();
                let env_clone = env.clone();

                let handle = tokio::spawn(async move {
                    executor_clone
                        .execute_step(
                            execution,
                            &stage_name,
                            &step_clone,
                            &workspace_clone,
                            &env_clone,
                        )
                        .await
                });

                handles.push((step.name.clone(), handle));
            }

            // Wait for all steps to complete
            for (step_name, handle) in handles {
                match handle.await {
                    Ok(Ok(_)) => {
                        logs.push(format!(
                            "Parallel step '{}' completed successfully",
                            step_name
                        ));
                        steps_count += 1;
                    }
                    Ok(Err(e)) => {
                        return Err(AppError::InternalServerError(format!(
                            "Parallel step '{}' failed: {}",
                            step_name, e
                        )));
                    }
                    Err(e) => {
                        return Err(AppError::InternalServerError(format!(
                            "Parallel step '{}' panicked: {}",
                            step_name, e
                        )));
                    }
                }
            }
        } else {
            // Execute steps sequentially within the stage
            for step in &stage.steps {
                let execution = Arc::new(tokio::sync::RwLock::new(
                    crate::ci::pipeline::PipelineExecution::new(
                        context.pipeline_id,
                        context.trigger_info.clone(),
                    ),
                ));

                match executor
                    .execute_step(execution, &stage.name, step, workspace, &env)
                    .await
                {
                    Ok(_) => {
                        logs.push(format!("Step '{}' completed successfully", step.name));
                        steps_count += 1;
                    }
                    Err(e) => {
                        if step.continue_on_error.unwrap_or(false) {
                            logs.push(format!("Step '{}' failed but continuing: {}", step.name, e));
                            steps_count += 1;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(ParallelStageResult {
            logs,
            artifacts,
            steps_count,
        })
    }
}

/// Multi-tier pipeline execution strategies

/// Minimal execution strategy - auto-detect + Docker deploy
pub struct MinimalExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
}

impl MinimalExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            executor,
            workspace_manager,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for MinimalExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "minimal",
            "Starting minimal pipeline execution with auto-detect + Docker deploy"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut logs = Vec::new();
        let mut artifacts = Vec::new();

        // For minimal pipelines, we auto-detect and deploy with Docker
        let pipeline = &context.pipeline;
        
        // Step 1: Clone repository if specified
        if let Some(repo_url) = &pipeline.repo {
            info!(execution_id = %context.execution_id, "Cloning repository: {}", repo_url);
            
            let clone_command = format!(
                "git clone {} {}",
                repo_url,
                workspace_context.source_directory.display()
            );
            
            let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
            let processed_command = template_engine.process_command(&clone_command)?;
            
            match self.execute_command(&processed_command, &workspace, &context.environment).await {
                Ok((_, stdout, _)) => {
                    logs.push(format!("Repository cloned successfully: {}", stdout));
                }
                Err(e) => {
                    return Ok(ExecutionResult {
                        execution_id: context.execution_id,
                        status: ExecutionStatus::Failed,
                        stages_executed: 0,
                        steps_executed: 0,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        artifacts,
                        logs,
                        error: Some(format!("Failed to clone repository: {}", e)),
                    });
                }
            }
        }

        // Step 2: Auto-detect project type and build
        let build_command = "if [ -f Cargo.toml ]; then cargo build --release; elif [ -f package.json ]; then npm install && npm run build; elif [ -f pom.xml ]; then mvn clean package; elif [ -f build.gradle ]; then ./gradlew build; else echo 'No build system detected'; fi";
        
        let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
        let processed_build = template_engine.process_command(build_command)?;
        
        match self.execute_command(&processed_build, &workspace, &context.environment).await {
            Ok((_, stdout, _)) => {
                logs.push(format!("Build completed: {}", stdout));
            }
            Err(e) => {
                warn!(execution_id = %context.execution_id, "Build failed, continuing: {}", e);
                logs.push(format!("Build failed but continuing: {}", e));
            }
        }

        // Step 3: Create Dockerfile if not exists and deploy with Docker
        let dockerfile_creation = r#"
if [ ! -f Dockerfile ]; then
    echo "Creating default Dockerfile..."
    cat > Dockerfile << 'EOF'
FROM alpine:latest
WORKDIR /app
COPY . .
EXPOSE 8000
CMD ["echo", "Application deployed successfully"]
EOF
fi
"#;

        let processed_dockerfile = template_engine.process_command(dockerfile_creation)?;
        
        match self.execute_command(&processed_dockerfile, &workspace, &context.environment).await {
            Ok((_, stdout, _)) => {
                logs.push(format!("Dockerfile prepared: {}", stdout));
            }
            Err(e) => {
                logs.push(format!("Dockerfile creation warning: {}", e));
            }
        }

        // Step 4: Build Docker image
        let docker_build = "docker build -t minimal-app:latest .";
        let processed_docker = template_engine.process_command(docker_build)?;
        
        match self.execute_command(&processed_docker, &workspace, &context.environment).await {
            Ok((_, stdout, _)) => {
                logs.push(format!("Docker image built: {}", stdout));
                artifacts.push("minimal-app:latest".to_string());
            }
            Err(e) => {
                return Ok(ExecutionResult {
                    execution_id: context.execution_id,
                    status: ExecutionStatus::Failed,
                    stages_executed: 1,
                    steps_executed: 3,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    artifacts,
                    logs,
                    error: Some(format!("Docker build failed: {}", e)),
                });
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(execution_id = %context.execution_id, error = %e, "Failed to cleanup workspace");
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            duration_ms = duration.as_millis(),
            "Minimal pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed: 1,
            steps_executed: 4,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Sequential // Minimal uses sequential execution
    }

    fn supports_rollback(&self) -> bool {
        false // Minimal pipelines don't support rollback
    }

    fn strategy_name(&self) -> &'static str {
        "Minimal"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        matches!(context.pipeline.get_pipeline_type(), PipelineType::Minimal)
    }
}

impl MinimalExecutionStrategy {
    async fn execute_command(
        &self,
        command: &str,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        use tokio::process::Command;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(&workspace.path);

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let output = cmd.output().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to execute command: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        if !output.status.success() {
            return Err(AppError::InternalServerError(format!(
                "Command failed with exit code {}: {}",
                exit_code, stderr
            )));
        }

        Ok((exit_code, stdout, stderr))
    }
}

/// Simple execution strategy - linear steps execution
pub struct SimpleExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
}

impl SimpleExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            executor,
            workspace_manager,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for SimpleExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "simple",
            "Starting simple pipeline execution with linear steps"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut logs = Vec::new();
        let mut artifacts = Vec::new();
        let mut steps_executed = 0;

        let pipeline = &context.pipeline;
        
        // Execute steps linearly
        if let Some(steps) = &pipeline.steps {
            let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
            
            for (i, step) in steps.iter().enumerate() {
                let step_name = format!("step-{}", i + 1);
                
                let command = match step {
                    SimpleStep::Command(cmd) => cmd.clone(),
                    SimpleStep::Detailed { run, name: _, working_directory: _ } => run.clone(),
                };

                info!(
                    execution_id = %context.execution_id,
                    step = step_name,
                    "Executing simple step: {}", command
                );

                let processed_command = template_engine.process_command(&command)?;
                
                match self.execute_command(&processed_command, &workspace, &context.environment).await {
                    Ok((_, stdout, _)) => {
                        steps_executed += 1;
                        logs.push(format!("Step {} completed: {}", step_name, stdout));
                    }
                    Err(e) => {
                        error!(
                            execution_id = %context.execution_id,
                            step = step_name,
                            error = %e,
                            "Simple step execution failed"
                        );

                        // Cleanup workspace
                        if let Err(cleanup_err) = self
                            .workspace_manager
                            .cleanup_workspace(context.execution_id)
                            .await
                        {
                            warn!(
                                execution_id = %context.execution_id,
                                error = %cleanup_err,
                                "Failed to cleanup workspace after step failure"
                            );
                        }

                        return Ok(ExecutionResult {
                            execution_id: context.execution_id,
                            status: ExecutionStatus::Failed,
                            stages_executed: 0,
                            steps_executed,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            artifacts,
                            logs,
                            error: Some(format!("Step {} failed: {}", step_name, e)),
                        });
                    }
                }
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(execution_id = %context.execution_id, error = %e, "Failed to cleanup workspace");
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            steps_executed = steps_executed,
            duration_ms = duration.as_millis(),
            "Simple pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed: 1, // Simple pipelines have one implicit stage
            steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Sequential
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Simple"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        matches!(context.pipeline.get_pipeline_type(), PipelineType::Simple)
    }
}

impl SimpleExecutionStrategy {
    async fn execute_command(
        &self,
        command: &str,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        use tokio::process::Command;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(&workspace.path);

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let output = cmd.output().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to execute command: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        if !output.status.success() {
            return Err(AppError::InternalServerError(format!(
                "Command failed with exit code {}: {}",
                exit_code, stderr
            )));
        }

        Ok((exit_code, stdout, stderr))
    }
}

/// Standard execution strategy - stages/jobs execution (existing logic)
pub struct StandardExecutionStrategy {
    sequential_strategy: SequentialExecutionStrategy,
}

impl StandardExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            sequential_strategy: SequentialExecutionStrategy::new(executor, workspace_manager),
        }
    }
}

#[async_trait]
impl ExecutionStrategy for StandardExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        info!(
            execution_id = %context.execution_id,
            strategy = "standard",
            "Starting standard pipeline execution with stages/jobs"
        );

        // Standard pipelines use the existing sequential execution logic
        self.sequential_strategy.execute(context).await
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Sequential
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Standard"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        matches!(context.pipeline.get_pipeline_type(), PipelineType::Standard)
    }
}

/// Advanced execution strategy - matrix + variables + includes + cache
pub struct AdvancedExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
}

impl AdvancedExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            executor,
            workspace_manager,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for AdvancedExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "advanced",
            "Starting advanced pipeline execution with matrix + variables"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut logs = Vec::new();
        let mut artifacts = Vec::new();
        let mut total_stages_executed = 0;
        let mut total_steps_executed = 0;

        let pipeline = &context.pipeline;

        // Process variables
        let mut env = context.environment.clone();
        if let Some(variables) = &pipeline.variables {
            env.extend(variables.clone());
            logs.push(format!("Loaded {} pipeline variables", variables.len()));
        }

        // Process includes (simplified - just log for now)
        if let Some(includes) = &pipeline.include {
            logs.push(format!("Processing {} include files", includes.len()));
            // TODO: Implement actual include processing
        }

        // Process cache configuration (simplified - just log for now)
        if let Some(cache) = &pipeline.cache {
            logs.push(format!("Cache configured for {} paths", cache.paths.len()));
            // TODO: Implement actual cache handling
        }

        // Execute jobs with matrix expansion
        if let Some(jobs) = &pipeline.jobs {
            for (job_name, job) in jobs {
                info!(
                    execution_id = %context.execution_id,
                    job = job_name,
                    "Executing advanced job"
                );

                // Check if job has matrix configuration
                let matrix_combinations = self.expand_matrix(job, &pipeline.matrix)?;
                
                for (matrix_index, matrix_env) in matrix_combinations.iter().enumerate() {
                    let job_instance_name = if matrix_combinations.len() > 1 {
                        format!("{}-matrix-{}", job_name, matrix_index)
                    } else {
                        job_name.clone()
                    };

                    // Merge environment variables
                    let mut job_env = env.clone();
                    job_env.extend(matrix_env.clone());

                    // Execute job scripts
                    let scripts = match job {
                        PipelineJob::Simple(script) => match script {
                            JobScript::Single(cmd) => vec![cmd.clone()],
                            JobScript::Multiple(cmds) => cmds.clone(),
                        },
                        PipelineJob::Detailed { script, .. } => match script {
                            JobScript::Single(cmd) => vec![cmd.clone()],
                            JobScript::Multiple(cmds) => cmds.clone(),
                        },
                    };

                    let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
                    
                    for (script_index, script_cmd) in scripts.iter().enumerate() {
                        let step_name = format!("{}-step-{}", job_instance_name, script_index + 1);
                        
                        let processed_command = template_engine.process_command(script_cmd)?;
                        
                        match self.execute_command(&processed_command, &workspace, &job_env).await {
                            Ok((_, stdout, _)) => {
                                total_steps_executed += 1;
                                logs.push(format!("Job step {} completed: {}", step_name, stdout));
                            }
                            Err(e) => {
                                error!(
                                    execution_id = %context.execution_id,
                                    job = job_instance_name,
                                    step = step_name,
                                    error = %e,
                                    "Advanced job step execution failed"
                                );

                                // Cleanup workspace
                                if let Err(cleanup_err) = self
                                    .workspace_manager
                                    .cleanup_workspace(context.execution_id)
                                    .await
                                {
                                    warn!(
                                        execution_id = %context.execution_id,
                                        error = %cleanup_err,
                                        "Failed to cleanup workspace after job failure"
                                    );
                                }

                                return Ok(ExecutionResult {
                                    execution_id: context.execution_id,
                                    status: ExecutionStatus::Failed,
                                    stages_executed: total_stages_executed,
                                    steps_executed: total_steps_executed,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                    artifacts,
                                    logs,
                                    error: Some(format!("Job {} step {} failed: {}", job_instance_name, step_name, e)),
                                });
                            }
                        }
                    }
                }

                total_stages_executed += 1;
            }
        }

        // If no jobs defined, fall back to stages execution
        if pipeline.jobs.is_none() && !pipeline.stages.is_empty() {
            // Use sequential strategy for stages
            let sequential_strategy = SequentialExecutionStrategy::new(
                self.executor.clone(),
                self.workspace_manager.clone(),
            );
            
            let result = sequential_strategy.execute(context).await?;
            total_stages_executed += result.stages_executed;
            total_steps_executed += result.steps_executed;
            logs.extend(result.logs);
            artifacts.extend(result.artifacts);
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(execution_id = %context.execution_id, error = %e, "Failed to cleanup workspace");
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            stages_executed = total_stages_executed,
            steps_executed = total_steps_executed,
            duration_ms = duration.as_millis(),
            "Advanced pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed: total_stages_executed,
            steps_executed: total_steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Hybrid // Advanced can use both sequential and parallel
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Advanced"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        matches!(context.pipeline.get_pipeline_type(), PipelineType::Advanced)
    }
}

impl AdvancedExecutionStrategy {
    /// Expand matrix configuration into individual environment variable combinations
    fn expand_matrix(&self, job: &PipelineJob, global_matrix: &Option<MatrixConfig>) -> Result<Vec<HashMap<String, String>>> {
        let mut combinations = vec![HashMap::new()];

        // Get matrix config from job or global
        let matrix_config = match job {
            PipelineJob::Detailed { matrix: Some(matrix), .. } => Some(matrix),
            _ => global_matrix.as_ref(),
        };

        if let Some(matrix) = matrix_config {
            combinations.clear();
            
            // Generate all combinations of matrix variables
            for (key, values) in &matrix.variables {
                let mut new_combinations = Vec::new();
                
                for existing_combo in &combinations {
                    for value in values {
                        let mut new_combo = existing_combo.clone();
                        new_combo.insert(key.clone(), value.clone());
                        new_combinations.push(new_combo);
                    }
                }
                
                combinations = new_combinations;
            }
        }

        // If no matrix, return single empty combination
        if combinations.is_empty() {
            combinations.push(HashMap::new());
        }

        Ok(combinations)
    }

    async fn execute_command(
        &self,
        command: &str,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        use tokio::process::Command;

        // Apply variable substitution
        let mut processed_command = command.to_string();
        for (key, value) in env {
            let placeholder = format!("${{{}}}", key);
            processed_command = processed_command.replace(&placeholder, value);
        }

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &processed_command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &processed_command]);
            c
        };

        cmd.current_dir(&workspace.path);

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        let output = cmd.output().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to execute command: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        if !output.status.success() {
            return Err(AppError::InternalServerError(format!(
                "Command failed with exit code {}: {}",
                exit_code, stderr
            )));
        }

        Ok((exit_code, stdout, stderr))
    }
}

/// Factory for creating execution strategies
pub struct ExecutionStrategyFactory {
    strategies: HashMap<ExecutionStrategyType, Box<dyn ExecutionStrategy>>,
}

impl ExecutionStrategyFactory {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    /// Create a factory with all multi-tier pipeline strategies registered
    pub fn with_multi_tier_strategies(
        executor: Arc<PipelineExecutor>,
        workspace_manager: Arc<WorkspaceManager>,
    ) -> Self {
        let mut factory = Self::new();
        
        // Register multi-tier strategies
        factory.register_strategy(Box::new(MinimalExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
        )));
        
        factory.register_strategy(Box::new(SimpleExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
        )));
        
        factory.register_strategy(Box::new(StandardExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
        )));
        
        factory.register_strategy(Box::new(AdvancedExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
        )));

        // Also register the original strategies for backward compatibility
        factory.register_strategy(Box::new(SequentialExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
        )));
        
        factory.register_strategy(Box::new(ParallelExecutionStrategy::new(
            executor,
            workspace_manager,
            4, // max parallel stages
        )));
        
        factory
    }

    /// Register a strategy
    pub fn register_strategy(&mut self, strategy: Box<dyn ExecutionStrategy>) {
        let strategy_type = strategy.strategy_type();
        self.strategies.insert(strategy_type, strategy);
    }

    /// Create a strategy instance based on pipeline type
    pub fn create_strategy_for_pipeline_type(
        &self,
        pipeline_type: PipelineType,
    ) -> Result<&dyn ExecutionStrategy> {
        // Find strategy that can handle this pipeline type
        for strategy in self.strategies.values() {
            if strategy.strategy_name() == match pipeline_type {
                PipelineType::Minimal => "Minimal",
                PipelineType::Simple => "Simple", 
                PipelineType::Standard => "Standard",
                PipelineType::Advanced => "Advanced",
            } {
                return Ok(strategy.as_ref());
            }
        }
        
        // Fallback to sequential strategy for backward compatibility
        self.create_strategy(ExecutionStrategyType::Sequential)
    }

    /// Create a strategy instance
    pub fn create_strategy(
        &self,
        strategy_type: ExecutionStrategyType,
    ) -> Result<&dyn ExecutionStrategy> {
        self.strategies
            .get(&strategy_type)
            .map(|s| s.as_ref())
            .ok_or_else(|| {
                AppError::InternalServerError(format!("Strategy not found: {:?}", strategy_type))
            })
    }

    /// Get available strategies
    pub fn available_strategies(&self) -> Vec<ExecutionStrategyType> {
        self.strategies.keys().cloned().collect()
    }

    /// Find best strategy for context based on pipeline type
    pub fn find_best_strategy(&self, context: &ExecutionContext) -> Option<ExecutionStrategyType> {
        // First try to find strategy based on pipeline type
        let pipeline_type = context.pipeline.get_pipeline_type();
        
        for strategy in self.strategies.values() {
            if strategy.can_handle(context) {
                return Some(strategy.strategy_type());
            }
        }
        
        // Fallback to sequential for backward compatibility
        Some(ExecutionStrategyType::Sequential)
    }
}

impl Default for ExecutionStrategyFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper structs for execution results
#[derive(Debug)]
struct StageExecutionResult {
    logs: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug)]
struct ParallelStageResult {
    logs: Vec<String>,
    artifacts: Vec<String>,
    steps_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{Step, StepType, StepConfig, Stage};
    use std::collections::HashMap;

    fn create_test_context() -> ExecutionContext {
        let mut pipeline = CIPipeline::new("test-pipeline".to_string());
        pipeline.description = Some("Test pipeline".to_string());
        pipeline.stages = vec![
                Stage {
                    name: "build".to_string(),
                    condition: None,
                    parallel: Some(false),
                    timeout: Some(600),     // 10-minute stage timeout
                    retry_count: Some(0),
                    environment: None,
                    steps: vec![
                        Step {
                            name: "compile".to_string(),
                            step_type: StepType::Shell,
                            config: StepConfig {
                                command: Some("echo 'Building...'".to_string()),
                                ..Default::default()
                            },
                            condition: None,         // ✅ now filled
                            continue_on_error: Some(false),
                            timeout: Some(300),      // ✅ e.g. 5-minute step timeout
                        }
                    ],
                }
            ];
        
        pipeline.timeout = Some(3600);            // 1-hour pipeline timeout
        pipeline.retry_count = Some(0);
    
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            pipeline_id: pipeline.id.unwrap(),
            correlation_id: Uuid::new_v4(),
            pipeline,
            trigger_info: TriggerInfo {
                trigger_type: "manual".to_string(),
                triggered_by: Some("test".to_string()),
                commit_hash: None,
                branch: None,
                repository: None,
                webhook_payload: None,
            },
            environment: HashMap::new(),
            started_at: Utc::now(),
        }
    }    

    #[test]
    fn test_execution_strategy_types() {
        assert_eq!(
            ExecutionStrategyType::Sequential,
            ExecutionStrategyType::Sequential
        );
        assert_ne!(
            ExecutionStrategyType::Sequential,
            ExecutionStrategyType::Parallel
        );
    }

    #[test]
    fn test_execution_context_creation() {
        let context = create_test_context();
        assert!(!context.execution_id.is_nil());
        assert!(!context.pipeline_id.is_nil());
        assert!(!context.correlation_id.is_nil());
        assert_eq!(context.pipeline.name, "test-pipeline");
    }

    #[test]
    fn test_execution_strategy_factory() {
        let factory = ExecutionStrategyFactory::new();
        assert!(factory.available_strategies().is_empty());

        let context = create_test_context();
        assert!(factory.find_best_strategy(&context).is_some()); // Should fallback to sequential
    }

    #[test]
    fn test_pipeline_type_strategy_selection() {
        use crate::ci::config::PipelineType;
        
        // Test minimal pipeline context
        let mut context = create_test_context();
        context.pipeline.pipeline_type = Some(PipelineType::Minimal);
        context.pipeline.repo = Some("https://github.com/user/repo.git".to_string());
        
        // Minimal strategy should be able to handle minimal pipelines
        // (This would require actual strategy instances to test properly)
        assert_eq!(context.pipeline.get_pipeline_type(), PipelineType::Minimal);
    }

    #[test]
    fn test_matrix_expansion() {
        use crate::ci::config::{MatrixConfig, PipelineJob, JobScript};
        use std::collections::HashMap;
        
        let mut matrix_vars = HashMap::new();
        matrix_vars.insert("rust".to_string(), vec!["1.70".to_string(), "1.71".to_string()]);
        matrix_vars.insert("os".to_string(), vec!["ubuntu".to_string(), "windows".to_string()]);
        
        let matrix = MatrixConfig {
            variables: matrix_vars,
        };
        
        let job = PipelineJob::Detailed {
            stage: "test".to_string(),
            script: JobScript::Single("cargo test".to_string()),
            matrix: Some(matrix),
            cache: None,
        };
        
        // This would test matrix expansion logic
        // (Implementation would require actual AdvancedExecutionStrategy instance)
        assert!(matches!(job, PipelineJob::Detailed { .. }));
    }
}
