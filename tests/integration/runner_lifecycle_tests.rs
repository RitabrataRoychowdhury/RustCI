//! Integration tests for runner lifecycle management
//!
//! These tests verify the complete runner lifecycle flow including:
//! - Runner registration with authentication
//! - Job triggering with proper authorization
//! - Log fetching with permission validation
//! - Artifact retrieval with access control
//! - Runner deregistration with cleanup

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tower::ServiceExt;
use uuid::Uuid;

use RustAutoDevOps::{
    config::AppConfiguration,
    core::security::{InMemoryAuditLogger, JwtClaims, JwtManager, Role},
    infrastructure::database::DatabaseManager,
    presentation::routes::api::create_api_router,
    AppState,
};

/// Helper function to create a test app state
async fn create_test_app_state() -> AppState {
    let config = AppConfiguration::default();
    let db = DatabaseManager::new("mongodb://localhost:27017", "test_rustci")
        .await
        .expect("Failed to create database manager");
    let audit_logger = std::sync::Arc::new(InMemoryAuditLogger::new());
    let config_manager = std::sync::Arc::new(tokio::sync::RwLock::new(
        RustAutoDevOps::config::HotReloadConfigManager::new(config.clone()),
    ));
    let observability = std::sync::Arc::new(
        RustAutoDevOps::core::observability::ObservabilityService::new(config.clone()),
    );

    AppState {
        env: std::sync::Arc::new(config),
        db: std::sync::Arc::new(db),
        audit_logger: Some(audit_logger),
        config_manager,
        observability,
    }
}

/// Helper function to create a JWT token for testing
fn create_test_jwt_token() -> String {
    let jwt_manager = JwtManager::new("test-secret".to_string(), 3600);
    let claims = JwtClaims::new(
        Uuid::new_v4(),
        "test@example.com".to_string(),
        vec![Role::Admin],
        3600,
    );
    jwt_manager.create_token(&claims).unwrap()
}

/// Helper function to create authenticated request
fn create_authenticated_request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    let token = create_test_jwt_token();
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json");

    if let Some(body_value) = body {
        request.body(Body::from(body_value.to_string())).unwrap()
    } else {
        request.body(Body::empty()).unwrap()
    }
}

#[tokio::test]
async fn test_complete_runner_lifecycle() {
    // Create test app
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    // 1. Register a new runner
    let register_request = create_authenticated_request(
        Method::POST,
        "/api/runners",
        Some(json!({
            "name": "test-runner",
            "runner_type": "local",
            "capacity": 4,
            "tags": ["rust", "linux"],
            "metadata": {
                "region": "us-west-1"
            }
        })),
    );

    let response = app.clone().oneshot(register_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let runner_response: Value = serde_json::from_slice(&body).unwrap();
    let runner_id = runner_response["id"].as_str().unwrap();

    // 2. List runners to verify registration
    let list_request = create_authenticated_request(Method::GET, "/api/runners", None);
    let response = app.clone().oneshot(list_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 3. Get runner status
    let status_request = create_authenticated_request(
        Method::GET,
        &format!("/api/runners/{}/status", runner_id),
        None,
    );
    let response = app.clone().oneshot(status_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 4. Trigger a job on the runner
    let job_request = create_authenticated_request(
        Method::POST,
        &format!("/api/runners/{}/jobs", runner_id),
        Some(json!({
            "name": "test-job",
            "steps": [
                {
                    "name": "build",
                    "command": "cargo",
                    "args": ["build", "--release"],
                    "timeout": 300
                },
                {
                    "name": "test",
                    "command": "cargo",
                    "args": ["test"],
                    "timeout": 600
                }
            ],
            "priority": "high",
            "timeout": 3600,
            "metadata": {
                "branch": "main",
                "commit": "abc123"
            }
        })),
    );

    let response = app.clone().oneshot(job_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let job_response: Value = serde_json::from_slice(&body).unwrap();
    let job_id = job_response["id"].as_str().unwrap();

    // 5. Fetch job logs
    let logs_request = create_authenticated_request(
        Method::GET,
        &format!("/api/runners/{}/jobs/{}/logs", runner_id, job_id),
        None,
    );
    let response = app.clone().oneshot(logs_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 6. Fetch job logs with query parameters
    let logs_with_params_request = create_authenticated_request(
        Method::GET,
        &format!("/api/runners/{}/jobs/{}/logs?follow=true&tail=50", runner_id, job_id),
        None,
    );
    let response = app.clone().oneshot(logs_with_params_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 7. Try to fetch job artifacts
    let artifacts_request = create_authenticated_request(
        Method::GET,
        &format!("/api/runners/{}/jobs/{}/artifacts", runner_id, job_id),
        None,
    );
    let response = app.clone().oneshot(artifacts_request).await.unwrap();
    // Expecting 404 since we don't have actual artifacts in the mock implementation
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // 8. Deregister the runner
    let deregister_request = create_authenticated_request(
        Method::DELETE,
        &format!("/api/runners/{}", runner_id),
        None,
    );
    let response = app.clone().oneshot(deregister_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_authentication_required() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    // Try to register runner without authentication
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/runners")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "test-runner",
                "runner_type": "local",
                "capacity": 4,
                "tags": ["rust"]
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_runner_id_format() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    // Try to get status with invalid runner ID
    let request = create_authenticated_request(
        Method::GET,
        "/api/runners/invalid-uuid/status",
        None,
    );
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_invalid_job_trigger_request() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    let runner_id = Uuid::new_v4();

    // Try to trigger job with invalid request body
    let request = create_authenticated_request(
        Method::POST,
        &format!("/api/runners/{}/jobs", runner_id),
        Some(json!({
            "name": "test-job",
            // Missing required "steps" field
            "priority": "high"
        })),
    );

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_runner_registration_validation() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    // Test with unsupported runner type
    let request = create_authenticated_request(
        Method::POST,
        "/api/runners",
        Some(json!({
            "name": "test-runner",
            "runner_type": "unsupported",
            "capacity": 4,
            "tags": ["rust"]
        })),
    );

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_job_priority_parsing() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    let runner_id = Uuid::new_v4();

    // Test different priority levels
    let priorities = vec!["low", "normal", "high", "critical"];

    for priority in priorities {
        let request = create_authenticated_request(
            Method::POST,
            &format!("/api/runners/{}/jobs", runner_id),
            Some(json!({
                "name": format!("test-job-{}", priority),
                "steps": [
                    {
                        "name": "echo",
                        "command": "echo",
                        "args": ["hello"],
                        "timeout": 30
                    }
                ],
                "priority": priority,
                "timeout": 300
            })),
        );

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
}

#[tokio::test]
async fn test_log_query_parameters() {
    let app_state = create_test_app_state().await;
    let app = create_api_router(app_state);

    let runner_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();

    // Test different log query parameters
    let test_cases = vec![
        format!("/api/runners/{}/jobs/{}/logs", runner_id, job_id),
        format!("/api/runners/{}/jobs/{}/logs?follow=true", runner_id, job_id),
        format!("/api/runners/{}/jobs/{}/logs?tail=100", runner_id, job_id),
        format!(
            "/api/runners/{}/jobs/{}/logs?follow=false&tail=50",
            runner_id, job_id
        ),
    ];

    for uri in test_cases {
        let request = create_authenticated_request(Method::GET, &uri, None);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    use RustAutoDevOps::core::security::{Permission, Role, SecurityContext};
    use std::collections::HashSet;

    fn create_limited_security_context() -> SecurityContext {
        SecurityContext {
            user_id: Uuid::new_v4(),
            roles: vec![Role::Viewer],
            permissions: Role::Viewer.permissions(),
            session_id: "test-session".to_string(),
            ip_address: None,
            user_agent: None,
        }
    }

    #[test]
    fn test_permission_validation() {
        let admin_ctx = SecurityContext {
            user_id: Uuid::new_v4(),
            roles: vec![Role::Admin],
            permissions: Role::Admin.permissions(),
            session_id: "admin-session".to_string(),
            ip_address: None,
            user_agent: None,
        };

        let viewer_ctx = create_limited_security_context();

        // Admin should have all permissions
        assert!(admin_ctx.has_permission(&Permission::ManageSystem));
        assert!(admin_ctx.has_permission(&Permission::ExecutePipelines));
        assert!(admin_ctx.has_permission(&Permission::ReadPipelines));

        // Viewer should have limited permissions
        assert!(!viewer_ctx.has_permission(&Permission::ManageSystem));
        assert!(!viewer_ctx.has_permission(&Permission::ExecutePipelines));
        assert!(viewer_ctx.has_permission(&Permission::ReadPipelines));
    }

    #[test]
    fn test_role_permissions() {
        let admin_perms = Role::Admin.permissions();
        let developer_perms = Role::Developer.permissions();
        let viewer_perms = Role::Viewer.permissions();

        // Admin should have all permissions
        assert!(admin_perms.contains(&Permission::ManageSystem));
        assert!(admin_perms.contains(&Permission::ManageRunners));
        assert!(admin_perms.contains(&Permission::ExecutePipelines));

        // Developer should have execution permissions but not system management
        assert!(!developer_perms.contains(&Permission::ManageSystem));
        assert!(!developer_perms.contains(&Permission::ManageRunners));
        assert!(developer_perms.contains(&Permission::ExecutePipelines));
        assert!(developer_perms.contains(&Permission::TriggerJobs));

        // Viewer should only have read permissions
        assert!(!viewer_perms.contains(&Permission::ManageSystem));
        assert!(!viewer_perms.contains(&Permission::ExecutePipelines));
        assert!(viewer_perms.contains(&Permission::ReadPipelines));
        assert!(viewer_perms.contains(&Permission::ViewLogs));
    }
}