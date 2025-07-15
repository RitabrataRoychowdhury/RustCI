use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Documentation generator for API endpoints
#[allow(dead_code)]
pub struct DocumentationGenerator {
    pub base_url: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub path: String,
    pub method: String,
    pub description: String,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBodySchema>,
    pub responses: HashMap<u16, ResponseSchema>,
    pub curl_example: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub param_type: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Body,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBodySchema {
    pub content_type: String,
    pub schema: String,
    pub example: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseSchema {
    pub description: String,
    pub content_type: String,
    pub schema: String,
    pub example: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CurlExample {
    pub endpoint: String,
    pub method: String,
    pub curl_command: String,
    pub description: String,
    pub expected_response: String,
}

#[allow(dead_code)]
impl DocumentationGenerator {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            auth_token: None,
        }
    }

    pub fn with_auth(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// Generate complete API documentation
    pub fn generate_api_docs(&self) -> String {
        let endpoints = self.get_all_endpoints();
        let mut docs = String::new();

        docs.push_str("# RustCI API Documentation\n\n");
        docs.push_str("Complete API reference for the RustCI platform with working cURL examples.\n\n");
        docs.push_str(&format!("## Base URL\n\n```\n{}\n```\n\n", self.base_url));

        // Authentication section
        docs.push_str("## Authentication\n\n");
        docs.push_str("Most endpoints require JWT authentication. Include the token in the Authorization header:\n\n");
        docs.push_str("```bash\nAuthorization: Bearer YOUR_JWT_TOKEN\n```\n\n");

        // Generate documentation for each endpoint
        for endpoint in endpoints {
            docs.push_str(&self.generate_endpoint_docs(&endpoint));
        }

        docs
    }

    /// Generate cURL examples for all endpoints
    pub fn generate_curl_examples(&self) -> Vec<CurlExample> {
        let endpoints = self.get_all_endpoints();
        endpoints.into_iter().map(|endpoint| {
            CurlExample {
                endpoint: endpoint.path.clone(),
                method: endpoint.method.clone(),
                curl_command: endpoint.curl_example.clone(),
                description: endpoint.description.clone(),
                expected_response: self.get_expected_response(&endpoint),
            }
        }).collect()
    }

    /// Get all API endpoints
    fn get_all_endpoints(&self) -> Vec<ApiEndpoint> {
        vec![
            // Health check
            ApiEndpoint {
                path: "/api/healthchecker".to_string(),
                method: "GET".to_string(),
                description: "Check if the API is running".to_string(),
                parameters: vec![],
                request_body: None,
                responses: self.create_health_responses(),
                curl_example: format!("curl -X GET {}/api/healthchecker", self.base_url),
            },

            // Pipeline creation (JSON)
            ApiEndpoint {
                path: "/api/ci/pipelines".to_string(),
                method: "POST".to_string(),
                description: "Create a new pipeline from JSON payload".to_string(),
                parameters: vec![],
                request_body: Some(RequestBodySchema {
                    content_type: "application/json".to_string(),
                    schema: "CreatePipelineRequest".to_string(),
                    example: r#"{"yaml_content": "name: \"Test Pipeline\"\nstages: [{\"name\": \"build\", \"steps\": []}]"}"#.to_string(),
                }),
                responses: self.create_pipeline_responses(),
                curl_example: format!(
                    "curl -X POST {}/api/ci/pipelines \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"yaml_content\": \"name: \\\"Test Pipeline\\\"\\nstages: [{{\\\"name\\\": \\\"build\\\", \\\"steps\\\": []}}]\"}}'",
                    self.base_url
                ),
            },

            // Pipeline creation (Multipart)
            ApiEndpoint {
                path: "/api/ci/pipelines/upload".to_string(),
                method: "POST".to_string(),
                description: "Create a new pipeline from uploaded YAML file".to_string(),
                parameters: vec![],
                request_body: Some(RequestBodySchema {
                    content_type: "multipart/form-data".to_string(),
                    schema: "File upload".to_string(),
                    example: "pipeline=@pipeline.yaml".to_string(),
                }),
                responses: self.create_pipeline_responses(),
                curl_example: format!(
                    "curl -X POST {}/api/ci/pipelines/upload \\\n  -F \"pipeline=@pipeline.yaml\"",
                    self.base_url
                ),
            },

            // List pipelines
            ApiEndpoint {
                path: "/api/ci/pipelines".to_string(),
                method: "GET".to_string(),
                description: "List all pipelines".to_string(),
                parameters: vec![],
                request_body: None,
                responses: self.create_pipeline_list_responses(),
                curl_example: format!("curl -X GET {}/api/ci/pipelines", self.base_url),
            },

            // Trigger pipeline
            ApiEndpoint {
                path: "/api/ci/pipelines/:pipeline_id/trigger".to_string(),
                method: "POST".to_string(),
                description: "Trigger a pipeline execution".to_string(),
                parameters: vec![
                    Parameter {
                        name: "pipeline_id".to_string(),
                        location: ParameterLocation::Path,
                        required: true,
                        param_type: "UUID".to_string(),
                        description: "Pipeline ID".to_string(),
                    }
                ],
                request_body: Some(RequestBodySchema {
                    content_type: "application/json".to_string(),
                    schema: "TriggerPipelineRequest".to_string(),
                    example: r#"{"trigger_type": "manual", "environment": {"NODE_ENV": "production"}}"#.to_string(),
                }),
                responses: self.create_trigger_responses(),
                curl_example: format!(
                    "curl -X POST {}/api/ci/pipelines/{{pipeline_id}}/trigger \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"trigger_type\": \"manual\", \"environment\": {{\"NODE_ENV\": \"production\"}}}}'",
                    self.base_url
                ),
            },

            // Get execution
            ApiEndpoint {
                path: "/api/ci/executions/:execution_id".to_string(),
                method: "GET".to_string(),
                description: "Get execution status and details".to_string(),
                parameters: vec![
                    Parameter {
                        name: "execution_id".to_string(),
                        location: ParameterLocation::Path,
                        required: true,
                        param_type: "UUID".to_string(),
                        description: "Execution ID".to_string(),
                    }
                ],
                request_body: None,
                responses: self.create_execution_responses(),
                curl_example: format!("curl -X GET {}/api/ci/executions/{{execution_id}}", self.base_url),
            },

            // List executions
            ApiEndpoint {
                path: "/api/ci/executions".to_string(),
                method: "GET".to_string(),
                description: "List all executions or filter by pipeline".to_string(),
                parameters: vec![
                    Parameter {
                        name: "pipeline_id".to_string(),
                        location: ParameterLocation::Query,
                        required: false,
                        param_type: "UUID".to_string(),
                        description: "Filter by pipeline ID".to_string(),
                    }
                ],
                request_body: None,
                responses: self.create_execution_list_responses(),
                curl_example: format!("curl -X GET {}/api/ci/executions", self.base_url),
            },

            // Cancel execution
            ApiEndpoint {
                path: "/api/ci/executions/:execution_id/cancel".to_string(),
                method: "DELETE".to_string(),
                description: "Cancel a running execution".to_string(),
                parameters: vec![
                    Parameter {
                        name: "execution_id".to_string(),
                        location: ParameterLocation::Path,
                        required: true,
                        param_type: "UUID".to_string(),
                        description: "Execution ID".to_string(),
                    }
                ],
                request_body: None,
                responses: self.create_cancel_responses(),
                curl_example: format!("curl -X DELETE {}/api/ci/executions/{{execution_id}}/cancel", self.base_url),
            },

            // Webhook handler
            ApiEndpoint {
                path: "/api/ci/pipelines/:pipeline_id/webhook".to_string(),
                method: "POST".to_string(),
                description: "Trigger pipeline via webhook (GitHub format)".to_string(),
                parameters: vec![
                    Parameter {
                        name: "pipeline_id".to_string(),
                        location: ParameterLocation::Path,
                        required: true,
                        param_type: "UUID".to_string(),
                        description: "Pipeline ID".to_string(),
                    }
                ],
                request_body: Some(RequestBodySchema {
                    content_type: "application/json".to_string(),
                    schema: "GitHub webhook payload".to_string(),
                    example: r#"{"ref": "refs/heads/main", "after": "abc123", "repository": {"full_name": "user/repo"}}"#.to_string(),
                }),
                responses: self.create_trigger_responses(),
                curl_example: format!(
                    "curl -X POST {}/api/ci/pipelines/{{pipeline_id}}/webhook \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"ref\": \"refs/heads/main\", \"after\": \"abc123\", \"repository\": {{\"full_name\": \"user/repo\"}}}}'",
                    self.base_url
                ),
            },
        ]
    }

    /// Generate documentation for a single endpoint
    fn generate_endpoint_docs(&self, endpoint: &ApiEndpoint) -> String {
        let mut docs = String::new();

        docs.push_str(&format!("### {} {}\n\n", endpoint.method, endpoint.path));
        docs.push_str(&format!("{}\n\n", endpoint.description));

        // Parameters
        if !endpoint.parameters.is_empty() {
            docs.push_str("**Parameters:**\n");
            for param in &endpoint.parameters {
                docs.push_str(&format!(
                    "- `{}` ({:?}): {} - {}\n",
                    param.name,
                    param.location,
                    if param.required { "Required" } else { "Optional" },
                    param.description
                ));
            }
            docs.push_str("\n");
        }

        // Request body
        if let Some(body) = &endpoint.request_body {
            docs.push_str("**Request Body:**\n");
            docs.push_str(&format!("```json\n{}\n```\n\n", body.example));
        }

        // cURL example
        docs.push_str("**cURL Example:**\n");
        docs.push_str(&format!("```bash\n{}\n```\n\n", endpoint.curl_example));

        // Response
        docs.push_str("**Response:**\n");
        if let Some(response) = endpoint.responses.get(&200) {
            docs.push_str(&format!("```json\n{}\n```\n\n", response.example));
        }

        docs.push_str("---\n\n");
        docs
    }

    fn get_expected_response(&self, endpoint: &ApiEndpoint) -> String {
        endpoint.responses.get(&200)
            .map(|r| r.example.clone())
            .unwrap_or_else(|| "Success".to_string())
    }

    // Response creation helpers
    fn create_health_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "Health check successful".to_string(),
            content_type: "application/json".to_string(),
            schema: "HealthResponse".to_string(),
            example: r#"{"status": "success", "message": "DevOps CI Server is running! 🚀"}"#.to_string(),
        });
        responses
    }

    fn create_pipeline_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "Pipeline created successfully".to_string(),
            content_type: "application/json".to_string(),
            schema: "PipelineResponse".to_string(),
            example: r#"{"id": "550e8400-e29b-41d4-a716-446655440000", "name": "Test Pipeline", "description": "A test pipeline"}"#.to_string(),
        });
        responses
    }

    fn create_pipeline_list_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "List of pipelines".to_string(),
            content_type: "application/json".to_string(),
            schema: "Array<PipelineResponse>".to_string(),
            example: r#"[{"id": "550e8400-e29b-41d4-a716-446655440000", "name": "Test Pipeline"}]"#.to_string(),
        });
        responses
    }

    fn create_trigger_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "Pipeline triggered successfully".to_string(),
            content_type: "application/json".to_string(),
            schema: "TriggerResponse".to_string(),
            example: r#"{"execution_id": "660e8400-e29b-41d4-a716-446655440001", "message": "Pipeline triggered successfully"}"#.to_string(),
        });
        responses
    }

    fn create_execution_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "Execution details".to_string(),
            content_type: "application/json".to_string(),
            schema: "PipelineExecution".to_string(),
            example: r#"{"id": "660e8400-e29b-41d4-a716-446655440001", "status": "running", "started_at": "2024-01-15T10:35:00Z"}"#.to_string(),
        });
        responses
    }

    fn create_execution_list_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "List of executions".to_string(),
            content_type: "application/json".to_string(),
            schema: "Array<ExecutionResponse>".to_string(),
            example: r#"[{"id": "660e8400-e29b-41d4-a716-446655440001", "status": "completed"}]"#.to_string(),
        });
        responses
    }

    fn create_cancel_responses(&self) -> HashMap<u16, ResponseSchema> {
        let mut responses = HashMap::new();
        responses.insert(200, ResponseSchema {
            description: "Execution cancelled".to_string(),
            content_type: "application/json".to_string(),
            schema: "CancelResponse".to_string(),
            example: r#"{"message": "Execution cancelled successfully", "execution_id": "660e8400-e29b-41d4-a716-446655440001"}"#.to_string(),
        });
        responses
    }
}

/// Helper function to create a documentation generator with default settings
#[allow(dead_code)]
pub fn create_docs_generator() -> DocumentationGenerator {
    let base_url = std::env::var("BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    
    DocumentationGenerator::new(base_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_documentation_generator() {
        let generator = DocumentationGenerator::new("http://localhost:8000".to_string());
        let docs = generator.generate_api_docs();
        
        assert!(docs.contains("# RustCI API Documentation"));
        assert!(docs.contains("/api/healthchecker"));
        assert!(docs.contains("/api/ci/pipelines"));
    }

    #[test]
    fn test_curl_examples() {
        let generator = DocumentationGenerator::new("http://localhost:8000".to_string());
        let examples = generator.generate_curl_examples();
        
        assert!(!examples.is_empty());
        assert!(examples.iter().any(|e| e.endpoint.contains("/api/healthchecker")));
        assert!(examples.iter().any(|e| e.endpoint.contains("/api/ci/pipelines")));
    }
}