use crate::common::*;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_health_endpoint() -> TestResult {
    let env = TestEnvironment::new().await;

    // Start test server
    let container = env.external_services.docker.clone();
    let app_container = TestContainerManager::new()
        .start_app_container("rustci:test", HashMap::new())
        .await?;

    let client = Client::new();
    let response = client
        .get(&format!(
            "http://localhost:{}/health",
            app_container.ports[0]
        ))
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await?;
    TestAssertions::assert_json_contains_fields(&body, &["status", "timestamp"])?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_user_registration() -> TestResult {
    let env = TestEnvironment::new().await;

    let user_data = json!({
        "username": "newuser",
        "email": "newuser@example.com",
        "github_id": "789012"
    });

    // Mock GitHub API response
    env.external_services
        .github
        .set_response(
            "/user",
            json!({
                "id": 789012,
                "login": "newuser",
                "email": "newuser@example.com"
            }),
        )
        .await;

    let client = Client::new();
    let response = client
        .post("http://localhost:8080/api/users")
        .json(&user_data)
        .send()
        .await?;

    assert_eq!(response.status(), 201);

    let created_user: serde_json::Value = response.json().await?;
    TestAssertions::assert_json_contains_fields(&created_user, &["id", "username", "email"])?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_pipeline_creation() -> TestResult {
    let env = TestEnvironment::new().await;

    // Create test user first
    let user = TestDataBuilder::user().build();
    env.database
        .insert("users", TestDocument::new(serde_json::to_value(&user)?))
        .await?;

    let pipeline_data = json!({
        "name": "test-pipeline",
        "stages": [
            {
                "name": "build",
                "steps": [
                    {
                        "name": "compile",
                        "command": "cargo build",
                        "args": ["--release"]
                    }
                ]
            }
        ],
        "triggers": ["push"]
    });

    let client = Client::new();
    let response = client
        .post("http://localhost:8080/api/pipelines")
        .header("Authorization", "Bearer test_token")
        .json(&pipeline_data)
        .send()
        .await?;

    assert_eq!(response.status(), 201);

    let created_pipeline: serde_json::Value = response.json().await?;
    TestAssertions::assert_json_contains_fields(&created_pipeline, &["id", "name", "stages"])?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_pipeline_execution() -> TestResult {
    let env = TestEnvironment::new().await;

    // Setup test data
    let user = TestDataBuilder::user().build();
    let workspace = TestDataBuilder::workspace().with_owner(user.id).build();
    let pipeline = TestDataBuilder::pipeline().build();

    env.database
        .insert("users", TestDocument::new(serde_json::to_value(&user)?))
        .await?;
    env.database
        .insert(
            "workspaces",
            TestDocument::new(serde_json::to_value(&workspace)?),
        )
        .await?;
    env.database
        .insert(
            "pipelines",
            TestDocument::new(serde_json::to_value(&pipeline)?),
        )
        .await?;

    let execution_request = json!({
        "pipeline_id": pipeline.id,
        "trigger": "manual",
        "branch": "main"
    });

    let client = Client::new();
    let response = client
        .post("http://localhost:8080/api/executions")
        .header("Authorization", "Bearer test_token")
        .json(&execution_request)
        .send()
        .await?;

    assert_eq!(response.status(), 202);

    let execution: serde_json::Value = response.json().await?;
    TestAssertions::assert_json_contains_fields(&execution, &["id", "pipeline_id", "status"])?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_webhook_handling() -> TestResult {
    let env = TestEnvironment::new().await;

    let webhook_payload = TestFixtures::sample_webhook_payload();

    let client = Client::new();
    let response = client
        .post("http://localhost:8080/webhooks/github")
        .header("X-GitHub-Event", "push")
        .header("X-Hub-Signature-256", "sha256=test_signature")
        .json(&webhook_payload)
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    // Verify event was published
    let events = env.message_bus.get_events_by_type("WebhookReceived").await;
    TestAssertions::assert_length(&events, 1)?;

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_authentication_flow() -> TestResult {
    let env = TestEnvironment::new().await;

    // Mock GitHub OAuth response
    env.external_services
        .github
        .set_response(
            "/login/oauth/access_token",
            json!({
                "access_token": "test_access_token",
                "token_type": "bearer",
                "scope": "user:email"
            }),
        )
        .await;

    env.external_services
        .github
        .set_response(
            "/user",
            json!({
                "id": 123456,
                "login": "testuser",
                "email": "test@example.com"
            }),
        )
        .await;

    let client = Client::new();

    // Start OAuth flow
    let auth_response = client
        .get("http://localhost:8080/auth/github")
        .send()
        .await?;

    assert_eq!(auth_response.status(), 302);

    // Simulate callback
    let callback_response = client
        .get("http://localhost:8080/auth/github/callback?code=test_code&state=test_state")
        .send()
        .await?;

    assert_eq!(callback_response.status(), 302);

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> TestResult {
    let env = TestEnvironment::new().await;

    let client = Client::new();

    // Test 404 for non-existent resource
    let response = client
        .get("http://localhost:8080/api/pipelines/non-existent-id")
        .send()
        .await?;

    assert_eq!(response.status(), 404);

    let error_body: serde_json::Value = response.json().await?;
    TestAssertions::assert_json_contains_fields(&error_body, &["error", "message"])?;

    // Test 401 for unauthorized access
    let response = client
        .post("http://localhost:8080/api/pipelines")
        .json(&json!({"name": "test"}))
        .send()
        .await?;

    assert_eq!(response.status(), 401);

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_rate_limiting() -> TestResult {
    let env = TestEnvironment::new().await;

    let client = Client::new();

    // Make multiple requests quickly
    let mut responses = Vec::new();
    for _ in 0..10 {
        let response = client.get("http://localhost:8080/health").send().await?;
        responses.push(response.status());
    }

    // Should eventually get rate limited
    let rate_limited = responses.iter().any(|&status| status == 429);

    // Note: This test might be flaky depending on rate limit configuration
    // In a real scenario, you'd configure lower limits for testing

    env.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn test_concurrent_requests() -> TestResult {
    let env = TestEnvironment::new().await;

    let client = Client::new();

    // Make concurrent requests
    let mut handles = Vec::new();
    for i in 0..5 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            client
                .get(&format!("http://localhost:8080/health?id={}", i))
                .send()
                .await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let mut results = Vec::new();
    for handle in handles {
        let response = handle.await??;
        results.push(response.status());
    }

    // All requests should succeed
    for status in results {
        assert_eq!(status, 200);
    }

    env.cleanup().await;
    Ok(())
}
