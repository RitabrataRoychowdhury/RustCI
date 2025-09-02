use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;

/// Health status of a storage backend
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Statistics for a storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub average_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub connections_active: u32,
    pub connections_idle: u32,
    pub memory_usage_bytes: u64,
    pub last_updated: SystemTime,
    pub backend_specific: HashMap<String, serde_json::Value>,
}

impl Default for StoreStats {
    fn default() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            average_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            connections_active: 0,
            connections_idle: 0,
            memory_usage_bytes: 0,
            last_updated: SystemTime::now(),
            backend_specific: HashMap::new(),
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    pub capacity: u32,
    pub tokens: AtomicU32,
    pub refill_rate: u32,
    pub last_refill: AtomicU64,
}

impl TokenBucket {
    pub fn new(capacity: u32, refill_rate: u32) -> Self {
        Self {
            capacity,
            tokens: AtomicU32::new(capacity),
            refill_rate,
            last_refill: AtomicU64::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            ),
        }
    }
    
    pub fn try_consume(&self, amount: u32) -> bool {
        self.refill();
        
        let current_tokens = self.tokens.load(std::sync::atomic::Ordering::Acquire);
        if current_tokens >= amount {
            let new_tokens = current_tokens - amount;
            self.tokens
                .compare_exchange(
                    current_tokens,
                    new_tokens,
                    std::sync::atomic::Ordering::Release,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
        } else {
            false
        }
    }
    
    fn refill(&self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
            
        let last_refill = self.last_refill.load(std::sync::atomic::Ordering::Acquire);
        let elapsed_ms = now.saturating_sub(last_refill);
        
        if elapsed_ms > 0 {
            let tokens_to_add = (elapsed_ms * self.refill_rate as u64) / 1000;
            if tokens_to_add > 0 {
                let current_tokens = self.tokens.load(std::sync::atomic::Ordering::Acquire);
                let new_tokens = (current_tokens + tokens_to_add as u32).min(self.capacity);
                
                if self.tokens
                    .compare_exchange(
                        current_tokens,
                        new_tokens,
                        std::sync::atomic::Ordering::Release,
                        std::sync::atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.last_refill.store(now, std::sync::atomic::Ordering::Release);
                }
            }
        }
    }
}

/// Node information for consistent hashing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: Uuid,
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub status: HealthStatus,
    pub last_seen: SystemTime,
}

/// Consistent hashing ring for key distribution
#[derive(Debug, Clone)]
pub struct ConsistentHashRing {
    pub nodes: Vec<NodeInfo>,
    pub virtual_nodes: HashMap<u64, Uuid>,
    pub replicas: u32,
}

impl ConsistentHashRing {
    pub fn new(replicas: u32) -> Self {
        Self {
            nodes: Vec::new(),
            virtual_nodes: HashMap::new(),
            replicas,
        }
    }
    
    pub fn add_node(&mut self, node: NodeInfo) {
        // Add virtual nodes for better distribution
        for i in 0..self.replicas {
            let virtual_key = self.hash(&format!("{}:{}", node.id, i));
            self.virtual_nodes.insert(virtual_key, node.id);
        }
        self.nodes.push(node);
    }
    
    pub fn remove_node(&mut self, node_id: Uuid) {
        // Remove virtual nodes
        self.virtual_nodes.retain(|_, id| *id != node_id);
        self.nodes.retain(|node| node.id != node_id);
    }
    
    pub fn get_node(&self, key: &str) -> Option<&NodeInfo> {
        if self.virtual_nodes.is_empty() {
            return None;
        }
        
        let key_hash = self.hash(key);
        
        // Find the first virtual node with hash >= key_hash
        let mut virtual_keys: Vec<_> = self.virtual_nodes.keys().collect();
        virtual_keys.sort();
        
        let virtual_node_hash = virtual_keys
            .iter()
            .find(|&&hash| *hash >= key_hash)
            .or_else(|| virtual_keys.first())
            .copied()?;
            
        let node_id = self.virtual_nodes.get(virtual_node_hash)?;
        self.nodes.iter().find(|node| node.id == *node_id)
    }
    
    fn hash(&self, input: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        hasher.finish()
    }
}

/// Performance metrics for operations
#[derive(Debug, Clone)]
pub struct OperationMetrics {
    pub operation_type: String,
    pub start_time: Instant,
    pub duration: Option<Duration>,
    pub success: bool,
    pub error: Option<String>,
    pub bytes_transferred: u64,
}

impl OperationMetrics {
    pub fn new(operation_type: String) -> Self {
        Self {
            operation_type,
            start_time: Instant::now(),
            duration: None,
            success: false,
            error: None,
            bytes_transferred: 0,
        }
    }
    
    pub fn complete_success(mut self, bytes_transferred: u64) -> Self {
        self.duration = Some(self.start_time.elapsed());
        self.success = true;
        self.bytes_transferred = bytes_transferred;
        self
    }
    
    pub fn complete_error(mut self, error: String) -> Self {
        self.duration = Some(self.start_time.elapsed());
        self.success = false;
        self.error = Some(error);
        self
    }
}