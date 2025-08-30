//! Interactive API Documentation System
//!
//! This module provides interactive API documentation with testing capabilities,
//! request/response examples, and live API exploration.

use crate::api::{
    documentation::{EnhancedApiDoc, PipelineRequest, ExecutionRequest},
    versioning::ApiVersion,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Interactive documentation router
pub fn interactive_docs_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(swagger_ui_handler))
        .route("/swagger-ui", get(swagger_ui_handler))
        .route("/redoc", get(redoc_handler))
        .route("/openapi.json", get(openapi_spec_handler))
        .route("/openapi/:version", get(versioned_openapi_spec_handler))
        .route("/examples", get(list_examples))
        .route("/examples/:endpoint", get(get_endpoint_examples))
        .route("/try-it", get(try_it_interface))
        .route("/schemas", get(list_schemas))
        .route("/schemas/:schema", get(get_schema_details))
}

/// Query parameters for documentation
#[derive(Debug, Deserialize)]
struct DocsQuery {
    version: Option<String>,
    theme: Option<String>,
    expand: Option<String>,
}

/// Swagger UI handler
async fn swagger_ui_handler(Query(query): Query<DocsQuery>) -> Html<String> {
    let version = query.version.unwrap_or_else(|| "v2".to_string());
    let theme = query.theme.unwrap_or_else(|| "light".to_string());
    let expand = query.expand.unwrap_or_else(|| "list".to_string());

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RustCI API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui.css" />
    <link rel="icon" type="image/png" href="https://unpkg.com/swagger-ui-dist@5.9.0/favicon-32x32.png" sizes="32x32" />
    <style>
        html {{
            box-sizing: border-box;
            overflow: -moz-scrollbars-vertical;
            overflow-y: scroll;
        }}
        *, *:before, *:after {{
            box-sizing: inherit;
        }}
        body {{
            margin:0;
            background: #fafafa;
        }}
        .swagger-ui .topbar {{
            background-color: #1f2937;
        }}
        .swagger-ui .topbar .download-url-wrapper .select-label {{
            color: #ffffff;
        }}
        .custom-header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            text-align: center;
            margin-bottom: 20px;
        }}
        .version-selector {{
            margin: 10px 0;
        }}
        .version-selector select {{
            padding: 8px 12px;
            border-radius: 4px;
            border: 1px solid #ddd;
            background: white;
        }}
    </style>
</head>
<body>
    <div class="custom-header">
        <h1>🦀 RustCI API Documentation</h1>
        <p>High-performance CI/CD platform built in Rust</p>
        <div class="version-selector">
            <label for="version-select">API Version:</label>
            <select id="version-select" onchange="changeVersion(this.value)">
                <option value="v1" {v1_selected}>v1 (Legacy - Deprecated)</option>
                <option value="v2" {v2_selected}>v2 (Current)</option>
            </select>
        </div>
    </div>
    <div id="swagger-ui"></div>
    
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-standalone-preset.js"></script>
    <script>
        function changeVersion(version) {{
            const url = new URL(window.location);
            url.searchParams.set('version', version);
            window.location.href = url.toString();
        }}
        
        window.onload = function() {{
            const ui = SwaggerUIBundle({{
                url: '/docs/openapi/{version}',
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout",
                theme: "{theme}",
                docExpansion: "{expand}",
                defaultModelsExpandDepth: 2,
                defaultModelExpandDepth: 2,
                showExtensions: true,
                showCommonExtensions: true,
                tryItOutEnabled: true,
                requestInterceptor: function(request) {{
                    // Add version header to all requests
                    request.headers['X-API-Version'] = '{version}';
                    return request;
                }},
                responseInterceptor: function(response) {{
                    // Log response for debugging
                    console.log('API Response:', response);
                    return response;
                }},
                onComplete: function() {{
                    console.log('Swagger UI loaded successfully');
                }},
                onFailure: function(error) {{
                    console.error('Swagger UI failed to load:', error);
                }}
            }});
            
            window.ui = ui;
        }};
    </script>
</body>
</html>
        "#,
        version = version,
        theme = theme,
        expand = expand,
        v1_selected = if version == "v1" { "selected" } else { "" },
        v2_selected = if version == "v2" { "selected" } else { "" },
    );

    Html(html)
}

/// ReDoc handler for alternative documentation UI
async fn redoc_handler(Query(query): Query<DocsQuery>) -> Html<String> {
    let version = query.version.unwrap_or_else(|| "v2".to_string());

    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>RustCI API Documentation - ReDoc</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link href="https://fonts.googleapis.com/css?family=Montserrat:300,400,700|Roboto:300,400,700" rel="stylesheet">
    <style>
        body {{
            margin: 0;
            padding: 0;
        }}
        .custom-header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            text-align: center;
        }}
    </style>
</head>
<body>
    <div class="custom-header">
        <h1>🦀 RustCI API Documentation</h1>
        <p>API Version: {version}</p>
    </div>
    <redoc spec-url='/docs/openapi/{version}'></redoc>
    <script src="https://cdn.jsdelivr.net/npm/redoc@2.1.3/bundles/redoc.standalone.js"></script>
</body>
</html>
        "#,
        version = version
    );

    Html(html)
}

/// OpenAPI specification handler
async fn openapi_spec_handler() -> Json<Value> {
    let openapi = EnhancedApiDoc::with_security();
    Json(serde_json::to_value(openapi).unwrap())
}

/// Version-specific OpenAPI specification handler
async fn versioned_openapi_spec_handler(
    Path(version_str): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let version = ApiVersion::from_str(&version_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "INVALID_VERSION",
                    "message": format!("Invalid API version: {}", version_str)
                }
            })),
        )
    })?;

    let openapi = EnhancedApiDoc::for_version(version);
    Ok(Json(serde_json::to_value(openapi).unwrap()))
}

/// List available examples
async fn list_examples() -> Json<Value> {
    let examples = json!({
        "endpoints": {
            "pipelines": {
                "create_pipeline": {
                    "description": "Create a new CI/CD pipeline",
                    "examples": [
                        {
                            "name": "Simple Node.js Pipeline",
                            "description": "Basic pipeline for Node.js application",
                            "request": create_nodejs_pipeline_example(),
                            "curl": generate_curl_example("POST", "/api/v2/pipelines", Some(create_nodejs_pipeline_example()))
                        },
                        {
                            "name": "Rust Application Pipeline",
                            "description": "Pipeline for Rust application with testing and deployment",
                            "request": create_rust_pipeline_example(),
                            "curl": generate_curl_example("POST", "/api/v2/pipelines", Some(create_rust_pipeline_example()))
                        }
                    ]
                },
                "trigger_pipeline": {
                    "description": "Trigger pipeline execution",
                    "examples": [
                        {
                            "name": "Basic Trigger",
                            "description": "Trigger pipeline with default settings",
                            "request": json!({}),
                            "curl": generate_curl_example("POST", "/api/v2/pipelines/{pipeline_id}/trigger", Some(json!({})))
                        },
                        {
                            "name": "Custom Branch Trigger",
                            "description": "Trigger pipeline for specific branch",
                            "request": json!({
                                "branch": "feature/new-feature",
                                "environment": {
                                    "DEPLOY_ENV": "staging"
                                }
                            }),
                            "curl": generate_curl_example("POST", "/api/v2/pipelines/{pipeline_id}/trigger", Some(json!({
                                "branch": "feature/new-feature",
                                "environment": {
                                    "DEPLOY_ENV": "staging"
                                }
                            })))
                        }
                    ]
                }
            },
            "authentication": {
                "login": {
                    "description": "Authenticate and obtain JWT token",
                    "examples": [
                        {
                            "name": "OAuth Login",
                            "description": "Initiate OAuth login flow",
                            "curl": generate_curl_example("GET", "/api/v2/auth/oauth/github", None)
                        }
                    ]
                }
            }
        },
        "response_examples": {
            "success_responses": {
                "pipeline_created": {
                    "status": 201,
                    "body": json!({
                        "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                        "name": "my-pipeline",
                        "status": "Active",
                        "created_at": "2024-01-01T00:00:00Z"
                    })
                },
                "execution_triggered": {
                    "status": 200,
                    "body": json!({
                        "execution_id": "660e8400-e29b-41d4-a716-446655440001",
                        "status": "Queued",
                        "started_at": "2024-01-01T00:00:00Z"
                    })
                }
            },
            "error_responses": {
                "validation_error": {
                    "status": 400,
                    "body": json!({
                        "error": {
                            "code": "VALIDATION_FAILED",
                            "message": "Input validation failed",
                            "details": {
                                "field_errors": {
                                    "name": ["Pipeline name is required"]
                                }
                            },
                            "recovery_suggestions": [
                                {
                                    "action": "fix_validation_errors",
                                    "description": "Correct the validation errors in the specified fields",
                                    "priority": "High"
                                }
                            ]
                        },
                        "request_id": "req_123456789",
                        "timestamp": "2024-01-01T00:00:00Z"
                    })
                },
                "unauthorized": {
                    "status": 401,
                    "body": json!({
                        "error": {
                            "code": "UNAUTHORIZED",
                            "message": "Authentication required",
                            "recovery_suggestions": [
                                {
                                    "action": "authenticate",
                                    "description": "Provide valid authentication credentials",
                                    "example": "Include 'Authorization: Bearer <token>' header",
                                    "priority": "High"
                                }
                            ]
                        }
                    })
                }
            }
        }
    });

    Json(examples)
}

/// Get examples for specific endpoint
async fn get_endpoint_examples(
    Path(endpoint): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let examples = match endpoint.as_str() {
        "create_pipeline" => json!({
            "endpoint": "POST /api/v2/pipelines",
            "description": "Create a new CI/CD pipeline",
            "request_examples": [
                {
                    "name": "Minimal Pipeline",
                    "request": json!({
                        "name": "minimal-pipeline",
                        "repository_url": "https://github.com/user/repo.git",
                        "config": {
                            "stages": [
                                {
                                    "name": "build",
                                    "steps": [{"run": "echo 'Hello World'"}]
                                }
                            ]
                        }
                    })
                },
                {
                    "name": "Full Pipeline",
                    "request": create_rust_pipeline_example()
                }
            ],
            "response_examples": [
                {
                    "status": 201,
                    "description": "Pipeline created successfully",
                    "body": json!({
                        "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
                        "name": "minimal-pipeline",
                        "status": "Active",
                        "created_at": "2024-01-01T00:00:00Z"
                    })
                }
            ]
        }),
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "ENDPOINT_NOT_FOUND",
                        "message": format!("No examples found for endpoint: {}", endpoint)
                    }
                })),
            ));
        }
    };

    Ok(Json(examples))
}

/// Try-it interface for testing API endpoints
async fn try_it_interface() -> Html<String> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RustCI API Try-It Interface</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #f5f5f5;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            overflow: hidden;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            text-align: center;
        }
        .content {
            padding: 20px;
        }
        .endpoint-selector {
            margin-bottom: 20px;
        }
        .endpoint-selector select {
            width: 100%;
            padding: 10px;
            border: 1px solid #ddd;
            border-radius: 4px;
            font-size: 16px;
        }
        .request-builder {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 20px;
            margin-bottom: 20px;
        }
        .input-group {
            margin-bottom: 15px;
        }
        .input-group label {
            display: block;
            margin-bottom: 5px;
            font-weight: 500;
        }
        .input-group input, .input-group textarea {
            width: 100%;
            padding: 8px;
            border: 1px solid #ddd;
            border-radius: 4px;
            font-family: monospace;
        }
        .input-group textarea {
            height: 200px;
            resize: vertical;
        }
        .button {
            background: #667eea;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 16px;
        }
        .button:hover {
            background: #5a67d8;
        }
        .response-section {
            margin-top: 20px;
            padding: 20px;
            background: #f8f9fa;
            border-radius: 4px;
        }
        .response-headers, .response-body {
            margin-bottom: 15px;
        }
        .response-headers pre, .response-body pre {
            background: #2d3748;
            color: #e2e8f0;
            padding: 15px;
            border-radius: 4px;
            overflow-x: auto;
        }
        .status-success { color: #38a169; }
        .status-error { color: #e53e3e; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🧪 RustCI API Try-It Interface</h1>
            <p>Test API endpoints interactively</p>
        </div>
        <div class="content">
            <div class="endpoint-selector">
                <label for="endpoint">Select Endpoint:</label>
                <select id="endpoint" onchange="loadEndpoint()">
                    <option value="">Choose an endpoint...</option>
                    <option value="create_pipeline">POST /api/v2/pipelines - Create Pipeline</option>
                    <option value="list_pipelines">GET /api/v2/pipelines - List Pipelines</option>
                    <option value="trigger_pipeline">POST /api/v2/pipelines/{id}/trigger - Trigger Pipeline</option>
                    <option value="get_execution">GET /api/v2/executions/{id} - Get Execution</option>
                </select>
            </div>
            
            <div class="request-builder">
                <div>
                    <div class="input-group">
                        <label for="method">HTTP Method:</label>
                        <input type="text" id="method" value="GET" readonly>
                    </div>
                    <div class="input-group">
                        <label for="url">URL:</label>
                        <input type="text" id="url" placeholder="/api/v2/...">
                    </div>
                    <div class="input-group">
                        <label for="headers">Headers (JSON):</label>
                        <textarea id="headers" placeholder='{"Authorization": "Bearer your-token"}'></textarea>
                    </div>
                </div>
                <div>
                    <div class="input-group">
                        <label for="body">Request Body (JSON):</label>
                        <textarea id="body" placeholder="Request body..."></textarea>
                    </div>
                    <button class="button" onclick="sendRequest()">Send Request</button>
                </div>
            </div>
            
            <div id="response-section" class="response-section" style="display: none;">
                <h3>Response</h3>
                <div class="response-headers">
                    <h4>Status: <span id="response-status"></span></h4>
                    <h4>Headers:</h4>
                    <pre id="response-headers-content"></pre>
                </div>
                <div class="response-body">
                    <h4>Body:</h4>
                    <pre id="response-body-content"></pre>
                </div>
            </div>
        </div>
    </div>
    
    <script>
        function loadEndpoint() {
            const endpoint = document.getElementById('endpoint').value;
            const methodEl = document.getElementById('method');
            const urlEl = document.getElementById('url');
            const bodyEl = document.getElementById('body');
            const headersEl = document.getElementById('headers');
            
            // Set default headers
            headersEl.value = JSON.stringify({
                "Authorization": "Bearer your-jwt-token",
                "Content-Type": "application/json",
                "X-API-Version": "v2"
            }, null, 2);
            
            switch(endpoint) {
                case 'create_pipeline':
                    methodEl.value = 'POST';
                    urlEl.value = '/api/v2/pipelines';
                    bodyEl.value = JSON.stringify({
                        "name": "test-pipeline",
                        "description": "A test pipeline",
                        "repository_url": "https://github.com/user/repo.git",
                        "branch": "main",
                        "config": {
                            "stages": [
                                {
                                    "name": "build",
                                    "steps": [
                                        {"run": "echo 'Building...'"}
                                    ]
                                }
                            ]
                        }
                    }, null, 2);
                    break;
                case 'list_pipelines':
                    methodEl.value = 'GET';
                    urlEl.value = '/api/v2/pipelines?page=1&limit=10';
                    bodyEl.value = '';
                    break;
                case 'trigger_pipeline':
                    methodEl.value = 'POST';
                    urlEl.value = '/api/v2/pipelines/{pipeline-id}/trigger';
                    bodyEl.value = JSON.stringify({
                        "branch": "main",
                        "environment": {
                            "NODE_ENV": "production"
                        }
                    }, null, 2);
                    break;
                case 'get_execution':
                    methodEl.value = 'GET';
                    urlEl.value = '/api/v2/executions/{execution-id}';
                    bodyEl.value = '';
                    break;
                default:
                    methodEl.value = 'GET';
                    urlEl.value = '';
                    bodyEl.value = '';
            }
        }
        
        async function sendRequest() {
            const method = document.getElementById('method').value;
            const url = document.getElementById('url').value;
            const headers = document.getElementById('headers').value;
            const body = document.getElementById('body').value;
            
            try {
                const requestHeaders = headers ? JSON.parse(headers) : {};
                const requestOptions = {
                    method: method,
                    headers: requestHeaders
                };
                
                if (body && method !== 'GET') {
                    requestOptions.body = body;
                }
                
                const response = await fetch(url, requestOptions);
                const responseText = await response.text();
                
                // Show response section
                document.getElementById('response-section').style.display = 'block';
                
                // Update status
                const statusEl = document.getElementById('response-status');
                statusEl.textContent = `${response.status} ${response.statusText}`;
                statusEl.className = response.ok ? 'status-success' : 'status-error';
                
                // Update headers
                const responseHeaders = {};
                for (const [key, value] of response.headers.entries()) {
                    responseHeaders[key] = value;
                }
                document.getElementById('response-headers-content').textContent = 
                    JSON.stringify(responseHeaders, null, 2);
                
                // Update body
                try {
                    const jsonBody = JSON.parse(responseText);
                    document.getElementById('response-body-content').textContent = 
                        JSON.stringify(jsonBody, null, 2);
                } catch {
                    document.getElementById('response-body-content').textContent = responseText;
                }
                
            } catch (error) {
                document.getElementById('response-section').style.display = 'block';
                document.getElementById('response-status').textContent = 'Error';
                document.getElementById('response-status').className = 'status-error';
                document.getElementById('response-body-content').textContent = 
                    `Error: ${error.message}`;
            }
        }
    </script>
</body>
</html>
    "#;

    Html(html.to_string())
}

/// List available schemas
async fn list_schemas() -> Json<Value> {
    let schemas = json!({
        "schemas": {
            "PipelineRequest": {
                "description": "Request schema for creating pipelines",
                "properties": ["name", "description", "repository_url", "branch", "config", "triggers"]
            },
            "PipelineResponse": {
                "description": "Response schema for pipeline information",
                "properties": ["pipeline_id", "name", "status", "created_at", "updated_at", "statistics"]
            },
            "ExecutionRequest": {
                "description": "Request schema for triggering executions",
                "properties": ["branch", "commit_sha", "environment", "parameters", "priority"]
            },
            "ExecutionResponse": {
                "description": "Response schema for execution information",
                "properties": ["execution_id", "pipeline_id", "status", "started_at", "completed_at", "steps"]
            },
            "ApiErrorResponse": {
                "description": "Standard error response format",
                "properties": ["error", "request_id", "timestamp", "path", "method"]
            }
        }
    });

    Json(schemas)
}

/// Get detailed schema information
async fn get_schema_details(
    Path(schema_name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let schema_details = match schema_name.as_str() {
        "PipelineRequest" => json!({
            "name": "PipelineRequest",
            "type": "object",
            "description": "Request schema for creating or updating pipelines",
            "required": ["name", "repository_url", "config"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Pipeline name (must be unique)",
                    "minLength": 1,
                    "maxLength": 100,
                    "pattern": "^[a-zA-Z0-9-_]+$",
                    "example": "my-awesome-pipeline"
                },
                "description": {
                    "type": "string",
                    "description": "Pipeline description",
                    "maxLength": 500,
                    "example": "A pipeline for building and testing my application"
                },
                "repository_url": {
                    "type": "string",
                    "format": "uri",
                    "description": "Git repository URL",
                    "example": "https://github.com/user/repo.git"
                },
                "branch": {
                    "type": "string",
                    "description": "Git branch to track",
                    "default": "main",
                    "example": "main"
                },
                "config": {
                    "type": "object",
                    "description": "Pipeline configuration (YAML or JSON)",
                    "example": {
                        "stages": [
                            {
                                "name": "build",
                                "steps": [
                                    {"run": "cargo build --release"}
                                ]
                            }
                        ]
                    }
                }
            }
        }),
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "SCHEMA_NOT_FOUND",
                        "message": format!("Schema '{}' not found", schema_name)
                    }
                })),
            ));
        }
    };

    Ok(Json(schema_details))
}

// Helper functions for generating examples

fn create_nodejs_pipeline_example() -> Value {
    json!({
        "name": "nodejs-app-pipeline",
        "description": "CI/CD pipeline for Node.js application",
        "repository_url": "https://github.com/user/nodejs-app.git",
        "branch": "main",
        "config": {
            "stages": [
                {
                    "name": "install",
                    "steps": [
                        {"run": "npm ci"}
                    ]
                },
                {
                    "name": "test",
                    "steps": [
                        {"run": "npm test"},
                        {"run": "npm run lint"}
                    ]
                },
                {
                    "name": "build",
                    "steps": [
                        {"run": "npm run build"}
                    ]
                },
                {
                    "name": "deploy",
                    "steps": [
                        {"run": "npm run deploy"}
                    ],
                    "when": "branch == 'main'"
                }
            ]
        },
        "triggers": {
            "push": true,
            "pull_request": true,
            "schedule": "0 2 * * *"
        },
        "environment": {
            "NODE_ENV": "production",
            "API_URL": "https://api.example.com"
        },
        "tags": ["nodejs", "web", "frontend"]
    })
}

fn create_rust_pipeline_example() -> Value {
    json!({
        "name": "rust-app-pipeline",
        "description": "CI/CD pipeline for Rust application with comprehensive testing",
        "repository_url": "https://github.com/user/rust-app.git",
        "branch": "main",
        "config": {
            "stages": [
                {
                    "name": "check",
                    "steps": [
                        {"run": "cargo fmt --check"},
                        {"run": "cargo clippy -- -D warnings"}
                    ]
                },
                {
                    "name": "test",
                    "steps": [
                        {"run": "cargo test --verbose"},
                        {"run": "cargo test --doc"}
                    ]
                },
                {
                    "name": "build",
                    "steps": [
                        {"run": "cargo build --release"}
                    ]
                },
                {
                    "name": "security",
                    "steps": [
                        {"run": "cargo audit"}
                    ]
                },
                {
                    "name": "deploy",
                    "steps": [
                        {"run": "docker build -t myapp ."},
                        {"run": "docker push myapp:latest"}
                    ],
                    "when": "branch == 'main'"
                }
            ]
        },
        "triggers": {
            "push": true,
            "pull_request": true
        },
        "environment": {
            "RUST_LOG": "info",
            "DATABASE_URL": "postgresql://localhost/myapp"
        },
        "tags": ["rust", "backend", "api"]
    })
}

fn generate_curl_example(method: &str, url: &str, body: Option<Value>) -> String {
    let mut curl = format!("curl -X {} \\\n  '{}'", method, url);
    
    curl.push_str(" \\\n  -H 'Authorization: Bearer YOUR_JWT_TOKEN'");
    curl.push_str(" \\\n  -H 'Content-Type: application/json'");
    curl.push_str(" \\\n  -H 'X-API-Version: v2'");
    
    if let Some(body) = body {
        curl.push_str(&format!(" \\\n  -d '{}'", serde_json::to_string(&body).unwrap()));
    }
    
    curl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_generation() {
        let nodejs_example = create_nodejs_pipeline_example();
        assert_eq!(nodejs_example["name"], "nodejs-app-pipeline");
        assert!(nodejs_example["config"]["stages"].is_array());

        let rust_example = create_rust_pipeline_example();
        assert_eq!(rust_example["name"], "rust-app-pipeline");
        assert!(rust_example["tags"].as_array().unwrap().contains(&json!("rust")));
    }

    #[test]
    fn test_curl_generation() {
        let curl = generate_curl_example("POST", "/api/v2/test", Some(json!({"test": "data"})));
        assert!(curl.contains("POST"));
        assert!(curl.contains("/api/v2/test"));
        assert!(curl.contains("Authorization: Bearer"));
        assert!(curl.contains(r#"{"test":"data"}"#));
    }
}