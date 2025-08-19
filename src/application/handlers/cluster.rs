//! Cluster management HTTP handlers
//!
//! This module provides HTTP API endpoints for cluster management,
//! including node registration, cluster status, and metrics.

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;
use utoipa::ToSchema;

use crate::core::cluster::cluster_coordinator::ClusterCoordinator;
use crate::domain::entities::{
    ClusterMetrics, ClusterNode, ClusterStatus, NodeId, NodeRole, NodeStatus,
};
use crate::error::{utils::ErrorUtils, AppError, Result};

/// Request to join a node to the cluster
#[derive(Debug, Deserialize, ToSchema)]
pub struct JoinNodeRequest {
    /// Node name
    pub name: String,
    /// Node address
    pub address: String,
    /// Node role
    pub role: NodeRole,
    /// Node metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Response for node join operation
#[derive(Debug, Serialize, ToSchema)]
pub struct JoinNodeResponse {
    /// Joined node information
    pub node: ClusterNode,
    /// Success message
    pub message: String,
}

/// Request to leave a node from the cluster
#[derive(Debug, Deserialize, ToSchema)]
pub struct LeaveNodeRequest {
    /// Reason for leaving
    pub reason: Option<String>,
}

// Additional DTOs for OpenAPI documentation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateRunnerRequest {
    pub name: String,
    pub runner_type: String,
    pub capacity: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RunnerMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub active_jobs: u32,
    pub completed_jobs: u32,
    pub failed_jobs: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeHealth {
    pub status: String,
    pub last_heartbeat: String,
    pub uptime_seconds: u64,
    pub load_average: f64,
}

/// Response for node leave operation
#[derive(Debug, Serialize)]
pub struct LeaveNodeResponse {
    /// Success message
    pub message: String,
}

/// Query parameters for listing nodes
#[derive(Debug, Deserialize)]
pub struct ListNodesQuery {
    /// Filter by status
    pub status: Option<NodeStatus>,
    /// Filter by role
    pub role: Option<NodeRole>,
    /// Include only healthy nodes
    pub healthy_only: Option<bool>,
    /// Page size
    pub limit: Option<usize>,
    /// Page offset
    pub offset: Option<usize>,
}

/// Response for listing nodes
#[derive(Debug, Serialize)]
pub struct ListNodesResponse {
    /// List of nodes
    pub nodes: Vec<ClusterNode>,
    /// Total count
    pub total: usize,
    /// Current page info
    pub pagination: PaginationInfo,
}

/// Pagination information
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// Current limit
    pub limit: usize,
    /// Current offset
    pub offset: usize,
    /// Total items
    pub total: usize,
    /// Has more pages
    pub has_more: bool,
}

/// Cluster status response
#[derive(Debug, Serialize)]
pub struct ClusterStatusResponse {
    /// Cluster status
    pub status: ClusterStatus,
    /// Cluster metrics
    pub metrics: ClusterMetrics,
    /// Additional cluster information
    pub info: ClusterInfo,
}

/// Additional cluster information
#[derive(Debug, Serialize)]
pub struct ClusterInfo {
    /// Cluster ID
    pub cluster_id: String,
    /// Cluster name
    pub cluster_name: String,
    /// Total nodes
    pub total_nodes: u32,
    /// Active nodes
    pub active_nodes: u32,
    /// Failed nodes
    pub failed_nodes: u32,
    /// Cluster uptime
    pub uptime_seconds: u64,
}

/// Node health check response
#[derive(Debug, Serialize)]
pub struct NodeHealthResponse {
    /// Node ID
    pub node_id: NodeId,
    /// Health status
    pub healthy: bool,
    /// Status message
    pub status: NodeStatus,
    /// Last heartbeat
    pub last_heartbeat: String,
    /// Resource usage
    pub resources: crate::domain::entities::NodeResources,
}

#[utoipa::path(
    post,
    path = "/api/cluster/nodes",
    tag = "cluster",

    request_body = JoinNodeRequest,
    responses(
    )
)]
pub async fn join_node(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
    Json(request): Json<JoinNodeRequest>,
) -> Result<Json<JoinNodeResponse>> {
    info!("Received request to join node: {}", request.name);

    // Parse address
    let address = request
        .address
        .parse()
        .map_err(|_| ErrorUtils::invalid_format_error("address"))?;

    // Create cluster node
    let mut node = ClusterNode::new(request.name.clone(), address, request.role);

    if let Some(metadata) = request.metadata {
        node.metadata = metadata;
    }

    // Join node to cluster
    let joined_node = coordinator.join_node(node).await?;

    let response = JoinNodeResponse {
        node: joined_node,
        message: format!("Node '{}' successfully joined the cluster", request.name),
    };

    info!("Node '{}' successfully joined cluster", request.name);
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/cluster/nodes/{node_id}",
    tag = "cluster",

    params(
    ),
    request_body = LeaveNodeRequest,
    responses(
    )
)]
pub async fn leave_node(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
    Path(node_id): Path<String>,
    Json(request): Json<LeaveNodeRequest>,
) -> Result<Json<LeaveNodeResponse>> {
    let node_id = ErrorUtils::parse_uuid(&node_id, "node")?;

    info!("Received request for node {} to leave cluster", node_id);

    let reason = request
        .reason
        .unwrap_or_else(|| "Manual removal".to_string());

    coordinator.leave_node(node_id, reason.clone()).await?;

    let response = LeaveNodeResponse {
        message: format!("Node {} successfully left the cluster: {}", node_id, reason),
    };

    info!("Node {} successfully left cluster", node_id);
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/cluster/nodes",
    tag = "cluster",
    params(),
    responses()
)]
pub async fn list_nodes(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
    Query(query): Query<ListNodesQuery>,
) -> Result<Json<ListNodesResponse>> {
    info!("Received request to list cluster nodes");

    let mut nodes = if query.healthy_only.unwrap_or(false) {
        coordinator.get_active_nodes().await?
    } else {
        coordinator.get_cluster_nodes().await?
    };

    // Apply filters
    if let Some(status) = query.status {
        nodes.retain(|node| node.status == status);
    }

    if let Some(role) = query.role {
        nodes.retain(|node| node.role == role);
    }

    let total = nodes.len();
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // Apply pagination
    let paginated_nodes: Vec<ClusterNode> = nodes.into_iter().skip(offset).take(limit).collect();

    let response = ListNodesResponse {
        nodes: paginated_nodes.clone(),
        total,
        pagination: PaginationInfo {
            limit,
            offset,
            total,
            has_more: offset + paginated_nodes.len() < total,
        },
    };

    info!(
        "Returning {} nodes (total: {})",
        paginated_nodes.len(),
        total
    );
    Ok(Json(response))
}

#[utoipa::path(get, path = "/api/cluster/status", tag = "cluster", responses())]
pub async fn get_cluster_status(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
) -> Result<Json<ClusterStatusResponse>> {
    info!("Received request for cluster status");

    let status = coordinator.get_cluster_status().await;
    let metrics = coordinator.get_cluster_metrics().await;
    let cluster_info = coordinator.get_cluster_info().await;

    let response = ClusterStatusResponse {
        status,
        metrics: metrics.clone(),
        info: ClusterInfo {
            cluster_id: cluster_info.id.to_string(),
            cluster_name: cluster_info.name,
            total_nodes: metrics.total_nodes,
            active_nodes: metrics.healthy_nodes,
            failed_nodes: metrics.failed_nodes,
            uptime_seconds: 0, // TODO: Calculate actual uptime
        },
    };

    info!("Returning cluster status: {:?}", response.status);
    Ok(Json(response))
}

/// Get detailed cluster metrics
pub async fn get_cluster_metrics(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
) -> Result<Json<ClusterMetrics>> {
    info!("Received request for cluster metrics");

    let metrics = coordinator.get_cluster_metrics().await;

    info!(
        "Returning cluster metrics: {} nodes, {:.1}% health",
        metrics.total_nodes,
        metrics.health_percentage()
    );
    Ok(Json(metrics))
}

/// Get specific node information
pub async fn get_node(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
    Path(node_id): Path<String>,
) -> Result<Json<ClusterNode>> {
    let node_id = ErrorUtils::parse_uuid(&node_id, "node")?;

    info!("Received request for node information: {}", node_id);

    let nodes = coordinator.get_cluster_nodes().await?;
    let node = nodes
        .into_iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| AppError::NotFound(format!("Node {} not found", node_id)))?;

    info!("Returning node information for: {}", node_id);
    Ok(Json(node))
}

#[utoipa::path(
    get,
    path = "/api/cluster/nodes/{node_id}/health",
    tag = "cluster",
    params(),
    responses()
)]
pub async fn get_node_health(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
    Path(node_id): Path<String>,
) -> Result<Json<NodeHealthResponse>> {
    let node_id = ErrorUtils::parse_uuid(&node_id, "node")?;

    info!("Received request for node health: {}", node_id);

    let nodes = coordinator.get_cluster_nodes().await?;
    let node = nodes
        .into_iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| AppError::NotFound(format!("Node {} not found", node_id)))?;

    let response = NodeHealthResponse {
        node_id: node.id,
        healthy: node.is_healthy(),
        status: node.status,
        last_heartbeat: node.last_heartbeat.to_rfc3339(),
        resources: node.resources,
    };

    info!(
        "Returning health status for node {}: healthy={}",
        node_id, response.healthy
    );
    Ok(Json(response))
}

/// Get job distribution statistics
pub async fn get_distribution_stats(
    State(coordinator): State<std::sync::Arc<ClusterCoordinator>>,
) -> Result<Json<crate::domain::repositories::DistributionStats>> {
    info!("Received request for job distribution statistics");

    let stats = coordinator.get_distribution_stats().await?;

    info!(
        "Returning distribution stats: {} total distributions",
        stats.total_distributions
    );
    Ok(Json(stats))
}

/// Health check endpoint for the cluster service
pub async fn health_check() -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "cluster-coordinator",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[test]
    fn test_join_node_request_deserialization() {
        let json = r#"{
            "name": "test-node",
            "address": "127.0.0.1:8080",
            "role": "Worker",
            "metadata": {
                "region": "us-west-1",
                "zone": "a"
            }
        }"#;

        let request: JoinNodeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "test-node");
        assert_eq!(request.address, "127.0.0.1:8080");
        assert_eq!(request.role, NodeRole::Worker);
        assert!(request.metadata.is_some());
    }

    #[test]
    fn test_list_nodes_query_deserialization() {
        let query = "status=Active&role=Worker&healthy_only=true&limit=10&offset=0";

        // In a real test, you would use axum's query parsing
        // This is just to show the structure
        assert!(query.contains("status=Active"));
        assert!(query.contains("role=Worker"));
        assert!(query.contains("healthy_only=true"));
    }

    #[test]
    fn test_pagination_info() {
        let pagination = PaginationInfo {
            limit: 10,
            offset: 0,
            total: 25,
            has_more: true,
        };

        assert_eq!(pagination.limit, 10);
        assert_eq!(pagination.total, 25);
        assert!(pagination.has_more);
    }

    #[test]
    fn test_cluster_info_serialization() {
        let info = ClusterInfo {
            cluster_id: "test-cluster-id".to_string(),
            cluster_name: "test-cluster".to_string(),
            total_nodes: 5,
            active_nodes: 4,
            failed_nodes: 1,
            uptime_seconds: 3600,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-cluster"));
        assert!(json.contains("\"total_nodes\":5"));
    }
}

// Runner management endpoints
#[utoipa::path(get, path = "/api/cluster/runners", tag = "cluster", responses())]
pub async fn list_runners() -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "runners": [],
        "total": 0
    })))
}

#[utoipa::path(
    post,
    path = "/api/cluster/runners",
    tag = "cluster",
    request_body = CreateRunnerRequest,
    responses(
    )
)]
pub async fn create_runner(
    Json(_request): Json<CreateRunnerRequest>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "message": "Runner creation not yet implemented"
    })))
}

#[utoipa::path(
    get,
    path = "/api/cluster/runners/{runner_id}/status",
    tag = "cluster",
    params(),
    responses()
)]
pub async fn get_runner_status(Path(_runner_id): Path<String>) -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "status": "unknown",
        "message": "Runner status not yet implemented"
    })))
}

#[utoipa::path(
    delete,
    path = "/api/cluster/runners/{runner_id}",
    tag = "cluster",
    params(),
    responses()
)]
pub async fn delete_runner(Path(_runner_id): Path<String>) -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "message": "Runner deletion not yet implemented"
    })))
}

#[utoipa::path(
    get,
    path = "/api/cluster/runners/{runner_id}/metrics",
    tag = "cluster",
    params(),
    responses()
)]
pub async fn get_runner_metrics(Path(_runner_id): Path<String>) -> Result<Json<RunnerMetrics>> {
    // Placeholder implementation
    Ok(Json(RunnerMetrics {
        cpu_usage: 0.0,
        memory_usage: 0.0,
        active_jobs: 0,
        completed_jobs: 0,
        failed_jobs: 0,
    }))
}

// Job management endpoints
#[utoipa::path(get, path = "/api/cluster/jobs", tag = "jobs", responses())]
pub async fn list_jobs() -> Result<Json<serde_json::Value>> {
    // Placeholder implementation
    Ok(Json(serde_json::json!({
        "jobs": [],
        "total": 0
    })))
}

#[utoipa::path(
    get,
    path = "/api/cluster/jobs/{job_id}/status",
    tag = "jobs",
    params(),
    responses()
)]
pub async fn get_job_status(Path(_job_id): Path<String>) -> Result<Json<serde_json::Value>> {
    // Placeholder implementation
    Ok(Json(serde_json::json!({
        "status": "unknown",
        "message": "Job status not yet implemented"
    })))
}

#[utoipa::path(
    post,
    path = "/api/cluster/jobs/{job_id}/cancel",
    tag = "jobs",
    params(),
    responses()
)]
pub async fn cancel_job(Path(_job_id): Path<String>) -> Result<Json<serde_json::Value>> {
    // Placeholder implementation
    Ok(Json(serde_json::json!({
        "message": "Job cancellation not yet implemented"
    })))
}

#[utoipa::path(
    post,
    path = "/api/cluster/jobs/{job_id}/retry",
    tag = "jobs",
    params(),
    responses()
)]
pub async fn retry_job(Path(_job_id): Path<String>) -> Result<Json<serde_json::Value>> {
    // Placeholder implementation
    Ok(Json(serde_json::json!({
        "message": "Job retry not yet implemented"
    })))
}

// Alias functions for OpenAPI documentation
// Removed unused alternative function names - these can be imported directly when needed
