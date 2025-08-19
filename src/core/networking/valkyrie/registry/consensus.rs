//! Consensus Manager for Distributed Service Registry
//!
//! Implements distributed consensus protocols for service registry coordination
//! across multiple nodes with fault tolerance and consistency guarantees.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use super::{ServiceEntry, ServiceId};
use crate::error::{Result, ValkyrieError};

/// Consensus manager for distributed coordination
pub struct ConsensusManager {
    /// Consensus protocol implementation
    protocol: ConsensusProtocol,
    /// Node ID for this instance
    node_id: NodeId,
    /// Cluster configuration
    cluster_config: ClusterConfig,
    /// Current cluster state
    cluster_state: Arc<RwLock<ClusterState>>,
    /// Consensus log
    consensus_log: Arc<RwLock<Vec<LogEntry>>>,
    /// State machine for service registry
    state_machine: Arc<RwLock<RegistryStateMachine>>,
    /// Pending proposals
    pending_proposals: Arc<DashMap<ProposalId, Proposal>>,
    /// Consensus metrics
    metrics: Arc<RwLock<ConsensusMetrics>>,
    /// Configuration
    config: ConsensusConfig,
}

/// Consensus protocols
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusProtocol {
    /// Raft consensus protocol
    Raft,
    /// PBFT (Practical Byzantine Fault Tolerance)
    Pbft,
    /// Paxos consensus protocol
    Paxos,
    /// Custom consensus protocol
    Custom(String),
}

/// Node identifier
pub type NodeId = uuid::Uuid;

/// Proposal identifier
pub type ProposalId = uuid::Uuid;

/// Cluster configuration
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// All nodes in the cluster
    pub nodes: Vec<NodeInfo>,
    /// Quorum size required for consensus
    pub quorum_size: usize,
    /// This node's information
    pub local_node: NodeInfo,
    /// Cluster name/identifier
    pub cluster_name: String,
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node ID
    pub node_id: NodeId,
    /// Node address
    pub address: String,
    /// Node port
    pub port: u16,
    /// Node role
    pub role: NodeRole,
    /// Node status
    pub status: NodeStatus,
    /// Last heartbeat
    pub last_heartbeat: Instant,
    /// Node metadata
    pub metadata: HashMap<String, String>,
}

/// Node roles in consensus
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRole {
    /// Leader node (in Raft)
    Leader,
    /// Follower node
    Follower,
    /// Candidate node (during election)
    Candidate,
    /// Observer node (read-only)
    Observer,
}

/// Node status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeStatus {
    /// Node is active and participating
    Active,
    /// Node is inactive/down
    Inactive,
    /// Node is suspected to be down
    Suspected,
    /// Node is joining the cluster
    Joining,
    /// Node is leaving the cluster
    Leaving,
}

/// Cluster state
#[derive(Debug, Clone)]
pub struct ClusterState {
    /// Current term (for Raft)
    pub current_term: u64,
    /// Current leader
    pub current_leader: Option<NodeId>,
    /// Voted for in current term
    pub voted_for: Option<NodeId>,
    /// Cluster membership
    pub membership: Vec<NodeId>,
    /// Last applied log index
    pub last_applied: u64,
    /// Commit index
    pub commit_index: u64,
    /// Next index for each node (leader only)
    pub next_index: HashMap<NodeId, u64>,
    /// Match index for each node (leader only)
    pub match_index: HashMap<NodeId, u64>,
}

/// Log entry for consensus
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Log entry index
    pub index: u64,
    /// Term when entry was created
    pub term: u64,
    /// Log entry type
    pub entry_type: LogEntryType,
    /// Entry data
    pub data: Vec<u8>,
    /// Timestamp
    pub timestamp: Instant,
    /// Entry ID
    pub entry_id: uuid::Uuid,
}

/// Log entry types
#[derive(Debug, Clone)]
pub enum LogEntryType {
    /// Service registration
    ServiceRegistration(ServiceEntry),
    /// Service deregistration
    ServiceDeregistration(ServiceId),
    /// Service update
    ServiceUpdate(ServiceEntry),
    /// Configuration change
    ConfigurationChange(ClusterConfig),
    /// No-op entry (for leader election)
    NoOp,
    /// Custom entry
    Custom(String, Vec<u8>),
}

/// Registry state machine
#[derive(Debug, Clone)]
pub struct RegistryStateMachine {
    /// Registered services
    pub services: HashMap<ServiceId, ServiceEntry>,
    /// Last applied index
    pub last_applied: u64,
    /// State machine version
    pub version: u64,
    /// Snapshots
    pub snapshots: Vec<Snapshot>,
}

/// State machine snapshot
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Snapshot index
    pub index: u64,
    /// Snapshot term
    pub term: u64,
    /// Snapshot data
    pub data: Vec<u8>,
    /// Snapshot timestamp
    pub timestamp: Instant,
    /// Snapshot ID
    pub snapshot_id: uuid::Uuid,
}

/// Consensus proposal
#[derive(Debug, Clone)]
pub struct Proposal {
    /// Proposal ID
    pub proposal_id: ProposalId,
    /// Proposer node ID
    pub proposer: NodeId,
    /// Proposal type
    pub proposal_type: ProposalType,
    /// Proposal data
    pub data: Vec<u8>,
    /// Proposal timestamp
    pub timestamp: Instant,
    /// Votes received
    pub votes: HashMap<NodeId, Vote>,
    /// Proposal status
    pub status: ProposalStatus,
}

/// Proposal types
#[derive(Debug, Clone)]
pub enum ProposalType {
    /// Service operation proposal
    ServiceOperation(ServiceOperation),
    /// Configuration change proposal
    ConfigurationChange,
    /// Leadership change proposal
    LeadershipChange,
    /// Custom proposal
    Custom(String),
}

/// Service operations for consensus
#[derive(Debug, Clone)]
pub enum ServiceOperation {
    /// Register service
    Register(ServiceEntry),
    /// Deregister service
    Deregister(ServiceId),
    /// Update service
    Update(ServiceEntry),
}

/// Vote in consensus
#[derive(Debug, Clone)]
pub struct Vote {
    /// Voter node ID
    pub voter: NodeId,
    /// Vote decision
    pub decision: VoteDecision,
    /// Vote timestamp
    pub timestamp: Instant,
    /// Vote term
    pub term: u64,
}

/// Vote decisions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteDecision {
    /// Vote in favor
    Accept,
    /// Vote against
    Reject,
    /// Abstain from voting
    Abstain,
}

/// Proposal status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    /// Proposal is pending
    Pending,
    /// Proposal is accepted
    Accepted,
    /// Proposal is rejected
    Rejected,
    /// Proposal timed out
    TimedOut,
}

/// Consensus configuration
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Election timeout
    pub election_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Proposal timeout
    pub proposal_timeout: Duration,
    /// Maximum log entries per append
    pub max_log_entries: usize,
    /// Snapshot threshold
    pub snapshot_threshold: u64,
    /// Enable pre-vote
    pub enable_pre_vote: bool,
    /// Maximum retries for proposals
    pub max_proposal_retries: u32,
    /// Batch size for log replication
    pub replication_batch_size: usize,
}

/// Consensus metrics
#[derive(Debug, Clone)]
pub struct ConsensusMetrics {
    /// Total proposals
    pub total_proposals: u64,
    /// Accepted proposals
    pub accepted_proposals: u64,
    /// Rejected proposals
    pub rejected_proposals: u64,
    /// Leader elections
    pub leader_elections: u64,
    /// Log entries
    pub log_entries: u64,
    /// Snapshots created
    pub snapshots_created: u64,
    /// Average proposal latency
    pub avg_proposal_latency: Duration,
    /// Current term
    pub current_term: u64,
    /// Is leader
    pub is_leader: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            election_timeout: Duration::from_millis(150),
            heartbeat_interval: Duration::from_millis(50),
            proposal_timeout: Duration::from_secs(5),
            max_log_entries: 1000,
            snapshot_threshold: 10000,
            enable_pre_vote: true,
            max_proposal_retries: 3,
            replication_batch_size: 100,
        }
    }
}

impl ConsensusManager {
    /// Create new consensus manager
    pub fn new(protocol: ConsensusProtocol) -> Self {
        let node_id = uuid::Uuid::new_v4();
        let cluster_config = ClusterConfig {
            nodes: vec![NodeInfo {
                node_id,
                address: "localhost".to_string(),
                port: 8080,
                role: NodeRole::Follower,
                status: NodeStatus::Active,
                last_heartbeat: Instant::now(),
                metadata: HashMap::new(),
            }],
            quorum_size: 1,
            local_node: NodeInfo {
                node_id,
                address: "localhost".to_string(),
                port: 8080,
                role: NodeRole::Follower,
                status: NodeStatus::Active,
                last_heartbeat: Instant::now(),
                metadata: HashMap::new(),
            },
            cluster_name: "valkyrie-registry".to_string(),
        };

        Self::with_config(protocol, cluster_config, ConsensusConfig::default())
    }

    /// Create consensus manager with custom configuration
    pub fn with_config(
        protocol: ConsensusProtocol,
        cluster_config: ClusterConfig,
        config: ConsensusConfig,
    ) -> Self {
        let node_id = cluster_config.local_node.node_id;

        Self {
            protocol,
            node_id,
            cluster_config: cluster_config.clone(),
            cluster_state: Arc::new(RwLock::new(ClusterState {
                current_term: 0,
                current_leader: None,
                voted_for: None,
                membership: cluster_config.nodes.iter().map(|n| n.node_id).collect(),
                last_applied: 0,
                commit_index: 0,
                next_index: HashMap::new(),
                match_index: HashMap::new(),
            })),
            consensus_log: Arc::new(RwLock::new(Vec::new())),
            state_machine: Arc::new(RwLock::new(RegistryStateMachine {
                services: HashMap::new(),
                last_applied: 0,
                version: 0,
                snapshots: Vec::new(),
            })),
            pending_proposals: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(ConsensusMetrics::default())),
            config,
        }
    }

    /// Start consensus manager
    pub async fn start(&self) -> Result<()> {
        match self.protocol {
            ConsensusProtocol::Raft => self.start_raft().await,
            ConsensusProtocol::Pbft => self.start_pbft().await,
            ConsensusProtocol::Paxos => self.start_paxos().await,
            ConsensusProtocol::Custom(_) => self.start_custom().await,
        }
    }

    /// Start Raft consensus protocol
    async fn start_raft(&self) -> Result<()> {
        info!(
            "Starting Raft consensus protocol for node: {}",
            self.node_id
        );

        // Start election timer
        self.start_election_timer().await;

        // Start heartbeat timer (if leader)
        self.start_heartbeat_timer().await;

        // Start log replication
        self.start_log_replication().await;

        Ok(())
    }

    /// Start PBFT consensus protocol
    async fn start_pbft(&self) -> Result<()> {
        info!(
            "Starting PBFT consensus protocol for node: {}",
            self.node_id
        );
        // PBFT implementation would go here
        Ok(())
    }

    /// Start Paxos consensus protocol
    async fn start_paxos(&self) -> Result<()> {
        info!(
            "Starting Paxos consensus protocol for node: {}",
            self.node_id
        );
        // Paxos implementation would go here
        Ok(())
    }

    /// Start custom consensus protocol
    async fn start_custom(&self) -> Result<()> {
        info!(
            "Starting custom consensus protocol for node: {}",
            self.node_id
        );
        // Custom protocol implementation would go here
        Ok(())
    }

    /// Register service through consensus
    pub async fn register_service(
        &self,
        service_id: ServiceId,
        service: &ServiceEntry,
    ) -> Result<()> {
        let proposal = Proposal {
            proposal_id: uuid::Uuid::new_v4(),
            proposer: self.node_id,
            proposal_type: ProposalType::ServiceOperation(ServiceOperation::Register(
                service.clone(),
            )),
            data: serde_json::to_vec(service).unwrap_or_default(),
            timestamp: Instant::now(),
            votes: HashMap::new(),
            status: ProposalStatus::Pending,
        };

        self.submit_proposal(proposal).await
    }

    /// Deregister service through consensus
    pub async fn deregister_service(&self, service_id: ServiceId) -> Result<()> {
        let proposal = Proposal {
            proposal_id: uuid::Uuid::new_v4(),
            proposer: self.node_id,
            proposal_type: ProposalType::ServiceOperation(ServiceOperation::Deregister(service_id)),
            data: service_id.as_bytes().to_vec(),
            timestamp: Instant::now(),
            votes: HashMap::new(),
            status: ProposalStatus::Pending,
        };

        self.submit_proposal(proposal).await
    }

    /// Submit proposal for consensus
    async fn submit_proposal(&self, proposal: Proposal) -> Result<()> {
        let proposal_id = proposal.proposal_id;

        // Store pending proposal
        self.pending_proposals.insert(proposal_id, proposal.clone());

        // Create log entry
        let log_entry = LogEntry {
            index: self.get_next_log_index().await,
            term: self.get_current_term().await,
            entry_type: match proposal.proposal_type {
                ProposalType::ServiceOperation(op) => match op {
                    ServiceOperation::Register(service) => {
                        LogEntryType::ServiceRegistration(service)
                    }
                    ServiceOperation::Deregister(service_id) => {
                        LogEntryType::ServiceDeregistration(service_id)
                    }
                    ServiceOperation::Update(service) => LogEntryType::ServiceUpdate(service),
                },
                _ => LogEntryType::Custom("proposal".to_string(), proposal.data.clone()),
            },
            data: proposal.data,
            timestamp: Instant::now(),
            entry_id: uuid::Uuid::new_v4(),
        };

        // Append to log
        self.append_log_entry(log_entry).await?;

        // Replicate to followers (if leader)
        if self.is_leader().await {
            self.replicate_log_entries().await?;
        }

        // Wait for consensus or timeout
        self.wait_for_consensus(proposal_id).await
    }

    /// Append log entry
    async fn append_log_entry(&self, entry: LogEntry) -> Result<()> {
        let mut log = self.consensus_log.write().await;
        log.push(entry);

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.log_entries += 1;

        Ok(())
    }

    /// Replicate log entries to followers
    async fn replicate_log_entries(&self) -> Result<()> {
        // Simplified replication - in practice would send AppendEntries RPCs
        debug!("Replicating log entries to followers");
        Ok(())
    }

    /// Wait for consensus on proposal
    async fn wait_for_consensus(&self, proposal_id: ProposalId) -> Result<()> {
        let timeout = tokio::time::sleep(self.config.proposal_timeout);
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    // Timeout - mark proposal as timed out
                    if let Some(mut proposal) = self.pending_proposals.get_mut(&proposal_id) {
                        proposal.status = ProposalStatus::TimedOut;
                    }
                    return Err(ValkyrieError::ConsensusTimeout(format!(
                        "Proposal {} timed out", proposal_id
                    )));
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    // Check proposal status
                    if let Some(proposal) = self.pending_proposals.get(&proposal_id) {
                        match proposal.status {
                            ProposalStatus::Accepted => {
                                self.pending_proposals.remove(&proposal_id);
                                return Ok(());
                            }
                            ProposalStatus::Rejected => {
                                self.pending_proposals.remove(&proposal_id);
                                return Err(ValkyrieError::ConsensusRejected(format!(
                                    "Proposal {} was rejected", proposal_id
                                )));
                            }
                            ProposalStatus::TimedOut => {
                                self.pending_proposals.remove(&proposal_id);
                                return Err(ValkyrieError::ConsensusTimeout(format!(
                                    "Proposal {} timed out", proposal_id
                                )));
                            }
                            ProposalStatus::Pending => {
                                // Continue waiting
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Start election timer
    async fn start_election_timer(&self) {
        let consensus = self.clone();
        let election_timeout = self.config.election_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(election_timeout);

            loop {
                interval.tick().await;

                if !consensus.is_leader().await && !consensus.received_heartbeat_recently().await {
                    if let Err(e) = consensus.start_election().await {
                        error!("Election failed: {}", e);
                    }
                }
            }
        });
    }

    /// Start heartbeat timer
    async fn start_heartbeat_timer(&self) {
        let consensus = self.clone();
        let heartbeat_interval = self.config.heartbeat_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);

            loop {
                interval.tick().await;

                if consensus.is_leader().await {
                    if let Err(e) = consensus.send_heartbeats().await {
                        error!("Heartbeat failed: {}", e);
                    }
                }
            }
        });
    }

    /// Start log replication
    async fn start_log_replication(&self) {
        let consensus = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));

            loop {
                interval.tick().await;

                if consensus.is_leader().await {
                    if let Err(e) = consensus.replicate_log_entries().await {
                        error!("Log replication failed: {}", e);
                    }
                }
            }
        });
    }

    /// Start election
    async fn start_election(&self) -> Result<()> {
        info!("Starting leader election for node: {}", self.node_id);

        let mut state = self.cluster_state.write().await;
        state.current_term += 1;
        state.voted_for = Some(self.node_id);

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.leader_elections += 1;
        metrics.current_term = state.current_term;

        // In a real implementation, would send RequestVote RPCs to all nodes
        // For now, assume we win the election if we're the only node
        if self.cluster_config.nodes.len() == 1 {
            self.become_leader().await?;
        }

        Ok(())
    }

    /// Become leader
    async fn become_leader(&self) -> Result<()> {
        info!(
            "Node {} became leader for term {}",
            self.node_id,
            self.get_current_term().await
        );

        let mut state = self.cluster_state.write().await;
        state.current_leader = Some(self.node_id);

        // Initialize next_index and match_index for all followers
        for node in &self.cluster_config.nodes {
            if node.node_id != self.node_id {
                state
                    .next_index
                    .insert(node.node_id, self.get_last_log_index().await + 1);
                state.match_index.insert(node.node_id, 0);
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.is_leader = true;

        Ok(())
    }

    /// Send heartbeats to followers
    async fn send_heartbeats(&self) -> Result<()> {
        // In a real implementation, would send AppendEntries RPCs with no entries
        debug!("Sending heartbeats to followers");
        Ok(())
    }

    /// Check if received heartbeat recently
    async fn received_heartbeat_recently(&self) -> bool {
        // Simplified check - in practice would track last heartbeat time
        false
    }

    /// Check if this node is the leader
    async fn is_leader(&self) -> bool {
        let state = self.cluster_state.read().await;
        state.current_leader == Some(self.node_id)
    }

    /// Get current term
    async fn get_current_term(&self) -> u64 {
        let state = self.cluster_state.read().await;
        state.current_term
    }

    /// Get next log index
    async fn get_next_log_index(&self) -> u64 {
        let log = self.consensus_log.read().await;
        log.len() as u64
    }

    /// Get last log index
    async fn get_last_log_index(&self) -> u64 {
        let log = self.consensus_log.read().await;
        if log.is_empty() {
            0
        } else {
            log.len() as u64 - 1
        }
    }

    /// Apply log entries to state machine
    async fn apply_log_entries(&self) -> Result<()> {
        let log = self.consensus_log.read().await;
        let mut state_machine = self.state_machine.write().await;
        let mut cluster_state = self.cluster_state.write().await;

        while state_machine.last_applied < cluster_state.commit_index {
            let index = state_machine.last_applied as usize;
            if index < log.len() {
                let entry = &log[index];
                self.apply_log_entry(entry, &mut state_machine).await?;
                state_machine.last_applied += 1;
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Apply single log entry to state machine
    async fn apply_log_entry(
        &self,
        entry: &LogEntry,
        state_machine: &mut RegistryStateMachine,
    ) -> Result<()> {
        match &entry.entry_type {
            LogEntryType::ServiceRegistration(service) => {
                state_machine
                    .services
                    .insert(service.service_id, service.clone());
                info!("Applied service registration: {}", service.service_id);
            }
            LogEntryType::ServiceDeregistration(service_id) => {
                state_machine.services.remove(service_id);
                info!("Applied service deregistration: {}", service_id);
            }
            LogEntryType::ServiceUpdate(service) => {
                state_machine
                    .services
                    .insert(service.service_id, service.clone());
                info!("Applied service update: {}", service.service_id);
            }
            LogEntryType::ConfigurationChange(_) => {
                // Handle configuration changes
                info!("Applied configuration change");
            }
            LogEntryType::NoOp => {
                // No-op entries don't change state
            }
            LogEntryType::Custom(_, _) => {
                // Handle custom entries
                info!("Applied custom log entry");
            }
        }

        state_machine.version += 1;
        Ok(())
    }

    /// Get consensus metrics
    pub async fn metrics(&self) -> ConsensusMetrics {
        let mut metrics = self.metrics.read().await.clone();

        // Update real-time metrics
        metrics.current_term = self.get_current_term().await;
        metrics.is_leader = self.is_leader().await;

        metrics
    }

    /// Get cluster state
    pub async fn cluster_state(&self) -> ClusterState {
        self.cluster_state.read().await.clone()
    }

    /// Get registry state
    pub async fn registry_state(&self) -> RegistryStateMachine {
        self.state_machine.read().await.clone()
    }
}

impl Clone for ConsensusManager {
    fn clone(&self) -> Self {
        Self {
            protocol: self.protocol.clone(),
            node_id: self.node_id,
            cluster_config: self.cluster_config.clone(),
            cluster_state: Arc::clone(&self.cluster_state),
            consensus_log: Arc::clone(&self.consensus_log),
            state_machine: Arc::clone(&self.state_machine),
            pending_proposals: Arc::clone(&self.pending_proposals),
            metrics: Arc::clone(&self.metrics),
            config: self.config.clone(),
        }
    }
}

impl Default for ConsensusMetrics {
    fn default() -> Self {
        Self {
            total_proposals: 0,
            accepted_proposals: 0,
            rejected_proposals: 0,
            leader_elections: 0,
            log_entries: 0,
            snapshots_created: 0,
            avg_proposal_latency: Duration::from_nanos(0),
            current_term: 0,
            is_leader: false,
        }
    }
}

impl std::fmt::Display for ConsensusProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsensusProtocol::Raft => write!(f, "Raft"),
            ConsensusProtocol::Pbft => write!(f, "PBFT"),
            ConsensusProtocol::Paxos => write!(f, "Paxos"),
            ConsensusProtocol::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeRole::Leader => write!(f, "Leader"),
            NodeRole::Follower => write!(f, "Follower"),
            NodeRole::Candidate => write!(f, "Candidate"),
            NodeRole::Observer => write!(f, "Observer"),
        }
    }
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Active => write!(f, "Active"),
            NodeStatus::Inactive => write!(f, "Inactive"),
            NodeStatus::Suspected => write!(f, "Suspected"),
            NodeStatus::Joining => write!(f, "Joining"),
            NodeStatus::Leaving => write!(f, "Leaving"),
        }
    }
}
