use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub allocation_id: Uuid,
    pub service_name: String,
    pub cpu_cores: f64,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub network_bandwidth_mbps: f64,
    pub allocated_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub priority: ResourcePriority,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourcePriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePool {
    pub pool_id: Uuid,
    pub name: String,
    pub total_cpu_cores: f64,
    pub total_memory_gb: f64,
    pub total_storage_gb: f64,
    pub total_network_bandwidth_mbps: f64,
    pub available_cpu_cores: f64,
    pub available_memory_gb: f64,
    pub available_storage_gb: f64,
    pub available_network_bandwidth_mbps: f64,
    pub allocations: Vec<Uuid>,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub request_id: Uuid,
    pub service_name: String,
    pub cpu_cores: f64,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub network_bandwidth_mbps: f64,
    pub duration: Option<Duration>,
    pub priority: ResourcePriority,
    pub requirements: ResourceRequirements,
    pub requested_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub min_cpu_cores: f64,
    pub min_memory_gb: f64,
    pub min_storage_gb: f64,
    pub min_network_bandwidth_mbps: f64,
    pub preferred_zone: Option<String>,
    pub anti_affinity_services: Vec<String>,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMonitoringData {
    pub service_name: String,
    pub allocation_id: Uuid,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub storage_usage_percent: f64,
    pub network_usage_percent: f64,
    pub iops: u64,
    pub network_latency_ms: f64,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceOptimizationRecommendation {
    pub recommendation_id: Uuid,
    pub service_name: String,
    pub recommendation_type: OptimizationType,
    pub current_allocation: ResourceAllocation,
    pub recommended_allocation: ResourceAllocation,
    pub potential_savings: f64,
    pub confidence_score: f64,
    pub implementation_complexity: ImplementationComplexity,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationType {
    Downsize,
    Upsize,
    Rebalance,
    Consolidate,
    Migrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationComplexity {
    Simple,
    Moderate,
    Complex,
}

pub trait ResourceManager: Send + Sync {
    async fn allocate_resources(&self, request: ResourceRequest) -> Result<ResourceAllocation>;
    async fn deallocate_resources(&self, allocation_id: Uuid) -> Result<()>;
    async fn update_allocation(&self, allocation_id: Uuid, new_resources: ResourceRequest) -> Result<ResourceAllocation>;
    async fn get_allocation(&self, allocation_id: Uuid) -> Result<ResourceAllocation>;
    async fn list_allocations(&self, service: Option<&str>) -> Result<Vec<ResourceAllocation>>;
    async fn get_resource_usage(&self, allocation_id: Uuid) -> Result<ResourceMonitoringData>;
}

pub trait ResourceOptimizer: Send + Sync {
    async fn analyze_resource_usage(&self, service: &str, duration: Duration) -> Result<Vec<ResourceOptimizationRecommendation>>;
    async fn optimize_allocation(&self, allocation_id: Uuid) -> Result<ResourceOptimizationRecommendation>;
    async fn get_efficiency_score(&self, service: &str) -> Result<f64>;
    async fn predict_resource_needs(&self, service: &str, forecast_duration: Duration) -> Result<ResourceRequest>;
}

#[async_trait::async_trait]
pub trait ResourceMonitor: Send + Sync {
    async fn start_monitoring(&self, allocation_id: Uuid) -> Result<()>;
    async fn stop_monitoring(&self, allocation_id: Uuid) -> Result<()>;
    async fn get_monitoring_data(&self, allocation_id: Uuid, duration: Duration) -> Result<Vec<ResourceMonitoringData>>;
    async fn set_alert_thresholds(&self, allocation_id: Uuid, thresholds: ResourceThresholds) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceThresholds {
    pub cpu_warning: f64,
    pub cpu_critical: f64,
    pub memory_warning: f64,
    pub memory_critical: f64,
    pub storage_warning: f64,
    pub storage_critical: f64,
    pub network_warning: f64,
    pub network_critical: f64,
}

impl Default for ResourceThresholds {
    fn default() -> Self {
        Self {
            cpu_warning: 70.0,
            cpu_critical: 90.0,
            memory_warning: 80.0,
            memory_critical: 95.0,
            storage_warning: 85.0,
            storage_critical: 95.0,
            network_warning: 80.0,
            network_critical: 95.0,
        }
    }
}

pub struct ProductionResourceManager {
    resource_pools: Arc<RwLock<HashMap<Uuid, ResourcePool>>>,
    allocations: Arc<RwLock<HashMap<Uuid, ResourceAllocation>>>,
    monitoring_data: Arc<RwLock<HashMap<Uuid, Vec<ResourceMonitoringData>>>>,
    optimization_recommendations: Arc<RwLock<HashMap<String, Vec<ResourceOptimizationRecommendation>>>>,
}

impl ProductionResourceManager {
    pub fn new() -> Self {
        Self {
            resource_pools: Arc::new(RwLock::new(HashMap::new())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            monitoring_data: Arc::new(RwLock::new(HashMap::new())),
            optimization_recommendations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_resource_pool(&self, pool: ResourcePool) -> Result<Uuid> {
        let pool_id = pool.pool_id;
        let mut pools = self.resource_pools.write().await;
        pools.insert(pool_id, pool);
        log::info!("Created resource pool: {}", pool_id);
        Ok(pool_id)
    }

    pub async fn remove_resource_pool(&self, pool_id: Uuid) -> Result<()> {
        let mut pools = self.resource_pools.write().await;
        pools.remove(&pool_id);
        log::info!("Removed resource pool: {}", pool_id);
        Ok(())
    }

    async fn find_suitable_pool(&self, request: &ResourceRequest) -> Result<Uuid> {
        let pools = self.resource_pools.read().await;
        
        for (pool_id, pool) in pools.iter() {
            if pool.available_cpu_cores >= request.cpu_cores
                && pool.available_memory_gb >= request.memory_gb
                && pool.available_storage_gb >= request.storage_gb
                && pool.available_network_bandwidth_mbps >= request.network_bandwidth_mbps
            {
                return Ok(*pool_id);
            }
        }
        
        Err(AppError::ResourceExhausted(
            "No suitable resource pool found for the request".to_string()
        ))
    }

    async fn update_pool_availability(&self, pool_id: Uuid, allocation: &ResourceAllocation, allocate: bool) -> Result<()> {
        let mut pools = self.resource_pools.write().await;
        
        if let Some(pool) = pools.get_mut(&pool_id) {
            let multiplier = if allocate { -1.0 } else { 1.0 };
            
            pool.available_cpu_cores += allocation.cpu_cores * multiplier;
            pool.available_memory_gb += allocation.memory_gb * multiplier;
            pool.available_storage_gb += allocation.storage_gb * multiplier;
            pool.available_network_bandwidth_mbps += allocation.network_bandwidth_mbps * multiplier;
            
            if allocate {
                pool.allocations.push(allocation.allocation_id);
            } else {
                pool.allocations.retain(|&id| id != allocation.allocation_id);
            }
        }
        
        Ok(())
    }

    async fn cleanup_expired_allocations(&self) -> Result<()> {
        let now = SystemTime::now();
        let mut expired_allocations = Vec::new();
        
        {
            let allocations = self.allocations.read().await;
            for (allocation_id, allocation) in allocations.iter() {
                if let Some(expires_at) = allocation.expires_at {
                    if now > expires_at {
                        expired_allocations.push(*allocation_id);
                    }
                }
            }
        }
        
        for allocation_id in expired_allocations {
            self.deallocate_resources(allocation_id).await?;
            log::info!("Cleaned up expired allocation: {}", allocation_id);
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl ResourceManager for ProductionResourceManager {
    async fn allocate_resources(&self, request: ResourceRequest) -> Result<ResourceAllocation> {
        // Clean up expired allocations first
        self.cleanup_expired_allocations().await?;
        
        // Find suitable resource pool
        let pool_id = self.find_suitable_pool(&request).await?;
        
        // Create allocation
        let allocation = ResourceAllocation {
            allocation_id: request.request_id,
            service_name: request.service_name.clone(),
            cpu_cores: request.cpu_cores,
            memory_gb: request.memory_gb,
            storage_gb: request.storage_gb,
            network_bandwidth_mbps: request.network_bandwidth_mbps,
            allocated_at: SystemTime::now(),
            expires_at: request.duration.map(|d| SystemTime::now() + d),
            priority: request.priority,
            tags: HashMap::new(),
        };
        
        // Update pool availability
        self.update_pool_availability(pool_id, &allocation, true).await?;
        
        // Store allocation
        {
            let mut allocations = self.allocations.write().await;
            allocations.insert(allocation.allocation_id, allocation.clone());
        }
        
        log::info!("Allocated resources for service {}: {} CPU, {} GB RAM, {} GB storage", 
                  allocation.service_name, allocation.cpu_cores, allocation.memory_gb, allocation.storage_gb);
        
        Ok(allocation)
    }

    async fn deallocate_resources(&self, allocation_id: Uuid) -> Result<()> {
        let allocation = {
            let mut allocations = self.allocations.write().await;
            allocations.remove(&allocation_id)
                .ok_or_else(|| AppError::NotFound(format!("Allocation {} not found", allocation_id)))?
        };
        
        // Find the pool that contains this allocation
        let pool_id_to_update = {
            let pools = self.resource_pools.read().await;
            let mut found_pool_id = None;
            for (pool_id, pool) in pools.iter() {
                if pool.allocations.contains(&allocation_id) {
                    found_pool_id = Some(*pool_id);
                    break;
                }
            }
            found_pool_id
        };
        
        if let Some(pool_id) = pool_id_to_update {
            self.update_pool_availability(pool_id, &allocation, false).await?;
        }
        
        // Clean up monitoring data
        {
            let mut monitoring = self.monitoring_data.write().await;
            monitoring.remove(&allocation_id);
        }
        
        log::info!("Deallocated resources for allocation: {}", allocation_id);
        Ok(())
    }

    async fn update_allocation(&self, allocation_id: Uuid, new_request: ResourceRequest) -> Result<ResourceAllocation> {
        // Get current allocation
        let current_allocation = self.get_allocation(allocation_id).await?;
        
        // Deallocate current resources
        self.deallocate_resources(allocation_id).await?;
        
        // Try to allocate new resources
        match self.allocate_resources(new_request).await {
            Ok(new_allocation) => Ok(new_allocation),
            Err(e) => {
                // If new allocation fails, restore the old one
                let restore_request = ResourceRequest {
                    request_id: current_allocation.allocation_id,
                    service_name: current_allocation.service_name.clone(),
                    cpu_cores: current_allocation.cpu_cores,
                    memory_gb: current_allocation.memory_gb,
                    storage_gb: current_allocation.storage_gb,
                    network_bandwidth_mbps: current_allocation.network_bandwidth_mbps,
                    duration: current_allocation.expires_at.map(|exp| {
                        exp.duration_since(SystemTime::now()).unwrap_or(Duration::from_secs(3600))
                    }),
                    priority: current_allocation.priority,
                    requirements: ResourceRequirements {
                        min_cpu_cores: current_allocation.cpu_cores,
                        min_memory_gb: current_allocation.memory_gb,
                        min_storage_gb: current_allocation.storage_gb,
                        min_network_bandwidth_mbps: current_allocation.network_bandwidth_mbps,
                        preferred_zone: None,
                        anti_affinity_services: Vec::new(),
                        required_capabilities: Vec::new(),
                    },
                    requested_at: SystemTime::now(),
                };
                
                let _ = self.allocate_resources(restore_request).await;
                Err(e)
            }
        }
    }

    async fn get_allocation(&self, allocation_id: Uuid) -> Result<ResourceAllocation> {
        let allocations = self.allocations.read().await;
        allocations.get(&allocation_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Allocation {} not found", allocation_id)))
    }

    async fn list_allocations(&self, service: Option<&str>) -> Result<Vec<ResourceAllocation>> {
        let allocations = self.allocations.read().await;
        
        let filtered_allocations: Vec<ResourceAllocation> = allocations.values()
            .filter(|allocation| {
                service.map_or(true, |s| allocation.service_name == s)
            })
            .cloned()
            .collect();
        
        Ok(filtered_allocations)
    }

    async fn get_resource_usage(&self, allocation_id: Uuid) -> Result<ResourceMonitoringData> {
        let monitoring = self.monitoring_data.read().await;
        
        if let Some(data_points) = monitoring.get(&allocation_id) {
            if let Some(latest) = data_points.last() {
                return Ok(latest.clone());
            }
        }
        
        // Return default monitoring data if no data available
        let allocation = self.get_allocation(allocation_id).await?;
        Ok(ResourceMonitoringData {
            service_name: allocation.service_name,
            allocation_id,
            cpu_usage_percent: 0.0,
            memory_usage_percent: 0.0,
            storage_usage_percent: 0.0,
            network_usage_percent: 0.0,
            iops: 0,
            network_latency_ms: 0.0,
            timestamp: SystemTime::now(),
        })
    }
}

pub struct ProductionResourceOptimizer {
    resource_manager: Arc<dyn ResourceManager>,
}

impl ProductionResourceOptimizer {
    pub fn new(resource_manager: Arc<dyn ResourceManager>) -> Self {
        Self { resource_manager }
    }

    async fn calculate_efficiency_score(&self, usage_data: &[ResourceMonitoringData]) -> f64 {
        if usage_data.is_empty() {
            return 0.0;
        }

        let avg_cpu = usage_data.iter().map(|d| d.cpu_usage_percent).sum::<f64>() / usage_data.len() as f64;
        let avg_memory = usage_data.iter().map(|d| d.memory_usage_percent).sum::<f64>() / usage_data.len() as f64;
        let avg_storage = usage_data.iter().map(|d| d.storage_usage_percent).sum::<f64>() / usage_data.len() as f64;
        let avg_network = usage_data.iter().map(|d| d.network_usage_percent).sum::<f64>() / usage_data.len() as f64;

        // Calculate efficiency as the average utilization (higher is better, but not over 90%)
        let overall_utilization = (avg_cpu + avg_memory + avg_storage + avg_network) / 4.0;
        
        // Optimal utilization is around 70-80%
        if overall_utilization < 30.0 {
            overall_utilization / 100.0 * 0.5 // Low efficiency for underutilized resources
        } else if overall_utilization > 90.0 {
            0.9 - (overall_utilization - 90.0) / 100.0 // Penalize over-utilization
        } else {
            overall_utilization / 100.0
        }
    }
}

#[async_trait::async_trait]
impl ResourceOptimizer for ProductionResourceOptimizer {
    async fn analyze_resource_usage(&self, service: &str, duration: Duration) -> Result<Vec<ResourceOptimizationRecommendation>> {
        let allocations = self.resource_manager.list_allocations(Some(service)).await?;
        let mut recommendations = Vec::new();
        
        for allocation in allocations {
            // Simulate usage analysis (in a real implementation, this would analyze historical data)
            let current_usage = self.resource_manager.get_resource_usage(allocation.allocation_id).await?;
            
            // Generate recommendation based on usage patterns
            let recommendation_type = if current_usage.cpu_usage_percent < 30.0 && current_usage.memory_usage_percent < 30.0 {
                OptimizationType::Downsize
            } else if current_usage.cpu_usage_percent > 80.0 || current_usage.memory_usage_percent > 80.0 {
                OptimizationType::Upsize
            } else {
                OptimizationType::Rebalance
            };

            let recommended_allocation = match recommendation_type {
                OptimizationType::Downsize => ResourceAllocation {
                    cpu_cores: allocation.cpu_cores * 0.7,
                    memory_gb: allocation.memory_gb * 0.7,
                    ..allocation.clone()
                },
                OptimizationType::Upsize => ResourceAllocation {
                    cpu_cores: allocation.cpu_cores * 1.5,
                    memory_gb: allocation.memory_gb * 1.5,
                    ..allocation.clone()
                },
                _ => allocation.clone(),
            };

            let potential_savings = match recommendation_type {
                OptimizationType::Downsize => (allocation.cpu_cores - recommended_allocation.cpu_cores) * 10.0,
                OptimizationType::Upsize => -((recommended_allocation.cpu_cores - allocation.cpu_cores) * 10.0),
                _ => 0.0,
            };

            recommendations.push(ResourceOptimizationRecommendation {
                recommendation_id: Uuid::new_v4(),
                service_name: service.to_string(),
                recommendation_type,
                current_allocation: allocation,
                recommended_allocation,
                potential_savings,
                confidence_score: 0.8,
                implementation_complexity: ImplementationComplexity::Moderate,
                created_at: SystemTime::now(),
            });
        }
        
        Ok(recommendations)
    }

    async fn optimize_allocation(&self, allocation_id: Uuid) -> Result<ResourceOptimizationRecommendation> {
        let allocation = self.resource_manager.get_allocation(allocation_id).await?;
        let usage = self.resource_manager.get_resource_usage(allocation_id).await?;
        
        // Simple optimization logic
        let cpu_ratio = usage.cpu_usage_percent / 100.0;
        let memory_ratio = usage.memory_usage_percent / 100.0;
        
        let optimal_cpu = allocation.cpu_cores * cpu_ratio * 1.2; // 20% buffer
        let optimal_memory = allocation.memory_gb * memory_ratio * 1.2;
        
        let recommended_allocation = ResourceAllocation {
            cpu_cores: optimal_cpu.max(0.5), // Minimum 0.5 cores
            memory_gb: optimal_memory.max(1.0), // Minimum 1GB
            ..allocation.clone()
        };
        
        let potential_savings = (allocation.cpu_cores - recommended_allocation.cpu_cores) * 10.0;
        
        Ok(ResourceOptimizationRecommendation {
            recommendation_id: Uuid::new_v4(),
            service_name: allocation.service_name.clone(),
            recommendation_type: if potential_savings > 0.0 { OptimizationType::Downsize } else { OptimizationType::Upsize },
            current_allocation: allocation,
            recommended_allocation,
            potential_savings,
            confidence_score: 0.75,
            implementation_complexity: ImplementationComplexity::Simple,
            created_at: SystemTime::now(),
        })
    }

    async fn get_efficiency_score(&self, service: &str) -> Result<f64> {
        let allocations = self.resource_manager.list_allocations(Some(service)).await?;
        
        if allocations.is_empty() {
            return Ok(0.0);
        }
        
        let mut total_efficiency = 0.0;
        let mut count = 0;
        
        for allocation in allocations {
            let usage = self.resource_manager.get_resource_usage(allocation.allocation_id).await?;
            let efficiency = self.calculate_efficiency_score(&[usage]).await;
            total_efficiency += efficiency;
            count += 1;
        }
        
        Ok(total_efficiency / count as f64)
    }

    async fn predict_resource_needs(&self, service: &str, forecast_duration: Duration) -> Result<ResourceRequest> {
        let allocations = self.resource_manager.list_allocations(Some(service)).await?;
        
        if allocations.is_empty() {
            return Err(AppError::NotFound(format!("No allocations found for service: {}", service)));
        }
        
        // Simple prediction based on current usage trends
        let mut total_cpu = 0.0;
        let mut total_memory = 0.0;
        let mut total_storage = 0.0;
        let mut total_network = 0.0;
        
        for allocation in &allocations {
            let usage = self.resource_manager.get_resource_usage(allocation.allocation_id).await?;
            
            // Project usage with a growth factor
            let growth_factor = 1.1; // 10% growth assumption
            total_cpu += allocation.cpu_cores * (usage.cpu_usage_percent / 100.0) * growth_factor;
            total_memory += allocation.memory_gb * (usage.memory_usage_percent / 100.0) * growth_factor;
            total_storage += allocation.storage_gb * (usage.storage_usage_percent / 100.0) * growth_factor;
            total_network += allocation.network_bandwidth_mbps * (usage.network_usage_percent / 100.0) * growth_factor;
        }
        
        Ok(ResourceRequest {
            request_id: Uuid::new_v4(),
            service_name: service.to_string(),
            cpu_cores: total_cpu,
            memory_gb: total_memory,
            storage_gb: total_storage,
            network_bandwidth_mbps: total_network,
            duration: Some(forecast_duration),
            priority: ResourcePriority::Medium,
            requirements: ResourceRequirements {
                min_cpu_cores: total_cpu * 0.8,
                min_memory_gb: total_memory * 0.8,
                min_storage_gb: total_storage * 0.8,
                min_network_bandwidth_mbps: total_network * 0.8,
                preferred_zone: None,
                anti_affinity_services: Vec::new(),
                required_capabilities: Vec::new(),
            },
            requested_at: SystemTime::now(),
        })
    }
}

impl Default for ProductionResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_allocation() {
        let manager = ProductionResourceManager::new();
        
        // Create a resource pool
        let pool = ResourcePool {
            pool_id: Uuid::new_v4(),
            name: "test-pool".to_string(),
            total_cpu_cores: 10.0,
            total_memory_gb: 20.0,
            total_storage_gb: 100.0,
            total_network_bandwidth_mbps: 1000.0,
            available_cpu_cores: 10.0,
            available_memory_gb: 20.0,
            available_storage_gb: 100.0,
            available_network_bandwidth_mbps: 1000.0,
            allocations: Vec::new(),
            created_at: SystemTime::now(),
        };
        
        manager.create_resource_pool(pool).await.unwrap();
        
        // Create a resource request
        let request = ResourceRequest {
            request_id: Uuid::new_v4(),
            service_name: "test-service".to_string(),
            cpu_cores: 2.0,
            memory_gb: 4.0,
            storage_gb: 10.0,
            network_bandwidth_mbps: 100.0,
            duration: Some(Duration::from_secs(3600)),
            priority: ResourcePriority::Medium,
            requirements: ResourceRequirements {
                min_cpu_cores: 1.0,
                min_memory_gb: 2.0,
                min_storage_gb: 5.0,
                min_network_bandwidth_mbps: 50.0,
                preferred_zone: None,
                anti_affinity_services: Vec::new(),
                required_capabilities: Vec::new(),
            },
            requested_at: SystemTime::now(),
        };
        
        // Allocate resources
        let allocation = manager.allocate_resources(request).await.unwrap();
        assert_eq!(allocation.cpu_cores, 2.0);
        assert_eq!(allocation.memory_gb, 4.0);
        
        // Get allocation
        let retrieved = manager.get_allocation(allocation.allocation_id).await.unwrap();
        assert_eq!(retrieved.service_name, "test-service");
        
        // List allocations
        let allocations = manager.list_allocations(Some("test-service")).await.unwrap();
        assert_eq!(allocations.len(), 1);
        
        // Deallocate resources
        manager.deallocate_resources(allocation.allocation_id).await.unwrap();
        
        let allocations = manager.list_allocations(Some("test-service")).await.unwrap();
        assert_eq!(allocations.len(), 0);
    }

    #[tokio::test]
    async fn test_resource_optimization() {
        let manager = Arc::new(ProductionResourceManager::new());
        let optimizer = ProductionResourceOptimizer::new(manager.clone());
        
        // Create a resource pool
        let pool = ResourcePool {
            pool_id: Uuid::new_v4(),
            name: "test-pool".to_string(),
            total_cpu_cores: 10.0,
            total_memory_gb: 20.0,
            total_storage_gb: 100.0,
            total_network_bandwidth_mbps: 1000.0,
            available_cpu_cores: 10.0,
            available_memory_gb: 20.0,
            available_storage_gb: 100.0,
            available_network_bandwidth_mbps: 1000.0,
            allocations: Vec::new(),
            created_at: SystemTime::now(),
        };
        
        manager.create_resource_pool(pool).await.unwrap();
        
        // Create and allocate resources
        let request = ResourceRequest {
            request_id: Uuid::new_v4(),
            service_name: "test-service".to_string(),
            cpu_cores: 4.0,
            memory_gb: 8.0,
            storage_gb: 20.0,
            network_bandwidth_mbps: 200.0,
            duration: None,
            priority: ResourcePriority::Medium,
            requirements: ResourceRequirements {
                min_cpu_cores: 2.0,
                min_memory_gb: 4.0,
                min_storage_gb: 10.0,
                min_network_bandwidth_mbps: 100.0,
                preferred_zone: None,
                anti_affinity_services: Vec::new(),
                required_capabilities: Vec::new(),
            },
            requested_at: SystemTime::now(),
        };
        
        let allocation = manager.allocate_resources(request).await.unwrap();
        
        // Get optimization recommendation
        let recommendation = optimizer.optimize_allocation(allocation.allocation_id).await.unwrap();
        assert_eq!(recommendation.service_name, "test-service");
        assert!(recommendation.confidence_score > 0.0);
        
        // Get efficiency score
        let efficiency = optimizer.get_efficiency_score("test-service").await.unwrap();
        assert!(efficiency >= 0.0 && efficiency <= 1.0);
    }

    #[tokio::test]
    async fn test_resource_prediction() {
        let manager = Arc::new(ProductionResourceManager::new());
        let optimizer = ProductionResourceOptimizer::new(manager.clone());
        
        // Create a resource pool and allocation (similar to previous test)
        let pool = ResourcePool {
            pool_id: Uuid::new_v4(),
            name: "test-pool".to_string(),
            total_cpu_cores: 10.0,
            total_memory_gb: 20.0,
            total_storage_gb: 100.0,
            total_network_bandwidth_mbps: 1000.0,
            available_cpu_cores: 10.0,
            available_memory_gb: 20.0,
            available_storage_gb: 100.0,
            available_network_bandwidth_mbps: 1000.0,
            allocations: Vec::new(),
            created_at: SystemTime::now(),
        };
        
        manager.create_resource_pool(pool).await.unwrap();
        
        let request = ResourceRequest {
            request_id: Uuid::new_v4(),
            service_name: "test-service".to_string(),
            cpu_cores: 2.0,
            memory_gb: 4.0,
            storage_gb: 10.0,
            network_bandwidth_mbps: 100.0,
            duration: None,
            priority: ResourcePriority::Medium,
            requirements: ResourceRequirements {
                min_cpu_cores: 1.0,
                min_memory_gb: 2.0,
                min_storage_gb: 5.0,
                min_network_bandwidth_mbps: 50.0,
                preferred_zone: None,
                anti_affinity_services: Vec::new(),
                required_capabilities: Vec::new(),
            },
            requested_at: SystemTime::now(),
        };
        
        manager.allocate_resources(request).await.unwrap();
        
        // Predict future resource needs
        let prediction = optimizer.predict_resource_needs("test-service", Duration::from_secs(86400)).await.unwrap();
        assert_eq!(prediction.service_name, "test-service");
        assert!(prediction.cpu_cores > 0.0);
        assert!(prediction.memory_gb > 0.0);
    }
}