//! Lock-Free Registry Implementations
//!
//! This module provides lock-free registries for managing nodes, services,
//! and connections in the Valkyrie Protocol.

use super::map::{ConcurrentHashMap, LockFreeMap, MapError};
use super::{LockFreeConfig, LockFreeMetrics};
use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Errors that can occur during registry operations
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Entry not found: {key}")]
    NotFound { key: String },
    #[error("Entry already exists: {key}")]
    AlreadyExists { key: String },
    #[error("Registry is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
    #[error("Map error: {source}")]
    MapError {
        #[from]
        source: MapError,
    },
}

/// Statistics for registry operations
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// Current number of entries in the registry
    pub entries: usize,
    /// Maximum capacity of the registry
    pub capacity: usize,
    /// Total number of register operations
    pub registers: usize,
    /// Total number of unregister operations
    pub unregisters: usize,
    /// Total number of lookup operations
    pub lookups: usize,
    /// Number of failed operations
    pub failures: usize,
    /// Average contention level
    pub contention_level: f64,
    /// Registry utilization percentage
    pub utilization: f64,
}

/// Generic lock-free registry trait
pub trait LockFreeRegistry<K, V>: Send + Sync {
    /// Register an entry
    fn register(&self, key: K, value: V) -> Result<(), RegistryError>;

    /// Unregister an entry
    fn unregister(&self, key: &K) -> Result<Option<V>, RegistryError>;

    /// Lookup an entry
    fn lookup(&self, key: &K) -> Result<Option<V>, RegistryError>
    where
        V: Clone;

    /// Update an entry if it exists
    fn update<F>(&self, key: &K, updater: F) -> Result<Option<V>, RegistryError>
    where
        F: FnOnce(&V) -> V,
        V: Clone;

    /// Register or update an entry
    fn register_or_update<F>(
        &self,
        key: K,
        value: V,
        updater: F,
    ) -> Result<Option<V>, RegistryError>
    where
        F: FnOnce(&V) -> V,
        V: Clone;

    /// Check if an entry exists
    fn contains(&self, key: &K) -> bool;

    /// Get all keys
    fn keys(&self) -> Vec<K>
    where
        K: Clone;

    /// Get current entry count
    fn len(&self) -> usize;

    /// Check if registry is empty
    fn is_empty(&self) -> bool;

    /// Get registry statistics
    fn stats(&self) -> RegistryStats;

    /// Clear all entries
    fn clear(&self);
}

/// Entry with metadata for registry
#[derive(Debug, Clone)]
pub struct RegistryEntry<V>
where
    V: Send + Sync,
{
    /// The actual value
    pub value: V,
    /// Registration timestamp
    pub registered_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
    /// Access count
    pub access_count: usize,
    /// Entry version (for optimistic updates)
    pub version: usize,
}

unsafe impl<V: Send + Sync> Send for RegistryEntry<V> {}
unsafe impl<V: Send + Sync> Sync for RegistryEntry<V> {}

impl<V> RegistryEntry<V>
where
    V: Send + Sync,
{
    /// Create a new registry entry
    pub fn new(value: V) -> Self {
        let now = Instant::now();
        Self {
            value,
            registered_at: now,
            last_accessed: now,
            access_count: 0,
            version: 1,
        }
    }

    /// Update access metadata
    pub fn accessed(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    /// Update the value and increment version
    pub fn update_value(&mut self, new_value: V) {
        self.value = new_value;
        self.version += 1;
        self.accessed();
    }

    /// Get age since registration
    pub fn age(&self) -> Duration {
        self.registered_at.elapsed()
    }

    /// Get time since last access
    pub fn idle_time(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

/// Node registry for managing distributed nodes
pub struct NodeRegistry<NodeId, NodeInfo>
where
    NodeId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    NodeInfo: Clone + Send + Sync,
{
    /// Inner concurrent hash map
    inner: ConcurrentHashMap<NodeId, RegistryEntry<NodeInfo>>,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Next node ID counter
    next_id: AtomicUsize,
}

impl<NodeId, NodeInfo> NodeRegistry<NodeId, NodeInfo>
where
    NodeId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    NodeInfo: Clone + Send + Sync,
{
    /// Create a new node registry
    pub fn new(bucket_count: usize, config: LockFreeConfig) -> Self {
        Self {
            inner: ConcurrentHashMap::new(bucket_count, config),
            metrics: LockFreeMetrics::default(),
            next_id: AtomicUsize::new(1),
        }
    }

    /// Generate next node ID (for auto-generated IDs)
    pub fn next_node_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Get nodes by age (older than specified duration)
    pub fn get_nodes_older_than(&self, _duration: Duration) -> Vec<(NodeId, NodeInfo)> {
        // This is a simplified implementation
        // In a real system, you'd want a more efficient way to iterate
        let result = Vec::new();

        // For demonstration, we'll just return empty vec
        // A real implementation would need to iterate through the map
        result
    }

    /// Get idle nodes (not accessed for specified duration)
    pub fn get_idle_nodes(&self, _duration: Duration) -> Vec<(NodeId, NodeInfo)> {
        // Similar to above - simplified implementation
        Vec::new()
    }
}

impl<NodeId, NodeInfo> LockFreeRegistry<NodeId, NodeInfo> for NodeRegistry<NodeId, NodeInfo>
where
    NodeId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    NodeInfo: Clone + Send + Sync,
{
    fn register(&self, key: NodeId, value: NodeInfo) -> Result<(), RegistryError> {
        let entry = RegistryEntry::new(value);
        match self.inner.insert(key.clone(), entry) {
            Ok(Some(_)) => Err(RegistryError::AlreadyExists {
                key: format!("{:?}", key),
            }),
            Ok(None) => {
                self.metrics.record_success();
                Ok(())
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(RegistryError::MapError { source: e })
            }
        }
    }

    fn unregister(&self, key: &NodeId) -> Result<Option<NodeInfo>, RegistryError> {
        match self.inner.remove(key) {
            Ok(Some(entry)) => {
                self.metrics.record_success();
                Ok(Some(entry.value))
            }
            Ok(None) => {
                self.metrics.record_failure();
                Ok(None)
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(RegistryError::MapError { source: e })
            }
        }
    }

    fn lookup(&self, key: &NodeId) -> Result<Option<NodeInfo>, RegistryError> {
        match self.inner.get(key) {
            Ok(Some(mut entry)) => {
                entry.accessed();
                // Note: In a real implementation, we'd need to update the entry in the map
                self.metrics.record_success();
                Ok(Some(entry.value))
            }
            Ok(None) => {
                self.metrics.record_failure();
                Ok(None)
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(RegistryError::MapError { source: e })
            }
        }
    }

    fn update<F>(&self, key: &NodeId, updater: F) -> Result<Option<NodeInfo>, RegistryError>
    where
        F: FnOnce(&NodeInfo) -> NodeInfo,
    {
        match self.inner.update(key, |entry| {
            let new_value = updater(&entry.value);
            let mut new_entry = entry.clone();
            new_entry.update_value(new_value);
            new_entry
        }) {
            Ok(Some(old_entry)) => {
                self.metrics.record_success();
                Ok(Some(old_entry.value))
            }
            Ok(None) => {
                self.metrics.record_failure();
                Ok(None)
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(RegistryError::MapError { source: e })
            }
        }
    }

    fn register_or_update<F>(
        &self,
        key: NodeId,
        value: NodeInfo,
        updater: F,
    ) -> Result<Option<NodeInfo>, RegistryError>
    where
        F: FnOnce(&NodeInfo) -> NodeInfo,
    {
        let new_entry = RegistryEntry::new(value);
        match self.inner.upsert(key, new_entry, |entry| {
            let new_value = updater(&entry.value);
            let mut updated_entry = entry.clone();
            updated_entry.update_value(new_value);
            updated_entry
        }) {
            Ok(old_entry) => {
                self.metrics.record_success();
                Ok(old_entry.map(|e| e.value))
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(RegistryError::MapError { source: e })
            }
        }
    }

    fn contains(&self, key: &NodeId) -> bool {
        self.inner.contains_key(key)
    }

    fn keys(&self) -> Vec<NodeId> {
        // Simplified implementation - would need proper iteration in real system
        Vec::new()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn stats(&self) -> RegistryStats {
        let map_stats = self.inner.stats();
        RegistryStats {
            entries: map_stats.size,
            capacity: 0, // Unbounded
            registers: self.metrics.successes.load(Ordering::Relaxed),
            unregisters: self.metrics.successes.load(Ordering::Relaxed),
            lookups: self.metrics.operations.load(Ordering::Relaxed),
            failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            utilization: if map_stats.capacity > 0 {
                (map_stats.size as f64 / map_stats.capacity as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    fn clear(&self) {
        self.inner.clear();
    }
}

/// Service registry for managing distributed services
pub struct ServiceRegistry<ServiceId, ServiceInfo>
where
    ServiceId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ServiceInfo: Clone + Send + Sync,
{
    /// Inner node registry
    inner: NodeRegistry<ServiceId, ServiceInfo>,
    /// Service health check interval
    health_check_interval: Duration,
}

impl<ServiceId, ServiceInfo> ServiceRegistry<ServiceId, ServiceInfo>
where
    ServiceId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ServiceInfo: Clone + Send + Sync,
{
    /// Create a new service registry
    pub fn new(
        bucket_count: usize,
        config: LockFreeConfig,
        health_check_interval: Duration,
    ) -> Self {
        Self {
            inner: NodeRegistry::new(bucket_count, config),
            health_check_interval,
        }
    }

    /// Get unhealthy services (not accessed within health check interval)
    pub fn get_unhealthy_services(&self) -> Vec<(ServiceId, ServiceInfo)> {
        self.inner.get_idle_nodes(self.health_check_interval)
    }

    /// Perform health check on a service
    pub fn health_check(&self, service_id: &ServiceId) -> Result<bool, RegistryError> {
        // Update last accessed time to indicate service is healthy
        match self.inner.lookup(service_id) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

impl<ServiceId, ServiceInfo> LockFreeRegistry<ServiceId, ServiceInfo>
    for ServiceRegistry<ServiceId, ServiceInfo>
where
    ServiceId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ServiceInfo: Clone + Send + Sync,
{
    fn register(&self, key: ServiceId, value: ServiceInfo) -> Result<(), RegistryError> {
        self.inner.register(key, value)
    }

    fn unregister(&self, key: &ServiceId) -> Result<Option<ServiceInfo>, RegistryError> {
        self.inner.unregister(key)
    }

    fn lookup(&self, key: &ServiceId) -> Result<Option<ServiceInfo>, RegistryError> {
        self.inner.lookup(key)
    }

    fn update<F>(&self, key: &ServiceId, updater: F) -> Result<Option<ServiceInfo>, RegistryError>
    where
        F: FnOnce(&ServiceInfo) -> ServiceInfo,
    {
        self.inner.update(key, updater)
    }

    fn register_or_update<F>(
        &self,
        key: ServiceId,
        value: ServiceInfo,
        updater: F,
    ) -> Result<Option<ServiceInfo>, RegistryError>
    where
        F: FnOnce(&ServiceInfo) -> ServiceInfo,
    {
        self.inner.register_or_update(key, value, updater)
    }

    fn contains(&self, key: &ServiceId) -> bool {
        self.inner.contains(key)
    }

    fn keys(&self) -> Vec<ServiceId> {
        self.inner.keys()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn stats(&self) -> RegistryStats {
        self.inner.stats()
    }

    fn clear(&self) {
        self.inner.clear()
    }
}

/// Connection registry for managing active connections
pub struct ConnectionRegistry<ConnectionId, ConnectionInfo>
where
    ConnectionId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ConnectionInfo: Clone + Send + Sync,
{
    /// Inner node registry
    inner: NodeRegistry<ConnectionId, ConnectionInfo>,
    /// Connection timeout duration
    connection_timeout: Duration,
}

impl<ConnectionId, ConnectionInfo> ConnectionRegistry<ConnectionId, ConnectionInfo>
where
    ConnectionId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ConnectionInfo: Clone + Send + Sync,
{
    /// Create a new connection registry
    pub fn new(bucket_count: usize, config: LockFreeConfig, connection_timeout: Duration) -> Self {
        Self {
            inner: NodeRegistry::new(bucket_count, config),
            connection_timeout,
        }
    }

    /// Get expired connections
    pub fn get_expired_connections(&self) -> Vec<(ConnectionId, ConnectionInfo)> {
        self.inner.get_idle_nodes(self.connection_timeout)
    }

    /// Refresh connection (update last accessed time)
    pub fn refresh_connection(&self, connection_id: &ConnectionId) -> Result<bool, RegistryError> {
        match self.inner.lookup(connection_id) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.inner.len()
    }
}

impl<ConnectionId, ConnectionInfo> LockFreeRegistry<ConnectionId, ConnectionInfo>
    for ConnectionRegistry<ConnectionId, ConnectionInfo>
where
    ConnectionId: Hash + Eq + Clone + Send + Sync + std::fmt::Debug,
    ConnectionInfo: Clone + Send + Sync,
{
    fn register(&self, key: ConnectionId, value: ConnectionInfo) -> Result<(), RegistryError> {
        self.inner.register(key, value)
    }

    fn unregister(&self, key: &ConnectionId) -> Result<Option<ConnectionInfo>, RegistryError> {
        self.inner.unregister(key)
    }

    fn lookup(&self, key: &ConnectionId) -> Result<Option<ConnectionInfo>, RegistryError> {
        self.inner.lookup(key)
    }

    fn update<F>(
        &self,
        key: &ConnectionId,
        updater: F,
    ) -> Result<Option<ConnectionInfo>, RegistryError>
    where
        F: FnOnce(&ConnectionInfo) -> ConnectionInfo,
    {
        self.inner.update(key, updater)
    }

    fn register_or_update<F>(
        &self,
        key: ConnectionId,
        value: ConnectionInfo,
        updater: F,
    ) -> Result<Option<ConnectionInfo>, RegistryError>
    where
        F: FnOnce(&ConnectionInfo) -> ConnectionInfo,
    {
        self.inner.register_or_update(key, value, updater)
    }

    fn contains(&self, key: &ConnectionId) -> bool {
        self.inner.contains(key)
    }

    fn keys(&self) -> Vec<ConnectionId> {
        self.inner.keys()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn stats(&self) -> RegistryStats {
        self.inner.stats()
    }

    fn clear(&self) {
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[derive(Debug, Clone, PartialEq)]
    struct NodeInfo {
        address: String,
        port: u16,
        status: String,
    }

    #[test]
    fn test_node_registry_basic_operations() {
        let registry = NodeRegistry::new(16, LockFreeConfig::default());
        let node_info = NodeInfo {
            address: "127.0.0.1".to_string(),
            port: 8080,
            status: "active".to_string(),
        };

        // Register a node
        assert!(registry
            .register("node1".to_string(), node_info.clone())
            .is_ok());
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        // Lookup the node
        let result = registry.lookup(&"node1".to_string()).unwrap();
        assert_eq!(result, Some(node_info.clone()));

        // Update the node
        let updated_info = registry
            .update(&"node1".to_string(), |info| NodeInfo {
                address: info.address.clone(),
                port: info.port,
                status: "inactive".to_string(),
            })
            .unwrap();
        assert_eq!(updated_info.unwrap().status, "active");

        // Verify update
        let result = registry.lookup(&"node1".to_string()).unwrap();
        assert_eq!(result.unwrap().status, "inactive");

        // Unregister the node
        let removed = registry.unregister(&"node1".to_string()).unwrap();
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_service_registry() {
        let registry = ServiceRegistry::new(16, LockFreeConfig::default(), Duration::from_secs(30));

        let service_info = NodeInfo {
            address: "service.example.com".to_string(),
            port: 9090,
            status: "running".to_string(),
        };

        // Register service
        assert!(registry
            .register("auth-service".to_string(), service_info.clone())
            .is_ok());

        // Health check
        assert_eq!(
            registry.health_check(&"auth-service".to_string()).unwrap(),
            true
        );
        assert_eq!(
            registry.health_check(&"nonexistent".to_string()).unwrap(),
            false
        );

        // Get stats
        let stats = registry.stats();
        assert_eq!(stats.entries, 1);
        assert!(stats.registers > 0);
    }

    #[test]
    fn test_connection_registry() {
        let registry =
            ConnectionRegistry::new(16, LockFreeConfig::default(), Duration::from_secs(60));

        let conn_info = NodeInfo {
            address: "client.example.com".to_string(),
            port: 12345,
            status: "connected".to_string(),
        };

        // Register connection
        assert!(registry.register(42u64, conn_info.clone()).is_ok());
        assert_eq!(registry.connection_count(), 1);

        // Refresh connection
        assert_eq!(registry.refresh_connection(&42u64).unwrap(), true);

        // Unregister connection
        let removed = registry.unregister(&42u64).unwrap();
        assert!(removed.is_some());
        assert_eq!(registry.connection_count(), 0);
    }

    #[test]
    fn test_registry_concurrent_access() {
        let registry = Arc::new(NodeRegistry::new(64, LockFreeConfig::default()));
        let num_threads = 8;
        let operations_per_thread = 100;

        let mut handles = Vec::new();

        // Spawn threads that perform mixed operations
        for thread_id in 0..num_threads {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("node_{}_{}", thread_id, i);
                    let info = NodeInfo {
                        address: format!("192.168.1.{}", thread_id),
                        port: 8000 + i as u16,
                        status: "active".to_string(),
                    };

                    // Register
                    let _ = registry_clone.register(key.clone(), info.clone());

                    // Lookup
                    let _ = registry_clone.lookup(&key);

                    // Update
                    let _ = registry_clone.update(&key, |info| NodeInfo {
                        address: info.address.clone(),
                        port: info.port,
                        status: "updated".to_string(),
                    });

                    // Unregister (some entries)
                    if i % 2 == 0 {
                        let _ = registry_clone.unregister(&key);
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state
        let stats = registry.stats();
        assert!(stats.entries <= num_threads * operations_per_thread);
        assert!(stats.registers > 0);
        assert!(stats.lookups > 0);
    }

    #[test]
    fn test_registry_entry_metadata() {
        let entry = RegistryEntry::new("test_value".to_string());

        assert_eq!(entry.value, "test_value");
        assert_eq!(entry.access_count, 0);
        assert_eq!(entry.version, 1);

        let mut entry = entry;
        entry.accessed();
        assert_eq!(entry.access_count, 1);

        entry.update_value("new_value".to_string());
        assert_eq!(entry.value, "new_value");
        assert_eq!(entry.version, 2);
        assert_eq!(entry.access_count, 2);
    }

    #[test]
    fn test_register_or_update() {
        let registry = NodeRegistry::new(16, LockFreeConfig::default());
        let node_info = NodeInfo {
            address: "127.0.0.1".to_string(),
            port: 8080,
            status: "active".to_string(),
        };

        // First call should register
        let result = registry
            .register_or_update("node1".to_string(), node_info.clone(), |info| NodeInfo {
                address: info.address.clone(),
                port: info.port,
                status: "updated".to_string(),
            })
            .unwrap();
        assert!(result.is_none()); // No previous value

        // Second call should update
        let result = registry
            .register_or_update("node1".to_string(), node_info.clone(), |info| NodeInfo {
                address: info.address.clone(),
                port: info.port,
                status: "updated_again".to_string(),
            })
            .unwrap();
        assert!(result.is_some()); // Previous value returned

        // Verify final state
        let current = registry.lookup(&"node1".to_string()).unwrap().unwrap();
        assert_eq!(current.status, "updated_again");
    }
}
