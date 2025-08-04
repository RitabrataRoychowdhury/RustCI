//! State Snapshot System for High Availability
//!
//! This module implements state snapshotting capabilities for the control plane,
//! allowing fast recovery by persisting critical system state to disk or S3.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::patterns::events::{DomainEvent, EventBus};
use crate::domain::entities::{JobId, NodeId, RunnerId};
use crate::error::{AppError, Result};

/// Snapshot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Snapshot storage type
    pub storage_type: SnapshotStorageType,
    /// Snapshot interval in seconds
    pub snapshot_interval: u64,
    /// Maximum number of snapshots to retain
    pub max_snapshots: u32,
    /// Compression enabled
    pub compression_enabled: bool,
    /// Encryption enabled
    pub encryption_enabled: bool,
    /// Local storage path (for disk storage)
    pub local_path: PathBuf,
    /// S3 configuration (for S3 storage)
    pub s3_config: Option<S3Config>,
}

/// Snapshot storage types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotStorageType {
    /// Store snapshots on local disk
    Disk,
    /// Store snapshots in S3
    S3,
    /// Store snapshots in both locations
    Both,
}

/// S3 configuration for snapshot storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// S3 region
    pub region: String,
    /// S3 access key ID
    pub access_key_id: String,
    /// S3 secret access key
    pub secret_access_key: String,
    /// S3 key prefix
    pub key_prefix: String,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            storage_type: SnapshotStorageType::Disk,
            snapshot_interval: 300, // 5 minutes
            max_snapshots: 10,
            compression_enabled: true,
            encryption_enabled: false,
            local_path: PathBuf::from("./snapshots"),
            s3_config: None,
        }
    }
}

/// Control plane state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneSnapshot {
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,
    /// Control plane state
    pub control_plane_state: ControlPlaneStateData,
    /// Cluster state
    pub cluster_state: ClusterStateData,
    /// Job state
    pub job_state: JobStateData,
    /// Runner state
    pub runner_state: RunnerStateData,
    /// Node health state
    pub node_health_state: NodeHealthStateData,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot ID
    pub id: Uuid,
    /// Control plane ID
    pub control_plane_id: Uuid,
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Snapshot version
    pub version: String,
    /// Snapshot size in bytes
    pub size: u64,
    /// Checksum for integrity verification
    pub checksum: String,
    /// Compression used
    pub compressed: bool,
    /// Encryption used
    pub encrypted: bool,
}

/// Control plane state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneStateData {
    /// Current control plane state
    pub state: String, // Serialized ControlPlaneState
    /// Current term (for leader election)
    pub current_term: u64,
    /// Current leader
    pub current_leader: Option<NodeId>,
    /// Configuration
    pub config: serde_json::Value,
}

/// Cluster state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStateData {
    /// Cluster ID
    pub cluster_id: Uuid,
    /// Cluster status
    pub status: String,
    /// Active nodes
    pub active_nodes: Vec<NodeId>,
    /// Failed nodes
    pub failed_nodes: Vec<NodeId>,
    /// Cluster metrics
    pub metrics: serde_json::Value,
}

/// Job state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStateData {
    /// Running jobs
    pub running_jobs: HashMap<JobId, JobInfo>,
    /// Queued jobs
    pub queued_jobs: Vec<JobId>,
    /// Job distribution mapping
    pub job_distribution: HashMap<JobId, NodeId>,
    /// Job retry counts
    pub job_retry_counts: HashMap<JobId, u32>,
}

/// Job information for snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    /// Job ID
    pub id: JobId,
    /// Assigned node
    pub node_id: NodeId,
    /// Assigned runner
    pub runner_id: RunnerId,
    /// Job status
    pub status: String,
    /// Started timestamp
    pub started_at: DateTime<Utc>,
    /// Job configuration
    pub config: serde_json::Value,
}

/// Runner state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStateData {
    /// Active runners
    pub active_runners: HashMap<RunnerId, RunnerInfo>,
    /// Runner assignments
    pub runner_assignments: HashMap<RunnerId, NodeId>,
    /// Runner health status
    pub runner_health: HashMap<RunnerId, String>,
}

/// Runner information for snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    /// Runner ID
    pub id: RunnerId,
    /// Runner type
    pub runner_type: String,
    /// Node ID
    pub node_id: NodeId,
    /// Runner status
    pub status: String,
    /// Resource usage
    pub resource_usage: serde_json::Value,
    /// Configuration
    pub config: serde_json::Value,
}

/// Node health state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealthStateData {
    /// Node health information
    pub node_health: HashMap<NodeId, NodeHealthInfo>,
    /// Failed nodes with failure info
    pub failed_nodes: HashMap<NodeId, FailureInfo>,
}

/// Node health information for snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealthInfo {
    /// Node ID
    pub id: NodeId,
    /// Reachable status
    pub reachable: bool,
    /// Last health check
    pub last_health_check: DateTime<Utc>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Resource metrics
    pub metrics: serde_json::Value,
}

/// Failure information for snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureInfo {
    /// Failure type
    pub failure_type: String,
    /// Failure timestamp
    pub failed_at: DateTime<Utc>,
    /// Failover attempts
    pub failover_attempts: u32,
    /// Target node for failover
    pub target_node: Option<NodeId>,
}

/// Snapshot events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotEvent {
    /// Snapshot created
    SnapshotCreated {
        snapshot_id: Uuid,
        control_plane_id: Uuid,
        size: u64,
        timestamp: DateTime<Utc>,
    },
    /// Snapshot restored
    SnapshotRestored {
        snapshot_id: Uuid,
        control_plane_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Snapshot deleted
    SnapshotDeleted {
        snapshot_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Snapshot failed
    SnapshotFailed {
        control_plane_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent for SnapshotEvent {
    fn event_type(&self) -> &'static str {
        match self {
            SnapshotEvent::SnapshotCreated { .. } => "snapshot.created",
            SnapshotEvent::SnapshotRestored { .. } => "snapshot.restored",
            SnapshotEvent::SnapshotDeleted { .. } => "snapshot.deleted",
            SnapshotEvent::SnapshotFailed { .. } => "snapshot.failed",
        }
    }

    fn aggregate_id(&self) -> Uuid {
        match self {
            SnapshotEvent::SnapshotCreated { control_plane_id, .. }
            | SnapshotEvent::SnapshotRestored { control_plane_id, .. }
            | SnapshotEvent::SnapshotFailed { control_plane_id, .. } => *control_plane_id,
            SnapshotEvent::SnapshotDeleted { snapshot_id, .. } => *snapshot_id,
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            SnapshotEvent::SnapshotCreated { timestamp, .. }
            | SnapshotEvent::SnapshotRestored { timestamp, .. }
            | SnapshotEvent::SnapshotDeleted { timestamp, .. }
            | SnapshotEvent::SnapshotFailed { timestamp, .. } => *timestamp,
        }
    }

    fn correlation_id(&self) -> Uuid {
        self.aggregate_id()
    }
}

/// State Snapshot Manager
pub struct StateSnapshotManager {
    /// Configuration
    config: SnapshotConfig,
    /// Control plane ID
    control_plane_id: Uuid,
    /// Event bus for publishing events
    event_bus: Arc<EventBus>,
    /// Event channel for internal events
    event_sender: mpsc::UnboundedSender<SnapshotEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<SnapshotEvent>>>>,
    /// Snapshot timer handle
    snapshot_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Available snapshots
    snapshots: Arc<RwLock<HashMap<Uuid, SnapshotMetadata>>>,
}

impl StateSnapshotManager {
    /// Create a new state snapshot manager
    pub fn new(
        config: SnapshotConfig,
        control_plane_id: Uuid,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            config,
            control_plane_id,
            event_bus,
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            snapshot_timer: Arc::new(RwLock::new(None)),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the snapshot manager
    pub async fn start(&self) -> Result<()> {
        info!(
            control_plane_id = %self.control_plane_id,
            "Starting state snapshot manager"
        );

        // Create storage directory if needed
        if matches!(self.config.storage_type, SnapshotStorageType::Disk | SnapshotStorageType::Both) {
            fs::create_dir_all(&self.config.local_path).await.map_err(|e| {
                AppError::ValidationError(format!("Failed to create snapshot directory: {}", e))
            })?;
        }

        // Load existing snapshots
        self.load_existing_snapshots().await?;

        // Start event processing
        self.start_event_processing().await;

        // Start periodic snapshots
        self.start_periodic_snapshots().await;

        info!(
            control_plane_id = %self.control_plane_id,
            "State snapshot manager started"
        );

        Ok(())
    }

    /// Stop the snapshot manager
    pub async fn stop(&self) -> Result<()> {
        info!(
            control_plane_id = %self.control_plane_id,
            "Stopping state snapshot manager"
        );

        // Stop periodic snapshots
        self.stop_periodic_snapshots().await;

        info!(
            control_plane_id = %self.control_plane_id,
            "State snapshot manager stopped"
        );

        Ok(())
    }

    /// Create a snapshot of the current state
    pub async fn create_snapshot(&self, state_data: ControlPlaneSnapshot) -> Result<Uuid> {
        let snapshot_id = Uuid::new_v4();
        
        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            "Creating state snapshot"
        );

        // Serialize snapshot data
        let serialized_data = serde_json::to_vec(&state_data).map_err(|e| {
            AppError::ValidationError(format!("Failed to serialize snapshot: {}", e))
        })?;

        // Compress if enabled
        let final_data = if self.config.compression_enabled {
            self.compress_data(&serialized_data)?
        } else {
            serialized_data
        };

        // Encrypt if enabled
        let final_data = if self.config.encryption_enabled {
            self.encrypt_data(&final_data)?
        } else {
            final_data
        };

        // Calculate checksum
        let checksum = self.calculate_checksum(&final_data);

        // Create metadata
        let metadata = SnapshotMetadata {
            id: snapshot_id,
            control_plane_id: self.control_plane_id,
            timestamp: Utc::now(),
            version: "1.0".to_string(),
            size: final_data.len() as u64,
            checksum,
            compressed: self.config.compression_enabled,
            encrypted: self.config.encryption_enabled,
        };

        // Store snapshot
        self.store_snapshot(&metadata, &final_data).await?;

        // Add to snapshots registry
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(snapshot_id, metadata.clone());
        }

        // Clean up old snapshots
        self.cleanup_old_snapshots().await?;

        // Publish event
        let event = SnapshotEvent::SnapshotCreated {
            snapshot_id,
            control_plane_id: self.control_plane_id,
            size: metadata.size,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            size = metadata.size,
            "State snapshot created successfully"
        );

        Ok(snapshot_id)
    }

    /// Restore state from a snapshot
    pub async fn restore_snapshot(&self, snapshot_id: Uuid) -> Result<ControlPlaneSnapshot> {
        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            "Restoring state from snapshot"
        );

        // Get snapshot metadata
        let metadata = {
            let snapshots = self.snapshots.read().await;
            snapshots.get(&snapshot_id).cloned()
        };

        let Some(metadata) = metadata else {
            return Err(AppError::ValidationError(
                format!("Snapshot {} not found", snapshot_id)
            ));
        };

        // Load snapshot data
        let snapshot_data = self.load_snapshot(&metadata).await?;

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&snapshot_data);
        if calculated_checksum != metadata.checksum {
            return Err(AppError::ValidationError(
                "Snapshot checksum verification failed".to_string()
            ));
        }

        // Decrypt if needed
        let decrypted_data = if metadata.encrypted {
            self.decrypt_data(&snapshot_data)?
        } else {
            snapshot_data
        };

        // Decompress if needed
        let decompressed_data = if metadata.compressed {
            self.decompress_data(&decrypted_data)?
        } else {
            decrypted_data
        };

        // Deserialize snapshot
        let snapshot: ControlPlaneSnapshot = serde_json::from_slice(&decompressed_data).map_err(|e| {
            AppError::ValidationError(format!("Failed to deserialize snapshot: {}", e))
        })?;

        // Publish event
        let event = SnapshotEvent::SnapshotRestored {
            snapshot_id,
            control_plane_id: self.control_plane_id,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            "State restored from snapshot successfully"
        );

        Ok(snapshot)
    }

    /// List available snapshots
    pub async fn list_snapshots(&self) -> Vec<SnapshotMetadata> {
        let snapshots = self.snapshots.read().await;
        let mut snapshot_list: Vec<_> = snapshots.values().cloned().collect();
        snapshot_list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        snapshot_list
    }

    /// Delete a snapshot
    pub async fn delete_snapshot(&self, snapshot_id: Uuid, reason: String) -> Result<()> {
        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            reason = %reason,
            "Deleting snapshot"
        );

        // Get snapshot metadata
        let metadata = {
            let mut snapshots = self.snapshots.write().await;
            snapshots.remove(&snapshot_id)
        };

        let Some(metadata) = metadata else {
            return Err(AppError::ValidationError(
                format!("Snapshot {} not found", snapshot_id)
            ));
        };

        // Delete from storage
        self.delete_snapshot_from_storage(&metadata).await?;

        // Publish event
        let event = SnapshotEvent::SnapshotDeleted {
            snapshot_id,
            reason,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_id = %snapshot_id,
            "Snapshot deleted successfully"
        );

        Ok(())
    }

    /// Get the latest snapshot
    pub async fn get_latest_snapshot(&self) -> Option<SnapshotMetadata> {
        let snapshots = self.snapshots.read().await;
        snapshots
            .values()
            .max_by_key(|metadata| metadata.timestamp)
            .cloned()
    }

    /// Store snapshot to configured storage
    async fn store_snapshot(&self, metadata: &SnapshotMetadata, data: &[u8]) -> Result<()> {
        match self.config.storage_type {
            SnapshotStorageType::Disk => {
                self.store_snapshot_to_disk(metadata, data).await?;
            }
            SnapshotStorageType::S3 => {
                self.store_snapshot_to_s3(metadata, data).await?;
            }
            SnapshotStorageType::Both => {
                self.store_snapshot_to_disk(metadata, data).await?;
                self.store_snapshot_to_s3(metadata, data).await?;
            }
        }
        Ok(())
    }

    /// Store snapshot to disk
    async fn store_snapshot_to_disk(&self, metadata: &SnapshotMetadata, data: &[u8]) -> Result<()> {
        let filename = format!("snapshot-{}.json", metadata.id);
        let filepath = self.config.local_path.join(filename);

        fs::write(&filepath, data).await.map_err(|e| {
            AppError::ValidationError(format!("Failed to write snapshot to disk: {}", e))
        })?;

        // Store metadata separately
        let metadata_filename = format!("snapshot-{}.meta", metadata.id);
        let metadata_filepath = self.config.local_path.join(metadata_filename);
        let metadata_json = serde_json::to_vec(metadata).map_err(|e| {
            AppError::ValidationError(format!("Failed to serialize metadata: {}", e))
        })?;

        fs::write(&metadata_filepath, metadata_json).await.map_err(|e| {
            AppError::ValidationError(format!("Failed to write metadata to disk: {}", e))
        })?;

        debug!(
            snapshot_id = %metadata.id,
            filepath = ?filepath,
            "Snapshot stored to disk"
        );

        Ok(())
    }

    /// Store snapshot to S3
    async fn store_snapshot_to_s3(&self, _metadata: &SnapshotMetadata, _data: &[u8]) -> Result<()> {
        // In a real implementation, this would use AWS SDK to upload to S3
        // For now, we'll just log that S3 storage is not implemented
        warn!("S3 snapshot storage not implemented yet");
        Ok(())
    }

    /// Load snapshot from storage
    async fn load_snapshot(&self, metadata: &SnapshotMetadata) -> Result<Vec<u8>> {
        match self.config.storage_type {
            SnapshotStorageType::Disk | SnapshotStorageType::Both => {
                self.load_snapshot_from_disk(metadata).await
            }
            SnapshotStorageType::S3 => {
                self.load_snapshot_from_s3(metadata).await
            }
        }
    }

    /// Load snapshot from disk
    async fn load_snapshot_from_disk(&self, metadata: &SnapshotMetadata) -> Result<Vec<u8>> {
        let filename = format!("snapshot-{}.json", metadata.id);
        let filepath = self.config.local_path.join(filename);

        fs::read(&filepath).await.map_err(|e| {
            AppError::ValidationError(format!("Failed to read snapshot from disk: {}", e))
        })
    }

    /// Load snapshot from S3
    async fn load_snapshot_from_s3(&self, _metadata: &SnapshotMetadata) -> Result<Vec<u8>> {
        // In a real implementation, this would use AWS SDK to download from S3
        Err(AppError::ValidationError("S3 snapshot loading not implemented yet".to_string()))
    }

    /// Delete snapshot from storage
    async fn delete_snapshot_from_storage(&self, metadata: &SnapshotMetadata) -> Result<()> {
        match self.config.storage_type {
            SnapshotStorageType::Disk => {
                self.delete_snapshot_from_disk(metadata).await?;
            }
            SnapshotStorageType::S3 => {
                self.delete_snapshot_from_s3(metadata).await?;
            }
            SnapshotStorageType::Both => {
                self.delete_snapshot_from_disk(metadata).await?;
                self.delete_snapshot_from_s3(metadata).await?;
            }
        }
        Ok(())
    }

    /// Delete snapshot from disk
    async fn delete_snapshot_from_disk(&self, metadata: &SnapshotMetadata) -> Result<()> {
        let filename = format!("snapshot-{}.json", metadata.id);
        let filepath = self.config.local_path.join(filename);

        if filepath.exists() {
            fs::remove_file(&filepath).await.map_err(|e| {
                AppError::ValidationError(format!("Failed to delete snapshot from disk: {}", e))
            })?;
        }

        // Delete metadata file
        let metadata_filename = format!("snapshot-{}.meta", metadata.id);
        let metadata_filepath = self.config.local_path.join(metadata_filename);

        if metadata_filepath.exists() {
            fs::remove_file(&metadata_filepath).await.map_err(|e| {
                AppError::ValidationError(format!("Failed to delete metadata from disk: {}", e))
            })?;
        }

        Ok(())
    }

    /// Delete snapshot from S3
    async fn delete_snapshot_from_s3(&self, _metadata: &SnapshotMetadata) -> Result<()> {
        // In a real implementation, this would use AWS SDK to delete from S3
        warn!("S3 snapshot deletion not implemented yet");
        Ok(())
    }

    /// Load existing snapshots from storage
    async fn load_existing_snapshots(&self) -> Result<()> {
        if !matches!(self.config.storage_type, SnapshotStorageType::Disk | SnapshotStorageType::Both) {
            return Ok(());
        }

        let mut entries = fs::read_dir(&self.config.local_path).await.map_err(|e| {
            AppError::ValidationError(format!("Failed to read snapshot directory: {}", e))
        })?;

        let mut snapshots = self.snapshots.write().await;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            AppError::ValidationError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".meta") {
                    // Load metadata file
                    let metadata_data = fs::read(&path).await.map_err(|e| {
                        AppError::ValidationError(format!("Failed to read metadata file: {}", e))
                    })?;

                    let metadata: SnapshotMetadata = serde_json::from_slice(&metadata_data).map_err(|e| {
                        AppError::ValidationError(format!("Failed to parse metadata: {}", e))
                    })?;

                    snapshots.insert(metadata.id, metadata);
                }
            }
        }

        info!(
            control_plane_id = %self.control_plane_id,
            snapshot_count = snapshots.len(),
            "Loaded existing snapshots"
        );

        Ok(())
    }

    /// Clean up old snapshots
    async fn cleanup_old_snapshots(&self) -> Result<()> {
        let snapshots_to_delete = {
            let snapshots = self.snapshots.read().await;
            if snapshots.len() <= self.config.max_snapshots as usize {
                return Ok(());
            }

            let mut sorted_snapshots: Vec<_> = snapshots.values().cloned().collect();
            sorted_snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            // Keep the most recent snapshots, delete the rest
            sorted_snapshots
                .into_iter()
                .skip(self.config.max_snapshots as usize)
                .collect::<Vec<_>>()
        };

        for metadata in snapshots_to_delete {
            self.delete_snapshot(metadata.id, "Automatic cleanup".to_string()).await?;
        }

        Ok(())
    }

    /// Compress data
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In a real implementation, this would use a compression library like flate2
        // For now, we'll just return the original data
        Ok(data.to_vec())
    }

    /// Decompress data
    fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In a real implementation, this would decompress the data
        // For now, we'll just return the original data
        Ok(data.to_vec())
    }

    /// Encrypt data
    fn encrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In a real implementation, this would encrypt the data using AES or similar
        // For now, we'll just return the original data
        Ok(data.to_vec())
    }

    /// Decrypt data
    fn decrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In a real implementation, this would decrypt the data
        // For now, we'll just return the original data
        Ok(data.to_vec())
    }

    /// Calculate checksum
    fn calculate_checksum(&self, data: &[u8]) -> String {
        // In a real implementation, this would calculate SHA-256 or similar
        // For now, we'll use a simple hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Start periodic snapshots
    async fn start_periodic_snapshots(&self) {
        let _event_sender = self.event_sender.clone();
        let control_plane_id = self.control_plane_id;
        let interval_secs = self.config.snapshot_interval;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                debug!(
                    control_plane_id = %control_plane_id,
                    "Periodic snapshot trigger"
                );

                // In a real implementation, this would trigger snapshot creation
                // by calling create_snapshot with current state data
            }
        });

        let mut timer = self.snapshot_timer.write().await;
        *timer = Some(handle);
    }

    /// Stop periodic snapshots
    async fn stop_periodic_snapshots(&self) {
        let mut timer = self.snapshot_timer.write().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
    }

    /// Publish an event
    async fn publish_event(&self, event: SnapshotEvent) -> Result<()> {
        // Send to internal channel
        if let Err(_) = self.event_sender.send(event.clone()) {
            warn!("Failed to send snapshot event to internal channel");
        }

        // Publish to event bus
        self.event_bus.publish(event).await?;

        Ok(())
    }

    /// Start event processing task
    async fn start_event_processing(&self) {
        let event_receiver = self.event_receiver.clone();
        let control_plane_id = self.control_plane_id;

        tokio::spawn(async move {
            let receiver = {
                let mut guard = event_receiver.write().await;
                guard.take()
            };

            if let Some(mut rx) = receiver {
                while let Some(event) = rx.recv().await {
                    Self::process_snapshot_event(control_plane_id, event).await;
                }
            }
        });
    }

    /// Process snapshot events
    async fn process_snapshot_event(control_plane_id: Uuid, event: SnapshotEvent) {
        match event {
            SnapshotEvent::SnapshotCreated { snapshot_id, size, timestamp, .. } => {
                info!(
                    control_plane_id = %control_plane_id,
                    snapshot_id = %snapshot_id,
                    size = size,
                    timestamp = %timestamp,
                    "Snapshot created"
                );
            }
            SnapshotEvent::SnapshotRestored { snapshot_id, timestamp, .. } => {
                info!(
                    control_plane_id = %control_plane_id,
                    snapshot_id = %snapshot_id,
                    timestamp = %timestamp,
                    "Snapshot restored"
                );
            }
            SnapshotEvent::SnapshotFailed { reason, timestamp, .. } => {
                error!(
                    control_plane_id = %control_plane_id,
                    reason = %reason,
                    timestamp = %timestamp,
                    "Snapshot failed"
                );
            }
            _ => {
                debug!(
                    control_plane_id = %control_plane_id,
                    event = ?event,
                    "Snapshot event processed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patterns::correlation::CorrelationTracker;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = SnapshotConfig {
            local_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let control_plane_id = Uuid::new_v4();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let snapshot_manager = StateSnapshotManager::new(config, control_plane_id, event_bus);
        
        // Test basic functionality
        let snapshots = snapshot_manager.list_snapshots().await;
        assert!(snapshots.is_empty());
        
        let latest = snapshot_manager.get_latest_snapshot().await;
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_creation_and_restoration() {
        let temp_dir = TempDir::new().unwrap();
        let config = SnapshotConfig {
            local_path: temp_dir.path().to_path_buf(),
            compression_enabled: false,
            encryption_enabled: false,
            ..Default::default()
        };
        
        let control_plane_id = Uuid::new_v4();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let snapshot_manager = StateSnapshotManager::new(config, control_plane_id, event_bus);
        snapshot_manager.start().await.unwrap();
        
        // Create test snapshot data
        let snapshot_data = ControlPlaneSnapshot {
            metadata: SnapshotMetadata {
                id: Uuid::new_v4(),
                control_plane_id,
                timestamp: Utc::now(),
                version: "1.0".to_string(),
                size: 0,
                checksum: "".to_string(),
                compressed: false,
                encrypted: false,
            },
            control_plane_state: ControlPlaneStateData {
                state: "Running".to_string(),
                current_term: 1,
                current_leader: Some(Uuid::new_v4()),
                config: serde_json::json!({}),
            },
            cluster_state: ClusterStateData {
                cluster_id: Uuid::new_v4(),
                status: "Healthy".to_string(),
                active_nodes: vec![],
                failed_nodes: vec![],
                metrics: serde_json::json!({}),
            },
            job_state: JobStateData {
                running_jobs: HashMap::new(),
                queued_jobs: vec![],
                job_distribution: HashMap::new(),
                job_retry_counts: HashMap::new(),
            },
            runner_state: RunnerStateData {
                active_runners: HashMap::new(),
                runner_assignments: HashMap::new(),
                runner_health: HashMap::new(),
            },
            node_health_state: NodeHealthStateData {
                node_health: HashMap::new(),
                failed_nodes: HashMap::new(),
            },
        };
        
        // Create snapshot
        let snapshot_id = snapshot_manager.create_snapshot(snapshot_data.clone()).await.unwrap();
        
        // Verify snapshot was created
        let snapshots = snapshot_manager.list_snapshots().await;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot_id);
        
        // Restore snapshot
        let restored_data = snapshot_manager.restore_snapshot(snapshot_id).await.unwrap();
        assert_eq!(restored_data.control_plane_state.state, "Running");
        assert_eq!(restored_data.control_plane_state.current_term, 1);
        
        snapshot_manager.stop().await.unwrap();
    }
}