//! Leader Election System for High Availability
//!
//! This module implements a distributed leader election system using a Raft-inspired
//! consensus algorithm to ensure only one control plane instance is active at a time.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::patterns::events::{DomainEvent, EventBus};
use crate::error::Result;

/// Unique identifier for a control plane node in leader election
pub type NodeId = Uuid;

/// Current term number in the election process
pub type Term = u64;

/// Leader election configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderElectionConfig {
    /// Node ID for this control plane instance
    pub node_id: NodeId,
    /// Election timeout range in milliseconds (min, max)
    pub election_timeout_range: (u64, u64),
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval: u64,
    /// Maximum number of nodes in the cluster
    pub max_nodes: u32,
    /// Minimum nodes required for quorum
    pub min_quorum: u32,
}

impl Default for LeaderElectionConfig {
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4(),
            election_timeout_range: (150, 300),
            heartbeat_interval: 50,
            max_nodes: 5,
            min_quorum: 3,
        }
    }
}

/// Node state in the leader election process
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeState {
    /// Node is a follower
    Follower,
    /// Node is a candidate seeking election
    Candidate,
    /// Node is the elected leader
    Leader,
    /// Node is offline or unreachable
    Offline,
}

/// Leader election events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaderElectionEvent {
    /// Election started
    ElectionStarted {
        node_id: NodeId,
        term: Term,
        timestamp: DateTime<Utc>,
    },
    /// Vote cast for a candidate
    VoteCast {
        voter_id: NodeId,
        candidate_id: NodeId,
        term: Term,
        timestamp: DateTime<Utc>,
    },
    /// Leader elected
    LeaderElected {
        leader_id: NodeId,
        term: Term,
        votes_received: u32,
        timestamp: DateTime<Utc>,
    },
    /// Leader lost (timeout or failure)
    LeaderLost {
        leader_id: NodeId,
        term: Term,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Node state changed
    NodeStateChanged {
        node_id: NodeId,
        old_state: NodeState,
        new_state: NodeState,
        term: Term,
        timestamp: DateTime<Utc>,
    },
    /// Heartbeat received from leader
    HeartbeatReceived {
        leader_id: NodeId,
        term: Term,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent for LeaderElectionEvent {
    fn event_type(&self) -> &'static str {
        match self {
            LeaderElectionEvent::ElectionStarted { .. } => "leader_election.election_started",
            LeaderElectionEvent::VoteCast { .. } => "leader_election.vote_cast",
            LeaderElectionEvent::LeaderElected { .. } => "leader_election.leader_elected",
            LeaderElectionEvent::LeaderLost { .. } => "leader_election.leader_lost",
            LeaderElectionEvent::NodeStateChanged { .. } => "leader_election.node_state_changed",
            LeaderElectionEvent::HeartbeatReceived { .. } => "leader_election.heartbeat_received",
        }
    }

    fn aggregate_id(&self) -> Uuid {
        match self {
            LeaderElectionEvent::ElectionStarted { node_id, .. }
            | LeaderElectionEvent::VoteCast { voter_id: node_id, .. }
            | LeaderElectionEvent::LeaderElected { leader_id: node_id, .. }
            | LeaderElectionEvent::LeaderLost { leader_id: node_id, .. }
            | LeaderElectionEvent::NodeStateChanged { node_id, .. }
            | LeaderElectionEvent::HeartbeatReceived { leader_id: node_id, .. } => *node_id,
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            LeaderElectionEvent::ElectionStarted { timestamp, .. }
            | LeaderElectionEvent::VoteCast { timestamp, .. }
            | LeaderElectionEvent::LeaderElected { timestamp, .. }
            | LeaderElectionEvent::LeaderLost { timestamp, .. }
            | LeaderElectionEvent::NodeStateChanged { timestamp, .. }
            | LeaderElectionEvent::HeartbeatReceived { timestamp, .. } => *timestamp,
        }
    }

    fn correlation_id(&self) -> Uuid {
        self.aggregate_id()
    }
}

/// Information about a peer node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerNode {
    /// Node ID
    pub id: NodeId,
    /// Node address for communication
    pub address: String,
    /// Current state
    pub state: NodeState,
    /// Last known term
    pub term: Term,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
    /// Whether this node voted for us in current term
    pub voted_for_us: bool,
}

/// Leader election state
#[derive(Debug, Clone)]
pub struct ElectionState {
    /// Current term
    pub current_term: Term,
    /// Node we voted for in current term
    pub voted_for: Option<NodeId>,
    /// Current node state
    pub state: NodeState,
    /// Current leader (if known)
    pub current_leader: Option<NodeId>,
    /// Votes received in current election
    pub votes_received: u32,
    /// Last heartbeat from leader
    pub last_heartbeat: DateTime<Utc>,
    /// Election timeout timestamp
    pub election_timeout: DateTime<Utc>,
}

/// Leader Election Manager
pub struct LeaderElectionManager {
    /// Configuration
    config: LeaderElectionConfig,
    /// Current election state
    state: Arc<RwLock<ElectionState>>,
    /// Known peer nodes
    peers: Arc<RwLock<HashMap<NodeId, PeerNode>>>,
    /// Event bus for publishing events
    event_bus: Arc<EventBus>,
    /// Event channel for internal events
    event_sender: mpsc::UnboundedSender<LeaderElectionEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<LeaderElectionEvent>>>>,
    /// Election timer handle
    election_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Heartbeat timer handle
    heartbeat_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl LeaderElectionManager {
    /// Create a new leader election manager
    pub fn new(config: LeaderElectionConfig, event_bus: Arc<EventBus>) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            config: config.clone(),
            state: Arc::new(RwLock::new(ElectionState {
                current_term: 0,
                voted_for: None,
                state: NodeState::Follower,
                current_leader: None,
                votes_received: 0,
                last_heartbeat: Utc::now(),
                election_timeout: Utc::now() + chrono::Duration::milliseconds(
                    config.election_timeout_range.1 as i64,
                ),
            })),
            peers: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            election_timer: Arc::new(RwLock::new(None)),
            heartbeat_timer: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the leader election process
    pub async fn start(&self) -> Result<()> {
        info!(
            node_id = %self.config.node_id,
            "Starting leader election manager"
        );

        // Start event processing
        self.start_event_processing().await;

        // Start election timer
        self.start_election_timer().await;

        // Transition to follower state
        self.transition_to_follower(0).await?;

        info!(
            node_id = %self.config.node_id,
            "Leader election manager started"
        );

        Ok(())
    }

    /// Stop the leader election process
    pub async fn stop(&self) -> Result<()> {
        info!(
            node_id = %self.config.node_id,
            "Stopping leader election manager"
        );

        // Stop timers
        self.stop_election_timer().await;
        self.stop_heartbeat_timer().await;

        // Transition to offline state
        {
            let mut state = self.state.write().await;
            state.state = NodeState::Offline;
        }

        info!(
            node_id = %self.config.node_id,
            "Leader election manager stopped"
        );

        Ok(())
    }

    /// Add a peer node to the cluster
    pub async fn add_peer(&self, peer: PeerNode) -> Result<()> {
        info!(
            node_id = %self.config.node_id,
            peer_id = %peer.id,
            "Adding peer node"
        );

        let mut peers = self.peers.write().await;
        peers.insert(peer.id, peer);

        Ok(())
    }

    /// Remove a peer node from the cluster
    pub async fn remove_peer(&self, peer_id: NodeId) -> Result<()> {
        info!(
            node_id = %self.config.node_id,
            peer_id = %peer_id,
            "Removing peer node"
        );

        let mut peers = self.peers.write().await;
        peers.remove(&peer_id);

        Ok(())
    }

    /// Get current leader
    pub async fn get_current_leader(&self) -> Option<NodeId> {
        let state = self.state.read().await;
        state.current_leader
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        let state = self.state.read().await;
        state.state == NodeState::Leader
    }

    /// Get current node state
    pub async fn get_state(&self) -> NodeState {
        let state = self.state.read().await;
        state.state.clone()
    }

    /// Get current term
    pub async fn get_current_term(&self) -> Term {
        let state = self.state.read().await;
        state.current_term
    }

    /// Handle vote request from another node
    pub async fn handle_vote_request(
        &self,
        candidate_id: NodeId,
        term: Term,
    ) -> Result<bool> {
        let mut state = self.state.write().await;

        // If term is older, reject
        if term < state.current_term {
            debug!(
                node_id = %self.config.node_id,
                candidate_id = %candidate_id,
                candidate_term = term,
                current_term = state.current_term,
                "Rejecting vote request - older term"
            );
            return Ok(false);
        }

        // If term is newer, update our term and become follower
        if term > state.current_term {
            state.current_term = term;
            state.voted_for = None;
            state.state = NodeState::Follower;
            state.current_leader = None;
        }

        // Grant vote if we haven't voted or voted for this candidate
        let grant_vote = state.voted_for.is_none() || state.voted_for == Some(candidate_id);

        if grant_vote {
            state.voted_for = Some(candidate_id);
            
            // Publish vote cast event
            let event = LeaderElectionEvent::VoteCast {
                voter_id: self.config.node_id,
                candidate_id,
                term,
                timestamp: Utc::now(),
            };
            self.publish_event(event).await?;

            info!(
                node_id = %self.config.node_id,
                candidate_id = %candidate_id,
                term = term,
                "Granted vote to candidate"
            );
        } else {
            debug!(
                node_id = %self.config.node_id,
                candidate_id = %candidate_id,
                term = term,
                voted_for = ?state.voted_for,
                "Rejected vote request - already voted"
            );
        }

        Ok(grant_vote)
    }

    /// Handle heartbeat from leader
    pub async fn handle_heartbeat(&self, leader_id: NodeId, term: Term) -> Result<()> {
        let mut state = self.state.write().await;

        // If term is newer, update our term and become follower
        if term >= state.current_term {
            state.current_term = term;
            state.current_leader = Some(leader_id);
            state.last_heartbeat = Utc::now();
            state.election_timeout = Utc::now() + chrono::Duration::milliseconds(
                self.config.election_timeout_range.1 as i64,
            );

            // If we were a candidate, become follower
            if state.state == NodeState::Candidate {
                state.state = NodeState::Follower;
                state.voted_for = None;
            }

            // Publish heartbeat event
            let event = LeaderElectionEvent::HeartbeatReceived {
                leader_id,
                term,
                timestamp: Utc::now(),
            };
            drop(state); // Release lock before async call
            self.publish_event(event).await?;

            debug!(
                node_id = %self.config.node_id,
                leader_id = %leader_id,
                term = term,
                "Received heartbeat from leader"
            );
        }

        Ok(())
    }

    /// Start election process
    async fn start_election(&self) -> Result<()> {
        let mut state = self.state.write().await;

        // Increment term and vote for ourselves
        state.current_term += 1;
        state.voted_for = Some(self.config.node_id);
        state.state = NodeState::Candidate;
        state.current_leader = None;
        state.votes_received = 1; // Vote for ourselves
        state.last_heartbeat = Utc::now();

        let current_term = state.current_term;
        drop(state); // Release lock

        info!(
            node_id = %self.config.node_id,
            term = current_term,
            "Starting election"
        );

        // Publish election started event
        let event = LeaderElectionEvent::ElectionStarted {
            node_id: self.config.node_id,
            term: current_term,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        // Request votes from peers
        self.request_votes_from_peers(current_term).await?;

        Ok(())
    }

    /// Request votes from all peer nodes
    async fn request_votes_from_peers(&self, term: Term) -> Result<()> {
        let peers = self.peers.read().await;
        
        for peer in peers.values() {
            if peer.state != NodeState::Offline {
                // In a real implementation, this would send actual network requests
                // For now, we'll simulate the vote request
                debug!(
                    node_id = %self.config.node_id,
                    peer_id = %peer.id,
                    term = term,
                    "Requesting vote from peer"
                );
            }
        }

        Ok(())
    }

    /// Handle vote response from a peer
    pub async fn handle_vote_response(
        &self,
        peer_id: NodeId,
        term: Term,
        vote_granted: bool,
    ) -> Result<()> {
        let mut state = self.state.write().await;

        // Only process if we're still a candidate and term matches
        if state.state != NodeState::Candidate || term != state.current_term {
            return Ok(());
        }

        if vote_granted {
            state.votes_received += 1;
            
            debug!(
                node_id = %self.config.node_id,
                peer_id = %peer_id,
                term = term,
                votes_received = state.votes_received,
                "Received vote from peer"
            );

            // Check if we have majority
            let total_nodes = self.peers.read().await.len() + 1; // +1 for ourselves
            let majority = (total_nodes / 2) + 1;

            if state.votes_received >= majority as u32 {
                // We won the election!
                let votes_received = state.votes_received;
                drop(state); // Release lock
                
                self.transition_to_leader(term, votes_received).await?;
            }
        }

        Ok(())
    }

    /// Transition to follower state
    async fn transition_to_follower(&self, term: Term) -> Result<()> {
        let old_state = {
            let mut state = self.state.write().await;
            let old_state = state.state.clone();
            
            if term > state.current_term {
                state.current_term = term;
                state.voted_for = None;
            }
            
            state.state = NodeState::Follower;
            state.current_leader = None;
            state.votes_received = 0;
            
            old_state
        };

        // Stop heartbeat timer if we were leader
        if old_state == NodeState::Leader {
            self.stop_heartbeat_timer().await;
        }

        // Start election timer
        self.start_election_timer().await;

        // Publish state change event
        let event = LeaderElectionEvent::NodeStateChanged {
            node_id: self.config.node_id,
            old_state,
            new_state: NodeState::Follower,
            term,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        info!(
            node_id = %self.config.node_id,
            term = term,
            "Transitioned to follower state"
        );

        Ok(())
    }

    /// Transition to leader state
    async fn transition_to_leader(&self, term: Term, votes_received: u32) -> Result<()> {
        let old_state = {
            let mut state = self.state.write().await;
            let old_state = state.state.clone();
            
            state.state = NodeState::Leader;
            state.current_leader = Some(self.config.node_id);
            
            old_state
        };

        // Stop election timer
        self.stop_election_timer().await;

        // Start heartbeat timer
        self.start_heartbeat_timer().await;

        // Publish events
        let leader_elected_event = LeaderElectionEvent::LeaderElected {
            leader_id: self.config.node_id,
            term,
            votes_received,
            timestamp: Utc::now(),
        };
        self.publish_event(leader_elected_event).await?;

        let state_change_event = LeaderElectionEvent::NodeStateChanged {
            node_id: self.config.node_id,
            old_state,
            new_state: NodeState::Leader,
            term,
            timestamp: Utc::now(),
        };
        self.publish_event(state_change_event).await?;

        info!(
            node_id = %self.config.node_id,
            term = term,
            votes_received = votes_received,
            "Elected as leader"
        );

        Ok(())
    }

    /// Start election timer
    async fn start_election_timer(&self) {
        self.stop_election_timer().await;

        let _state = self.state.clone();
        let _event_sender = self.event_sender.clone();
        let node_id = self.config.node_id;
        let timeout_range = self.config.election_timeout_range;

        // Generate random timeout outside the async block
        let timeout_ms = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            rng.gen_range(timeout_range.0..=timeout_range.1)
        };

        let handle = tokio::spawn(async move {
            let timeout = Duration::from_millis(timeout_ms);
            tokio::time::sleep(timeout).await;

            debug!(
                node_id = %node_id,
                timeout_ms = timeout_ms,
                "Election timeout triggered"
            );

            // In a real implementation, this would trigger election
            // For now, we just log the timeout
        });

        let mut timer = self.election_timer.write().await;
        *timer = Some(handle);
    }

    /// Stop election timer
    async fn stop_election_timer(&self) {
        let mut timer = self.election_timer.write().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
    }

    /// Start heartbeat timer (for leaders)
    async fn start_heartbeat_timer(&self) {
        self.stop_heartbeat_timer().await;

        let peers = self.peers.clone();
        let state = self.state.clone();
        let node_id = self.config.node_id;
        let heartbeat_interval = self.config.heartbeat_interval;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(heartbeat_interval));

            loop {
                interval.tick().await;

                // Check if we're still leader
                let (is_leader, current_term) = {
                    let state_guard = state.read().await;
                    (state_guard.state == NodeState::Leader, state_guard.current_term)
                };

                if !is_leader {
                    break;
                }

                // Send heartbeats to all peers
                let peers_guard = peers.read().await;
                for peer in peers_guard.values() {
                    if peer.state != NodeState::Offline {
                        debug!(
                            leader_id = %node_id,
                            peer_id = %peer.id,
                            term = current_term,
                            "Sending heartbeat to peer"
                        );
                        // In a real implementation, send actual heartbeat message
                    }
                }
            }
        });

        let mut timer = self.heartbeat_timer.write().await;
        *timer = Some(handle);
    }

    /// Stop heartbeat timer
    async fn stop_heartbeat_timer(&self) {
        let mut timer = self.heartbeat_timer.write().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
    }

    /// Publish an event
    async fn publish_event(&self, event: LeaderElectionEvent) -> Result<()> {
        // Send to internal channel
        if self.event_sender.send(event.clone()).is_err() {
            warn!("Failed to send leader election event to internal channel");
        }

        // Publish to event bus
        self.event_bus.publish(event).await?;

        Ok(())
    }

    /// Start event processing task
    async fn start_event_processing(&self) {
        let event_receiver = self.event_receiver.clone();
        let node_id = self.config.node_id;

        tokio::spawn(async move {
            let receiver = {
                let mut guard = event_receiver.write().await;
                guard.take()
            };

            if let Some(mut rx) = receiver {
                while let Some(event) = rx.recv().await {
                    Self::process_election_event(node_id, event).await;
                }
            }
        });
    }

    /// Process leader election events
    async fn process_election_event(node_id: NodeId, event: LeaderElectionEvent) {
        match event {
            LeaderElectionEvent::ElectionStarted { term, timestamp, .. } => {
                info!(
                    node_id = %node_id,
                    term = term,
                    timestamp = %timestamp,
                    "Election started"
                );
            }
            LeaderElectionEvent::LeaderElected { leader_id, term, votes_received, timestamp } => {
                info!(
                    node_id = %node_id,
                    leader_id = %leader_id,
                    term = term,
                    votes_received = votes_received,
                    timestamp = %timestamp,
                    "Leader elected"
                );
            }
            LeaderElectionEvent::LeaderLost { leader_id, term, reason, timestamp } => {
                warn!(
                    node_id = %node_id,
                    leader_id = %leader_id,
                    term = term,
                    reason = %reason,
                    timestamp = %timestamp,
                    "Leader lost"
                );
            }
            _ => {
                debug!(
                    node_id = %node_id,
                    event = ?event,
                    "Leader election event processed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patterns::correlation::CorrelationTracker;

    #[tokio::test]
    async fn test_leader_election_creation() {
        let config = LeaderElectionConfig::default();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let election_manager = LeaderElectionManager::new(config.clone(), event_bus);
        
        let state = election_manager.get_state().await;
        assert_eq!(state, NodeState::Follower);
        
        let term = election_manager.get_current_term().await;
        assert_eq!(term, 0);
        
        assert!(!election_manager.is_leader().await);
    }

    #[tokio::test]
    async fn test_vote_request_handling() {
        let config = LeaderElectionConfig::default();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let election_manager = LeaderElectionManager::new(config.clone(), event_bus);
        
        let candidate_id = Uuid::new_v4();
        
        // Should grant vote for first request
        let vote_granted = election_manager.handle_vote_request(candidate_id, 1).await.unwrap();
        assert!(vote_granted);
        
        // Should reject vote for different candidate in same term
        let other_candidate = Uuid::new_v4();
        let vote_granted = election_manager.handle_vote_request(other_candidate, 1).await.unwrap();
        assert!(!vote_granted);
        
        // Should grant vote for same candidate in same term
        let vote_granted = election_manager.handle_vote_request(candidate_id, 1).await.unwrap();
        assert!(vote_granted);
    }

    #[tokio::test]
    async fn test_heartbeat_handling() {
        let config = LeaderElectionConfig::default();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let election_manager = LeaderElectionManager::new(config.clone(), event_bus);
        
        let leader_id = Uuid::new_v4();
        
        // Handle heartbeat
        election_manager.handle_heartbeat(leader_id, 1).await.unwrap();
        
        // Should update current leader
        let current_leader = election_manager.get_current_leader().await;
        assert_eq!(current_leader, Some(leader_id));
        
        // Should update term
        let term = election_manager.get_current_term().await;
        assert_eq!(term, 1);
    }
}