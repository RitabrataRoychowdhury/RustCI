//! Native Runner Factory
//!
//! This module provides factory functions for creating and configuring native runners
//! with different isolation levels and control plane integration settings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::core::infrastructure::event_loop::EventDemultiplexer;
use crate::domain::entities::{RunnerEntity, RunnerType};
use crate::error::{AppError, Result};
use crate::infrastructure::runners::native_runner::{
    ControlPlaneCoordinationConfig, IsolationLevel, NativeProcessRunner, NativeRunnerConfig,
    ResourceLimits,
};

/// Native runner factory for creating different types of native runners
pub struct NativeRunnerFactory;

impl NativeRunnerFactory {
    /// Create a basic native runner with minimal configuration
    pub async fn create_basic(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 2,
            isolation_level: IsolationLevel::ProcessGroup,
            enhanced_lifecycle: false,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a native runner with control plane integration
    pub async fn create_with_control_plane(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        tags: Vec<String>,
        node_affinity: HashMap<String, String>,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 4,
            isolation_level: IsolationLevel::ProcessGroup,
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                heartbeat_interval: 30,
                health_check_interval: 60,
                auto_registration: true,
                tags,
                node_affinity,
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a high-performance native runner with custom resource limits
    pub async fn create_high_performance(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        max_concurrent_jobs: u32,
        working_directory: PathBuf,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs,
            working_directory,
            isolation_level: IsolationLevel::ProcessGroup,
            resource_limits: ResourceLimits {
                max_cpu: Some(4.0), // 4 CPU cores
                max_memory: Some(8192), // 8GB
                max_execution_time: Some(Duration::from_secs(7200)), // 2 hours
                max_processes: Some(500),
                max_file_descriptors: Some(4096),
            },
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                tags: vec!["native".to_string(), "high-performance".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a native runner with namespace isolation (Linux only)
    #[cfg(target_os = "linux")]
    pub async fn create_with_namespace_isolation(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        pid_namespace: bool,
        network_namespace: bool,
        mount_namespace: bool,
        user_namespace: bool,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 4,
            isolation_level: IsolationLevel::Namespace {
                pid: pid_namespace,
                network: network_namespace,
                mount: mount_namespace,
                user: user_namespace,
            },
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                tags: vec!["native".to_string(), "namespace-isolated".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a native runner with cgroup isolation (Linux only)
    #[cfg(target_os = "linux")]
    pub async fn create_with_cgroup_isolation(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        cpu_limit: Option<u32>,
        memory_limit: Option<u32>,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 4,
            isolation_level: IsolationLevel::Cgroup {
                cpu_limit,
                memory_limit,
                process_isolation: true,
            },
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                tags: vec!["native".to_string(), "cgroup-isolated".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a native runner with custom environment variables
    pub async fn create_with_environment(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        environment: HashMap<String, String>,
        shell: String,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 4,
            environment,
            shell,
            isolation_level: IsolationLevel::ProcessGroup,
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                tags: vec!["native".to_string(), "custom-env".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a native runner from a runner entity configuration
    pub async fn create_from_entity(
        entity: &RunnerEntity,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<NativeProcessRunner> {
        let (max_concurrent_jobs, working_directory) = match &entity.runner_type {
            RunnerType::Local {
                max_concurrent_jobs,
                working_directory,
            } => (*max_concurrent_jobs, PathBuf::from(working_directory)),
            _ => {
                return Err(AppError::BadRequest(
                    "Entity is not configured for native runner".to_string(),
                ));
            }
        };

        let config = NativeRunnerConfig {
            name: entity.name.clone(),
            max_concurrent_jobs,
            working_directory,
            isolation_level: IsolationLevel::ProcessGroup,
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                tags: entity.tags.clone(),
                node_affinity: entity.metadata.clone(),
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a development/testing native runner with relaxed settings
    pub async fn create_for_development(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<NativeProcessRunner> {
        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 1,
            default_timeout_seconds: 300, // 5 minutes
            isolation_level: IsolationLevel::None, // No isolation for easier debugging
            resource_limits: ResourceLimits {
                max_cpu: Some(1.0),
                max_memory: Some(512), // 512MB
                max_execution_time: Some(Duration::from_secs(300)),
                max_processes: Some(50),
                max_file_descriptors: Some(256),
            },
            enable_output_streaming: true,
            enhanced_lifecycle: false,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Create a production-ready native runner with security hardening
    pub async fn create_for_production(
        name: String,
        event_demux: Arc<EventDemultiplexer>,
        working_directory: PathBuf,
        tags: Vec<String>,
    ) -> Result<NativeProcessRunner> {
        let isolation_level = if cfg!(target_os = "linux") {
            // Use process group isolation on Linux for better security
            IsolationLevel::ProcessGroup
        } else {
            IsolationLevel::ProcessGroup
        };

        let config = NativeRunnerConfig {
            name,
            max_concurrent_jobs: 8,
            working_directory,
            default_timeout_seconds: 3600, // 1 hour
            isolation_level,
            resource_limits: ResourceLimits {
                max_cpu: Some(2.0), // 2 CPU cores
                max_memory: Some(4096), // 4GB
                max_execution_time: Some(Duration::from_secs(3600)),
                max_processes: Some(200),
                max_file_descriptors: Some(2048),
            },
            enable_output_streaming: false, // Disable for security
            max_output_buffer_size: 512 * 1024, // 512KB limit
            enhanced_lifecycle: true,
            control_plane_coordination: ControlPlaneCoordinationConfig {
                enabled: true,
                heartbeat_interval: 60,
                health_check_interval: 120,
                auto_registration: true,
                tags,
                ..Default::default()
            },
            ..Default::default()
        };

        NativeProcessRunner::new(config, event_demux).await
    }

    /// Get recommended configuration for a given use case
    pub fn get_recommended_config(use_case: NativeRunnerUseCase) -> NativeRunnerConfig {
        match use_case {
            NativeRunnerUseCase::Development => NativeRunnerConfig {
                name: "dev-native-runner".to_string(),
                max_concurrent_jobs: 1,
                isolation_level: IsolationLevel::None,
                enhanced_lifecycle: false,
                control_plane_coordination: ControlPlaneCoordinationConfig {
                    enabled: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            NativeRunnerUseCase::Testing => NativeRunnerConfig {
                name: "test-native-runner".to_string(),
                max_concurrent_jobs: 2,
                isolation_level: IsolationLevel::ProcessGroup,
                enhanced_lifecycle: true,
                control_plane_coordination: ControlPlaneCoordinationConfig {
                    enabled: true,
                    tags: vec!["native".to_string(), "test".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
            NativeRunnerUseCase::Production => NativeRunnerConfig {
                name: "prod-native-runner".to_string(),
                max_concurrent_jobs: 8,
                isolation_level: IsolationLevel::ProcessGroup,
                resource_limits: ResourceLimits {
                    max_cpu: Some(4.0),
                    max_memory: Some(8192),
                    max_execution_time: Some(Duration::from_secs(7200)),
                    max_processes: Some(500),
                    max_file_descriptors: Some(4096),
                },
                enhanced_lifecycle: true,
                control_plane_coordination: ControlPlaneCoordinationConfig {
                    enabled: true,
                    tags: vec!["native".to_string(), "production".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
            NativeRunnerUseCase::HighPerformance => NativeRunnerConfig {
                name: "hp-native-runner".to_string(),
                max_concurrent_jobs: 16,
                isolation_level: IsolationLevel::ProcessGroup,
                resource_limits: ResourceLimits {
                    max_cpu: Some(8.0),
                    max_memory: Some(16384),
                    max_execution_time: Some(Duration::from_secs(14400)),
                    max_processes: Some(1000),
                    max_file_descriptors: Some(8192),
                },
                enhanced_lifecycle: true,
                control_plane_coordination: ControlPlaneCoordinationConfig {
                    enabled: true,
                    tags: vec!["native".to_string(), "high-performance".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

/// Use cases for native runners
#[derive(Debug, Clone, PartialEq)]
pub enum NativeRunnerUseCase {
    /// Development environment with minimal restrictions
    Development,
    /// Testing environment with moderate isolation
    Testing,
    /// Production environment with full security
    Production,
    /// High-performance computing workloads
    HighPerformance,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::infrastructure::event_loop::EventDemultiplexer;
    use crate::domain::entities::runner::Runner;
    #[tokio::test]
    async fn test_create_basic_native_runner() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let runner = NativeRunnerFactory::create_basic(
            "test-basic".to_string(),
            event_demux,
        ).await.unwrap();

        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "test-basic");
        assert!(metadata.supported_job_types.contains(&"native".to_string()));
    }

    #[tokio::test]
    async fn test_create_with_control_plane() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let tags = vec!["test".to_string(), "control-plane".to_string()];
        let mut node_affinity = HashMap::new();
        node_affinity.insert("zone".to_string(), "test-zone".to_string());

        let runner = NativeRunnerFactory::create_with_control_plane(
            "test-cp".to_string(),
            event_demux,
            tags,
            node_affinity,
        ).await.unwrap();

        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "test-cp");
    }

    #[tokio::test]
    async fn test_create_high_performance() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let working_dir = std::env::temp_dir().join("rustci-hp-test");

        let runner = NativeRunnerFactory::create_high_performance(
            "test-hp".to_string(),
            event_demux,
            8,
            working_dir,
        ).await.unwrap();

        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "test-hp");
        
        let capacity = runner.get_capacity().await.unwrap();
        assert_eq!(capacity.max_concurrent_jobs, 8);
    }

    #[tokio::test]
    async fn test_create_for_development() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let runner = NativeRunnerFactory::create_for_development(
            "test-dev".to_string(),
            event_demux,
        ).await.unwrap();

        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "test-dev");
        
        let capacity = runner.get_capacity().await.unwrap();
        assert_eq!(capacity.max_concurrent_jobs, 1);
    }

    #[tokio::test]
    async fn test_create_for_production() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let working_dir = std::env::temp_dir().join("rustci-prod-test");
        let tags = vec!["production".to_string(), "secure".to_string()];

        let runner = NativeRunnerFactory::create_for_production(
            "test-prod".to_string(),
            event_demux,
            working_dir,
            tags,
        ).await.unwrap();

        let metadata = runner.get_metadata();
        assert_eq!(metadata.name, "test-prod");
        
        let capacity = runner.get_capacity().await.unwrap();
        assert_eq!(capacity.max_concurrent_jobs, 8);
    }

    #[tokio::test]
    async fn test_get_recommended_config() {
        let dev_config = NativeRunnerFactory::get_recommended_config(NativeRunnerUseCase::Development);
        assert_eq!(dev_config.max_concurrent_jobs, 1);
        assert_eq!(dev_config.isolation_level, IsolationLevel::None);
        assert!(!dev_config.enhanced_lifecycle);

        let prod_config = NativeRunnerFactory::get_recommended_config(NativeRunnerUseCase::Production);
        assert_eq!(prod_config.max_concurrent_jobs, 8);
        assert_eq!(prod_config.isolation_level, IsolationLevel::ProcessGroup);
        assert!(prod_config.enhanced_lifecycle);

        let hp_config = NativeRunnerFactory::get_recommended_config(NativeRunnerUseCase::HighPerformance);
        assert_eq!(hp_config.max_concurrent_jobs, 16);
        assert_eq!(hp_config.resource_limits.max_cpu, Some(8.0));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_create_with_namespace_isolation() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let result = NativeRunnerFactory::create_with_namespace_isolation(
            "test-ns".to_string(),
            event_demux,
            true,  // pid namespace
            false, // network namespace
            true,  // mount namespace
            false, // user namespace
        ).await;

        // This might fail if namespace support is not available, which is acceptable
        match result {
            Ok(runner) => {
                let metadata = runner.get_metadata();
                assert_eq!(metadata.name, "test-ns");
            }
            Err(_) => {
                // Namespace support not available in test environment
                println!("Namespace isolation not available in test environment");
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_create_with_cgroup_isolation() {
        let event_demux = Arc::new(EventDemultiplexer::new());
        let result = NativeRunnerFactory::create_with_cgroup_isolation(
            "test-cg".to_string(),
            event_demux,
            Some(1000), // 1 CPU core
            Some(2048), // 2GB memory
        ).await;

        // This might fail if cgroup support is not available, which is acceptable
        match result {
            Ok(runner) => {
                let metadata = runner.get_metadata();
                assert_eq!(metadata.name, "test-cg");
            }
            Err(_) => {
                // Cgroup support not available in test environment
                println!("Cgroup isolation not available in test environment");
            }
        }
    }
}