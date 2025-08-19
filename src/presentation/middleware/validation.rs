use crate::error::{AppError, Result};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::error;

/// Validation middleware for request data
pub async fn validation_middleware(
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Extract request information for validation
    let method = request.method().clone();
    let uri = request.uri().clone();

    // Log the request for debugging
    tracing::debug!("🔍 Validating request: {} {}", method, uri);

    // Perform basic request validation
    if let Err(validation_error) = validate_request(&request) {
        error!("❌ Request validation failed: {}", validation_error);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Continue with the request
    let response = next.run(request).await;

    // Log successful validation
    tracing::debug!("✅ Request validation passed: {} {}", method, uri);

    Ok(response)
}

/// Validate incoming request
fn validate_request(request: &Request) -> Result<()> {
    // Check request size limits
    if let Some(content_length) = request.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                const MAX_REQUEST_SIZE: usize = 50 * 1024 * 1024; // 50MB
                if length > MAX_REQUEST_SIZE {
                    return Err(AppError::ValidationError(format!(
                        "Request size {} exceeds maximum allowed size of {} bytes",
                        length, MAX_REQUEST_SIZE
                    )));
                }
            }
        }
    }

    // Check for required headers based on content type
    if let Some(content_type) = request.headers().get("content-type") {
        let content_type_str = content_type.to_str().unwrap_or("");

        if content_type_str.starts_with("application/json") {
            // Additional JSON validation could be added here
            tracing::debug!("📝 JSON content type detected");
        } else if content_type_str.starts_with("multipart/form-data") {
            // Additional multipart validation could be added here
            tracing::debug!("📁 Multipart content type detected");
        }
    }

    Ok(())
}

/// Validate JSON payload structure
pub fn validate_json_payload(payload: &Value, required_fields: &[&str]) -> Result<()> {
    if !payload.is_object() {
        return Err(AppError::ValidationError(
            "Request body must be a JSON object".to_string(),
        ));
    }

    let obj = payload.as_object().unwrap();
    let mut missing_fields = Vec::new();

    for field in required_fields {
        if !obj.contains_key(*field) {
            missing_fields.push(*field);
        }
    }

    if !missing_fields.is_empty() {
        return Err(AppError::ValidationError(format!(
            "Missing required fields: {}",
            missing_fields.join(", ")
        )));
    }

    Ok(())
}

/// Validate string field constraints
pub fn validate_string_field(
    value: &str,
    field_name: &str,
    min_length: Option<usize>,
    max_length: Option<usize>,
) -> Result<()> {
    if let Some(min) = min_length {
        if value.len() < min {
            return Err(AppError::ValidationError(format!(
                "{} must be at least {} characters long",
                field_name, min
            )));
        }
    }

    if let Some(max) = max_length {
        if value.len() > max {
            return Err(AppError::ValidationError(format!(
                "{} must be no more than {} characters long",
                field_name, max
            )));
        }
    }

    // Check for empty strings
    if value.trim().is_empty() {
        return Err(AppError::ValidationError(format!(
            "{} cannot be empty",
            field_name
        )));
    }

    Ok(())
}

/// Validate email format
pub fn validate_email(email: &str) -> Result<()> {
    let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .map_err(|e| AppError::InternalServerError(format!("Regex compilation failed: {}", e)))?;

    if !email_regex.is_match(email) {
        return Err(AppError::ValidationError(
            "Invalid email format".to_string(),
        ));
    }

    Ok(())
}

/// Validate URL format
pub fn validate_url(url: &str, field_name: &str) -> Result<()> {
    match url::Url::parse(url) {
        Ok(parsed_url) => {
            // Check for supported schemes
            match parsed_url.scheme() {
                "http" | "https" => Ok(()),
                _ => Err(AppError::ValidationError(format!(
                    "{} must use http or https protocol",
                    field_name
                ))),
            }
        }
        Err(_) => Err(AppError::ValidationError(format!(
            "{} is not a valid URL",
            field_name
        ))),
    }
}

/// Validate UUID format
pub fn validate_uuid(uuid_str: &str, field_name: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(uuid_str)
        .map_err(|_| AppError::ValidationError(format!("{} is not a valid UUID", field_name)))
}

/// Validate repository name format (owner/repo)
pub fn validate_repository_name(repo_name: &str) -> Result<()> {
    if !repo_name.contains('/') {
        return Err(AppError::ValidationError(
            "Repository name must be in format 'owner/repo'".to_string(),
        ));
    }

    let parts: Vec<&str> = repo_name.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::ValidationError(
            "Repository name must contain exactly one '/' separator".to_string(),
        ));
    }

    let (owner, repo) = (parts[0], parts[1]);

    // Validate owner and repo names
    validate_string_field(owner, "repository owner", Some(1), Some(39))?;
    validate_string_field(repo, "repository name", Some(1), Some(100))?;

    // Check for valid characters (GitHub naming rules)
    let valid_chars = regex::Regex::new(r"^[a-zA-Z0-9._-]+$")
        .map_err(|e| AppError::InternalServerError(format!("Regex compilation failed: {}", e)))?;

    if !valid_chars.is_match(owner) {
        return Err(AppError::ValidationError(
            "Repository owner contains invalid characters".to_string(),
        ));
    }

    if !valid_chars.is_match(repo) {
        return Err(AppError::ValidationError(
            "Repository name contains invalid characters".to_string(),
        ));
    }

    Ok(())
}

/// Enhanced validation framework
pub struct ValidationFramework {
    validators: HashMap<String, Box<dyn Validator>>,
    rules: HashMap<String, Vec<ValidationRule>>,
}

impl Default for ValidationFramework {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationFramework {
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            rules: HashMap::new(),
        }
    }

    pub fn add_validator(&mut self, name: String, validator: Box<dyn Validator>) {
        self.validators.insert(name, validator);
    }

    pub fn add_rule(&mut self, endpoint: String, rule: ValidationRule) {
        self.rules.entry(endpoint).or_default().push(rule);
    }

    pub async fn validate(
        &self,
        endpoint: &str,
        input: &dyn std::any::Any,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        // Apply endpoint-specific rules
        if let Some(rules) = self.rules.get(endpoint) {
            for rule in rules {
                match rule.validate(input).await {
                    Ok(()) => result.add_success(rule.name.clone()),
                    Err(e) => result.add_error(rule.name.clone(), e.to_string()),
                }
            }
        }

        // Apply registered validators
        for (name, validator) in &self.validators {
            match validator.validate(input).await {
                Ok(validation_result) => {
                    if validation_result.is_valid() {
                        result.add_success(name.clone());
                    } else {
                        for error in validation_result.errors {
                            result.add_error(name.clone(), error);
                        }
                    }
                }
                Err(e) => result.add_error(name.clone(), e.to_string()),
            }
        }

        Ok(result)
    }
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub successes: Vec<String>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            successes: Vec::new(),
        }
    }

    pub fn add_error(&mut self, field: String, error: String) {
        self.is_valid = false;
        self.errors.push(format!("{}: {}", field, error));
    }

    pub fn add_warning(&mut self, field: String, warning: String) {
        self.warnings.push(format!("{}: {}", field, warning));
    }

    pub fn add_success(&mut self, field: String) {
        self.successes.push(field);
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid && self.errors.is_empty()
    }
}

/// Validator trait
#[async_trait::async_trait]
pub trait Validator: Send + Sync {
    async fn validate(&self, input: &dyn std::any::Any) -> Result<ValidationResult>;
    fn validator_name(&self) -> &'static str;
}

/// Validation rule
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub name: String,
    pub rule_type: ValidationRuleType,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ValidationRuleType {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    Email,
    Url,
    Uuid,
    Custom(String), // Custom validation function name
}

impl ValidationRule {
    pub fn new(name: String, rule_type: ValidationRuleType, message: String) -> Self {
        Self {
            name,
            rule_type,
            message,
        }
    }

    pub async fn validate(&self, _input: &dyn std::any::Any) -> Result<()> {
        // This is a simplified implementation
        // In a real implementation, you would extract the field value from input
        // and apply the validation rule
        match &self.rule_type {
            ValidationRuleType::Required => {
                // Check if field exists and is not empty
                Ok(())
            }
            ValidationRuleType::MinLength(_min) => {
                // Check minimum length
                Ok(())
            }
            ValidationRuleType::MaxLength(_max) => {
                // Check maximum length
                Ok(())
            }
            ValidationRuleType::Pattern(_pattern) => {
                // Check regex pattern
                Ok(())
            }
            ValidationRuleType::Email => {
                // Validate email format
                Ok(())
            }
            ValidationRuleType::Url => {
                // Validate URL format
                Ok(())
            }
            ValidationRuleType::Uuid => {
                // Validate UUID format
                Ok(())
            }
            ValidationRuleType::Custom(_) => {
                // Apply custom validation
                Ok(())
            }
        }
    }
}

/// JSON schema validator
pub struct JsonSchemaValidator {
    schema: serde_json::Value,
}

impl JsonSchemaValidator {
    pub fn new(schema: serde_json::Value) -> Self {
        Self { schema }
    }
}

#[async_trait::async_trait]
impl Validator for JsonSchemaValidator {
    async fn validate(&self, _input: &dyn std::any::Any) -> Result<ValidationResult> {
        // In a real implementation, you would use a JSON schema validation library
        // like jsonschema-rs to validate the input against the schema
        let mut result = ValidationResult::new();
        result.add_success("json_schema".to_string());
        Ok(result)
    }

    fn validator_name(&self) -> &'static str {
        "json_schema"
    }
}

/// Pipeline configuration validator
pub struct PipelineConfigValidator;

#[async_trait::async_trait]
impl Validator for PipelineConfigValidator {
    async fn validate(&self, _input: &dyn std::any::Any) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        // In a real implementation, you would:
        // 1. Cast input to pipeline configuration type
        // 2. Validate pipeline structure
        // 3. Check for required fields
        // 4. Validate step configurations
        // 5. Check for circular dependencies

        result.add_success("pipeline_config".to_string());
        Ok(result)
    }

    fn validator_name(&self) -> &'static str {
        "pipeline_config"
    }
}

/// Security validation for sensitive operations
pub struct SecurityValidator {
    required_permissions: Vec<crate::core::networking::security::Permission>,
}

impl SecurityValidator {
    pub fn new(required_permissions: Vec<crate::core::networking::security::Permission>) -> Self {
        Self {
            required_permissions,
        }
    }
}

#[async_trait::async_trait]
impl Validator for SecurityValidator {
    async fn validate(&self, _input: &dyn std::any::Any) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        // In a real implementation, you would:
        // 1. Extract security context from input
        // 2. Check user permissions
        // 3. Validate operation is allowed
        // 4. Check rate limits
        // 5. Audit the validation attempt

        result.add_success("security".to_string());
        Ok(result)
    }

    fn validator_name(&self) -> &'static str {
        "security"
    }
}

/// Enhanced error response with suggestions
pub fn create_validation_error_response(error: &AppError) -> serde_json::Value {
    let mut suggestions = Vec::new();

    match error {
        AppError::ValidationError(msg) => {
            // Add specific suggestions based on error message
            if msg.contains("email") {
                suggestions.push("Ensure email follows format: user@domain.com".to_string());
            }
            if msg.contains("URL") {
                suggestions.push("Ensure URL includes protocol (http:// or https://)".to_string());
            }
            if msg.contains("UUID") {
                suggestions.push(
                    "Ensure UUID follows format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".to_string(),
                );
            }
            if msg.contains("repository") {
                suggestions.push("Repository name should be in format 'owner/repo'".to_string());
            }
            if msg.contains("required fields") {
                suggestions.push("Check API documentation for required request fields".to_string());
            }
        }
        AppError::FileSizeExceeded { limit, .. } => {
            suggestions.push(format!("Reduce file size to under {} bytes", limit));
            suggestions.push("Consider compressing the file before upload".to_string());
        }
        AppError::UnsupportedFileType(_) => {
            suggestions.push("Supported file types: .yaml, .yml".to_string());
            suggestions.push("Ensure file has correct extension".to_string());
        }
        _ => {
            suggestions.push("Check the API documentation for correct usage".to_string());
        }
    }

    serde_json::json!({
        "error": format!("{}", error),
        "suggestions": suggestions,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "documentation": "https://github.com/RitabrataRoychowdhury/RustCI/blob/main/API_DOCUMENTATION.md"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_string_field() {
        // Valid string
        assert!(validate_string_field("test", "field", Some(1), Some(10)).is_ok());

        // Too short
        assert!(validate_string_field("", "field", Some(1), None).is_err());

        // Too long
        assert!(validate_string_field("very long string", "field", None, Some(5)).is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("invalid-email").is_err());
        assert!(validate_email("@example.com").is_err());
    }

    #[test]
    fn test_validate_repository_name() {
        assert!(validate_repository_name("owner/repo").is_ok());
        assert!(validate_repository_name("invalid-repo").is_err());
        assert!(validate_repository_name("owner/repo/extra").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com", "url").is_ok());
        assert!(validate_url("http://example.com", "url").is_ok());
        assert!(validate_url("ftp://example.com", "url").is_err());
        assert!(validate_url("not-a-url", "url").is_err());
    }
}
