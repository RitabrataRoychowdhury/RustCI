use crate::error::{AppError, Result};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde_json::Value;
use tracing::error;

/// Validation middleware for request data
#[allow(dead_code)]
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
#[allow(dead_code)]
fn validate_request(request: &Request) -> Result<()> {
    // Check request size limits
    if let Some(content_length) = request.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                const MAX_REQUEST_SIZE: usize = 50 * 1024 * 1024; // 50MB
                if length > MAX_REQUEST_SIZE {
                    return Err(AppError::ValidationError(
                        format!("Request size {} exceeds maximum allowed size of {} bytes", 
                               length, MAX_REQUEST_SIZE)
                    ));
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
#[allow(dead_code)]
pub fn validate_json_payload(payload: &Value, required_fields: &[&str]) -> Result<()> {
    if !payload.is_object() {
        return Err(AppError::ValidationError(
            "Request body must be a JSON object".to_string()
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
        return Err(AppError::ValidationError(
            format!("Missing required fields: {}", missing_fields.join(", "))
        ));
    }
    
    Ok(())
}

/// Validate string field constraints
pub fn validate_string_field(
    value: &str, 
    field_name: &str, 
    min_length: Option<usize>, 
    max_length: Option<usize>
) -> Result<()> {
    if let Some(min) = min_length {
        if value.len() < min {
            return Err(AppError::ValidationError(
                format!("{} must be at least {} characters long", field_name, min)
            ));
        }
    }
    
    if let Some(max) = max_length {
        if value.len() > max {
            return Err(AppError::ValidationError(
                format!("{} must be no more than {} characters long", field_name, max)
            ));
        }
    }
    
    // Check for empty strings
    if value.trim().is_empty() {
        return Err(AppError::ValidationError(
            format!("{} cannot be empty", field_name)
        ));
    }
    
    Ok(())
}

/// Validate email format
#[allow(dead_code)]
pub fn validate_email(email: &str) -> Result<()> {
    let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .map_err(|e| AppError::InternalServerError(format!("Regex compilation failed: {}", e)))?;
    
    if !email_regex.is_match(email) {
        return Err(AppError::ValidationError(
            "Invalid email format".to_string()
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
                _ => Err(AppError::ValidationError(
                    format!("{} must use http or https protocol", field_name)
                ))
            }
        }
        Err(_) => Err(AppError::ValidationError(
            format!("{} is not a valid URL", field_name)
        ))
    }
}

/// Validate UUID format
#[allow(dead_code)]
pub fn validate_uuid(uuid_str: &str, field_name: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(uuid_str)
        .map_err(|_| AppError::ValidationError(
            format!("{} is not a valid UUID", field_name)
        ))
}

/// Validate repository name format (owner/repo)
pub fn validate_repository_name(repo_name: &str) -> Result<()> {
    if !repo_name.contains('/') {
        return Err(AppError::ValidationError(
            "Repository name must be in format 'owner/repo'".to_string()
        ));
    }
    
    let parts: Vec<&str> = repo_name.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::ValidationError(
            "Repository name must contain exactly one '/' separator".to_string()
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
            "Repository owner contains invalid characters".to_string()
        ));
    }
    
    if !valid_chars.is_match(repo) {
        return Err(AppError::ValidationError(
            "Repository name contains invalid characters".to_string()
        ));
    }
    
    Ok(())
}

/// Enhanced error response with suggestions
#[allow(dead_code)]
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
                suggestions.push("Ensure UUID follows format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".to_string());
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