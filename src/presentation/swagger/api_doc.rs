use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;

use crate::domain::entities::{
    cluster::{ClusterNode, ClusterStatus},
    runner::{Job, JobStatus, RunnerEntity, RunnerStatus, RunnerType},
};

#[derive(OpenApi)]
#[openapi(
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
