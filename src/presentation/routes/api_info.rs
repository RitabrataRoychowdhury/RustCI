//! API information and versioning endpoints
//!
//! This module provides endpoints for API version information, migration guides,
//! and compatibility documentation.

use crate::{
    api::{
        routing::MigrationGuideGenerator,
        versioning::{ApiVersion, ApiVersionInfo},
    },
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Create API information router
pub fn api_info_router() -> Router<AppState> {
    Router::new()
        .route("/version", get(get_api_version_info))
        .route("/versions", get(get_supported_versions))
        .route("/migration/:from/:to", get(get_migration_guide))
        .route("/compatibility", get(get_compatibility_info))
        .route("/deprecation", get(get_deprecation_notices))
}

/// Query parameters for version information
#[derive(Debug, Deserialize)]
struct VersionQuery {
    include_deprecated: Option<bool>,
    format: Option<String>,
}

/// Get current API version information
async fn get_api_version_info(
    Query(query): Query<VersionQuery>,
) -> Result<Json<ApiVersionInfo>, (StatusCode, Json<Value>)> {
    let version_info = ApiVersionInfo::new();
    Ok(Json(version_info))
}

/// Get all supported API versions
async fn get_supported_versions(
    Query(query): Query<VersionQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let include_deprecated = query.include_deprecated.unwrap_or(true);
    
    let mut versions = vec![
        json!({
            "version": "v2",
            "status": "current",
            "release_date": "2024-01-01",
            "features": [
                "Enhanced error handling",
                "Improved pagination",
                "JWT authentication",
                "Structured responses"
            ]
        }),
    ];

    if include_deprecated {
        versions.push(json!({
            "version": "v1",
            "status": "deprecated",
            "release_date": "2023-01-01",
            "deprecation_date": "2024-01-01",
            "sunset_date": "2024-12-31",
            "features": [
                "Basic authentication",
                "Simple error responses",
                "Legacy pagination"
            ]
        }));
    }

    let response = json!({
        "supported_versions": versions,
        "current_version": "v2",
        "minimum_supported_version": "v1",
        "version_selection": {
            "header": "X-API-Version: v1|v2",
            "accept_header": "Accept: application/vnd.rustci.v1+json",
            "path": "/api/v1/* or /api/v2/*",
            "query_param": "?version=v1"
        }
    });

    Ok(Json(response))
}

/// Get migration guide between versions
async fn get_migration_guide(
    Path((from, to)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let from_version = ApiVersion::from_str(&from).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "INVALID_VERSION",
                    "message": format!("Invalid source version: {}", from)
                }
            })),
        )
    })?;

    let to_version = ApiVersion::from_str(&to).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "INVALID_VERSION",
                    "message": format!("Invalid target version: {}", to)
                }
            })),
        )
    })?;

    let migration_guide = MigrationGuideGenerator::generate_guide(from_version, to_version);
    Ok(Json(serde_json::to_value(migration_guide).unwrap()))
}

/// Get API compatibility information
async fn get_compatibility_info() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let compatibility_info = json!({
        "backward_compatibility": {
            "v1_to_v2": {
                "supported": true,
                "automatic_transformation": true,
                "breaking_changes": [
                    {
                        "category": "Authentication",
                        "impact": "medium",
                        "description": "JWT tokens required for new endpoints"
                    },
                    {
                        "category": "Error Responses",
                        "impact": "low",
                        "description": "Structured error format with error codes"
                    },
                    {
                        "category": "Pagination",
                        "impact": "low",
                        "description": "Parameter names changed from page_number/per_page to page/limit"
                    }
                ]
            }
        },
        "forward_compatibility": {
            "v2_to_v1": {
                "supported": false,
                "reason": "V1 lacks features present in V2"
            }
        },
        "content_negotiation": {
            "supported_formats": ["json"],
            "version_headers": [
                "X-API-Version",
                "Accept: application/vnd.rustci.v{version}+json"
            ]
        },
        "deprecation_policy": {
            "notice_period": "12 months",
            "sunset_period": "6 months after replacement",
            "support_period": "18 months total"
        }
    });

    Ok(Json(compatibility_info))
}

/// Get deprecation notices for all versions
async fn get_deprecation_notices() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let notices = vec![
        json!({
            "version": "v1",
            "status": "deprecated",
            "deprecated_since": "2024-01-01",
            "sunset_date": "2024-12-31",
            "replacement_version": "v2",
            "migration_guide_url": "/api/info/migration/v1/v2",
            "breaking_changes_count": 3,
            "impact_level": "medium",
            "recommended_action": "Migrate to v2 before sunset date",
            "support_contact": "api-support@rustci.dev"
        })
    ];

    let response = json!({
        "deprecation_notices": notices,
        "active_deprecations": 1,
        "upcoming_deprecations": 0,
        "deprecation_timeline": {
            "current_date": chrono::Utc::now().format("%Y-%m-%d").to_string(),
            "next_deprecation": null,
            "next_sunset": "2024-12-31"
        }
    });

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_api_version_info_endpoint() {
        let app = api_info_router();

        let request = Request::builder()
            .uri("/version")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_supported_versions_endpoint() {
        let app = api_info_router();

        let request = Request::builder()
            .uri("/versions")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_migration_guide_endpoint() {
        let app = api_info_router();

        let request = Request::builder()
            .uri("/migration/v1/v2")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_migration_versions() {
        let app = api_info_router();

        let request = Request::builder()
            .uri("/migration/v1/v99")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}