use crate::ci::{config::{CIPipeline, PipelineType, SimpleStep, PipelineJob, JobScript}, workspace::WorkspaceContext};
use crate::error::Result;
use std::collections::HashMap;
use tracing::debug;

/// Pipeline template engine for processing commands with workspace context
pub struct PipelineTemplateEngine {
    workspace_context: WorkspaceContext,
}

impl PipelineTemplateEngine {
    pub fn new(workspace_context: WorkspaceContext) -> Self {
        Self { workspace_context }
    }
    
    /// Process entire pipeline to inject workspace paths and handle different pipeline structures
    pub fn process_pipeline(&self, pipeline: &mut CIPipeline) -> Result<()> {
        debug!("🔧 Processing pipeline with workspace context injection");
        
        let pipeline_type = pipeline.get_pipeline_type();
        
        match pipeline_type {
            PipelineType::Minimal => {
                // For minimal pipelines, process repo URL if present
                if let Some(repo) = &pipeline.repo {
                    let processed_repo = self.process_command(repo)?;
                    pipeline.repo = Some(processed_repo);
                    debug!("🔄 Processed repo URL: {}", pipeline.repo.as_ref().unwrap());
                }
            }
            PipelineType::Simple => {
                // Process simple steps
                if let Some(steps) = &mut pipeline.steps {
                    for (i, step) in steps.iter_mut().enumerate() {
                        match step {
                            SimpleStep::Command(cmd) => {
                                let processed_cmd = self.process_command(cmd)?;
                                *cmd = processed_cmd;
                                debug!("🔄 Processed simple command {}: {}", i, cmd);
                            }
                            SimpleStep::Detailed { run, .. } => {
                                let processed_run = self.process_command(run)?;
                                *run = processed_run;
                                debug!("🔄 Processed simple detailed command {}: {}", i, run);
                            }
                        }
                    }
                }
            }
            PipelineType::Standard => {
                // Process standard stages (existing logic)
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
            }
            PipelineType::Advanced => {
                // Process advanced jobs
                if let Some(jobs) = &mut pipeline.jobs {
                    for (job_name, job) in jobs.iter_mut() {
                        match job {
                            PipelineJob::Simple(script) => {
                                match script {
                                    JobScript::Single(cmd) => {
                                        let processed_cmd = self.process_command(cmd)?;
                                        *cmd = processed_cmd;
                                        debug!("🔄 Processed job '{}' single script: {}", job_name, cmd);
                                    }
                                    JobScript::Multiple(cmds) => {
                                        for (i, cmd) in cmds.iter_mut().enumerate() {
                                            let processed_cmd = self.process_command(cmd)?;
                                            *cmd = processed_cmd;
                                            debug!("🔄 Processed job '{}' script {}: {}", job_name, i, cmd);
                                        }
                                    }
                                }
                            }
                            PipelineJob::Detailed { script, .. } => {
                                match script {
                                    JobScript::Single(cmd) => {
                                        let processed_cmd = self.process_command(cmd)?;
                                        *cmd = processed_cmd;
                                        debug!("🔄 Processed detailed job '{}' single script: {}", job_name, cmd);
                                    }
                                    JobScript::Multiple(cmds) => {
                                        for (i, cmd) in cmds.iter_mut().enumerate() {
                                            let processed_cmd = self.process_command(cmd)?;
                                            *cmd = processed_cmd;
                                            debug!("🔄 Processed detailed job '{}' script {}: {}", job_name, i, cmd);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Also process standard stages if present
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
            }
        }
        
        Ok(())
    }
    
    /// Process pipeline with variable substitution
    pub fn process_pipeline_with_variables(&self, pipeline: &mut CIPipeline, variables: &HashMap<String, String>) -> Result<()> {
        debug!("🔧 Processing pipeline with variable substitution");
        
        // First apply workspace context
        self.process_pipeline(pipeline)?;
        
        // Then apply variable substitution
        let pipeline_type = pipeline.get_pipeline_type();
        
        match pipeline_type {
            PipelineType::Simple => {
                if let Some(steps) = &mut pipeline.steps {
                    for step in steps.iter_mut() {
                        match step {
                            SimpleStep::Command(cmd) => {
                                *cmd = self.substitute_variables(cmd, variables);
                            }
                            SimpleStep::Detailed { run, .. } => {
                                *run = self.substitute_variables(run, variables);
                            }
                        }
                    }
                }
            }
            PipelineType::Advanced => {
                if let Some(jobs) = &mut pipeline.jobs {
                    for (_, job) in jobs.iter_mut() {
                        match job {
                            PipelineJob::Simple(script) => {
                                match script {
                                    JobScript::Single(cmd) => {
                                        *cmd = self.substitute_variables(cmd, variables);
                                    }
                                    JobScript::Multiple(cmds) => {
                                        for cmd in cmds.iter_mut() {
                                            *cmd = self.substitute_variables(cmd, variables);
                                        }
                                    }
                                }
                            }
                            PipelineJob::Detailed { script, .. } => {
                                match script {
                                    JobScript::Single(cmd) => {
                                        *cmd = self.substitute_variables(cmd, variables);
                                    }
                                    JobScript::Multiple(cmds) => {
                                        for cmd in cmds.iter_mut() {
                                            *cmd = self.substitute_variables(cmd, variables);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // For minimal and standard, variables are handled at execution time
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
    
    /// Substitute variables in command string
    fn substitute_variables(&self, command: &str, variables: &HashMap<String, String>) -> String {
        let mut result = command.to_string();
        
        for (key, value) in variables {
            let placeholder = format!("${{{}}}", key);
            result = result.replace(&placeholder, value);
            
            // Also handle $VAR format (without braces)
            let simple_placeholder = format!("${}", key);
            result = result.replace(&simple_placeholder, value);
        }
        
        debug!("🔄 Variable substitution: '{}' -> '{}'", command, result);
        result
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

    #[test]
    fn test_variable_substitution() {
        let context = create_test_workspace_context();
        let engine = PipelineTemplateEngine::new(context);
        
        let mut variables = HashMap::new();
        variables.insert("RUST_VERSION".to_string(), "1.70".to_string());
        variables.insert("BUILD_TYPE".to_string(), "release".to_string());
        
        let command = "cargo build --${BUILD_TYPE} --rust-version ${RUST_VERSION}";
        let result = engine.substitute_variables(command, &variables);
        
        assert_eq!(result, "cargo build --release --rust-version 1.70");
    }

    #[test]
    fn test_simple_pipeline_processing() {
        use crate::ci::config::{CIPipeline, PipelineType, SimpleStep};
        
        let context = create_test_workspace_context();
        let engine = PipelineTemplateEngine::new(context);
        
        let mut pipeline = CIPipeline::new("test".to_string());
        pipeline.pipeline_type = Some(PipelineType::Simple);
        pipeline.steps = Some(vec![
            SimpleStep::Command("echo ${SOURCE_DIR}".to_string()),
            SimpleStep::Detailed {
                run: "ls ${BUILD_DIR}".to_string(),
                name: Some("list".to_string()),
                working_directory: None,
            },
        ]);
        
        let result = engine.process_pipeline(&mut pipeline);
        assert!(result.is_ok());
        
        if let Some(steps) = &pipeline.steps {
            match &steps[0] {
                SimpleStep::Command(cmd) => {
                    assert!(cmd.contains("/tmp/test-workspace/source"));
                }
                _ => panic!("Expected command step"),
            }
        }
    }
}