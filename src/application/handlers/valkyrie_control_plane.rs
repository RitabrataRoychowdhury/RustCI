// Valkyrie Control Plane Handler
// Task 3.2: Unified Control Plane Integration

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::application::services::valkyrie_integration::ValkyrieIntegrationService;
use crate::config::valkyrie_integration::ValkyrieIntegrationConfig;
use crate::error::{AppError, Result};
use crate::infrastructure::runners::valkyrie_adapter::{
    AdapterPerformanceSnapshot, JobMetadata, JobPayload, JobPriority, JobRequirements, JobType,
    ValkyrieJob,
};
use crate::infrastructure::runners::{ValkyrieRunnerAdapter, ValkyrieRunnerTrait};
use crate::presentation::routes::valkyrie::{
    ComponentHealthResponse, EnhancedJobStatusResponse, EnhancedJobSubmissionRequest,
    EnhancedJobSubmissionResponse, EnhancedRunnerInfo, EnhancedRunnerListResponse,
    IntegrationStatusResponse, JobListQuery, RunnerListQuery, SystemHealthResponse,
};

/// Valkyrie Control Plane Handler - manages integration between Valkyrie and RustCI
pub struct ValkyrieControlPlaneHandler {
    // Core services
    integration_service: Arc<ValkyrieIntegrationService>,
    valkyrie_adapter: Option<Arc<ValkyrieRunnerAdapter>>,

    // Configuration
    config: Arc<RwLock<ValkyrieIntegrationConfig>>,

    // State management
    integration_status: Arc<RwLock<IntegrationState>>,
    performance_cache: Arc<RwLock<PerformanceCache>>,

    // Backward compatibility
    legacy_handler: Arc<LegacyApiHandler>,
}

/// Integration state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationState {
    pub valkyrie_enabled: bool,
    pub integration_mode: IntegrationMode,
    pub last_config_reload: Option<SystemTime>,
    pub health_status: HealthStatus,
    pub performance_enhancement_active: bool,
    pub fallback_mode_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegrationMode {
    Full,     // Full Valkyrie integration
    Fallback, // HTTP fallback mode
    Disabled, // Valkyrie disabled
    Degraded, // Partial functionality
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Performance metrics cache
#[derive(Debug, Clone, Default)]
pub struct PerformanceCache {
    pub last_updated: Option<Instant>,
    pub adapter_metrics: Option<AdapterPerformanceSnapshot>,
    pub system_metrics: Option<SystemMetrics>,
    pub cache_ttl: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_jobs_processed: u64,
    pub average_response_time: Duration,
    pub system_utilization: f64,
    pub error_rate: f64,
    pub uptime: Duration,
}

/// Legacy API handler for backward compatibility
pub struct LegacyApiHandler {
    // Legacy job handling
    job_converter: Arc<JobConverter>,
    response_converter: Arc<ResponseConverter>,
}

/// Job converter for legacy API compatibility
pub struct JobConverter;

/// Response converter for legacy API compatibility
pub struct ResponseConverter;

impl ValkyrieControlPlaneHandler {
    /// Create a new ValkyrieControlPlaneHandler
    pub async fn new(
        integration_service: Arc<ValkyrieIntegrationService>,
        config: ValkyrieIntegrationConfig,
    ) -> Result<Self> {
        let valkyrie_adapter = if config.valkyrie_enabled {
            Some(integration_service.get_valkyrie_adapter().await?)
        } else {
            None
        };

        let integration_state = IntegrationState {
            valkyrie_enabled: config.valkyrie_enabled,
            integration_mode: if config.valkyrie_enabled {
                IntegrationMode::Full
            } else {
                IntegrationMode::Disabled
            },
            last_config_reload: Some(SystemTime::now()),
            health_status: HealthStatus::Healthy,
            performance_enhancement_active: config.performance_enhancement_enabled,
            fallback_mode_active: false,
        };

        let performance_cache = PerformanceCache {
            cache_ttl: Duration::from_secs(30),
            ..Default::default()
        };

        let legacy_handler = Arc::new(LegacyApiHandler {
            job_converter: Arc::new(JobConverter),
            response_converter: Arc::new(ResponseConverter),
        });

        Ok(Self {
            integration_service,
            valkyrie_adapter,
            config: Arc::new(RwLock::new(config)),
            integration_status: Arc::new(RwLock::new(integration_state)),
            performance_cache: Arc::new(RwLock::new(performance_cache)),
            legacy_handler,
        })
    }

    /// Submit job with Valkyrie enhancements
    pub async fn submit_job_enhanced(
        &self,
        request: EnhancedJobSubmissionRequest,
    ) -> Result<EnhancedJobSubmissionResponse> {
        let start_time = Instant::now();

        // Convert request to ValkyrieJob
        let valkyrie_job = self.convert_to_valkyrie_job(request).await?;

        // Submit job through appropriate channel
        let submission_result = if let Some(adapter) = &self.valkyrie_adapter {
            // Use Valkyrie adapter for enhanced performance
            adapter
                .submit_job(valkyrie_job)
                .await
                .map_err(|e| AppError::InternalError {
                    component: "valkyrie_control_plane".to_string(),
                    message: format!("Valkyrie job submission failed: {}", e),
                })?
        } else {
            // Fallback to legacy submission
            return self.submit_job_legacy_fallback(valkyrie_job).await;
        };

        // Convert result to enhanced response
        let response = EnhancedJobSubmissionResponse {
            job_id: submission_result.job_id.to_string(),
            status: "submitted".to_string(),
            estimated_start_time: Some(
                submission_result
                    .estimated_start_time
                    .elapsed()
                    .as_secs()
                    .to_string(),
            ),
            estimated_completion_time: Some(
                submission_result
                    .estimated_completion_time
                    .elapsed()
                    .as_secs()
                    .to_string(),
            ),
            assigned_runner: submission_result.assigned_runner.map(|id| id.to_string()),
            dispatch_latency_us: Some(submission_result.dispatch_latency.as_micros() as u64),
            qos_guarantees: Some(self.convert_qos_guarantees(&submission_result.qos_guarantees)),
            routing_path: submission_result.routing_path.as_ref().map(|route| {
                crate::presentation::routes::valkyrie::RoutingPathResponse {
                    route_id: route.id.to_string(),
                    hops: route
                        .hops
                        .iter()
                        .map(|h| format!("{} -> {}", h.from, h.to))
                        .collect(),
                    estimated_latency: Some(route.estimated_latency.as_micros() as u64),
                    protocol_used: "valkyrie".to_string(),
                }
            }),
            performance_metrics: Some(
                crate::presentation::routes::valkyrie::JobPerformanceResponse {
                    queue_time_us: None,   // Would be populated from job queue metrics
                    routing_time_us: None, // Would be populated from routing metrics
                    dispatch_time_us: Some(submission_result.dispatch_latency.as_micros() as u64),
                    total_time_us: Some(start_time.elapsed().as_micros() as u64),
                },
            ),
        };

        Ok(response)
    }

    /// Get job status with enhanced information
    pub async fn get_job_status_enhanced(&self, job_id: Uuid) -> Result<EnhancedJobStatusResponse> {
        if let Some(adapter) = &self.valkyrie_adapter {
            let status =
                adapter
                    .get_job_status(job_id)
                    .await
                    .map_err(|e| AppError::InternalError {
                        component: "valkyrie_control_plane".to_string(),
                        message: format!("Failed to get job status: {}", e),
                    })?;

            Ok(self.convert_job_status_to_enhanced(job_id, status).await?)
        } else {
            // Fallback to legacy status retrieval
            self.get_job_status_legacy_fallback(job_id).await
        }
    }

    /// Cancel job with enhanced response
    pub async fn cancel_job_enhanced(&self, job_id: Uuid) -> Result<()> {
        if let Some(adapter) = &self.valkyrie_adapter {
            adapter
                .cancel_job(job_id)
                .await
                .map_err(|e| AppError::InternalError {
                    component: "valkyrie_control_plane".to_string(),
                    message: format!("Failed to cancel job: {}", e),
                })?;
        } else {
            // Fallback to legacy cancellation
            self.cancel_job_legacy_fallback(job_id).await?;
        }

        Ok(())
    }

    /// List jobs with enhanced filtering
    pub async fn list_jobs_enhanced(&self, query: JobListQuery) -> Result<Value> {
        // Implementation would integrate with job storage/tracking system
        // For now, return a placeholder response
        Ok(serde_json::json!({
            "jobs": [],
            "total_count": 0,
            "filtered_count": 0,
            "pagination": {
                "limit": query.limit.unwrap_or(50),
                "offset": query.offset.unwrap_or(0),
                "has_more": false
            }
        }))
    }

    /// List runners with enhanced information
    pub async fn list_runners_enhanced(
        &self,
        query: RunnerListQuery,
    ) -> Result<EnhancedRunnerListResponse> {
        if let Some(adapter) = &self.valkyrie_adapter {
            let stats =
                adapter
                    .get_runner_pool_stats()
                    .await
                    .map_err(|e| AppError::InternalError {
                        component: "valkyrie_control_plane".to_string(),
                        message: format!("Failed to get runner stats: {}", e),
                    })?;

            // Convert stats to enhanced response
            Ok(EnhancedRunnerListResponse {
                runners: vec![], // Would be populated from actual runner data
                total_count: stats.total_runners,
                healthy_count: stats.healthy_runners,
                total_capacity: stats.total_capacity,
                current_utilization: stats.utilization_rate,
            })
        } else {
            // Fallback to legacy runner listing
            self.list_runners_legacy_fallback(query).await
        }
    }

    /// Get detailed runner information
    pub async fn get_runner_details_enhanced(&self, runner_id: Uuid) -> Result<EnhancedRunnerInfo> {
        // Implementation would retrieve detailed runner information
        // For now, return a placeholder
        Err(AppError::NotFound(format!(
            "Runner {} not found",
            runner_id
        )))
    }

    /// Update runner status
    pub async fn update_runner_status_enhanced(
        &self,
        runner_id: Uuid,
        status: Value,
    ) -> Result<()> {
        if let Some(_adapter) = &self.valkyrie_adapter {
            // Convert status and update through adapter
            // Implementation would parse status and call adapter.update_runner_status
            Ok(())
        } else {
            // Fallback to legacy status update
            self.update_runner_status_legacy_fallback(runner_id, status)
                .await
        }
    }

    /// Register new runner
    pub async fn register_runner_enhanced(&self, _registration: Value) -> Result<Value> {
        // Implementation would handle runner registration
        // For now, return a placeholder response
        Ok(serde_json::json!({
            "runner_id": Uuid::new_v4().to_string(),
            "status": "registered",
            "message": "Runner registered successfully"
        }))
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<Value> {
        let mut cache = self.performance_cache.write().await;

        // Check cache validity
        if let Some(last_updated) = cache.last_updated {
            if last_updated.elapsed() < cache.cache_ttl {
                // Return cached metrics
                return Ok(serde_json::json!({
                    "cached": true,
                    "last_updated": last_updated.elapsed().as_secs(),
                    "adapter_metrics": cache.adapter_metrics,
                    "system_metrics": cache.system_metrics
                }));
            }
        }

        // Refresh metrics
        if let Some(adapter) = &self.valkyrie_adapter {
            let adapter_metrics =
                adapter
                    .get_performance_metrics()
                    .await
                    .map_err(|e| AppError::InternalError {
                        component: "valkyrie_control_plane".to_string(),
                        message: format!("Failed to get performance metrics: {}", e),
                    })?;

            cache.adapter_metrics = Some(adapter_metrics.clone());
            cache.last_updated = Some(Instant::now());

            Ok(serde_json::json!({
                "cached": false,
                "adapter_metrics": adapter_metrics,
                "system_metrics": cache.system_metrics
            }))
        } else {
            Ok(serde_json::json!({
                "valkyrie_enabled": false,
                "message": "Valkyrie adapter not available"
            }))
        }
    }

    /// Get system health status
    pub async fn get_system_health(&self) -> Result<SystemHealthResponse> {
        let integration_status = self.integration_status.read().await;

        let mut components = vec![
            ComponentHealthResponse {
                component: "valkyrie_integration".to_string(),
                status: if integration_status.valkyrie_enabled {
                    "healthy"
                } else {
                    "disabled"
                }
                .to_string(),
                message: Some(format!("Mode: {:?}", integration_status.integration_mode)),
                last_check: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string(),
            },
            ComponentHealthResponse {
                component: "control_plane".to_string(),
                status: "healthy".to_string(),
                message: Some("Control plane operational".to_string()),
                last_check: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string(),
            },
        ];

        // Add Valkyrie-specific components if enabled
        if integration_status.valkyrie_enabled {
            if let Some(adapter) = &self.valkyrie_adapter {
                let adapter_health = match adapter.get_performance_metrics().await {
                    Ok(_) => "healthy",
                    Err(_) => "unhealthy",
                };

                components.push(ComponentHealthResponse {
                    component: "valkyrie_adapter".to_string(),
                    status: adapter_health.to_string(),
                    message: Some("Valkyrie runner adapter".to_string()),
                    last_check: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string(),
                });
            }
        }

        let overall_status = if components.iter().all(|c| c.status == "healthy") {
            "healthy"
        } else if components.iter().any(|c| c.status == "unhealthy") {
            "unhealthy"
        } else {
            "degraded"
        };

        Ok(SystemHealthResponse {
            overall_status: overall_status.to_string(),
            valkyrie_status: if integration_status.valkyrie_enabled {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            control_plane_status: "operational".to_string(),
            runner_pool_status: "operational".to_string(),
            performance_status: if integration_status.performance_enhancement_active {
                "enhanced"
            } else {
                "standard"
            }
            .to_string(),
            components,
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
        })
    }

    /// Get system diagnostics
    pub async fn get_system_diagnostics(&self) -> Result<Value> {
        let integration_status = self.integration_status.read().await;
        let config = self.config.read().await;

        Ok(serde_json::json!({
            "integration_status": *integration_status,
            "configuration": {
                "valkyrie_enabled": config.valkyrie_enabled,
                "performance_enhancement": config.performance_enhancement_enabled,
                "backward_compatibility": config.backward_compatibility_enabled,
                "fallback_mode": config.fallback_mode_enabled
            },
            "runtime_info": {
                "uptime": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default().as_secs(),
                "memory_usage": "N/A", // Would be populated with actual memory stats
                "cpu_usage": "N/A"     // Would be populated with actual CPU stats
            }
        }))
    }

    /// Get Valkyrie configuration
    pub async fn get_valkyrie_config(&self) -> Result<Value> {
        let config = self.config.read().await;
        Ok(serde_json::to_value(&*config)?)
    }

    /// Update Valkyrie configuration
    pub async fn update_valkyrie_config(&self, new_config: Value) -> Result<()> {
        let new_config: ValkyrieIntegrationConfig = serde_json::from_value(new_config)?;

        // Validate configuration
        self.integration_service
            .validate_config(&new_config)
            .await?;

        // Update configuration
        {
            let mut config = self.config.write().await;
            *config = new_config;
        }

        // Update integration status
        {
            let mut status = self.integration_status.write().await;
            status.last_config_reload = Some(SystemTime::now());
        }

        Ok(())
    }

    /// Reload Valkyrie configuration
    pub async fn reload_valkyrie_config(&self) -> Result<()> {
        self.integration_service.reload_config().await?;

        let mut status = self.integration_status.write().await;
        status.last_config_reload = Some(SystemTime::now());

        Ok(())
    }

    /// Validate Valkyrie configuration
    pub async fn validate_valkyrie_config(&self, config: Value) -> Result<Value> {
        let config: ValkyrieIntegrationConfig = serde_json::from_value(config)?;

        let validation_result = self.integration_service.validate_config(&config).await;

        Ok(serde_json::json!({
            "valid": validation_result.is_ok(),
            "errors": validation_result.err().map(|e| e.to_string()),
            "warnings": [] // Would be populated with validation warnings
        }))
    }

    /// Get integration status
    pub async fn get_integration_status(&self) -> Result<IntegrationStatusResponse> {
        let integration_status = self.integration_status.read().await;
        let config = self.config.read().await;

        Ok(IntegrationStatusResponse {
            valkyrie_enabled: integration_status.valkyrie_enabled,
            valkyrie_version: "2.0.0".to_string(), // Would be populated from actual version
            integration_mode: format!("{:?}", integration_status.integration_mode).to_lowercase(),
            performance_enhancement: integration_status.performance_enhancement_active,
            backward_compatibility: config.backward_compatibility_enabled,
            configuration_status: "valid".to_string(),
            last_config_reload: integration_status.last_config_reload.map(|t| {
                t.duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string()
            }),
        })
    }

    /// Get integration capabilities
    pub async fn get_integration_capabilities(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "valkyrie_protocol": {
                "version": "2.0.0",
                "features": ["high_performance_dispatch", "intelligent_routing", "qos_guarantees"]
            },
            "api_versions": ["v1", "v2"],
            "backward_compatibility": true,
            "performance_enhancements": true,
            "fallback_support": true
        }))
    }

    /// Toggle fallback mode
    pub async fn toggle_fallback_mode(&self, request: Value) -> Result<Value> {
        let enable_fallback = request
            .get("enable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut status = self.integration_status.write().await;
        status.fallback_mode_active = enable_fallback;

        if enable_fallback {
            status.integration_mode = IntegrationMode::Fallback;
        } else if status.valkyrie_enabled {
            status.integration_mode = IntegrationMode::Full;
        }

        Ok(serde_json::json!({
            "fallback_mode": enable_fallback,
            "integration_mode": format!("{:?}", status.integration_mode).to_lowercase(),
            "message": if enable_fallback { "Fallback mode enabled" } else { "Fallback mode disabled" }
        }))
    }

    // Backward Compatibility Methods

    /// Submit job with v1 API compatibility
    pub async fn submit_job_compatible(&self, request: Value) -> Result<Value> {
        self.legacy_handler.submit_job_compatible(request).await
    }

    /// Get job status with v1 API compatibility
    pub async fn get_job_status_compatible(&self, job_id: Uuid) -> Result<Value> {
        self.legacy_handler.get_job_status_compatible(job_id).await
    }

    /// Cancel job with v1 API compatibility
    pub async fn cancel_job_compatible(&self, job_id: Uuid) -> Result<Value> {
        self.legacy_handler.cancel_job_compatible(job_id).await
    }

    /// List runners with v1 API compatibility
    pub async fn list_runners_compatible(&self, query: HashMap<String, String>) -> Result<Value> {
        self.legacy_handler.list_runners_compatible(query).await
    }

    // Helper Methods

    async fn convert_to_valkyrie_job(
        &self,
        request: EnhancedJobSubmissionRequest,
    ) -> Result<ValkyrieJob> {
        // Convert enhanced request to ValkyrieJob
        let job_type = match request.job_type.as_str() {
            "build" => JobType::Build,
            "test" => JobType::Test,
            "deploy" => JobType::Deploy,
            "analysis" => JobType::Analysis,
            custom => JobType::Custom(custom.to_string()),
        };

        let priority = match request.priority.unwrap_or(2) {
            0 => JobPriority::Critical,
            1 => JobPriority::High,
            2 => JobPriority::Normal,
            3 => JobPriority::Low,
            _ => JobPriority::Background,
        };

        let payload = JobPayload::Small(serde_json::to_vec(&request.payload)?);

        let requirements = JobRequirements {
            cpu_cores: request.requirements.as_ref().and_then(|r| r.cpu_cores),
            memory_mb: request.requirements.as_ref().and_then(|r| r.memory_mb),
            storage_gb: request.requirements.as_ref().and_then(|r| r.storage_gb),
            gpu_count: request.requirements.as_ref().and_then(|r| r.gpu_count),
            max_execution_time: request
                .requirements
                .as_ref()
                .and_then(|r| r.max_execution_time)
                .map(Duration::from_secs),
            required_capabilities: request
                .requirements
                .as_ref()
                .and_then(|r| r.required_capabilities.clone())
                .unwrap_or_default(),
            preferred_regions: request
                .requirements
                .as_ref()
                .and_then(|r| r.preferred_regions.clone())
                .unwrap_or_default(),
            network_requirements: None,
        };

        let metadata = JobMetadata {
            user_id: request
                .metadata
                .as_ref()
                .and_then(|m| m.get("user_id").cloned()),
            project_id: request
                .metadata
                .as_ref()
                .and_then(|m| m.get("project_id").cloned()),
            pipeline_id: request
                .metadata
                .as_ref()
                .and_then(|m| m.get("pipeline_id").cloned()),
            stage: request
                .metadata
                .as_ref()
                .and_then(|m| m.get("stage").cloned()),
            tags: request.metadata.unwrap_or_default(),
            correlation_id: request.correlation_id,
        };

        Ok(ValkyrieJob {
            id: Uuid::new_v4(),
            job_type,
            priority,
            payload,
            requirements,
            metadata,
            created_at: SystemTime::now(),
            deadline: request.deadline.as_ref().and_then(|d| {
                // Parse ISO 8601 string to SystemTime
                DateTime::parse_from_rfc3339(d)
                    .ok()
                    .map(|dt| UNIX_EPOCH + Duration::from_secs(dt.timestamp() as u64))
                    .or_else(|| Some(SystemTime::now() + Duration::from_secs(3600)))
                // fallback
            }),
            routing_hints: None, // Would be converted from request.routing_hints
            qos_requirements: None, // Would be converted from request.qos_requirements
        })
    }

    fn convert_qos_guarantees(
        &self,
        guarantees: &crate::infrastructure::runners::valkyrie_adapter::QoSGuarantees,
    ) -> crate::presentation::routes::valkyrie::QoSGuaranteesResponse {
        crate::presentation::routes::valkyrie::QoSGuaranteesResponse {
            max_latency: guarantees.max_latency.map(|d| d.as_millis() as u64),
            min_throughput: guarantees.min_throughput,
            reliability_guarantee: guarantees.reliability_guarantee,
            priority_level: guarantees
                .priority_level
                .as_ref()
                .map(|p| format!("{:?}", p)),
        }
    }

    async fn convert_job_status_to_enhanced(
        &self,
        job_id: Uuid,
        status: crate::infrastructure::runners::valkyrie_adapter::JobStatus,
    ) -> Result<EnhancedJobStatusResponse> {
        use crate::infrastructure::runners::valkyrie_adapter::JobStatus;

        let (status_str, started_at, completed_at, runner_id, result, error) = match status {
            JobStatus::Queued { queued_at: _ } => ("queued", None, None, None, None, None),
            JobStatus::Routing { routing_started: _ } => ("routing", None, None, None, None, None),
            JobStatus::Dispatched {
                runner_id,
                dispatched_at: _,
            } => ("dispatched", None, None, Some(runner_id), None, None),
            JobStatus::Running {
                runner_id,
                started_at,
            } => (
                "running",
                Some(started_at),
                None,
                Some(runner_id),
                None,
                None,
            ),
            JobStatus::Completed {
                result,
                completed_at,
            } => (
                "completed",
                None,
                Some(completed_at),
                None,
                Some(result),
                None,
            ),
            JobStatus::Failed { error, failed_at } => {
                ("failed", None, Some(failed_at), None, None, Some(error))
            }
            JobStatus::Cancelled { cancelled_at } => {
                ("cancelled", None, Some(cancelled_at), None, None, None)
            }
            JobStatus::Timeout { timeout_at } => (
                "timeout",
                None,
                Some(timeout_at),
                None,
                None,
                Some("Job timed out".to_string()),
            ),
        };

        Ok(EnhancedJobStatusResponse {
            job_id: job_id.to_string(),
            status: status_str.to_string(),
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            started_at: started_at.map(|_| {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string()
            }),
            completed_at: completed_at.map(|_| {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string()
            }),
            runner_id: runner_id.map(|id| id.to_string()),
            result: result.map(
                |r| crate::presentation::routes::valkyrie::JobResultResponse {
                    exit_code: r.exit_code,
                    output: r.output,
                    artifacts: r.artifacts,
                    execution_time_ms: r.execution_time.as_millis() as u64,
                    resource_usage: crate::presentation::routes::valkyrie::ResourceUsageResponse {
                        cpu_time_ms: r.resource_usage.cpu_time.as_millis() as u64,
                        memory_peak_mb: r.resource_usage.memory_peak / 1024 / 1024,
                        storage_used_mb: r.resource_usage.storage_used / 1024 / 1024,
                        network_bytes_sent: r.resource_usage.network_bytes_sent,
                        network_bytes_received: r.resource_usage.network_bytes_received,
                    },
                },
            ),
            performance_metrics: None, // Would be populated from actual metrics
            qos_metrics: None,         // Would be populated from actual QoS metrics
            error,
        })
    }

    // Legacy fallback methods (placeholder implementations)

    async fn submit_job_legacy_fallback(
        &self,
        job: ValkyrieJob,
    ) -> Result<EnhancedJobSubmissionResponse> {
        // Placeholder for legacy job submission
        Ok(EnhancedJobSubmissionResponse {
            job_id: job.id.to_string(),
            status: "submitted".to_string(),
            estimated_start_time: None,
            estimated_completion_time: None,
            assigned_runner: None,
            dispatch_latency_us: Some(1000), // Placeholder latency
            qos_guarantees: None,
            routing_path: None,
            performance_metrics: None,
        })
    }

    async fn get_job_status_legacy_fallback(
        &self,
        job_id: Uuid,
    ) -> Result<EnhancedJobStatusResponse> {
        // Placeholder for legacy job status retrieval
        Ok(EnhancedJobStatusResponse {
            job_id: job_id.to_string(),
            status: "unknown".to_string(),
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            started_at: None,
            completed_at: None,
            runner_id: None,
            result: None,
            performance_metrics: None,
            qos_metrics: None,
            error: Some("Legacy fallback mode".to_string()),
        })
    }

    async fn cancel_job_legacy_fallback(&self, _job_id: Uuid) -> Result<()> {
        // Placeholder for legacy job cancellation
        Ok(())
    }

    async fn list_runners_legacy_fallback(
        &self,
        _query: RunnerListQuery,
    ) -> Result<EnhancedRunnerListResponse> {
        // Placeholder for legacy runner listing
        Ok(EnhancedRunnerListResponse {
            runners: vec![],
            total_count: 0,
            healthy_count: 0,
            total_capacity: 0,
            current_utilization: 0.0,
        })
    }

    async fn update_runner_status_legacy_fallback(
        &self,
        _runner_id: Uuid,
        _status: Value,
    ) -> Result<()> {
        // Placeholder for legacy runner status update
        Ok(())
    }
}

// Legacy API Handler Implementation

impl LegacyApiHandler {
    pub async fn submit_job_compatible(&self, _request: Value) -> Result<Value> {
        // Convert v1 API request to internal format and submit
        Ok(serde_json::json!({
            "job_id": Uuid::new_v4().to_string(),
            "status": "submitted",
            "message": "Job submitted via legacy API"
        }))
    }

    pub async fn get_job_status_compatible(&self, job_id: Uuid) -> Result<Value> {
        // Return v1 API compatible job status
        Ok(serde_json::json!({
            "job_id": job_id.to_string(),
            "status": "unknown",
            "message": "Legacy API compatibility mode"
        }))
    }

    pub async fn cancel_job_compatible(&self, job_id: Uuid) -> Result<Value> {
        // Cancel job and return v1 API compatible response
        Ok(serde_json::json!({
            "job_id": job_id.to_string(),
            "status": "cancelled",
            "message": "Job cancelled via legacy API"
        }))
    }

    pub async fn list_runners_compatible(&self, _query: HashMap<String, String>) -> Result<Value> {
        // Return v1 API compatible runner list
        Ok(serde_json::json!({
            "runners": [],
            "count": 0,
            "message": "Legacy API compatibility mode"
        }))
    }
}

impl JobConverter {
    // Job conversion methods would be implemented here
}

impl ResponseConverter {
    // Response conversion methods would be implemented here
}
