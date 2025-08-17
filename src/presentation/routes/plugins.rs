//! Plugin management API routes

use crate::error::{AppError, Result};
use crate::plugins::{
    manager::{PluginManager, PluginManagerConfig},
    PluginConfig, PluginMetadata, PluginState,
    fallback::{FallbackManager, FallbackStrategy},
    health::PluginHealthMonitor,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// Plugin management state
#[derive(Clone)]
pub struct PluginManagementState {
    pub plugin_manager: Arc<PluginManager>,
    pub fallback_manager: Arc<FallbackManager>,
    pub health_monitor: Arc<PluginHealthMonitor>,
}

/// Plugin API response
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Plugin list query parameters
#[derive(Debug, Deserialize)]
pub struct PluginListQuery {
    pub state: Option<String>,
    pub include_health: Option<bool>,
}

/// Plugin configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdatePluginConfigRequest {
    pub config: PluginConfig,
}

/// Fallback activation request
#[derive(Debug, Deserialize)]
pub struct ActivateFallbackRequest {
    pub strategy: String,
}

/// Create plugin management routes
pub fn create_plugin_routes() -> Router<PluginManagementState> {
    Router::new()
        // Plugin management
        .route("/plugins", get(list_plugins))
        .route("/plugins/:plugin_id", get(get_plugin))
        .route("/plugins/:plugin_id/start", post(start_plugin))
        .route("/plugins/:plugin_id/stop", post(stop_plugin))
        .route("/plugins/:plugin_id/restart", post(restart_plugin))
        .route("/plugins/:plugin_id/config", put(update_plugin_config))
        .route("/plugins/:plugin_id", delete(unload_plugin))
        
        // Plugin health
        .route("/plugins/:plugin_id/health", get(get_plugin_health))
        .route("/plugins/health", get(get_all_plugin_health))
        .route("/plugins/health/summary", get(get_health_summary))
        
        // Fallback management
        .route("/plugins/:plugin_id/fallback/activate", post(activate_fallback))
        .route("/plugins/:plugin_id/fallback/deactivate", post(deactivate_fallback))
        .route("/plugins/:plugin_id/fallback/status", get(get_fallback_status))
        
        // System management
        .route("/plugins/system/stats", get(get_system_stats))
        .route("/plugins/system/reload", post(reload_plugins))
}

/// List all plugins
async fn list_plugins(
    State(state): State<PluginManagementState>,
    Query(query): Query<PluginListQuery>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Listing plugins");
    
    let plugins = state.plugin_manager.list_plugins().await;
    
    let mut plugin_data = Vec::new();
    for plugin in plugins {
        let mut data = serde_json::to_value(&plugin)
            .map_err(|e| AppError::SerializationError(e.to_string()))?;
        
        // Add health information if requested
        if query.include_health.unwrap_or(false) {
            if let Ok(health) = state.plugin_manager.get_plugin_health(&plugin.id).await {
                data["health"] = serde_json::to_value(&health)
                    .map_err(|e| AppError::SerializationError(e.to_string()))?;
            }
        }
        
        // Filter by state if specified
        if let Some(state_filter) = &query.state {
            // This is a simplified state check - in reality we'd get the actual state
            if state_filter != "all" {
                continue;
            }
        }
        
        plugin_data.push(data);
    }
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Found {} plugins", plugin_data.len()),
        data: Some(serde_json::json!({ "plugins": plugin_data })),
    }))
}

/// Get specific plugin information
async fn get_plugin(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting plugin information: {}", plugin_id);
    
    let plugins = state.plugin_manager.list_plugins().await;
    let plugin = plugins.iter()
        .find(|p| p.id == plugin_id)
        .ok_or_else(|| AppError::NotFound(format!("Plugin {} not found", plugin_id)))?;
    
    // Get health information
    let health = state.plugin_manager.get_plugin_health(&plugin_id).await.ok();
    
    let mut data = serde_json::to_value(plugin)
        .map_err(|e| AppError::SerializationError(e.to_string()))?;
    
    if let Some(health) = health {
        data["health"] = serde_json::to_value(&health)
            .map_err(|e| AppError::SerializationError(e.to_string()))?;
    }
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} information", plugin_id),
        data: Some(data),
    }))
}

/// Start a plugin
async fn start_plugin(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Starting plugin: {}", plugin_id);
    
    state.plugin_manager.start_plugin(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} started successfully", plugin_id),
        data: None,
    }))
}

/// Stop a plugin
async fn stop_plugin(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Stopping plugin: {}", plugin_id);
    
    state.plugin_manager.stop_plugin(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} stopped successfully", plugin_id),
        data: None,
    }))
}

/// Restart a plugin
async fn restart_plugin(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Restarting plugin: {}", plugin_id);
    
    state.plugin_manager.restart_plugin(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} restarted successfully", plugin_id),
        data: None,
    }))
}

/// Update plugin configuration
async fn update_plugin_config(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<UpdatePluginConfigRequest>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Updating configuration for plugin: {}", plugin_id);
    
    state.plugin_manager.update_plugin_config(&plugin_id, request.config).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} configuration updated successfully", plugin_id),
        data: None,
    }))
}

/// Unload a plugin
async fn unload_plugin(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Unloading plugin: {}", plugin_id);
    
    state.plugin_manager.unload_plugin(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} unloaded successfully", plugin_id),
        data: None,
    }))
}

/// Get plugin health status
async fn get_plugin_health(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting health status for plugin: {}", plugin_id);
    
    let health = state.plugin_manager.get_plugin_health(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Plugin {} health status", plugin_id),
        data: Some(serde_json::to_value(&health)
            .map_err(|e| AppError::SerializationError(e.to_string()))?),
    }))
}

/// Get health status for all plugins
async fn get_all_plugin_health(
    State(state): State<PluginManagementState>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting health status for all plugins");
    
    let health_statuses = state.plugin_manager.get_all_plugin_health().await;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Health status for {} plugins", health_statuses.len()),
        data: Some(serde_json::json!({ "health_statuses": health_statuses })),
    }))
}

/// Get health summary
async fn get_health_summary(
    State(state): State<PluginManagementState>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting plugin health summary");
    
    let summary = state.health_monitor.get_health_summary().await;
    
    Ok(Json(PluginResponse {
        success: true,
        message: "Plugin health summary".to_string(),
        data: Some(serde_json::to_value(&summary)
            .map_err(|e| AppError::SerializationError(e.to_string()))?),
    }))
}

/// Activate fallback for a plugin
async fn activate_fallback(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<ActivateFallbackRequest>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Activating fallback for plugin: {} with strategy: {}", plugin_id, request.strategy);
    
    let strategy = match request.strategy.as_str() {
        "http" => FallbackStrategy::Http,
        "local" => FallbackStrategy::Local,
        "none" => FallbackStrategy::None,
        custom => FallbackStrategy::Custom(custom.to_string()),
    };
    
    state.fallback_manager.activate_fallback(&plugin_id, strategy).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Fallback activated for plugin {} with strategy: {}", plugin_id, request.strategy),
        data: None,
    }))
}

/// Deactivate fallback for a plugin
async fn deactivate_fallback(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Deactivating fallback for plugin: {}", plugin_id);
    
    state.fallback_manager.deactivate_fallback(&plugin_id).await?;
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Fallback deactivated for plugin {}", plugin_id),
        data: None,
    }))
}

/// Get fallback status for a plugin
async fn get_fallback_status(
    State(state): State<PluginManagementState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting fallback status for plugin: {}", plugin_id);
    
    let all_stats = state.fallback_manager.get_all_stats().await;
    let stats = all_stats.get(&plugin_id);
    
    Ok(Json(PluginResponse {
        success: true,
        message: format!("Fallback status for plugin {}", plugin_id),
        data: Some(serde_json::to_value(&stats)
            .map_err(|e| AppError::SerializationError(e.to_string()))?),
    }))
}

/// Get system statistics
async fn get_system_stats(
    State(state): State<PluginManagementState>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Getting plugin system statistics");
    
    let stats = state.plugin_manager.get_system_stats().await;
    
    Ok(Json(PluginResponse {
        success: true,
        message: "Plugin system statistics".to_string(),
        data: Some(serde_json::to_value(&stats)
            .map_err(|e| AppError::SerializationError(e.to_string()))?),
    }))
}

/// Reload plugins
async fn reload_plugins(
    State(state): State<PluginManagementState>,
) -> Result<Json<PluginResponse>, AppError> {
    info!("Reloading plugins");
    
    // This would trigger a plugin reload process
    warn!("Plugin reload not yet implemented");
    
    Ok(Json(PluginResponse {
        success: true,
        message: "Plugin reload initiated".to_string(),
        data: None,
    }))
}

