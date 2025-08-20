//! Runner Discovery and Registration System
//!
//! This module provides automatic runner discovery across the network,
//! node capability advertisement, and secure runner authentication for cluster joining.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, info};
use uuid::Uuid;

use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::networking::transport::NetworkConditions;
use crate::core::networking::transport::Transport;
use crate::domain::entities::{ClusterNode, NodeId, NodeRole, NodeStatus, RunnerType};
use crate::error::{AppError, Result};

/// Runner discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerDiscoveryConfig {
    /// Discovery interval in seconds
    pub discovery_interval: u64,
    /// Discovery timeout in seconds
    pub discovery_timeout: u64,
    /// Network broadcast address for discovery
    pub broadcast_address: String,
    /// Discovery port
    pub discovery_port: u16,
    /// Authentication token for secure joining
    pub auth_token: String,
    /// Maximum number of discovery attempts
    pub max_discovery_attempts: u32,
    /// Enable automatic registration
    pub auto_register: bool,
    /// Discovery method
    pub discovery_method: DiscoveryMethod,
}

impl Default for RunnerDiscoveryConfig {
    fn default() -> Self {
        Self {
            discovery_interval: 30,
            discovery_timeout: 10,
            broadcast_address: "255.255.255.255".to_string(),
            discovery_port: 8765,
            auth_token: "default-token".to_string(),
            max_discovery_attempts: 3,
            auto_register: true,
            discovery_method: DiscoveryMethod::Broadcast,
        }
    }
}

/// Discovery methods supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// UDP broadcast discovery
    Broadcast,
    /// Multicast discovery
    Multicast { group: String },
    /// DNS-based service discovery
    Dns { domain: String },
    /// Static node list
    Static { nodes: Vec<SocketAddr> },
    /// Consul-based discovery
    Consul { endpoint: String },
}

/// Runner capability advertisement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerCapabilities {
    /// Supported runner types
    pub supported_types: Vec<RunnerType>,
    /// Available CPU cores
    pub cpu_cores: u32,
    /// Available memory in MB
    pub memory_mb: u32,
    /// Available disk space in MB
    pub disk_mb: u32,
    /// Supported architectures
    pub architectures: Vec<String>,
    /// Operating system
    pub os: String,
    /// Kernel version
    pub kernel_version: String,
    /// Container runtime versions
    pub container_runtimes: HashMap<String, String>,
    /// Network interfaces
    pub network_interfaces: Vec<NetworkInterface>,
    /// Custom capabilities
    pub custom_capabilities: HashMap<String, String>,
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name
    pub name: String,
    /// IP address
    pub ip_address: String,
    /// MAC address
    pub mac_address: String,
    /// Interface type
    pub interface_type: String,
}

/// Discovery message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMessage {
    /// Announce runner availability
    Announce {
        node_id: NodeId,
        capabilities: RunnerCapabilities,
        endpoint: SocketAddr,
        auth_token: String,
        timestamp: DateTime<Utc>,
    },
    /// Request runner information
    Discover {
        requester_id: NodeId,
        auth_token: String,
        timestamp: DateTime<Utc>,
    },
    /// Response to discovery request
    Response {
        node_id: NodeId,
        capabilities: RunnerCapabilities,
        endpoint: SocketAddr,
        auth_token: String,
        timestamp: DateTime<Utc>,
    },
    /// Registration request
    RegisterRequest {
        node_id: NodeId,
        capabilities: RunnerCapabilities,
        endpoint: SocketAddr,
        auth_token: String,
        timestamp: DateTime<Utc>,
    },
    /// Registration response
    RegisterResponse {
        accepted: bool,
        node_id: NodeId,
        cluster_config: Option<ClusterConfig>,
        error_message: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

/// Cluster configuration provided during registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Cluster ID
    pub cluster_id: Uuid,
    /// Control plane endpoints
    pub control_plane_endpoints: Vec<SocketAddr>,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Node configuration
    pub node_config: HashMap<String, String>,
}

/// Discovered runner information
#[derive(Debug, Clone)]
pub struct DiscoveredRunner {
    /// Node ID
    pub node_id: NodeId,
    /// Network endpoint
    pub endpoint: SocketAddr,
    /// Runner capabilities
    pub capabilities: RunnerCapabilities,
    /// Discovery timestamp
    pub discovered_at: DateTime<Utc>,
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    /// Authentication status
    pub authenticated: bool,
    /// Registration status
    pub registered: bool,
}

/// Runner discovery service trait
#[async_trait]
pub trait RunnerDiscoveryService: Send + Sync {
    /// Start the discovery service
    async fn start(&self) -> Result<()>;

    /// Stop the discovery service
    async fn stop(&self) -> Result<()>;

    /// Announce this runner's availability
    async fn announce(&self, capabilities: RunnerCapabilities) -> Result<()>;

    /// Discover available runners
    async fn discover(&self) -> Result<Vec<DiscoveredRunner>>;

    /// Register a discovered runner
    async fn register_runner(&self, runner: &DiscoveredRunner) -> Result<ClusterNode>;

    /// Get discovered runners
    async fn get_discovered_runners(&self) -> Result<Vec<DiscoveredRunner>>;

    /// Authenticate a runner
    async fn authenticate_runner(&self, node_id: NodeId, auth_token: &str) -> Result<bool>;
}

/// Default implementation of runner discovery service
pub struct DefaultRunnerDiscoveryService {
    /// Configuration
    config: RunnerDiscoveryConfig,
    /// Node registry for registration
    node_registry: Arc<NodeRegistry>,
    /// Transport for communication
    transport: Arc<dyn Transport>,
    /// Discovered runners cache
    discovered_runners: Arc<RwLock<HashMap<NodeId, DiscoveredRunner>>>,
    /// Running state
    running: Arc<Mutex<bool>>,
    /// Local node ID
    local_node_id: NodeId,
    /// Local capabilities
    local_capabilities: Arc<RwLock<Option<RunnerCapabilities>>>,
}

impl DefaultRunnerDiscoveryService {
    /// Create a new discovery service
    pub fn new(
        config: RunnerDiscoveryConfig,
        node_registry: Arc<NodeRegistry>,
        transport: Arc<dyn Transport>,
    ) -> Self {
        Self {
            config,
            node_registry,
            transport,
            discovered_runners: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            local_node_id: Uuid::new_v4(),
            local_capabilities: Arc::new(RwLock::new(None)),
        }
    }

    /// Detect local runner capabilities
    async fn detect_local_capabilities(&self) -> Result<RunnerCapabilities> {
        let mut capabilities = RunnerCapabilities {
            supported_types: vec![RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            }],
            cpu_cores: num_cpus::get() as u32,
            memory_mb: self.get_total_memory().await?,
            disk_mb: self.get_available_disk_space().await?,
            architectures: vec![std::env::consts::ARCH.to_string()],
            os: std::env::consts::OS.to_string(),
            kernel_version: self.get_kernel_version().await?,
            container_runtimes: HashMap::new(),
            network_interfaces: self.get_network_interfaces().await?,
            custom_capabilities: HashMap::new(),
        };

        // Check for Docker availability
        if crate::infrastructure::runners::health_checks::is_docker_available().await {
            capabilities.supported_types.push(RunnerType::Docker {
                max_concurrent_jobs: 8,
                docker_config: crate::domain::entities::DockerConfig {
                    endpoint: "unix:///var/run/docker.sock".to_string(),
                    default_image: "ubuntu:latest".to_string(),
                    network: None,
                    volumes: vec![],
                    environment: HashMap::new(),
                },
            });
            capabilities
                .container_runtimes
                .insert("docker".to_string(), "20.10+".to_string());
        }

        // Check for Kubernetes availability
        if crate::infrastructure::runners::health_checks::is_kubernetes_available().await {
            capabilities.supported_types.push(RunnerType::Kubernetes {
                namespace: "default".to_string(),
                resource_limits: crate::domain::entities::ResourceLimits {
                    cpu_limit: 1000,
                    memory_limit: 1024,
                    storage_limit: Some(10240),
                },
                max_concurrent_jobs: 10,
            });
            capabilities
                .container_runtimes
                .insert("kubernetes".to_string(), "1.20+".to_string());
        }

        Ok(capabilities)
    }

    /// Get total system memory
    async fn get_total_memory(&self) -> Result<u32> {
        // This is a simplified implementation
        // In a real system, you'd use system APIs to get actual memory info
        Ok(8192) // 8GB default
    }

    /// Get available disk space
    async fn get_available_disk_space(&self) -> Result<u32> {
        // This is a simplified implementation
        // In a real system, you'd check actual disk space
        Ok(102400) // 100GB default
    }

    /// Get kernel version
    async fn get_kernel_version(&self) -> Result<String> {
        // This is a simplified implementation
        Ok("5.4.0".to_string())
    }

    /// Get network interfaces
    async fn get_network_interfaces(&self) -> Result<Vec<NetworkInterface>> {
        // This is a simplified implementation
        // In a real system, you'd enumerate actual network interfaces
        Ok(vec![NetworkInterface {
            name: "eth0".to_string(),
            ip_address: "192.168.1.100".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface_type: "ethernet".to_string(),
        }])
    }

    /// Start discovery broadcast task
    async fn start_discovery_broadcast(&self) -> Result<()> {
        let config = self.config.clone();
        let _transport = self.transport.clone();
        let local_node_id = self.local_node_id;
        let local_capabilities = self.local_capabilities.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.discovery_interval));

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Get local capabilities
                let capabilities = {
                    let caps_guard = local_capabilities.read().await;
                    if let Some(caps) = caps_guard.as_ref() {
                        caps.clone()
                    } else {
                        continue;
                    }
                };

                // Create announcement message
                let _message = DiscoveryMessage::Announce {
                    node_id: local_node_id,
                    capabilities,
                    endpoint: SocketAddr::from(([127, 0, 0, 1], config.discovery_port)),
                    auth_token: config.auth_token.clone(),
                    timestamp: Utc::now(),
                };

                // Broadcast announcement (simplified for now)
                debug!(
                    "Would broadcast discovery announcement for node {}",
                    local_node_id
                );
            }
        });

        Ok(())
    }

    /// Start discovery listener task
    async fn start_discovery_listener(&self) -> Result<()> {
        let _transport = self.transport.clone();
        let _discovered_runners = self.discovered_runners.clone();
        let _config = self.config.clone();
        let _local_node_id = self.local_node_id;
        let running = self.running.clone();

        tokio::spawn(async move {
            loop {
                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Listen for discovery messages (simplified for now)
                debug!("Would listen for discovery messages");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }

    /// Cleanup stale discovered runners
    async fn cleanup_stale_runners(&self) -> Result<()> {
        let mut runners = self.discovered_runners.write().await;
        let now = Utc::now();
        let stale_threshold = chrono::Duration::seconds(self.config.discovery_interval as i64 * 3);

        runners.retain(|node_id, runner| {
            let is_stale = now - runner.last_seen > stale_threshold;
            if is_stale {
                info!("Removing stale discovered runner: {}", node_id);
            }
            !is_stale
        });

        Ok(())
    }
}

#[async_trait]
impl RunnerDiscoveryService for DefaultRunnerDiscoveryService {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest(
                "Discovery service is already running".to_string(),
            ));
        }

        *running = true;

        // Detect local capabilities
        let capabilities = self.detect_local_capabilities().await?;
        {
            let mut caps_guard = self.local_capabilities.write().await;
            *caps_guard = Some(capabilities);
        }

        // Start discovery tasks
        self.start_discovery_broadcast().await?;
        self.start_discovery_listener().await?;

        // Start cleanup task
        let discovered_runners = self.discovered_runners.clone();
        let config = self.config.clone();
        let running_clone = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Cleanup every minute

            loop {
                interval.tick().await;

                {
                    let running_guard = running_clone.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                let now = Utc::now();
                let stale_threshold =
                    chrono::Duration::seconds(config.discovery_interval as i64 * 3);

                let mut runners = discovered_runners.write().await;
                runners.retain(|node_id, runner| {
                    let is_stale = now - runner.last_seen > stale_threshold;
                    if is_stale {
                        info!("Removing stale discovered runner: {}", node_id);
                    }
                    !is_stale
                });
            }
        });

        info!("Runner discovery service started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }

        *running = false;

        info!("Runner discovery service stopped");
        Ok(())
    }

    async fn announce(&self, capabilities: RunnerCapabilities) -> Result<()> {
        let _message = DiscoveryMessage::Announce {
            node_id: self.local_node_id,
            capabilities,
            endpoint: SocketAddr::from(([127, 0, 0, 1], self.config.discovery_port)),
            auth_token: self.config.auth_token.clone(),
            timestamp: Utc::now(),
        };

        // Simplified broadcast for now
        debug!(
            "Would broadcast announcement for node {}",
            self.local_node_id
        );
        Ok(())
    }

    async fn discover(&self) -> Result<Vec<DiscoveredRunner>> {
        let _message = DiscoveryMessage::Discover {
            requester_id: self.local_node_id,
            auth_token: self.config.auth_token.clone(),
            timestamp: Utc::now(),
        };

        // Send discovery request (simplified for now)
        debug!(
            "Would send discovery request from node {}",
            self.local_node_id
        );

        // Wait for responses
        tokio::time::sleep(Duration::from_secs(self.config.discovery_timeout)).await;

        // Return discovered runners
        let runners = self.discovered_runners.read().await;
        Ok(runners.values().cloned().collect())
    }

    async fn register_runner(&self, runner: &DiscoveredRunner) -> Result<ClusterNode> {
        // Create cluster node from discovered runner
        let mut cluster_node = ClusterNode::new(
            format!("runner-{}", runner.node_id),
            runner.endpoint,
            NodeRole::Worker,
        );

        cluster_node.id = runner.node_id;
        cluster_node.status = NodeStatus::Active;

        // Register with node registry
        let registered_node = self.node_registry.register_node(cluster_node).await?;

        // Mark as registered in discovered runners
        {
            let mut runners = self.discovered_runners.write().await;
            if let Some(discovered) = runners.get_mut(&runner.node_id) {
                discovered.registered = true;
            }
        }

        info!(
            "Successfully registered discovered runner: {}",
            runner.node_id
        );
        Ok(registered_node)
    }

    async fn get_discovered_runners(&self) -> Result<Vec<DiscoveredRunner>> {
        let runners = self.discovered_runners.read().await;
        Ok(runners.values().cloned().collect())
    }

    async fn authenticate_runner(&self, node_id: NodeId, auth_token: &str) -> Result<bool> {
        let is_authenticated = auth_token == self.config.auth_token;

        if is_authenticated {
            // Update authentication status
            let mut runners = self.discovered_runners.write().await;
            if let Some(runner) = runners.get_mut(&node_id) {
                runner.authenticated = true;
            }
        }

        Ok(is_authenticated)
    }
}

/// Runner authentication service
pub struct RunnerAuthenticationService {
    /// Valid authentication tokens
    valid_tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    /// Token expiration time
    token_expiration: Duration,
}

/// Authentication token information
#[derive(Debug, Clone)]
pub struct AuthToken {
    /// Token value
    pub token: String,
    /// Node ID associated with token
    pub node_id: NodeId,
    /// Token creation time
    pub created_at: DateTime<Utc>,
    /// Token expiration time
    pub expires_at: DateTime<Utc>,
    /// Token permissions
    pub permissions: Vec<String>,
}

impl RunnerAuthenticationService {
    /// Create a new authentication service
    pub fn new(token_expiration: Duration) -> Self {
        Self {
            valid_tokens: Arc::new(RwLock::new(HashMap::new())),
            token_expiration,
        }
    }

    /// Generate a new authentication token
    pub async fn generate_token(
        &self,
        node_id: NodeId,
        permissions: Vec<String>,
    ) -> Result<String> {
        let token = Uuid::new_v4().to_string();
        let now = Utc::now();

        let auth_token = AuthToken {
            token: token.clone(),
            node_id,
            created_at: now,
            expires_at: now
                + chrono::Duration::from_std(self.token_expiration).map_err(|e| {
                    AppError::BadRequest(format!("Invalid token expiration: {}", e))
                })?,
            permissions,
        };

        let mut tokens = self.valid_tokens.write().await;
        tokens.insert(token.clone(), auth_token);

        Ok(token)
    }

    /// Validate an authentication token
    pub async fn validate_token(&self, token: &str) -> Result<Option<AuthToken>> {
        let tokens = self.valid_tokens.read().await;

        if let Some(auth_token) = tokens.get(token) {
            if Utc::now() < auth_token.expires_at {
                Ok(Some(auth_token.clone()))
            } else {
                Ok(None) // Token expired
            }
        } else {
            Ok(None) // Token not found
        }
    }

    /// Revoke an authentication token
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        let mut tokens = self.valid_tokens.write().await;
        tokens.remove(token);
        Ok(())
    }

    /// Cleanup expired tokens
    pub async fn cleanup_expired_tokens(&self) -> Result<u32> {
        let mut tokens = self.valid_tokens.write().await;
        let now = Utc::now();
        let initial_count = tokens.len();

        tokens.retain(|_, auth_token| now < auth_token.expires_at);

        let removed_count = initial_count - tokens.len();
        Ok(removed_count as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cluster::node_registry::tests::create_test_registry;
    use crate::core::networking::transport::{
        Connection, Transport, TransportConfig, TransportEndpoint, TransportMetrics,
    };

    // Mock transport for testing
    struct MockTransport;

    #[async_trait]
    impl Transport for MockTransport {
        async fn listen(
            &self,
            _config: &TransportConfig,
        ) -> Result<Box<dyn crate::core::networking::transport::Listener>> {
            Err(AppError::NotImplemented(
                "Mock transport listen not implemented".to_string(),
            ))
        }

        async fn connect(&self, _endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
            Err(AppError::NotImplemented(
                "Mock transport connect not implemented".to_string(),
            ))
        }

        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }

        async fn get_metrics(&self) -> TransportMetrics {
            Default::default()
        }

        fn transport_type(&self) -> crate::core::networking::transport::TransportType {
            crate::core::networking::transport::TransportType::Tcp
        }

        fn capabilities(&self) -> crate::core::networking::valkyrie::types::TransportCapabilities {
            Default::default()
        }

        async fn configure(&mut self, _config: TransportConfig) -> Result<()> {
            Ok(())
        }

        fn supports_endpoint(&self, _endpoint: &TransportEndpoint) -> bool {
            false
        }

        async fn optimize_for_conditions(
            &self,
            _conditions: &NetworkConditions,
        ) -> TransportConfig {
            Default::default()
        }
    }

    impl MockTransport {
        fn new() -> Self {
            Self
        }
    }

    fn create_test_discovery_service() -> DefaultRunnerDiscoveryService {
        let config = RunnerDiscoveryConfig::default();
        let node_registry = Arc::new(create_test_registry());
        let transport = Arc::new(MockTransport::new());

        DefaultRunnerDiscoveryService::new(config, node_registry, transport)
    }

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let service = create_test_discovery_service();
        assert!(!*service.running.lock().await);
    }

    #[tokio::test]
    async fn test_capability_detection() {
        let service = create_test_discovery_service();
        let capabilities = service.detect_local_capabilities().await.unwrap();

        assert!(!capabilities.supported_types.is_empty());
        assert!(capabilities.cpu_cores > 0);
        assert!(capabilities.memory_mb > 0);
        assert_eq!(capabilities.os, std::env::consts::OS);
    }

    #[tokio::test]
    async fn test_authentication_service() {
        let auth_service = RunnerAuthenticationService::new(Duration::from_secs(3600));
        let node_id = Uuid::new_v4();
        let permissions = vec!["read".to_string(), "write".to_string()];

        let token = auth_service
            .generate_token(node_id, permissions.clone())
            .await
            .unwrap();
        assert!(!token.is_empty());

        let validated = auth_service.validate_token(&token).await.unwrap();
        assert!(validated.is_some());

        let auth_token = validated.unwrap();
        assert_eq!(auth_token.node_id, node_id);
        assert_eq!(auth_token.permissions, permissions);

        auth_service.revoke_token(&token).await.unwrap();
        let revoked = auth_service.validate_token(&token).await.unwrap();
        assert!(revoked.is_none());
    }

    #[tokio::test]
    async fn test_discovered_runner_creation() {
        let node_id = Uuid::new_v4();
        let endpoint = SocketAddr::from(([127, 0, 0, 1], 8080));
        let capabilities = RunnerCapabilities {
            supported_types: vec![RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            }],
            cpu_cores: 4,
            memory_mb: 8192,
            disk_mb: 102400,
            architectures: vec!["x86_64".to_string()],
            os: "linux".to_string(),
            kernel_version: "5.4.0".to_string(),
            container_runtimes: HashMap::new(),
            network_interfaces: vec![],
            custom_capabilities: HashMap::new(),
        };

        let discovered_runner = DiscoveredRunner {
            node_id,
            endpoint,
            capabilities: capabilities.clone(),
            discovered_at: Utc::now(),
            last_seen: Utc::now(),
            authenticated: true,
            registered: false,
        };

        assert_eq!(discovered_runner.node_id, node_id);
        assert_eq!(discovered_runner.endpoint, endpoint);
        assert!(discovered_runner.authenticated);
        assert!(!discovered_runner.registered);
    }
}
