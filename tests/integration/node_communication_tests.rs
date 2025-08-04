use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use RustAutoDevOps::core::node_communication::{
    NodeId, ProtocolMessage, MessagePayload, NodeMessage, ControlPlaneMessage,
    NodeInfo, NodeCapabilities, NodeStatus, NodeMetrics, NodeResources,
    NodeType, RunnerType, JobDefinition, ExecutionContext, JobPriority,
    ResourceRequirements, ArtifactSpec, ArtifactType
};
use RustAutoDevOps::core::transport::{
    Transport, TransportConfig, TransportType, TransportEndpoint,
    AuthenticationConfig, AuthMethod, TimeoutConfig, BufferConfig
};
use RustAutoDevOps::core::secure_transport::{
    SecureTransport, AuthenticationManager, EncryptionManager
};
use RustAutoDevOps::core::node_communication_manager::{
    NodeCommunicationManager, CommunicationConfig, RegistrationHandler,
    HeartbeatHandler, JobResultHandler, MessageHandler
};

#[tokio::test]
async fn test_node_registration_flow() {
    let auth_manager = Arc::new(AuthenticationManager::new(
        "test-secret",
        Duration::from_secs(3600),
    ));
    
    let node_id = Uuid::new_v4();
    let token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string(), "docker".to_string()],
    ).await.unwrap();
    
    // Verify token can be validated
    let claims = auth_manager.verify_token(&token).await.unwrap();
    assert_eq!(claims.node_id, node_id);
    assert_eq!(claims.node_type, "worker");
    assert!(claims.capabilities.contains(&"native".to_string()));
}

#[tokio::test]
async fn test_protocol_message_serialization() {
    let node_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    
    // Test NodeMessage serialization
    let node_message = NodeMessage::RegisterNode {
        node_info: NodeInfo {
            hostname: "test-node".to_string(),
            ip_address: "192.168.1.100".to_string(),
            port: 8080,
            node_type: NodeType::Worker,
            version: "1.0.0".to_string(),
            platform: "linux".to_string(),
            architecture: "x86_64".to_string(),
            tags: HashMap::new(),
        },
        capabilities: NodeCapabilities {
            runner_types: vec![RunnerType::Native, RunnerType::Docker],
            max_resources: NodeResources {
                cpu_cores: 8,
                memory_mb: 16384,
                disk_gb: 500,
                network_mbps: 1000,
                available_cpu: 8.0,
                available_memory_mb: 16384,
                available_disk_gb: 500,
            },
            supported_job_types: vec!["build".to_string(), "test".to_string()],
            features: vec!["isolation".to_string()],
            protocols: vec!["tcp".to_string(), "websocket".to_string()],
        },
        auth_token: "test-token".to_string(),
    };
    
    let protocol_message = ProtocolMessage::new(
        node_id,
        MessagePayload::NodeMessage(node_message),
    );
    
    // Serialize and deserialize
    let serialized = serde_json::to_string(&protocol_message).unwrap();
    let deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(protocol_message.source, deserialized.source);
    assert_eq!(protocol_message.id, deserialized.id);
    
    // Test ControlPlaneMessage serialization
    let cp_message = ControlPlaneMessage::JobAssignment {
        job_id,
        job: Box::new(JobDefinition {
            name: "test-job".to_string(),
            image: Some("ubuntu:20.04".to_string()),
            command: vec!["echo".to_string(), "hello".to_string()],
            environment: HashMap::new(),
            working_directory: Some("/tmp".to_string()),
            timeout: Some(Duration::from_secs(300)),
            resource_requirements: ResourceRequirements {
                cpu_cores: Some(2.0),
                memory_mb: Some(1024),
                disk_gb: Some(10),
                gpu_count: None,
            },
            artifacts: vec![
                ArtifactSpec {
                    name: "logs".to_string(),
                    path: "/tmp/logs".to_string(),
                    artifact_type: ArtifactType::Directory,
                    compress: true,
                }
            ],
        }),
        execution_context: ExecutionContext {
            workspace_id: "test-workspace".to_string(),
            pipeline_id: "test-pipeline".to_string(),
            build_number: 1,
            git_ref: Some("main".to_string()),
            git_commit: Some("abc123".to_string()),
            variables: HashMap::new(),
            secrets: HashMap::new(),
        },
        deadline: None,
        priority: JobPriority::Normal,
    };
    
    let cp_protocol_message = ProtocolMessage::new(
        Uuid::new_v4(),
        MessagePayload::ControlPlaneMessage(cp_message),
    );
    
    let cp_serialized = serde_json::to_string(&cp_protocol_message).unwrap();
    let cp_deserialized: ProtocolMessage = serde_json::from_str(&cp_serialized).unwrap();
    
    assert_eq!(cp_protocol_message.source, cp_deserialized.source);
}

#[tokio::test]
async fn test_heartbeat_message() {
    let node_id = Uuid::new_v4();
    
    let heartbeat = NodeMessage::Heartbeat {
        node_id,
        status: NodeStatus::Ready,
        resources: NodeResources {
            cpu_cores: 8,
            memory_mb: 16384,
            disk_gb: 500,
            network_mbps: 1000,
            available_cpu: 6.5,
            available_memory_mb: 12000,
            available_disk_gb: 450,
        },
        metrics: NodeMetrics {
            cpu_usage_percent: 18.75,
            memory_usage_percent: 26.56,
            disk_usage_percent: 10.0,
            network_rx_mbps: 50.0,
            network_tx_mbps: 25.0,
            load_average: 1.2,
            active_jobs: 2,
            completed_jobs: 150,
            failed_jobs: 5,
            uptime_seconds: 86400,
        },
        timestamp: chrono::Utc::now(),
    };
    
    let protocol_message = ProtocolMessage::new(
        node_id,
        MessagePayload::NodeMessage(heartbeat),
    );
    
    // Verify serialization works
    let serialized = serde_json::to_string(&protocol_message).unwrap();
    let deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
    
    if let MessagePayload::NodeMessage(NodeMessage::Heartbeat { status, .. }) = deserialized.message {
        assert!(matches!(status, NodeStatus::Ready));
    } else {
        panic!("Expected heartbeat message");
    }
}

#[tokio::test]
async fn test_authentication_manager() {
    let auth_manager = AuthenticationManager::new(
        "test-secret-key",
        Duration::from_secs(3600),
    );
    
    let node_id = Uuid::new_v4();
    
    // Generate token
    let token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    // Verify token
    let claims = auth_manager.verify_token(&token).await.unwrap();
    assert_eq!(claims.node_id, node_id);
    assert_eq!(claims.node_type, "worker");
    
    // Revoke token
    auth_manager.revoke_token(&token).await.unwrap();
    
    // Verify revoked token fails
    let result = auth_manager.verify_token(&token).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_message_handlers() {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let auth_manager = Arc::new(AuthenticationManager::new(
        "test-secret",
        Duration::from_secs(3600),
    ));
    
    let node_id = Uuid::new_v4();
    let token = auth_manager.generate_token(
        node_id,
        "worker".to_string(),
        vec!["native".to_string()],
    ).await.unwrap();
    
    // Test registration handler
    let registration_handler = RegistrationHandler::new(
        auth_manager.clone(),
        event_tx.clone(),
    );
    
    let register_message = NodeMessage::RegisterNode {
        node_info: NodeInfo {
            hostname: "test-node".to_string(),
            ip_address: "192.168.1.100".to_string(),
            port: 8080,
            node_type: NodeType::Worker,
            version: "1.0.0".to_string(),
            platform: "linux".to_string(),
            architecture: "x86_64".to_string(),
            tags: HashMap::new(),
        },
        capabilities: NodeCapabilities {
            runner_types: vec![RunnerType::Native],
            max_resources: NodeResources {
                cpu_cores: 4,
                memory_mb: 8192,
                disk_gb: 100,
                network_mbps: 1000,
                available_cpu: 4.0,
                available_memory_mb: 8192,
                available_disk_gb: 100,
            },
            supported_job_types: vec!["build".to_string()],
            features: vec![],
            protocols: vec!["tcp".to_string()],
        },
        auth_token: token,
    };
    
    let result = registration_handler.handle_message(node_id, register_message).await;
    assert!(result.is_ok());
    
    // Verify event was sent
    let event = timeout(Duration::from_millis(100), event_rx.recv()).await;
    assert!(event.is_ok());
    
    // Test heartbeat handler
    let heartbeat_handler = HeartbeatHandler::new(event_tx.clone());
    
    let heartbeat_message = NodeMessage::Heartbeat {
        node_id,
        status: NodeStatus::Ready,
        resources: NodeResources {
            cpu_cores: 4,
            memory_mb: 8192,
            disk_gb: 100,
            network_mbps: 1000,
            available_cpu: 3.5,
            available_memory_mb: 6000,
            available_disk_gb: 90,
        },
        metrics: NodeMetrics {
            cpu_usage_percent: 12.5,
            memory_usage_percent: 26.8,
            disk_usage_percent: 10.0,
            network_rx_mbps: 10.0,
            network_tx_mbps: 5.0,
            load_average: 0.8,
            active_jobs: 1,
            completed_jobs: 50,
            failed_jobs: 2,
            uptime_seconds: 3600,
        },
        timestamp: chrono::Utc::now(),
    };
    
    let result = heartbeat_handler.handle_message(node_id, heartbeat_message).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transport_config() {
    let config = TransportConfig {
        transport_type: TransportType::Tcp,
        bind_address: "127.0.0.1".to_string(),
        port: Some(8080),
        tls_config: None,
        authentication: AuthenticationConfig {
            method: AuthMethod::JwtToken,
            jwt_secret: Some("test-secret".to_string()),
            token_expiry: Duration::from_secs(3600),
            require_mutual_auth: false,
        },
        timeouts: TimeoutConfig::default(),
        buffer_sizes: BufferConfig::default(),
    };
    
    // Verify config serialization
    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: TransportConfig = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(config.bind_address, deserialized.bind_address);
    assert_eq!(config.port, deserialized.port);
    assert!(matches!(deserialized.transport_type, TransportType::Tcp));
}

#[tokio::test]
async fn test_communication_config() {
    let config = CommunicationConfig::default();
    
    assert_eq!(config.heartbeat_timeout, Duration::from_secs(60));
    assert_eq!(config.message_timeout, Duration::from_secs(30));
    assert_eq!(config.max_connections, 1000);
    assert!(config.enable_compression);
    assert!(config.enable_encryption);
}

#[tokio::test]
async fn test_job_assignment_message() {
    let job_id = Uuid::new_v4();
    
    let job_assignment = ControlPlaneMessage::JobAssignment {
        job_id,
        job: Box::new(JobDefinition {
            name: "integration-test".to_string(),
            image: Some("rust:1.70".to_string()),
            command: vec!["cargo".to_string(), "test".to_string()],
            environment: [
                ("RUST_LOG".to_string(), "debug".to_string()),
                ("CI".to_string(), "true".to_string()),
            ].into(),
            working_directory: Some("/workspace".to_string()),
            timeout: Some(Duration::from_secs(1800)),
            resource_requirements: ResourceRequirements {
                cpu_cores: Some(4.0),
                memory_mb: Some(4096),
                disk_gb: Some(20),
                gpu_count: None,
            },
            artifacts: vec![
                ArtifactSpec {
                    name: "test-results".to_string(),
                    path: "/workspace/target/test-results".to_string(),
                    artifact_type: ArtifactType::Directory,
                    compress: true,
                },
                ArtifactSpec {
                    name: "coverage-report".to_string(),
                    path: "/workspace/target/coverage".to_string(),
                    artifact_type: ArtifactType::Archive,
                    compress: false,
                },
            ],
        }),
        execution_context: ExecutionContext {
            workspace_id: "rust-project".to_string(),
            pipeline_id: "ci-pipeline".to_string(),
            build_number: 42,
            git_ref: Some("refs/heads/feature/node-communication".to_string()),
            git_commit: Some("a1b2c3d4e5f6".to_string()),
            variables: [
                ("BRANCH_NAME".to_string(), "feature/node-communication".to_string()),
                ("BUILD_ID".to_string(), "42".to_string()),
            ].into(),
            secrets: [
                ("GITHUB_TOKEN".to_string(), "ghp_xxxxxxxxxxxx".to_string()),
            ].into(),
        },
        deadline: Some(chrono::Utc::now() + chrono::Duration::minutes(30)),
        priority: JobPriority::High,
    };
    
    let protocol_message = ProtocolMessage::new(
        Uuid::new_v4(),
        MessagePayload::ControlPlaneMessage(job_assignment),
    );
    
    // Test serialization
    let serialized = serde_json::to_string(&protocol_message).unwrap();
    let deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
    
    if let MessagePayload::ControlPlaneMessage(ControlPlaneMessage::JobAssignment { 
        job_id: deserialized_job_id, 
        priority, 
        .. 
    }) = deserialized.message {
        assert_eq!(job_id, deserialized_job_id);
        assert!(matches!(priority, JobPriority::High));
    } else {
        panic!("Expected job assignment message");
    }
}

#[tokio::test]
async fn test_encryption_manager() {
    let encryption_manager = EncryptionManager::new(true, None);
    
    let original_message = ProtocolMessage::new(
        Uuid::new_v4(),
        MessagePayload::NodeMessage(NodeMessage::Heartbeat {
            node_id: Uuid::new_v4(),
            status: NodeStatus::Ready,
            resources: NodeResources {
                cpu_cores: 4,
                memory_mb: 8192,
                disk_gb: 100,
                network_mbps: 1000,
                available_cpu: 3.0,
                available_memory_mb: 6000,
                available_disk_gb: 90,
            },
            metrics: NodeMetrics {
                cpu_usage_percent: 25.0,
                memory_usage_percent: 26.6,
                disk_usage_percent: 10.0,
                network_rx_mbps: 15.0,
                network_tx_mbps: 8.0,
                load_average: 1.0,
                active_jobs: 2,
                completed_jobs: 100,
                failed_jobs: 3,
                uptime_seconds: 7200,
            },
            timestamp: chrono::Utc::now(),
        }),
    );
    
    // Test encryption/decryption cycle
    let encrypted = encryption_manager.encrypt_message(&original_message).await.unwrap();
    let decrypted = encryption_manager.decrypt_message(&encrypted).await.unwrap();
    
    assert_eq!(original_message.id, decrypted.id);
    assert_eq!(original_message.source, decrypted.source);
}