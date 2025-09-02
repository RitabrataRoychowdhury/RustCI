use std::sync::Arc;
use tracing::{info, warn};

use crate::storage::{
    RedisAdapter, StoreAdapter, StoreConfig, StoreError, StoreMode, StoreResult, ValKeyAdapter,
    YggdrasilAdapter,
};

/// Factory for creating storage adapters based on configuration
pub struct StoreFactory;

impl StoreFactory {
    /// Create a storage adapter based on the configuration
    pub async fn create_adapter(config: &StoreConfig) -> StoreResult<Arc<dyn StoreAdapter>> {
        match config.mode {
            StoreMode::Redis => {
                let redis_config = config.redis.as_ref().ok_or_else(|| {
                    StoreError::ConfigurationError("Redis configuration missing".to_string())
                })?;
                
                info!("Creating Redis adapter");
                let adapter = RedisAdapter::new(redis_config.clone()).await?;
                Ok(Arc::new(adapter))
            }
            StoreMode::ValKey => {
                let valkey_config = config.valkey.as_ref().ok_or_else(|| {
                    StoreError::ConfigurationError("ValKey configuration missing".to_string())
                })?;
                
                info!("Creating ValKey adapter");
                let adapter = ValKeyAdapter::new(valkey_config.clone()).await?;
                Ok(Arc::new(adapter))
            }
            StoreMode::Yggdrasil => {
                let yggdrasil_config = config.yggdrasil.as_ref().ok_or_else(|| {
                    StoreError::ConfigurationError("Yggdrasil configuration missing".to_string())
                })?;
                
                if yggdrasil_config.enabled {
                    return Err(StoreError::ConfigurationError(
                        "Yggdrasil is not yet implemented. Please use Redis or ValKey.".to_string(),
                    ));
                }
                
                warn!("Creating Yggdrasil adapter (placeholder only)");
                let adapter = YggdrasilAdapter::new(yggdrasil_config.clone()).await?;
                Ok(Arc::new(adapter))
            }
        }
    }

    /// Create a fallback chain of adapters
    pub async fn create_fallback_chain(
        config: &StoreConfig,
    ) -> StoreResult<Vec<Arc<dyn StoreAdapter>>> {
        let mut adapters = Vec::new();

        // Create primary adapter
        let primary = Self::create_adapter(config).await?;
        adapters.push(primary);

        // Create fallback adapters
        for fallback_mode in &config.fallback_chain {
            if *fallback_mode == config.mode {
                continue; // Skip primary mode
            }

            let fallback_config = StoreConfig {
                mode: fallback_mode.clone(),
                fallback_chain: vec![], // Prevent infinite recursion
                redis: config.redis.clone(),
                valkey: config.valkey.clone(),
                yggdrasil: config.yggdrasil.clone(),
                timeouts: config.timeouts.clone(),
                retry: config.retry.clone(),
            };

            match Self::create_adapter(&fallback_config).await {
                Ok(adapter) => {
                    info!("Created fallback adapter: {}", adapter.backend_name());
                    adapters.push(adapter);
                }
                Err(e) => {
                    warn!("Failed to create fallback adapter for {:?}: {}", fallback_mode, e);
                }
            }
        }

        if adapters.is_empty() {
            return Err(StoreError::ConfigurationError(
                "No storage adapters could be created".to_string(),
            ));
        }

        info!("Created {} storage adapters in fallback chain", adapters.len());
        Ok(adapters)
    }

    /// Validate storage configuration
    pub fn validate_config(config: &StoreConfig) -> StoreResult<()> {
        // Check that the primary mode has corresponding configuration
        match config.mode {
            StoreMode::Redis => {
                if config.redis.is_none() {
                    return Err(StoreError::ConfigurationError(
                        "Redis mode selected but no Redis configuration provided".to_string(),
                    ));
                }
            }
            StoreMode::ValKey => {
                if config.valkey.is_none() {
                    return Err(StoreError::ConfigurationError(
                        "ValKey mode selected but no ValKey configuration provided".to_string(),
                    ));
                }
            }
            StoreMode::Yggdrasil => {
                if config.yggdrasil.is_none() {
                    return Err(StoreError::ConfigurationError(
                        "Yggdrasil mode selected but no Yggdrasil configuration provided".to_string(),
                    ));
                }
                
                if config.yggdrasil.as_ref().unwrap().enabled {
                    return Err(StoreError::ConfigurationError(
                        "Yggdrasil is not yet implemented. Please use Redis or ValKey.".to_string(),
                    ));
                }
            }
        }

        // Validate fallback chain
        for fallback_mode in &config.fallback_chain {
            match fallback_mode {
                StoreMode::Redis => {
                    if config.redis.is_none() {
                        return Err(StoreError::ConfigurationError(
                            "Redis in fallback chain but no Redis configuration provided".to_string(),
                        ));
                    }
                }
                StoreMode::ValKey => {
                    if config.valkey.is_none() {
                        return Err(StoreError::ConfigurationError(
                            "ValKey in fallback chain but no ValKey configuration provided".to_string(),
                        ));
                    }
                }
                StoreMode::Yggdrasil => {
                    if config.yggdrasil.is_none() {
                        return Err(StoreError::ConfigurationError(
                            "Yggdrasil in fallback chain but no Yggdrasil configuration provided".to_string(),
                        ));
                    }
                }
            }
        }

        // Validate timeout values
        if config.timeouts.default_timeout_ms == 0 {
            return Err(StoreError::ConfigurationError(
                "Default timeout must be greater than 0".to_string(),
            ));
        }

        if config.timeouts.connection_timeout_ms == 0 {
            return Err(StoreError::ConfigurationError(
                "Connection timeout must be greater than 0".to_string(),
            ));
        }

        // Validate retry configuration
        if config.retry.max_retries > 10 {
            warn!("High retry count configured: {}", config.retry.max_retries);
        }

        if config.retry.backoff_multiplier <= 1.0 {
            return Err(StoreError::ConfigurationError(
                "Backoff multiplier must be greater than 1.0".to_string(),
            ));
        }

        if config.retry.jitter_factor < 0.0 || config.retry.jitter_factor > 1.0 {
            return Err(StoreError::ConfigurationError(
                "Jitter factor must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Hybrid storage manager that handles fallback between adapters
pub struct HybridStoreManager {
    adapters: Vec<Arc<dyn StoreAdapter>>,
    config: StoreConfig,
}

impl HybridStoreManager {
    /// Create a new hybrid store manager
    pub async fn new(config: StoreConfig) -> StoreResult<Self> {
        StoreFactory::validate_config(&config)?;
        let adapters = StoreFactory::create_fallback_chain(&config).await?;

        Ok(Self { adapters, config })
    }

    /// Execute an operation with fallback logic
    pub async fn execute_with_fallback<F, T>(&self, operation: F) -> StoreResult<T>
    where
        F: Fn(Arc<dyn StoreAdapter>) -> std::pin::Pin<Box<dyn std::future::Future<Output = StoreResult<T>> + Send>> + Send + Sync,
        T: Send,
    {
        let mut last_error = None;

        for (index, adapter) in self.adapters.iter().enumerate() {
            match operation(Arc::clone(adapter)).await {
                Ok(result) => {
                    if index > 0 {
                        info!("Operation succeeded using fallback adapter: {}", adapter.backend_name());
                    }
                    return Ok(result);
                }
                Err(e) => {
                    warn!("Operation failed on adapter {}: {}", adapter.backend_name(), e);
                    last_error = Some(e);
                    
                    // Don't retry on certain error types
                    if !e.is_retryable() {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            StoreError::InternalError("No adapters available".to_string())
        }))
    }

    /// Get the primary adapter
    pub fn primary_adapter(&self) -> &Arc<dyn StoreAdapter> {
        &self.adapters[0]
    }

    /// Get all adapters
    pub fn all_adapters(&self) -> &[Arc<dyn StoreAdapter>] {
        &self.adapters
    }

    /// Check health of all adapters
    pub async fn health_check_all(&self) -> Vec<(String, StoreResult<crate::storage::HealthStatus>)> {
        let mut results = Vec::new();

        for adapter in &self.adapters {
            let health = adapter.health_check().await;
            results.push((adapter.backend_name().to_string(), health));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{RedisConfig, StoreConfig, StoreMode};

    #[tokio::test]
    async fn test_validate_config_valid() {
        let config = StoreConfig {
            mode: StoreMode::Redis,
            fallback_chain: vec![StoreMode::Redis],
            redis: Some(RedisConfig::default()),
            valkey: None,
            yggdrasil: None,
            timeouts: crate::storage::TimeoutConfig::default(),
            retry: crate::storage::RetryConfig::default(),
        };

        assert!(StoreFactory::validate_config(&config).is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_missing_redis() {
        let config = StoreConfig {
            mode: StoreMode::Redis,
            fallback_chain: vec![],
            redis: None,
            valkey: None,
            yggdrasil: None,
            timeouts: crate::storage::TimeoutConfig::default(),
            retry: crate::storage::RetryConfig::default(),
        };

        assert!(StoreFactory::validate_config(&config).is_err());
    }

    #[tokio::test]
    async fn test_validate_config_yggdrasil_enabled() {
        let mut yggdrasil_config = crate::storage::YggdrasilConfig::default();
        yggdrasil_config.enabled = true;

        let config = StoreConfig {
            mode: StoreMode::Yggdrasil,
            fallback_chain: vec![],
            redis: None,
            valkey: None,
            yggdrasil: Some(yggdrasil_config),
            timeouts: crate::storage::TimeoutConfig::default(),
            retry: crate::storage::RetryConfig::default(),
        };

        assert!(StoreFactory::validate_config(&config).is_err());
    }
}