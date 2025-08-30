//! Resource Management and Lifecycle Components
//! 
//! This module provides comprehensive resource management capabilities including:
//! - Resource lifecycle management with automatic cleanup
//! - Background job management and monitoring
//! - File upload and storage management
//! - Cache management with intelligent eviction
//! - Connection pool management
//! - Resource quota and throttling systems

pub mod lifecycle;
pub mod background_jobs;
pub mod file_storage;
pub mod cache_management;
pub mod connection_pools;
pub mod quota_throttling;

pub use lifecycle::*;
pub use background_jobs::*;
pub use file_storage::*;
pub use cache_management::*;
pub use connection_pools::*;
pub use quota_throttling::*;

// Re-export commonly used types
pub use lifecycle::{ResourceLifecycleManager, ResourceMetadata, ResourceType as LifecycleResourceType, ResourceState, RetentionPolicy};
pub use background_jobs::{BackgroundJobManager, JobHandler, JobPriority, JobState, RetryPolicy as JobRetryPolicy};
pub use file_storage::{FileStorageManager, FileStorageType, FileMetadata, StorageQuota, VirusScanner, StorageBackend};
pub use cache_management::{CacheManager, CacheConfig, EvictionPolicy, CachePriority, InvalidationPattern};
pub use connection_pools::{ConnectionPool, ConnectionPoolManager, PoolConfig, ConnectionType, ConnectionFactory, HealthChecker};
pub use quota_throttling::{ResourceQuotaManager, ResourceQuota, ResourceType as QuotaResourceType, AllocationRequest, AllocationPriority, ThrottlingConfig};