use std::time::Duration;
use tokio::time::timeout;
use serde_json::json;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions};
use rustci::core::networking::valkyrie::bridge::http_gateway::HttpGateway;
use rustci::core::networking::valkyrie::bridge::protocol_negotiation::ProtocolNegotiator;
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;

/// HTTP/HTTPS bridge integration tests
#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[tokio::test]
    async fn test_http_to_valkyrie_conversion() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        // Test HTTP POST request conversion to Valkyrie message
        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8080/api/v1/jobs")
            .json(&json!({
                "pipeline": "test-pipeline",
                "steps": ["build", "test", "deploy"]
            }))
            .send()
            .await
            .unwrap();
        
        assert_eq!(response.status(), 200, "HTTP request should be successful");
        
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body.get("job_id").is_some(), "Response should contain job_id");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_protocol_negotiation() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let negotiator = ProtocolNegotiator::new();
        
        // Test Valkyrie protocol detection
        let valkyrie_headers = vec![
            ("X-Valkyrie-Version", "1.0"),
            ("X-Valkyrie-Node-Id", "test-client"),
        ];
        
        let protocol = negotiator.negotiate_protocol(&valkyrie_headers);
        assert_eq!(protocol, "valkyrie", "Should detect Valkyrie protocol");
        
        // Test HTTP fallback
        let http_headers = vec![
            ("User-Agent", "Mozilla/5.0"),
            ("Accept", "application/json"),
        ];
        
        let protocol = negotiator.negotiate_protocol(&http_headers);
        assert_eq!(protocol, "http", "Should fallback to HTTP");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_websocket_upgrade() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        // Test WebSocket upgrade to Valkyrie protocol
        let ws_url = "ws://127.0.0.1:8080/valkyrie";
        
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();
        
        // Send a test message over WebSocket
        let test_message = json!({
            "type": "job_request",
            "payload": {
                "pipeline": "websocket-test"
            }
        });
        
        // This would typically involve sending the message and receiving a response
        // For now, we just verify the connection was established
        assert!(ws_stream.get_ref().peer_addr().is_ok(), "WebSocket connection should be established");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_rest_api_translation() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        let client = reqwest::Client::new();
        
        // Test GET request translation
        let response = client
            .get("http://127.0.0.1:8080/api/v1/jobs/test-job-123")
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "GET request should succeed");
        
        // Test PUT request translation
        let response = client
            .put("http://127.0.0.1:8080/api/v1/jobs/test-job-123")
            .json(&json!({
                "status": "running"
            }))
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "PUT request should succeed");
        
        // Test DELETE request translation
        let response = client
            .delete("http://127.0.0.1:8080/api/v1/jobs/test-job-123")
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "DELETE request should succeed");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_https_bridge() {
        let mut config = TestFixtures::secure_config();
        config.enable_https_bridge = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new_secure(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        // Test HTTPS request
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // For testing only
            .build()
            .unwrap();
        
        let response = client
            .post("https://127.0.0.1:8443/api/v1/jobs")
            .json(&json!({
                "pipeline": "secure-pipeline"
            }))
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "HTTPS request should succeed");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_bridge_performance() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        let client = reqwest::Client::new();
        
        // Warm up the HTTP bridge
        for _ in 0..50 {
            let _ = client
                .post("http://127.0.0.1:8080/api/v1/jobs")
                .json(&json!({"pipeline": "warmup"}))
                .send()
                .await;
        }
        
        // Allow bridge to stabilize
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Measure HTTP-to-Valkyrie conversion latency with high precision
        let request_count = 1000;
        let mut latencies = Vec::new();
        
        for _ in 0..request_count {
            let start = std::time::Instant::now();
            let response = client
                .post("http://127.0.0.1:8080/api/v1/jobs")
                .json(&json!({
                    "pipeline": "perf-test"
                }))
                .send()
                .await
                .unwrap();
            let latency = start.elapsed();
            
            // Ensure request was successful
            assert!(response.status().is_success(), "HTTP request should succeed");
            
            latencies.push(latency);
        }
        
        let stats = crate::valkyrie::PerformanceUtils::calculate_percentiles(latencies);
        
        println!("HTTP Bridge Performance:");
        println!("  Min: {:?} ({:.2}μs)", stats.min, stats.min.as_nanos() as f64 / 1000.0);
        println!("  P50: {:?} ({:.2}μs)", stats.p50, stats.p50.as_nanos() as f64 / 1000.0);
        println!("  P95: {:?} ({:.2}μs)", stats.p95, stats.p95.as_nanos() as f64 / 1000.0);
        println!("  P99: {:?} ({:.2}μs)", stats.p99, stats.p99.as_nanos() as f64 / 1000.0);
        println!("  Max: {:?} ({:.2}μs)", stats.max, stats.max.as_nanos() as f64 / 1000.0);
        println!("  Mean: {:?} ({:.2}μs)", stats.mean, stats.mean.as_nanos() as f64 / 1000.0);
        
        // Strict sub-millisecond assertions for HTTP bridge
        TestAssertions::assert_latency_below(stats.p50, Duration::from_micros(800)); // P50 < 800μs
        TestAssertions::assert_latency_below(stats.p95, Duration::from_micros(950)); // P95 < 950μs
        TestAssertions::assert_latency_below(stats.p99, Duration::from_millis(1)); // P99 < 1ms
        
        // Count sub-millisecond HTTP bridge requests
        let sub_ms_count = latencies.iter().filter(|&&lat| lat < Duration::from_millis(1)).count();
        let sub_ms_percentage = (sub_ms_count as f64 / request_count as f64) * 100.0;
        
        println!("  Sub-millisecond HTTP bridge requests: {}/{} ({:.2}%)", 
                sub_ms_count, request_count, sub_ms_percentage);
        
        assert!(
            sub_ms_percentage >= 95.0,
            "At least 95% of HTTP bridge requests should be sub-millisecond, got {:.2}%",
            sub_ms_percentage
        );
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_dns_load_balancer_integration() {
        // This test simulates DNS load balancer integration
        let node_count = 3;
        let mut engines = Vec::new();
        let mut gateways = Vec::new();
        
        // Start multiple Valkyrie nodes
        for i in 0..node_count {
            let mut config = TestFixtures::minimal_config();
            config.node_id = format!("lb-node-{}", i);
            config.bind_port = 8080 + i as u16;
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            
            let gateway = HttpGateway::new(engine.clone()).await.unwrap();
            gateway.start().await.unwrap();
            
            engines.push(engine);
            gateways.push(gateway);
        }
        
        let client = reqwest::Client::new();
        
        // Test load balancing across nodes
        let request_count = 30;
        let mut successful_requests = 0;
        
        for i in 0..request_count {
            let port = 8080 + (i % node_count) as u16;
            let url = format!("http://127.0.0.1:{}/api/v1/jobs", port);
            
            let response = client
                .post(&url)
                .json(&json!({
                    "pipeline": format!("lb-test-{}", i)
                }))
                .send()
                .await;
            
            if response.is_ok() && response.unwrap().status().is_success() {
                successful_requests += 1;
            }
        }
        
        assert!(
            successful_requests >= request_count * 2 / 3,
            "Most load-balanced requests should succeed"
        );
        
        // Cleanup
        for gateway in gateways {
            gateway.stop().await.unwrap();
        }
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_kubernetes_ingress_compatibility() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        let client = reqwest::Client::new();
        
        // Test Kubernetes ingress headers
        let response = client
            .post("http://127.0.0.1:8080/api/v1/jobs")
            .header("X-Forwarded-For", "10.0.0.1")
            .header("X-Forwarded-Proto", "https")
            .header("X-Real-IP", "192.168.1.100")
            .json(&json!({
                "pipeline": "k8s-ingress-test"
            }))
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "Kubernetes ingress request should succeed");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_docker_api_compatibility() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        let client = reqwest::Client::new();
        
        // Test Docker API-style request
        let response = client
            .post("http://127.0.0.1:8080/v1.41/containers/create")
            .header("Content-Type", "application/json")
            .json(&json!({
                "Image": "alpine:latest",
                "Cmd": ["echo", "hello world"]
            }))
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "Docker API request should succeed");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_mcp_like_functionality() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let gateway = HttpGateway::new(engine.clone()).await.unwrap();
        gateway.start().await.unwrap();
        
        let client = reqwest::Client::new();
        
        // Test MCP-like tool discovery
        let response = client
            .get("http://127.0.0.1:8080/mcp/tools")
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "MCP tools discovery should succeed");
        
        let tools: serde_json::Value = response.json().await.unwrap();
        assert!(tools.get("tools").is_some(), "Should return available tools");
        
        // Test MCP-like tool execution
        let response = client
            .post("http://127.0.0.1:8080/mcp/tools/execute")
            .json(&json!({
                "tool": "pipeline_runner",
                "arguments": {
                    "pipeline": "mcp-test-pipeline"
                }
            }))
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success(), "MCP tool execution should succeed");
        
        gateway.stop().await.unwrap();
        engine.stop().await.unwrap();
    }
}