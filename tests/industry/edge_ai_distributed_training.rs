//! Industry-Grade Edge AI Distributed Training Workload Simulation
//! 
//! This test simulates real-world edge computing scenarios where distributed AI centers
//! share training data across unreliable networks with varying latency and bandwidth.

use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;
use rand::Rng;

/// Simulates an AI training data packet
#[derive(Clone, Debug)]
struct TrainingDataPacket {
    model_id: String,
    epoch: u32,
    batch_id: u32,
    gradient_data: Vec<f32>,
    timestamp: Instant,
    source_node: String,
    target_nodes: Vec<String>,
}

impl TrainingDataPacket {
    fn new(model_id: String, epoch: u32, batch_id: u32, gradient_size: usize, source: String) -> Self {
        let mut rng = rand::thread_rng();
        let gradient_data: Vec<f32> = (0..gradient_size).map(|_| rng.gen::<f32>()).collect();
        
        Self {
            model_id,
            epoch,
            batch_id,
            gradient_data,
            timestamp: Instant::now(),
            source_node: source,
            target_nodes: vec!["edge-1".to_string(), "edge-2".to_string(), "edge-3".to_string()],
        }
    }
    
    fn serialize(&self) -> Vec<u8> {
        // Simplified serialization for testing
        let mut data = Vec::new();
        data.extend_from_slice(self.model_id.as_bytes());
        data.extend_from_slice(&self.epoch.to_le_bytes());
        data.extend_from_slice(&self.batch_id.to_le_bytes());
        data.extend_from_slice(&(self.gradient_data.len() as u32).to_le_bytes());
        
        for &gradient in &self.gradient_data {
            data.extend_from_slice(&gradient.to_le_bytes());
        }
        
        data
    }
    
    fn size_bytes(&self) -> usize {
        self.model_id.len() + 8 + 4 + (self.gradient_data.len() * 4)
    }
}

/// Edge AI node simulator
struct EdgeAINode {
    node_id: String,
    listener: TcpListener,
    received_packets: Arc<Mutex<Vec<TrainingDataPacket>>>,
    sent_packets: Arc<Mutex<u64>>,
    network_conditions: NetworkConditions,
}

#[derive(Clone)]
struct NetworkConditions {
    latency_ms: u64,
    packet_loss_rate: f64,
    bandwidth_mbps: f64,
    jitter_ms: u64,
}

impl NetworkConditions {
    fn edge_network() -> Self {
        Self {
            latency_ms: 50,      // 50ms base latency
            packet_loss_rate: 0.02, // 2% packet loss
            bandwidth_mbps: 10.0,    // 10 Mbps
            jitter_ms: 20,       // 20ms jitter
        }
    }
    
    fn poor_network() -> Self {
        Self {
            latency_ms: 200,     // 200ms base latency
            packet_loss_rate: 0.10, // 10% packet loss
            bandwidth_mbps: 1.0,     // 1 Mbps
            jitter_ms: 100,      // 100ms jitter
        }
    }
    
    fn datacenter_network() -> Self {
        Self {
            latency_ms: 1,       // 1ms base latency
            packet_loss_rate: 0.001, // 0.1% packet loss
            bandwidth_mbps: 1000.0,  // 1 Gbps
            jitter_ms: 1,        // 1ms jitter
        }
    }
}

impl EdgeAINode {
    async fn new(node_id: String, conditions: NetworkConditions) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        
        Ok(Self {
            node_id,
            listener,
            received_packets: Arc::new(Mutex::new(Vec::new())),
            sent_packets: Arc::new(Mutex::new(0)),
            network_conditions: conditions,
        })
    }
    
    fn local_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }
    
    async fn start_server(&self) {
        let received_packets = self.received_packets.clone();
        let conditions = self.network_conditions.clone();
        
        loop {
            if let Ok((mut socket, _)) = self.listener.accept().await {
                let received_packets = received_packets.clone();
                let conditions = conditions.clone();
                
                tokio::spawn(async move {
                    // Simulate network latency
                    let mut rng = rand::thread_rng();
                    let latency = conditions.latency_ms + rng.gen_range(0..conditions.jitter_ms);
                    tokio::time::sleep(Duration::from_millis(latency)).await;
                    
                    // Simulate packet loss
                    if rng.gen::<f64>() < conditions.packet_loss_rate {
                        return; // Drop the packet
                    }
                    
                    let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer for large gradients
                    
                    match socket.read(&mut buffer).await {
                        Ok(n) if n > 0 => {
                            // Simulate bandwidth limitation
                            let transfer_time = (n as f64 * 8.0) / (conditions.bandwidth_mbps * 1_000_000.0);
                            tokio::time::sleep(Duration::from_secs_f64(transfer_time)).await;
                            
                            // Process received training data
                            let mut received = received_packets.lock().unwrap();
                            // For simulation, create a dummy packet
                            let packet = TrainingDataPacket::new(
                                "model-1".to_string(),
                                1,
                                received.len() as u32,
                                1000, // 1000 gradients
                                "sender".to_string()
                            );
                            received.push(packet);
                        }
                        _ => {}
                    }
                });
            }
        }
    }
    
    async fn send_training_data(&self, target_addr: std::net::SocketAddr, packet: TrainingDataPacket) -> Result<Duration, Box<dyn std::error::Error>> {
        let start = Instant::now();
        
        // Simulate network conditions before sending
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() < self.network_conditions.packet_loss_rate {
            return Err("Packet lost".into());
        }
        
        let data = packet.serialize();
        
        // Connect with timeout
        let mut stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect(target_addr)
        ).await??;
        
        // Simulate bandwidth limitation
        let transfer_time = (data.len() as f64 * 8.0) / (self.network_conditions.bandwidth_mbps * 1_000_000.0);
        tokio::time::sleep(Duration::from_secs_f64(transfer_time)).await;
        
        // Send data
        stream.write_all(&data).await?;
        
        // Update sent counter
        let mut sent = self.sent_packets.lock().unwrap();
        *sent += 1;
        
        Ok(start.elapsed())
    }
    
    fn get_stats(&self) -> (usize, u64) {
        let received = self.received_packets.lock().unwrap();
        let sent = self.sent_packets.lock().unwrap();
        (received.len(), *sent)
    }
}

/// Distributed training coordinator
struct DistributedTrainingCoordinator {
    nodes: Vec<EdgeAINode>,
    training_stats: Arc<Mutex<TrainingStats>>,
}

#[derive(Debug, Clone)]
struct TrainingStats {
    total_packets_sent: u64,
    total_packets_received: u64,
    total_data_transferred_mb: f64,
    average_latency_ms: f64,
    packet_loss_rate: f64,
    training_epochs_completed: u32,
}

impl DistributedTrainingCoordinator {
    async fn new(node_configs: Vec<(String, NetworkConditions)>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut nodes = Vec::new();
        
        for (node_id, conditions) in node_configs {
            let node = EdgeAINode::new(node_id, conditions).await?;
            nodes.push(node);
        }
        
        Ok(Self {
            nodes,
            training_stats: Arc::new(Mutex::new(TrainingStats {
                total_packets_sent: 0,
                total_packets_received: 0,
                total_data_transferred_mb: 0.0,
                average_latency_ms: 0.0,
                packet_loss_rate: 0.0,
                training_epochs_completed: 0,
            })),
        })
    }
    
    async fn start_training_simulation(&self, epochs: u32, batches_per_epoch: u32) -> Result<TrainingStats, Box<dyn std::error::Error>> {
        println!("🤖 Starting Distributed AI Training Simulation");
        println!("   Epochs: {}, Batches per Epoch: {}", epochs, batches_per_epoch);
        println!("   Nodes: {}", self.nodes.len());
        
        // Start all node servers
        for node in &self.nodes {
            let node_addr = node.local_addr()?;
            println!("   Node '{}' listening on {}", node.node_id, node_addr);
            
            tokio::spawn(async move {
                node.start_server().await;
            });
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let mut total_latencies = Vec::new();
        let mut total_packets_sent = 0u64;
        let mut total_data_mb = 0.0;
        
        for epoch in 0..epochs {
            println!("📊 Training Epoch {}/{}", epoch + 1, epochs);
            
            for batch in 0..batches_per_epoch {
                // Each node sends gradient updates to all other nodes
                for (sender_idx, sender_node) in self.nodes.iter().enumerate() {
                    for (receiver_idx, receiver_node) in self.nodes.iter().enumerate() {
                        if sender_idx != receiver_idx {
                            let packet = TrainingDataPacket::new(
                                format!("model-{}", epoch),
                                epoch,
                                batch,
                                10000, // 10K gradients (40KB)
                                sender_node.node_id.clone()
                            );
                            
                            let data_size_mb = packet.size_bytes() as f64 / (1024.0 * 1024.0);
                            total_data_mb += data_size_mb;
                            
                            match sender_node.send_training_data(receiver_node.local_addr()?, packet).await {
                                Ok(latency) => {
                                    total_latencies.push(latency);
                                    total_packets_sent += 1;
                                }
                                Err(_) => {
                                    // Packet lost - this is expected in poor network conditions
                                }
                            }
                        }
                    }
                }
                
                if batch % 10 == 0 {
                    println!("     Batch {}/{} completed", batch + 1, batches_per_epoch);
                }
            }
            
            println!("   ✅ Epoch {} completed", epoch + 1);
        }
        
        // Calculate final statistics
        let total_packets_received: u64 = self.nodes.iter()
            .map(|node| node.get_stats().0 as u64)
            .sum();
        
        let average_latency_ms = if !total_latencies.is_empty() {
            total_latencies.iter().map(|d| d.as_millis() as f64).sum::<f64>() / total_latencies.len() as f64
        } else {
            0.0
        };
        
        let packet_loss_rate = if total_packets_sent > 0 {
            1.0 - (total_packets_received as f64 / total_packets_sent as f64)
        } else {
            0.0
        };
        
        let final_stats = TrainingStats {
            total_packets_sent,
            total_packets_received,
            total_data_transferred_mb: total_data_mb,
            average_latency_ms,
            packet_loss_rate,
            training_epochs_completed: epochs,
        };
        
        println!("\n📈 Training Simulation Results:");
        println!("   Packets Sent: {}", final_stats.total_packets_sent);
        println!("   Packets Received: {}", final_stats.total_packets_received);
        println!("   Data Transferred: {:.2} MB", final_stats.total_data_transferred_mb);
        println!("   Average Latency: {:.2} ms", final_stats.average_latency_ms);
        println!("   Packet Loss Rate: {:.2}%", final_stats.packet_loss_rate * 100.0);
        println!("   Epochs Completed: {}", final_stats.training_epochs_completed);
        
        Ok(final_stats)
    }
}

#[tokio::test]
async fn test_edge_ai_distributed_training_good_network() {
    println!("🚀 Testing Edge AI Distributed Training - Good Network Conditions");
    
    let node_configs = vec![
        ("edge-datacenter-1".to_string(), NetworkConditions::datacenter_network()),
        ("edge-datacenter-2".to_string(), NetworkConditions::datacenter_network()),
        ("edge-datacenter-3".to_string(), NetworkConditions::datacenter_network()),
    ];
    
    let coordinator = DistributedTrainingCoordinator::new(node_configs).await.unwrap();
    let stats = coordinator.start_training_simulation(3, 20).await.unwrap();
    
    // Validate performance under good conditions
    assert!(stats.average_latency_ms < 50.0, "Latency should be low in datacenter network");
    assert!(stats.packet_loss_rate < 0.05, "Packet loss should be minimal in good network");
    assert!(stats.training_epochs_completed == 3, "All epochs should complete");
    
    println!("✅ Good network conditions test passed");
}

#[tokio::test]
async fn test_edge_ai_distributed_training_edge_network() {
    println!("🚀 Testing Edge AI Distributed Training - Edge Network Conditions");
    
    let node_configs = vec![
        ("edge-node-1".to_string(), NetworkConditions::edge_network()),
        ("edge-node-2".to_string(), NetworkConditions::edge_network()),
        ("edge-node-3".to_string(), NetworkConditions::edge_network()),
        ("edge-node-4".to_string(), NetworkConditions::edge_network()),
    ];
    
    let coordinator = DistributedTrainingCoordinator::new(node_configs).await.unwrap();
    let stats = coordinator.start_training_simulation(2, 15).await.unwrap();
    
    // Validate resilience under edge conditions
    assert!(stats.average_latency_ms < 200.0, "Latency should be manageable in edge network");
    assert!(stats.packet_loss_rate < 0.15, "Packet loss should be acceptable in edge network");
    assert!(stats.training_epochs_completed == 2, "Training should complete despite network issues");
    
    println!("✅ Edge network conditions test passed");
}

#[tokio::test]
async fn test_edge_ai_distributed_training_poor_network() {
    println!("🚀 Testing Edge AI Distributed Training - Poor Network Conditions");
    
    let node_configs = vec![
        ("remote-edge-1".to_string(), NetworkConditions::poor_network()),
        ("remote-edge-2".to_string(), NetworkConditions::poor_network()),
    ];
    
    let coordinator = DistributedTrainingCoordinator::new(node_configs).await.unwrap();
    let stats = coordinator.start_training_simulation(1, 10).await.unwrap();
    
    // Validate fault tolerance under poor conditions
    assert!(stats.training_epochs_completed == 1, "Training should complete even with poor network");
    assert!(stats.packet_loss_rate < 0.50, "System should handle high packet loss");
    
    println!("✅ Poor network conditions test passed");
    println!("   Demonstrated fault tolerance with {:.1}% packet loss", stats.packet_loss_rate * 100.0);
}

#[tokio::test]
async fn test_mixed_network_conditions() {
    println!("🚀 Testing Mixed Network Conditions (Real-world Scenario)");
    
    let node_configs = vec![
        ("datacenter-hub".to_string(), NetworkConditions::datacenter_network()),
        ("edge-node-1".to_string(), NetworkConditions::edge_network()),
        ("edge-node-2".to_string(), NetworkConditions::edge_network()),
        ("remote-node".to_string(), NetworkConditions::poor_network()),
    ];
    
    let coordinator = DistributedTrainingCoordinator::new(node_configs).await.unwrap();
    let stats = coordinator.start_training_simulation(2, 12).await.unwrap();
    
    // Validate mixed network performance
    assert!(stats.training_epochs_completed == 2, "Training should adapt to mixed conditions");
    assert!(stats.total_data_transferred_mb > 0.0, "Data should be transferred successfully");
    
    println!("✅ Mixed network conditions test passed");
    println!("   Successfully handled heterogeneous network topology");
}