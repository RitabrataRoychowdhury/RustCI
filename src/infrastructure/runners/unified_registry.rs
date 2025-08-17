// Unified Runner Registry
// Task 3.3: Unified Runner System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use super::capability_detector::{
    RunnerCapabilityDetector, DetectedCapabilities, ProtocolSupport, PreferredProtocol
};

/// Unified registry supporting both HTTP and Valkyrie runners
pub struct UnifiedRunnerRegistry {
    // Runner storage
    runners: Arc<RwLock<HashMap<Uuid, RegisteredRunner>>>,
    
    // Protocol-specific indices
    valkyrie_runners: Arc<RwLock<HashMap<Uuid, ValkyrieRunnerInfo>>>,
    http_runners: Arc<RwLock<HashMap<Uuid, HttpRunnerInfo>>>,
    
    // Capability detector
    capability_detector: Arc<RunnerCapabilityDetector>,
    
    // Event broadcasting
    event_sender: broadcast::Sender<RegistryEvent>,
    
    // Configuration
    config: UnifiedRegistryConfig,
}

/// Configuration for unified registry
#[derive(Debug, Clone)]
pub struct UnifiedRegistryConfig {
    pub auto_capability_detection: bool,
    pub capability_refresh_interval: Duration,
    pub health_check_interval: Duration,
    pub runner_timeout: Duration,
    pub max_runners: usize,
    pub enable_protocol_migration: bool,
    pub preferred_protocol_bias: f64,
}

/// Registered runner information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredRunner {
    pub id: Uuid,
    pub endpoint: String,
    pub runner_type: RunnerType,
    pub capabilities: DetectedCapabilities,
    pub status: RunnerStatus,
    pub metadata: RunnerMetadata,
    pub registration_time: std::time::SystemTime,
    pub last_seen: std::time::SystemTime,
    pub performance_metrics: PerformanceMetrics,
}

/// Runner type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerType {
    Valkyrie {
        version: String,
        features: Vec<String>,
    },
    Http {
        version: String,
        features: Vec<String>,
    },
    Hybrid {
        primary_protocol: PreferredProtocol,
        fallback_protocols: Vec<PreferredProtocol>,
    },
    Unknown,
}

/// Runner status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerStatus {
    Online,
    Offline,
    Busy,
    Draining,
    Maintenance,
    Error(String),
}

/// Runner metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerMetadata {
    pub name: Option<String>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub owner: Option<String>,
    pub environment: Option<String>,
    pub region: Option<String>,
    pub zone: Option<String>,
}

/// Performance metrics for runners
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub average_job_duration: Option<Duration>,
    pub success_rate: f64,
    pub last_job_completion: Option<std::time::SystemTime>,
    pub resource_utilization: ResourceUtilization,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub storage_usage: f64,
    pub network_usage: f64,
}

/// Valkyrie-specific runner information
#[derive(Debug, Clone)]
pub struct ValkyrieRunnerInfo {
    pub runner_id: Uuid,
    pub valkyrie_node_id: Uuid,
    pub connection_info: ValkyrieConnectionInfo,
    pub performance_tier: super::capability_detector::PerformanceTier,
    pub features: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ValkyrieConnectionInfo {
    pub endpoint: String,
    pub protocol_version: String,
    pub connection_pool_size: u32,
    pub last_ping: Option<Duration>,
    pub throughput_benchmark: Option<u64>,
}

/// HTTP-specific runner information
#[derive(Debug, Clone)]
pub struct HttpRunnerInfo {
    pub runner_id: Uuid,
    pub http_config: HttpConnectionConfig,
    pub api_version: String,
    pub supported_features: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HttpConnectionConfig {
    pub base_url: String,
    pub timeout: Duration,
    pub max_concurrent_requests: u32,
    pub keep_alive: bool,
    pub compression: bool,
    pub authentication: Option<HttpAuthentication>,
}

#[derive(Debug, Clone)]
pub enum HttpAuthentication {
    Bearer(String),
    ApiKey { header: String, value: String },
    Basic { username: String, password: String },
}

/// Registry events
#[derive(Debug, Clone)]
pub enum RegistryEvent {
    RunnerRegistered {
        runner_id: Uuid,
        runner_type: RunnerType,
        endpoint: String,
    },
    RunnerDeregistered {
        runner_id: Uuid,
        reason: String,
    },
    RunnerStatusChanged {
        runner_id: Uuid,
        old_status: RunnerStatus,
        new_status: RunnerStatus,
    },
    CapabilitiesUpdated {
        runner_id: Uuid,
        capabilities: DetectedCapabilities,
    },
    ProtocolMigrated {
        runner_id: Uuid,
        from_protocol: PreferredProtocol,
        to_protocol: PreferredProtocol,
    },
}

impl Default for UnifiedRegistryConfig {
    fn default() -> Self {
        Self {
            auto_capability_detection: true,
            capability_refresh_interval: Duration::from_secs(300), // 5 minutes
            health_check_interval: Duration::from_secs(30),
            runner_timeout: Duration::from_secs(120),
            max_runners: 10000,
            enable_protocol_migration: true,
            preferred_protocol_bias: 0.2, // 20% bias towards preferred protocol
        }
    }
}

impl UnifiedRunnerRegistry {
    /// Create a new unified runner registry
    pub async fn new(
        capability_detector: Arc<RunnerCapabilityDetector>,
        config: UnifiedRegistryConfig,
    ) -> Result<Self> {
        let (event_sender, _) = broadcast::channel(1000);
        
        let registry = Self {
            runners: Arc::new(RwLock::new(HashMap::new())),
            valkyrie_runners: Arc::new(RwLock::new(HashMap::new())),
            http_runners: Arc::new(RwLock::new(HashMap::new())),
            capability_detector,
            event_sender,
            config,
        };
        
        // Start background tasks
        registry.start_background_tasks().await;
        
        Ok(registry)
    }
    
    /// Register a new runner
    pub async fn register_runner(
        &self,
        endpoint: String,
        metadata: RunnerMetadata,
    ) -> Result<Uuid> {
        let runner_id = Uuid::new_v4();
        
        // Detect capabilities if auto-detection is enabled
        let capabilities = if self.config.auto_capability_detection {
            self.capability_detector.detect_capabilities(&endpoint).await?
        } else {
            // Use default capabilities
            DetectedCapabilities::default()
        };
        
        // Determine runner type based on capabilities
        let runner_type = self.determine_runner_type(&capabilities);
        
        // Create registered runner
        let registered_runner = RegisteredRunner {
            id: runner_id,
            endpoint: endpoint.clone(),
            runner_type: runner_type.clone(),
            capabilities: capabilities.clone(),
            status: RunnerStatus::Online,
            metadata,
            registration_time: std::time::SystemTime::now(),
            last_seen: std::time::SystemTime::now(),
            performance_metrics: PerformanceMetrics::default(),
        };
        
        // Store in main registry
        {
            let mut runners = self.runners.write().await;
            if runners.len() >= self.config.max_runners {
                return Err(AppError::ValidationError(
                    "Maximum number of runners reached".to_string()
                ));
            }
            runners.insert(runner_id, registered_runner);
        }
        
        // Store in protocol-specific indices
        self.update_protocol_indices(runner_id, &runner_type, &endpoint, &capabilities).await?;
        
        // Emit registration event
        let _ = self.event_sender.send(RegistryEvent::RunnerRegistered {
            runner_id,
            runner_type,
            endpoint,
        });
        
        Ok(runner_id)
    }
    
    /// Deregister a runner
    pub async fn deregister_runner(&self, runner_id: Uuid, reason: String) -> Result<()> {
        // Remove from main registry
        let removed_runner = {
            let mut runners = self.runners.write().await;
            runners.remove(&runner_id)
        };
        
        if removed_runner.is_none() {
            return Err(AppError::NotFound(format!("Runner {} not found", runner_id)));
        }
        
        // Remove from protocol-specific indices
        {
            let mut valkyrie_runners = self.valkyrie_runners.write().await;
            valkyrie_runners.remove(&runner_id);
        }
        
        {
            let mut http_runners = self.http_runners.write().await;
            http_runners.remove(&runner_id);
        }
        
        // Emit deregistration event
        let _ = self.event_sender.send(RegistryEvent::RunnerDeregistered {
            runner_id,
            reason,
        });
        
        Ok(())
    }
    
    /// Get runner by ID
    pub async fn get_runner(&self, runner_id: Uuid) -> Option<RegisteredRunner> {
        let runners = self.runners.read().await;
        runners.get(&runner_id).cloned()
    }
    
    /// List all runners
    pub async fn list_runners(&self) -> Vec<RegisteredRunner> {
        let runners = self.runners.read().await;
        runners.values().cloned().collect()
    }
    
    /// List runners by type
    pub async fn list_runners_by_type(&self, runner_type: &RunnerType) -> Vec<RegisteredRunner> {
        let runners = self.runners.read().await;
        runners.values()
            .filter(|r| std::mem::discriminant(&r.runner_type) == std::mem::discriminant(runner_type))
            .cloned()
            .collect()
    }
    
    /// List runners by status
    pub async fn list_runners_by_status(&self, status: &RunnerStatus) -> Vec<RegisteredRunner> {
        let runners = self.runners.read().await;
        runners.values()
            .filter(|r| &r.status == status)
            .cloned()
            .collect()
    }
    
    /// Update runner status
    pub async fn update_runner_status(
        &self,
        runner_id: Uuid,
        new_status: RunnerStatus,
    ) -> Result<()> {
        let old_status = {
            let mut runners = self.runners.write().await;
            let runner = runners.get_mut(&runner_id)
                .ok_or_else(|| AppError::NotFound(format!("Runner {} not found", runner_id)))?;
            
            let old_status = runner.status.clone();
            runner.status = new_status.clone();
            runner.last_seen = std::time::SystemTime::now();
            old_status
        };
        
        // Emit status change event
        let _ = self.event_sender.send(RegistryEvent::RunnerStatusChanged {
            runner_id,
            old_status,
            new_status,
        });
        
        Ok(())
    }
    
    /// Update runner capabilities
    pub async fn update_runner_capabilities(
        &self,
        runner_id: Uuid,
        capabilities: DetectedCapabilities,
    ) -> Result<()> {
        let (endpoint, old_runner_type) = {
            let mut runners = self.runners.write().await;
            let runner = runners.get_mut(&runner_id)
                .ok_or_else(|| AppError::NotFound(format!("Runner {} not found for capability update", runner_id)))?;
            
            let endpoint = runner.endpoint.clone();
            let old_runner_type = runner.runner_type.clone();
            
            runner.capabilities = capabilities.clone();
            runner.runner_type = self.determine_runner_type(&capabilities);
            runner.last_seen = std::time::SystemTime::now();
            
            (endpoint, old_runner_type)
        };
        
        // Update protocol-specific indices
        let new_runner_type = self.determine_runner_type(&capabilities);
        if old_runner_type != new_runner_type {
            self.update_protocol_indices(runner_id, &new_runner_type, &endpoint, &capabilities).await?;
        }
        
        // Emit capabilities update event
        let _ = self.event_sender.send(RegistryEvent::CapabilitiesUpdated {
            runner_id,
            capabilities,
        });
        
        Ok(())
    }
    
    /// Get Valkyrie runners
    pub async fn get_valkyrie_runners(&self) -> Vec<ValkyrieRunnerInfo> {
        let valkyrie_runners = self.valkyrie_runners.read().await;
        valkyrie_runners.values().cloned().collect()
    }
    
    /// Get HTTP runners
    pub async fn get_http_runners(&self) -> Vec<HttpRunnerInfo> {
        let http_runners = self.http_runners.read().await;
        http_runners.values().cloned().collect()
    }
    
    /// Find optimal runner for a job
    pub async fn find_optimal_runner(
        &self,
        requirements: &JobRequirements,
    ) -> Option<Uuid> {
        let runners = self.runners.read().await;
        
        let mut candidates: Vec<_> = runners.values()
            .filter(|r| r.status == RunnerStatus::Online)
            .filter(|r| self.matches_requirements(r, requirements))
            .collect();
        
        if candidates.is_empty() {
            return None;
        }
        
        // Sort by preference score
        candidates.sort_by(|a, b| {
            let score_a = self.calculate_preference_score(a, requirements);
            let score_b = self.calculate_preference_score(b, requirements);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        candidates.first().map(|r| r.id)
    }
    
    /// Subscribe to registry events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<RegistryEvent> {
        self.event_sender.subscribe()
    }
    
    /// Get registry statistics
    pub async fn get_statistics(&self) -> RegistryStatistics {
        let runners = self.runners.read().await;
        let valkyrie_runners = self.valkyrie_runners.read().await;
        let http_runners = self.http_runners.read().await;
        
        let total_runners = runners.len();
        let online_runners = runners.values()
            .filter(|r| r.status == RunnerStatus::Online)
            .count();
        let busy_runners = runners.values()
            .filter(|r| r.status == RunnerStatus::Busy)
            .count();
        
        let valkyrie_count = valkyrie_runners.len();
        let http_count = http_runners.len();
        let hybrid_count = runners.values()
            .filter(|r| matches!(r.runner_type, RunnerType::Hybrid { .. }))
            .count();
        
        RegistryStatistics {
            total_runners,
            online_runners,
            busy_runners,
            offline_runners: total_runners - online_runners - busy_runners,
            valkyrie_runners: valkyrie_count,
            http_runners: http_count,
            hybrid_runners: hybrid_count,
            average_success_rate: runners.values()
                .map(|r| r.performance_metrics.success_rate)
                .sum::<f64>() / total_runners as f64,
        }
    }
    
    // Private helper methods
    
    /// Determine runner type based on capabilities
    fn determine_runner_type(&self, capabilities: &DetectedCapabilities) -> RunnerType {
        let protocol = &capabilities.protocol_support;
        
        match (&protocol.valkyrie.supported, &protocol.http.supported) {
            (true, true) => {
                // Hybrid runner - determine primary protocol
                let primary = match protocol.preferred_protocol {
                    PreferredProtocol::Valkyrie => PreferredProtocol::Valkyrie,
                    PreferredProtocol::Http => PreferredProtocol::Http,
                    _ => {
                        // Auto-determine based on performance
                        if matches!(protocol.valkyrie.performance_tier, 
                                   super::capability_detector::PerformanceTier::Ultra | 
                                   super::capability_detector::PerformanceTier::High) {
                            PreferredProtocol::Valkyrie
                        } else {
                            PreferredProtocol::Http
                        }
                    }
                };
                
                RunnerType::Hybrid {
                    primary_protocol: primary,
                    fallback_protocols: vec![PreferredProtocol::Http, PreferredProtocol::Valkyrie],
                }
            }
            (true, false) => RunnerType::Valkyrie {
                version: protocol.valkyrie.version.clone().unwrap_or_default(),
                features: protocol.valkyrie.features.clone(),
            },
            (false, true) => RunnerType::Http {
                version: protocol.http.version.clone(),
                features: protocol.http.features.clone(),
            },
            (false, false) => RunnerType::Unknown,
        }
    }
    
    /// Update protocol-specific indices
    async fn update_protocol_indices(
        &self,
        runner_id: Uuid,
        runner_type: &RunnerType,
        endpoint: &str,
        capabilities: &DetectedCapabilities,
    ) -> Result<()> {
        match runner_type {
            RunnerType::Valkyrie { version, features } => {
                let valkyrie_info = ValkyrieRunnerInfo {
                    runner_id,
                    valkyrie_node_id: Uuid::new_v4(), // Would be actual node ID
                    connection_info: ValkyrieConnectionInfo {
                        endpoint: endpoint.to_string(),
                        protocol_version: version.clone(),
                        connection_pool_size: 10,
                        last_ping: None,
                        throughput_benchmark: capabilities.protocol_support.valkyrie.throughput_benchmark,
                    },
                    performance_tier: capabilities.protocol_support.valkyrie.performance_tier.clone(),
                    features: features.clone(),
                };
                
                let mut valkyrie_runners = self.valkyrie_runners.write().await;
                valkyrie_runners.insert(runner_id, valkyrie_info);
            }
            RunnerType::Http { version, features } => {
                let http_info = HttpRunnerInfo {
                    runner_id,
                    http_config: HttpConnectionConfig {
                        base_url: format!("http://{}", endpoint),
                        timeout: Duration::from_secs(30),
                        max_concurrent_requests: capabilities.protocol_support.http.max_concurrent_requests.unwrap_or(10),
                        keep_alive: capabilities.protocol_support.http.keep_alive_support,
                        compression: !capabilities.protocol_support.http.compression_support.is_empty(),
                        authentication: None, // Would be configured
                    },
                    api_version: version.clone(),
                    supported_features: features.clone(),
                };
                
                let mut http_runners = self.http_runners.write().await;
                http_runners.insert(runner_id, http_info);
            }
            RunnerType::Hybrid { primary_protocol, .. } => {
                // Add to both indices
                match primary_protocol {
                    PreferredProtocol::Valkyrie => {
                        // Add to Valkyrie index as primary
                        let valkyrie_info = ValkyrieRunnerInfo {
                            runner_id,
                            valkyrie_node_id: Uuid::new_v4(),
                            connection_info: ValkyrieConnectionInfo {
                                endpoint: endpoint.to_string(),
                                protocol_version: capabilities.protocol_support.valkyrie.version.clone().unwrap_or_default(),
                                connection_pool_size: 10,
                                last_ping: None,
                                throughput_benchmark: capabilities.protocol_support.valkyrie.throughput_benchmark,
                            },
                            performance_tier: capabilities.protocol_support.valkyrie.performance_tier.clone(),
                            features: capabilities.protocol_support.valkyrie.features.clone(),
                        };
                        
                        let mut valkyrie_runners = self.valkyrie_runners.write().await;
                        valkyrie_runners.insert(runner_id, valkyrie_info);
                    }
                    _ => {
                        // Add to HTTP index as primary
                        let http_info = HttpRunnerInfo {
                            runner_id,
                            http_config: HttpConnectionConfig {
                                base_url: format!("http://{}", endpoint),
                                timeout: Duration::from_secs(30),
                                max_concurrent_requests: capabilities.protocol_support.http.max_concurrent_requests.unwrap_or(10),
                                keep_alive: capabilities.protocol_support.http.keep_alive_support,
                                compression: !capabilities.protocol_support.http.compression_support.is_empty(),
                                authentication: None,
                            },
                            api_version: capabilities.protocol_support.http.version.clone(),
                            supported_features: capabilities.protocol_support.http.features.clone(),
                        };
                        
                        let mut http_runners = self.http_runners.write().await;
                        http_runners.insert(runner_id, http_info);
                    }
                }
            }
            RunnerType::Unknown => {
                // Don't add to protocol-specific indices
            }
        }
        
        Ok(())
    }
    
    /// Check if runner matches job requirements
    fn matches_requirements(&self, runner: &RegisteredRunner, requirements: &JobRequirements) -> bool {
        let caps = &runner.capabilities;
        
        // Check hardware requirements
        if let Some(required_cpu) = requirements.cpu_cores {
            if caps.hardware.cpu.cores < required_cpu {
                return false;
            }
        }
        
        if let Some(required_memory) = requirements.memory_gb {
            let available_memory_gb = caps.hardware.memory.available / (1024 * 1024 * 1024);
            if available_memory_gb < required_memory as u64 {
                return false;
            }
        }
        
        if let Some(required_storage) = requirements.storage_gb {
            let available_storage_gb = caps.hardware.storage.available / (1024 * 1024 * 1024);
            if available_storage_gb < required_storage as u64 {
                return false;
            }
        }
        
        // Check software requirements
        for required_lang in &requirements.required_languages {
            if !caps.software.programming_languages.iter()
                .any(|lang| lang.name.to_lowercase() == required_lang.to_lowercase()) {
                return false;
            }
        }
        
        for required_tool in &requirements.required_tools {
            let has_tool = caps.software.build_tools.iter()
                .any(|tool| tool.name.to_lowercase() == required_tool.to_lowercase()) ||
                caps.software.testing_frameworks.iter()
                .any(|framework| framework.name.to_lowercase() == required_tool.to_lowercase()) ||
                caps.software.deployment_tools.iter()
                .any(|tool| tool.name.to_lowercase() == required_tool.to_lowercase());
            
            if !has_tool {
                return false;
            }
        }
        
        true
    }
    
    /// Calculate preference score for runner selection
    fn calculate_preference_score(&self, runner: &RegisteredRunner, requirements: &JobRequirements) -> f64 {
        let mut score = 0.0;
        
        // Protocol preference score
        match &runner.runner_type {
            RunnerType::Valkyrie { .. } => {
                score += 1.0 + self.config.preferred_protocol_bias;
            }
            RunnerType::Http { .. } => {
                score += 0.8;
            }
            RunnerType::Hybrid { primary_protocol, .. } => {
                match primary_protocol {
                    PreferredProtocol::Valkyrie => score += 0.9 + self.config.preferred_protocol_bias,
                    _ => score += 0.7,
                }
            }
            RunnerType::Unknown => {
                score += 0.1;
            }
        }
        
        // Performance score
        score += runner.performance_metrics.success_rate * 0.5;
        
        // Resource utilization score (prefer less utilized runners)
        let utilization = &runner.performance_metrics.resource_utilization;
        let avg_utilization = (utilization.cpu_usage + utilization.memory_usage) / 2.0;
        score += (1.0 - avg_utilization) * 0.3;
        
        // Geographic preference (if specified)
        if let Some(preferred_region) = &requirements.preferred_region {
            if let Some(runner_region) = &runner.metadata.region {
                if runner_region == preferred_region {
                    score += 0.2;
                }
            }
        }
        
        score
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Health check task
        let registry_clone = Arc::new(self.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(registry_clone.config.health_check_interval);
            loop {
                interval.tick().await;
                registry_clone.perform_health_checks().await;
            }
        });
        
        // Capability refresh task
        let registry_clone = Arc::new(self.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(registry_clone.config.capability_refresh_interval);
            loop {
                interval.tick().await;
                registry_clone.refresh_capabilities().await;
            }
        });
    }
    
    /// Perform health checks on all runners
    async fn perform_health_checks(&self) {
        let runners: Vec<_> = {
            let runners = self.runners.read().await;
            runners.values().cloned().collect()
        };
        
        for runner in runners {
            if runner.status == RunnerStatus::Online {
                let is_healthy = self.capability_detector
                    .verify_capabilities(&runner.endpoint)
                    .await
                    .unwrap_or(false);
                
                if !is_healthy {
                    let _ = self.update_runner_status(
                        runner.id,
                        RunnerStatus::Error("Health check failed".to_string())
                    ).await;
                }
            }
        }
    }
    
    /// Refresh capabilities for all runners
    async fn refresh_capabilities(&self) {
        let runners: Vec<_> = {
            let runners = self.runners.read().await;
            runners.values().cloned().collect()
        };
        
        for runner in runners {
            if runner.status == RunnerStatus::Online {
                if let Ok(capabilities) = self.capability_detector
                    .detect_capabilities(&runner.endpoint)
                    .await {
                    let _ = self.update_runner_capabilities(runner.id, capabilities).await;
                }
            }
        }
    }
}

impl Clone for UnifiedRunnerRegistry {
    fn clone(&self) -> Self {
        Self {
            runners: self.runners.clone(),
            valkyrie_runners: self.valkyrie_runners.clone(),
            http_runners: self.http_runners.clone(),
            capability_detector: self.capability_detector.clone(),
            event_sender: self.event_sender.clone(),
            config: self.config.clone(),
        }
    }
}

/// Job requirements for runner matching
#[derive(Debug, Clone, Default)]
pub struct JobRequirements {
    pub cpu_cores: Option<u32>,
    pub memory_gb: Option<u32>,
    pub storage_gb: Option<u32>,
    pub gpu_count: Option<u32>,
    pub required_languages: Vec<String>,
    pub required_tools: Vec<String>,
    pub preferred_region: Option<String>,
    pub max_execution_time: Option<Duration>,
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStatistics {
    pub total_runners: usize,
    pub online_runners: usize,
    pub busy_runners: usize,
    pub offline_runners: usize,
    pub valkyrie_runners: usize,
    pub http_runners: usize,
    pub hybrid_runners: usize,
    pub average_success_rate: f64,
}

impl Default for DetectedCapabilities {
    fn default() -> Self {
        Self {
            protocol_support: ProtocolSupport {
                valkyrie: super::capability_detector::ValkyrieProtocolSupport {
                    supported: false,
                    version: None,
                    features: Vec::new(),
                    performance_tier: super::capability_detector::PerformanceTier::Basic,
                    latency_benchmark: None,
                    throughput_benchmark: None,
                },
                http: super::capability_detector::HttpProtocolSupport {
                    supported: false,
                    version: "HTTP/1.1".to_string(),
                    features: Vec::new(),
                    max_concurrent_requests: None,
                    keep_alive_support: false,
                    compression_support: Vec::new(),
                },
                websocket: super::capability_detector::WebSocketSupport {
                    supported: false,
                    version: None,
                    extensions: Vec::new(),
                    max_message_size: None,
                },
                grpc: super::capability_detector::GrpcSupport {
                    supported: false,
                    version: None,
                    streaming_support: false,
                    compression: Vec::new(),
                },
                preferred_protocol: super::capability_detector::PreferredProtocol::Auto,
            },
            hardware: super::capability_detector::HardwareCapabilities {
                cpu: super::capability_detector::CpuCapabilities {
                    cores: 1,
                    threads: 1,
                    architecture: "unknown".to_string(),
                    features: Vec::new(),
                    frequency: None,
                    cache_sizes: Vec::new(),
                },
                memory: super::capability_detector::MemoryCapabilities {
                    total: 0,
                    available: 0,
                    memory_type: "unknown".to_string(),
                    bandwidth: None,
                },
                storage: super::capability_detector::StorageCapabilities {
                    total: 0,
                    available: 0,
                    storage_type: "unknown".to_string(),
                    read_speed: None,
                    write_speed: None,
                    iops: None,
                },
                gpu: None,
                network: super::capability_detector::NetworkHardware {
                    interfaces: Vec::new(),
                    bandwidth: None,
                    latency: None,
                },
            },
            software: super::capability_detector::SoftwareCapabilities {
                operating_system: super::capability_detector::OperatingSystem {
                    name: "unknown".to_string(),
                    version: "unknown".to_string(),
                    distribution: None,
                    kernel_version: None,
                },
                container_runtime: Vec::new(),
                programming_languages: Vec::new(),
                build_tools: Vec::new(),
                testing_frameworks: Vec::new(),
                deployment_tools: Vec::new(),
            },
            performance: super::capability_detector::PerformanceCapabilities {
                benchmark_results: super::capability_detector::BenchmarkResults {
                    cpu_benchmark: None,
                    memory_benchmark: None,
                    storage_benchmark: None,
                    network_benchmark: None,
                    job_execution_benchmark: None,
                },
                resource_limits: super::capability_detector::ResourceLimits {
                    max_concurrent_jobs: 1,
                    max_memory_per_job: None,
                    max_cpu_per_job: None,
                    max_execution_time: None,
                },
                scaling_characteristics: super::capability_detector::ScalingCharacteristics {
                    supports_horizontal_scaling: false,
                    supports_vertical_scaling: false,
                    auto_scaling_enabled: false,
                    scaling_metrics: Vec::new(),
                },
            },
            network: super::capability_detector::NetworkCapabilities {
                connectivity: super::capability_detector::ConnectivityInfo {
                    public_ip: None,
                    private_ip: None,
                    hostname: "unknown".to_string(),
                    ports: Vec::new(),
                    firewall_rules: Vec::new(),
                },
                security: super::capability_detector::NetworkSecurity {
                    tls_support: super::capability_detector::TlsSupport {
                        versions: Vec::new(),
                        cipher_suites: Vec::new(),
                        certificate_validation: false,
                    },
                    vpn_support: false,
                    firewall_enabled: false,
                    intrusion_detection: false,
                },
                quality_of_service: super::capability_detector::QosCapabilities {
                    bandwidth_control: false,
                    priority_queuing: false,
                    traffic_shaping: false,
                    latency_guarantees: false,
                },
            },
            security: super::capability_detector::SecurityCapabilities {
                authentication: super::capability_detector::AuthenticationCapabilities {
                    methods: Vec::new(),
                    multi_factor: false,
                    certificate_auth: false,
                    api_key_auth: false,
                },
                authorization: super::capability_detector::AuthorizationCapabilities {
                    rbac_support: false,
                    abac_support: false,
                    policy_engine: None,
                    fine_grained_permissions: false,
                },
                encryption: super::capability_detector::EncryptionCapabilities {
                    at_rest: false,
                    in_transit: false,
                    key_management: Vec::new(),
                    algorithms: Vec::new(),
                },
                compliance: super::capability_detector::ComplianceCapabilities {
                    standards: Vec::new(),
                    audit_logging: false,
                    data_residency: false,
                    privacy_controls: false,
                },
            },
            custom: HashMap::new(),
        }
    }
}
