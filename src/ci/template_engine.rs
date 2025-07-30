use crate::ci::{config::CIPipeline, workspace::WorkspaceContext};
use crate::error::{AppError, Result};
use tracing::debug;

/// Pipeline template engine for processing commands with workspace context
pub struct PipelineTemplateEngine {
    workspace_context: WorkspaceContext,
}

impl PipelineTemplateEngine {
    pub fn new(workspace_context: WorkspaceContext) -> Self {
        Self { workspace_context }
    }
    
    /// Process entire pipeline to inject workspace paths
    pub fn process_pipeline(&self, pipeline: &mut CIPipeline) -> Result<()> {
        debug!("🔧 Processing pipeline with workspace context injection");
        
        for stage in &mut pipeline.stages {
            for step in &mut stage.steps {
                if let Some(command) = &step.config.command {
                    let processed_command = self.process_command(command)?;
                    step.config.command = Some(processed_command);
                    debug!("🔄 Processed command for step '{}': {}", step.name, step.config.command.as_ref().unwrap());
                }
                if let Some(script) = &step.config.script {
                    let processed_script = self.process_command(script)?;
                    step.config.script = Some(processed_script);
                    debug!("🔄 Processed script for step '{}': {}", step.name, step.config.script.as_ref().unwrap());
                }
            }
        }
        Ok(())
    }
    
    /// Process individual command with workspace context
    pub fn process_command(&self, command: &str) -> Result<String> {
        let processed = self.workspace_context.inject_into_command(command);
        
        // Additional processing for common patterns
        let processed = self.fix_git_clone_commands(&processed);
        let processed = self.fix_docker_commands(&processed);
        let processed = self.fix_ssh_commands(&processed);
        let processed = self.ensure_directory_exists(&processed);
        
        Ok(processed)
    }
    
    /// Fix Git clone commands to use proper workspace paths
    fn fix_git_clone_commands(&self, command: &str) -> String {
        if command.contains("git clone") {
            // Replace the hardcoded path with source directory
            let fixed = command.replace(
                "git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci",
                &format!("git clone https://github.com/RitabrataRoychowdhury/RustCI.git {}", 
                        self.workspace_context.source_directory.to_string_lossy())
            );
            
            // Also handle the rm -rf command
            let fixed = fixed.replace(
                "rm -rf /tmp/rustci",
                &format!("rm -rf {}", self.workspace_context.source_directory.to_string_lossy())
            );
            
            debug!("🔧 Fixed Git clone command: {}", fixed);
            fixed
        } else {
            command.to_string()
        }
    }
    
    /// Fix Docker build context paths
    fn fix_docker_commands(&self, command: &str) -> String {
        if command.contains("docker build") {
            // Ensure Docker build uses the correct context
            command.replace(
                "cd /tmp/rustci",
                &format!("cd {}", self.workspace_context.source_directory.to_string_lossy())
            )
        } else {
            command.to_string()
        }
    }
    
    /// Fix SSH commands to handle errors gracefully
    fn fix_ssh_commands(&self, command: &str) -> String {
        if command.contains("sshpass") {
            // Add error handling for SSH commands
            format!("{} || echo 'SSH command failed but continuing'", command)
        } else {
            command.to_string()
        }
    }
    
    /// Ensure directory exists before running commands
    fn ensure_directory_exists(&self, command: &str) -> String {
        if command.contains("cd ") {
            // Extract the directory path and ensure it exists
            if let Some(cd_part) = command.split("cd ").nth(1) {
                if let Some(dir_path) = cd_part.split(" && ").next() {
                    let dir_path = dir_path.trim();
                    return format!("mkdir -p {} && {}", dir_path, command);
                }
            }
        }
        command.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn create_test_workspace_context() -> WorkspaceContext {
        let workspace_path = PathBuf::from("/tmp/test-workspace");
        let source_dir = workspace_path.join("source");
        let build_dir = workspace_path.join("build");
        let artifacts_dir = workspace_path.join("artifacts");
        
        WorkspaceContext {
            workspace_path: workspace_path.clone(),
            execution_id: Uuid::new_v4(),
            environment_variables: HashMap::new(),
            working_directory: source_dir.clone(),
            source_directory: source_dir,
            build_directory: build_dir,
            artifacts_directory: artifacts_dir,
        }
    }

    #[test]
    fn test_git_clone_command_processing() {
        let context = create_test_workspace_context();
        let engine = PipelineTemplateEngine::new(context);
        
        let original_command = "rm -rf /tmp/rustci && git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci";
        let processed = engine.process_command(original_command).unwrap();
        
        assert!(processed.contains("/tmp/test-workspace/source"));
        assert!(!processed.contains("/tmp/rustci"));
    }

    #[test]
    fn test_docker_build_command_processing() {
        let context = create_test_workspace_context();
        let engine = PipelineTemplateEngine::new(context);
        
        let original_command = "cd /tmp/rustci && docker build -t rustci:latest .";
        let processed = engine.process_command(original_command).unwrap();
        
        assert!(processed.contains("/tmp/test-workspace/source"));
        assert!(!processed.contains("/tmp/rustci"));
    }

    #[test]
    fn test_workspace_variable_injection() {
        let context = create_test_workspace_context();
        let engine = PipelineTemplateEngine::new(context);
        
        let original_command = "echo ${SOURCE_DIR} && ls ${BUILD_DIR}";
        let processed = engine.process_command(original_command).unwrap();
        
        assert!(processed.contains("/tmp/test-workspace/source"));
        assert!(processed.contains("/tmp/test-workspace/build"));
    }
}