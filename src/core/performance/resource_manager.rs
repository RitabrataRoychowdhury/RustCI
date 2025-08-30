use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: u64,
    pub network_bandwidth: u64,
    pub priority: ResourcePriority,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceAllocation {
    pub allocation_id: Uuid,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: u64,
    pub network_bandwidth: u64,
    #[serde(skip)]
    pub allocated_at: Instant,
    #[serde(skip)]
    pub expires_at: Option<Instant>,
    pub priority: ResourcePriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePriority {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub max_cpu_cores: u32,
    pub max_memory_mb: u64,
    pub max_disk_gb: u64,
    pub max_network_bandwidth: u64,
    pub max_concurrent_allocations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub allocated_cpu_cores: u32,
    pub allocated_memory_mb: u64,
    pub allocated_disk_gb: u64,
    pub allocated_network_bandwidth: u64,
    pub active_allocations: u32,
    pub utilization_percentage: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationReport {
    pub recommendations: Vec<OptimizationRecommendation>,
    pub potential_savings: ResourceSavings,
    pub efficiency_score: f64,
    #[serde(skip)]
    pub generated_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub recommendation_type: RecommendationType,
    pub description: String,
    pub impact: ImpactLevel,
    pub estimated_savings: ResourceSavings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    ScaleDown,
    ScaleUp,
    Redistribute,
    Consolidate,
    Terminate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImpactLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSavings {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: u64,
    pub network_bandwidth: u64,
    pub cost_percentage: f64,
}

pub trait ResourceManager: Send + Sync {
    fn allocate_resources(&self, requirements: ResourceRequirements) -> impl std::future::Future<Output = Result<ResourceAllocation, AppError>> + Send;
    fn release_resources(&self, allocation: ResourceAllocation) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn optimize_resource_usage(&self) -> impl std::future::Future<Output = Result<OptimizationReport, AppError>> + Send;
    fn get_resource_usage(&self) -> impl std::future::Future<Output = Result<ResourceUsage, AppError>> + Send;
    fn set_resource_quota(&self, quota: ResourceQuota) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn get_available_resources(&self) -> impl std::future::Future<Output = Result<ResourceRequirements, AppError>> + Send;
}

pub struct ProductionResourceManager {
    quota: Arc<RwLock<ResourceQuota>>,
    allocations: Arc<RwLock<HashMap<Uuid, ResourceAllocation>>>,
    cpu_semaphore: Arc<Semaphore>,
    memory_semaphore: Arc<Semaphore>,
    disk_semaphore: Arc<Semaphore>,
    network_semaphore: Arc<Semaphore>,
    allocation_semaphore: Arc<Semaphore>,
}

impl ProductionResourceManager {
    pub fn new(quota: ResourceQuota) -> Self {
        Self {
            cpu_semaphore: Arc::new(Semaphore::new(quota.max_cpu_cores as usize)),
            memory_semaphore: Arc::new(Semaphore::new((quota.max_memory_mb / 100) as usize)), // 100MB units
            disk_semaphore: Arc::new(Semaphore::new(quota.max_disk_gb as usize)),
            network_semaphore: Arc::new(Semaphore::new((quota.max_network_bandwidth / 1000) as usize)), // 1Gbps units
            allocation_semaphore: Arc::new(Semaphore::new(quota.max_concurrent_allocations as usize)),
            quota: Arc::new(RwLock::new(quota)),
            allocations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn can_allocate(&self, requirements: &ResourceRequirements) -> Result<bool, AppError> {
        let quota = self.quota.read().await;
        let allocations = self.allocations.read().await;
        
        let current_usage = self.calculate_current_usage(&allocations).await;
        
        Ok(current_usage.allocated_cpu_cores + requirements.cpu_cores <= quota.max_cpu_cores
            && current_usage.allocated_memory_mb + requirements.memory_mb <= quota.max_memory_mb
            && current_usage.allocated_disk_gb + requirements.disk_gb <= quota.max_disk_gb
            && current_usage.allocated_network_bandwidth + requirements.network_bandwidth <= quota.max_network_bandwidth
            && current_usage.active_allocations < quota.max_concurrent_allocations)
    }

    async fn calculate_current_usage(&self, allocations: &HashMap<Uuid, ResourceAllocation>) -> ResourceUsage {
        let now = Instant::now();
        let active_allocations: Vec<&ResourceAllocation> = allocations
            .values()
            .filter(|alloc| alloc.expires_at.map_or(true, |exp| exp > now))
            .collect();

        let allocated_cpu_cores = active_allocations.iter().map(|a| a.cpu_cores).sum();
        let allocated_memory_mb = active_allocations.iter().map(|a| a.memory_mb).sum();
        let allocated_disk_gb = active_allocations.iter().map(|a| a.disk_gb).sum();
        let allocated_network_bandwidth = active_allocations.iter().map(|a| a.network_bandwidth).sum();

        let quota = self.quota.read().await;
        let utilization = (allocated_cpu_cores as f64 / quota.max_cpu_cores as f64) * 100.0;

        ResourceUsage {
            allocated_cpu_cores,
            allocated_memory_mb,
            allocated_disk_gb,
            allocated_network_bandwidth,
            active_allocations: active_allocations.len() as u32,
            utilization_percentage: utilization,
        }
    }

    async fn cleanup_expired_allocations(&self) -> Result<(), AppError> {
        let mut allocations = self.allocations.write().await;
        let now = Instant::now();
        
        allocations.retain(|_, allocation| {
            allocation.expires_at.map_or(true, |exp| exp > now)
        });
        
        Ok(())
    }

    async fn generate_optimization_recommendations(&self, usage: &ResourceUsage) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();
        
        // High utilization recommendation
        if usage.utilization_percentage > 85.0 {
            recommendations.push(OptimizationRecommendation {
                recommendation_type: RecommendationType::ScaleUp,
                description: "High resource utilization detected. Consider scaling up resources.".to_string(),
                impact: ImpactLevel::High,
                estimated_savings: ResourceSavings {
                    cpu_cores: 0,
                    memory_mb: 0,
                    disk_gb: 0,
                    network_bandwidth: 0,
                    cost_percentage: -20.0, // Negative indicates cost increase
                },
            });
        }
        
        // Low utilization recommendation
        if usage.utilization_percentage < 30.0 {
            recommendations.push(OptimizationRecommendation {
                recommendation_type: RecommendationType::ScaleDown,
                description: "Low resource utilization detected. Consider scaling down resources.".to_string(),
                impact: ImpactLevel::Medium,
                estimated_savings: ResourceSavings {
                    cpu_cores: usage.allocated_cpu_cores / 4,
                    memory_mb: usage.allocated_memory_mb / 4,
                    disk_gb: usage.allocated_disk_gb / 4,
                    network_bandwidth: usage.allocated_network_bandwidth / 4,
                    cost_percentage: 25.0,
                },
            });
        }
        
        recommendations
    }
}

impl ResourceManager for ProductionResourceManager {
    fn allocate_resources(&self, requirements: ResourceRequirements) -> impl std::future::Future<Output = Result<ResourceAllocation, AppError>> + Send {
        async move {
        // Cleanup expired allocations first
        self.cleanup_expired_allocations().await?;
        
        // Check if allocation is possible
        if !self.can_allocate(&requirements).await? {
            return Err(AppError::ResourceExhausted(
                "Insufficient resources available for allocation".to_string()
            ));
        }

        // Acquire semaphore permits
        let _cpu_permit = self.cpu_semaphore.acquire_many(requirements.cpu_cores).await
            .map_err(|e| AppError::ResourceAllocation(format!("Failed to acquire CPU resources: {}", e)))?;
        let _memory_permit = self.memory_semaphore.acquire_many((requirements.memory_mb / 100) as u32).await
            .map_err(|e| AppError::ResourceAllocation(format!("Failed to acquire memory resources: {}", e)))?;
        let _disk_permit = self.disk_semaphore.acquire_many(requirements.disk_gb as u32).await
            .map_err(|e| AppError::ResourceAllocation(format!("Failed to acquire disk resources: {}", e)))?;
        let _network_permit = self.network_semaphore.acquire_many((requirements.network_bandwidth / 1000) as u32).await
            .map_err(|e| AppError::ResourceAllocation(format!("Failed to acquire network resources: {}", e)))?;
        let _allocation_permit = self.allocation_semaphore.acquire().await
            .map_err(|e| AppError::ResourceAllocation(format!("Failed to acquire allocation slot: {}", e)))?;

        let allocation = ResourceAllocation {
            allocation_id: Uuid::new_v4(),
            cpu_cores: requirements.cpu_cores,
            memory_mb: requirements.memory_mb,
            disk_gb: requirements.disk_gb,
            network_bandwidth: requirements.network_bandwidth,
            allocated_at: Instant::now(),
            expires_at: requirements.timeout.map(|timeout| Instant::now() + timeout),
            priority: requirements.priority,
        };

        // Store allocation
        let mut allocations = self.allocations.write().await;
        allocations.insert(allocation.allocation_id, allocation.clone());

        // Forget permits to keep them acquired
        std::mem::forget(_cpu_permit);
        std::mem::forget(_memory_permit);
        std::mem::forget(_disk_permit);
        std::mem::forget(_network_permit);
        std::mem::forget(_allocation_permit);

        Ok(allocation)
        }
    }

    fn release_resources(&self, allocation: ResourceAllocation) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        async move {
        let mut allocations = self.allocations.write().await;
        
        if allocations.remove(&allocation.allocation_id).is_some() {
            // Release semaphore permits
            self.cpu_semaphore.add_permits(allocation.cpu_cores as usize);
            self.memory_semaphore.add_permits((allocation.memory_mb / 100) as usize);
            self.disk_semaphore.add_permits(allocation.disk_gb as usize);
            self.network_semaphore.add_permits((allocation.network_bandwidth / 1000) as usize);
            self.allocation_semaphore.add_permits(1);
            
            Ok(())
        } else {
            Err(AppError::ResourceNotFound(
                format!("Allocation {} not found", allocation.allocation_id)
            ))
        }
        }
    }

    fn optimize_resource_usage(&self) -> impl std::future::Future<Output = Result<OptimizationReport, AppError>> + Send {
        async move {
        self.cleanup_expired_allocations().await?;
        
        let usage = self.get_resource_usage().await?;
        let recommendations = self.generate_optimization_recommendations(&usage).await;
        
        let potential_savings = recommendations.iter()
            .fold(ResourceSavings {
                cpu_cores: 0,
                memory_mb: 0,
                disk_gb: 0,
                network_bandwidth: 0,
                cost_percentage: 0.0,
            }, |acc, rec| ResourceSavings {
                cpu_cores: acc.cpu_cores + rec.estimated_savings.cpu_cores,
                memory_mb: acc.memory_mb + rec.estimated_savings.memory_mb,
                disk_gb: acc.disk_gb + rec.estimated_savings.disk_gb,
                network_bandwidth: acc.network_bandwidth + rec.estimated_savings.network_bandwidth,
                cost_percentage: acc.cost_percentage + rec.estimated_savings.cost_percentage,
            });

        let efficiency_score = if usage.utilization_percentage > 70.0 && usage.utilization_percentage < 85.0 {
            95.0
        } else if usage.utilization_percentage > 50.0 {
            80.0
        } else {
            60.0
        };

        Ok(OptimizationReport {
            recommendations,
            potential_savings,
            efficiency_score,
            generated_at: Instant::now(),
        })
        }
    }

    fn get_resource_usage(&self) -> impl std::future::Future<Output = Result<ResourceUsage, AppError>> + Send {
        async move {
        let allocations = self.allocations.read().await;
        Ok(self.calculate_current_usage(&allocations).await)
        }
    }

    fn set_resource_quota(&self, quota: ResourceQuota) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        async move {
        let mut current_quota = self.quota.write().await;
        *current_quota = quota;
        Ok(())
        }
    }

    fn get_available_resources(&self) -> impl std::future::Future<Output = Result<ResourceRequirements, AppError>> + Send {
        async move {
        let quota = self.quota.read().await;
        let usage = self.get_resource_usage().await?;
        
        Ok(ResourceRequirements {
            cpu_cores: quota.max_cpu_cores.saturating_sub(usage.allocated_cpu_cores),
            memory_mb: quota.max_memory_mb.saturating_sub(usage.allocated_memory_mb),
            disk_gb: quota.max_disk_gb.saturating_sub(usage.allocated_disk_gb),
            network_bandwidth: quota.max_network_bandwidth.saturating_sub(usage.allocated_network_bandwidth),
            priority: ResourcePriority::Medium,
            timeout: None,
        })
        }
    }
}