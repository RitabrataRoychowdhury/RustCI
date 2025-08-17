pub mod docker_runner;
pub mod health_checks;
pub mod kubernetes_runner;
pub mod local_runner;
pub mod native_runner;
pub mod native_runner_factory;
pub mod unified_runner;

// Valkyrie Protocol integration modules
pub mod valkyrie_adapter;
pub mod job_queue;
pub mod selection;
pub mod http_fallback;

// Task 3.3: Unified Runner System components
pub mod capability_detector;
pub mod unified_registry;
pub mod unified_interface;
pub mod protocol_load_balancer;

// Re-export key types for backward compatibility
pub use unified_runner::{
    UnifiedRunner, UnifiedRunnerConfig, UnifiedRunnerFactory, UnifiedRunnerType,
    CompatibilityFlags, ControlPlaneIntegration, LegacyRunnerConfig,
};

// Re-export Valkyrie adapter types
pub use valkyrie_adapter::{
    ValkyrieRunnerAdapter, ValkyrieRunnerTrait, ValkyrieAdapterConfig,
    ValkyrieJob, JobSubmissionResult, AdapterPerformanceSnapshot,
};

// Re-export Unified Runner System types
pub use capability_detector::{
    RunnerCapabilityDetector, DetectedCapabilities, ProtocolSupport, 
    PerformanceTier, CapabilityDetectorConfig,
};

pub use unified_registry::{
    UnifiedRunnerRegistry, RegisteredRunner, RunnerType, RunnerStatus,
    UnifiedRegistryConfig, RegistryEvent, JobRequirements,
};

pub use unified_interface::{
    UnifiedRunnerInterface, UnifiedInterfaceConfig, JobExecutionContext,
    PerformanceRequirements, JobPriority,
};

pub use protocol_load_balancer::{
    ProtocolLoadBalancer, LoadBalancerConfig, LoadBalancingAlgorithm,
    LoadBalancerStatistics, ProtocolStats,
};
