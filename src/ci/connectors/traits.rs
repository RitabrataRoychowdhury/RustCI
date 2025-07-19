use crate::ci::{config::Step, workspace::Workspace};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Represents the result of a connector execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    pub fn new(exit_code: i32, stdout: String, stderr: String) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn success(stdout: String) -> Self {
        Self::new(0, stdout, String::new())
    }

    pub fn failure(exit_code: i32, stderr: String) -> Self {
        Self::new(exit_code, String::new(), stderr)
    }

    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Enum representing different types of connectors
#[allow(dead_code)] // Some variants will be used as CI system grows
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorType {
    Docker,
    Kubernetes,
    AWS,
    Azure,
    GCP,
    GitHub,
    GitLab,
    Custom(String),
}

impl std::fmt::Display for ConnectorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorType::Docker => write!(f, "docker"),
            ConnectorType::Kubernetes => write!(f, "kubernetes"),
            ConnectorType::AWS => write!(f, "aws"),
            ConnectorType::Azure => write!(f, "azure"),
            ConnectorType::GCP => write!(f, "gcp"),
            ConnectorType::GitHub => write!(f, "github"),
            ConnectorType::GitLab => write!(f, "gitlab"),
            ConnectorType::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Core trait that all connectors must implement
#[async_trait]
pub trait Connector: Send + Sync {
    /// Execute a step using this connector
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult>;

    /// Get the connector type
    fn connector_type(&self) -> ConnectorType;

    /// Get the connector name
    fn name(&self) -> &str;

    /// Validate the step configuration for this connector
    fn validate_config(&self, step: &Step) -> Result<()> {
        debug!("🔍 Validating config for connector: {}", self.name());

        if step.name.is_empty() {
            return Err(AppError::ValidationError(
                "Step name cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Pre-execution hook (optional)
    async fn pre_execute(&self, step: &Step) -> Result<()> {
        debug!("🚀 Pre-execution hook for step: {}", step.name);
        Ok(())
    }

    /// Post-execution hook (optional)
    async fn post_execute(&self, step: &Step, result: &ExecutionResult) -> Result<()> {
        debug!(
            "🏁 Post-execution hook for step: {} (exit_code: {})",
            step.name, result.exit_code
        );
        Ok(())
    }
}

/// Extended step configuration for Kubernetes-specific fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub namespace: String,
    pub timeout_seconds: u64,
    pub use_hostpath: bool,
    pub resource_requests: Option<HashMap<String, String>>,
    pub resource_limits: Option<HashMap<String, String>>,
    pub repo_url: Option<String>,
    /// Use PersistentVolumeClaims instead of hostPath for cloud compatibility
    pub use_pvc: bool,
    /// PVC storage class for dynamic provisioning
    pub storage_class: Option<String>,
    /// PVC storage size (e.g., "10Gi")
    pub storage_size: Option<String>,
    /// Pre-execution lifecycle hooks
    pub pre_hooks: Vec<LifecycleHook>,
    /// Post-execution lifecycle hooks  
    pub post_hooks: Vec<LifecycleHook>,
    /// Service account for RBAC permissions
    pub service_account: Option<String>,
    /// Security context settings
    pub security_context: Option<SecurityContext>,
}

/// Lifecycle hook configuration for MongoDB integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleHook {
    pub name: String,
    pub hook_type: LifecycleHookType,
    pub mongodb_collection: String,
    pub mongodb_operation: MongoOperation,
    pub data: HashMap<String, String>,
}

/// Types of lifecycle hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LifecycleHookType {
    PreExecution,
    PostExecution,
    OnSuccess,
    OnFailure,
}

/// MongoDB operations for lifecycle hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MongoOperation {
    Insert,
    Update,
    Delete,
    FindAndUpdate,
}

/// Security context configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub run_as_user: Option<u64>,
    pub run_as_group: Option<u64>,
    pub run_as_non_root: Option<bool>,
    pub fs_group: Option<u64>,
    pub supplemental_groups: Option<Vec<u64>>,
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            timeout_seconds: 180,
            use_hostpath: true,
            resource_requests: None,
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            service_account: None,
            security_context: None,
        }
    }
}

impl KubernetesConfig {
    pub fn from_step(step: &Step) -> Self {
        // Extract resource requests and limits from step parameters
        let resource_requests = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("resource_requests"))
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });

        let resource_limits = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("resource_limits"))
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });

        // Extract PVC configuration
        let use_pvc = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("use_pvc"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let storage_class = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("storage_class"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let storage_size = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("storage_size"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some("10Gi".to_string()));

        // Extract service account
        let service_account = step
            .config
            .parameters
            .as_ref()
            .and_then(|params| params.get("service_account"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Self {
            namespace: step
                .config
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            timeout_seconds: step.timeout.unwrap_or(180),
            use_hostpath: !use_pvc, // Use hostpath if PVC is not enabled
            resource_requests,
            resource_limits,
            repo_url: step.config.repository_url.clone(),
            use_pvc,
            storage_class,
            storage_size,
            pre_hooks: Vec::new(), // Will be populated from step config in future
            post_hooks: Vec::new(), // Will be populated from step config in future
            service_account,
            security_context: None, // Will be populated from step config in future
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.namespace.is_empty() {
            return Err(AppError::ValidationError(
                "Kubernetes namespace cannot be empty".to_string(),
            ));
        }

        if self.timeout_seconds == 0 {
            return Err(AppError::ValidationError(
                "Kubernetes timeout must be greater than 0".to_string(),
            ));
        }

        // Validate resource requests format
        if let Some(requests) = &self.resource_requests {
            self.validate_resource_format(requests, "requests")?;
        }

        // Validate resource limits format
        if let Some(limits) = &self.resource_limits {
            self.validate_resource_format(limits, "limits")?;
        }

        // Validate PVC configuration
        if self.use_pvc {
            if let Some(size) = &self.storage_size {
                if !self.is_valid_storage_size(size) {
                    return Err(AppError::ValidationError(format!(
                        "Invalid storage size format: {}",
                        size
                    )));
                }
            }
        }

        // Validate lifecycle hooks
        for hook in &self.pre_hooks {
            self.validate_lifecycle_hook(hook)?;
        }
        for hook in &self.post_hooks {
            self.validate_lifecycle_hook(hook)?;
        }

        Ok(())
    }

    fn validate_resource_format(
        &self,
        resources: &HashMap<String, String>,
        resource_type: &str,
    ) -> Result<()> {
        for (key, value) in resources {
            match key.as_str() {
                "cpu" => {
                    if !self.is_valid_cpu_format(value) {
                        return Err(AppError::ValidationError(format!(
                            "Invalid CPU {} format: {}",
                            resource_type, value
                        )));
                    }
                }
                "memory" => {
                    if !self.is_valid_memory_format(value) {
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

    fn is_valid_cpu_format(&self, cpu: &str) -> bool {
        if cpu.ends_with('m') {
            cpu[..cpu.len() - 1].parse::<u32>().is_ok()
        } else {
            cpu.parse::<f64>().is_ok()
        }
    }

    fn is_valid_memory_format(&self, memory: &str) -> bool {
        let suffixes = [
            "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "K", "M", "G", "T", "P", "E",
        ];

        for suffix in &suffixes {
            if memory.ends_with(suffix) {
                return memory[..memory.len() - suffix.len()].parse::<u64>().is_ok();
            }
        }

        memory.parse::<u64>().is_ok()
    }

    fn is_valid_storage_size(&self, size: &str) -> bool {
        self.is_valid_memory_format(size)
    }

    fn validate_lifecycle_hook(&self, hook: &LifecycleHook) -> Result<()> {
        if hook.name.is_empty() {
            return Err(AppError::ValidationError(
                "Lifecycle hook name cannot be empty".to_string(),
            ));
        }

        if hook.mongodb_collection.is_empty() {
            return Err(AppError::ValidationError(
                "Lifecycle hook MongoDB collection cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}
