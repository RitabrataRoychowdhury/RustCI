//! Core traits and interfaces for the system
//!
//! This module defines the fundamental traits that form the contracts
//! between different layers of the application.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::Result;

/// Generic repository trait for data access
#[async_trait]
pub trait Repository<T, ID>: Send + Sync {
    /// Find an entity by its ID
    async fn find_by_id(&self, id: ID) -> Result<Option<T>>;

    /// Find all entities
    async fn find_all(&self) -> Result<Vec<T>>;

    /// Find entities with pagination
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<T>>;

    /// Save an entity (insert or update)
    async fn save(&self, entity: &T) -> Result<T>;

    /// Delete an entity by ID
    async fn delete(&self, id: ID) -> Result<()>;

    /// Check if an entity exists
    async fn exists(&self, id: ID) -> Result<bool>;

    /// Count total entities
    async fn count(&self) -> Result<u64>;

    /// Find entities by criteria
    async fn find_by_criteria(&self, criteria: HashMap<String, String>) -> Result<Vec<T>>;
}

/// Unit of Work pattern for managing transactions
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Begin a new transaction
    async fn begin_transaction(&mut self) -> Result<()>;

    /// Commit the current transaction
    async fn commit(&mut self) -> Result<()>;

    /// Rollback the current transaction
    async fn rollback(&mut self) -> Result<()>;

    /// Check if a transaction is active
    fn is_transaction_active(&self) -> bool;

    /// Get a repository instance within this unit of work
    fn get_repository<T, ID>(&self, repo_type: &str) -> Result<Arc<dyn Repository<T, ID>>>;
}

/// Service trait for business logic components
#[async_trait]
pub trait Service: Send + Sync {
    /// Get the service name
    fn service_name(&self) -> &'static str;

    /// Initialize the service
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    /// Shutdown the service gracefully
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// Health check for the service
    async fn health_check(&self) -> Result<ServiceHealth>;
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub service_name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub details: HashMap<String, String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Factory trait for creating instances
pub trait Factory<T>: Send + Sync {
    /// Create a new instance
    fn create(&self) -> Result<T>;

    /// Create an instance with configuration
    fn create_with_config(&self, config: HashMap<String, String>) -> Result<T>;

    /// Check if this factory can create the requested type
    fn can_create(&self, type_name: &str) -> bool;
}

/// Validator trait for input validation
#[async_trait]
pub trait Validator<T>: Send + Sync {
    /// Validate the input
    async fn validate(&self, input: &T) -> Result<ValidationResult>;

    /// Get validator name
    fn validator_name(&self) -> &'static str;
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub code: String,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failed validation result
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Add an error to the result
    pub fn add_error(&mut self, field: &str, message: &str, code: &str) {
        self.errors.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
            code: code.to_string(),
        });
        self.is_valid = false;
    }

    /// Add a warning to the result
    pub fn add_warning(&mut self, field: &str, message: &str, code: &str) {
        self.warnings.push(ValidationWarning {
            field: field.to_string(),
            message: message.to_string(),
            code: code.to_string(),
        });
    }
}

/// Cache trait for caching implementations
#[async_trait]
pub trait Cache: Send + Sync {
    /// Get a value from cache
    async fn get(&self, key: &str) -> Result<Option<String>>;

    /// Set a value in cache with TTL
    async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()>;

    /// Delete a value from cache
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if a key exists in cache
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Clear all cache entries
    async fn clear(&self) -> Result<()>;

    /// Get cache statistics
    async fn stats(&self) -> Result<CacheStats>;
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_keys: u64,
    pub memory_usage: u64,
    pub hit_rate: f64,
    pub miss_rate: f64,
}

/// Metrics collector trait
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a counter metric
    async fn increment_counter(&self, name: &str, value: u64, tags: HashMap<String, String>);

    /// Record a gauge metric
    async fn record_gauge(&self, name: &str, value: f64, tags: HashMap<String, String>);

    /// Record a histogram metric
    async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>);

    /// Record a timer metric
    async fn record_timer(&self, name: &str, duration_ms: u64, tags: HashMap<String, String>);

    /// Get collector name
    fn collector_name(&self) -> &'static str;
}

/// Event store trait for event sourcing
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append events to the store
    async fn append_events(&self, stream_id: &str, events: Vec<StoredEvent>) -> Result<()>;

    /// Read events from a stream
    async fn read_events(&self, stream_id: &str, from_version: u64) -> Result<Vec<StoredEvent>>;

    /// Read all events from a stream
    async fn read_all_events(&self, stream_id: &str) -> Result<Vec<StoredEvent>>;

    /// Get the current version of a stream
    async fn get_stream_version(&self, stream_id: &str) -> Result<u64>;

    /// Check if a stream exists
    async fn stream_exists(&self, stream_id: &str) -> Result<bool>;
}

/// Stored event for event sourcing
#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub event_id: Uuid,
    pub stream_id: String,
    pub event_type: String,
    pub event_data: String, // JSON serialized
    pub metadata: HashMap<String, String>,
    pub version: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Configuration provider trait
#[async_trait]
pub trait ConfigurationProvider: Send + Sync {
    /// Get a configuration value
    async fn get_string(&self, key: &str) -> Result<Option<String>>;

    /// Get a configuration value with default
    async fn get_string_or_default(&self, key: &str, default: &str) -> Result<String>;

    /// Get an integer configuration value
    async fn get_int(&self, key: &str) -> Result<Option<i64>>;

    /// Get a boolean configuration value
    async fn get_bool(&self, key: &str) -> Result<Option<bool>>;

    /// Get all configuration values with a prefix
    async fn get_with_prefix(&self, prefix: &str) -> Result<HashMap<String, String>>;

    /// Reload configuration
    async fn reload(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_success() {
        let result = ValidationResult::success();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validation_result_failure() {
        let errors = vec![ValidationError {
            field: "name".to_string(),
            message: "Name is required".to_string(),
            code: "REQUIRED".to_string(),
        }];

        let result = ValidationResult::failure(errors);
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_validation_result_add_error() {
        let mut result = ValidationResult::success();
        result.add_error("email", "Invalid email format", "INVALID_FORMAT");

        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].field, "email");
    }

    #[test]
    fn test_validation_result_add_warning() {
        let mut result = ValidationResult::success();
        result.add_warning("password", "Password is weak", "WEAK_PASSWORD");

        assert!(result.is_valid); // Warnings don't make result invalid
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].field, "password");
    }

    #[test]
    fn test_health_status() {
        let health = ServiceHealth {
            service_name: "test-service".to_string(),
            status: HealthStatus::Healthy,
            message: Some("All systems operational".to_string()),
            details: HashMap::new(),
            checked_at: chrono::Utc::now(),
        };

        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.service_name, "test-service");
    }
}
