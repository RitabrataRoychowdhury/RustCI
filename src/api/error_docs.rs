//! API Error Documentation and Troubleshooting System
//!
//! This module provides comprehensive error documentation, troubleshooting guides,
//! and error resolution assistance for API consumers.

use crate::api::errors::{ApiErrorCode, RecoverySuggestion, SuggestionPriority};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Error documentation router
pub fn error_docs_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/errors", get(list_error_codes))
        .route("/errors/:code", get(get_error_documentation))
        .route("/errors/:code/troubleshoot", get(get_troubleshooting_guide))
        .route("/errors/search", get(search_errors))
        .route("/troubleshooting", get(get_general_troubleshooting))
}

/// Query parameters for error listing
#[derive(Debug, Deserialize)]
struct ErrorListQuery {
    category: Option<String>,
    severity: Option<String>,
    include_examples: Option<bool>,
}

/// Query parameters for error search
#[derive(Debug, Deserialize)]
struct ErrorSearchQuery {
    q: String,
    limit: Option<u32>,
}

/// Error documentation structure
#[derive(Debug, Serialize)]
struct ErrorDocumentation {
    pub code: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub severity: String,
    pub http_status: u16,
    pub common_causes: Vec<String>,
    pub resolution_steps: Vec<ResolutionStep>,
    pub examples: Vec<ErrorExample>,
    pub related_errors: Vec<String>,
    pub prevention_tips: Vec<String>,
}

/// Resolution step for error fixing
#[derive(Debug, Serialize)]
struct ResolutionStep {
    pub step: u32,
    pub title: String,
    pub description: String,
    pub code_example: Option<String>,
    pub expected_outcome: String,
}

/// Error example with context
#[derive(Debug, Serialize)]
struct ErrorExample {
    pub scenario: String,
    pub request_example: Option<Value>,
    pub response_example: Value,
    pub explanation: String,
}

/// Troubleshooting guide structure
#[derive(Debug, Serialize)]
struct TroubleshootingGuide {
    pub error_code: String,
    pub quick_fixes: Vec<QuickFix>,
    pub diagnostic_steps: Vec<DiagnosticStep>,
    pub advanced_troubleshooting: Vec<AdvancedStep>,
    pub when_to_contact_support: Vec<String>,
}

/// Quick fix suggestion
#[derive(Debug, Serialize)]
struct QuickFix {
    pub title: String,
    pub description: String,
    pub command: Option<String>,
    pub success_indicator: String,
}

/// Diagnostic step
#[derive(Debug, Serialize)]
struct DiagnosticStep {
    pub step: u32,
    pub title: String,
    pub description: String,
    pub check_command: Option<String>,
    pub expected_result: String,
    pub if_failed: String,
}

/// Advanced troubleshooting step
#[derive(Debug, Serialize)]
struct AdvancedStep {
    pub title: String,
    pub description: String,
    pub prerequisites: Vec<String>,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

/// List all error codes with documentation
async fn list_error_codes(
    Query(query): Query<ErrorListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let all_errors = get_all_error_documentation();
    
    let mut filtered_errors = all_errors;
    
    // Filter by category if specified
    if let Some(category) = &query.category {
        filtered_errors.retain(|error| error.category.to_lowercase() == category.to_lowercase());
    }
    
    // Filter by severity if specified
    if let Some(severity) = &query.severity {
        filtered_errors.retain(|error| error.severity.to_lowercase() == severity.to_lowercase());
    }
    
    // Remove examples if not requested
    if !query.include_examples.unwrap_or(false) {
        for error in &mut filtered_errors {
            error.examples.clear();
        }
    }
    
    let response = json!({
        "errors": filtered_errors,
        "total_count": filtered_errors.len(),
        "categories": get_error_categories(),
        "severities": ["low", "medium", "high", "critical"]
    });
    
    Ok(Json(response))
}

/// Get documentation for a specific error code
async fn get_error_documentation(
    Path(code): Path<String>,
) -> Result<Json<ErrorDocumentation>, (StatusCode, Json<Value>)> {
    let error_code = ApiErrorCode::from_str(&code.to_uppercase()).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "ERROR_CODE_NOT_FOUND",
                    "message": format!("Error code '{}' not found", code)
                }
            })),
        )
    })?;
    
    let documentation = create_error_documentation(error_code);
    Ok(Json(documentation))
}

/// Get troubleshooting guide for a specific error
async fn get_troubleshooting_guide(
    Path(code): Path<String>,
) -> Result<Json<TroubleshootingGuide>, (StatusCode, Json<Value>)> {
    let error_code = ApiErrorCode::from_str(&code.to_uppercase()).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "ERROR_CODE_NOT_FOUND",
                    "message": format!("Error code '{}' not found", code)
                }
            })),
        )
    })?;
    
    let guide = create_troubleshooting_guide(error_code);
    Ok(Json(guide))
}

/// Search errors by description or symptoms
async fn search_errors(
    Query(query): Query<ErrorSearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let all_errors = get_all_error_documentation();
    let search_term = query.q.to_lowercase();
    let limit = query.limit.unwrap_or(10);
    
    let mut matching_errors: Vec<_> = all_errors
        .into_iter()
        .filter(|error| {
            error.name.to_lowercase().contains(&search_term)
                || error.description.to_lowercase().contains(&search_term)
                || error.common_causes.iter().any(|cause| cause.to_lowercase().contains(&search_term))
        })
        .take(limit as usize)
        .collect();
    
    // Sort by relevance (simple scoring based on matches)
    matching_errors.sort_by(|a, b| {
        let score_a = calculate_relevance_score(&a.name, &a.description, &search_term);
        let score_b = calculate_relevance_score(&b.name, &b.description, &search_term);
        score_b.cmp(&score_a)
    });
    
    let response = json!({
        "results": matching_errors,
        "total_found": matching_errors.len(),
        "search_term": query.q,
        "suggestions": generate_search_suggestions(&query.q)
    });
    
    Ok(Json(response))
}

/// Get general troubleshooting information
async fn get_general_troubleshooting() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let troubleshooting_info = json!({
        "general_steps": [
            {
                "step": 1,
                "title": "Check API Version",
                "description": "Ensure you're using the correct API version",
                "details": "Verify the API version in your request headers or URL path"
            },
            {
                "step": 2,
                "title": "Verify Authentication",
                "description": "Check that your authentication credentials are valid",
                "details": "Ensure your API key or JWT token is valid and not expired"
            },
            {
                "step": 3,
                "title": "Validate Request Format",
                "description": "Ensure your request follows the correct format",
                "details": "Check request headers, body format, and required fields"
            },
            {
                "step": 4,
                "title": "Check Rate Limits",
                "description": "Verify you haven't exceeded rate limits",
                "details": "Check rate limit headers in previous responses"
            },
            {
                "step": 5,
                "title": "Review Error Details",
                "description": "Examine the error response for specific guidance",
                "details": "Look at error codes, messages, and recovery suggestions"
            }
        ],
        "common_issues": [
            {
                "issue": "Authentication Failures",
                "symptoms": ["401 Unauthorized", "403 Forbidden"],
                "solutions": ["Check API credentials", "Verify token expiration", "Review permissions"]
            },
            {
                "issue": "Validation Errors",
                "symptoms": ["400 Bad Request", "422 Unprocessable Entity"],
                "solutions": ["Check required fields", "Validate data formats", "Review API documentation"]
            },
            {
                "issue": "Rate Limiting",
                "symptoms": ["429 Too Many Requests"],
                "solutions": ["Implement exponential backoff", "Reduce request frequency", "Check rate limit headers"]
            }
        ],
        "debugging_tips": [
            "Always include the request ID when reporting issues",
            "Check the API status page for known issues",
            "Use the API documentation for correct request formats",
            "Implement proper error handling in your application",
            "Monitor your API usage to avoid rate limits"
        ],
        "support_resources": {
            "documentation": "/docs/api",
            "status_page": "/status",
            "support_email": "api-support@rustci.dev",
            "community_forum": "https://community.rustci.dev"
        }
    });
    
    Ok(Json(troubleshooting_info))
}

// Helper functions

fn get_all_error_documentation() -> Vec<ErrorDocumentation> {
    vec![
        create_error_documentation(ApiErrorCode::Unauthorized),
        create_error_documentation(ApiErrorCode::Forbidden),
        create_error_documentation(ApiErrorCode::ValidationFailed),
        create_error_documentation(ApiErrorCode::ResourceNotFound),
        create_error_documentation(ApiErrorCode::RateLimitExceeded),
        create_error_documentation(ApiErrorCode::InternalServerError),
        create_error_documentation(ApiErrorCode::PipelineNotFound),
        create_error_documentation(ApiErrorCode::PipelineExecutionFailed),
        create_error_documentation(ApiErrorCode::InvalidPipelineConfig),
        // Add more as needed
    ]
}

fn create_error_documentation(error_code: ApiErrorCode) -> ErrorDocumentation {
    match error_code {
        ApiErrorCode::Unauthorized => ErrorDocumentation {
            code: "UNAUTHORIZED".to_string(),
            name: "Unauthorized Access".to_string(),
            description: "The request requires authentication credentials".to_string(),
            category: "Authentication".to_string(),
            severity: "high".to_string(),
            http_status: 401,
            common_causes: vec![
                "Missing authentication token".to_string(),
                "Expired authentication token".to_string(),
                "Invalid authentication credentials".to_string(),
                "Malformed Authorization header".to_string(),
            ],
            resolution_steps: vec![
                ResolutionStep {
                    step: 1,
                    title: "Check Authentication Header".to_string(),
                    description: "Ensure the Authorization header is present and correctly formatted".to_string(),
                    code_example: Some("Authorization: Bearer <your-jwt-token>".to_string()),
                    expected_outcome: "Request includes valid authorization header".to_string(),
                },
                ResolutionStep {
                    step: 2,
                    title: "Verify Token Validity".to_string(),
                    description: "Check that your authentication token is not expired".to_string(),
                    code_example: Some("curl -H \"Authorization: Bearer <token>\" /api/v2/auth/verify".to_string()),
                    expected_outcome: "Token validation returns success".to_string(),
                },
            ],
            examples: vec![
                ErrorExample {
                    scenario: "Missing Authorization Header".to_string(),
                    request_example: Some(json!({
                        "method": "GET",
                        "url": "/api/v2/pipelines",
                        "headers": {}
                    })),
                    response_example: json!({
                        "error": {
                            "code": "UNAUTHORIZED",
                            "message": "Authentication required"
                        }
                    }),
                    explanation: "Request made without authentication credentials".to_string(),
                },
            ],
            related_errors: vec!["FORBIDDEN".to_string(), "TOKEN_EXPIRED".to_string()],
            prevention_tips: vec![
                "Always include authentication headers in requests".to_string(),
                "Implement token refresh logic before expiration".to_string(),
                "Store tokens securely and never expose them in logs".to_string(),
            ],
        },
        
        ApiErrorCode::ValidationFailed => ErrorDocumentation {
            code: "VALIDATION_FAILED".to_string(),
            name: "Input Validation Failed".to_string(),
            description: "The request data failed validation checks".to_string(),
            category: "Validation".to_string(),
            severity: "medium".to_string(),
            http_status: 400,
            common_causes: vec![
                "Missing required fields".to_string(),
                "Invalid data format".to_string(),
                "Values outside allowed range".to_string(),
                "Incorrect data types".to_string(),
            ],
            resolution_steps: vec![
                ResolutionStep {
                    step: 1,
                    title: "Review Validation Errors".to_string(),
                    description: "Check the error details for specific field validation failures".to_string(),
                    code_example: None,
                    expected_outcome: "Identify which fields need correction".to_string(),
                },
                ResolutionStep {
                    step: 2,
                    title: "Correct Invalid Fields".to_string(),
                    description: "Fix the data format and values for failed fields".to_string(),
                    code_example: Some("Ensure email format: user@example.com".to_string()),
                    expected_outcome: "All fields pass validation".to_string(),
                },
            ],
            examples: vec![
                ErrorExample {
                    scenario: "Invalid Email Format".to_string(),
                    request_example: Some(json!({
                        "email": "invalid-email",
                        "name": "John Doe"
                    })),
                    response_example: json!({
                        "error": {
                            "code": "VALIDATION_FAILED",
                            "message": "Input validation failed",
                            "details": {
                                "field_errors": {
                                    "email": ["Invalid email format"]
                                }
                            }
                        }
                    }),
                    explanation: "Email field contains invalid format".to_string(),
                },
            ],
            related_errors: vec!["INVALID_INPUT".to_string(), "MISSING_REQUIRED_FIELD".to_string()],
            prevention_tips: vec![
                "Validate input on the client side before sending".to_string(),
                "Use proper data types and formats".to_string(),
                "Check API documentation for field requirements".to_string(),
            ],
        },
        
        // Add more error documentation as needed
        _ => ErrorDocumentation {
            code: error_code.to_string(),
            name: "Error".to_string(),
            description: "An error occurred".to_string(),
            category: "General".to_string(),
            severity: "medium".to_string(),
            http_status: error_code.status_code().as_u16(),
            common_causes: vec!["Various causes".to_string()],
            resolution_steps: vec![],
            examples: vec![],
            related_errors: vec![],
            prevention_tips: vec![],
        },
    }
}

fn create_troubleshooting_guide(error_code: ApiErrorCode) -> TroubleshootingGuide {
    match error_code {
        ApiErrorCode::Unauthorized => TroubleshootingGuide {
            error_code: "UNAUTHORIZED".to_string(),
            quick_fixes: vec![
                QuickFix {
                    title: "Add Authorization Header".to_string(),
                    description: "Include the Authorization header with your JWT token".to_string(),
                    command: Some("curl -H \"Authorization: Bearer <token>\" <url>".to_string()),
                    success_indicator: "Request returns 200 OK instead of 401".to_string(),
                },
            ],
            diagnostic_steps: vec![
                DiagnosticStep {
                    step: 1,
                    title: "Check Token Presence".to_string(),
                    description: "Verify that the Authorization header is included in the request".to_string(),
                    check_command: Some("Check request headers in browser dev tools or API client".to_string()),
                    expected_result: "Authorization header is present".to_string(),
                    if_failed: "Add the Authorization header to your request".to_string(),
                },
            ],
            advanced_troubleshooting: vec![
                AdvancedStep {
                    title: "Token Debugging".to_string(),
                    description: "Decode and inspect JWT token contents".to_string(),
                    prerequisites: vec!["JWT debugging tool".to_string()],
                    steps: vec![
                        "Copy the JWT token from your request".to_string(),
                        "Use jwt.io or similar tool to decode the token".to_string(),
                        "Check the 'exp' claim for expiration".to_string(),
                        "Verify the 'aud' and 'iss' claims match expected values".to_string(),
                    ],
                    warnings: vec!["Never share JWT tokens in public forums".to_string()],
                },
            ],
            when_to_contact_support: vec![
                "Token appears valid but still getting 401 errors".to_string(),
                "Authentication worked previously but suddenly stopped".to_string(),
                "Getting 401 errors on endpoints that don't require authentication".to_string(),
            ],
        },
        _ => TroubleshootingGuide {
            error_code: error_code.to_string(),
            quick_fixes: vec![],
            diagnostic_steps: vec![],
            advanced_troubleshooting: vec![],
            when_to_contact_support: vec!["Contact support if the issue persists".to_string()],
        },
    }
}

fn get_error_categories() -> Vec<String> {
    vec![
        "Authentication".to_string(),
        "Authorization".to_string(),
        "Validation".to_string(),
        "Resource Management".to_string(),
        "Rate Limiting".to_string(),
        "System".to_string(),
        "Pipeline".to_string(),
        "File Operations".to_string(),
    ]
}

fn calculate_relevance_score(name: &str, description: &str, search_term: &str) -> u32 {
    let mut score = 0;
    
    // Exact match in name gets highest score
    if name.to_lowercase().contains(search_term) {
        score += 10;
    }
    
    // Match in description gets medium score
    if description.to_lowercase().contains(search_term) {
        score += 5;
    }
    
    // Word boundary matches get bonus points
    if name.to_lowercase().split_whitespace().any(|word| word == search_term) {
        score += 5;
    }
    
    score
}

fn generate_search_suggestions(query: &str) -> Vec<String> {
    let common_terms = vec![
        "authentication", "validation", "not found", "rate limit", 
        "server error", "pipeline", "execution", "token", "permission"
    ];
    
    common_terms
        .into_iter()
        .filter(|term| term.contains(&query.to_lowercase()) || query.to_lowercase().contains(term))
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_documentation_creation() {
        let doc = create_error_documentation(ApiErrorCode::Unauthorized);
        assert_eq!(doc.code, "UNAUTHORIZED");
        assert_eq!(doc.http_status, 401);
        assert!(!doc.common_causes.is_empty());
    }

    #[test]
    fn test_troubleshooting_guide_creation() {
        let guide = create_troubleshooting_guide(ApiErrorCode::Unauthorized);
        assert_eq!(guide.error_code, "UNAUTHORIZED");
        assert!(!guide.quick_fixes.is_empty());
    }

    #[test]
    fn test_relevance_scoring() {
        let score1 = calculate_relevance_score("Authentication Error", "User authentication failed", "auth");
        let score2 = calculate_relevance_score("Validation Error", "Input validation failed", "auth");
        assert!(score1 > score2);
    }

    #[test]
    fn test_search_suggestions() {
        let suggestions = generate_search_suggestions("auth");
        assert!(suggestions.contains(&"authentication".to_string()));
    }
}