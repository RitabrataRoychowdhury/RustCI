//! Input Validation Middleware
//!
//! This module provides middleware for automatic input validation and sanitization
//! across all API endpoints.

use crate::core::security::input_sanitizer::{InputSanitizer, ValidationRule, ValidationResult};
use crate::error::AppError;
use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Configuration for input validation middleware
#[derive(Debug, Clone)]
pub struct ValidationMiddlewareConfig {
    pub enabled: bool,
    pub strict_mode: bool,
    pub validate_headers: bool,
    pub validate_query_params: bool,
    pub validate_json_body: bool,
    pub max_body_size: usize,
    pub endpoint_rules: HashMap<String, Vec<ValidationRule>>,
}

impl Default for ValidationMiddlewareConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strict_mode: true,
            validate_headers: true,
            validate_query_params: true,
            validate_json_body: true,
            max_body_size: 1_048_576, // 1MB
            endpoint_rules: HashMap::new(),
        }
    }
}

/// Input validation middleware
pub struct InputValidationMiddleware {
    sanitizer: InputSanitizer,
    config: ValidationMiddlewareConfig,
}

impl InputValidationMiddleware {
    /// Create new input validation middleware
    pub fn new(config: ValidationMiddlewareConfig) -> Result<Self, AppError> {
        let sanitizer = InputSanitizer::new()?;
        Ok(Self { sanitizer, config })
    }

    /// Validate request and continue to next middleware if valid
    pub async fn validate_request(
        &self,
        mut request: Request,
        next: Next,
    ) -> Result<Response, AppError> {
        if !self.config.enabled {
            return Ok(next.run(request).await);
        }

        let path = request.uri().path().to_string();
        debug!("Validating request for path: {}", path);

        // Validate headers if configured
        if self.config.validate_headers {
            if let Err(e) = self.validate_headers(request.headers()).await {
                warn!("Header validation failed for {}: {}", path, e);
                return Ok(self.create_validation_error_response(e));
            }
        }

        // Validate query parameters if configured
        if self.config.validate_query_params {
            if let Some(query) = request.uri().query() {
                if let Err(e) = self.validate_query_string(query).await {
                    warn!("Query parameter validation failed for {}: {}", path, e);
                    return Ok(self.create_validation_error_response(e));
                }
            }
        }

        // Validate JSON body if configured
        if self.config.validate_json_body {
            let content_type = request
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if content_type.contains("application/json") {
                match self.validate_and_sanitize_json_body(&mut request).await {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("JSON body validation failed for {}: {}", path, e);
                        return Ok(self.create_validation_error_response(e));
                    }
                }
            }
        }

        Ok(next.run(request).await)
    }

    /// Validate HTTP headers
    async fn validate_headers(&self, headers: &HeaderMap) -> Result<(), AppError> {
        let common_rules = vec![
            ValidationRule::Length { min: 0, max: 1000 },
            ValidationRule::XssProtection,
            ValidationRule::CommandInjection,
        ];

        for (name, value) in headers {
            let header_name = name.as_str();
            let header_value = value.to_str().map_err(|_| {
                AppError::ValidationError("Invalid header value encoding".to_string())
            })?;

            // Skip validation for certain headers
            if self.should_skip_header_validation(header_name) {
                continue;
            }

            let result = self
                .sanitizer
                .validate_and_sanitize(header_value, &common_rules)?;

            if !result.is_valid && self.config.strict_mode {
                return Err(AppError::ValidationError(format!(
                    "Invalid header '{}': {}",
                    header_name,
                    result.violations
                        .iter()
                        .map(|v| v.description.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }

        Ok(())
    }

    /// Validate query string parameters
    async fn validate_query_string(&self, query: &str) -> Result<(), AppError> {
        let rules = vec![
            ValidationRule::Length { min: 0, max: 2000 },
            ValidationRule::SqlInjection,
            ValidationRule::XssProtection,
            ValidationRule::CommandInjection,
        ];

        // Parse query parameters
        let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        for (key, value) in params {
            // Validate parameter name
            let key_result = self.sanitizer.validate_and_sanitize(&key, &rules)?;
            if !key_result.is_valid && self.config.strict_mode {
                return Err(AppError::ValidationError(format!(
                    "Invalid query parameter name '{}': {}",
                    key,
                    key_result.violations
                        .iter()
                        .map(|v| v.description.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }

            // Validate parameter value
            let value_result = self.sanitizer.validate_and_sanitize(&value, &rules)?;
            if !value_result.is_valid && self.config.strict_mode {
                return Err(AppError::ValidationError(format!(
                    "Invalid query parameter value for '{}': {}",
                    key,
                    value_result.violations
                        .iter()
                        .map(|v| v.description.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }

        Ok(())
    }

    /// Validate and sanitize JSON body
    async fn validate_and_sanitize_json_body(&self, request: &mut Request) -> Result<(), AppError> {
        // Extract body
        let body_bytes = std::mem::take(request.body_mut());
        let body = axum::body::to_bytes(body_bytes, self.config.max_body_size)
            .await
            .map_err(|e| AppError::ValidationError(format!("Failed to read request body: {}", e)))?;

        if body.is_empty() {
            return Ok(());
        }

        // Parse JSON
        let json_value: Value = serde_json::from_slice(&body)
            .map_err(|e| AppError::ValidationError(format!("Invalid JSON: {}", e)))?;

        // Validate JSON recursively
        let sanitized_json = self.validate_json_value(&json_value).await?;

        // Replace request body with sanitized version
        let sanitized_body = serde_json::to_vec(&sanitized_json)
            .map_err(|e| AppError::InternalServerError(format!("Failed to serialize JSON: {}", e)))?;

        *request.body_mut() = Body::from(sanitized_body);

        Ok(())
    }

    /// Recursively validate JSON values
    fn validate_json_value<'a>(&'a self, value: &'a Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, AppError>> + Send + 'a>> {
        Box::pin(async move {
        match value {
            Value::String(s) => {
                let rules = vec![
                    ValidationRule::Length { min: 0, max: 10000 },
                    ValidationRule::SqlInjection,
                    ValidationRule::XssProtection,
                    ValidationRule::CommandInjection,
                ];

                let result = self.sanitizer.validate_and_sanitize(s, &rules)?;

                if !result.is_valid && self.config.strict_mode {
                    return Err(AppError::ValidationError(format!(
                        "Invalid string value: {}",
                        result.violations
                            .iter()
                            .map(|v| v.description.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }

                Ok(Value::String(
                    result.sanitized_input.unwrap_or_else(|| s.clone())
                ))
            }
            Value::Array(arr) => {
                let mut sanitized_array = Vec::new();
                for item in arr {
                    sanitized_array.push(self.validate_json_value(item).await?);
                }
                Ok(Value::Array(sanitized_array))
            }
            Value::Object(obj) => {
                let mut sanitized_object = serde_json::Map::new();
                for (key, val) in obj {
                    // Validate object keys
                    let key_rules = vec![
                        ValidationRule::Length { min: 1, max: 100 },
                        ValidationRule::Alphanumeric,
                    ];
                    let key_result = self.sanitizer.validate_and_sanitize(key, &key_rules)?;
                    
                    if !key_result.is_valid && self.config.strict_mode {
                        return Err(AppError::ValidationError(format!(
                            "Invalid object key '{}': {}",
                            key,
                            key_result.violations
                                .iter()
                                .map(|v| v.description.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                    }

                    let sanitized_key = key_result.sanitized_input.unwrap_or_else(|| key.clone());
                    let sanitized_value = self.validate_json_value(val).await?;
                    sanitized_object.insert(sanitized_key, sanitized_value);
                }
                Ok(Value::Object(sanitized_object))
            }
            // Numbers, booleans, and null are safe as-is
            _ => Ok(value.clone()),
        }
        })
    }

    /// Check if header should be skipped during validation
    fn should_skip_header_validation(&self, header_name: &str) -> bool {
        matches!(
            header_name.to_lowercase().as_str(),
            "authorization" | "cookie" | "set-cookie" | "content-length" | "content-encoding"
        )
    }

    /// Create validation error response
    fn create_validation_error_response(&self, error: AppError) -> Response {
        let error_response = json!({
            "error": "Validation Error",
            "message": error.to_string(),
            "status": 400,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        (StatusCode::BAD_REQUEST, Json(error_response)).into_response()
    }
}

/// Helper function to create input validation middleware
pub async fn input_validation_middleware(
    request: Request,
    next: Next,
) -> Response {
    let config = ValidationMiddlewareConfig::default();
    let middleware = match InputValidationMiddleware::new(config) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to initialize input validation",
                    "message": e.to_string()
                }))
            ).into_response();
        }
    };
    
    match middleware.validate_request(request, next).await {
        Ok(response) => response,
        Err(e) => middleware.create_validation_error_response(e),
    }
}

// Make InputValidationMiddleware cloneable for use in middleware
impl Clone for InputValidationMiddleware {
    fn clone(&self) -> Self {
        Self {
            sanitizer: InputSanitizer::new().expect("Failed to create InputSanitizer"),
            config: self.config.clone(),
        }
    }
}