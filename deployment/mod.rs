// Deployment and migration tooling
pub mod migrations;
pub mod blue_green;
pub mod validation;
pub mod rollback;
pub mod health_checks;

// Production-grade blue-green deployment system
pub mod blue_green_manager;
pub mod health_checker;
pub mod traffic_router;
pub mod automation;
pub mod circuit_breaker;
pub mod graceful_degradation;
pub mod auto_scaling;
pub mod resource_management;
pub mod maintenance;
pub mod disaster_recovery;
pub mod observability;

pub use blue_green_manager::{
    BlueGreenDeploymentManager, ProductionBlueGreenManager, DeploymentConfig,
    DeploymentEnvironment, DeploymentStatus, DeploymentState, RollbackConfig,
    HealthCheckResult, ValidationResult, ValidationRule
};
pub use health_checker::{ProductionHealthChecker, HealthChecker};
pub use traffic_router::{
    ProductionTrafficRouter, TrafficRouter, TrafficRoutingConfig, TrafficSplit,
    LoadBalancerAdapter, HAProxyAdapter, MockLoadBalancerAdapter
};
pub use automation::{
    ProductionDeploymentAutomation, DeploymentAutomation, AutomatedDeploymentConfig,
    DeploymentPipeline, DeploymentStage, StageType, RollbackStrategy, NotificationConfig,
    DeploymentExecution, ExecutionStatus, StageResult, StageStatus, NotificationService,
    MockNotificationService
};
pub use circuit_breaker::{
    CircuitBreaker, ProductionCircuitBreaker, CircuitBreakerConfig, CircuitBreakerState,
    CircuitBreakerStats, DegradationMode, DegradationConfig, GracefulDegradationManager,
    ProductionGracefulDegradationManager
};
pub use graceful_degradation::{
    SystemHealthMonitor, ProductionSystemHealthMonitor, FallbackExecutor,
    ProductionFallbackExecutor, SystemDegradationOrchestrator, ServiceHealthStatus,
    FallbackMechanism, FallbackType, FallbackConfig, SystemDegradationState
};
pub use auto_scaling::{
    ProductionAutoScaler, ScalingMetrics, ScalingPolicy, ScalingAction, ScalingEvent,
    MetricsCollector, InstanceManager, CostOptimizer, MockMetricsCollector,
    MockInstanceManager, MockCostOptimizer, ResourceQuota, ResourceUsage,
    CostRecommendation, CostRecommendationType, ImplementationEffort, InstanceHealth
};
pub use resource_management::{
    ResourceManager, ProductionResourceManager, ResourceOptimizer, ProductionResourceOptimizer,
    ResourceMonitor, ResourceAllocation, ResourceRequest, ResourceRequirements, ResourcePriority,
    ResourcePool, ResourceMonitoringData, ResourceOptimizationRecommendation, OptimizationType,
    ImplementationComplexity, ResourceThresholds
};
pub use maintenance::{
    MaintenanceManager, ProductionMaintenanceManager, TrafficDrainer, DatabaseMigrator,
    MaintenanceWindow, MaintenanceType, MaintenanceStatus, MaintenanceExecution, TrafficDrainConfig,
    DrainStatus, MigrationPlan, MigrationResult, MigrationStatus, RollbackPlan, RollbackTrigger,
    MockTrafficDrainer, MockDatabaseMigrator, MockNotificationService
};
pub use disaster_recovery::{
    DisasterRecoveryManager, ProductionDisasterRecoveryManager, BackupManager, MockBackupManager,
    DisasterRecoveryPlan, BackupStrategy, FailoverStrategy, RecoveryProcedure, ValidationTest,
    BackupJob, RestoreJob, FailoverEvent, TestResult, ValidationResult, RecoveryPriority,
    BackupType, BackupFrequency, FailoverType, TestType, TestFrequency
};
pub use observability::{
    ProductionObservabilitySystem, DistributedTracer, StructuredLogger, MetricsCollector,
    HealthMonitor, AlertManager, TraceContext, StructuredLog, MetricPoint, HealthCheck, Alert,
    ServiceObservabilitySummary, MockDistributedTracer, MockStructuredLogger, MockMetricsCollector,
    MockHealthMonitor, MockAlertManager, SpanStatus, LogLevel, MetricType, HealthStatus,
    AlertSeverity, AlertStatus
};