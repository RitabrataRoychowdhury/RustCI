//! Kubernetes YAML generation utilities
//! 
//! This module provides functions to generate Kubernetes Job manifests
//! with proper configuration for CI/CD step execution.

use crate::error::{AppError, Result};
use crate::ci::{config::Step, workspace::Workspace};
use super::super::traits::KubernetesConfig;
use std::collections::HashMap;
use tracing::debug;
use uuid::Uuid;

/// Kubernetes YAML generator for CI/CD jobs
pub struct KubernetesYamlGenerator;

impl KubernetesYamlGenerator {
    /// Generate a Kubernetes Job YAML for the given step
    pub fn generate_job_yaml(
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
        config: &KubernetesConfig,
    ) -> Result<String> {
        debug!("🏗️ Generating Kubernetes Job YAML for step: {}", step.name);

        let job_name = Self::generate_job_name(&step.name);
        let image = step.config.image.as_ref()
            .ok_or_else(|| AppError::ValidationError(
                "Kubernetes step requires image to be specified".to_string()
            ))?;

        let command = Self::build_command(step)?;
        let env_vars = Self::build_environment_variables(env);
        let volume_mounts = Self::build_volume_mounts(workspace, config);
        let volumes = Self::build_volumes(workspace, config);
        let resources = Self::build_resource_requirements(config);
        let init_containers = Self::build_init_containers(step, config)?;

        let service_account = Self::build_service_account(config);
        let pod_security_context = Self::build_pod_security_context(config);
        let container_security_context = Self::build_container_security_context(config);

        let yaml = format!(
            r#"apiVersion: batch/v1
kind: Job
metadata:
  name: {job_name}
  namespace: {namespace}
  labels:
    app: ci-pipeline
    step: {step_label}
    connector: kubernetes
  annotations:
    ci.pipeline/step-name: "{step_name}"
    ci.pipeline/timeout: "{timeout}"
spec:
  ttlSecondsAfterFinished: 300
  backoffLimit: 1
  activeDeadlineSeconds: {timeout}
  template:
    metadata:
      labels:
        app: ci-pipeline
        step: {step_label}
    spec:
      restartPolicy: Never
      {service_account}
      {pod_security_context}
      {init_containers}
      containers:
      - name: step-executor
        image: {image}
        command: {command}
        {env_vars}
        {volume_mounts}
        {resources}
        {container_security_context}
      {volumes}
"#,
            job_name = job_name,
            namespace = config.namespace,
            step_label = Self::sanitize_label(&step.name),
            step_name = step.name,
            timeout = config.timeout_seconds,
            image = image,
            command = command,
            env_vars = env_vars,
            volume_mounts = volume_mounts,
            resources = resources,
            volumes = volumes,
            init_containers = init_containers,
        );

        debug!("✅ Generated Kubernetes Job YAML ({} bytes)", yaml.len());
        Ok(yaml)
    }

    /// Generate a valid Kubernetes job name
    fn generate_job_name(step_name: &str) -> String {
        let sanitized = step_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();
        
        let uuid_suffix = Uuid::new_v4().to_string()[..8].to_string();
        format!("ci-{}-{}", sanitized, uuid_suffix)
    }

    /// Sanitize string for use as Kubernetes label value
    fn sanitize_label(input: &str) -> String {
        input
            .chars()
            .take(63) // Kubernetes label value limit
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    /// Build command array for the container
    fn build_command(step: &Step) -> Result<String> {
        let command = if let Some(cmd) = &step.config.command {
            format!("[\"/bin/sh\", \"-c\", \"{}\"]", Self::escape_yaml_string(cmd))
        } else if let Some(script) = &step.config.script {
            format!("[\"/bin/sh\", \"-c\", \"{}\"]", Self::escape_yaml_string(script))
        } else {
            return Err(AppError::ValidationError(
                "Kubernetes step requires command or script".to_string()
            ));
        };

        Ok(command)
    }

    /// Build environment variables section
    fn build_environment_variables(env: &HashMap<String, String>) -> String {
        if env.is_empty() {
            return String::new();
        }

        let mut env_vars = String::from("env:\n");
        for (key, value) in env {
            env_vars.push_str(&format!(
                "        - name: {}\n          value: \"{}\"\n",
                key,
                Self::escape_yaml_string(value)
            ));
        }

        env_vars
    }

    /// Build volume mounts section
    fn build_volume_mounts(_workspace: &Workspace, config: &KubernetesConfig) -> String {
        let mut mounts = String::from("volumeMounts:\n");
        
        // Add workspace volume mount
        if config.use_hostpath || config.use_pvc {
            mounts.push_str("        - name: workspace\n          mountPath: /workspace\n");
        }

        // Add temporary volume for scratch space
        mounts.push_str("        - name: tmp-volume\n          mountPath: /tmp\n");

        mounts
    }

    /// Build volumes section
    fn build_volumes(workspace: &Workspace, config: &KubernetesConfig) -> String {
        let mut volumes = String::from("volumes:\n");
        
        // Add workspace volume
        if config.use_hostpath {
            volumes.push_str(&format!(
                "      - name: workspace\n        hostPath:\n          path: {}\n          type: Directory\n",
                workspace.path.display()
            ));
        } else if config.use_pvc {
            volumes.push_str("      - name: workspace\n        persistentVolumeClaim:\n          claimName: workspace-pvc\n");
        }

        // Add temporary volume
        volumes.push_str("      - name: tmp-volume\n        emptyDir: {}\n");

        volumes
    }

    /// Generate PersistentVolumeClaim YAML for workspace storage
    pub fn generate_pvc_yaml(
        workspace_id: &str,
        config: &KubernetesConfig,
    ) -> Result<String> {
        debug!("🗂️ Generating PVC YAML for workspace: {}", workspace_id);

        let storage_size = config.storage_size.as_ref()
            .ok_or_else(|| AppError::ValidationError(
                "Storage size must be specified for PVC".to_string()
            ))?;

        let storage_class_spec = if let Some(storage_class) = &config.storage_class {
            format!("  storageClassName: {}\n", storage_class)
        } else {
            String::new()
        };

        let yaml = format!(
            r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: workspace-pvc
  namespace: {namespace}
  labels:
    app: ci-pipeline
    workspace-id: {workspace_id}
    type: workspace-storage
spec:
{storage_class_spec}  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: {storage_size}
"#,
            namespace = config.namespace,
            workspace_id = workspace_id,
            storage_class_spec = storage_class_spec,
            storage_size = storage_size,
        );

        debug!("✅ Generated PVC YAML ({} bytes)", yaml.len());
        Ok(yaml)
    }

    /// Build resource requirements section
    fn build_resource_requirements(config: &KubernetesConfig) -> String {
        let has_requests = config.resource_requests.as_ref().map_or(false, |r| !r.is_empty());
        let has_limits = config.resource_limits.as_ref().map_or(false, |r| !r.is_empty());

        if !has_requests && !has_limits {
            return String::new();
        }

        let mut resources = String::from("resources:\n");

        if let Some(requests) = &config.resource_requests {
            if !requests.is_empty() {
                resources.push_str("          requests:\n");
                for (key, value) in requests {
                    resources.push_str(&format!("            {}: {}\n", key, value));
                }
            }
        }

        if let Some(limits) = &config.resource_limits {
            if !limits.is_empty() {
                resources.push_str("          limits:\n");
                for (key, value) in limits {
                    resources.push_str(&format!("            {}: {}\n", key, value));
                }
            }
        }

        resources
    }

    /// Build init containers section for git operations
    fn build_init_containers(_step: &Step, config: &KubernetesConfig) -> Result<String> {
        if let Some(repo_url) = &config.repo_url {
            debug!("🔄 Adding git init container for repo: {}", repo_url);
            
            let init_container = format!(
                r#"initContainers:
      - name: git-clone
        image: alpine/git:latest
        command: ["/bin/sh", "-c"]
        args:
        - |
          git clone {} /workspace/repo
          cd /workspace/repo
          echo "Repository cloned successfully"
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        securityContext:
          runAsNonRoot: false
      "#,
                Self::escape_yaml_string(repo_url)
            );
            
            Ok(init_container)
        } else {
            Ok(String::new())
        }
    }

    /// Escape string for safe YAML inclusion
    fn escape_yaml_string(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    /// Generate a ConfigMap YAML for scripts or complex configurations
    pub fn generate_configmap_yaml(
        name: &str,
        namespace: &str,
        data: &HashMap<String, String>,
    ) -> Result<String> {
        debug!("🗂️ Generating ConfigMap YAML: {}", name);

        let mut data_section = String::new();
        for (key, value) in data {
            data_section.push_str(&format!("  {}: |\n", key));
            for line in value.lines() {
                data_section.push_str(&format!("    {}\n", line));
            }
        }

        let yaml = format!(
            r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    app: ci-pipeline
    type: script-config
data:
{data_section}"#,
            name = name,
            namespace = namespace,
            data_section = data_section,
        );

        Ok(yaml)
    }

    /// Build service account section
    fn build_service_account(config: &KubernetesConfig) -> String {
        if let Some(service_account) = &config.service_account {
            format!("serviceAccountName: {}\n      ", service_account)
        } else {
            String::new()
        }
    }

    /// Build pod security context section
    fn build_pod_security_context(config: &KubernetesConfig) -> String {
        if let Some(security_context) = &config.security_context {
            let mut context = String::from("securityContext:\n");
            
            if let Some(run_as_user) = security_context.run_as_user {
                context.push_str(&format!("        runAsUser: {}\n", run_as_user));
            }
            
            if let Some(run_as_group) = security_context.run_as_group {
                context.push_str(&format!("        runAsGroup: {}\n", run_as_group));
            }
            
            if let Some(run_as_non_root) = security_context.run_as_non_root {
                context.push_str(&format!("        runAsNonRoot: {}\n", run_as_non_root));
            }
            
            if let Some(fs_group) = security_context.fs_group {
                context.push_str(&format!("        fsGroup: {}\n", fs_group));
            }
            
            if let Some(supplemental_groups) = &security_context.supplemental_groups {
                if !supplemental_groups.is_empty() {
                    context.push_str("        supplementalGroups:\n");
                    for group in supplemental_groups {
                        context.push_str(&format!("        - {}\n", group));
                    }
                }
            }
            
            context.push_str("      ");
            context
        } else {
            String::new()
        }
    }

    /// Build container security context section
    fn build_container_security_context(config: &KubernetesConfig) -> String {
        let mut context = String::from("securityContext:\n");
        
        // Default security settings
        context.push_str("          runAsNonRoot: false\n");
        context.push_str("          allowPrivilegeEscalation: false\n");
        context.push_str("          readOnlyRootFilesystem: false\n");
        
        // Override with custom settings if provided
        if let Some(security_context) = &config.security_context {
            if let Some(run_as_user) = security_context.run_as_user {
                context.push_str(&format!("          runAsUser: {}\n", run_as_user));
            }
            
            if let Some(run_as_group) = security_context.run_as_group {
                context.push_str(&format!("          runAsGroup: {}\n", run_as_group));
            }
            
            if let Some(run_as_non_root) = security_context.run_as_non_root {
                // Override the default
                context = context.replace("runAsNonRoot: false", &format!("runAsNonRoot: {}", run_as_non_root));
            }
        }
        
        context
    }

    /// Validate generated YAML syntax (basic check)
    pub fn validate_yaml_syntax(yaml: &str) -> Result<()> {
        // Basic YAML validation - check for common issues
        let lines: Vec<&str> = yaml.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            
            // Check for tabs (YAML doesn't allow tabs for indentation)
            if line.contains('\t') {
                return Err(AppError::ValidationError(
                    format!("YAML contains tab character at line {}", line_num)
                ));
            }
            
            // Check for trailing spaces (can cause issues)
            if line.ends_with(' ') && !line.trim().is_empty() {
                debug!("⚠️ YAML line {} has trailing spaces", line_num);
            }
        }

        // Try to parse as YAML (basic validation)
        match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
            Ok(_) => {
                debug!("✅ YAML syntax validation passed");
                Ok(())
            }
            Err(e) => {
                Err(AppError::ValidationError(
                    format!("Invalid YAML syntax: {}", e)
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{Step, StepConfig, StepType};
    use std::path::PathBuf;

    fn create_test_step() -> Step {
        Step {
            name: "test-step".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig {
                image: Some("ubuntu:latest".to_string()),
                command: Some("echo 'Hello World'".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
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
    fn test_generate_job_name() {
        let name = KubernetesYamlGenerator::generate_job_name("Test Step Name");
        assert!(name.starts_with("ci-test-step-name-"));
        assert!(name.len() > 20); // Should include UUID suffix
    }

    #[test]
    fn test_sanitize_label() {
        assert_eq!(KubernetesYamlGenerator::sanitize_label("test-step"), "test-step");
        assert_eq!(KubernetesYamlGenerator::sanitize_label("Test Step!"), "Test-Step");
        assert_eq!(KubernetesYamlGenerator::sanitize_label("test_step.name"), "test_step.name");
    }

    #[test]
    fn test_escape_yaml_string() {
        assert_eq!(KubernetesYamlGenerator::escape_yaml_string("hello"), "hello");
        assert_eq!(KubernetesYamlGenerator::escape_yaml_string("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(KubernetesYamlGenerator::escape_yaml_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_generate_job_yaml() {
        let step = create_test_step();
        let workspace = create_test_workspace();
        let env = HashMap::new();
        let config = KubernetesConfig::default();

        let yaml = KubernetesYamlGenerator::generate_job_yaml(&step, &workspace, &env, &config);
        assert!(yaml.is_ok());
        
        let yaml_content = yaml.unwrap();
        assert!(yaml_content.contains("kind: Job"));
        assert!(yaml_content.contains("image: ubuntu:latest"));
        assert!(yaml_content.contains("namespace: default"));
    }
}