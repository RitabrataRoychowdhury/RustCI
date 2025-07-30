//! Advanced database patterns and infrastructure
//!
//! This module provides advanced database functionality including:
//! - Connection pooling and management
//! - Query builders for complex queries
//! - Database migrations
//! - Failover and retry mechanisms
//! - Read/write path optimization

use async_trait::async_trait;
use mongodb::{
    bson::{doc, Bson, Document},
    options::{ClientOptions, FindOptions, WriteConcern},
    Client, Collection, Database,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(1800),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

/// Advanced database manager with connection pooling and failover
pub struct AdvancedDatabaseManager {
    primary_client: Client,
    secondary_clients: Vec<Client>,
    database_name: String,
    pool_config: ConnectionPoolConfig,
    connection_semaphore: Arc<Semaphore>,
    health_status: Arc<RwLock<DatabaseHealthStatus>>,
    metrics: Arc<RwLock<DatabaseMetrics>>,
}

#[derive(Debug, Clone)]
pub struct DatabaseHealthStatus {
    pub primary_healthy: bool,
    pub secondary_healthy: Vec<bool>,
    pub last_check: Instant,
    pub error_count: u64,
    pub recovery_mode: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DatabaseMetrics {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub average_response_time_ms: f64,
    pub connection_pool_usage: f64,
    pub failover_count: u64,
}

impl AdvancedDatabaseManager {
    /// Create a new advanced database manager
    pub async fn new(
        primary_uri: &str,
        secondary_uris: Vec<String>,
        database_name: String,
        pool_config: ConnectionPoolConfig,
    ) -> Result<Self> {
        info!("🔄 Initializing advanced database manager...");

        // Create primary client
        let primary_client = Self::create_client(primary_uri, &pool_config).await?;

        // Create secondary clients
        let mut secondary_clients = Vec::new();
        for uri in secondary_uris {
            match Self::create_client(&uri, &pool_config).await {
                Ok(client) => secondary_clients.push(client),
                Err(e) => warn!("Failed to create secondary client for {}: {}", uri, e),
            }
        }

        let connection_semaphore = Arc::new(Semaphore::new(pool_config.max_connections as usize));

        let health_status = Arc::new(RwLock::new(DatabaseHealthStatus {
            primary_healthy: true,
            secondary_healthy: vec![true; secondary_clients.len()],
            last_check: Instant::now(),
            error_count: 0,
            recovery_mode: false,
        }));

        let metrics = Arc::new(RwLock::new(DatabaseMetrics::default()));

        let manager = Self {
            primary_client,
            secondary_clients,
            database_name,
            pool_config,
            connection_semaphore,
            health_status,
            metrics,
        };

        // Start health check task
        manager.start_health_check_task().await;

        info!("✅ Advanced database manager initialized successfully");
        Ok(manager)
    }

    /// Create a MongoDB client with optimized settings
    async fn create_client(uri: &str, config: &ConnectionPoolConfig) -> Result<Client> {
        let mut client_options = ClientOptions::parse(uri)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to parse MongoDB URI: {}", e)))?;

        // Configure connection pool
        client_options.min_pool_size = Some(config.min_connections);
        client_options.max_pool_size = Some(config.max_connections);
        client_options.max_idle_time = Some(config.idle_timeout);
        client_options.connect_timeout = Some(config.connection_timeout);

        // Configure write concern
        client_options.write_concern = Some(
            WriteConcern::builder()
                .w(mongodb::options::Acknowledgment::Majority)
                .journal(true)
                .build(),
        );

        Client::with_options(client_options)
            .map_err(|e| AppError::DatabaseError(format!("Failed to create MongoDB client: {}", e)))
    }

    /// Get database instance with failover support
    pub async fn get_database(&self, prefer_read: bool) -> Result<Database> {
        let _permit =
            self.connection_semaphore.acquire().await.map_err(|e| {
                AppError::DatabaseError(format!("Connection pool exhausted: {}", e))
            })?;

        let health = self.health_status.read().await;

        if prefer_read && !self.secondary_clients.is_empty() {
            // Try secondary clients first for read operations
            for (i, client) in self.secondary_clients.iter().enumerate() {
                if *health.secondary_healthy.get(i).unwrap_or(&false) {
                    return Ok(client.database(&self.database_name));
                }
            }
        }

        // Fall back to primary
        if health.primary_healthy {
            Ok(self.primary_client.database(&self.database_name))
        } else {
            Err(AppError::DatabaseError(
                "All database connections are unhealthy".to_string(),
            ))
        }
    }

    /// Execute operation with retry and failover
    pub async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send
            + Sync,
        T: Send,
    {
        let mut last_error = None;

        for attempt in 0..self.pool_config.retry_attempts {
            let start_time = Instant::now();

            match operation().await {
                Ok(result) => {
                    self.record_success(start_time.elapsed()).await;
                    return Ok(result);
                }
                Err(e) => {
                    self.record_failure().await;
                    last_error = Some(e);

                    if attempt < self.pool_config.retry_attempts - 1 {
                        tokio::time::sleep(self.pool_config.retry_delay * (attempt + 1)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::DatabaseError("Unknown error".to_string())))
    }

    /// Record successful operation
    async fn record_success(&self, duration: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        metrics.successful_queries += 1;

        // Update average response time (simple moving average)
        let new_time_ms = duration.as_millis() as f64;
        if metrics.average_response_time_ms == 0.0 {
            metrics.average_response_time_ms = new_time_ms;
        } else {
            metrics.average_response_time_ms =
                (metrics.average_response_time_ms * 0.9) + (new_time_ms * 0.1);
        }
    }

    /// Record failed operation
    async fn record_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_queries += 1;
        metrics.failed_queries += 1;

        let mut health = self.health_status.write().await;
        health.error_count += 1;
    }

    /// Start background health check task
    async fn start_health_check_task(&self) {
        let health_status = self.health_status.clone();
        let primary_client = self.primary_client.clone();
        let secondary_clients = self.secondary_clients.clone();
        let database_name = self.database_name.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut health = health_status.write().await;

                // Check primary
                health.primary_healthy =
                    Self::check_connection_health(&primary_client, &database_name).await;

                // Check secondaries
                for (i, client) in secondary_clients.iter().enumerate() {
                    if let Some(healthy) = health.secondary_healthy.get_mut(i) {
                        *healthy = Self::check_connection_health(client, &database_name).await;
                    }
                }

                health.last_check = Instant::now();

                debug!(
                    primary_healthy = health.primary_healthy,
                    secondary_count = health.secondary_healthy.len(),
                    "Database health check completed"
                );
            }
        });
    }

    /// Check if a connection is healthy
    async fn check_connection_health(client: &Client, database_name: &str) -> bool {
        match client
            .database(database_name)
            .run_command(doc! {"ping": 1}, None)
            .await
        {
            Ok(_) => true,
            Err(e) => {
                warn!("Database health check failed: {}", e);
                false
            }
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> DatabaseMetrics {
        self.metrics.read().await.clone()
    }

    /// Get current health status
    pub async fn get_health_status(&self) -> DatabaseHealthStatus {
        self.health_status.read().await.clone()
    }
}

/// Query builder for complex MongoDB queries
#[allow(dead_code)]
pub struct QueryBuilder {
    filter: Document,
    sort: Option<Document>,
    limit: Option<i64>,
    skip: Option<u64>,
    projection: Option<Document>,
    aggregation_pipeline: Vec<Document>,
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            filter: doc! {},
            sort: None,
            limit: None,
            skip: None,
            projection: None,
            aggregation_pipeline: Vec::new(),
        }
    }

    /// Add a filter condition
    pub fn filter(mut self, key: &str, value: impl Into<Bson>) -> Self {
        self.filter.insert(key, value.into());
        self
    }

    /// Add multiple filter conditions
    pub fn filters(mut self, filters: Document) -> Self {
        for (key, value) in filters {
            self.filter.insert(key, value);
        }
        self
    }

    /// Add a range filter
    pub fn range<T: Into<Bson>>(mut self, key: &str, min: Option<T>, max: Option<T>) -> Self {
        let mut range_doc = doc! {};

        if let Some(min_val) = min {
            range_doc.insert("$gte", min_val.into());
        }

        if let Some(max_val) = max {
            range_doc.insert("$lte", max_val.into());
        }

        if !range_doc.is_empty() {
            self.filter.insert(key, range_doc);
        }

        self
    }

    /// Add text search
    pub fn text_search(mut self, text: &str) -> Self {
        self.filter.insert("$text", doc! {"$search": text});
        self
    }

    /// Add sorting
    pub fn sort(mut self, key: &str, ascending: bool) -> Self {
        let sort_value = if ascending { 1 } else { -1 };

        match &mut self.sort {
            Some(sort_doc) => {
                sort_doc.insert(key, sort_value);
            }
            None => {
                self.sort = Some(doc! {key: sort_value});
            }
        }

        self
    }

    /// Set limit
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set skip (offset)
    pub fn skip(mut self, skip: u64) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Set projection (select specific fields)
    pub fn project(mut self, fields: Vec<&str>) -> Self {
        let mut projection = doc! {};
        for field in fields {
            projection.insert(field, 1);
        }
        self.projection = Some(projection);
        self
    }

    /// Add aggregation stage
    pub fn aggregate(mut self, stage: Document) -> Self {
        self.aggregation_pipeline.push(stage);
        self
    }

    /// Add match stage to aggregation
    pub fn match_stage(mut self, filter: Document) -> Self {
        self.aggregation_pipeline.push(doc! {"$match": filter});
        self
    }

    /// Add group stage to aggregation
    pub fn group(mut self, group_doc: Document) -> Self {
        self.aggregation_pipeline.push(doc! {"$group": group_doc});
        self
    }

    /// Add lookup (join) stage
    pub fn lookup(
        mut self,
        from: &str,
        local_field: &str,
        foreign_field: &str,
        as_field: &str,
    ) -> Self {
        self.aggregation_pipeline.push(doc! {
            "$lookup": {
                "from": from,
                "localField": local_field,
                "foreignField": foreign_field,
                "as": as_field
            }
        });
        self
    }

    /// Build find options
    pub fn build_find_options(&self) -> FindOptions {
        let mut options = FindOptions::default();

        if let Some(sort) = &self.sort {
            options.sort = Some(sort.clone());
        }

        if let Some(limit) = self.limit {
            options.limit = Some(limit);
        }

        if let Some(skip) = self.skip {
            options.skip = Some(skip);
        }

        if let Some(projection) = &self.projection {
            options.projection = Some(projection.clone());
        }

        options
    }

    /// Get filter document
    pub fn get_filter(&self) -> &Document {
        &self.filter
    }

    /// Get aggregation pipeline
    pub fn get_pipeline(&self) -> &Vec<Document> {
        &self.aggregation_pipeline
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Advanced repository with query builder support
#[allow(dead_code)]
pub struct AdvancedMongoRepository<T> {
    collection: Collection<T>,
    db_manager: Arc<AdvancedDatabaseManager>,
}

impl<T> AdvancedMongoRepository<T>
where
    T: Serialize + DeserializeOwned + Unpin + Send + Sync + 'static,
{
    /// Create a new advanced repository
    pub fn new(db_manager: Arc<AdvancedDatabaseManager>, collection_name: &str) -> Self {
        // Get database from manager (this is simplified - in practice you'd handle this better)
        let database = db_manager
            .primary_client
            .database(&db_manager.database_name);
        let collection = database.collection(collection_name);

        Self {
            collection,
            db_manager,
        }
    }

    /// Find with query builder
    pub async fn find_with_query(&self, query: QueryBuilder) -> Result<Vec<T>> {
        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();
        let filter = query.get_filter().clone();
        let options = query.build_find_options();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();
                let filter = filter.clone();
                let options = options.clone();

                Box::pin(async move {
                    let mut cursor = collection
                        .find(filter, options)
                        .await
                        .map_err(|e| AppError::DatabaseError(format!("Query failed: {}", e)))?;

                    let mut results = Vec::new();
                    while cursor.advance().await.map_err(|e| {
                        AppError::DatabaseError(format!("Cursor advance failed: {}", e))
                    })? {
                        let entity = cursor.deserialize_current().map_err(|e| {
                            AppError::DatabaseError(format!("Deserialization failed: {}", e))
                        })?;
                        results.push(entity);
                    }
                    Ok(results)
                })
            })
            .await
    }

    /// Execute aggregation pipeline
    pub async fn aggregate(&self, query: QueryBuilder) -> Result<Vec<Document>> {
        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();
        let pipeline = query.get_pipeline().clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();
                let pipeline = pipeline.clone();

                Box::pin(async move {
                    let mut cursor = collection.aggregate(pipeline, None).await.map_err(|e| {
                        AppError::DatabaseError(format!("Aggregation failed: {}", e))
                    })?;

                    let mut results = Vec::new();
                    while cursor.advance().await.map_err(|e| {
                        AppError::DatabaseError(format!("Cursor advance failed: {}", e))
                    })? {
                        let doc = cursor.deserialize_current().map_err(|e| {
                            AppError::DatabaseError(format!("Deserialization failed: {}", e))
                        })?;
                        results.push(doc);
                    }
                    Ok(results)
                })
            })
            .await
    }

    /// Bulk insert with retry
    pub async fn bulk_insert(&self, entities: Vec<T>) -> Result<u64>
    where
        T: Clone,
    {
        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();
                let entities = entities.clone();

                Box::pin(async move {
                    let result = collection.insert_many(entities, None).await.map_err(|e| {
                        AppError::DatabaseError(format!("Bulk insert failed: {}", e))
                    })?;
                    Ok(result.inserted_ids.len() as u64)
                })
            })
            .await
    }

    /// Count with query
    pub async fn count_with_query(&self, query: QueryBuilder) -> Result<u64> {
        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();
        let filter = query.get_filter().clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();
                let filter = filter.clone();

                Box::pin(async move {
                    collection
                        .count_documents(filter, None)
                        .await
                        .map_err(|e| AppError::DatabaseError(format!("Count failed: {}", e)))
                })
            })
            .await
    }
}

/// Database migration system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub description: String,
    pub applied_at: Option<chrono::DateTime<chrono::Utc>>,
    pub checksum: String,
}

#[async_trait]
#[allow(dead_code)]
pub trait MigrationRunner: Send + Sync {
    /// Apply the migration
    async fn up(&self, database: &Database) -> Result<()>;

    /// Rollback the migration
    async fn down(&self, database: &Database) -> Result<()>;

    /// Get migration metadata
    fn metadata(&self) -> Migration;
}

/// Migration manager
#[allow(dead_code)]
pub struct MigrationManager {
    database: Database,
    migrations: Vec<Box<dyn MigrationRunner>>,
}

impl MigrationManager {
    /// Create a new migration manager
    pub fn new(database: Database) -> Self {
        Self {
            database,
            migrations: Vec::new(),
        }
    }

    /// Add a migration
    pub fn add_migration(&mut self, migration: Box<dyn MigrationRunner>) {
        self.migrations.push(migration);
    }

    /// Run pending migrations
    pub async fn migrate(&self) -> Result<Vec<String>> {
        info!("🔄 Running database migrations...");

        // Ensure migrations collection exists
        self.ensure_migrations_collection().await?;

        // Get applied migrations
        let applied = self.get_applied_migrations().await?;
        let applied_ids: std::collections::HashSet<String> = applied.into_iter().collect();

        let mut applied_migrations = Vec::new();

        // Sort migrations by version
        let mut sorted_migrations: Vec<_> = self.migrations.iter().collect();
        sorted_migrations.sort_by_key(|m| m.metadata().version);

        for migration in sorted_migrations {
            let metadata = migration.metadata();

            if !applied_ids.contains(&metadata.id) {
                info!("Applying migration: {} - {}", metadata.id, metadata.name);

                // Apply migration
                migration.up(&self.database).await?;

                // Record migration
                self.record_migration(&metadata).await?;
                applied_migrations.push(metadata.id);

                info!("✅ Migration applied: {}", metadata.name);
            }
        }

        if applied_migrations.is_empty() {
            info!("✅ No pending migrations");
        } else {
            info!("✅ Applied {} migrations", applied_migrations.len());
        }

        Ok(applied_migrations)
    }

    /// Ensure migrations collection exists
    async fn ensure_migrations_collection(&self) -> Result<()> {
        let collection: Collection<Migration> = self.database.collection("_migrations");

        // Create index on migration ID
        let index = mongodb::IndexModel::builder()
            .keys(doc! {"id": 1})
            .options(
                mongodb::options::IndexOptions::builder()
                    .unique(true)
                    .build(),
            )
            .build();

        collection.create_index(index, None).await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to create migration index: {}", e))
        })?;

        Ok(())
    }

    /// Get applied migrations
    async fn get_applied_migrations(&self) -> Result<Vec<String>> {
        let collection: Collection<Migration> = self.database.collection("_migrations");

        let mut cursor = collection
            .find(doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to query migrations: {}", e)))?;

        let mut migrations = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Cursor advance failed: {}", e)))?
        {
            let migration = cursor
                .deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Deserialization failed: {}", e)))?;
            migrations.push(migration.id);
        }

        Ok(migrations)
    }

    /// Record applied migration
    async fn record_migration(&self, metadata: &Migration) -> Result<()> {
        let collection: Collection<Migration> = self.database.collection("_migrations");

        let mut migration = metadata.clone();
        migration.applied_at = Some(chrono::Utc::now());

        collection
            .insert_one(migration, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to record migration: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let query = QueryBuilder::new()
            .filter("status", "active")
            .range("age", Some(18), Some(65))
            .sort("created_at", false)
            .limit(10)
            .skip(20);

        let filter = query.get_filter();
        assert!(filter.contains_key("status"));
        assert!(filter.contains_key("age"));

        let options = query.build_find_options();
        assert_eq!(options.limit, Some(10));
        assert_eq!(options.skip, Some(20));
    }

    #[test]
    fn test_query_builder_aggregation() {
        let query = QueryBuilder::new()
            .match_stage(doc! {"status": "active"})
            .group(doc! {
                "_id": "$department",
                "count": {"$sum": 1}
            })
            .lookup("departments", "_id", "_id", "department_info");

        let pipeline = query.get_pipeline();
        assert_eq!(pipeline.len(), 3);
        assert!(pipeline[0].contains_key("$match"));
        assert!(pipeline[1].contains_key("$group"));
        assert!(pipeline[2].contains_key("$lookup"));
    }

    #[tokio::test]
    async fn test_connection_pool_config() {
        let config = ConnectionPoolConfig::default();
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.max_connections, 100);
        assert!(config.connection_timeout > Duration::from_secs(0));
    }
}
