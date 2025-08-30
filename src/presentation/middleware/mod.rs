// This file declares middleware modules and re-exports their contents
// This allows other parts of the code to use `use crate::middleware::function_name`

pub mod auth;
pub mod comprehensive;
pub mod enhanced_pipeline;
pub mod enhanced_validation;
pub mod pipeline;
pub mod pipeline_manager;
pub mod rate_limit;
pub mod response_optimizer;
pub mod security_pipeline;
pub mod validation;

// Tests are handled in individual module files

// Re-export key public items from the modules
pub use auth::auth;
pub use comprehensive::comprehensive_security_middleware;
pub use pipeline::create_cors_middleware;
pub use rate_limit::RateLimiter;
pub use response_optimizer::{
    ResponseOptimizer, ResponseOptimizerConfig, create_response_optimizer_middleware,
    ResponseOptimizationSummary
};
// Removed unused import: rate_limit_middleware, security_headers_middleware

use crate::{error::Result, AppState};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use tracing::{debug, info};

/// Middleware pipeline orchestrator that manages the order and execution of all middleware
pub struct MiddlewarePipeline {
    state: AppState,
}

impl MiddlewarePipeline {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Main middleware pipeline that orchestrates all security and validation middleware
    pub async fn process_request(
        State(state): State<AppState>,
        req: Request,
        next: Next,
    ) -> Result<Response> {
        debug!("🔄 Starting middleware pipeline");

        // The middleware pipeline is applied in reverse order due to how Axum layers work
        // So the actual execution order will be:
        // 1. CORS (handled by tower-http)
        // 2. Tracing (handled by tower-http)
        // 3. Compression (handled by tower-http)
        // 4. Comprehensive security middleware (this function)
        //    - Rate limiting
        //    - Request validation
        //    - Authentication & authorization
        //    - Security headers
        //    - Audit logging

        let response = comprehensive_security_middleware(State(state), req, next).await;

        debug!("✅ Middleware pipeline completed");
        Ok(response)
    }
}

/// Create a properly ordered middleware stack for the application
pub fn create_middleware_stack(_state: AppState) -> Vec<Box<dyn Fn() + Send + Sync>> {
    let middleware_stack = Vec::new();

    // Add middleware in the order they should be applied
    // Note: Axum applies middleware in reverse order, so we add them in reverse

    info!("🔧 Building comprehensive middleware stack");

    // This is a placeholder - in practice, middleware is applied using .layer() in main.rs
    // This function serves as documentation of the intended middleware order

    middleware_stack
}
