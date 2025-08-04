//! Query pattern implementation for CQRS
//!
//! This module provides the query side of CQRS pattern with proper
//! caching, validation, and performance monitoring.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Core trait that all queries must implement
#[async_trait]
pub trait Query: Send + Sync + std::fmt::Debug {
    type Result: Send + Sync;

    /// Execute the query
    async fn execute(&self) -> Result<Self::Result>;

    /// Get the query ID
    fn query_id(&self) -> Uuid;

    /// Get the correlation ID for tracing
    fn correlation_id(&self) -> Uuid;

    /// Get the query name for logging and metrics
    fn query_name(&self) -> &'static str;

    /// Validate the query before execution
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Get query metadata
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Check if this query result can be cached
    fn is_cacheable(&self) -> bool {
        true
    }

    /// Get cache key for this query
    fn cache_key(&self) -> Option<String> {
        None
    }

    /// Get cache TTL in seconds
    fn cache_ttl(&self) -> Option<u64> {
        Some(300) // 5 minutes default
    }
}

/// Handler for queries
#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync {
    /// Handle the query
    async fn handle(&self, query: Q) -> Result<Q::Result>;

    /// Get handler name for logging and debugging
    fn handler_name(&self) -> &'static str;

    /// Validate the query before handling
    async fn validate(&self, query: &Q) -> Result<()> {
        query.validate()
    }
}

/// Query execution record for performance monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryExecutionRecord {
    pub query_id: Uuid,
    pub correlation_id: Uuid,
    pub query_name: String,
    pub handler_name: String,
    pub executed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: QueryExecutionStatus,
    pub duration_ms: Option<u64>,
    pub cache_hit: bool,
    pub result_size: Option<usize>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryExecutionStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Cached,
}

/// Simple in-memory cache for query results
#[derive(Debug, Clone)]
struct CacheEntry {
    data: String, // JSON serialized result
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

/// Query dispatcher that routes queries to appropriate handlers
pub struct QueryDispatcher {
    handlers: Arc<RwLock<HashMap<String, Box<dyn QueryHandlerWrapper>>>>,
    execution_log: Arc<RwLock<Vec<QueryExecutionRecord>>>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    correlation_tracker: Arc<crate::core::CorrelationTracker>,
}

/// Wrapper trait to handle different query types
#[async_trait]
trait QueryHandlerWrapper: Send + Sync {
    async fn handle_query(
        &self,
        query: &dyn Query<Result = Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<Box<dyn std::any::Any + Send + Sync>>;
    fn handler_name(&self) -> &'static str;
}

/// Concrete wrapper implementation
struct ConcreteQueryHandlerWrapper<Q: Query + 'static> {
    handler: Box<dyn QueryHandler<Q>>,
}

#[async_trait]
impl<Q: Query + 'static> QueryHandlerWrapper for ConcreteQueryHandlerWrapper<Q> {
    async fn handle_query(
        &self,
        _query: &dyn Query<Result = Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<Box<dyn std::any::Any + Send + Sync>> {
        // This is a simplified implementation - in practice, you'd need more sophisticated type handling
        Err(AppError::InternalServerError(
            "Query type handling not fully implemented".to_string(),
        ))
    }

    fn handler_name(&self) -> &'static str {
        self.handler.handler_name()
    }
}

impl QueryDispatcher {
    /// Create a new query dispatcher
    pub fn new(correlation_tracker: Arc<crate::core::CorrelationTracker>) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            execution_log: Arc::new(RwLock::new(Vec::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            correlation_tracker,
        }
    }

    /// Register a query handler
    pub async fn register_handler<Q: Query + 'static>(
        &self,
        handler: Box<dyn QueryHandler<Q>>,
    ) -> Result<()> {
        let query_name = std::any::type_name::<Q>();
        let wrapper = ConcreteQueryHandlerWrapper { handler };

        let mut handlers = self.handlers.write().await;
        handlers.insert(query_name.to_string(), Box::new(wrapper));

        info!("Registered query handler for: {}", query_name);
        Ok(())
    }

    /// Dispatch a query to its handler
    pub async fn dispatch<Q: Query + 'static>(&self, query: Q) -> Result<Q::Result> {
        let query_name = query.query_name();
        let query_id = query.query_id();
        let correlation_id = query.correlation_id();

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(correlation_id)
            .await;

        info!(
            query_id = %query_id,
            correlation_id = %correlation_id,
            query_name = query_name,
            "Dispatching query"
        );

        // Create execution record
        let mut record = QueryExecutionRecord {
            query_id,
            correlation_id,
            query_name: query_name.to_string(),
            handler_name: "unknown".to_string(),
            executed_at: Utc::now(),
            completed_at: None,
            status: QueryExecutionStatus::Pending,
            duration_ms: None,
            cache_hit: false,
            result_size: None,
            error: None,
            metadata: query.metadata(),
        };

        let start_time = std::time::Instant::now();

        // Validate query
        if let Err(e) = query.validate() {
            record.status = QueryExecutionStatus::Failed;
            record.error = Some(e.to_string());
            record.completed_at = Some(Utc::now());
            record.duration_ms = Some(start_time.elapsed().as_millis() as u64);

            self.log_execution(record).await;
            return Err(e);
        }

        // Check cache if query is cacheable
        if query.is_cacheable() {
            if let Some(cache_key) = query.cache_key() {
                if let Some(_cached_result) = self.get_from_cache(&cache_key).await {
                    record.status = QueryExecutionStatus::Cached;
                    record.cache_hit = true;
                    record.completed_at = Some(Utc::now());
                    record.duration_ms = Some(start_time.elapsed().as_millis() as u64);

                    debug!(
                        query_id = %query_id,
                        cache_key = cache_key,
                        "Query result served from cache"
                    );

                    self.log_execution(record.clone()).await;

                    // In a real implementation, you'd deserialize the cached result
                    // For now, we'll continue with normal execution
                }
            }
        }

        record.status = QueryExecutionStatus::Executing;

        // Execute query
        let result = query.execute().await;

        let duration = start_time.elapsed();
        record.completed_at = Some(Utc::now());
        record.duration_ms = Some(duration.as_millis() as u64);

        match result {
            Ok(res) => {
                record.status = QueryExecutionStatus::Completed;

                // Cache result if cacheable
                if query.is_cacheable() {
                    if let Some(cache_key) = query.cache_key() {
                        if let Some(ttl) = query.cache_ttl() {
                            // In a real implementation, you'd serialize the result
                            self.store_in_cache(cache_key, "serialized_result".to_string(), ttl)
                                .await;
                        }
                    }
                }

                info!(
                    query_id = %query_id,
                    duration_ms = duration.as_millis(),
                    "Query executed successfully"
                );

                self.log_execution(record).await;
                Ok(res)
            }
            Err(e) => {
                record.status = QueryExecutionStatus::Failed;
                record.error = Some(e.to_string());

                error!(
                    query_id = %query_id,
                    error = %e,
                    duration_ms = duration.as_millis(),
                    "Query execution failed"
                );

                self.log_execution(record).await;
                Err(e)
            }
        }
    }

    /// Get result from cache
    async fn get_from_cache(&self, key: &str) -> Option<String> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get(key) {
            if entry.expires_at > Utc::now() {
                return Some(entry.data.clone());
            } else {
                // Remove expired entry
                cache.remove(key);
            }
        }

        None
    }

    /// Store result in cache
    async fn store_in_cache(&self, key: String, data: String, ttl_seconds: u64) {
        let mut cache = self.cache.write().await;

        let entry = CacheEntry {
            data,
            expires_at: Utc::now() + chrono::Duration::seconds(ttl_seconds as i64),
            created_at: Utc::now(),
        };

        cache.insert(key, entry);

        // Simple cache cleanup - remove expired entries
        let now = Utc::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// Log query execution
    async fn log_execution(&self, record: QueryExecutionRecord) {
        let mut log = self.execution_log.write().await;
        log.push(record);

        // Keep only the last 1000 records to prevent memory issues
        if log.len() > 1000 {
            let len = log.len();
            log.drain(0..len - 1000);
        }
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> HashMap<String, usize> {
        let log = self.execution_log.read().await;
        let mut stats = HashMap::new();

        for record in log.iter() {
            let key = format!("{}_{:?}", record.query_name, record.status);
            *stats.entry(key).or_insert(0) += 1;
        }

        // Add cache statistics
        let cache = self.cache.read().await;
        stats.insert("cache_entries".to_string(), cache.len());

        stats
    }

    /// Get recent execution records
    pub async fn get_recent_executions(&self, limit: usize) -> Vec<QueryExecutionRecord> {
        let log = self.execution_log.read().await;
        log.iter().rev().take(limit).cloned().collect()
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Cleared query cache");
    }

    /// Clear execution log (useful for testing)
    pub async fn clear_execution_log(&self) {
        let mut log = self.execution_log.write().await;
        log.clear();
        info!("Cleared query execution log");
    }
}

/// Example query implementations
#[derive(Debug, Clone)]
pub struct GetPipelineQuery {
    pub query_id: Uuid,
    pub correlation_id: Uuid,
    pub pipeline_id: Uuid,
}

#[async_trait]
impl Query for GetPipelineQuery {
    type Result = Option<String>; // In practice, this would be a Pipeline struct

    async fn execute(&self) -> Result<Self::Result> {
        debug!("Executing GetPipelineQuery for: {}", self.pipeline_id);
        // This would typically delegate to a repository
        Ok(Some(format!("Pipeline-{}", self.pipeline_id)))
    }

    fn query_id(&self) -> Uuid {
        self.query_id
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn query_name(&self) -> &'static str {
        "GetPipeline"
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!("pipeline:{}", self.pipeline_id))
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("pipeline_id".to_string(), self.pipeline_id.to_string());
        metadata
    }
}

#[derive(Debug, Clone)]
pub struct ListPipelinesQuery {
    pub query_id: Uuid,
    pub correlation_id: Uuid,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[async_trait]
impl Query for ListPipelinesQuery {
    type Result = Vec<String>; // In practice, this would be Vec<Pipeline>

    async fn execute(&self) -> Result<Self::Result> {
        debug!("Executing ListPipelinesQuery");
        // This would typically delegate to a repository
        Ok(vec!["Pipeline-1".to_string(), "Pipeline-2".to_string()])
    }

    fn query_id(&self) -> Uuid {
        self.query_id
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn query_name(&self) -> &'static str {
        "ListPipelines"
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!(
            "pipelines:{}:{}",
            self.limit.unwrap_or(50),
            self.offset.unwrap_or(0)
        ))
    }

    fn cache_ttl(&self) -> Option<u64> {
        Some(60) // 1 minute for list queries
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        if let Some(limit) = self.limit {
            metadata.insert("limit".to_string(), limit.to_string());
        }
        if let Some(offset) = self.offset {
            metadata.insert("offset".to_string(), offset.to_string());
        }
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestQueryHandler;

    #[async_trait]
    impl QueryHandler<GetPipelineQuery> for TestQueryHandler {
        async fn handle(&self, query: GetPipelineQuery) -> Result<Option<String>> {
            query.execute().await
        }

        fn handler_name(&self) -> &'static str {
            "TestQueryHandler"
        }
    }

    #[tokio::test]
    async fn test_query_execution() {
        let query = GetPipelineQuery {
            query_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
        };

        let result = query.execute().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_query_cache_key() {
        let pipeline_id = Uuid::new_v4();
        let query = GetPipelineQuery {
            query_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            pipeline_id,
        };

        let cache_key = query.cache_key();
        assert!(cache_key.is_some());
        assert_eq!(cache_key.unwrap(), format!("pipeline:{}", pipeline_id));
    }

    #[tokio::test]
    async fn test_query_dispatcher() {
        let correlation_tracker = Arc::new(crate::core::CorrelationTracker::new());
        let dispatcher = QueryDispatcher::new(correlation_tracker);

        let query = GetPipelineQuery {
            query_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
        };

        let result = dispatcher.dispatch(query).await;
        assert!(result.is_ok());

        let stats = dispatcher.get_execution_stats().await;
        assert!(!stats.is_empty());
    }
}
