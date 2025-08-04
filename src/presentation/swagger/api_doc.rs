use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;

use crate::{
    domain::entities::{
        cluster::{ClusterNode, ClusterStatus},
        runner::{Job, JobStatus, RunnerEntity, RunnerStatus, RunnerType},
    },
    ci::config::{
        PipelineType, CIPipeline, ServerConfig, SimpleStep, MatrixConfig, 
        CacheConfig, IncludeConfig, PipelineJob, JobScript
    },
    application::handlers::ci::{
        CreatePipelineRequest, TriggerPipelineRequest, ExecutePipelineRequest,
        PipelineResponse, PipelineStatus, ExecutionResponse, TriggerResponse
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::application::handlers::ci::create_pipeline,
        crate::application::handlers::ci::create_pipeline_multipart,
        crate::application::handlers::ci::list_pipelines,
        crate::application::handlers::ci::get_pipeline,
        crate::application::handlers::ci::get_pipeline_yaml,
        crate::application::handlers::ci::trigger_pipeline,
        crate::application::handlers::ci::get_execution,
        crate::application::handlers::ci::list_executions,
    ),
    components(
        schemas(
            // Domain entities
            ClusterNode,
            ClusterStatus,
            RunnerEntity,
            RunnerType,
            RunnerStatus,
            Job,
            JobStatus,

            // Pipeline types and configurations
            PipelineType,
            CIPipeline,
            ServerConfig,
            SimpleStep,
            MatrixConfig,
            CacheConfig,
            IncludeConfig,
            PipelineJob,
            JobScript,

            // API request/response types
            CreatePipelineRequest,
            TriggerPipelineRequest,
            ExecutePipelineRequest,
            PipelineResponse,
            PipelineStatus,
            ExecutionResponse,
            TriggerResponse,

            // Common types
            crate::error::ApiError,
        )
    ),
    tags(
        (name = "auth", description = "Authentication and authorization endpoints"),
        (name = "oauth", description = "OAuth integration endpoints"),
        (name = "ci", description = "CI/CD pipeline management"),
        (name = "cluster", description = "Cluster and runner management"),
        (name = "jobs", description = "Job execution and monitoring"),
        (name = "dockerfile", description = "Dockerfile generation and validation"),
        (name = "pr", description = "Pull request automation"),
        (name = "repository", description = "Repository management"),
        (name = "workspace", description = "Workspace management"),
        (name = "health", description = "Health check and monitoring"),
    ),
    info(
        title = "RustCI API",
        version = "1.0.0",
        description = "A high-performance CI/CD platform built in Rust",
        contact(
            name = "RustCI Team",
            email = "support@rustci.dev"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development server"),
        (url = "https://api.rustci.dev", description = "Production server")
    )
)]
pub struct ApiDoc;

impl ApiDoc {
    pub fn with_security() -> utoipa::openapi::OpenApi {
        let mut openapi = Self::openapi();

        // Add JWT Bearer authentication
        let jwt_auth = SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("JWT")
                .description(Some("JWT token for API authentication"))
                .build(),
        );

        openapi
            .components
            .as_mut()
            .unwrap()
            .add_security_scheme("jwt_auth", jwt_auth);

        openapi
    }
}
