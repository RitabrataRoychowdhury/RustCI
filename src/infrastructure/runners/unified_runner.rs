//! Unified Runner Abstraction for Backward Compatibility
//!
//! This module provides a unified interface that maintains backward compatibility
//! with existing Docker and Kubernetes runners while supporting the new native runner.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::domain::entities::{
    HealthStatus, Job, JobResult, Runner, RunnerCapacity, RunnerId, RunnerMetadata,
};
use crate::error::Result;
use crate::infrastructure::runners::{
    docker_runner::{DockerRunner, DockerRunnerConfig},
    kubernetes_runner::{KubernetesRunner, KubernetesRunnerConfig},
    native_runner::{NativeProcessRunner, NativeRunnerConfig},
};

/// Unified runner configuration supporting all runner types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedRunnerConfig {
    /// Runner name
    pub name: String,
    /// Runner type and specific configuration
    pub runner_type: UnifiedRunnerType,
    /// Feature flags for backward compatibility
    pub compatibility_flags: CompatibilityFlags,
    /// Control plane integration settings
    pub control_plane_integration: ControlPlaneIntegration,
}

/// Unified runner type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnifiedRunnerType {
    /// Docker runner configuration
    Docker(DockerRunnerConfig),
    /// Kubernetes runner configuration
    Kubernetes(KubernetesRunnerConfig),
    /// Native runner configuration
    Native(NativeRunnerConfig),
}

/// Compatibility flags for safe rollout and rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityFlags {
    /// Enable legacy mode for existing deployments
    pub legacy_mode: bool,
    /// Enable runner compatibility mode
    pub runner_compat: bool,
    /// Enable hybrid deployments (native + legacy runners)
    pub hybrid_deployment: bool,
    /// Enable feature flags for gradual rollout
    pub feature_flags: HashMap<String, bool>,
}

/// Control plane integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneIntegration {
    /// Enable control plane integration
    pub enabled: bool,
    /// Control plane endpoint
    pub endpoint: Option<String>,
    /// Authentication configuration
    pub auth_config: Option<AuthConfig>,
    /// Registration settings
    pub registration: RegistrationConfig,
}

/// Authentication configuration for control plane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication method
    pub method: AuthMethod,
    /// Credentials or token
    pub credentials: String,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// JWT token authentication
    Jwt,
    /// Mutual TLS authentication
    MutualTls,
    /// API key authentication
    ApiKey,
}

/// Registration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationConfig {
    /// Auto-register with control plane
    pub auto_register: bool,
    /// Registration retry attempts
    pub retry_attempts: u32,
    /// Registration timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for CompatibilityFlags {
    fn default() -> Self {
        Self {
            legacy_mode: false,
            runner_compat: true,
            hybrid_deployment: true,
            feature_flags: HashMap::new(),
        }
    }
}

impl Default for ControlPlaneIntegration {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            auth_config: None,
            registration: RegistrationConfig::default(),
        }
    }
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            auto_register: true,
            retry_attempts: 3,
            timeout_seconds: 30,
        }
    }
}

/// Unified runner implementation that wraps different runner types
pub struct UnifiedRunner {
    /// Runner ID
    id: RunnerId,
    /// Configuration
    config: UnifiedRunnerConfig,
    /// Actual runner implementation
    inner: Box<dyn Runner>,
    /// Compatibility mode flag
    compatibility_mode: bool,
}

impl UnifiedRunner {
    /// Create a new unified runner
    pub async fn new(
        config: UnifiedRunnerConfig,
        event_demux: Arc<crate::core::infrastructure::event_loop::EventDemultiplexer>,
    ) -> Result<Self> {
        let id = Uuid::new_v4();
        let compatibility_mode = config.compatibility_flags.legacy_mode
            || config.compatibility_flags.runner_compat;

        // Create the appropriate runner based on configuration
        let inner: Box<dyn Runner> = match &config.runner_type {
            UnifiedRunnerType::Docker(docker_config) => {
                info!("Creating Docker runner with unified interface");
                let runner = DockerRunner::new(docker_config.clone(), event_demux).await?;
                Box::new(runner)
            }
            UnifiedRunnerType::Kubernetes(k8s_config) => {
                info!("Creating Kubernetes runner with unified interface");
                let runner = KubernetesRunner::new(k8s_config.clone(), event_demux).await?;
                Box::new(runner)
            }
            UnifiedRunnerType::Native(native_config) => {
                info!("Creating Native runner with unified interface");
                let runner = NativeProcessRunner::new(native_config.clone(), event_demux).await?;
                Box::new(runner)
            }
        };

        let unified_runner = Self {
            id,
            config,
            inner,
            compatibility_mode,
        };

        // Register with control plane if enabled
        if unified_runner.config.control_plane_integration.enabled {
            unified_runner.register_with_control_plane().await?;
        }

        Ok(unified_runner)
    }

    /// Register with control plane
    async fn register_with_control_plane(&self) -> Result<()> {
        if !self.config.control_plane_integration.enabled {
            return Ok(());
        }

        info!("Registering unified runner {} with control plane", self.id);

        // In a real implementation, this would make HTTP calls to the control plane
        // For now, we'll just log the registration
        debug!(
            "Runner {} registered with control plane (compatibility_mode: {})",
            self.id, self.compatibility_mode
        );

        Ok(())
    }

    /// Check if runner is in compatibility mode
    pub fn is_compatibility_mode(&self) -> bool {
        self.compatibility_mode
    }

    /// Get compatibility flags
    pub fn get_compatibility_flags(&self) -> &CompatibilityFlags {
        &self.config.compatibility_flags
    }

    /// Enable/disable feature flag
    pub fn set_feature_flag(&mut self, flag: String, enabled: bool) {
        self.config
            .compatibility_flags
            .feature_flags
            .insert(flag, enabled);
    }

    /// Check if feature flag is enabled
    pub fn is_feature_enabled(&self, flag: &str) -> bool {
        self.config
            .compatibility_flags
            .feature_flags
            .get(flag)
            .copied()
            .unwrap_or(false)
    }

    /// Get runner type string for compatibility
    pub fn get_runner_type_string(&self) -> String {
        match &self.config.runner_type {
            UnifiedRunnerType::Docker(_) => "docker".to_string(),
            UnifiedRunnerType::Kubernetes(_) => "kubernetes".to_string(),
            UnifiedRunnerType::Native(_) => "native".to_string(),
        }
    }

    /// Migrate configuration from legacy format
    pub fn migrate_from_legacy(legacy_config: LegacyRunnerConfig) -> UnifiedRunnerConfig {
        info!("Migrating legacy runner configuration");

        let runner_type = match legacy_config.runner_type.as_str() {
            "docker" => UnifiedRunnerType::Docker(DockerRunnerConfig {
                name: legacy_config.name.clone(),
                max_concurrent_jobs: legacy_config.max_concurrent_jobs,
                docker_endpoint: legacy_config
                    .docker_endpoint
                    .unwrap_or_else(|| "unix:///var/run/docker.sock".to_string()),
                default_image: legacy_config
                    .default_image
                    .unwrap_or_else(|| "ubuntu:22.04".to_string()),
                ..Default::default()
            }),
            "kubernetes" => UnifiedRunnerType::Kubernetes(KubernetesRunnerConfig {
                name: legacy_config.name.clone(),
                max_concurrent_jobs: legacy_config.max_concurrent_jobs,
                namespace: legacy_config
                    .namespace
                    .unwrap_or_else(|| "default".to_string()),
                ..Default::default()
            }),
            _ => UnifiedRunnerType::Native(NativeRunnerConfig {
                name: legacy_config.name.clone(),
                max_concurrent_jobs: legacy_config.max_concurrent_jobs,
                ..Default::default()
            }),
        };

        UnifiedRunnerConfig {
            name: legacy_config.name,
            runner_type,
            compatibility_flags: CompatibilityFlags {
                legacy_mode: true,
                runner_compat: true,
                hybrid_deployment: false,
                feature_flags: HashMap::new(),
            },
            control_plane_integration: ControlPlaneIntegration::default(),
        }
    }
}

/// Legacy runner configuration for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRunnerConfig {
    pub name: String,
    pub runner_type: String,
    pub max_concurrent_jobs: u32,
    pub docker_endpoint: Option<String>,
    pub default_image: Option<String>,
    pub namespace: Option<String>,
}

#[async_trait]
impl Runner for UnifiedRunner {
    async fn execute(&self, job: Job) -> Result<JobResult> {
        debug!(
            "Executing job {} on unified runner {} (type: {}, compatibility: {})",
            job.id,
            self.id,
            self.get_runner_type_string(),
            self.compatibility_mode
        );

        // Add compatibility metadata to job if in compatibility mode
        let mut job = job;
        if self.compatibility_mode {
            job.metadata.insert(
                "compatibility_mode".to_string(),
                "true".to_string(),
            );
            job.metadata.insert(
                "runner_type".to_string(),
                self.get_runner_type_string(),
            );
        }

        // Execute using the inner runner
        let result = self.inner.execute(job).await;

        // Log execution result for compatibility tracking
        match &result {
            Ok(job_result) => {
                debug!(
                    "Job {} completed successfully on unified runner {} (compatibility: {})",
                    job_result.job_id, self.id, self.compatibility_mode
                );
            }
            Err(e) => {
                warn!(
                    "Job execution failed on unified runner {} (compatibility: {}): {}",
                    self.id, self.compatibility_mode, e
                );
            }
        }

        result
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let health = self.inner.health_check().await?;

        // Add compatibility information to health status
        if self.compatibility_mode {
            match health {
                HealthStatus::Healthy => Ok(HealthStatus::Healthy),
                HealthStatus::Degraded { reason } => Ok(HealthStatus::Degraded {
                    reason: format!("{} (compatibility mode)", reason),
                }),
                HealthStatus::Unhealthy { reason } => Ok(HealthStatus::Unhealthy {
                    reason: format!("{} (compatibility mode)", reason),
                }),
            }
        } else {
            Ok(health)
        }
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        self.inner.get_capacity().await
    }

    async fn shutdown(&self) -> Result<()> {
        info!(
            "Shutting down unified runner {} (compatibility: {})",
            self.id, self.compatibility_mode
        );
        self.inner.shutdown().await
    }

    fn get_metadata(&self) -> RunnerMetadata {
        let mut metadata = self.inner.get_metadata();

        // Add unified runner metadata
        metadata.id = self.id;
        metadata.name = self.config.name.clone();

        // Add compatibility information
        if self.compatibility_mode {
            metadata.supported_job_types.push("legacy".to_string());
        }

        metadata
    }

    async fn can_handle_job(&self, job: &Job) -> Result<bool> {
        // Check compatibility requirements
        if self.compatibility_mode {
            // In compatibility mode, be more permissive
            if job.metadata.contains_key("requires_legacy") {
                return Ok(true);
            }
        }

        // Check feature flags
        if let Some(required_feature) = job.metadata.get("required_feature") {
            if !self.is_feature_enabled(required_feature) {
                return Ok(false);
            }
        }

        // Delegate to inner runner
        self.inner.can_handle_job(job).await
    }
}

/// Factory for creating unified runners
pub struct UnifiedRunnerFactory;

impl UnifiedRunnerFactory {
    /// Create a unified runner from configuration
    pub async fn create_runner(
        config: UnifiedRunnerConfig,
        event_demux: Arc<crate::core::infrastructure::event_loop::EventDemultiplexer>,
    ) -> Result<UnifiedRunner> {
        UnifiedRunner::new(config, event_demux).await
    }

    /// Create a unified runner from legacy configuration
    pub async fn create_from_legacy(
        legacy_config: LegacyRunnerConfig,
        event_demux: Arc<crate::core::infrastructure::event_loop::EventDemultiplexer>,
    ) -> Result<UnifiedRunner> {
        let unified_config = UnifiedRunner::migrate_from_legacy(legacy_config);
        Self::create_runner(unified_config, event_demux).await
    }

    /// Detect runner type from environment
    pub fn detect_runner_type() -> String {
        // Check for Docker
        if std::process::Command::new("docker")
            .arg("version")
            .output()
            .is_ok()
        {
            return "docker".to_string();
        }

        // Check for Kubernetes
        if std::process::Command::new("kubectl")
            .arg("version")
            .arg("--client")
            .output()
            .is_ok()
        {
            return "kubernetes".to_string();
        }

        // Default to native
        "native".to_string()
    }

    /// Create runner with auto-detection
    pub async fn create_auto_detected(
        name: String,
        event_demux: Arc<crate::core::infrastructure::event_loop::EventDemultiplexer>,
    ) -> Result<UnifiedRunner> {
        let detected_type = Self::detect_runner_type();
        info!("Auto-detected runner type: {}", detected_type);

        let runner_type = match detected_type.as_str() {
            "docker" => UnifiedRunnerType::Docker(DockerRunnerConfig {
                name: name.clone(),
                ..Default::default()
            }),
            "kubernetes" => UnifiedRunnerType::Kubernetes(KubernetesRunnerConfig {
                name: name.clone(),
                ..Default::default()
            }),
            _ => UnifiedRunnerType::Native(NativeRunnerConfig {
                name: name.clone(),
                ..Default::default()
            }),
        };

        let config = UnifiedRunnerConfig {
            name,
            runner_type,
            compatibility_flags: CompatibilityFlags::default(),
            control_plane_integration: ControlPlaneIntegration::default(),
        };

        Self::create_runner(config, event_demux).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::infrastructure::event_loop::EventDemultiplexer;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_legacy_config_migration() {
        let legacy_config = LegacyRunnerConfig {
            name: "test-runner".to_string(),
            runner_type: "docker".to_string(),
            max_concurrent_jobs: 4,
            docker_endpoint: Some("unix:///var/run/docker.sock".to_string()),
            default_image: Some("ubuntu:22.04".to_string()),
            namespace: None,
        };

        let unified_config = UnifiedRunner::migrate_from_legacy(legacy_config);

        assert_eq!(unified_config.name, "test-runner");
        assert!(unified_config.compatibility_flags.legacy_mode);
        assert!(unified_config.compatibility_flags.runner_compat);

        match unified_config.runner_type {
            UnifiedRunnerType::Docker(docker_config) => {
                assert_eq!(docker_config.name, "test-runner");
                assert_eq!(docker_config.max_concurrent_jobs, 4);
            }
            _ => panic!("Expected Docker runner type"),
        }
    }

    #[test]
    fn test_runner_type_detection() {
        let detected = UnifiedRunnerFactory::detect_runner_type();
        assert!(["docker", "kubernetes", "native"].contains(&detected.as_str()));
    }

    #[test]
    fn test_compatibility_flags() {
        let flags = CompatibilityFlags::default();
        assert!(!flags.legacy_mode);
        assert!(flags.runner_compat);
        assert!(flags.hybrid_deployment);
    }
}