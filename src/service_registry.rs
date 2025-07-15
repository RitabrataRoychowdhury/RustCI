use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use mongodb::{bson::doc, Collection, Database};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

/// Registered service in the service registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredService {
    pub id: Uuid,
    pub name: String,
    pub service_type: ServiceType,
    pub endpoint: String,
    pub deployment_id: Uuid,
    pub agent_capable: bool,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub status: ServiceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Type of service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    WebServer,
    ApiServer,
    Database,
    Cache,
    MessageQueue,
    Frontend,
    Backend,
    Custom,
}

/// Status of a registered service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
    Unknown,
}

/// Service registry for tracking deployed services
#[allow(dead_code)]
pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<Uuid, RegisteredService>>>,
    database: Arc<Database>,
    collection: Collection<RegisteredService>,
}

#[allow(dead_code)]
impl ServiceRegistry {
    /// Create a new service registry
    pub fn new(database: Arc<Database>) -> Self {
        let collection = database.collection::<RegisteredService>("services");
        
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            database,
            collection,
        }
    }

    /// Register a new service
    pub async fn register_service(&self, mut service: RegisteredService) -> Result<()> {
        info!("📝 Registering service: {} ({})", service.name, service.id);

        // Set timestamps
        let now = Utc::now();
        service.created_at = now;
        service.updated_at = now;
        service.last_heartbeat = Some(now);

        // Store in memory cache
        {
            let mut services = self.services.write().await;
            services.insert(service.id, service.clone());
        }

        // Store in database
        self.collection
            .insert_one(&service, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to register service: {}", e)))?;

        info!("✅ Service registered successfully: {}", service.name);
        Ok(())
    }

    /// Get a service by ID
    pub async fn get_service(&self, service_id: Uuid) -> Result<Option<RegisteredService>> {
        // Try memory cache first
        {
            let services = self.services.read().await;
            if let Some(service) = services.get(&service_id) {
                return Ok(Some(service.clone()));
            }
        }

        // Fallback to database
        let service = self.collection
            .find_one(doc! {"id": service_id.to_string()}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to get service: {}", e)))?;

        // Update cache if found
        if let Some(ref service) = service {
            let mut services = self.services.write().await;
            services.insert(service_id, service.clone());
        }

        Ok(service)
    }

    /// Get services by type
    pub async fn get_services_by_type(&self, service_type: ServiceType) -> Result<Vec<RegisteredService>> {
        let services = self.collection
            .find(doc! {"service_type": serde_json::to_string(&service_type).unwrap()}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to get services by type: {}", e)))?;

        let mut result = Vec::new();
        let mut cursor = services;
        
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(e.to_string()))? {
            let service = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize service: {}", e)))?;
            result.push(service);
        }

        Ok(result)
    }

    /// Get all services
    pub async fn get_all_services(&self) -> Result<Vec<RegisteredService>> {
        let services = self.collection
            .find(doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to get all services: {}", e)))?;

        let mut result = Vec::new();
        let mut cursor = services;
        
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(e.to_string()))? {
            let service = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize service: {}", e)))?;
            result.push(service);
        }

        Ok(result)
    }

    /// Update service heartbeat
    pub async fn update_heartbeat(&self, service_id: Uuid) -> Result<()> {
        let now = Utc::now();

        // Update memory cache
        {
            let mut services = self.services.write().await;
            if let Some(service) = services.get_mut(&service_id) {
                service.last_heartbeat = Some(now);
                service.updated_at = now;
                service.status = ServiceStatus::Running;
            }
        }

        // Update database
        self.collection
            .update_one(
                doc! {"id": service_id.to_string()},
                doc! {
                    "$set": {
                        "last_heartbeat": mongodb::bson::to_bson(&now).unwrap(),
                        "updated_at": mongodb::bson::to_bson(&now).unwrap(),
                        "status": "running"
                    }
                },
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update heartbeat: {}", e)))?;

        Ok(())
    }

    /// Update service status
    pub async fn update_service_status(&self, service_id: Uuid, status: ServiceStatus) -> Result<()> {
        let now = Utc::now();

        info!("🔄 Updating service status: {} -> {:?}", service_id, status);

        // Update memory cache
        {
            let mut services = self.services.write().await;
            if let Some(service) = services.get_mut(&service_id) {
                service.status = status.clone();
                service.updated_at = now;
            }
        }

        // Update database
        self.collection
            .update_one(
                doc! {"id": service_id.to_string()},
                doc! {
                    "$set": {
                        "status": serde_json::to_string(&status).unwrap(),
                        "updated_at": mongodb::bson::to_bson(&now).unwrap()
                    }
                },
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update service status: {}", e)))?;

        Ok(())
    }

    /// Remove a service from the registry
    pub async fn unregister_service(&self, service_id: Uuid) -> Result<()> {
        info!("🗑️ Unregistering service: {}", service_id);

        // Remove from memory cache
        {
            let mut services = self.services.write().await;
            services.remove(&service_id);
        }

        // Remove from database
        self.collection
            .delete_one(doc! {"id": service_id.to_string()}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to unregister service: {}", e)))?;

        info!("✅ Service unregistered successfully: {}", service_id);
        Ok(())
    }

    /// Get services that are agent-capable
    pub async fn get_agent_capable_services(&self) -> Result<Vec<RegisteredService>> {
        let services = self.collection
            .find(doc! {"agent_capable": true}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to get agent-capable services: {}", e)))?;

        let mut result = Vec::new();
        let mut cursor = services;
        
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(e.to_string()))? {
            let service = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize service: {}", e)))?;
            result.push(service);
        }

        Ok(result)
    }

    /// Check for stale services (no heartbeat for a while)
    pub async fn check_stale_services(&self, stale_threshold_minutes: i64) -> Result<Vec<RegisteredService>> {
        let threshold = Utc::now() - chrono::Duration::minutes(stale_threshold_minutes);
        let mut stale_services = Vec::new();

        let services = self.services.read().await;
        for service in services.values() {
            if let Some(last_heartbeat) = service.last_heartbeat {
                if last_heartbeat < threshold {
                    warn!("⚠️ Stale service detected: {} (last heartbeat: {})", service.name, last_heartbeat);
                    stale_services.push(service.clone());
                }
            }
        }

        Ok(stale_services)
    }

    /// Update service metadata
    pub async fn update_service_metadata(&self, service_id: Uuid, metadata: HashMap<String, String>) -> Result<()> {
        let now = Utc::now();

        // Update memory cache
        {
            let mut services = self.services.write().await;
            if let Some(service) = services.get_mut(&service_id) {
                service.metadata = metadata.clone();
                service.updated_at = now;
            }
        }

        // Update database
        self.collection
            .update_one(
                doc! {"id": service_id.to_string()},
                doc! {
                    "$set": {
                        "metadata": mongodb::bson::to_bson(&metadata).unwrap(),
                        "updated_at": mongodb::bson::to_bson(&now).unwrap()
                    }
                },
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update service metadata: {}", e)))?;

        Ok(())
    }

    /// Get service statistics
    pub async fn get_service_stats(&self) -> Result<ServiceStats> {
        let all_services = self.get_all_services().await?;
        
        let mut stats = ServiceStats {
            total_services: all_services.len(),
            running_services: 0,
            failed_services: 0,
            agent_capable_services: 0,
            services_by_type: HashMap::new(),
        };

        for service in &all_services {
            match service.status {
                ServiceStatus::Running => stats.running_services += 1,
                ServiceStatus::Failed => stats.failed_services += 1,
                _ => {}
            }

            if service.agent_capable {
                stats.agent_capable_services += 1;
            }

            let type_key = format!("{:?}", service.service_type);
            *stats.services_by_type.entry(type_key).or_insert(0) += 1;
        }

        Ok(stats)
    }
}

/// Service registry statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStats {
    pub total_services: usize,
    pub running_services: usize,
    pub failed_services: usize,
    pub agent_capable_services: usize,
    pub services_by_type: HashMap<String, usize>,
}

/// Helper function to create a service registry instance
#[allow(dead_code)]
pub fn create_service_registry(database: Arc<Database>) -> ServiceRegistry {
    ServiceRegistry::new(database)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> RegisteredService {
        RegisteredService {
            id: Uuid::new_v4(),
            name: "test-service".to_string(),
            service_type: ServiceType::WebServer,
            endpoint: "http://localhost:3000".to_string(),
            deployment_id: Uuid::new_v4(),
            agent_capable: true,
            last_heartbeat: Some(Utc::now()),
            metadata: HashMap::new(),
            status: ServiceStatus::Running,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_service_creation() {
        let service = create_test_service();
        assert_eq!(service.name, "test-service");
        assert!(matches!(service.service_type, ServiceType::WebServer));
        assert!(service.agent_capable);
    }

    #[test]
    fn test_service_serialization() {
        let service = create_test_service();
        let json = serde_json::to_string(&service).unwrap();
        let deserialized: RegisteredService = serde_json::from_str(&json).unwrap();
        
        assert_eq!(service.id, deserialized.id);
        assert_eq!(service.name, deserialized.name);
    }
}