//! API Versioning and Backward Compatibility System
//!
//! This module provides comprehensive API versioning support with backward compatibility
//! guarantees, migration guides, and version-specific routing.

use axum::{
    extract::{Path, Request},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use tracing::{debug, warn};

/// Supported API versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ApiVersion {
    V1,
    V2,
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiVersion::V1 => write!(f, "v1"),
            ApiVersion::V2 => write!(f, "v2"),
        }
    }
}

impl ApiVersion {
    /// Parse version from string
    pub fn from_str(s: &str) -> Result<Self, ApiVersionError> {
        match s.to_lowercase().as_str() {
            "v1" | "1" | "1.0" => Ok(ApiVersion::V1),
            "v2" | "2" | "2.0" => Ok(ApiVersion::V2),
            _ => Err(ApiVersionError::UnsupportedVersion(s.to_string())),
        }
    }

    /// Get the current/latest version
    pub fn current() -> Self {
        ApiVersion::V2
    }

    /// Get the minimum supported version
    pub fn minimum_supported() -> Self {
        ApiVersion::V1
    }

    /// Check if this version is supported
    pub fn is_supported(&self) -> bool {
        *self >= Self::minimum_supported()
    }

    /// Check if this version is deprecated
    pub fn is_deprecated(&self) -> bool {
        *self < Self::current()
    }

    /// Get deprecation notice if applicable
    pub fn deprecation_notice(&self) -> Option<DeprecationNotice> {
        match self {
            ApiVersion::V1 => Some(DeprecationNotice {
                version: *self,
                deprecated_since: "2024-01-01".to_string(),
                sunset_date: Some("2024-12-31".to_string()),
                migration_guide_url: Some("/docs/migration/v1-to-v2".to_string()),
                replacement_version: ApiVersion::V2,
                breaking_changes: vec![
                    "Authentication now requires JWT tokens".to_string(),
                    "Error response format has changed".to_string(),
                    "Pagination parameters renamed".to_string(),
                ],
            }),
            ApiVersion::V2 => None,
        }
    }
}

/// API version extraction errors
#[derive(Debug, thiserror::Error)]
pub enum ApiVersionError {
    #[error("Unsupported API version: {0}")]
    UnsupportedVersion(String),
    #[error("Missing API version")]
    MissingVersion,
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),
}

/// Deprecation notice for API versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationNotice {
    pub version: ApiVersion,
    pub deprecated_since: String,
    pub sunset_date: Option<String>,
    pub migration_guide_url: Option<String>,
    pub replacement_version: ApiVersion,
    pub breaking_changes: Vec<String>,
}

/// API version extractor that supports multiple methods
#[derive(Debug, Clone)]
pub struct ApiVersionExtractor {
    pub version: ApiVersion,
    pub source: VersionSource,
}

/// Source of version information
#[derive(Debug, Clone, PartialEq)]
pub enum VersionSource {
    Header,
    Path,
    QueryParam,
    Default,
}

impl ApiVersionExtractor {
    /// Extract version from request using multiple strategies
    pub fn extract_from_request(req: &Request) -> Self {
        // 1. Try to extract from Accept header (preferred)
        if let Some(version) = Self::extract_from_accept_header(req.headers()) {
            return ApiVersionExtractor {
                version,
                source: VersionSource::Header,
            };
        }

        // 2. Try to extract from custom header
        if let Some(version) = Self::extract_from_custom_header(req.headers()) {
            return ApiVersionExtractor {
                version,
                source: VersionSource::Header,
            };
        }

        // 3. Try to extract from path
        if let Some(version) = Self::extract_from_path(req.uri().path()) {
            return ApiVersionExtractor {
                version,
                source: VersionSource::Path,
            };
        }

        // 4. Try to extract from query parameter
        if let Some(version) = Self::extract_from_query(req.uri().query()) {
            return ApiVersionExtractor {
                version,
                source: VersionSource::QueryParam,
            };
        }

        // 5. Default to current version
        ApiVersionExtractor {
            version: ApiVersion::current(),
            source: VersionSource::Default,
        }
    }

    /// Extract version from Accept header (e.g., "application/vnd.rustci.v1+json")
    fn extract_from_accept_header(headers: &HeaderMap) -> Option<ApiVersion> {
        headers
            .get("accept")
            .and_then(|h| h.to_str().ok())
            .and_then(|accept| {
                if accept.contains("application/vnd.rustci.v1") {
                    Some(ApiVersion::V1)
                } else if accept.contains("application/vnd.rustci.v2") {
                    Some(ApiVersion::V2)
                } else {
                    None
                }
            })
    }

    /// Extract version from custom header (e.g., "X-API-Version: v1")
    fn extract_from_custom_header(headers: &HeaderMap) -> Option<ApiVersion> {
        headers
            .get("x-api-version")
            .and_then(|h| h.to_str().ok())
            .and_then(|version| ApiVersion::from_str(version).ok())
    }

    /// Extract version from URL path (e.g., "/api/v1/pipelines")
    fn extract_from_path(path: &str) -> Option<ApiVersion> {
        if path.contains("/v1/") {
            Some(ApiVersion::V1)
        } else if path.contains("/v2/") {
            Some(ApiVersion::V2)
        } else {
            None
        }
    }

    /// Extract version from query parameter (e.g., "?version=v1")
    fn extract_from_query(query: Option<&str>) -> Option<ApiVersion> {
        query
            .and_then(|q| {
                q.split('&')
                    .find(|param| param.starts_with("version="))
                    .and_then(|param| param.split('=').nth(1))
            })
            .and_then(|version| ApiVersion::from_str(version).ok())
    }
}

/// Middleware for API versioning
pub async fn api_versioning_middleware(mut req: Request, next: Next) -> Response {
    let version_info = ApiVersionExtractor::extract_from_request(&req);
    
    debug!(
        "API version extracted: {} from {:?}",
        version_info.version, version_info.source
    );

    // Check if version is supported
    if !version_info.version.is_supported() {
        return create_unsupported_version_response(&version_info.version);
    }

    // Add version info to request extensions
    req.extensions_mut().insert(version_info.clone());

    // Call the next middleware/handler
    let mut response = next.run(req).await;

    // Add version headers to response
    let headers = response.headers_mut();
    
    // Add current version header
    headers.insert(
        "X-API-Version",
        HeaderValue::from_str(&version_info.version.to_string()).unwrap(),
    );

    // Add supported versions header
    headers.insert(
        "X-Supported-Versions",
        HeaderValue::from_static("v1,v2"),
    );

    // Add deprecation warning if applicable
    if let Some(deprecation) = version_info.version.deprecation_notice() {
        headers.insert(
            "X-API-Deprecated",
            HeaderValue::from_static("true"),
        );
        
        if let Some(sunset_date) = &deprecation.sunset_date {
            headers.insert(
                "X-API-Sunset",
                HeaderValue::from_str(sunset_date).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
            );
        }

        if let Some(migration_url) = &deprecation.migration_guide_url {
            headers.insert(
                "X-API-Migration-Guide",
                HeaderValue::from_str(migration_url).unwrap_or_else(|_| HeaderValue::from_static("unavailable")),
            );
        }

        warn!(
            "Deprecated API version {} used. Sunset date: {:?}",
            version_info.version,
            deprecation.sunset_date
        );
    }

    response
}

/// Create response for unsupported API version
fn create_unsupported_version_response(version: &ApiVersion) -> Response {
    let error_response = serde_json::json!({
        "error": {
            "code": "UNSUPPORTED_API_VERSION",
            "message": format!("API version {} is not supported", version),
            "supported_versions": ["v1", "v2"],
            "current_version": ApiVersion::current().to_string(),
            "migration_guide": "/docs/api-versioning"
        }
    });

    (StatusCode::BAD_REQUEST, Json(error_response)).into_response()
}

/// Version-specific request/response transformation
pub trait VersionTransformer {
    /// Transform request for backward compatibility
    fn transform_request(&self, version: ApiVersion, request: serde_json::Value) -> serde_json::Value;
    
    /// Transform response for backward compatibility
    fn transform_response(&self, version: ApiVersion, response: serde_json::Value) -> serde_json::Value;
}

/// Default transformer that handles common transformations
#[derive(Default)]
pub struct DefaultVersionTransformer {
    transformations: HashMap<ApiVersion, Box<dyn Fn(serde_json::Value) -> serde_json::Value + Send + Sync>>,
}

impl DefaultVersionTransformer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a transformation for a specific version
    pub fn add_transformation<F>(mut self, version: ApiVersion, transform: F) -> Self
    where
        F: Fn(serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
    {
        self.transformations.insert(version, Box::new(transform));
        self
    }
}

impl VersionTransformer for DefaultVersionTransformer {
    fn transform_request(&self, version: ApiVersion, request: serde_json::Value) -> serde_json::Value {
        if let Some(transformer) = self.transformations.get(&version) {
            transformer(request)
        } else {
            request
        }
    }

    fn transform_response(&self, version: ApiVersion, response: serde_json::Value) -> serde_json::Value {
        match version {
            ApiVersion::V1 => {
                // Transform V2 response format to V1 format
                self.transform_to_v1_format(response)
            }
            ApiVersion::V2 => response, // Current format
        }
    }
}

impl DefaultVersionTransformer {
    /// Transform response to V1 format for backward compatibility
    fn transform_to_v1_format(&self, mut response: serde_json::Value) -> serde_json::Value {
        // Example transformations for V1 compatibility
        if let Some(obj) = response.as_object_mut() {
            // V1 used 'id' instead of 'pipeline_id'
            if let Some(pipeline_id) = obj.remove("pipeline_id") {
                obj.insert("id".to_string(), pipeline_id);
            }

            // V1 used different pagination format
            if let Some(pagination) = obj.get_mut("pagination") {
                if let Some(page_obj) = pagination.as_object_mut() {
                    if let Some(page_number) = page_obj.remove("page") {
                        page_obj.insert("page_number".to_string(), page_number);
                    }
                    if let Some(page_size) = page_obj.remove("limit") {
                        page_obj.insert("per_page".to_string(), page_size);
                    }
                }
            }

            // V1 had different error format
            if let Some(error) = obj.get_mut("error") {
                if let Some(error_obj) = error.as_object_mut() {
                    if let Some(message) = error_obj.get("message") {
                        error_obj.insert("error_message".to_string(), message.clone());
                    }
                }
            }
        }

        response
    }
}

/// API version information endpoint response
#[derive(Debug, Serialize)]
pub struct ApiVersionInfo {
    pub current_version: ApiVersion,
    pub supported_versions: Vec<ApiVersion>,
    pub deprecated_versions: Vec<DeprecationNotice>,
    pub migration_guides: HashMap<String, String>,
}

impl ApiVersionInfo {
    pub fn new() -> Self {
        let supported_versions = vec![ApiVersion::V1, ApiVersion::V2];
        let deprecated_versions = supported_versions
            .iter()
            .filter_map(|v| v.deprecation_notice())
            .collect();

        let mut migration_guides = HashMap::new();
        migration_guides.insert(
            "v1-to-v2".to_string(),
            "/docs/migration/v1-to-v2".to_string(),
        );

        Self {
            current_version: ApiVersion::current(),
            supported_versions,
            deprecated_versions,
            migration_guides,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_version_parsing() {
        assert_eq!(ApiVersion::from_str("v1").unwrap(), ApiVersion::V1);
        assert_eq!(ApiVersion::from_str("V2").unwrap(), ApiVersion::V2);
        assert_eq!(ApiVersion::from_str("1").unwrap(), ApiVersion::V1);
        assert!(ApiVersion::from_str("v3").is_err());
    }

    #[test]
    fn test_version_support() {
        assert!(ApiVersion::V1.is_supported());
        assert!(ApiVersion::V2.is_supported());
    }

    #[test]
    fn test_deprecation_notice() {
        assert!(ApiVersion::V1.deprecation_notice().is_some());
        assert!(ApiVersion::V2.deprecation_notice().is_none());
    }

    #[test]
    fn test_header_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-version", HeaderValue::from_static("v1"));
        
        let version = ApiVersionExtractor::extract_from_custom_header(&headers);
        assert_eq!(version, Some(ApiVersion::V1));
    }

    #[test]
    fn test_accept_header_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("accept", HeaderValue::from_static("application/vnd.rustci.v1+json"));
        
        let version = ApiVersionExtractor::extract_from_accept_header(&headers);
        assert_eq!(version, Some(ApiVersion::V1));
    }

    #[test]
    fn test_path_extraction() {
        let version = ApiVersionExtractor::extract_from_path("/api/v1/pipelines");
        assert_eq!(version, Some(ApiVersion::V1));
        
        let version = ApiVersionExtractor::extract_from_path("/api/v2/jobs");
        assert_eq!(version, Some(ApiVersion::V2));
    }

    #[test]
    fn test_query_extraction() {
        let version = ApiVersionExtractor::extract_from_query(Some("version=v1&other=param"));
        assert_eq!(version, Some(ApiVersion::V1));
    }
}