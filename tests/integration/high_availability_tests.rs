//! Integration tests for High Availability features
//!
//! This module tests the leader election, failover, state snapshots,
//! and idempotent job execution features.

use std::sync::Arc;
use uuid::Uuid;

use crate::common::*;
use RustAutoDevOps::{
    core::{
        control_plane::{ControlPlaneConfig, ControlPlaneMaster, ControlPlaneSettings},
        events::EventBus,
        failover::{FailoverConfig, FailoverManager, FailureType},
        idempotent_jobs::{IdempotencyConfig, IdempotentJobManager, IdempotencyKey, JobResult},
        leader_election::{LeaderElectionConfig, LeaderElectionManager},
        state_snapshot::{SnapshotConfig, StateSnapshotManager},
        CorrelationTracker,
    },
    domain::{entities, repositories},
    error::Result,
};

#[tokio::test]
async fn test_leader_election_basic_functionality() {
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
    
    let config = LeaderElectionConfig::default();
    let leader_election = LeaderElectionManager::new(config, event_bus);
    
    // Initially should be follower
    assert_eq!(leader_election.get_state().await, RustAutoDevOps::core::leader_election::NodeState::Follower);
    assert_eq!(leader_election.get_current_term().await, 0);
    assert!(!leader_election.is_leader().await);
    
    // Start leader election
    leader_election.start().await.unwrap();
    
    // Should still be follower initially
    assert_eq!(leader_election.get_state().await, RustAutoDevOps::core::leader_election::NodeState::Follower);
    
    leader_election.stop().await.unwrap();
}

#[tokio::test]
async fn test_failover_manager_node_registration() {
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
    let leader_election = Arc::new(LeaderElectionManager::new(
        LeaderElectionConfig::default(),
        event_bus.clone(),
    ));
    
    let config = FailoverConfig::default();
    let failover_manager = FailoverManager::new(config, leader_election, event_bus);
    
    failover_manager.start().await.unwrap();
    
    let node_id = Uuid::new_v4();
    
    // Register a node
    failover_manager.register_node(node_id).await.unwrap();
    
    // Unregister the node
    failover_manager.unregister_node(node_id).await.unwrap();
    
    failover_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_state_snapshot_creation_and_restoration() {
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config = SnapshotConfig {
        local_path: temp_dir.path().to_path_buf(),
        compression_enabled: false,
        encryption_enabled: false,
        ..Default::default()
    };
    
    let control_plane_id = Uuid::new_v4();
    let snapshot_manager = StateSnapshotManager::new(config, control_plane_id, event_bus);
    
    snapshot_manager.start().await.unwrap();
    
    // Create test snapshot data
    let snapshot_data = RustAutoDevOps::core::state_snapshot::ControlPlaneSnapshot {
        metadata: RustAutoDevOps::core::state_snapshot::SnapshotMetadata {
            id: Uuid::new_v4(),
            control_plane_id,
            timestamp: chrono::Utc::now(),
            version: "1.0".to_string(),
            size: 0,
            checksum: "".to_string(),
            compressed: false,
            encrypted: false,
        },
        control_plane_state: RustAutoDevOps::core::state_snapshot::ControlPlaneStateData {
            state: "Running".to_string(),
            current_term: 1,
            current_leader: Some(Uuid::new_v4()),
            config: serde_json::json!({}),
        },
        cluster_state: RustAutoDevOps::core::state_snapshot::ClusterStateData {
            cluster_id: Uuid::new_v4(),
            status: "Healthy".to_string(),
            active_nodes: vec![],
            failed_nodes: vec![],
            metrics: serde_json::json!({}),
        },
        job_state: RustAutoDevOps::core::state_snapshot::JobStateData {
            running_jobs: std::collections::HashMap::new(),
            queued_jobs: vec![],
            job_distribution: std::collections::HashMap::new(),
            job_retry_counts: std::collections::HashMap::new(),
        },
        runner_state: RustAutoDevOps::core::state_snapshot::RunnerStateData {
            active_runners: std::collections::HashMap::new(),
            runner_assignments: std::collections::HashMap::new(),
            runner_health: std::collections::HashMap::new(),
        },
        node_health_state: RustAutoDevOps::core::state_snapshot::NodeHealthStateData {
            node_health: std::collections::HashMap::new(),
            failed_nodes: std::collections::HashMap::new(),
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

#[tokio::test]
async fn test_idempotent_job_execution() {
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
    
    let config = IdempotencyConfig::default();
    let job_manager = IdempotentJobManager::new(config, event_bus);
    
    job_manager.start().await.unwrap();
    
    let job_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let runner_id = Uuid::new_v4();
    let key = IdempotencyKey::new(job_id, "test-hash".to_string(), 1);
    let job_config = serde_json::json!({"command": "echo hello"});
    
    // Check if job is idempotent (should be true initially)
    let is_idempotent = job_manager.is_job_idempotent(&key).await.unwrap();
    assert!(is_idempotent);
    
    // Start job execution
    let result = job_manager.start_job_execution(
        key.clone(),
        node_id,
        runner_id,
        job_config.clone(),
    ).await.unwrap();
    
    assert!(matches!(result, RustAutoDevOps::core::idempotent_jobs::JobExecutionResult::Proceed));
    
    // Try to start same job again (should be duplicate)
    let result = job_manager.start_job_execution(
        key.clone(),
        node_id,
        runner_id,
        job_config,
    ).await.unwrap();
    
    assert!(matches!(result, RustAutoDevOps::core::idempotent_jobs::JobExecutionResult::Duplicate(_)));
    
    // Complete the job successfully
    let job_result = JobResult {
        success: true,
        exit_code: Some(0),
        output: Some("hello".to_string()),
        error: None,
        duration_ms: 100,
        resource_usage: None,
    };
    
    job_manager.complete_job_execution(key.clone(), job_result).await.unwrap();
    
    // Try to start same job again (should return cached result)
    let result = job_manager.start_job_execution(
        key.clone(),
        node_id,
        runner_id,
        serde_json::json!({"command": "echo hello"}),
    ).await.unwrap();
    
    assert!(matches!(result, RustAutoDevOps::core::idempotent_jobs::JobExecutionResult::Cached(_)));
    
    job_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_control_plane_with_high_availability() {
    // Create test repositories
    let cluster_repo = Arc::new(create_mock_cluster_repository());
    let job_distribution_repo = Arc::new(create_mock_job_distribution_repository());
    
    // Create dependencies
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
    let node_registry = Arc::new(create_test_node_registry());
    
    // Create cluster coordinator
    let cluster_coordinator = Arc::new(
        RustAutoDevOps::core::cluster_coordinator::ClusterCoordinator::new(
            RustAutoDevOps::core::cluster_coordinator::ClusterCoordinatorConfig::default(),
            node_registry.clone(),
            cluster_repo,
            job_distribution_repo,
        )
        .await
        .unwrap(),
    );
    
    // Create job scheduler
    let runner_pool = Arc::new(RustAutoDevOps::core::runner_pool::DefaultRunnerPoolManager::with_default_config());
    let job_scheduler = Arc::new(RustAutoDevOps::core::job_scheduler::DefaultJobScheduler::new(
        runner_pool,
        RustAutoDevOps::core::job_scheduler::JobSchedulerConfig::default(),
    ));
    
    // Create control plane config with HA enabled
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mut config = ControlPlaneConfig::default();
    config.control_plane_settings.high_availability = true;
    config.control_plane_settings.snapshot_config.local_path = temp_dir.path().to_path_buf();
    
    // Create control plane
    let control_plane = ControlPlaneMaster::new(
        config,
        cluster_coordinator,
        node_registry,
        job_scheduler,
        event_bus,
    )
    .await
    .unwrap();
    
    // Start control plane
    control_plane.start().await.unwrap();
    
    // Test HA features
    assert!(!control_plane.is_leader().await); // Should be false initially (follower)
    
    // Test snapshot creation
    let snapshot_id = control_plane.create_snapshot().await.unwrap();
    assert!(snapshot_id.is_some());
    
    // Test idempotent job execution
    let job_id = Uuid::new_v4();
    let key = IdempotencyKey::new(job_id, "test-hash".to_string(), 1);
    let is_idempotent = control_plane.is_job_idempotent(&key).await.unwrap();
    assert!(is_idempotent);
    
    // Stop control plane
    control_plane.stop("Test completed".to_string()).await.unwrap();
}

// Helper functions for creating mock objects
fn create_mock_cluster_repository() -> impl RustAutoDevOps::domain::repositories::ClusterRepository {
    struct MockClusterRepository;
    
    #[async_trait::async_trait]
    impl RustAutoDevOps::domain::repositories::ClusterRepository for MockClusterRepository {
        async fn create(&self, cluster: &RustAutoDevOps::domain::entities::Cluster) -> RustAutoDevOps::error::Result<RustAutoDevOps::domain::entities::Cluster> {
            Ok(cluster.clone())
        }
        
        async fn find_by_id(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId) -> RustAutoDevOps::error::Result<Option<RustAutoDevOps::domain::entities::Cluster>> {
            Ok(None)
        }
        
        async fn find_by_name(&self, _name: &str) -> RustAutoDevOps::error::Result<Option<RustAutoDevOps::domain::entities::Cluster>> {
            Ok(None)
        }
        
        async fn find_all(&self) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::Cluster>> {
            Ok(Vec::new())
        }
        
        async fn update(&self, cluster: &RustAutoDevOps::domain::entities::Cluster) -> RustAutoDevOps::error::Result<RustAutoDevOps::domain::entities::Cluster> {
            Ok(cluster.clone())
        }
        
        async fn delete(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId) -> RustAutoDevOps::error::Result<()> {
            Ok(())
        }
        
        async fn exists(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId) -> RustAutoDevOps::error::Result<bool> {
            Ok(false)
        }
        
        async fn count(&self) -> RustAutoDevOps::error::Result<u64> {
            Ok(0)
        }
        
        async fn add_node(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId, _node_id: RustAutoDevOps::domain::entities::NodeId) -> RustAutoDevOps::error::Result<()> {
            Ok(())
        }
        
        async fn remove_node(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId, _node_id: RustAutoDevOps::domain::entities::NodeId) -> RustAutoDevOps::error::Result<()> {
            Ok(())
        }
        
        async fn get_metrics(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId) -> RustAutoDevOps::error::Result<RustAutoDevOps::domain::entities::ClusterMetrics> {
            Ok(RustAutoDevOps::domain::entities::ClusterMetrics::new())
        }
        
        async fn update_metrics(&self, _cluster_id: RustAutoDevOps::domain::entities::ClusterId, _metrics: &RustAutoDevOps::domain::entities::ClusterMetrics) -> RustAutoDevOps::error::Result<()> {
            Ok(())
        }
    }
    
    MockClusterRepository
}

fn create_mock_job_distribution_repository() -> impl RustAutoDevOps::domain::repositories::JobDistributionRepository {
    struct MockJobDistributionRepository;
    
    #[async_trait::async_trait]
    impl RustAutoDevOps::domain::repositories::JobDistributionRepository for MockJobDistributionRepository {
        async fn create(&self, distribution: &RustAutoDevOps::domain::entities::JobDistribution) -> RustAutoDevOps::error::Result<RustAutoDevOps::domain::entities::JobDistribution> {
            Ok(distribution.clone())
        }
        
        async fn find_by_job_id(&self, _job_id: RustAutoDevOps::domain::entities::JobId) -> RustAutoDevOps::error::Result<Option<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(None)
        }
        
        async fn find_by_node_id(&self, _node_id: RustAutoDevOps::domain::entities::NodeId) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(Vec::new())
        }
        
        async fn find_by_runner_id(&self, _runner_id: RustAutoDevOps::domain::entities::RunnerId) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(Vec::new())
        }
        
        async fn find_by_strategy(&self, _strategy: RustAutoDevOps::domain::entities::LoadBalancingStrategy) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(Vec::new())
        }
        
        async fn find_by_time_range(&self, _start: chrono::DateTime<chrono::Utc>, _end: chrono::DateTime<chrono::Utc>) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(Vec::new())
        }
        
        async fn delete(&self, _job_id: RustAutoDevOps::domain::entities::JobId) -> RustAutoDevOps::error::Result<()> {
            Ok(())
        }
        
        async fn get_distribution_stats(&self) -> RustAutoDevOps::error::Result<RustAutoDevOps::domain::repositories::DistributionStats> {
            Ok(RustAutoDevOps::domain::repositories::DistributionStats::new())
        }
        
        async fn find_with_pagination(&self, _limit: usize, _offset: usize) -> RustAutoDevOps::error::Result<Vec<RustAutoDevOps::domain::entities::JobDistribution>> {
            Ok(Vec::new())
        }
    }
    
    MockJobDistributionRepository
}

fn create_test_node_registry() -> RustAutoDevOps::core::node_registry::NodeRegistry {
    // This would create a test node registry
    // For now, we'll use a placeholder that compiles
    RustAutoDevOps::core::node_registry::tests::create_test_registry()
}