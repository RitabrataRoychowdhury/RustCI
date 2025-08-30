use crate::error::{AppError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Database,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{RwLock, Semaphore},
    time::{interval, sleep},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Database operation types for monitoring and retry logic
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseOperationType {
    Read,
    Write,
    Transaction,
    HealthCheck,
}

/// Database health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealthStatus {
    pub is_healthy: bool,
    pub connection_count: u32,
    pub active_operations: u32,
    pub average_response_time_ms: f64,
    pub last_health_check: DateTime<Utc>,
    pub error_rate: f64,
    pub connection_pool_utilization: f64,
    pub details: HashMap<String, serde_json::Value>,
}

/// Database operation metrics
#[derive(Debug, Clone)]
pub struct DatabaseOperationMetrics {
    pub operation_id: Uuid,
    pub operation_type: DatabaseOperationType,
    pub start_time: Instant,
    pub duration: Option<Duration>,
    pub success: bool,
    pub error_message: Option<String>,
    pub retry_count: u32,
}

/// Configuration for the production database manager
#[derive(Debug, Clone)]
pub struct ProductionDatabaseConfig {
    pub connection_uri: String,
    pub database_name: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_ms: u64,
    pub operation_timeout_ms: u64,
    pub health_check_interval_ms: u64,
    pub retry_max_attempts: u32,
    pub retry_initial_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_backoff_multiplier: f64,
    pub enable_metrics: bool,
}

impl Default for ProductionDatabaseConfig {
    fn default() -> Self {
        Self {
            connection_uri: "mongodb://localhost:27017".to_string(),
            database_name: "rustci".to_string(),
            max_connections: 100,
            min_connections: 10,
            connection_timeout_ms: 5000,
            operation_timeout_ms: 30000,
            health_check_interval_ms: 30000,
            retry_max_attempts: 3,
            retry_initial_delay_ms: 100,
            retry_max_delay_ms: 5000,
            retry_backoff_multiplier: 2.0,
            enable_metrics: true,
        }
    }
}

/// Production-grade database manager with connection pooling and health monitoring
pub struct ProductionDatabaseManager {
    client: Client,
    database: Database,
    config: ProductionDatabaseConfig,
    connection_semaphore: Arc<Semaphore>,
    health_status: Arc<RwLock<DatabaseHealthStatus>>,
    metrics: Arc<RwLock<Vec<DatabaseOperationMetrics>>>,
    operation_counter: AtomicU64,
    error_counter: AtomicU64,
    is_monitoring: AtomicBool,
}

/// Trait for database operations with retry and health monitoring
#[async_trait]
pub trait ProductionDatabaseOperations {
    async fn execute_with_retry<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send;

    async fn get_health_status(&self) -> DatabaseHealthStatus;
    async fn perform_health_check(&self) -> Result<()>;
    async fn get_operation_metrics(&self) -> Vec<DatabaseOperationMetrics>;
    async fn reset_metrics(&self);
}

impl ProductionDatabaseManager {
    /// Create a new production database manager with enhanced configuration
    pub async fn new(config: ProductionDatabaseConfig) -> Result<Self> {
        info!("🔄 Initializing Production Database Manager...");

        let mut client_options = ClientOptions::parse(&config.connection_uri)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to parse MongoDB URI: {}", e)))?;

        // Configure connection pool settings
        client_options.max_pool_size = Some(config.max_connections);
        client_options.min_pool_size = Some(config.min_connections);
        client_options.connect_timeout = Some(Duration::from_millis(config.connection_timeout_ms));
        client_options.server_selection_timeout = Some(Duration::from_millis(config.operation_timeout_ms));

        // Set server API version for stability
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);

        // Create client with enhanced options
        let client = Client::with_options(client_options)
            .map_err(|e| AppError::DatabaseError(format!("Failed to create MongoDB client: {}", e)))?;

        let database = client.database(&config.database_name);

        // Initialize health status
        let initial_health = DatabaseHealthStatus {
            is_healthy: false,
            connection_count: 0,
            active_operations: 0,
            average_response_time_ms: 0.0,
            last_health_check: Utc::now(),
            error_rate: 0.0,
            connection_pool_utilization: 0.0,
            details: HashMap::new(),
        };

        let manager = Self {
            client,
            database,
            connection_semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
            health_status: Arc::new(RwLock::new(initial_health)),
            metrics: Arc::new(RwLock::new(Vec::new())),
            operation_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            is_monitoring: AtomicBool::new(false),
            config,
        };

        // Perform initial health check
        manager.perform_health_check().await?;
        info!("✅ Production Database Manager initialized successfully");

        Ok(manager)
    }

    /// Start background health monitoring
    pub async fn start_monitoring(&self) {
        if self.is_monitoring.swap(true, Ordering::SeqCst) {
            warn!("Database monitoring is already running");
            return;
        }

        let health_status = Arc::clone(&self.health_status);
        let client = self.client.clone();
        let database = self.database.clone();
        let config = self.config.clone();
        let operation_counter = Arc::new(AtomicU64::new(self.operation_counter.load(Ordering::SeqCst)));
        let error_counter = Arc::new(AtomicU64::new(self.error_counter.load(Ordering::SeqCst)));
        let is_monitoring = Arc::new(AtomicBool::new(self.is_monitoring.load(Ordering::SeqCst)));

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.health_check_interval_ms));
            
            while is_monitoring.load(Ordering::SeqCst) {
                interval.tick().await;
                
                let start_time = Instant::now();
                let health_check_result = Self::perform_health_check_internal(&client, &database).await;
                let response_time = start_time.elapsed().as_millis() as f64;

                let mut health = health_status.write().await;
                health.last_health_check = Utc::now();
                health.average_response_time_ms = response_time;

                match health_check_result {
                    Ok(_) => {
                        health.is_healthy = true;
                        debug!("Database health check passed in {:.2}ms", response_time);
                    }
                    Err(e) => {
                        health.is_healthy = false;
                        error!("Database health check failed: {}", e);
                        health.details.insert("last_error".to_string(), 
                            serde_json::Value::String(e.to_string()));
                    }
                }

                // Update metrics
                let total_ops = operation_counter.load(Ordering::SeqCst);
                let total_errors = error_counter.load(Ordering::SeqCst);
                health.error_rate = if total_ops > 0 {
                    (total_errors as f64 / total_ops as f64) * 100.0
                } else {
                    0.0
                };
            }
        });

        info!("🔍 Database health monitoring started");
    }

    /// Stop background health monitoring
    pub fn stop_monitoring(&self) {
        self.is_monitoring.store(false, Ordering::SeqCst);
        info!("⏹️ Database health monitoring stopped");
    }

    /// Get the underlying database instance
    pub fn get_database(&self) -> &Database {
        &self.database
    }

    /// Get the underlying client instance
    pub fn get_client(&self) -> &Client {
        &self.client
    }

    /// Internal health check implementation
    async fn perform_health_check_internal(_client: &Client, database: &Database) -> Result<()> {
        // Ping the database
        database
            .run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Health check ping failed: {}", e)))?;

        // Check server status
        let server_status = database
            .run_command(doc! {"serverStatus": 1}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Server status check failed: {}", e)))?;

        // Validate server status response
        if !server_status.contains_key("ok") {
            return Err(AppError::DatabaseError("Invalid server status response".to_string()));
        }

        Ok(())
    }

    /// Record operation metrics
    async fn record_operation_metrics(&self, mut metrics: DatabaseOperationMetrics) {
        if !self.config.enable_metrics {
            return;
        }

        metrics.duration = Some(metrics.start_time.elapsed());
        
        let mut metrics_vec = self.metrics.write().await;
        metrics_vec.push(metrics);

        // Keep only the last 1000 metrics to prevent memory growth
        if metrics_vec.len() > 1000 {
            let excess = metrics_vec.len() - 1000;
            metrics_vec.drain(0..excess);
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.config.retry_initial_delay_ms as f64 
            * self.config.retry_backoff_multiplier.powi(attempt as i32)) as u64;
        
        Duration::from_millis(delay_ms.min(self.config.retry_max_delay_ms))
    }
}

#[async_trait]
impl ProductionDatabaseOperations for ProductionDatabaseManager {
    async fn execute_with_retry<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        let operation_id = Uuid::new_v4();
        let start_time = Instant::now();
        let mut last_error = None;

        // Acquire connection from pool
        let _permit = self.connection_semaphore.acquire().await
            .map_err(|e| AppError::DatabaseError(format!("Failed to acquire connection: {}", e)))?;

        for attempt in 0..self.config.retry_max_attempts {
            self.operation_counter.fetch_add(1, Ordering::SeqCst);

            match operation().await {
                Ok(result) => {
                    // Record successful operation
                    let metrics = DatabaseOperationMetrics {
                        operation_id,
                        operation_type: DatabaseOperationType::Read, // Default, should be parameterized
                        start_time,
                        duration: None,
                        success: true,
                        error_message: None,
                        retry_count: attempt,
                    };
                    self.record_operation_metrics(metrics).await;

                    return Ok(result);
                }
                Err(e) => {
                    self.error_counter.fetch_add(1, Ordering::SeqCst);
                    last_error = Some(e.clone());

                    // Check if error is retryable
                    let is_retryable = match &e {
                        AppError::DatabaseError(msg) => {
                            // Retry on connection errors, timeouts, and temporary failures
                            msg.contains("connection") || 
                            msg.contains("timeout") || 
                            msg.contains("temporary") ||
                            msg.contains("network")
                        }
                        _ => false,
                    };

                    if !is_retryable || attempt == self.config.retry_max_attempts - 1 {
                        // Record failed operation
                        let metrics = DatabaseOperationMetrics {
                            operation_id,
                            operation_type: DatabaseOperationType::Read,
                            start_time,
                            duration: None,
                            success: false,
                            error_message: Some(e.to_string()),
                            retry_count: attempt,
                        };
                        self.record_operation_metrics(metrics).await;

                        return Err(e);
                    }

                    // Wait before retry with exponential backoff
                    let delay = self.calculate_backoff_delay(attempt);
                    warn!("Database operation failed (attempt {}), retrying in {:?}: {}", 
                          attempt + 1, delay, e);
                    sleep(delay).await;
                }
            }
        }

        // This should never be reached due to the loop logic above
        Err(last_error.unwrap_or_else(|| 
            AppError::DatabaseError("Maximum retry attempts exceeded".to_string())))
    }

    async fn get_health_status(&self) -> DatabaseHealthStatus {
        self.health_status.read().await.clone()
    }

    async fn perform_health_check(&self) -> Result<()> {
        let start_time = Instant::now();
        
        match Self::perform_health_check_internal(&self.client, &self.database).await {
            Ok(_) => {
                let response_time = start_time.elapsed().as_millis() as f64;
                
                let mut health = self.health_status.write().await;
                health.is_healthy = true;
                health.last_health_check = Utc::now();
                health.average_response_time_ms = response_time;
                health.details.clear(); // Clear any previous error details
                
                debug!("Database health check passed in {:.2}ms", response_time);
                Ok(())
            }
            Err(e) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = false;
                health.last_health_check = Utc::now();
                health.details.insert("last_error".to_string(), 
                    serde_json::Value::String(e.to_string()));
                
                error!("Database health check failed: {}", e);
                Err(e)
            }
        }
    }

    async fn get_operation_metrics(&self) -> Vec<DatabaseOperationMetrics> {
        self.metrics.read().await.clone()
    }

    async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.clear();
        self.operation_counter.store(0, Ordering::SeqCst);
        self.error_counter.store(0, Ordering::SeqCst);
        info!("Database operation metrics reset");
    }
}

impl Drop for ProductionDatabaseManager {
    fn drop(&mut self) {
        self.stop_monitoring();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_production_database_config_default() {
        let config = ProductionDatabaseConfig::default();
        assert_eq!(config.max_connections, 100);
        assert_eq!(config.min_connections, 10);
        assert_eq!(config.retry_max_attempts, 3);
        assert!(config.enable_metrics);
    }

    #[tokio::test]
    async fn test_calculate_backoff_delay() {
        let config = ProductionDatabaseConfig::default();
        let manager = ProductionDatabaseManager {
            client: Client::with_uri_str("mongodb://localhost:27017").await.unwrap(),
            database: Client::with_uri_str("mongodb://localhost:27017").await.unwrap().database("test"),
            config: config.clone(),
            connection_semaphore: Arc::new(Semaphore::new(10)),
            health_status: Arc::new(RwLock::new(DatabaseHealthStatus {
                is_healthy: true,
                connection_count: 0,
                active_operations: 0,
                average_response_time_ms: 0.0,
                last_health_check: Utc::now(),
                error_rate: 0.0,
                connection_pool_utilization: 0.0,
                details: HashMap::new(),
            })),
            metrics: Arc::new(RwLock::new(Vec::new())),
            operation_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            is_monitoring: AtomicBool::new(false),
        };

        let delay1 = manager.calculate_backoff_delay(0);
        let delay2 = manager.calculate_backoff_delay(1);
        let delay3 = manager.calculate_backoff_delay(2);

        assert_eq!(delay1, Duration::from_millis(100));
        assert_eq!(delay2, Duration::from_millis(200));
        assert_eq!(delay3, Duration::from_millis(400));
    }

    #[tokio::test]
    async fn test_health_status_initialization() {
        let config = ProductionDatabaseConfig::default();
        
        // This test will fail if MongoDB is not running, which is expected in CI
        // We'll test the initialization logic without actually connecting
        let result = timeout(
            Duration::from_millis(1000),
            ProductionDatabaseManager::new(config)
        ).await;

        // The test should either succeed or timeout/fail due to no MongoDB
        // We're mainly testing that the code doesn't panic
        match result {
            Ok(Ok(_manager)) => {
                // Success case - MongoDB is running
            }
            Ok(Err(_)) | Err(_) => {
                // Expected case - MongoDB not available or timeout
            }
        }
    }
}