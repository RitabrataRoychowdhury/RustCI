use crate::ci::{config::Step, workspace::Workspace};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, debug};

#[allow(dead_code)] // Some variants will be used as CI system grows
#[derive(Debug, Clone)]
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

#[async_trait]
pub trait Connector: Send + Sync {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)>;

    #[allow(dead_code)] // Will be used for connector management
    fn connector_type(&self) -> ConnectorType;
    #[allow(dead_code)] // Will be used for connector identification
    fn name(&self) -> &str;
}

pub struct ConnectorManager {
    connectors: HashMap<String, Arc<dyn Connector>>,
}

impl ConnectorManager {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        let mut manager = Self {
            connectors: HashMap::new(),
        };

        // Register built-in connectors
        manager.register_connector(Arc::new(DockerConnector::new()));
        manager.register_connector(Arc::new(KubernetesConnector::new()));
        manager.register_connector(Arc::new(AWSConnector::new()));
        manager.register_connector(Arc::new(AzureConnector::new()));
        manager.register_connector(Arc::new(GCPConnector::new()));
        manager.register_connector(Arc::new(GitHubConnector::new()));
        manager.register_connector(Arc::new(GitLabConnector::new()));

        manager
    }

    #[allow(dead_code)] // Will be used for dynamic connector registration
    pub fn register_connector(&mut self, connector: Arc<dyn Connector>) {
        let name = connector.name().to_string();
        info!("📦 Registering connector: {}", name);
        self.connectors.insert(name, connector);
    }

    pub async fn get_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
        let name = match connector_type {
            ConnectorType::Docker => "docker",
            ConnectorType::Kubernetes => "kubernetes",
            ConnectorType::AWS => "aws",
            ConnectorType::Azure => "azure",
            ConnectorType::GCP => "gcp",
            ConnectorType::GitHub => "github",
            ConnectorType::GitLab => "gitlab",
            ConnectorType::Custom(ref name) => name,
        };

        self.connectors.get(name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Connector not found: {}", name)))
    }

    pub async fn get_custom_connector(&self, name: &str) -> Result<Arc<dyn Connector>> {
        self.connectors.get(name)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Custom connector not found: {}", name)))
    }

    #[allow(dead_code)] // Will be used for connector listing API
    pub fn list_connectors(&self) -> Vec<String> {
        self.connectors.keys().cloned().collect()
    }
}

// Built-in connector implementations

pub struct DockerConnector;

impl DockerConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for DockerConnector {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("🐳 Executing Docker step: {}", step.name);

        if let Some(image) = &step.config.image {
            // Docker run command
            let mut docker_args: Vec<String> = vec![
                "run".into(),
                "--rm".into(),
            ]; // ✅ Change to Vec<String> so we can own formatted strings

            // Add environment variables
            for (key, value) in env {
                docker_args.push("-e".into());
                let env_var = format!("{}={}", key, value); // ✅ Store the formatted string
                docker_args.push(env_var); // ✅ Push owned String (not reference)
            }

            // Mount workspace
            docker_args.push("-v".into());
            let mount_path = format!("{}:/workspace", workspace.path.display()); // ✅ Store formatted mount path
            docker_args.push(mount_path); // ✅ Push owned String
            docker_args.push("-w".into());
            docker_args.push("/workspace".into());

            docker_args.push(image.clone()); // ✅ Push owned image string

            if let Some(command) = &step.config.command {
                docker_args.extend(command.split_whitespace().map(|s| s.to_string())); // ✅ Convert &str to String
            }

            let output = tokio::process::Command::new("docker")
                .args(&docker_args)
                .output()
                .await
                .map_err(|e| AppError::ExternalServiceError(format!("Docker command failed: {}", e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            if !output.status.success() {
                return Err(AppError::ExternalServiceError(format!("Docker command failed: {}", stderr)));
            }

            Ok((exit_code, stdout, stderr))
        } else {
            Err(AppError::ValidationError("Docker step requires image configuration".to_string()))
        }
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Docker
    }

    fn name(&self) -> &str {
        "docker"
    }
}


pub struct KubernetesConnector;

impl KubernetesConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for KubernetesConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace,
        _env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("☸️ Executing Kubernetes step: {}", step.name);
        
        // TODO: Implement Kubernetes operations
        // This would involve kubectl commands or using the Kubernetes API
        
        Err(AppError::InternalServerError("Kubernetes connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Kubernetes
    }

    fn name(&self) -> &str {
        "kubernetes"
    }
}

pub struct AWSConnector;

impl AWSConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for AWSConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace,
        _env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("☁️ Executing AWS step: {}", step.name);
        
        // TODO: Implement AWS operations
        // This would involve AWS CLI commands or using AWS SDK
        
        Err(AppError::InternalServerError("AWS connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::AWS
    }

    fn name(&self) -> &str {
        "aws"
    }
}

pub struct AzureConnector;

impl AzureConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for AzureConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace,
        _env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("🔷 Executing Azure step: {}", step.name);
        
        // TODO: Implement Azure operations
        
        Err(AppError::InternalServerError("Azure connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Azure
    }

    fn name(&self) -> &str {
        "azure"
    }
}

pub struct GCPConnector;

impl GCPConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for GCPConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace,
        _env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("🌐 Executing GCP step: {}", step.name);
        
        // TODO: Implement GCP operations
        
        Err(AppError::InternalServerError("GCP connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::GCP
    }

    fn name(&self) -> &str {
        "gcp"
    }
}

pub struct GitHubConnector;

impl GitHubConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for GitHubConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace, // ✅ underscore to silence warning
        _env: &HashMap<String, String>, // ✅ underscore to silence warning
    ) -> Result<(i32, String, String)> {
        debug!("🐙 Executing GitHub step: {}", step.name);
        
        // TODO: Implement GitHub operations (clone, push, create PR, etc.)
        // This would use the existing GitHub OAuth integration
        
        Err(AppError::InternalServerError("GitHub connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::GitHub
    }

    fn name(&self) -> &str {
        "github"
    }
}


pub struct GitLabConnector;

impl GitLabConnector {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Connector for GitLabConnector {
    async fn execute_step(
        &self,
        step: &Step,
        _workspace: &Workspace,
        _env: &HashMap<String, String>,
    ) -> Result<(i32, String, String)> {
        debug!("🦊 Executing GitLab step: {}", step.name);
        
        // TODO: Implement GitLab operations
        
        Err(AppError::InternalServerError("GitLab connector not yet implemented".to_string()))
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::GitLab
    }

    fn name(&self) -> &str {
        "gitlab"
    }
}