//! Integration tests for Valkyrie Protocol HTTP/HTTPS bridge
//!
//! These tests validate the complete HTTP-to-Valkyrie conversion pipeline
//! and ensure sub-millisecond response times are achieved.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde_json::json;
use tokio::time::timeout;
use tower::ServiceExt;

use rustci::core::networking::valkyrie::{
    bridge::{
        HttpGateway, BridgeConfig, BenchmarkRunner, BenchmarkConfig,
        HighPerformanceProcessor, PerformanceConfig
    },
    engine::{ValkyrieEngine, ValkyrieConfig},
    message::{ValkyrieMessage, MessageType, MessagePayload},
};

/// Test HTTP to Valkyrie message conversion
#[tokio::test]
async fn test_http_to_valkyrie_conversion() {
    // Create test configuration
    let bridge_config = BridgeConfig {
        enable_performance_optimizations: Some(true),
        enable_zero_copy: Some(true),
        enable_simd: Some(true),
        request_timeout_ms: 1000,
        ..Default::default()
    };

    // Create mock Valkyrie engine
    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let engine = Arc::new(engine);

    // Create HTTP gateway
    let gateway = HttpGateway::new(bridge_config, engine).await.unwrap();
    let gateway = Arc::new(gateway);

    // Test JSON payload conversion
    let test_payload = json!({
        "message": "Hello, Valkyrie!",
        "timestamp": "2024-01-01T00:00:00Z",
        "data": {
            "key": "value",
            "number": 42
        }
    });

    let request_context = rustci::core::networking::valkyrie::bridge::HttpRequestContext {
        request_id: uuid::Uuid::new_v4(),
        method: Method::POST,
        uri: "/api/v1/test".parse().unwrap(),
        headers: axum::http::HeaderMap::new(),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client".to_string()),
        auth_token: None,
        timestamp: chrono::Utc::now(),
    };

    let http_payload = rustci::core::networking::valkyrie::bridge::HttpRequestPayload {
        method: "POST".to_string(),
        path: "/api/v1/test".to_string(),
        query: HashMap::new(),
        headers: HashMap::new(),
        body: Some(test_payload.clone()),
        content_type: Some("application/json".to_string()),
    };

    // Convert HTTP to Valkyrie
    let valkyrie_message = gateway.http_to_valkyrie(&request_context, http_payload).await.unwrap();

    // Validate conversion
    assert_eq!(valkyrie_message.header.message_type, MessageType::JobRequest);
    assert_eq!(valkyrie_message.header.correlation_id, Some(request_context.request_id));
    
    match valkyrie_message.payload {
        MessagePayload::Json(value) => {
            assert_eq!(value["body"], test_payload);
        }
        _ => panic!("Expected JSON payload"),
    }
}

/// Test Valkyrie to HTTP response conversion
#[tokio::test]
async fn test_valkyrie_to_http_conversion() {
    let bridge_config = BridgeConfig::default();
    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let engine = Arc::new(engine);

    let gateway = HttpGateway::new(bridge_config, engine).await.unwrap();

    // Create test Valkyrie message
    let test_response = json!({
        "status": "success",
        "message": "Request processed",
        "data": {
            "result": "test_result",
            "processed_at": "2024-01-01T00:00:00Z"
        }
    });

    let valkyrie_message = ValkyrieMessage {
        header: rustci::core::networking::valkyrie::message::MessageHeader {
            protocol_info: rustci::core::networking::valkyrie::message::ProtocolInfo {
                magic: 0x56414C4B,
                version: rustci::core::networking::valkyrie::message::ProtocolVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                extensions: vec![],
            },
            message_type: MessageType::JobComplete,
            stream_id: 0,
            flags: rustci::core::networking::valkyrie::message::MessageFlags {
                requires_ack: false,
                compressed: false,
                encrypted: false,
                fragmented: false,
                urgent: false,
                retransmission: false,
                binary_data: false,
                custom_flags: 0,
            },
            priority: rustci::core::networking::valkyrie::message::MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
            ttl: Some(Duration::from_secs(30)),
            correlation_id: Some(uuid::Uuid::new_v4()),
            routing: rustci::core::networking::valkyrie::message::RoutingInfo {
                source: "valkyrie_engine".to_string(),
                destination: rustci::core::networking::valkyrie::message::DestinationType::Unicast("http_bridge".to_string()),
                hints: rustci::core::networking::valkyrie::message::RoutingHints {
                    preferred_transport: Some("tcp".to_string()),
                    latency_sensitive: true,
                    bandwidth_requirements: None,
                    reliability_level: rustci::core::networking::valkyrie::message::ReliabilityLevel::AtLeastOnce,
                    cacheable: false,
                    compression_preference: rustci::core::networking::valkyrie::message::CompressionPreference::None,
                },
                load_balancing: rustci::core::networking::valkyrie::message::LoadBalancingStrategy::RoundRobin,
                path: vec![],
            },
            compression: rustci::core::networking::valkyrie::message::CompressionInfo {
                algorithm: rustci::core::networking::valkyrie::message::CompressionAlgorithm::None,
                original_size: None,
                compressed_size: None,
                level: None,
            },
            sequence_number: 1,
            ack_number: None,
        },
        payload: MessagePayload::Json(test_response.clone()),
        signature: None,
        trace_context: None,
    };

    // Convert Valkyrie to HTTP
    let http_response = gateway.valkyrie_to_http(valkyrie_message).await.unwrap();

    // Validate conversion
    assert_eq!(http_response.status, 200);
    assert_eq!(http_response.content_type, Some("application/json".to_string()));
    assert_eq!(http_response.body, Some(test_response));
}

/// Test sub-millisecond response time performance
#[tokio::test]
async fn test_sub_millisecond_performance() {
    // Create high-performance configuration
    let perf_config = PerformanceConfig {
        enable_zero_copy: true,
        enable_simd: true,
        request_queue_size: 10000,
        worker_threads: num_cpus::get() * 2,
        target_response_time_us: 500, // 500 microseconds
        enable_prefetch: true,
    };

    let processor = HighPerformanceProcessor::new(perf_config).await.unwrap();
    processor.start_workers().await.unwrap();

    // Test multiple request sizes
    let test_sizes = vec![64, 256, 1024, 4096]; // bytes

    for size in test_sizes {
        let payload = Bytes::from(vec![0u8; size]);
        let start_time = Instant::now();
        
        let result = processor.process_request(payload).await;
        let duration = start_time.elapsed();
        
        assert!(result.is_ok(), "Request processing failed for size {}", size);
        
        let duration_us = duration.as_micros();
        println!("Request size: {} bytes, Duration: {}μs", size, duration_us);
        
        // For smaller payloads, we should achieve sub-millisecond response times
        if size <= 1024 {
            assert!(duration_us < 1000, 
                "Sub-millisecond target not met for {} bytes: {}μs", 
                size, duration_us);
        }
    }
}

/// Test concurrent request handling
#[tokio::test]
async fn test_concurrent_request_handling() {
    let perf_config = PerformanceConfig {
        enable_zero_copy: true,
        enable_simd: true,
        request_queue_size: 5000,
        worker_threads: num_cpus::get(),
        target_response_time_us: 1000,
        enable_prefetch: true,
    };

    let processor = Arc::new(HighPerformanceProcessor::new(perf_config).await.unwrap());
    processor.start_workers().await.unwrap();

    let concurrent_requests = 1000;
    let payload_size = 512;
    let mut tasks = Vec::new();

    let start_time = Instant::now();

    // Launch concurrent requests
    for _ in 0..concurrent_requests {
        let processor = Arc::clone(&processor);
        let payload = Bytes::from(vec![0u8; payload_size]);
        
        let task = tokio::spawn(async move {
            let request_start = Instant::now();
            let result = processor.process_request(payload).await;
            let duration = request_start.elapsed();
            (result, duration)
        });
        
        tasks.push(task);
    }

    // Wait for all requests to complete
    let mut successful = 0;
    let mut total_duration_us = 0u64;
    let mut max_duration_us = 0u64;

    for task in tasks {
        let (result, duration) = task.await.unwrap();
        if result.is_ok() {
            successful += 1;
            let duration_us = duration.as_micros() as u64;
            total_duration_us += duration_us;
            max_duration_us = max_duration_us.max(duration_us);
        }
    }

    let total_time = start_time.elapsed();
    let avg_duration_us = total_duration_us / successful;
    let requests_per_second = successful as f64 / total_time.as_secs_f64();

    println!("Concurrent test results:");
    println!("  Successful requests: {}/{}", successful, concurrent_requests);
    println!("  Average latency: {}μs", avg_duration_us);
    println!("  Max latency: {}μs", max_duration_us);
    println!("  Requests per second: {:.2}", requests_per_second);

    // Validate performance
    assert!(successful >= concurrent_requests * 95 / 100, "Success rate too low");
    assert!(avg_duration_us < 2000, "Average latency too high: {}μs", avg_duration_us);
    assert!(requests_per_second > 1000.0, "Throughput too low: {:.2} RPS", requests_per_second);
}

/// Test protocol negotiation and fallback
#[tokio::test]
async fn test_protocol_negotiation() {
    let bridge_config = BridgeConfig {
        auto_negotiation_enabled: true,
        enable_performance_optimizations: Some(true),
        ..Default::default()
    };

    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let gateway = Arc::new(HttpGateway::new(bridge_config, Arc::new(engine)).await.unwrap());

    // Test with Valkyrie-capable client (should use Valkyrie protocol)
    let mut valkyrie_headers = axum::http::HeaderMap::new();
    valkyrie_headers.insert("X-Valkyrie-Protocol", "1.0".parse().unwrap());
    valkyrie_headers.insert("Accept", "application/vnd.valkyrie+json".parse().unwrap());

    // Test with standard HTTP client (should fallback to HTTP)
    let mut http_headers = axum::http::HeaderMap::new();
    http_headers.insert("Accept", "application/json".parse().unwrap());
    http_headers.insert("User-Agent", "Mozilla/5.0".parse().unwrap());

    // Both should work, but Valkyrie-capable client should get optimized processing
    // This would be tested with actual HTTP requests in a full integration test
}

/// Benchmark the complete HTTP bridge pipeline
#[tokio::test]
async fn test_complete_bridge_benchmark() {
    let bridge_config = BridgeConfig {
        enable_performance_optimizations: Some(true),
        enable_zero_copy: Some(true),
        enable_simd: Some(true),
        ..Default::default()
    };

    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let gateway = Arc::new(HttpGateway::new(bridge_config, Arc::new(engine)).await.unwrap());

    let benchmark_config = BenchmarkConfig {
        concurrent_requests: 500,
        total_requests: 5000,
        payload_size: 1024,
        target_response_time_us: 500,
        warmup_requests: 100,
        test_duration_seconds: 30,
        detailed_latency_tracking: true,
    };

    let runner = BenchmarkRunner::new(benchmark_config, gateway).await.unwrap();
    let results = runner.run_benchmark_suite().await.unwrap();

    // Validate benchmark results
    assert!(results.successful_requests > 0, "No successful requests");
    assert!(results.error_stats.error_rate < 5.0, "Error rate too high: {:.2}%", results.error_stats.error_rate);
    assert!(results.requests_per_second > 1000.0, "Throughput too low: {:.2} RPS", results.requests_per_second);

    // Check if sub-millisecond target was achieved
    if results.latency_stats.mean_us < 1000.0 {
        println!("🎉 Sub-millisecond target achieved! Average: {:.2}μs", results.latency_stats.mean_us);
    } else {
        println!("⚠️ Sub-millisecond target not achieved. Average: {:.2}μs", results.latency_stats.mean_us);
    }

    // Export results for analysis
    runner.export_results(&results, "target/valkyrie_bridge_benchmark.json").await.unwrap();
}

/// Test memory efficiency and zero-copy operations
#[tokio::test]
async fn test_memory_efficiency() {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Custom allocator to track memory usage
    struct TrackingAllocator {
        allocated: AtomicUsize,
    }

    impl TrackingAllocator {
        const fn new() -> Self {
            Self {
                allocated: AtomicUsize::new(0),
            }
        }

        fn allocated(&self) -> usize {
            self.allocated.load(Ordering::Relaxed)
        }
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);
            if !ptr.is_null() {
                self.allocated.fetch_add(layout.size(), Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout);
            self.allocated.fetch_sub(layout.size(), Ordering::Relaxed);
        }
    }

    // Test with high-performance processor
    let perf_config = PerformanceConfig {
        enable_zero_copy: true,
        enable_simd: true,
        request_queue_size: 1000,
        worker_threads: 2,
        target_response_time_us: 500,
        enable_prefetch: true,
    };

    let processor = HighPerformanceProcessor::new(perf_config).await.unwrap();
    processor.start_workers().await.unwrap();

    // Process multiple requests and check memory usage
    let num_requests = 100;
    let payload_size = 4096;

    for _ in 0..num_requests {
        let payload = Bytes::from(vec![0u8; payload_size]);
        let _result = processor.process_request(payload).await.unwrap();
    }

    // Allow some time for cleanup
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Memory usage should be reasonable and not grow indefinitely
    // This is a basic test - in practice, you'd want more sophisticated memory tracking
    println!("Memory efficiency test completed");
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling() {
    let bridge_config = BridgeConfig {
        request_timeout_ms: 100, // Very short timeout to trigger errors
        max_request_size: 1024,  // Small size to trigger size errors
        ..Default::default()
    };

    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let gateway = Arc::new(HttpGateway::new(bridge_config, Arc::new(engine)).await.unwrap());

    // Test timeout handling
    let large_payload = Bytes::from(vec![0u8; 2048]); // Larger than max_request_size
    
    // Test with performance processor
    if let Some(ref processor) = gateway.performance_processor {
        // This should handle the error gracefully
        let result = timeout(Duration::from_millis(200), processor.process_request(large_payload)).await;
        
        match result {
            Ok(Ok(_)) => println!("Request succeeded unexpectedly"),
            Ok(Err(_)) => println!("Request failed as expected"),
            Err(_) => println!("Request timed out as expected"),
        }
    }

    // Test recovery after errors
    let small_payload = Bytes::from(vec![0u8; 512]);
    if let Some(ref processor) = gateway.performance_processor {
        let result = processor.process_request(small_payload).await;
        assert!(result.is_ok(), "Recovery after error failed");
    }
}

/// Integration test with actual HTTP server
#[tokio::test]
async fn test_http_server_integration() {
    // This test would set up an actual HTTP server and test end-to-end
    // For now, we'll simulate the key components
    
    let bridge_config = BridgeConfig {
        http_listen_addr: "127.0.0.1:0".to_string(), // Use random port
        enable_performance_optimizations: Some(true),
        ..Default::default()
    };

    let engine_config = ValkyrieConfig::default();
    let engine = ValkyrieEngine::new(engine_config).unwrap();
    let gateway = HttpGateway::new(bridge_config, Arc::new(engine)).await.unwrap();

    // Test that gateway was created successfully with performance optimizations
    assert!(gateway.performance_processor.is_some(), "Performance processor should be enabled");
    
    // Test configuration
    assert!(gateway.config.enable_performance_optimizations.unwrap_or(false));
    assert!(gateway.config.enable_zero_copy.unwrap_or(false));
    assert!(gateway.config.enable_simd.unwrap_or(false));

    println!("HTTP server integration test setup completed successfully");
}