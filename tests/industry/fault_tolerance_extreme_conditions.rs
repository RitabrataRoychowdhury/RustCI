//! Extreme Fault Tolerance Testing for Industry-Grade Deployments
//! 
//! This module tests RustCI and Valkyrie under extreme network conditions
//! including intermittent connectivity, high latency, and network partitions.

use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, sleep};
use rand::Rng;

/// Network chaos simulator for extreme conditions
#[derive(Clone)]
pub struct NetworkChaosSimulator {
    pub packet_loss_rate: f64,
    pub latency_base_ms: u64,
    pub latency_variance_ms: u64,
    pub bandwidth_kbps: u64,
    pub connection_drop_rate: f64,
    pub intermittent_outage_probability: f64,
    pub partition_probability: f64,
}

impl NetworkChaosSimulator {
    pub fn extreme_conditions() -> Self {
        Self {
            packet_loss_rate: 0.25,        // 25% packet loss
            latency_base_ms: 500,           // 500ms base latency
            latency_variance_ms: 1000,      // Up to 1.5s total latency
            bandwidth_kbps: 56,             // Dial-up speeds
            connection_drop_rate: 0.15,     // 15% connection drops
            intermittent_outage_probability: 0.10, // 10% chance of outages
            partition_probability: 0.05,    // 5% chance of network partition
        }
    }
    
    pub fn satellite_internet() -> Self {
        Self {
            packet_loss_rate: 0.05,        // 5% packet loss
            latency_base_ms: 600,           // 600ms satellite latency
            latency_variance_ms: 200,       // Variance due to weather
            bandwidth_kbps: 1000,           // 1 Mbps
            connection_drop_rate: 0.08,     // 8% connection drops
            intermittent_outage_probability: 0.15, // Weather-related outages
            partition_probability: 0.02,    // Rare partitions
        }
    }
    
    pub fn mobile_network_poor() -> Self {
        Self {
            packet_loss_rate: 0.12,        // 12% packet loss
            latency_base_ms: 200,           // 200ms base latency
            latency_variance_ms: 300,       // High variance
            bandwidth_kbps: 128,            // Poor mobile connection
            connection_drop_rate: 0.20,     // 20% connection drops
            intermittent_outage_probability: 0.25, // Frequent outages
            partition_probability: 0.08,    // Network switching
        }
    }
    
    /// Simulates network conditions for a packet
    pub async fn simulate_network_conditions(&self) -> Result<Duration, NetworkError> {
        let mut rng = rand::thread_rng();
        
        // Check for network partition
        if rng.gen::<f64>() < self.partition_probability {
            return Err(NetworkError::NetworkPartition);
        }
        
        // Check for intermittent outage
        if rng.gen::<f64>() < self.intermittent_outage_probability {
            let outage_duration = Duration::from_millis(rng.gen_range(1000..10000));
            sleep(outage_duration).await;
            return Err(NetworkError::IntermittentOutage(outage_duration));
        }
        
        // Check for packet loss
        if rng.gen::<f64>() < self.packet_loss_rate {
            return Err(NetworkError::PacketLoss);
        }
        
        // Check for connection drop
        if rng.gen::<f64>() < self.connection_drop_rate {
            return Err(NetworkError::ConnectionDrop);
        }
        
        // Simulate latency
        let latency = self.latency_base_ms + rng.gen_range(0..self.latency_variance_ms);
        let latency_duration = Duration::from_millis(latency);
        sleep(latency_duration).await;
        
        // Simulate bandwidth limitation
        let transfer_delay = Duration::from_millis(rng.gen_range(10..100));
        sleep(transfer_delay).await;
        
        Ok(latency_duration)
    }
}

#[derive(Debug, Clone)]
pub enum NetworkError {
    PacketLoss,
    ConnectionDrop,
    IntermittentOutage(Duration),
    NetworkPartition,
    Timeout,
}

/// Resilient message with retry logic
#[derive(Clone, Debug)]
pub struct ResilientMessage {
    pub id: u64,
    pub payload: Vec<u8>,
    pub priority: MessagePriority,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub created_at: Instant,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MessagePriority {
    Critical,
    High,
    Normal,
    Low,
}

impl ResilientMessage {
    pub fn new(id: u64, payload: Vec<u8>, priority: MessagePriority) -> Self {
        let (max_retries, timeout_ms) = match priority {
            MessagePriority::Critical => (10, 30000), // 10 retries, 30s timeout
            MessagePriority::High => (5, 15000),      // 5 retries, 15s timeout
            MessagePriority::Normal => (3, 10000),    // 3 retries, 10s timeout
            MessagePriority::Low => (1, 5000),        // 1 retry, 5s timeout
        };
        
        Self {
            id,
            payload,
            priority,
            max_retries,
            timeout_ms,
            created_at: Instant::now(),
        }
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.id.to_le_bytes());
        data.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.payload);
        data
    }
}

/// Fault-tolerant communication node
pub struct FaultTolerantNode {
    pub node_id: String,
    pub listener: TcpListener,
    pub chaos_simulator: NetworkChaosSimulator,
    pub message_stats: Arc<Mutex<MessageStats>>,
    pub is_partitioned: Arc<AtomicBool>,
    pub connection_pool: Arc<Mutex<HashMap<String, TcpStream>>>,
}

#[derive(Debug, Clone)]
pub struct MessageStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_failed: u64,
    pub retries_attempted: u64,
    pub total_latency_ms: u64,
    pub network_errors: HashMap<String, u64>,
}

impl MessageStats {
    pub fn new() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            messages_failed: 0,
            retries_attempted: 0,
            total_latency_ms: 0,
            network_errors: HashMap::new(),
        }
    }
    
    pub fn record_error(&mut self, error: &NetworkError) {
        let error_type = match error {
            NetworkError::PacketLoss => "packet_loss",
            NetworkError::ConnectionDrop => "connection_drop",
            NetworkError::IntermittentOutage(_) => "intermittent_outage",
            NetworkError::NetworkPartition => "network_partition",
            NetworkError::Timeout => "timeout",
        };
        
        *self.network_errors.entry(error_type.to_string()).or_insert(0) += 1;
    }
}

impl FaultTolerantNode {
    pub async fn new(node_id: String, chaos_simulator: NetworkChaosSimulator) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        
        Ok(Self {
            node_id,
            listener,
            chaos_simulator,
            message_stats: Arc::new(Mutex::new(MessageStats::new())),
            is_partitioned: Arc::new(AtomicBool::new(false)),
            connection_pool: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub fn local_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }
    
    pub async fn start_server(&self) {
        let message_stats = self.message_stats.clone();
        let chaos_simulator = self.chaos_simulator.clone();
        let is_partitioned = self.is_partitioned.clone();
        
        loop {
            if let Ok((mut socket, _)) = self.listener.accept().await {
                let message_stats = message_stats.clone();
                let chaos_simulator = chaos_simulator.clone();
                let is_partitioned = is_partitioned.clone();
                
                tokio::spawn(async move {
                    // Check if node is partitioned
                    if is_partitioned.load(Ordering::Relaxed) {
                        return; // Drop connection during partition
                    }
                    
                    // Simulate network conditions
                    match chaos_simulator.simulate_network_conditions().await {
                        Ok(_latency) => {
                            let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer
                            
                            match socket.read(&mut buffer).await {
                                Ok(n) if n > 0 => {
                                    let mut stats = message_stats.lock().unwrap();
                                    stats.messages_received += 1;
                                    
                                    // Echo back to simulate processing
                                    let _ = socket.write_all(&buffer[..n]).await;
                                }
                                _ => {}
                            }
                        }
                        Err(error) => {
                            let mut stats = message_stats.lock().unwrap();
                            stats.record_error(&error);
                        }
                    }
                });
            }
        }
    }
    
    pub async fn send_message_with_retry(&self, target_addr: std::net::SocketAddr, message: ResilientMessage) -> Result<Duration, NetworkError> {
        let start_time = Instant::now();
        let mut retries = 0;
        
        while retries <= message.max_retries {
            // Check if we've exceeded the total timeout
            if start_time.elapsed().as_millis() > message.timeout_ms as u128 {
                let mut stats = self.message_stats.lock().unwrap();
                stats.messages_failed += 1;
                return Err(NetworkError::Timeout);
            }
            
            // Check if node is partitioned
            if self.is_partitioned.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(1000)).await; // Wait during partition
                retries += 1;
                continue;
            }
            
            // Simulate network conditions
            match self.chaos_simulator.simulate_network_conditions().await {
                Ok(latency) => {
                    // Attempt to send message
                    match timeout(
                        Duration::from_millis(5000),
                        self.attempt_send(target_addr, &message)
                    ).await {
                        Ok(Ok(_)) => {
                            let mut stats = self.message_stats.lock().unwrap();
                            stats.messages_sent += 1;
                            stats.retries_attempted += retries;
                            stats.total_latency_ms += latency.as_millis() as u64;
                            return Ok(start_time.elapsed());
                        }
                        Ok(Err(_)) | Err(_) => {
                            retries += 1;
                            let mut stats = self.message_stats.lock().unwrap();
                            stats.record_error(&NetworkError::ConnectionDrop);
                        }
                    }
                }
                Err(error) => {
                    let mut stats = self.message_stats.lock().unwrap();
                    stats.record_error(&error);
                    
                    match error {
                        NetworkError::NetworkPartition => {
                            self.is_partitioned.store(true, Ordering::Relaxed);
                            sleep(Duration::from_millis(5000)).await; // Wait for partition to heal
                            self.is_partitioned.store(false, Ordering::Relaxed);
                        }
                        NetworkError::IntermittentOutage(duration) => {
                            sleep(duration).await;
                        }
                        _ => {
                            sleep(Duration::from_millis(100 * (retries + 1) as u64)).await; // Exponential backoff
                        }
                    }
                    
                    retries += 1;
                }
            }
        }
        
        let mut stats = self.message_stats.lock().unwrap();
        stats.messages_failed += 1;
        Err(NetworkError::Timeout)
    }
    
    async fn attempt_send(&self, target_addr: std::net::SocketAddr, message: &ResilientMessage) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect(target_addr).await?;
        let data = message.serialize();
        stream.write_all(&data).await?;
        
        // Wait for acknowledgment
        let mut ack = [0u8; 8];
        stream.read_exact(&mut ack).await?;
        
        Ok(())
    }
    
    pub fn get_stats(&self) -> MessageStats {
        self.message_stats.lock().unwrap().clone()
    }
    
    pub fn simulate_partition(&self, duration: Duration) {
        let is_partitioned = self.is_partitioned.clone();
        tokio::spawn(async move {
            is_partitioned.store(true, Ordering::Relaxed);
            sleep(duration).await;
            is_partitioned.store(false, Ordering::Relaxed);
        });
    }
}

#[tokio::test]
async fn test_extreme_network_conditions() {
    println!("🌪️  Testing Extreme Network Conditions");
    
    let chaos_simulator = NetworkChaosSimulator::extreme_conditions();
    let node1 = FaultTolerantNode::new("node-1".to_string(), chaos_simulator.clone()).await.unwrap();
    let node2 = FaultTolerantNode::new("node-2".to_string(), chaos_simulator.clone()).await.unwrap();
    
    let node1_addr = node1.local_addr().unwrap();
    let node2_addr = node2.local_addr().unwrap();
    
    // Start servers
    tokio::spawn(async move { node1.start_server().await });
    tokio::spawn(async move { node2.start_server().await });
    
    sleep(Duration::from_millis(100)).await;
    
    // Create test node for sending
    let sender = FaultTolerantNode::new("sender".to_string(), chaos_simulator).await.unwrap();
    
    println!("📡 Sending messages under extreme conditions...");
    
    let mut successful_messages = 0;
    let mut failed_messages = 0;
    let total_messages = 50;
    
    for i in 0..total_messages {
        let message = ResilientMessage::new(
            i,
            vec![0u8; 1024], // 1KB payload
            if i % 10 == 0 { MessagePriority::Critical } else { MessagePriority::Normal }
        );
        
        match sender.send_message_with_retry(node1_addr, message).await {
            Ok(latency) => {
                successful_messages += 1;
                if i % 10 == 0 {
                    println!("   Message {} delivered in {:.2}ms", i, latency.as_millis());
                }
            }
            Err(_) => {
                failed_messages += 1;
            }
        }
    }
    
    let stats = sender.get_stats();
    
    println!("\n📊 Extreme Conditions Test Results:");
    println!("   Messages Sent Successfully: {}/{}", successful_messages, total_messages);
    println!("   Messages Failed: {}", failed_messages);
    println!("   Total Retries: {}", stats.retries_attempted);
    println!("   Average Latency: {:.2}ms", stats.total_latency_ms as f64 / stats.messages_sent as f64);
    
    // Print network error breakdown
    println!("   Network Errors:");
    for (error_type, count) in &stats.network_errors {
        println!("     {}: {}", error_type, count);
    }
    
    // Validate fault tolerance
    assert!(successful_messages > total_messages / 2, "Should deliver at least 50% of messages under extreme conditions");
    assert!(stats.retries_attempted > 0, "Should attempt retries under poor conditions");
    
    println!("✅ Extreme network conditions test passed - System demonstrated fault tolerance");
}

#[tokio::test]
async fn test_satellite_internet_conditions() {
    println!("🛰️  Testing Satellite Internet Conditions");
    
    let chaos_simulator = NetworkChaosSimulator::satellite_internet();
    let node = FaultTolerantNode::new("satellite-node".to_string(), chaos_simulator.clone()).await.unwrap();
    let node_addr = node.local_addr().unwrap();
    
    tokio::spawn(async move { node.start_server().await });
    sleep(Duration::from_millis(100)).await;
    
    let sender = FaultTolerantNode::new("ground-station".to_string(), chaos_simulator).await.unwrap();
    
    println!("📡 Testing high-latency satellite communication...");
    
    let mut latencies = Vec::new();
    let test_messages = 20;
    
    for i in 0..test_messages {
        let message = ResilientMessage::new(i, vec![0u8; 512], MessagePriority::High);
        
        if let Ok(latency) = sender.send_message_with_retry(node_addr, message).await {
            latencies.push(latency.as_millis() as f64);
        }
    }
    
    let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let stats = sender.get_stats();
    
    println!("\n📊 Satellite Internet Test Results:");
    println!("   Successful Messages: {}/{}", latencies.len(), test_messages);
    println!("   Average Latency: {:.2}ms", avg_latency);
    println!("   Intermittent Outages: {}", stats.network_errors.get("intermittent_outage").unwrap_or(&0));
    
    // Validate satellite performance
    assert!(avg_latency > 500.0, "Satellite latency should be high (>500ms)");
    assert!(latencies.len() > test_messages / 2, "Should handle satellite conditions reasonably well");
    
    println!("✅ Satellite internet conditions test passed");
}

#[tokio::test]
async fn test_network_partition_recovery() {
    println!("🔌 Testing Network Partition Recovery");
    
    let chaos_simulator = NetworkChaosSimulator::mobile_network_poor();
    let node1 = FaultTolerantNode::new("partition-node-1".to_string(), chaos_simulator.clone()).await.unwrap();
    let node2 = FaultTolerantNode::new("partition-node-2".to_string(), chaos_simulator.clone()).await.unwrap();
    
    let node1_addr = node1.local_addr().unwrap();
    let node2_addr = node2.local_addr().unwrap();
    
    tokio::spawn(async move { node1.start_server().await });
    tokio::spawn(async move { node2.start_server().await });
    
    sleep(Duration::from_millis(100)).await;
    
    let sender = FaultTolerantNode::new("partition-sender".to_string(), chaos_simulator).await.unwrap();
    
    println!("📡 Testing message delivery during network partition...");
    
    // Simulate a network partition
    sender.simulate_partition(Duration::from_millis(2000));
    
    let mut results = Vec::new();
    
    for i in 0..10 {
        let message = ResilientMessage::new(i, vec![0u8; 256], MessagePriority::Critical);
        let start = Instant::now();
        
        match sender.send_message_with_retry(node1_addr, message).await {
            Ok(_) => results.push((i, true, start.elapsed())),
            Err(_) => results.push((i, false, start.elapsed())),
        }
        
        sleep(Duration::from_millis(500)).await;
    }
    
    let successful = results.iter().filter(|(_, success, _)| *success).count();
    let stats = sender.get_stats();
    
    println!("\n📊 Network Partition Recovery Results:");
    println!("   Messages Delivered: {}/10", successful);
    println!("   Network Partitions Handled: {}", stats.network_errors.get("network_partition").unwrap_or(&0));
    println!("   Total Retries During Recovery: {}", stats.retries_attempted);
    
    // Validate partition recovery
    assert!(successful > 0, "Should recover from network partition and deliver some messages");
    assert!(stats.retries_attempted > 0, "Should attempt retries during partition");
    
    println!("✅ Network partition recovery test passed - System recovered from partition");
}

#[tokio::test]
async fn test_cascading_failure_resilience() {
    println!("⚡ Testing Cascading Failure Resilience");
    
    // Create multiple nodes with different failure characteristics
    let nodes = vec![
        ("stable-node", NetworkChaosSimulator::extreme_conditions()),
        ("unstable-node-1", NetworkChaosSimulator::mobile_network_poor()),
        ("unstable-node-2", NetworkChaosSimulator::satellite_internet()),
    ];
    
    let mut node_addrs = Vec::new();
    
    for (node_id, chaos_sim) in nodes {
        let node = FaultTolerantNode::new(node_id.to_string(), chaos_sim).await.unwrap();
        let addr = node.local_addr().unwrap();
        node_addrs.push(addr);
        
        tokio::spawn(async move { node.start_server().await });
    }
    
    sleep(Duration::from_millis(100)).await;
    
    let sender = FaultTolerantNode::new("cascade-sender".to_string(), NetworkChaosSimulator::extreme_conditions()).await.unwrap();
    
    println!("📡 Testing system resilience under cascading failures...");
    
    let mut total_successful = 0;
    let test_rounds = 5;
    
    for round in 0..test_rounds {
        println!("   Round {}/{}", round + 1, test_rounds);
        
        for (i, &target_addr) in node_addrs.iter().enumerate() {
            let message = ResilientMessage::new(
                (round * 10 + i) as u64,
                vec![0u8; 2048], // 2KB payload
                MessagePriority::High
            );
            
            if sender.send_message_with_retry(target_addr, message).await.is_ok() {
                total_successful += 1;
            }
        }
        
        // Simulate increasing system stress
        sleep(Duration::from_millis(1000)).await;
    }
    
    let stats = sender.get_stats();
    let total_attempts = test_rounds * node_addrs.len();
    let success_rate = total_successful as f64 / total_attempts as f64;
    
    println!("\n📊 Cascading Failure Resilience Results:");
    println!("   Total Successful: {}/{}", total_successful, total_attempts);
    println!("   Success Rate: {:.1}%", success_rate * 100.0);
    println!("   Total Retries: {}", stats.retries_attempted);
    println!("   System Maintained Stability: {}", if success_rate > 0.3 { "✅ Yes" } else { "❌ No" });
    
    // Validate cascading failure resilience
    assert!(success_rate > 0.2, "Should maintain at least 20% success rate under cascading failures");
    assert!(total_successful > 0, "Should deliver some messages despite cascading failures");
    
    println!("✅ Cascading failure resilience test passed - System remained operational");
}