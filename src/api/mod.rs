//! Valkyrie Protocol API Module
//!
//! This module provides the external API for the Valkyrie Protocol,
//! including high-level abstractions and developer-friendly interfaces.

pub mod valkyrie;
pub mod codegen;
pub mod adapters;
pub mod conversions;
pub mod versioning;
pub mod routing;
pub mod errors;
pub mod error_middleware;
pub mod error_docs;
pub mod documentation;
pub mod interactive_docs;
pub mod rate_limiting;
pub mod rate_limit_monitoring;
pub mod authentication;
pub mod response_optimization;

pub use valkyrie::ValkyrieEngineAdapter;
pub use codegen::{CodeGenerator, extract_api_definition};
pub use versioning::{ApiVersion, ApiVersionExtractor, api_versioning_middleware};
pub use routing::{VersionedRouter, VersionedResponse, MigrationGuideGenerator};
pub use errors::{ApiError, ApiErrorBuilder, ApiErrorCode, ApiErrorResponse, create_error_response};
pub use error_middleware::{api_error_middleware, ApiResult, IntoApiError};
pub use documentation::{EnhancedApiDoc, ApiVersionInfo, PipelineRequest, ExecutionRequest};
pub use interactive_docs::interactive_docs_router;
pub use rate_limiting::{RateLimitConfig, RateLimitService, rate_limiting_middleware};
pub use rate_limit_monitoring::{RateLimitMonitor, RateLimitMonitoringConfig};
pub use authentication::{AuthService, JwtConfig, auth_middleware, optional_auth_middleware};
pub use response_optimization::{ResponseOptimizationService, PaginationConfig, response_optimization_middleware};