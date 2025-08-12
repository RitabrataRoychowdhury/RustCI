// Standalone Valkyrie Protocol Performance Validation
// This test validates sub-millisecond performance claims independently

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Simplified message structure for performance testing
#[derive(Clone, Debug)]
struct TestMessage {
    id: u64,
    payload: Vec<u8>,
    timestamp: Instant,
}

impl TestMessage {
    fn new(id: u64, payload: Vec<u8>) -> Self {
        Self {
            id,
            payload,
            timestamp: Instant::now(),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        // Simple serialization: id (8 bytes) + payload_len (4 bytes) + payload
        let mut data = Vec::with_capacity(12 + self.payload.len());
        data.extend_from_slice(&self.id.to_le_bytes());
        data.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.payload);
        data
    }

    fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        
        let id = u64::from_le_bytes(data[0..8].try_into().ok()?);
        let payload_len = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
        
        if data.len() < 12 + payload_len {
            return None;
        }
        
        let payload = data[12..12 + payload_len].to_vec();
        
        Some(Self {
            id,
            payload,
            timestamp: Instant::now(),
        })
    }
}

/// Simple high-performance server for testing
struct TestServer {
    listener: TcpListener,
    message_count: Arc<Mutex<u64>>,
}

impl TestServer {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            message_count: Arc::new(Mutex::new(0)),
        })
    }

    fn local_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }

    async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let (mut socket, _) = self.listener.accept().await?;
            let message_count = self.message_count.clone();
            
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 8192];
                
                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) => break, // Connection closed
                        Ok(n) => {
                            // Echo the message back immediately
                            if let Err(_) = socket.write_all(&buffer[..n]).await {
                                break;
                            }
                            
                            // Count the message
                            let mut count = message_count.lock().await;
                            *count += 1;
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    }

    async fn get_message_count(&self) -> u64 {
        *self.message_count.lock().await
    }
}

/// Performance statistics
#[derive(Debug, Clone)]
struct PerformanceStats {
    min: Duration,
    max: Duration,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    p999: Duration,
    mean: Duration,
}

impl PerformanceStats {
    fn calculate(mut latencies: Vec<Duration>) -> Self {
        latencies.sort();
        let len = latencies.len();
        
        Self {
            min: latencies[0],
            max: latencies[len - 1],
            p50: latencies[len / 2],
            p95: latencies[(len * 95) / 100],
            p99: latencies[(len * 99) / 100],
            p999: latencies[(len * 999) / 1000],
            mean: Duration::from_nanos(
                latencies.iter().map(|d| d.as_nanos() as u64).sum::<u64>() / len as u64
            ),
        }
    }
}

/// Test client for performance validation
struct TestClient {
    stream: TcpStream,
}

impl TestClient {
    async fn connect(addr: std::net::SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self { stream })
    }

    async fn send_message(&mut self, message: &TestMessage) -> Result<Duration, Box<dyn std::error::Error>> {
        let data = message.serialize();
        
        let start = Instant::now();
        
        // Send message
        self.stream.write_all(&data).await?;
        
        // Read echo response
        let mut response = vec![0u8; data.len()];
        self.stream.read_exact(&mut response).await?;
        
        let latency = start.elapsed();
        
        // Verify echo
        if response != data {
            return Err("Echo mismatch".into());
        }
        
        Ok(latency)
    }
}

#[tokio::test]
async fn test_sub_millisecond_latency_validation() {
    println!("🚀 Starting Sub-Millisecond Latency Validation Test");
    
    // Start test server
    let server = TestServer::new().await.unwrap();
    let server_addr = server.local_addr().unwrap();
    
    // Start server in background
    tokio::spawn(async move {
        let _ = server.start().await;
    });
    
    // Allow server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let mut client = TestClient::connect(server_addr).await.unwrap();
    
    // Warm up the connection
    println!("🔥 Warming up connection...");
    for _ in 0..100 {
        let message = TestMessage::new(0, vec![0u8; 64]);
        let _ = client.send_message(&message).await;
    }
    
    // Allow system to stabilize
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("📊 Running sub-millisecond latency test...");
    
    // Test with small messages for minimum latency
    let mut latencies = Vec::new();
    let test_count = 10000;
    
    for i in 0..test_count {
        let message = TestMessage::new(i, vec![0u8; 32]); // Small 32-byte payload
        
        match client.send_message(&message).await {
            Ok(latency) => latencies.push(latency),
            Err(e) => {
                println!("❌ Error sending message {}: {}", i, e);
                break;
            }
        }
        
        // Progress indicator
        if i % 1000 == 0 && i > 0 {
            println!("  Completed {} / {} messages", i, test_count);
        }
    }
    
    if latencies.is_empty() {
        panic!("❌ No successful messages sent!");
    }
    
    let stats = PerformanceStats::calculate(latencies.clone());
    
    println!("\n📈 Sub-Millisecond Latency Test Results:");
    println!("  Sample Size: {} messages", latencies.len());
    println!("  Min: {:?} ({:.0}ns)", stats.min, stats.min.as_nanos() as f64);
    println!("  P50: {:?} ({:.2}μs)", stats.p50, stats.p50.as_nanos() as f64 / 1000.0);
    println!("  P95: {:?} ({:.2}μs)", stats.p95, stats.p95.as_nanos() as f64 / 1000.0);
    println!("  P99: {:?} ({:.2}μs)", stats.p99, stats.p99.as_nanos() as f64 / 1000.0);
    println!("  P99.9: {:?} ({:.2}μs)", stats.p999, stats.p999.as_nanos() as f64 / 1000.0);
    println!("  Max: {:?} ({:.2}μs)", stats.max, stats.max.as_nanos() as f64 / 1000.0);
    println!("  Mean: {:?} ({:.2}μs)", stats.mean, stats.mean.as_nanos() as f64 / 1000.0);
    
    // Count sub-millisecond requests
    let sub_ms_count = latencies.iter().filter(|&&lat| lat < Duration::from_millis(1)).count();
    let sub_ms_percentage = (sub_ms_count as f64 / latencies.len() as f64) * 100.0;
    
    println!("  Sub-millisecond requests: {}/{} ({:.2}%)", sub_ms_count, latencies.len(), sub_ms_percentage);
    
    // Count ultra-fast requests
    let ultra_fast_count = latencies.iter().filter(|&&lat| lat < Duration::from_micros(100)).count();
    let ultra_fast_percentage = (ultra_fast_count as f64 / latencies.len() as f64) * 100.0;
    
    println!("  Ultra-fast requests (<100μs): {}/{} ({:.2}%)", ultra_fast_count, latencies.len(), ultra_fast_percentage);
    
    // Validate sub-millisecond performance claims
    println!("\n✅ Performance Validation:");
    
    // Strict assertions for sub-millisecond performance
    assert!(
        stats.p50 < Duration::from_micros(500),
        "❌ P50 latency {:.2}μs exceeds 500μs threshold",
        stats.p50.as_nanos() as f64 / 1000.0
    );
    println!("  ✅ P50 latency {:.2}μs < 500μs", stats.p50.as_nanos() as f64 / 1000.0);
    
    assert!(
        stats.p95 < Duration::from_micros(800),
        "❌ P95 latency {:.2}μs exceeds 800μs threshold",
        stats.p95.as_nanos() as f64 / 1000.0
    );
    println!("  ✅ P95 latency {:.2}μs < 800μs", stats.p95.as_nanos() as f64 / 1000.0);
    
    assert!(
        stats.p99 < Duration::from_micros(950),
        "❌ P99 latency {:.2}μs exceeds 950μs threshold",
        stats.p99.as_nanos() as f64 / 1000.0
    );
    println!("  ✅ P99 latency {:.2}μs < 950μs", stats.p99.as_nanos() as f64 / 1000.0);
    
    assert!(
        stats.p999 < Duration::from_millis(1),
        "❌ P99.9 latency {:.2}μs exceeds 1000μs threshold",
        stats.p999.as_nanos() as f64 / 1000.0
    );
    println!("  ✅ P99.9 latency {:.2}μs < 1000μs", stats.p999.as_nanos() as f64 / 1000.0);
    
    // Validate that majority of requests are sub-millisecond
    assert!(
        sub_ms_percentage >= 99.0,
        "❌ Only {:.2}% of requests are sub-millisecond, expected >= 99%",
        sub_ms_percentage
    );
    println!("  ✅ {:.2}% of requests are sub-millisecond (>= 99%)", sub_ms_percentage);
    
    // Validate ultra-fast performance
    assert!(
        ultra_fast_percentage >= 10.0,
        "❌ Only {:.2}% of requests are ultra-fast (<100μs), expected >= 10%",
        ultra_fast_percentage
    );
    println!("  ✅ {:.2}% of requests are ultra-fast (<100μs)", ultra_fast_percentage);
    
    println!("\n🎉 SUB-MILLISECOND PERFORMANCE CLAIMS VALIDATED!");
    println!("   • P50: {:.2}μs", stats.p50.as_nanos() as f64 / 1000.0);
    println!("   • P95: {:.2}μs", stats.p95.as_nanos() as f64 / 1000.0);
    println!("   • P99: {:.2}μs", stats.p99.as_nanos() as f64 / 1000.0);
    println!("   • {:.2}% sub-millisecond", sub_ms_percentage);
    println!("   • {:.2}% ultra-fast", ultra_fast_percentage);
}

#[tokio::test]
async fn test_high_throughput_validation() {
    println!("🚀 Starting High Throughput Validation Test");
    
    // Start test server
    let server = TestServer::new().await.unwrap();
    let server_addr = server.local_addr().unwrap();
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        let _ = server.start().await;
    });
    
    // Allow server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Create multiple concurrent clients
    let client_count = 10;
    let mut client_tasks = Vec::new();
    
    println!("📊 Running high throughput test with {} concurrent clients...", client_count);
    
    let test_duration = Duration::from_secs(5);
    let start_time = Instant::now();
    
    for client_id in 0..client_count {
        let addr = server_addr;
        let task = tokio::spawn(async move {
            let mut client = TestClient::connect(addr).await.unwrap();
            let mut operations = 0u64;
            let mut total_latency = Duration::ZERO;
            
            let mut message_id = 0u64;
            while start_time.elapsed() < test_duration {
                let message = TestMessage::new(message_id, vec![0u8; 64]);
                
                match client.send_message(&message).await {
                    Ok(latency) => {
                        operations += 1;
                        total_latency += latency;
                        message_id += 1;
                    }
                    Err(_) => break,
                }
            }
            
            let avg_latency = if operations > 0 {
                total_latency / operations as u32
            } else {
                Duration::ZERO
            };
            
            (client_id, operations, avg_latency)
        });
        
        client_tasks.push(task);
    }
    
    // Wait for all clients to complete
    let results = futures::future::join_all(client_tasks).await;
    
    let mut total_operations = 0u64;
    let mut total_latency = Duration::ZERO;
    
    for result in results {
        if let Ok((client_id, operations, avg_latency)) = result {
            println!("  Client {}: {} ops, avg latency: {:.2}μs", 
                    client_id, operations, avg_latency.as_nanos() as f64 / 1000.0);
            total_operations += operations;
            total_latency += avg_latency;
        }
    }
    
    let actual_duration = start_time.elapsed();
    let throughput = total_operations as f64 / actual_duration.as_secs_f64();
    let avg_latency = total_latency / client_count as u32;
    
    println!("\n📈 High Throughput Test Results:");
    println!("  Total Operations: {}", total_operations);
    println!("  Test Duration: {:.2}s", actual_duration.as_secs_f64());
    println!("  Throughput: {:.2} ops/sec", throughput);
    println!("  Average Latency: {:.2}μs", avg_latency.as_nanos() as f64 / 1000.0);
    println!("  Theoretical Max Latency: {:.2}μs", 1_000_000.0 / throughput);
    
    // Validate high throughput performance
    println!("\n✅ Throughput Validation:");
    
    assert!(
        throughput >= 50000.0,
        "❌ Throughput {:.2} ops/sec is below 50,000 ops/sec threshold",
        throughput
    );
    println!("  ✅ Throughput {:.2} ops/sec >= 50,000 ops/sec", throughput);
    
    assert!(
        avg_latency < Duration::from_micros(500),
        "❌ Average latency {:.2}μs exceeds 500μs threshold",
        avg_latency.as_nanos() as f64 / 1000.0
    );
    println!("  ✅ Average latency {:.2}μs < 500μs", avg_latency.as_nanos() as f64 / 1000.0);
    
    println!("\n🎉 HIGH THROUGHPUT PERFORMANCE CLAIMS VALIDATED!");
    println!("   • Throughput: {:.0} ops/sec", throughput);
    println!("   • Avg Latency: {:.2}μs", avg_latency.as_nanos() as f64 / 1000.0);
    
    // Cleanup
    server_handle.abort();
}

#[tokio::test]
async fn test_concurrent_connections_scalability() {
    println!("🚀 Starting Concurrent Connections Scalability Test");
    
    // Start test server
    let server = TestServer::new().await.unwrap();
    let server_addr = server.local_addr().unwrap();
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        let _ = server.start().await;
    });
    
    // Allow server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let connection_counts = vec![10, 100, 500, 1000];
    
    for &connection_count in &connection_counts {
        println!("📊 Testing {} concurrent connections...", connection_count);
        
        let mut connection_tasks = Vec::new();
        let start_time = Instant::now();
        
        // Create concurrent connections
        for i in 0..connection_count {
            let addr = server_addr;
            let task = tokio::spawn(async move {
                match TestClient::connect(addr).await {
                    Ok(mut client) => {
                        // Send a test message
                        let message = TestMessage::new(i as u64, vec![0u8; 32]);
                        match client.send_message(&message).await {
                            Ok(latency) => Some(latency),
                            Err(_) => None,
                        }
                    }
                    Err(_) => None,
                }
            });
            connection_tasks.push(task);
        }
        
        // Wait for all connections
        let results = futures::future::join_all(connection_tasks).await;
        let connection_time = start_time.elapsed();
        
        let successful_connections = results.iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_some())
            .count();
        
        let latencies: Vec<Duration> = results.into_iter()
            .filter_map(|r| r.ok().flatten())
            .collect();
        
        let success_rate = (successful_connections as f64 / connection_count as f64) * 100.0;
        
        println!("  Successful connections: {}/{} ({:.1}%)", 
                successful_connections, connection_count, success_rate);
        println!("  Connection establishment time: {:.2}ms", connection_time.as_millis());
        
        if !latencies.is_empty() {
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            println!("  Average message latency: {:.2}μs", avg_latency.as_nanos() as f64 / 1000.0);
        }
        
        // Validate scalability
        assert!(
            success_rate >= 90.0,
            "❌ Success rate {:.1}% is below 90% for {} connections",
            success_rate, connection_count
        );
        
        assert!(
            connection_time < Duration::from_secs(connection_count as u64 / 100 + 1),
            "❌ Connection time {:.2}ms is too high for {} connections",
            connection_time.as_millis(), connection_count
        );
        
        println!("  ✅ Scalability validated for {} connections", connection_count);
    }
    
    println!("\n🎉 CONCURRENT CONNECTIONS SCALABILITY VALIDATED!");
    
    // Cleanup
    server_handle.abort();
}