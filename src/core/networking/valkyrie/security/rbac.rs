/// Role-Based Access Control (RBAC) system for the Valkyrie Protocol
/// 
/// This module provides:
/// - Hierarchical role management
/// - Fine-grained permission system
/// - Policy-based access control
/// - Dynamic role assignment
/// - Attribute-based access control (ABAC) extensions

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::core::networking::node_communication::NodeId;
use crate::error::Result;
use super::AuthSubject;

/// RBAC manager for handling roles, permissions, and access control
pub struct RbacManager {
    config: RbacConfig,
    roles: Arc<RwLock<HashMap<String, Role>>>,
    permissions: Arc<RwLock<HashMap<String, Permission>>>,
    policies: Arc<RwLock<HashMap<String, AccessPolicy>>>,
    role_assignments: Arc<RwLock<HashMap<NodeId, Vec<String>>>>,
    metrics: Arc<RwLock<RbacMetrics>>,
}

/// RBAC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    pub enable_hierarchical_roles: bool,
    pub enable_dynamic_permissions: bool,
    pub enable_attribute_based_access: bool,
    pub default_roles: Vec<String>,
    pub admin_roles: Vec<String>,
    pub cache_ttl_seconds: u64,
    pub max_role_depth: u32,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            enable_hierarchical_roles: true,
            enable_dynamic_permissions: true,
            enable_attribute_based_access: true,
            default_roles: vec!["node".to_string()],
            admin_roles: vec!["admin".to_string(), "super_admin".to_string()],
            cache_ttl_seconds: 300, // 5 minutes
            max_role_depth: 10,
        }
    }
}

/// Role definition with hierarchical support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub permissions: HashSet<String>,
    pub parent_roles: Vec<String>,
    pub child_roles: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

impl Role {
    pub fn new(name: String, description: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            name,
            description,
            permissions: HashSet::new(),
            parent_roles: Vec::new(),
            child_roles: Vec::new(),
            attributes: HashMap::new(),
            created_at: now,
            updated_at: now,
            active: true,
        }
    }

    pub fn add_permission(&mut self, permission: String) {
        self.permissions.insert(permission);
        self.updated_at = chrono::Utc::now();
    }

    pub fn remove_permission(&mut self, permission: &str) {
        self.permissions.remove(permission);
        self.updated_at = chrono::Utc::now();
    }

    pub fn add_parent_role(&mut self, parent: String) {
        if !self.parent_roles.contains(&parent) {
            self.parent_roles.push(parent);
            self.updated_at = chrono::Utc::now();
        }
    }

    pub fn add_child_role(&mut self, child: String) {
        if !self.child_roles.contains(&child) {
            self.child_roles.push(child);
            self.updated_at = chrono::Utc::now();
        }
    }
}

/// Permission definition with resource and action scoping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub name: String,
    pub description: String,
    pub resource_type: String,
    pub resource_pattern: String,
    pub actions: HashSet<String>,
    pub conditions: Vec<PermissionCondition>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

impl Permission {
    pub fn new(name: String, description: String, resource_type: String) -> Self {
        Self {
            name,
            description,
            resource_type,
            resource_pattern: "*".to_string(),
            actions: HashSet::new(),
            conditions: Vec::new(),
            created_at: chrono::Utc::now(),
            active: true,
        }
    }

    pub fn with_actions(mut self, actions: Vec<String>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }

    pub fn with_resource_pattern(mut self, pattern: String) -> Self {
        self.resource_pattern = pattern;
        self
    }

    pub fn with_conditions(mut self, conditions: Vec<PermissionCondition>) -> Self {
        self.conditions = conditions;
        self
    }
}

/// Permission condition for attribute-based access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCondition {
    pub attribute: String,
    pub operator: ConditionOperator,
    pub value: String,
    pub required: bool,
}

/// Condition operators for permission evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Regex,
    In,
    NotIn,
}

/// Access policy for complex authorization rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub name: String,
    pub description: String,
    pub rules: Vec<PolicyRule>,
    pub effect: PolicyEffect,
    pub priority: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

/// Policy rule for fine-grained access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub resource_pattern: String,
    pub actions: Vec<String>,
    pub conditions: Vec<PermissionCondition>,
    pub effect: PolicyEffect,
}

/// Policy effect (allow or deny)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyEffect {
    Allow,
    Deny,
}

/// RBAC metrics for monitoring and auditing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacMetrics {
    pub total_authorization_checks: u64,
    pub successful_authorizations: u64,
    pub failed_authorizations: u64,
    pub role_assignments: u64,
    pub permission_grants: u64,
    pub policy_evaluations: u64,
    pub average_check_time_ms: f64,
}

impl RbacManager {
    pub fn new(config: RbacConfig) -> Result<Self> {
        let manager = Self {
            config: config.clone(),
            roles: Arc::new(RwLock::new(HashMap::new())),
            permissions: Arc::new(RwLock::new(HashMap::new())),
            policies: Arc::new(RwLock::new(HashMap::new())),
            role_assignments: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(RbacMetrics {
                total_authorization_checks: 0,
                successful_authorizations: 0,
                failed_authorizations: 0,
                role_assignments: 0,
                permission_grants: 0,
                policy_evaluations: 0,
                average_check_time_ms: 0.0,
            })),
        };

        let manager_clone = Self {
            config: config.clone(),
            roles: manager.roles.clone(),
            permissions: manager.permissions.clone(),
            policies: manager.policies.clone(),
            role_assignments: manager.role_assignments.clone(),
            metrics: manager.metrics.clone(),
        };

        // Initialize default roles and permissions
        tokio::spawn(async move {
            if let Err(e) = manager_clone.initialize_defaults().await {
                eprintln!("Failed to initialize RBAC defaults: {}", e);
            }
        });

        Ok(manager)
    }

    /// Initialize default roles and permissions
    async fn initialize_defaults(&self) -> Result<()> {
        // Create default roles
        self.create_role("node".to_string(), "Basic node role".to_string()).await?;
        self.create_role("admin".to_string(), "Administrator role".to_string()).await?;
        self.create_role("super_admin".to_string(), "Super administrator role".to_string()).await?;

        // Create default permissions
        let permissions = vec![
            Permission::new("node.read".to_string(), "Read node information".to_string(), "node".to_string())
                .with_actions(vec!["read".to_string()]),
            Permission::new("node.write".to_string(), "Write node information".to_string(), "node".to_string())
                .with_actions(vec!["write".to_string(), "update".to_string()]),
            Permission::new("job.execute".to_string(), "Execute jobs".to_string(), "job".to_string())
                .with_actions(vec!["execute".to_string(), "cancel".to_string()]),
            Permission::new("cluster.manage".to_string(), "Manage cluster".to_string(), "cluster".to_string())
                .with_actions(vec!["read".to_string(), "write".to_string(), "delete".to_string()]),
            Permission::new("admin.all".to_string(), "All administrative permissions".to_string(), "*".to_string())
                .with_actions(vec!["*".to_string()]),
        ];

        for permission in permissions {
            self.create_permission(permission).await?;
        }

        // Assign permissions to roles
        self.assign_permission_to_role("node".to_string(), "node.read".to_string()).await?;
        self.assign_permission_to_role("node".to_string(), "job.execute".to_string()).await?;
        
        self.assign_permission_to_role("admin".to_string(), "node.read".to_string()).await?;
        self.assign_permission_to_role("admin".to_string(), "node.write".to_string()).await?;
        self.assign_permission_to_role("admin".to_string(), "job.execute".to_string()).await?;
        self.assign_permission_to_role("admin".to_string(), "cluster.manage".to_string()).await?;
        
        self.assign_permission_to_role("super_admin".to_string(), "admin.all".to_string()).await?;

        // Set up role hierarchy
        self.add_role_parent("admin".to_string(), "node".to_string()).await?;
        self.add_role_parent("super_admin".to_string(), "admin".to_string()).await?;

        Ok(())
    }

    /// Create a new role
    pub async fn create_role(&self, name: String, description: String) -> Result<()> {
        let role = Role::new(name.clone(), description);
        self.roles.write().await.insert(name, role);
        Ok(())
    }

    /// Create a new permission
    pub async fn create_permission(&self, permission: Permission) -> Result<()> {
        let name = permission.name.clone();
        self.permissions.write().await.insert(name, permission);
        Ok(())
    }

    /// Assign a permission to a role
    pub async fn assign_permission_to_role(&self, role_name: String, permission_name: String) -> Result<()> {
        let mut roles = self.roles.write().await;
        if let Some(role) = roles.get_mut(&role_name) {
            role.add_permission(permission_name);
            Ok(())
        } else {
            Err(crate::error::AppError::SecurityError(format!("Role '{}' not found", role_name)).into())
        }
    }

    /// Add a parent role to create hierarchy
    pub async fn add_role_parent(&self, child_role: String, parent_role: String) -> Result<()> {
        let mut roles = self.roles.write().await;
        
        // Add parent to child
        if let Some(child) = roles.get_mut(&child_role) {
            child.add_parent_role(parent_role.clone());
        } else {
            return Err(crate::error::AppError::SecurityError(format!("Child role '{}' not found", child_role)).into());
        }

        // Add child to parent
        if let Some(parent) = roles.get_mut(&parent_role) {
            parent.add_child_role(child_role);
        } else {
            return Err(crate::error::AppError::SecurityError(format!("Parent role '{}' not found", parent_role)).into());
        }

        Ok(())
    }

    /// Assign roles to a node
    pub async fn assign_roles_to_node(&self, node_id: NodeId, roles: Vec<String>) -> Result<()> {
        // Validate that all roles exist
        let role_map = self.roles.read().await;
        for role_name in &roles {
            if !role_map.contains_key(role_name) {
                return Err(crate::error::AppError::SecurityError(format!("Role '{}' not found", role_name)).into());
            }
        }
        drop(role_map);

        self.role_assignments.write().await.insert(node_id, roles);
        
        let mut metrics = self.metrics.write().await;
        metrics.role_assignments += 1;
        
        Ok(())
    }

    /// Get all permissions for a node (including inherited permissions)
    pub async fn get_node_permissions(&self, node_id: &NodeId) -> Result<HashSet<String>> {
        let role_assignments = self.role_assignments.read().await;
        let node_roles = role_assignments.get(node_id).cloned().unwrap_or_default();
        drop(role_assignments);

        let mut all_permissions = HashSet::new();
        let roles = self.roles.read().await;

        for role_name in node_roles {
            self.collect_role_permissions(&role_name, &roles, &mut all_permissions, &mut HashSet::new(), 0).await?;
        }

        Ok(all_permissions)
    }

    /// Recursively collect permissions from role hierarchy
    fn collect_role_permissions<'a>(
        &'a self,
        role_name: &'a str,
        roles: &'a HashMap<String, Role>,
        permissions: &'a mut HashSet<String>,
        visited: &'a mut HashSet<String>,
        depth: u32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if depth > self.config.max_role_depth {
                return Err(crate::error::AppError::SecurityError("Role hierarchy too deep".to_string()).into());
            }

            if visited.contains(role_name) {
                return Ok(()); // Avoid cycles
            }
            visited.insert(role_name.to_string());

            if let Some(role) = roles.get(role_name) {
                if !role.active {
                    return Ok(());
                }

                // Add direct permissions
                permissions.extend(role.permissions.iter().cloned());

                // Add permissions from parent roles
                for parent_role in &role.parent_roles {
                    self.collect_role_permissions(parent_role, roles, permissions, visited, depth + 1).await?;
                }
            }

            Ok(())
        })
    }

    /// Check if a subject has permission to perform an action on a resource
    pub async fn check_permission(&self, subject: &AuthSubject, resource: &str, action: &str) -> Result<bool> {
        let start_time = std::time::Instant::now();
        
        // Get node permissions
        let node_permissions = self.get_node_permissions(&subject.node_id).await?;
        
        // Check direct permissions
        let permission_granted = self.evaluate_permissions(&node_permissions, resource, action).await?;
        
        // If not granted by permissions, check policies
        let policy_result = if !permission_granted {
            self.evaluate_policies(subject, resource, action).await?
        } else {
            true
        };

        let final_result = permission_granted || policy_result;

        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_authorization_checks += 1;
        if final_result {
            metrics.successful_authorizations += 1;
        } else {
            metrics.failed_authorizations += 1;
        }
        metrics.average_check_time_ms = (metrics.average_check_time_ms * (metrics.total_authorization_checks - 1) as f64 + duration.as_millis() as f64) / metrics.total_authorization_checks as f64;

        Ok(final_result)
    }

    /// Evaluate permissions against resource and action
    async fn evaluate_permissions(&self, permissions: &HashSet<String>, resource: &str, action: &str) -> Result<bool> {
        let permission_map = self.permissions.read().await;
        
        for permission_name in permissions {
            if let Some(permission) = permission_map.get(permission_name) {
                if !permission.active {
                    continue;
                }

                // Check if permission applies to this resource type
                if permission.resource_type != "*" && !resource.starts_with(&permission.resource_type) {
                    continue;
                }

                // Check resource pattern
                if !self.matches_pattern(&permission.resource_pattern, resource) {
                    continue;
                }

                // Check action
                if permission.actions.contains("*") || permission.actions.contains(action) {
                    // Evaluate conditions
                    if self.evaluate_conditions(&permission.conditions, &HashMap::new()).await? {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Evaluate access policies
    async fn evaluate_policies(&self, subject: &AuthSubject, resource: &str, action: &str) -> Result<bool> {
        let policies = self.policies.read().await;
        let mut allow = false;
        let mut deny = false;

        // Sort policies by priority (higher priority first)
        let mut sorted_policies: Vec<_> = policies.values().collect();
        sorted_policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        for policy in sorted_policies {
            if !policy.active {
                continue;
            }

            for rule in &policy.rules {
                if !self.matches_pattern(&rule.resource_pattern, resource) {
                    continue;
                }

                if !rule.actions.contains(&"*".to_string()) && !rule.actions.contains(&action.to_string()) {
                    continue;
                }

                if self.evaluate_conditions(&rule.conditions, &subject.attributes).await? {
                    match rule.effect {
                        PolicyEffect::Allow => allow = true,
                        PolicyEffect::Deny => deny = true,
                    }
                }
            }
        }

        let mut metrics = self.metrics.write().await;
        metrics.policy_evaluations += 1;

        // Deny takes precedence over allow
        Ok(allow && !deny)
    }

    /// Evaluate permission conditions
    async fn evaluate_conditions(&self, conditions: &[PermissionCondition], attributes: &HashMap<String, String>) -> Result<bool> {
        for condition in conditions {
            let attribute_value = attributes.get(&condition.attribute);
            
            if condition.required && attribute_value.is_none() {
                return Ok(false);
            }

            if let Some(value) = attribute_value {
                let matches = match condition.operator {
                    ConditionOperator::Equals => value == &condition.value,
                    ConditionOperator::NotEquals => value != &condition.value,
                    ConditionOperator::Contains => value.contains(&condition.value),
                    ConditionOperator::NotContains => !value.contains(&condition.value),
                    ConditionOperator::GreaterThan => {
                        value.parse::<f64>().unwrap_or(0.0) > condition.value.parse::<f64>().unwrap_or(0.0)
                    }
                    ConditionOperator::LessThan => {
                        value.parse::<f64>().unwrap_or(0.0) < condition.value.parse::<f64>().unwrap_or(0.0)
                    }
                    ConditionOperator::GreaterThanOrEqual => {
                        value.parse::<f64>().unwrap_or(0.0) >= condition.value.parse::<f64>().unwrap_or(0.0)
                    }
                    ConditionOperator::LessThanOrEqual => {
                        value.parse::<f64>().unwrap_or(0.0) <= condition.value.parse::<f64>().unwrap_or(0.0)
                    }
                    ConditionOperator::Regex => {
                        // In a real implementation, we would use a regex crate
                        value.contains(&condition.value) // Simplified for now
                    }
                    ConditionOperator::In => {
                        condition.value.split(',').any(|v| v.trim() == value)
                    }
                    ConditionOperator::NotIn => {
                        !condition.value.split(',').any(|v| v.trim() == value)
                    }
                };

                if !matches {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Check if a resource matches a pattern (simplified glob matching)
    fn matches_pattern(&self, pattern: &str, resource: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return resource.starts_with(prefix);
        }

        if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            return resource.ends_with(suffix);
        }

        pattern == resource
    }

    /// Get RBAC metrics
    pub async fn get_metrics(&self) -> RbacMetrics {
        self.metrics.read().await.clone()
    }

    /// Get all roles
    pub async fn get_roles(&self) -> HashMap<String, Role> {
        self.roles.read().await.clone()
    }

    /// Get all permissions
    pub async fn get_permissions(&self) -> HashMap<String, Permission> {
        self.permissions.read().await.clone()
    }

    /// Get role assignments for a node
    pub async fn get_node_roles(&self, node_id: &NodeId) -> Vec<String> {
        self.role_assignments.read().await.get(node_id).cloned().unwrap_or_default()
    }
}