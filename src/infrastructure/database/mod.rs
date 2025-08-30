pub mod db;
pub mod production_manager;
pub mod transaction_manager;
pub mod health_monitor;

pub use db::DatabaseManager;
pub use production_manager::{
    ProductionDatabaseManager, 
    ProductionDatabaseOperations, 
    ProductionDatabaseConfig,
    DatabaseHealthStatus,
    DatabaseOperationMetrics,
    DatabaseOperationType
};
pub use transaction_manager::{
    ProductionTransactionManager,
    TransactionOperations,
    TransactionManagerConfig,
    TransactionHandle,
    TransactionContext,
    TransactionIsolationLevel,
    TransactionStatus,
    TransactionStats,
    CompensationAction,
};
pub use health_monitor::{
    ProductionDatabaseHealthMonitor,
    DatabaseHealthMonitor,
    DatabaseHealthConfig,
    DatabaseHealthStatus as HealthMonitorStatus,
    ConnectionStatus,
    PerformanceMetrics,
    QueryMetrics,
    ErrorMetrics,
    ResourceUsage,
    HealthAlert,
    AlertType,
    AlertSeverity,
    SlowQueryInfo,
    ErrorInfo,
};
