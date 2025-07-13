use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use tracing::{info, debug, error, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub deployment_type: DeploymentType,
    pub target_directory: Option<PathBuf>,
    pub docker_config: Option<DockerDeploymentConfig>,
    pub port_mappings: Vec<PortMapping>,
    pub environment_variables: HashMap<String, String>,
    pub health_check: Option<HealthCheckConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentType {
    LocalDirectory,
    DockerContainer,
    LocalService,
    Hybrid, // Both directory and container
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerDeploymentConfig {
    pub image_name: String,
    pub dockerfile_path: Option<String>,
    pub build_context: Option<String>,
    pub base_image: Option<String>,
    pub distroless: bool,
    pub registry: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String, // tcp, udp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub endpoint: String,
    pub timeout_seconds: u32,
    pub retries: u32,
    pub interval_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct DeploymentResult {
    pub deployment_id: Uuid,
    pub deployment_type: DeploymentType,
    pub status: DeploymentStatus,
    pub artifacts: Vec<DeploymentArtifact>,
    pub services: Vec<DeployedService>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    Pending,
    Building,
    Deploying,
    Running,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentArtifact {
    pub name: String,
    pub path: PathBuf,
    pub artifact_type: ArtifactType,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Binary,
    Archive,
    DockerImage,
    StaticFiles,
    Configuration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedService {
    pub name: String,
    pub service_type: ServiceType,
    pub endpoint: String,
    pub ports: Vec<PortMapping>,
    pub container_id: Option<String>,
    pub process_id: Option<u32>,
    pub status: ServiceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    WebServer,
    ApiServer,
    Database,
    Cache,
    MessageQueue,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

pub struct LocalDeploymentManager {
    base_deployment_dir: PathBuf,
    docker_registry: Option<String>,
    port_manager: PortManager,
}

impl LocalDeploymentManager {
    pub fn new(base_deployment_dir: PathBuf) -> Self {
        Self {
            base_deployment_dir,
            docker_registry: None,
            port_manager: PortManager::new(),
        }
    }

    pub async fn deploy(
        &mut self,
        execution_id: Uuid,
        workspace_path: &Path,
        config: &DeploymentConfig,
    ) -> Result<DeploymentResult> {
        info!("🚀 Starting deployment for execution: {}", execution_id);

        let deployment_id = Uuid::new_v4();
        let mut result = DeploymentResult {
            deployment_id,
            deployment_type: config.deployment_type.clone(),
            status: DeploymentStatus::Pending,
            artifacts: Vec::new(),
            services: Vec::new(),
            logs: Vec::new(),
        };

        match &config.deployment_type {
            DeploymentType::LocalDirectory => {
                self.deploy_to_directory(execution_id, workspace_path, config, &mut result).await?;
            }
            DeploymentType::DockerContainer => {
                self.deploy_to_docker(execution_id, workspace_path, config, &mut result).await?;
            }
            DeploymentType::LocalService => {
                self.deploy_as_local_service(execution_id, workspace_path, config, &mut result).await?;
            }
            DeploymentType::Hybrid => {
                self.deploy_to_directory(execution_id, workspace_path, config, &mut result).await?;
                self.deploy_to_docker(execution_id, workspace_path, config, &mut result).await?;
            }
        }

        info!("✅ Deployment completed: {} with status: {:?}", deployment_id, result.status);
        Ok(result)
    }

    async fn deploy_to_directory(
        &self,
        execution_id: Uuid,
        workspace_path: &Path,
        config: &DeploymentConfig,
        result: &mut DeploymentResult,
    ) -> Result<()> {
        info!("📁 Deploying to local directory");
        result.status = DeploymentStatus::Deploying;

        let target_dir = config.target_directory
            .clone()
            .unwrap_or_else(|| self.base_deployment_dir.join(format!("deployment-{}", execution_id)));

        // Create deployment directory
        fs::create_dir_all(&target_dir).await
            .map_err(|e| AppError::InternalServerError(format!("Failed to create deployment directory: {}", e)))?;

        // Copy built artifacts
        self.copy_artifacts(workspace_path, &target_dir, result).await?;

        // Create deployment metadata
        self.create_deployment_metadata(&target_dir, execution_id, config).await?;

        result.status = DeploymentStatus::Running;
        result.logs.push(format!("Successfully deployed to directory: {}", target_dir.display()));

        Ok(())
    }

    async fn deploy_to_docker(
        &self,
        execution_id: Uuid,
        workspace_path: &Path,
        config: &DeploymentConfig,
        result: &mut DeploymentResult,
    ) -> Result<()> {
        info!("🐳 Deploying to Docker container");
        result.status = DeploymentStatus::Building;

        let docker_config = config.docker_config.as_ref()
            .ok_or_else(|| AppError::ValidationError("Docker deployment requires docker_config".to_string()))?;

        // Build Docker image
        let image_name = self.build_docker_image(execution_id, workspace_path, docker_config, result).await?;

        // Run Docker container
        let container_id = self.run_docker_container(&image_name, config, result).await?;

        // Create deployed service record
        let service = DeployedService {
            name: docker_config.image_name.clone(),
            service_type: ServiceType::WebServer, // Default, should be configurable
            endpoint: self.generate_service_endpoint(&config.port_mappings),
            ports: config.port_mappings.clone(),
            container_id: Some(container_id),
            process_id: None,
            status: ServiceStatus::Running,
        };

        result.services.push(service);
        result.status = DeploymentStatus::Running;

        Ok(())
    }

    async fn deploy_as_local_service(
        &self,
        execution_id: Uuid,
        workspace_path: &Path,
        config: &DeploymentConfig,
        result: &mut DeploymentResult,
    ) -> Result<()> {
        info!("⚙️ Deploying as local service");
        result.status = DeploymentStatus::Deploying;

        // This would start the application directly on the host
        // Implementation depends on the project type
        let project_type = self.detect_project_type(workspace_path).await?;
        
        match project_type {
            ProjectType::NodeJs => {
                self.start_nodejs_service(workspace_path, config, result).await?;
            }
            ProjectType::Python => {
                self.start_python_service(workspace_path, config, result).await?;
            }
            ProjectType::Rust => {
                self.start_rust_service(workspace_path, config, result).await?;
            }
            ProjectType::Java => {
                self.start_java_service(workspace_path, config, result).await?;
            }
            ProjectType::Static => {
                // For static sites, we might use a simple HTTP server
                self.start_static_service(workspace_path, config, result).await?;
            }
        }

        result.status = DeploymentStatus::Running;
        Ok(())
    }

    async fn build_docker_image(
        &self,
        execution_id: Uuid,
        workspace_path: &Path,
        docker_config: &DockerDeploymentConfig,
        result: &mut DeploymentResult,
    ) -> Result<String> {
        let image_name = format!("{}:{}", docker_config.image_name, execution_id);
        
        // Check if Dockerfile exists, if not create one based on project type
        let dockerfile_path = if let Some(dockerfile) = &docker_config.dockerfile_path {
            workspace_path.join(dockerfile)
        } else {
            let generated_dockerfile = self.generate_dockerfile(workspace_path, docker_config).await?;
            workspace_path.join("Dockerfile.generated")
        };

        let build_context = docker_config.build_context
            .as_ref()
            .map(|ctx| workspace_path.join(ctx))
            .unwrap_or_else(|| workspace_path.to_path_buf());

        info!("🔨 Building Docker image: {}", image_name);

        let output = Command::new("docker")
            .args([
                "build",
                "-t", &image_name,
                "-f", dockerfile_path.to_str().unwrap(),
                build_context.to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to execute docker build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            result.logs.push(format!("Docker build failed: {}", stderr));
            return Err(AppError::ExternalServiceError(format!("Docker build failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        result.logs.push(format!("Docker build output: {}", stdout));

        // Add image as artifact
        result.artifacts.push(DeploymentArtifact {
            name: image_name.clone(),
            path: PathBuf::from("docker://".to_string() + &image_name),
            artifact_type: ArtifactType::DockerImage,
            size_bytes: 0, // Would need to inspect image for actual size
        });

        Ok(image_name)
    }

    async fn run_docker_container(
        &mut self,
        image_name: &str,
        config: &DeploymentConfig,
        result: &mut DeploymentResult,
    ) -> Result<String> {
        info!("🏃 Running Docker container from image: {}", image_name);

        let container_name = format!("ci-deployment-{}", Uuid::new_v4());
        let mut docker_args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            container_name.clone(),
        ];

        // Add port mappings
        for port_mapping in &config.port_mappings {
            // Allocate host port if not specified
            let host_port = if port_mapping.host_port == 0 {
                self.port_manager.allocate_port()?
            } else {
                port_mapping.host_port
            };

            docker_args.push("-p".to_string());
            docker_args.push(format!("{}:{}", host_port, port_mapping.container_port));
        }

        // Add environment variables
        for (key, value) in &config.environment_variables {
            docker_args.push("-e".to_string());
            docker_args.push(format!("{}={}", key, value));
        }

        docker_args.push(image_name.to_string());

        let output = Command::new("docker")
            .args(&docker_args)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to run docker container: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            result.logs.push(format!("Docker run failed: {}", stderr));
            return Err(AppError::ExternalServiceError(format!("Docker run failed: {}", stderr)));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        result.logs.push(format!("Container started with ID: {}", container_id));

        Ok(container_id)
    }

    async fn copy_artifacts(
        &self,
        source_path: &Path,
        target_path: &Path,
        result: &mut DeploymentResult,
    ) -> Result<()> {
        info!("📦 Copying artifacts from {} to {}", source_path.display(), target_path.display());

        // Common build output directories to copy
        let artifact_patterns = [
            "dist/",
            "build/",
            "target/release/",
            "target/debug/",
            "out/",
            "public/",
            "static/",
            "*.jar",
            "*.war",
            "*.tar.gz",
            "*.zip",
        ];

        for pattern in &artifact_patterns {
            let source_pattern = source_path.join(pattern);
            if source_pattern.exists() {
                let target_artifact = target_path.join(pattern);
                
                if source_pattern.is_dir() {
                    self.copy_directory(&source_pattern, &target_artifact).await?;
                } else {
                    if let Some(parent) = target_artifact.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    fs::copy(&source_pattern, &target_artifact).await?;
                }

                // Record artifact
                let metadata = fs::metadata(&target_artifact).await?;
                result.artifacts.push(DeploymentArtifact {
                    name: pattern.to_string(),
                    path: target_artifact,
                    artifact_type: if source_pattern.is_dir() { 
                        ArtifactType::StaticFiles 
                    } else { 
                        ArtifactType::Archive 
                    },
                    size_bytes: metadata.len(),
                });
            }
        }

        Ok(())
    }

    async fn copy_directory(&self, source: &Path, target: &Path) -> Result<()> {
        fs::create_dir_all(target).await?;
        
        let mut entries = fs::read_dir(source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());
            
            if source_path.is_dir() {
                self.copy_directory(&source_path, &target_path).await?;
            } else {
                fs::copy(&source_path, &target_path).await?;
            }
        }
        
        Ok(())
    }

    async fn detect_project_type(&self, workspace_path: &Path) -> Result<ProjectType> {
        if workspace_path.join("package.json").exists() {
            Ok(ProjectType::NodeJs)
        } else if workspace_path.join("requirements.txt").exists() || workspace_path.join("pyproject.toml").exists() {
            Ok(ProjectType::Python)
        } else if workspace_path.join("Cargo.toml").exists() {
            Ok(ProjectType::Rust)
        } else if workspace_path.join("pom.xml").exists() || workspace_path.join("build.gradle").exists() {
            Ok(ProjectType::Java)
        } else if workspace_path.join("index.html").exists() {
            Ok(ProjectType::Static)
        } else {
            Err(AppError::ValidationError("Unable to detect project type".to_string()))
        }
    }

    async fn generate_dockerfile(&self, workspace_path: &Path, docker_config: &DockerDeploymentConfig) -> Result<String> {
        let project_type = self.detect_project_type(workspace_path).await?;
        
        let dockerfile_content = match project_type {
            ProjectType::NodeJs => self.generate_nodejs_dockerfile(docker_config),
            ProjectType::Python => self.generate_python_dockerfile(docker_config),
            ProjectType::Rust => self.generate_rust_dockerfile(docker_config),
            ProjectType::Java => self.generate_java_dockerfile(docker_config),
            ProjectType::Static => self.generate_static_dockerfile(docker_config),
        };

        let dockerfile_path = workspace_path.join("Dockerfile.generated");
        fs::write(&dockerfile_path, dockerfile_content).await?;
        
        Ok(dockerfile_path.to_string_lossy().to_string())
    }

    fn generate_nodejs_dockerfile(&self, config: &DockerDeploymentConfig) -> String {
        let base_image = config.base_image.as_deref().unwrap_or("node:18-alpine");
        
        if config.distroless {
            format!(r#"
# Build stage
FROM {base_image} AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production

# Production stage
FROM gcr.io/distroless/nodejs18-debian11
WORKDIR /app
COPY --from=builder /app/node_modules ./node_modules
COPY . .
EXPOSE 3000
CMD ["index.js"]
"#, base_image = base_image)
        } else {
            format!(r#"
FROM {base_image}
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
EXPOSE 3000
CMD ["npm", "start"]
"#, base_image = base_image)
        }
    }

    fn generate_python_dockerfile(&self, config: &DockerDeploymentConfig) -> String {
        let base_image = config.base_image.as_deref().unwrap_or("python:3.11-slim");
        
        format!(r#"
FROM {base_image}
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .
EXPOSE 8000
CMD ["python", "app.py"]
"#, base_image = base_image)
    }

    fn generate_rust_dockerfile(&self, config: &DockerDeploymentConfig) -> String {
        let base_image = config.base_image.as_deref().unwrap_or("rust:1.70");
        
        if config.distroless {
            format!(r#"
# Build stage
FROM {base_image} AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Production stage
FROM gcr.io/distroless/cc-debian11
WORKDIR /app
COPY --from=builder /app/target/release/* ./
EXPOSE 8000
CMD ["./app"]
"#, base_image = base_image)
        } else {
            format!(r#"
FROM {base_image}
WORKDIR /app
COPY . .
RUN cargo build --release
EXPOSE 8000
CMD ["./target/release/app"]
"#, base_image = base_image)
        }
    }

    fn generate_java_dockerfile(&self, config: &DockerDeploymentConfig) -> String {
        let base_image = config.base_image.as_deref().unwrap_or("openjdk:17-jdk-slim");
        
        format!(r#"
FROM {base_image}
WORKDIR /app
COPY . .
RUN ./mvnw clean package -DskipTests
EXPOSE 8080
CMD ["java", "-jar", "target/*.jar"]
"#, base_image = base_image)
    }

    fn generate_static_dockerfile(&self, config: &DockerDeploymentConfig) -> String {
        let base_image = config.base_image.as_deref().unwrap_or("nginx:alpine");
        
        format!(r#"
FROM {base_image}
COPY . /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
"#, base_image = base_image)
    }

    async fn start_nodejs_service(&self, workspace_path: &Path, config: &DeploymentConfig, result: &mut DeploymentResult) -> Result<()> {
        info!("🟢 Starting Node.js service");
        // Implementation for starting Node.js service locally
        // This would use npm start or node directly
        Ok(())
    }

    async fn start_python_service(&self, workspace_path: &Path, config: &DeploymentConfig, result: &mut DeploymentResult) -> Result<()> {
        info!("🐍 Starting Python service");
        // Implementation for starting Python service locally
        Ok(())
    }

    async fn start_rust_service(&self, workspace_path: &Path, config: &DeploymentConfig, result: &mut DeploymentResult) -> Result<()> {
        info!("🦀 Starting Rust service");
        // Implementation for starting Rust service locally
        Ok(())
    }

    async fn start_java_service(&self, workspace_path: &Path, config: &DeploymentConfig, result: &mut DeploymentResult) -> Result<()> {
        info!("☕ Starting Java service");
        // Implementation for starting Java service locally
        Ok(())
    }

    async fn start_static_service(&self, workspace_path: &Path, config: &DeploymentConfig, result: &mut DeploymentResult) -> Result<()> {
        info!("📄 Starting static file service");
        // Implementation for serving static files
        Ok(())
    }

    async fn create_deployment_metadata(&self, target_dir: &Path, execution_id: Uuid, config: &DeploymentConfig) -> Result<()> {
        let metadata = serde_json::json!({
            "execution_id": execution_id,
            "deployment_type": config.deployment_type,
            "deployed_at": chrono::Utc::now(),
            "port_mappings": config.port_mappings,
            "environment_variables": config.environment_variables
        });

        let metadata_path = target_dir.join("deployment-metadata.json");
        fs::write(metadata_path, serde_json::to_string_pretty(&metadata)?).await?;
        
        Ok(())
    }

    fn generate_service_endpoint(&self, port_mappings: &[PortMapping]) -> String {
        if let Some(first_port) = port_mappings.first() {
            format!("http://localhost:{}", first_port.host_port)
        } else {
            "http://localhost".to_string()
        }
    }
}

#[derive(Debug, Clone)]
enum ProjectType {
    NodeJs,
    Python,
    Rust,
    Java,
    Static,
}

pub struct PortManager {
    allocated_ports: std::collections::HashSet<u16>,
    next_port: u16,
}

impl PortManager {
    pub fn new() -> Self {
        Self {
            allocated_ports: std::collections::HashSet::new(),
            next_port: 8000,
        }
    }

    pub fn allocate_port(&mut self) -> Result<u16> {
        for _ in 0..1000 { // Try up to 1000 ports
            if !self.allocated_ports.contains(&self.next_port) {
                let port = self.next_port;
                self.allocated_ports.insert(port);
                self.next_port += 1;
                return Ok(port);
            }
            self.next_port += 1;
            if self.next_port > 65535 {
                self.next_port = 8000;
            }
        }
        Err(AppError::InternalServerError("No available ports".to_string()))
    }

    pub fn release_port(&mut self, port: u16) {
        self.allocated_ports.remove(&port);
    }
}