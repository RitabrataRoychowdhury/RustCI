pub mod alerting;
pub mod auto_scaler;
pub mod auto_scaling_load_balancer;
pub mod cache_manager;
pub mod connection_pool;
pub mod critical_path_optimizer;
pub mod dashboard;
pub mod load_balancer;
pub mod memory_efficient;
pub mod metrics;
pub mod monitor;
pub mod performance_alerting_system;
pub mod resource_manager;
pub mod self_healing;

// Alerting
pub use alerting::{
    AlertManager, AlertRule, AlertCondition, ComparisonOperator, LogicalOperator,
    NotificationChannel, AlertNotification, AlertNotificationType, ActiveAlert,
    ProductionAlertManager, IssueSeverity as AlertIssueSeverity
};

// Auto Scaler
pub use auto_scaler::{
    ProductionAutoScaler, AutoScalerConfig, ScalingMetrics, ScalingEvent,
    AutoScalerStats, InstanceProvider, MetricsCollector as AutoScalerMetricsCollector,
    MockInstanceProvider, MockMetricsCollector
};

// Cache Manager
pub use cache_manager::{
    MultiTierCacheManager, CacheConfig, CacheEntry, CacheTier,
    EvictionPolicy, CacheMetrics
};

// Connection Pool
pub use connection_pool::{
    ConnectionPool, ConnectionPoolConfig, ConnectionGuard,
    PoolStats as ConnectionPoolStats, PooledConnection, ConnectionFactory,
    HttpConnection, HttpConnectionFactory
};

// Dashboard
pub use dashboard::{
    ProductionPerformanceDashboard, DashboardMetrics, PerformanceDashboard,
    PerformanceReport, AlertSummary, SystemOverview, HealthStatus as DashboardHealthStatus,
    ComponentHealth, HistoricalData, TimeRange, TimeSeriesPoint
};

// Load Balancer
pub use load_balancer::{
    ProductionLoadBalancer, LoadBalancer, LoadBalancerConfig, LoadBalancingAlgorithm,
    ServiceEndpoint, HealthStatus as LoadBalancerHealthStatus, RoutingContext, LoadBalancerStats
};

// Memory Efficient
pub use memory_efficient::{
    CircularBuffer, LruTtlCache, CacheStats as MemoryCacheStats, StringInterner,
    MemoryPool, PoolStats as MemoryPoolStats, BitVector
};

// Metrics
pub use metrics::{
    PerformanceMetrics, MemoryMetrics, DiskMetrics, NetworkMetrics,
    DatabaseMetrics, RequestMetrics, PerformanceIssue, PerformanceIssueType,
    IssueSeverity as MetricsIssueSeverity, PerformanceAlert, AlertType, ScalingAction, ScalingActionType
};

// Monitor
pub use monitor::{
    PerformanceMonitor, ProductionPerformanceMonitor, IssueDetector,
    CpuUsageDetector, MemoryUsageDetector, ResponseTimeDetector,
    DatabasePerformanceDetector
};

// Resource Manager
pub use resource_manager::{
    ResourceManager, ProductionResourceManager, ResourceRequirements, ResourceAllocation, 
    ResourceQuota, ResourceUsage as ResourceManagerUsage, ResourcePriority,
    OptimizationReport, OptimizationRecommendation, RecommendationType, ImpactLevel, ResourceSavings
};

// Auto Scaling Load Balancer
pub use auto_scaling_load_balancer::{
    AutoScalingLoadBalancer, AutoScalingLoadBalancerConfig, AutoScalingLoadBalancerStats,
    IntegrationStats
};

// Critical Path Optimizer
pub use critical_path_optimizer::{
    CriticalPathOptimizer, OptimizerConfig, BatchProcessor, BatchOperation, OperationType,
    CacheOptimizer, CompressionEngine, CompressedData, CacheStats, OptimizationMetrics
};

// Self Healing
pub use self_healing::{
    SelfHealingManager, SelfHealingConfig, PerformanceIssueDetector, HealingEngine,
    DetectedIssue, HealingAction, HealingResult, HealingStrategy,
    PerformanceSnapshot, SelfHealingMetrics
};

// Performance Alerting System
pub use performance_alerting_system::{
    PerformanceAlertingSystem, PerformanceAlertingConfig, PerformanceThresholds,
    NotificationSettings, PerformanceAlertRule, EscalationPolicy, EscalationStep,
    AlertEvent, AlertEventType, PerformanceAlertingMetrics
};