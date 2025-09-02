//! Component Registry
//!
//! Manages registration and discovery of all system components with their
//! metadata, dependencies, and lifecycle information.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Component registry for managing all system components
pub struct ComponentRegistry {
    components: Arc<RwLock<HashMap<String, ComponentInfo>>>,
    dependencies: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

/// Information about a registered component
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub id: Uuid,
    pub name: String,
    pub component_type: ComponentType,
    pub version: String,
    pub status: ComponentStatus,
    pub health_score: f64,
    pub registered_at: DateTime<Utc>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Type of component
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentType {
    Infrastructure,
    Security,
    Performance,
    Storage,
    Testing,
    Deployment,
    Observability,
    Api,
    Legacy,
}

/// Status of a component
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentStatus {
    Initializing,
    Running,
    Degraded,
    Failed,
    ShuttingDown,
    Stopped,
}

impl ComponentRegistry {
    /// Create a new component registry
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            dependencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a new component
    pub async fn register_component(
        &self,
        name: String,
        component_type: ComponentType,
        version: String,
        dependencies: Vec<String>,
        metadata: HashMap<String, String>,
    ) -> Uuid {
        let component_id = Uuid::new_v4();
        
        let component_info = ComponentInfo {
            id: component_id,
            name: name.clone(),
            component_type,
            version,
            status: ComponentStatus::Initializing,
            health_score: 1.0,
            registered_at: Utc::now(),
            last_health_check: None,
            metadata,
        };
        
        {
            let mut components = self.components.write().await;
            components.insert(name.clone(), component_info);
        }
        
        {
            let mut deps = self.dependencies.write().await;
            deps.insert(name, dependencies);
        }
        
        component_id
    }
    
    /// Update component status
    pub async fn update_component_status(&self, name: &str, status: ComponentStatus) {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.status = status;
        }
    }
    
    /// Update component health score
    pub async fn update_health_score(&self, name: &str, health_score: f64) {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.health_score = health_score;
            component.last_health_check = Some(Utc::now());
        }
    }
    
    /// Get component information
    pub async fn get_component(&self, name: &str) -> Option<ComponentInfo> {
        let components = self.components.read().await;
        components.get(name).cloned()
    }
    
    /// Get all components of a specific type
    pub async fn get_components_by_type(&self, component_type: ComponentType) -> Vec<ComponentInfo> {
        let components = self.components.read().await;
        components
            .values()
            .filter(|c| c.component_type == component_type)
            .cloned()
            .collect()
    }
    
    /// Get component dependencies
    pub async fn get_dependencies(&self, name: &str) -> Vec<String> {
        let dependencies = self.dependencies.read().await;
        dependencies.get(name).cloned().unwrap_or_default()
    }
    
    /// Get components that depend on the given component
    pub async fn get_dependents(&self, name: &str) -> Vec<String> {
        let dependencies = self.dependencies.read().await;
        dependencies
            .iter()
            .filter(|(_, deps)| deps.contains(&name.to_string()))
            .map(|(component_name, _)| component_name.clone())
            .collect()
    }
    
    /// Get all components
    pub async fn get_all_components(&self) -> Vec<ComponentInfo> {
        let components = self.components.read().await;
        components.values().cloned().collect()
    }
    
    /// Get components in dependency order (topological sort)
    pub async fn get_initialization_order(&self) -> Vec<String> {
        let dependencies = self.dependencies.read().await;
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        
        for component in dependencies.keys() {
            if !visited.contains(component) {
                self.topological_sort_visit(
                    component,
                    &dependencies,
                    &mut visited,
                    &mut temp_visited,
                    &mut result,
                );
            }
        }
        
        result
    }
    
    /// Get components in shutdown order (reverse dependency order)
    pub async fn get_shutdown_order(&self) -> Vec<String> {
        let mut init_order = self.get_initialization_order().await;
        init_order.reverse();
        init_order
    }
    
    /// Perform topological sort for dependency resolution
    fn topological_sort_visit(
        &self,
        component: &str,
        dependencies: &HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
        temp_visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<String>,
    ) {
        if temp_visited.contains(component) {
            // Circular dependency detected - log warning but continue
            tracing::warn!("Circular dependency detected involving component: {}", component);
            return;
        }
        
        if visited.contains(component) {
            return;
        }
        
        temp_visited.insert(component.to_string());
        
        if let Some(deps) = dependencies.get(component) {
            for dep in deps {
                self.topological_sort_visit(dep, dependencies, visited, temp_visited, result);
            }
        }
        
        temp_visited.remove(component);
        visited.insert(component.to_string());
        result.push(component.to_string());
    }
    
    /// Get system health summary
    pub async fn get_system_health_summary(&self) -> SystemHealthSummary {
        let components = self.components.read().await;
        
        let total_components = components.len();
        let healthy_components = components
            .values()
            .filter(|c| c.status == ComponentStatus::Running && c.health_score > 0.8)
            .count();
        let degraded_components = components
            .values()
            .filter(|c| c.status == ComponentStatus::Degraded || (c.status == ComponentStatus::Running && c.health_score <= 0.8))
            .count();
        let failed_components = components
            .values()
            .filter(|c| c.status == ComponentStatus::Failed)
            .count();
        
        let average_health_score = if total_components > 0 {
            components.values().map(|c| c.health_score).sum::<f64>() / total_components as f64
        } else {
            0.0
        };
        
        SystemHealthSummary {
            total_components,
            healthy_components,
            degraded_components,
            failed_components,
            average_health_score,
            last_updated: Utc::now(),
        }
    }
    
    /// Remove a component from the registry
    pub async fn unregister_component(&self, name: &str) {
        {
            let mut components = self.components.write().await;
            components.remove(name);
        }
        
        {
            let mut dependencies = self.dependencies.write().await;
            dependencies.remove(name);
            
            // Remove this component from other components' dependencies
            for deps in dependencies.values_mut() {
                deps.retain(|dep| dep != name);
            }
        }
    }
}

/// System health summary
#[derive(Debug, Clone)]
pub struct SystemHealthSummary {
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub failed_components: usize,
    pub average_health_score: f64,
    pub last_updated: DateTime<Utc>,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}