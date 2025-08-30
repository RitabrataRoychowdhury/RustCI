//! Resource Quota and Throttling System
//! 
//! Provides system-wide resource quotas and throttling, resource usage monitoring and alerting,
//! and resource allocation optimization and balancing.

use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex, Semaphore};
use tokio::time::{interval, sleep, Instant};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

/// Resource types that can be managed by quotas
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Memory,
    CPU,
    Disk,
    Network,
    Connections,
    Requests,
    Files,
    Custom(String),
}

/// Resource quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub resource_type: ResourceType,
    pub max_allocation: u64,
    pub warning_threshold: u64,
    pub critical_threshold: u64,
    pub time_window: Duration,
    pub burst_allowance: Option<u64>,
    pub recovery_rate: Option<u64>, // Units per second
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            resource_type: ResourceType::Custom("default".to_string()),
            max_allocation: 1000,
            warning_threshold: 800,
            critical_threshold: 950,
            time_window: Duration::from_secs(60),
            burst_allowance: Some(100),
            recovery_rate: Some(10),
        }
    }
}

/// Resource allocation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationRequest {
    pub id: Uuid,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub priority: AllocationPriority,
    pub requester_id: String,
    pub timeout: Option<Duration>,
    pub tags: HashMap<String, String>,
}

/// Allocation priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AllocationPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Resource allocation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationResult {
    pub request_id: Uuid,
    pub success: bool,
    pub allocated_amount: u64,
    pub allocation_id: Option<Uuid>,
    pub wait_time: Duration,
    pub expires_at: Option<SystemTime>,
    pub error_message: Option<String>,
}

/// Current resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub resource_type: ResourceType,
    pub current_usage: u64,
    pub peak_usage: u64,
    pub average_usage: f64,
    pub allocation_count: usize,
    pub last_updated: SystemTime,
}

/// Resource allocation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub id: Uuid,
    pub request_id: Uuid,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub requester_id: String,
    pub allocated_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub priority: AllocationPriority,
    pub tags: HashMap<String, String>,
}

/// Throttling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottlingConfig {
    pub resource_type: ResourceType,
    pub rate_limit: u64, // Operations per time window
    pub time_window: Duration,
    pub burst_size: u64,
    pub queue_size: usize,
    pub backoff_strategy: BackoffStrategy,
}

/// Backoff strategies for throttling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Linear { increment: Duration },
    Exponential { base: f64, max_delay: Duration },
    Fixed { delay: Duration },
    Custom(String),
}

/// Resource monitoring alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAlert {
    pub id: Uuid,
    pub resource_type: ResourceType,
    pub alert_type: AlertType,
    pub current_usage: u64,
    pub threshold: u64,
    pub message: String,
    pub triggered_at: SystemTime,
    pub severity: AlertSeverity,
}

/// Alert types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertType {
    QuotaExceeded,
    WarningThreshold,
    CriticalThreshold,
    AllocationFailed,
    ThrottlingActive,
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info = 1,
    Warning = 2,
    Critical = 3,
    Emergency = 4,
}

/// Resource quota and throttling statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaStats {
    pub total_requests: u64,
    pub successful_allocations: u64,
    pub failed_allocations: u64,
    pub throttled_requests: u64,
    pub average_wait_time: Duration,
    pub peak_usage_by_resource: HashMap<ResourceType, u64>,
    pub active_allocations: usize,
    pub alerts_triggered: u64,
}

/// Trait for resource monitors
#[async_trait::async_trait]
pub trait ResourceMonitor: Send + Sync {
    async fn get_current_usage(&self, resource_type: &ResourceType) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
    fn get_monitor_name(&self) -> &str;
}

/// Trait for alert handlers
#[async_trait::async_trait]
pub trait AlertHandler: Send + Sync {
    async fn handle_alert(&self, alert: &ResourceAlert) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn can_handle(&self, alert_type: &AlertType) -> bool;
    fn get_handler_name(&self) -> &str;
}

/// Resource quota and throttling manager
pub struct ResourceQuotaManager {
    quotas: Arc<RwLock<HashMap<ResourceType, ResourceQuota>>>,
    throttling_configs: Arc<RwLock<HashMap<ResourceType, ThrottlingConfig>>>,
    allocations: Arc<RwLock<HashMap<Uuid, ResourceAllocation>>>,
    usage_tracking: Arc<RwLock<HashMap<ResourceType, ResourceUsage>>>,
    rate_limiters: Arc<RwLock<HashMap<ResourceType, Arc<Semaphore>>>>,
    monitors: Arc<RwLock<Vec<Arc<dyn ResourceMonitor>>>>,
    alert_handlers: Arc<RwLock<Vec<Arc<dyn AlertHandler>>>>,
    stats: Arc<Mutex<QuotaStats>>,
    is_running: Arc<Mutex<bool>>,
}

impl ResourceQuotaManager {
    /// Create a new resource quota manager
    pub fn new() -> Self {
        Self {
            quotas: Arc::new(RwLock::new(HashMap::new())),
            throttling_configs: Arc::new(RwLock::new(HashMap::new())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            usage_tracking: Arc::new(RwLock::new(HashMap::new())),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            monitors: Arc::new(RwLock::new(Vec::new())),
            alert_handlers: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(Mutex::new(QuotaStats::default())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Set resource quota
    pub async fn set_quota(&self, quota: ResourceQuota) {
        let resource_type = quota.resource_type.clone();
        let mut quotas = self.quotas.write().await;
        quotas.insert(resource_type.clone(), quota);
        info!("Set quota for resource type: {:?}", resource_type);
    }

    /// Set throttling configuration
    pub async fn set_throttling_config(&self, config: ThrottlingConfig) {
        let resource_type = config.resource_type.clone();
        
        // Create rate limiter semaphore
        let semaphore = Arc::new(Semaphore::new(config.burst_size as usize));
        
        {
            let mut configs = self.throttling_configs.write().await;
            configs.insert(resource_type.clone(), config);
        }
        
        {
            let mut limiters = self.rate_limiters.write().await;
            limiters.insert(resource_type.clone(), semaphore);
        }
        
        info!("Set throttling config for resource type: {:?}", resource_type);
    }

    /// Register a resource monitor
    pub async fn register_monitor(&self, monitor: Arc<dyn ResourceMonitor>) {
        let mut monitors = self.monitors.write().await;
        monitors.push(monitor);
    }

    /// Register an alert handler
    pub async fn register_alert_handler(&self, handler: Arc<dyn AlertHandler>) {
        let mut handlers = self.alert_handlers.write().await;
        handlers.push(handler);
    }

    /// Request resource allocation
    pub async fn request_allocation(&self, request: AllocationRequest) -> AllocationResult {
        let start_time = Instant::now();
        let request_id = request.id;
        
        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.total_requests += 1;
        }

        // Check throttling first
        if let Err(e) = self.check_throttling(&request.resource_type).await {
            let mut stats = self.stats.lock().await;
            stats.throttled_requests += 1;
            
            return AllocationResult {
                request_id,
                success: false,
                allocated_amount: 0,
                allocation_id: None,
                wait_time: start_time.elapsed(),
                expires_at: None,
                error_message: Some(format!("Request throttled: {}", e)),
            };
        }

        // Check quota availability
        match self.check_quota_availability(&request.resource_type, request.amount).await {
            Ok(()) => {
                // Allocate resource
                let allocation_id = Uuid::new_v4();
                let expires_at = request.timeout.map(|t| SystemTime::now() + t);
                let allocation = ResourceAllocation {
                    id: allocation_id,
                    request_id,
                    resource_type: request.resource_type.clone(),
                    amount: request.amount,
                    requester_id: request.requester_id,
                    allocated_at: SystemTime::now(),
                    expires_at,
                    priority: request.priority,
                    tags: request.tags,
                };

                // Store allocation
                {
                    let mut allocations = self.allocations.write().await;
                    allocations.insert(allocation_id, allocation);
                }

                // Update usage tracking
                self.update_usage_tracking(&request.resource_type, request.amount, true).await;

                // Update stats
                {
                    let mut stats = self.stats.lock().await;
                    stats.successful_allocations += 1;
                    stats.active_allocations += 1;
                    
                    let wait_time = start_time.elapsed();
                    let total_successful = stats.successful_allocations;
                    if total_successful > 0 {
                        let current_avg_nanos = stats.average_wait_time.as_nanos() as u64;
                        let new_wait_nanos = wait_time.as_nanos() as u64;
                        let new_avg_nanos = (current_avg_nanos * (total_successful - 1) + new_wait_nanos) / total_successful;
                        stats.average_wait_time = Duration::from_nanos(new_avg_nanos);
                    }
                }

                // Check for alerts
                self.check_and_trigger_alerts(&request.resource_type).await;

                AllocationResult {
                    request_id,
                    success: true,
                    allocated_amount: request.amount,
                    allocation_id: Some(allocation_id),
                    wait_time: start_time.elapsed(),
                    expires_at,
                    error_message: None,
                }
            }
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.failed_allocations += 1;
                
                AllocationResult {
                    request_id,
                    success: false,
                    allocated_amount: 0,
                    allocation_id: None,
                    wait_time: start_time.elapsed(),
                    expires_at: None,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    /// Release resource allocation
    pub async fn release_allocation(&self, allocation_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let allocation = {
            let mut allocations = self.allocations.write().await;
            allocations.remove(&allocation_id)
        };

        if let Some(allocation) = allocation {
            // Update usage tracking
            self.update_usage_tracking(&allocation.resource_type, allocation.amount, false).await;

            // Update stats
            {
                let mut stats = self.stats.lock().await;
                if stats.active_allocations > 0 {
                    stats.active_allocations -= 1;
                }
            }

            info!("Released allocation {} for {} units of {:?}", 
                  allocation_id, allocation.amount, allocation.resource_type);
            Ok(())
        } else {
            Err(format!("Allocation {} not found", allocation_id).into())
        }
    }

    /// Get current resource usage
    pub async fn get_resource_usage(&self, resource_type: &ResourceType) -> Option<ResourceUsage> {
        let usage_tracking = self.usage_tracking.read().await;
        usage_tracking.get(resource_type).cloned()
    }

    /// Get all active allocations
    pub async fn get_active_allocations(&self) -> Vec<ResourceAllocation> {
        let allocations = self.allocations.read().await;
        allocations.values().cloned().collect()
    }

    /// Get allocations by requester
    pub async fn get_allocations_by_requester(&self, requester_id: &str) -> Vec<ResourceAllocation> {
        let allocations = self.allocations.read().await;
        allocations.values()
            .filter(|a| a.requester_id == requester_id)
            .cloned()
            .collect()
    }

    /// Get quota statistics
    pub async fn get_stats(&self) -> QuotaStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }

    /// Start monitoring and cleanup processes
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        // Start monitoring task
        let monitors = Arc::clone(&self.monitors);
        let usage_tracking = Arc::clone(&self.usage_tracking);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::monitoring_loop(monitors, usage_tracking, is_running).await;
        });

        // Start cleanup task
        let allocations = Arc::clone(&self.allocations);
        let usage_tracking = Arc::clone(&self.usage_tracking);
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::cleanup_loop(allocations, usage_tracking, stats, is_running).await;
        });

        info!("Started resource quota manager");
        Ok(())
    }

    /// Stop monitoring and cleanup processes
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("Stopped resource quota manager");
    }

    /// Check throttling limits
    async fn check_throttling(&self, resource_type: &ResourceType) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let limiters = self.rate_limiters.read().await;
        if let Some(semaphore) = limiters.get(resource_type) {
            // Try to acquire permit (non-blocking)
            match semaphore.try_acquire() {
                Ok(_permit) => {
                    // Permit acquired, request can proceed
                    // Note: In a real implementation, we would need to manage permit release
                    Ok(())
                }
                Err(_) => {
                    Err("Rate limit exceeded".into())
                }
            }
        } else {
            // No throttling configured for this resource type
            Ok(())
        }
    }

    /// Check quota availability
    async fn check_quota_availability(&self, resource_type: &ResourceType, amount: u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let quotas = self.quotas.read().await;
        if let Some(quota) = quotas.get(resource_type) {
            let usage_tracking = self.usage_tracking.read().await;
            let current_usage = usage_tracking.get(resource_type)
                .map(|u| u.current_usage)
                .unwrap_or(0);

            if current_usage + amount > quota.max_allocation {
                Err(format!("Quota exceeded: {} + {} > {}", current_usage, amount, quota.max_allocation).into())
            } else {
                Ok(())
            }
        } else {
            // No quota configured, allow allocation
            Ok(())
        }
    }

    /// Update usage tracking
    async fn update_usage_tracking(&self, resource_type: &ResourceType, amount: u64, is_allocation: bool) {
        let mut usage_tracking = self.usage_tracking.write().await;
        let usage = usage_tracking.entry(resource_type.clone()).or_insert_with(|| {
            ResourceUsage {
                resource_type: resource_type.clone(),
                current_usage: 0,
                peak_usage: 0,
                average_usage: 0.0,
                allocation_count: 0,
                last_updated: SystemTime::now(),
            }
        });

        if is_allocation {
            usage.current_usage += amount;
            usage.allocation_count += 1;
            if usage.current_usage > usage.peak_usage {
                usage.peak_usage = usage.current_usage;
            }
        } else {
            usage.current_usage = usage.current_usage.saturating_sub(amount);
            if usage.allocation_count > 0 {
                usage.allocation_count -= 1;
            }
        }

        // Update average usage (simple moving average)
        usage.average_usage = (usage.average_usage + usage.current_usage as f64) / 2.0;
        usage.last_updated = SystemTime::now();
    }

    /// Check and trigger alerts
    async fn check_and_trigger_alerts(&self, resource_type: &ResourceType) {
        let quotas = self.quotas.read().await;
        let usage_tracking = self.usage_tracking.read().await;
        
        if let (Some(quota), Some(usage)) = (quotas.get(resource_type), usage_tracking.get(resource_type)) {
            let mut alerts = Vec::new();

            // Check warning threshold
            if usage.current_usage >= quota.warning_threshold && usage.current_usage < quota.critical_threshold {
                alerts.push(ResourceAlert {
                    id: Uuid::new_v4(),
                    resource_type: resource_type.clone(),
                    alert_type: AlertType::WarningThreshold,
                    current_usage: usage.current_usage,
                    threshold: quota.warning_threshold,
                    message: format!("Resource usage warning: {} >= {}", usage.current_usage, quota.warning_threshold),
                    triggered_at: SystemTime::now(),
                    severity: AlertSeverity::Warning,
                });
            }

            // Check critical threshold
            if usage.current_usage >= quota.critical_threshold {
                alerts.push(ResourceAlert {
                    id: Uuid::new_v4(),
                    resource_type: resource_type.clone(),
                    alert_type: AlertType::CriticalThreshold,
                    current_usage: usage.current_usage,
                    threshold: quota.critical_threshold,
                    message: format!("Resource usage critical: {} >= {}", usage.current_usage, quota.critical_threshold),
                    triggered_at: SystemTime::now(),
                    severity: AlertSeverity::Critical,
                });
            }

            drop(quotas);
            drop(usage_tracking);

            // Send alerts to handlers
            for alert in alerts {
                self.send_alert(alert).await;
            }
        }
    }

    /// Send alert to handlers
    async fn send_alert(&self, alert: ResourceAlert) {
        let handlers = self.alert_handlers.read().await;
        
        for handler in handlers.iter() {
            if handler.can_handle(&alert.alert_type) {
                if let Err(e) = handler.handle_alert(&alert).await {
                    error!("Alert handler {} failed: {}", handler.get_handler_name(), e);
                }
            }
        }

        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.alerts_triggered += 1;
        }

        debug!("Triggered alert: {:?}", alert.alert_type);
    }

    /// Monitoring loop
    async fn monitoring_loop(
        monitors: Arc<RwLock<Vec<Arc<dyn ResourceMonitor>>>>,
        usage_tracking: Arc<RwLock<HashMap<ResourceType, ResourceUsage>>>,
        is_running: Arc<Mutex<bool>>,
    ) {
        let mut monitoring_interval = interval(Duration::from_secs(30));
        
        while {
            let running = is_running.lock().await;
            *running
        } {
            monitoring_interval.tick().await;
            
            let monitors_vec = monitors.read().await;
            for monitor in monitors_vec.iter() {
                // In a real implementation, we would query specific resource types
                // For now, we just demonstrate the monitoring capability
                debug!("Running monitor: {}", monitor.get_monitor_name());
            }
        }
    }

    /// Cleanup loop for expired allocations
    async fn cleanup_loop(
        allocations: Arc<RwLock<HashMap<Uuid, ResourceAllocation>>>,
        usage_tracking: Arc<RwLock<HashMap<ResourceType, ResourceUsage>>>,
        stats: Arc<Mutex<QuotaStats>>,
        is_running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(60));
        
        while {
            let running = is_running.lock().await;
            *running
        } {
            cleanup_interval.tick().await;
            
            let now = SystemTime::now();
            let mut expired_allocations = Vec::new();
            
            // Find expired allocations
            {
                let allocations_map = allocations.read().await;
                for (id, allocation) in allocations_map.iter() {
                    if let Some(expires_at) = allocation.expires_at {
                        if now > expires_at {
                            expired_allocations.push((*id, allocation.clone()));
                        }
                    }
                }
            }

            // Remove expired allocations
            if !expired_allocations.is_empty() {
                {
                    let mut allocations_map = allocations.write().await;
                    for (id, _) in &expired_allocations {
                        allocations_map.remove(id);
                    }
                }

                // Update usage tracking
                {
                    let mut usage_map = usage_tracking.write().await;
                    for (_, allocation) in &expired_allocations {
                        if let Some(usage) = usage_map.get_mut(&allocation.resource_type) {
                            usage.current_usage = usage.current_usage.saturating_sub(allocation.amount);
                            if usage.allocation_count > 0 {
                                usage.allocation_count -= 1;
                            }
                        }
                    }
                }

                // Update stats
                {
                    let mut stats_guard = stats.lock().await;
                    stats_guard.active_allocations = stats_guard.active_allocations.saturating_sub(expired_allocations.len());
                }

                info!("Cleaned up {} expired allocations", expired_allocations.len());
            }
        }
    }
}

impl Default for ResourceQuotaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Default alert handler that logs alerts
pub struct LoggingAlertHandler;

#[async_trait::async_trait]
impl AlertHandler for LoggingAlertHandler {
    async fn handle_alert(&self, alert: &ResourceAlert) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match alert.severity {
            AlertSeverity::Info => info!("Resource Alert: {}", alert.message),
            AlertSeverity::Warning => warn!("Resource Alert: {}", alert.message),
            AlertSeverity::Critical => error!("Resource Alert: {}", alert.message),
            AlertSeverity::Emergency => error!("EMERGENCY Resource Alert: {}", alert.message),
        }
        Ok(())
    }

    fn can_handle(&self, _alert_type: &AlertType) -> bool {
        true // Can handle all alert types
    }

    fn get_handler_name(&self) -> &str {
        "LoggingAlertHandler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_resource_quota_manager_creation() {
        let manager = ResourceQuotaManager::new();
        let stats = manager.get_stats().await;
        
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_allocations, 0);
        assert_eq!(stats.failed_allocations, 0);
        assert_eq!(stats.active_allocations, 0);
    }

    #[tokio::test]
    async fn test_quota_setting_and_allocation() {
        let manager = ResourceQuotaManager::new();
        
        // Set quota
        let quota = ResourceQuota {
            resource_type: ResourceType::Memory,
            max_allocation: 1000,
            warning_threshold: 800,
            critical_threshold: 950,
            time_window: Duration::from_secs(60),
            burst_allowance: Some(100),
            recovery_rate: Some(10),
        };
        manager.set_quota(quota).await;
        
        // Request allocation
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Memory,
            amount: 500,
            priority: AllocationPriority::Normal,
            requester_id: "test_user".to_string(),
            timeout: Some(Duration::from_secs(300)),
            tags: HashMap::new(),
        };
        
        let result = manager.request_allocation(request).await;
        assert!(result.success);
        assert_eq!(result.allocated_amount, 500);
        assert!(result.allocation_id.is_some());
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.successful_allocations, 1);
        assert_eq!(stats.active_allocations, 1);
    }

    #[tokio::test]
    async fn test_quota_exceeded() {
        let manager = ResourceQuotaManager::new();
        
        // Set small quota
        let quota = ResourceQuota {
            resource_type: ResourceType::CPU,
            max_allocation: 100,
            warning_threshold: 80,
            critical_threshold: 95,
            time_window: Duration::from_secs(60),
            burst_allowance: Some(10),
            recovery_rate: Some(5),
        };
        manager.set_quota(quota).await;
        
        // Request allocation that exceeds quota
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::CPU,
            amount: 150,
            priority: AllocationPriority::Normal,
            requester_id: "test_user".to_string(),
            timeout: None,
            tags: HashMap::new(),
        };
        
        let result = manager.request_allocation(request).await;
        assert!(!result.success);
        assert_eq!(result.allocated_amount, 0);
        assert!(result.error_message.is_some());
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.failed_allocations, 1);
        assert_eq!(stats.active_allocations, 0);
    }

    #[tokio::test]
    async fn test_allocation_release() {
        let manager = ResourceQuotaManager::new();
        
        let quota = ResourceQuota {
            resource_type: ResourceType::Disk,
            max_allocation: 1000,
            ..Default::default()
        };
        manager.set_quota(quota).await;
        
        // Allocate resource
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Disk,
            amount: 300,
            priority: AllocationPriority::High,
            requester_id: "test_user".to_string(),
            timeout: None,
            tags: HashMap::new(),
        };
        
        let result = manager.request_allocation(request).await;
        assert!(result.success);
        let allocation_id = result.allocation_id.unwrap();
        
        // Release allocation
        let release_result = manager.release_allocation(allocation_id).await;
        assert!(release_result.is_ok());
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_allocations, 0);
    }

    #[tokio::test]
    async fn test_throttling_configuration() {
        let manager = ResourceQuotaManager::new();
        
        let throttling_config = ThrottlingConfig {
            resource_type: ResourceType::Requests,
            rate_limit: 10,
            time_window: Duration::from_secs(1),
            burst_size: 5,
            queue_size: 100,
            backoff_strategy: BackoffStrategy::Fixed { delay: Duration::from_millis(100) },
        };
        
        manager.set_throttling_config(throttling_config).await;
        
        // Make multiple requests to test throttling
        let mut successful_requests = 0;
        let mut throttled_requests = 0;
        
        for i in 0..10 {
            let request = AllocationRequest {
                id: Uuid::new_v4(),
                resource_type: ResourceType::Requests,
                amount: 1,
                priority: AllocationPriority::Normal,
                requester_id: format!("user_{}", i),
                timeout: None,
                tags: HashMap::new(),
            };
            
            let result = manager.request_allocation(request).await;
            if result.success {
                successful_requests += 1;
            } else {
                throttled_requests += 1;
            }
        }
        
        // Some requests should be throttled
        assert!(throttled_requests > 0);
        assert!(successful_requests > 0);
    }
}