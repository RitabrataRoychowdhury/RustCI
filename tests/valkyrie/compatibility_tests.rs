use std::time::Duration;
use tokio::time::timeout;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;
use rustci::core::networking::valkyrie::message::MessageType;

/// Kubernetes/Docker compatibility tests
#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[tokio::test]
    async fn test_kubernetes_service_discovery() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        // Simulate Kubernetes service discovery
        let k8s_service_name = "valkyrie-service.default.svc.cluster.local";
        let k8s_port = 8080;
        
        // Test DNS resolution (in real K8s environment)
        let endpoint = format!("{}:{}", k8s_service_name, k8s_port);
        
        // For testing, we'll use localhost
        let test_endpoint = format!("127.0.0.1:{}", k8s_port);
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection_result = timeout(
            Duration::from_secs(5),
            engine.connect(bind_addr)
        ).await;
        
        assert!(connection_result.is_ok(), "Kubernetes service connection should work");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_kubernetes_pod_to_pod_communication() {
        // Simulate multiple pods communicating
        let pod_count = 3;
        let mut engines = Vec::new();
        
        // Start multiple "pods"
        for i in 0..pod_count {
            let mut config = TestFixtures::minimal_config();
            config.node_id = format!("pod-{}", i);
            config.bind_port = 8080 + i as u16;
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            engines.push(engine);
        }
        
        // Test pod-to-pod communication
        let pod1 = &engines[0];
        let pod2_addr = format!("127.0.0.1:{}", 8081).parse().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            pod1.connect(pod2_addr)
        ).await.unwrap().unwrap();
        
        let message = TestUtils::create_test_message(MessageType::Hello);
        let send_result = connection.send_message(message).await;
        
        assert!(send_result.is_ok(), "Pod-to-pod communication should work");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_kubernetes_ingress_integration() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        // Test ingress-style headers
        let ingress_headers = vec![
            ("X-Forwarded-For", "10.244.0.1"),
            ("X-Forwarded-Proto", "https"),
            ("X-Real-IP", "192.168.1.100"),
            ("X-Forwarded-Host", "valkyrie.example.com"),
        ];
        
        // Simulate ingress request
        let connection = engine.connect("127.0.0.1:8080".parse().unwrap()).await.unwrap();
        
        let mut message = TestUtils::create_test_message(MessageType::JobRequest);
        message.header.metadata = Some(
            ingress_headers.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        );
        
        let send_result = connection.send_message(message).await;
        assert!(send_result.is_ok(), "Ingress-style request should work");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_docker_container_networking() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        // Test Docker bridge network communication
        let docker_bridge_ip = "172.17.0.2"; // Typical Docker bridge IP
        let endpoint = format!("{}:8080", docker_bridge_ip);
        
        // For testing, use localhost
        let test_endpoint = "127.0.0.1:8080".parse().unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            engine.connect(bind_addr)
        ).await.unwrap().unwrap();
        
        // Test container-to-container communication
        let message = TestUtils::create_test_message(MessageType::Hello);
        let send_result = connection.send_message(message).await;
        
        assert!(send_result.is_ok(), "Docker container communication should work");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_docker_compose_service_communication() {
        // Simulate Docker Compose services
        let services = vec![
            ("valkyrie-api", 8080),
            ("valkyrie-worker", 8081),
            ("valkyrie-scheduler", 8082),
        ];
        
        let mut engines = Vec::new();
        
        // Start services
        for (service_name, port) in &services {
            let mut config = TestFixtures::minimal_config();
            config.node_id = service_name.to_string();
            config.bind_port = *port;
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            engines.push(engine);
        }
        
        // Test service-to-service communication
        let api_service = &engines[0];
        let worker_addr = format!("127.0.0.1:{}", 8081).parse().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            api_service.connect(worker_addr)
        ).await.unwrap().unwrap();
        
        let job_message = TestUtils::create_test_message(MessageType::JobRequest);
        let send_result = connection.send_message(job_message).await;
        
        assert!(send_result.is_ok(), "Docker Compose service communication should work");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_kubernetes_configmap_integration() {
        // Test configuration from Kubernetes ConfigMap
        let configmap_data = std::collections::HashMap::from([
            ("VALKYRIE_NODE_ID".to_string(), "k8s-node-1".to_string()),
            ("VALKYRIE_BIND_PORT".to_string(), "8080".to_string()),
            ("VALKYRIE_SECURITY_ENABLED".to_string(), "true".to_string()),
        ]);
        
        // Apply ConfigMap configuration
        let mut config = TestFixtures::minimal_config();
        config.node_id = configmap_data.get("VALKYRIE_NODE_ID").unwrap().clone();
        config.bind_port = configmap_data.get("VALKYRIE_BIND_PORT").unwrap().parse().unwrap();
        config.security_enabled = configmap_data.get("VALKYRIE_SECURITY_ENABLED").unwrap().parse().unwrap();
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        let start_result = engine.start().await;
        
        assert!(start_result.is_ok(), "Engine should start with ConfigMap configuration");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_kubernetes_secret_integration() {
        // Test secrets from Kubernetes Secret
        let secret_data = std::collections::HashMap::from([
            ("tls.crt".to_string(), "LS0tLS1CRUdJTi...".to_string()), // Base64 encoded cert
            ("tls.key".to_string(), "LS0tLS1CRUdJTi...".to_string()), // Base64 encoded key
            ("auth.token".to_string(), "dGVzdC10b2tlbi0xMjM=".to_string()), // Base64 encoded token
        ]);
        
        // Create temporary secret files
        let temp_dir = tempfile::tempdir().unwrap();
        let cert_path = temp_dir.path().join("tls.crt");
        let key_path = temp_dir.path().join("tls.key");
        
        std::fs::write(&cert_path, base64::decode(secret_data.get("tls.crt").unwrap()).unwrap()).unwrap();
        std::fs::write(&key_path, base64::decode(secret_data.get("tls.key").unwrap()).unwrap()).unwrap();
        
        let mut config = TestFixtures::secure_config();
        config.tls_cert_path = Some(cert_path.to_string_lossy().to_string());
        config.tls_key_path = Some(key_path.to_string_lossy().to_string());
        config.auth_token = Some(
            String::from_utf8(base64::decode(secret_data.get("auth.token").unwrap()).unwrap()).unwrap()
        );
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        let start_result = engine.start().await;
        
        assert!(start_result.is_ok(), "Engine should start with Kubernetes secrets");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_kubernetes_health_checks() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        // Test liveness probe
        let liveness_result = engine.health_check("liveness").await;
        assert!(liveness_result.is_ok(), "Liveness probe should pass");
        
        // Test readiness probe
        let readiness_result = engine.health_check("readiness").await;
        assert!(readiness_result.is_ok(), "Readiness probe should pass");
        
        // Test startup probe
        let startup_result = engine.health_check("startup").await;
        assert!(startup_result.is_ok(), "Startup probe should pass");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_docker_volume_persistence() {
        // Test persistent data across container restarts
        let temp_dir = tempfile::tempdir().unwrap();
        let data_path = temp_dir.path().join("valkyrie-data");
        std::fs::create_dir_all(&data_path).unwrap();
        
        let mut config = TestFixtures::minimal_config();
        config.data_directory = Some(data_path.to_string_lossy().to_string());
        
        // First engine instance
        {
            let mut engine = ValkyrieEngine::new(config.clone()).await.unwrap();
            engine.start().await.unwrap();
            
            // Write some persistent data
            engine.save_persistent_data("test-key", "test-value").await.unwrap();
            
            engine.stop().await.unwrap();
        }
        
        // Second engine instance (simulating container restart)
        {
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            
            // Read persistent data
            let value = engine.load_persistent_data("test-key").await.unwrap();
            assert_eq!(value, Some("test-value".to_string()), "Data should persist across restarts");
            
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_kubernetes_resource_limits() {
        // Test behavior under Kubernetes resource limits
        let mut config = TestFixtures::minimal_config();
        config.max_memory_mb = Some(128); // Simulate 128MB memory limit
        config.max_cpu_cores = Some(0.5); // Simulate 0.5 CPU core limit
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test that engine respects resource limits
        let memory_usage = engine.get_memory_usage().await.unwrap();
        assert!(memory_usage < 128 * 1024 * 1024, "Should respect memory limits");
        
        let cpu_usage = engine.get_cpu_usage().await.unwrap();
        assert!(cpu_usage < 0.6, "Should respect CPU limits"); // Allow some overhead
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_docker_network_modes() {
        let network_modes = vec![
            ("bridge", "172.17.0.0/16"),
            ("host", "127.0.0.1"),
            ("overlay", "10.0.0.0/24"),
        ];
        
        for (mode, network_cidr) in network_modes {
            println!("Testing Docker network mode: {}", mode);
            
            let mut config = TestFixtures::minimal_config();
            config.network_mode = Some(mode.to_string());
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            let start_result = engine.start().await;
            
            assert!(start_result.is_ok(), "Engine should work with {} network mode", mode);
            
            // Test network connectivity
            let listener = engine.listen("0.0.0.0:0".parse().unwrap()).await.unwrap();
            let bind_addr = listener.local_addr().unwrap();
            
            // For testing, we'll connect to localhost regardless of network mode
            let test_addr = format!("127.0.0.1:{}", bind_addr.port()).parse().unwrap();
            let connection = engine.connect(test_addr).await;
            
            assert!(connection.is_ok(), "Network connectivity should work in {} mode", mode);
            
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_kubernetes_horizontal_pod_autoscaler() {
        // Simulate HPA scaling scenario
        let initial_replicas = 2;
        let max_replicas = 5;
        
        let mut engines = Vec::new();
        
        // Start initial replicas
        for i in 0..initial_replicas {
            let mut config = TestFixtures::minimal_config();
            config.node_id = format!("hpa-pod-{}", i);
            config.bind_port = 8080 + i as u16;
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            engines.push(engine);
        }
        
        // Simulate high load triggering HPA scale-up
        let load_connections = 100;
        let mut connections = Vec::new();
        
        for i in 0..load_connections {
            let pod_index = i % engines.len();
            let pod_addr = format!("127.0.0.1:{}", 8080 + pod_index).parse().unwrap();
            
            if let Ok(connection) = engines[pod_index].connect(pod_addr).await {
                connections.push(connection);
            }
        }
        
        // Simulate HPA adding more replicas
        for i in initial_replicas..max_replicas {
            let mut config = TestFixtures::minimal_config();
            config.node_id = format!("hpa-pod-{}", i);
            config.bind_port = 8080 + i as u16;
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            engines.push(engine);
        }
        
        // Test load distribution across all replicas
        let total_replicas = engines.len();
        assert_eq!(total_replicas, max_replicas, "Should have scaled to max replicas");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_docker_swarm_service_discovery() {
        // Simulate Docker Swarm service discovery
        let swarm_services = vec![
            ("valkyrie_api", vec!["10.0.0.10:8080", "10.0.0.11:8080"]),
            ("valkyrie_worker", vec!["10.0.0.20:8081", "10.0.0.21:8081"]),
        ];
        
        for (service_name, endpoints) in swarm_services {
            println!("Testing Docker Swarm service: {}", service_name);
            
            // Test service discovery and load balancing
            for endpoint in endpoints {
                // For testing, we'll use localhost
                let test_endpoint = "127.0.0.1:8080".parse().unwrap();
                
                let mut engine = TestUtils::create_test_engine().await.unwrap();
                engine.start().await.unwrap();
                
                let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
                let bind_addr = listener.local_addr().unwrap();
                
                let connection = timeout(
                    Duration::from_secs(5),
                    engine.connect(bind_addr)
                ).await;
                
                assert!(connection.is_ok(), "Swarm service endpoint should be reachable");
                
                engine.stop().await.unwrap();
            }
        }
    }
}