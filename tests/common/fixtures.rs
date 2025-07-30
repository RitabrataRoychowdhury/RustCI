use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

/// Test data fixtures for consistent test data
pub struct TestFixtures;

impl TestFixtures {
    /// Create a sample user fixture
    pub fn sample_user() -> Value {
        json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "username": "testuser",
            "email": "test@example.com",
            "github_id": "123456",
            "roles": ["user"],
            "created_at": "2024-01-01T00:00:00Z",
            "last_login": null
        })
    }

    /// Create an admin user fixture
    pub fn admin_user() -> Value {
        json!({
            "id": "550e8400-e29b-41d4-a716-446655440001",
            "username": "admin",
            "email": "admin@example.com",
            "github_id": "654321",
            "roles": ["user", "admin"],
            "created_at": "2024-01-01T00:00:00Z",
            "last_login": "2024-01-02T10:30:00Z"
        })
    }

    /// Create a sample workspace fixture
    pub fn sample_workspace() -> Value {
        json!({
            "id": "660e8400-e29b-41d4-a716-446655440000",
            "name": "test-workspace",
            "owner_id": "550e8400-e29b-41d4-a716-446655440000",
            "repository_url": "https://github.com/test/repo",
            "branch": "main",
            "status": "active",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        })
    }

    /// Create a sample pipeline fixture
    pub fn sample_pipeline() -> Value {
        json!({
            "id": "770e8400-e29b-41d4-a716-446655440000",
            "name": "test-pipeline",
            "workspace_id": "660e8400-e29b-41d4-a716-446655440000",
            "stages": [
                {
                    "name": "build",
                    "steps": [
                        {
                            "name": "compile",
                            "command": "cargo build",
                            "args": ["--release"],
                            "environment": {
                                "RUST_LOG": "info"
                            }
                        }
                    ],
                    "depends_on": []
                },
                {
                    "name": "test",
                    "steps": [
                        {
                            "name": "unit-tests",
                            "command": "cargo test",
                            "args": ["--lib"],
                            "environment": {}
                        }
                    ],
                    "depends_on": ["build"]
                }
            ],
            "variables": {
                "CARGO_TARGET_DIR": "target",
                "RUST_BACKTRACE": "1"
            },
            "triggers": ["push", "pull_request"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        })
    }

    /// Create a sample execution fixture
    pub fn sample_execution() -> Value {
        json!({
            "id": "880e8400-e29b-41d4-a716-446655440000",
            "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
            "workspace_id": "660e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "running",
            "started_at": "2024-01-01T10:00:00Z",
            "completed_at": null,
            "trigger": "push",
            "commit_sha": "abc123def456",
            "branch": "main",
            "variables": {
                "CARGO_TARGET_DIR": "target",
                "RUST_BACKTRACE": "1"
            },
            "stages": [
                {
                    "name": "build",
                    "status": "completed",
                    "started_at": "2024-01-01T10:00:00Z",
                    "completed_at": "2024-01-01T10:02:00Z",
                    "steps": [
                        {
                            "name": "compile",
                            "status": "completed",
                            "started_at": "2024-01-01T10:00:00Z",
                            "completed_at": "2024-01-01T10:02:00Z",
                            "exit_code": 0,
                            "logs": "Compiling rustci v0.1.0\nFinished release [optimized] target(s)"
                        }
                    ]
                },
                {
                    "name": "test",
                    "status": "running",
                    "started_at": "2024-01-01T10:02:00Z",
                    "completed_at": null,
                    "steps": [
                        {
                            "name": "unit-tests",
                            "status": "running",
                            "started_at": "2024-01-01T10:02:00Z",
                            "completed_at": null,
                            "exit_code": null,
                            "logs": "Running tests..."
                        }
                    ]
                }
            ]
        })
    }

    /// Create a sample command fixture
    pub fn sample_command() -> Value {
        json!({
            "id": "990e8400-e29b-41d4-a716-446655440000",
            "command_type": "StartPipeline",
            "payload": {
                "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
                "trigger": "push",
                "commit_sha": "abc123def456",
                "branch": "main"
            },
            "correlation_id": "aa0e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "created_at": "2024-01-01T10:00:00Z",
            "status": "pending"
        })
    }

    /// Create a sample event fixture
    pub fn sample_event() -> Value {
        json!({
            "id": "bb0e8400-e29b-41d4-a716-446655440000",
            "event_type": "PipelineStarted",
            "aggregate_id": "770e8400-e29b-41d4-a716-446655440000",
            "data": {
                "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
                "execution_id": "880e8400-e29b-41d4-a716-446655440000",
                "triggered_by": "push",
                "commit_sha": "abc123def456"
            },
            "correlation_id": "aa0e8400-e29b-41d4-a716-446655440000",
            "occurred_at": "2024-01-01T10:00:00Z",
            "version": 1
        })
    }

    /// Create a sample configuration fixture
    pub fn sample_config() -> Value {
        json!({
            "server": {
                "host": "127.0.0.1",
                "port": 8080,
                "workers": 4
            },
            "database": {
                "url": "mongodb://localhost:27017",
                "database": "rustci_test",
                "max_connections": 10,
                "timeout": 30
            },
            "jwt": {
                "secret": "test_secret_key",
                "expiration": 3600
            },
            "github": {
                "client_id": "test_client_id",
                "client_secret": "test_client_secret",
                "webhook_secret": "test_webhook_secret"
            },
            "observability": {
                "logging": {
                    "level": "info",
                    "format": "json"
                },
                "metrics": {
                    "enabled": true,
                    "port": 9090
                },
                "tracing": {
                    "enabled": true,
                    "endpoint": "http://localhost:14268/api/traces"
                }
            }
        })
    }

    /// Create a sample Docker configuration
    pub fn sample_docker_config() -> Value {
        json!({
            "image": "rust:1.70",
            "working_dir": "/workspace",
            "environment": {
                "CARGO_HOME": "/usr/local/cargo",
                "RUSTUP_HOME": "/usr/local/rustup"
            },
            "volumes": [
                "/workspace:/workspace",
                "cargo-cache:/usr/local/cargo"
            ],
            "network": "rustci-network",
            "resources": {
                "memory": "2g",
                "cpu": "1.0"
            }
        })
    }

    /// Create a sample Kubernetes configuration
    pub fn sample_k8s_config() -> Value {
        json!({
            "namespace": "rustci-test",
            "job": {
                "name": "pipeline-job",
                "image": "rust:1.70",
                "command": ["cargo", "build"],
                "resources": {
                    "requests": {
                        "memory": "1Gi",
                        "cpu": "500m"
                    },
                    "limits": {
                        "memory": "2Gi",
                        "cpu": "1000m"
                    }
                },
                "env": [
                    {
                        "name": "RUST_LOG",
                        "value": "info"
                    }
                ]
            }
        })
    }

    /// Create a sample webhook payload
    pub fn sample_webhook_payload() -> Value {
        json!({
            "ref": "refs/heads/main",
            "before": "0000000000000000000000000000000000000000",
            "after": "abc123def456789012345678901234567890abcd",
            "repository": {
                "id": 123456789,
                "name": "test-repo",
                "full_name": "testuser/test-repo",
                "owner": {
                    "name": "testuser",
                    "email": "test@example.com"
                },
                "private": false,
                "html_url": "https://github.com/testuser/test-repo",
                "clone_url": "https://github.com/testuser/test-repo.git",
                "ssh_url": "git@github.com:testuser/test-repo.git",
                "default_branch": "main"
            },
            "pusher": {
                "name": "testuser",
                "email": "test@example.com"
            },
            "sender": {
                "login": "testuser",
                "id": 123456,
                "avatar_url": "https://github.com/images/error/testuser_happy.gif",
                "type": "User"
            },
            "commits": [
                {
                    "id": "abc123def456789012345678901234567890abcd",
                    "message": "Add new feature",
                    "timestamp": "2024-01-01T10:00:00Z",
                    "url": "https://github.com/testuser/test-repo/commit/abc123def456789012345678901234567890abcd",
                    "author": {
                        "name": "Test User",
                        "email": "test@example.com",
                        "username": "testuser"
                    },
                    "committer": {
                        "name": "Test User",
                        "email": "test@example.com",
                        "username": "testuser"
                    },
                    "added": ["src/new_feature.rs"],
                    "removed": [],
                    "modified": ["Cargo.toml"]
                }
            ]
        })
    }

    /// Create sample error responses
    pub fn sample_error_response(error_type: &str, message: &str) -> Value {
        json!({
            "error": {
                "type": error_type,
                "message": message,
                "timestamp": "2024-01-01T10:00:00Z",
                "correlation_id": "cc0e8400-e29b-41d4-a716-446655440000"
            }
        })
    }

    /// Create sample metrics data
    pub fn sample_metrics() -> Value {
        json!({
            "timestamp": "2024-01-01T10:00:00Z",
            "metrics": {
                "pipeline_executions_total": 150,
                "pipeline_executions_success": 142,
                "pipeline_executions_failed": 8,
                "average_execution_time_seconds": 125.5,
                "active_executions": 3,
                "queue_size": 2,
                "system": {
                    "cpu_usage_percent": 45.2,
                    "memory_usage_percent": 62.8,
                    "disk_usage_percent": 23.1
                },
                "database": {
                    "connections_active": 8,
                    "connections_idle": 2,
                    "query_duration_avg_ms": 15.3
                }
            }
        })
    }

    /// Create sample log entries
    pub fn sample_log_entries() -> Vec<Value> {
        vec![
            json!({
                "timestamp": "2024-01-01T10:00:00Z",
                "level": "INFO",
                "message": "Pipeline execution started",
                "correlation_id": "aa0e8400-e29b-41d4-a716-446655440000",
                "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
                "execution_id": "880e8400-e29b-41d4-a716-446655440000"
            }),
            json!({
                "timestamp": "2024-01-01T10:01:30Z",
                "level": "DEBUG",
                "message": "Stage 'build' completed successfully",
                "correlation_id": "aa0e8400-e29b-41d4-a716-446655440000",
                "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
                "execution_id": "880e8400-e29b-41d4-a716-446655440000",
                "stage": "build",
                "duration_ms": 90000
            }),
            json!({
                "timestamp": "2024-01-01T10:02:00Z",
                "level": "ERROR",
                "message": "Test step failed with exit code 1",
                "correlation_id": "aa0e8400-e29b-41d4-a716-446655440000",
                "pipeline_id": "770e8400-e29b-41d4-a716-446655440000",
                "execution_id": "880e8400-e29b-41d4-a716-446655440000",
                "stage": "test",
                "step": "unit-tests",
                "exit_code": 1,
                "error": "Test assertion failed"
            }),
        ]
    }

    /// Create a collection of related test data
    pub fn complete_test_scenario() -> HashMap<String, Value> {
        let mut scenario = HashMap::new();

        scenario.insert("user".to_string(), Self::sample_user());
        scenario.insert("admin".to_string(), Self::admin_user());
        scenario.insert("workspace".to_string(), Self::sample_workspace());
        scenario.insert("pipeline".to_string(), Self::sample_pipeline());
        scenario.insert("execution".to_string(), Self::sample_execution());
        scenario.insert("command".to_string(), Self::sample_command());
        scenario.insert("event".to_string(), Self::sample_event());
        scenario.insert("config".to_string(), Self::sample_config());
        scenario.insert("webhook".to_string(), Self::sample_webhook_payload());
        scenario.insert("metrics".to_string(), Self::sample_metrics());

        scenario
    }

    /// Generate random test data
    pub fn random_user() -> Value {
        let id = Uuid::new_v4();
        let username = format!("user_{}", &id.to_string()[0..8]);
        let email = format!("{}@example.com", username);

        json!({
            "id": id,
            "username": username,
            "email": email,
            "github_id": format!("{}", rand::random::<u32>()),
            "roles": ["user"],
            "created_at": Utc::now(),
            "last_login": null
        })
    }

    pub fn random_workspace(owner_id: Uuid) -> Value {
        let id = Uuid::new_v4();
        let name = format!("workspace_{}", &id.to_string()[0..8]);

        json!({
            "id": id,
            "name": name,
            "owner_id": owner_id,
            "repository_url": format!("https://github.com/test/{}", name),
            "branch": "main",
            "status": "active",
            "created_at": Utc::now(),
            "updated_at": Utc::now()
        })
    }
}
