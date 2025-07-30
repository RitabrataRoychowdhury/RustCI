use crate::{
    config::AppConfiguration,
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use serde_json::Value;
use tracing::{debug, warn};

/// Enhanced request validation middleware
pub struct EnhancedValidationMiddleware;

impl EnhancedValidationMiddleware {
    /// Main validation middleware function
    pub async fn validate_request(
        State(state): State<AppState>,
        req: Request,
        next: Next,
    ) -> Result<Response> {
        let method = req.method().clone();
        let uri = req.uri().clone();
        let path = uri.path();

        debug!(
            method = %method,
            path = path,
            "🔍 Starting enhanced request validation"
        );

        // 1. Validate HTTP method
        Self::validate_http_method(&method)?;

        // 2. Validate URL structure
        Self::validate_url_structure(&uri)?;

        // 3. Validate headers
        Self::validate_headers(req.headers())?;

        // 4. Validate query parameters
        if let Some(query) = uri.query() {
            Self::validate_query_parameters(query)?;
        }

        // 5. Content-specific validation for POST/PUT requests
        if matches!(method, axum::http::Method::POST | axum::http::Method::PUT) {
            Self::validate_content_headers(req.headers())?;
        }

        // 6. Path-specific validation
        Self::validate_path_specific(&state.env, path, &method)?;

        let response = next.run(req).await;

        debug!(
            method = %method,
            path = path,
            status = response.status().as_u16(),
            "✅ Enhanced request validation completed"
        );

        Ok(response)
    }

    /// Validate HTTP method
    fn validate_http_method(method: &axum::http::Method) -> Result<()> {
        let allowed_methods = [
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::PATCH,
            axum::http::Method::OPTIONS,
            axum::http::Method::HEAD,
        ];

        if !allowed_methods.contains(method) {
            return Err(AppError::ValidationError(format!(
                "HTTP method {} not allowed",
                method
            )));
        }

        Ok(())
    }

    /// Validate URL structure for security threats
    fn validate_url_structure(uri: &axum::http::Uri) -> Result<()> {
        let path = uri.path();

        // Check for directory traversal attacks
        if path.contains("..") || path.contains("//") {
            return Err(AppError::ValidationError(
                "Invalid path: directory traversal detected".to_string(),
            ));
        }

        // Check for null bytes
        if path.contains('\0') {
            return Err(AppError::ValidationError(
                "Invalid path: null byte detected".to_string(),
            ));
        }

        // Check path length
        if path.len() > 2048 {
            return Err(AppError::ValidationError("Path too long".to_string()));
        }

        // Check for suspicious patterns
        let suspicious_patterns = [
            "/.env",
            "/.git",
            "/proc/",
            "/etc/",
            "/var/log/",
            "wp-admin",
            "wp-content",
            "phpmyadmin",
        ];

        for pattern in &suspicious_patterns {
            if path.to_lowercase().contains(pattern) {
                warn!(
                    path = path,
                    pattern = pattern,
                    "🚨 Suspicious path pattern detected"
                );
                return Err(AppError::ValidationError(format!(
                    "Suspicious path pattern: {}",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Validate HTTP headers for security
    fn validate_headers(headers: &axum::http::HeaderMap) -> Result<()> {
        // Check for header injection attacks
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                // Check for CRLF injection
                if value_str.contains('\n') || value_str.contains('\r') {
                    return Err(AppError::ValidationError(format!(
                        "Invalid characters in header {}",
                        name
                    )));
                }

                // Check header value length
                if value_str.len() > 8192 {
                    return Err(AppError::ValidationError(format!(
                        "Header {} value too long",
                        name
                    )));
                }
            }
        }

        // Validate specific headers
        if let Some(host) = headers.get("host") {
            if let Ok(host_str) = host.to_str() {
                if host_str.contains(' ') || host_str.contains('\t') {
                    return Err(AppError::ValidationError("Invalid Host header".to_string()));
                }
            }
        }

        // Check for suspicious user agents
        if let Some(user_agent) = headers.get("user-agent") {
            if let Ok(ua_str) = user_agent.to_str() {
                let suspicious_ua_patterns = [
                    "sqlmap", "nikto", "nmap", "masscan", "zap", "burp", "w3af", "acunetix",
                    "nessus",
                ];

                for pattern in &suspicious_ua_patterns {
                    if ua_str.to_lowercase().contains(pattern) {
                        warn!(user_agent = ua_str, "🚨 Suspicious user agent detected");
                        return Err(AppError::ValidationError(
                            "Suspicious user agent detected".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate query parameters
    fn validate_query_parameters(query: &str) -> Result<()> {
        // Check query string length
        if query.len() > 4096 {
            return Err(AppError::ValidationError(
                "Query string too long".to_string(),
            ));
        }

        // Check for SQL injection patterns
        let sql_patterns = [
            "union", "select", "insert", "update", "delete", "drop", "create", "alter", "exec",
            "execute", "sp_", "xp_", "/*", "*/", "--", ";--", "@@", "char(",
        ];

        let query_lower = query.to_lowercase();
        for pattern in &sql_patterns {
            if query_lower.contains(pattern) {
                warn!(
                    query = query,
                    pattern = pattern,
                    "🚨 Potential SQL injection detected"
                );
                // Don't block immediately, just log - could be legitimate content
            }
        }

        // Check for XSS patterns
        let xss_patterns = [
            "<script",
            "</script>",
            "javascript:",
            "vbscript:",
            "onload=",
            "onerror=",
            "onclick=",
            "onmouseover=",
            "eval(",
            "alert(",
            "confirm(",
            "prompt(",
        ];

        for pattern in &xss_patterns {
            if query_lower.contains(pattern) {
                warn!(
                    query = query,
                    pattern = pattern,
                    "🚨 Potential XSS detected"
                );
                return Err(AppError::ValidationError(
                    "Potentially malicious query parameter detected".to_string(),
                ));
            }
        }

        // Validate URL encoding
        if query.contains('%') {
            // Basic URL decode validation
            let mut i = 0;
            let chars: Vec<char> = query.chars().collect();
            while i < chars.len() {
                if chars[i] == '%' {
                    if i + 2 >= chars.len() {
                        return Err(AppError::ValidationError(
                            "Invalid URL encoding".to_string(),
                        ));
                    }

                    let hex_chars = &chars[i + 1..i + 3];
                    if !hex_chars.iter().all(|c| c.is_ascii_hexdigit()) {
                        return Err(AppError::ValidationError(
                            "Invalid URL encoding".to_string(),
                        ));
                    }
                    i += 3;
                } else {
                    i += 1;
                }
            }
        }

        Ok(())
    }

    /// Validate content-related headers for POST/PUT requests
    fn validate_content_headers(headers: &axum::http::HeaderMap) -> Result<()> {
        // Validate Content-Type
        if let Some(content_type) = headers.get("content-type") {
            if let Ok(ct_str) = content_type.to_str() {
                let allowed_content_types = [
                    "application/json",
                    "application/x-www-form-urlencoded",
                    "multipart/form-data",
                    "text/plain",
                    "application/yaml",
                    "text/yaml",
                ];

                if !allowed_content_types
                    .iter()
                    .any(|&allowed| ct_str.starts_with(allowed))
                {
                    return Err(AppError::ValidationError(format!(
                        "Content type '{}' not allowed",
                        ct_str
                    )));
                }
            }
        }

        // Validate Content-Length
        if let Some(content_length) = headers.get("content-length") {
            if let Ok(length_str) = content_length.to_str() {
                if let Ok(length) = length_str.parse::<usize>() {
                    const MAX_CONTENT_LENGTH: usize = 100 * 1024 * 1024; // 100MB
                    if length > MAX_CONTENT_LENGTH {
                        return Err(AppError::ValidationError(format!(
                            "Content length {} exceeds maximum allowed size",
                            length
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Path-specific validation based on endpoint
    fn validate_path_specific(
        _config: &AppConfiguration,
        path: &str,
        method: &axum::http::Method,
    ) -> Result<()> {
        // API endpoint validation
        if path.starts_with("/api/") {
            // Check API version if required
            if !path.starts_with("/api/healthchecker")
                && !path.starts_with("/api/sessions")
                && !path.starts_with("/api/ci")
                && !path.starts_with("/api/pr")
            {
                return Err(AppError::ValidationError(
                    "Unknown API endpoint".to_string(),
                ));
            }
        }

        // File upload validation
        if path.contains("/upload")
            && !matches!(*method, axum::http::Method::POST | axum::http::Method::PUT)
        {
            return Err(AppError::ValidationError(
                "Invalid method for upload endpoint".to_string(),
            ));
        }

        // Admin endpoint validation
        if path.contains("/admin") {
            // In production, admin endpoints should be more restricted
            if std::env::var("ENVIRONMENT").unwrap_or_default() == "production" {
                warn!(path = path, "🚨 Admin endpoint accessed in production");
            }
        }

        // OAuth callback validation
        if path.contains("/oauth")
            && path.contains("/callback")
            && !matches!(*method, axum::http::Method::GET | axum::http::Method::POST)
        {
            return Err(AppError::ValidationError(
                "Invalid method for OAuth callback".to_string(),
            ));
        }

        Ok(())
    }
}

/// JSON payload validation
pub struct JsonPayloadValidator;

impl JsonPayloadValidator {
    /// Validate JSON payload structure and content
    pub fn validate_json_payload(payload: &Value, schema_name: &str) -> Result<()> {
        // Basic JSON structure validation
        Self::validate_json_structure(payload)?;

        // Schema-specific validation
        match schema_name {
            "pipeline" => Self::validate_pipeline_payload(payload),
            "user" => Self::validate_user_payload(payload),
            "config" => Self::validate_config_payload(payload),
            _ => Ok(()),
        }
    }

    fn validate_json_structure(payload: &Value) -> Result<()> {
        // Check JSON depth to prevent deeply nested attacks
        fn check_depth(value: &Value, current_depth: usize, max_depth: usize) -> Result<()> {
            if current_depth > max_depth {
                return Err(AppError::ValidationError(
                    "JSON payload too deeply nested".to_string(),
                ));
            }

            match value {
                Value::Object(obj) => {
                    for (_, v) in obj {
                        check_depth(v, current_depth + 1, max_depth)?;
                    }
                }
                Value::Array(arr) => {
                    for v in arr {
                        check_depth(v, current_depth + 1, max_depth)?;
                    }
                }
                _ => {}
            }

            Ok(())
        }

        check_depth(payload, 0, 20)?; // Max depth of 20

        // Check for excessively large arrays
        fn check_array_sizes(value: &Value) -> Result<()> {
            match value {
                Value::Array(arr) => {
                    if arr.len() > 1000 {
                        return Err(AppError::ValidationError("Array too large".to_string()));
                    }
                    for v in arr {
                        check_array_sizes(v)?;
                    }
                }
                Value::Object(obj) => {
                    if obj.len() > 100 {
                        return Err(AppError::ValidationError(
                            "Object has too many properties".to_string(),
                        ));
                    }
                    for (_, v) in obj {
                        check_array_sizes(v)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }

        check_array_sizes(payload)?;

        Ok(())
    }

    fn validate_pipeline_payload(payload: &Value) -> Result<()> {
        if let Value::Object(obj) = payload {
            // Required fields
            let required_fields = ["name", "steps"];
            for field in &required_fields {
                if !obj.contains_key(*field) {
                    return Err(AppError::ValidationError(format!(
                        "Missing required field: {}",
                        field
                    )));
                }
            }

            // Validate name
            if let Some(Value::String(name)) = obj.get("name") {
                if name.is_empty() || name.len() > 100 {
                    return Err(AppError::ValidationError(
                        "Pipeline name must be 1-100 characters".to_string(),
                    ));
                }
            }

            // Validate steps
            if let Some(Value::Array(steps)) = obj.get("steps") {
                if steps.is_empty() {
                    return Err(AppError::ValidationError(
                        "Pipeline must have at least one step".to_string(),
                    ));
                }
                if steps.len() > 50 {
                    return Err(AppError::ValidationError(
                        "Pipeline cannot have more than 50 steps".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_user_payload(payload: &Value) -> Result<()> {
        if let Value::Object(obj) = payload {
            // Validate email if present
            if let Some(Value::String(email)) = obj.get("email") {
                if !email.contains('@') || email.len() > 254 {
                    return Err(AppError::ValidationError(
                        "Invalid email format".to_string(),
                    ));
                }
            }

            // Validate username if present
            if let Some(Value::String(username)) = obj.get("username") {
                if username.len() < 3 || username.len() > 50 {
                    return Err(AppError::ValidationError(
                        "Username must be 3-50 characters".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_config_payload(payload: &Value) -> Result<()> {
        if let Value::Object(obj) = payload {
            // Validate configuration structure
            let allowed_sections = [
                "server",
                "database",
                "security",
                "observability",
                "ci",
                "external_services",
                "features",
            ];

            for key in obj.keys() {
                if !allowed_sections.contains(&key.as_str()) {
                    return Err(AppError::ValidationError(format!(
                        "Unknown configuration section: {}",
                        key
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_http_method() {
        assert!(
            EnhancedValidationMiddleware::validate_http_method(&axum::http::Method::GET).is_ok()
        );
        assert!(
            EnhancedValidationMiddleware::validate_http_method(&axum::http::Method::POST).is_ok()
        );
    }

    #[test]
    fn test_validate_url_structure() {
        let valid_uri: axum::http::Uri = "/api/test".parse().unwrap();
        assert!(EnhancedValidationMiddleware::validate_url_structure(&valid_uri).is_ok());

        let invalid_uri: axum::http::Uri = "/api/../etc/passwd".parse().unwrap();
        assert!(EnhancedValidationMiddleware::validate_url_structure(&invalid_uri).is_err());
    }

    #[test]
    fn test_validate_query_parameters() {
        assert!(EnhancedValidationMiddleware::validate_query_parameters("param=value").is_ok());
        assert!(EnhancedValidationMiddleware::validate_query_parameters(
            "param=<script>alert(1)</script>"
        )
        .is_err());
    }

    #[test]
    fn test_json_payload_validation() {
        let valid_pipeline = json!({
            "name": "test-pipeline",
            "steps": [{"name": "build", "command": "cargo build"}]
        });
        assert!(JsonPayloadValidator::validate_json_payload(&valid_pipeline, "pipeline").is_ok());

        let invalid_pipeline = json!({
            "steps": []
        });
        assert!(
            JsonPayloadValidator::validate_json_payload(&invalid_pipeline, "pipeline").is_err()
        );
    }
}
