use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

/// Container management for integration tests
pub struct TestContainerManager {
    containers: Vec<TestContainer>,
    networks: Vec<String>,
}

impl TestContainerManager {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            networks: Vec::new(),
        }
    }

    /// Start a MongoDB container for testing
    pub async fn start_mongodb(&mut self) -> Result<TestContainer, ContainerError> {
        let container_name = format!("rustci-test-mongo-{}", Uuid::new_v4());
        let port = self.find_free_port().await?;

        let output = Command::new("docker")
            .args(&[
                "run",
                "-d",
                "--name",
                &container_name,
                "-p",
                &format!("{}:27017", port),
                "-e",
                "MONGO_INITDB_DATABASE=rustci_test",
                "mongo:7",
            ])
            .output()
            .await
            .map_err(|e| ContainerError::StartFailed(format!("Failed to start MongoDB: {}", e)))?;

        if !output.status.success() {
            return Err(ContainerError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let container = TestContainer {
            id: container_id,
            name: container_name,
            image: "mongo:7".to_string(),
            ports: vec![port],
            environment: HashMap::from([(
                "MONGO_INITDB_DATABASE".to_string(),
                "rustci_test".to_string(),
            )]),
            status: ContainerStatus::Starting,
        };

        // Wait for container to be ready
        self.wait_for_container_ready(&container).await?;

        self.containers.push(container.clone());
        Ok(container)
    }

    /// Start a Redis container for testing
    pub async fn start_redis(&mut self) -> Result<TestContainer, ContainerError> {
        let container_name = format!("rustci-test-redis-{}", Uuid::new_v4());
        let port = self.find_free_port().await?;

        let output = Command::new("docker")
            .args(&[
                "run",
                "-d",
                "--name",
                &container_name,
                "-p",
                &format!("{}:6379", port),
                "redis:7-alpine",
            ])
            .output()
            .await
            .map_err(|e| ContainerError::StartFailed(format!("Failed to start Redis: {}", e)))?;

        if !output.status.success() {
            return Err(ContainerError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let container = TestContainer {
            id: container_id,
            name: container_name,
            image: "redis:7-alpine".to_string(),
            ports: vec![port],
            environment: HashMap::new(),
            status: ContainerStatus::Starting,
        };

        self.wait_for_container_ready(&container).await?;

        self.containers.push(container.clone());
        Ok(container)
    }

    /// Start a test application container
    pub async fn start_app_container(
        &mut self,
        image: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<TestContainer, ContainerError> {
        let container_name = format!("rustci-test-app-{}", Uuid::new_v4());
        let port = self.find_free_port().await?;

        let mut args = vec![
            "run",
            "-d",
            "--name",
            &container_name,
            "-p",
            &format!("{}:8080", port),
        ];

        // Add environment variables
        let env_args: Vec<String> = env_vars
            .iter()
            .flat_map(|(k, v)| vec!["-e".to_string(), format!("{}={}", k, v)])
            .collect();

        for arg in &env_args {
            args.push(arg);
        }

        args.push(image);

        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| {
                ContainerError::StartFailed(format!("Failed to start app container: {}", e))
            })?;

        if !output.status.success() {
            return Err(ContainerError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let container = TestContainer {
            id: container_id,
            name: container_name,
            image: image.to_string(),
            ports: vec![port],
            environment: env_vars,
            status: ContainerStatus::Starting,
        };

        self.wait_for_container_ready(&container).await?;

        self.containers.push(container.clone());
        Ok(container)
    }

    /// Create a test network
    pub async fn create_network(&mut self, name: &str) -> Result<(), ContainerError> {
        let output = Command::new("docker")
            .args(&["network", "create", name])
            .output()
            .await
            .map_err(|e| {
                ContainerError::NetworkFailed(format!("Failed to create network: {}", e))
            })?;

        if !output.status.success() {
            return Err(ContainerError::NetworkFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        self.networks.push(name.to_string());
        Ok(())
    }

    /// Connect container to network
    pub async fn connect_to_network(
        &self,
        container: &TestContainer,
        network: &str,
    ) -> Result<(), ContainerError> {
        let output = Command::new("docker")
            .args(&["network", "connect", network, &container.name])
            .output()
            .await
            .map_err(|e| {
                ContainerError::NetworkFailed(format!("Failed to connect to network: {}", e))
            })?;

        if !output.status.success() {
            return Err(ContainerError::NetworkFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(())
    }

    /// Get container logs
    pub async fn get_logs(&self, container: &TestContainer) -> Result<String, ContainerError> {
        let output = Command::new("docker")
            .args(&["logs", &container.name])
            .output()
            .await
            .map_err(|e| ContainerError::LogsFailed(format!("Failed to get logs: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute command in container
    pub async fn exec(
        &self,
        container: &TestContainer,
        command: &[&str],
    ) -> Result<String, ContainerError> {
        let mut args = vec!["exec", &container.name];
        args.extend(command);

        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| ContainerError::ExecFailed(format!("Failed to execute command: {}", e)))?;

        if !output.status.success() {
            return Err(ContainerError::ExecFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Wait for container to be ready
    async fn wait_for_container_ready(
        &self,
        container: &TestContainer,
    ) -> Result<(), ContainerError> {
        let max_attempts = 30;
        let mut attempts = 0;

        while attempts < max_attempts {
            let output = Command::new("docker")
                .args(&["inspect", &container.name, "--format", "{{.State.Status}}"])
                .output()
                .await
                .map_err(|e| {
                    ContainerError::InspectFailed(format!("Failed to inspect container: {}", e))
                })?;

            let status = String::from_utf8_lossy(&output.stdout).trim();

            if status == "running" {
                // Additional health check based on container type
                if container.image.contains("mongo") {
                    if self.check_mongodb_ready(container).await? {
                        return Ok(());
                    }
                } else if container.image.contains("redis") {
                    if self.check_redis_ready(container).await? {
                        return Ok(());
                    }
                } else {
                    // For app containers, check if port is responding
                    if self.check_port_ready(container.ports[0]).await {
                        return Ok(());
                    }
                }
            }

            attempts += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(ContainerError::StartTimeout)
    }

    async fn check_mongodb_ready(&self, container: &TestContainer) -> Result<bool, ContainerError> {
        let result = self
            .exec(container, &["mongosh", "--eval", "db.runCommand('ping')"])
            .await;
        Ok(result.is_ok() && result.unwrap().contains("ok"))
    }

    async fn check_redis_ready(&self, container: &TestContainer) -> Result<bool, ContainerError> {
        let result = self.exec(container, &["redis-cli", "ping"]).await;
        Ok(result.is_ok() && result.unwrap().trim() == "PONG")
    }

    async fn check_port_ready(&self, port: u16) -> bool {
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
    }

    async fn find_free_port(&self) -> Result<u16, ContainerError> {
        for port in 30000..40000 {
            if tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .is_ok()
            {
                return Ok(port);
            }
        }
        Err(ContainerError::NoFreePort)
    }

    /// Cleanup all containers and networks
    pub async fn cleanup(&mut self) {
        // Stop and remove containers
        for container in &self.containers {
            let _ = Command::new("docker")
                .args(&["stop", &container.name])
                .output()
                .await;

            let _ = Command::new("docker")
                .args(&["rm", &container.name])
                .output()
                .await;
        }

        // Remove networks
        for network in &self.networks {
            let _ = Command::new("docker")
                .args(&["network", "rm", network])
                .output()
                .await;
        }

        self.containers.clear();
        self.networks.clear();
    }
}

impl Drop for TestContainerManager {
    fn drop(&mut self) {
        // Cleanup in background - this is best effort
        let containers = self.containers.clone();
        let networks = self.networks.clone();

        tokio::spawn(async move {
            for container in containers {
                let _ = Command::new("docker")
                    .args(&["stop", &container.name])
                    .output()
                    .await;

                let _ = Command::new("docker")
                    .args(&["rm", &container.name])
                    .output()
                    .await;
            }

            for network in networks {
                let _ = Command::new("docker")
                    .args(&["network", "rm", &network])
                    .output()
                    .await;
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct TestContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub ports: Vec<u16>,
    pub environment: HashMap<String, String>,
    pub status: ContainerStatus,
}

impl TestContainer {
    pub fn get_connection_string(&self, database: &str) -> String {
        if self.image.contains("mongo") {
            format!("mongodb://localhost:{}/{}", self.ports[0], database)
        } else if self.image.contains("redis") {
            format!("redis://localhost:{}", self.ports[0])
        } else {
            format!("http://localhost:{}", self.ports[0])
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContainerStatus {
    Starting,
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("Failed to start container: {0}")]
    StartFailed(String),

    #[error("Container start timeout")]
    StartTimeout,

    #[error("Network operation failed: {0}")]
    NetworkFailed(String),

    #[error("Failed to get logs: {0}")]
    LogsFailed(String),

    #[error("Failed to execute command: {0}")]
    ExecFailed(String),

    #[error("Failed to inspect container: {0}")]
    InspectFailed(String),

    #[error("No free port available")]
    NoFreePort,
}

/// Docker Compose helper for complex test scenarios
pub struct DockerComposeHelper {
    compose_file: String,
    project_name: String,
}

impl DockerComposeHelper {
    pub fn new(compose_file: &str) -> Self {
        Self {
            compose_file: compose_file.to_string(),
            project_name: format!("rustci-test-{}", Uuid::new_v4()),
        }
    }

    pub async fn up(&self) -> Result<(), ContainerError> {
        let output = Command::new("docker-compose")
            .args(&[
                "-f",
                &self.compose_file,
                "-p",
                &self.project_name,
                "up",
                "-d",
            ])
            .output()
            .await
            .map_err(|e| ContainerError::StartFailed(format!("Docker compose up failed: {}", e)))?;

        if !output.status.success() {
            return Err(ContainerError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(())
    }

    pub async fn down(&self) -> Result<(), ContainerError> {
        let output = Command::new("docker-compose")
            .args(&[
                "-f",
                &self.compose_file,
                "-p",
                &self.project_name,
                "down",
                "-v",
            ])
            .output()
            .await
            .map_err(|e| {
                ContainerError::StartFailed(format!("Docker compose down failed: {}", e))
            })?;

        if !output.status.success() {
            return Err(ContainerError::StartFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(())
    }

    pub async fn logs(&self, service: &str) -> Result<String, ContainerError> {
        let output = Command::new("docker-compose")
            .args(&[
                "-f",
                &self.compose_file,
                "-p",
                &self.project_name,
                "logs",
                service,
            ])
            .output()
            .await
            .map_err(|e| {
                ContainerError::LogsFailed(format!("Failed to get compose logs: {}", e))
            })?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
