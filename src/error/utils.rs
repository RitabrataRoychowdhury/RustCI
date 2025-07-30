use crate::error::AppError;
use uuid::Uuid;

/// Common error handling utilities
pub struct ErrorUtils;

impl ErrorUtils {
    /// Create a validation error for missing required field
    pub fn missing_field_error(field_name: &str) -> AppError {
        AppError::ValidationError(format!("{} is required", field_name))
    }

    /// Create a validation error for invalid format
    pub fn invalid_format_error(field_name: &str) -> AppError {
        AppError::ValidationError(format!("Invalid {} format", field_name))
    }

    /// Create a not found error with context
    pub fn not_found_error(resource: &str, id: &str) -> AppError {
        AppError::NotFound(format!("{} {} not found", resource, id))
    }

    /// Create a workspace not found error
    pub fn workspace_not_found() -> AppError {
        AppError::WorkspaceNotFound
    }

    /// Create a repository not found error
    pub fn repository_not_found(message: &str) -> AppError {
        AppError::RepositoryNotFound(message.to_string())
    }

    /// Create an authentication error
    pub fn auth_error(message: &str) -> AppError {
        AppError::AuthError(message.to_string())
    }

    /// Create a GitHub API error
    pub fn github_api_error(message: &str) -> AppError {
        AppError::GitHubApiError(message.to_string())
    }

    /// Parse UUID with proper error handling
    pub fn parse_uuid(uuid_str: &str, field_name: &str) -> Result<Uuid, AppError> {
        Uuid::parse_str(uuid_str)
            .map_err(|_| Self::invalid_format_error(&format!("{} ID", field_name)))
    }

    /// Validate non-empty string
    pub fn validate_non_empty(value: &str, field_name: &str) -> Result<(), AppError> {
        if value.trim().is_empty() {
            Err(Self::missing_field_error(field_name))
        } else {
            Ok(())
        }
    }

    /// Create a validation error for already existing resource
    pub fn already_exists_error(resource: &str) -> AppError {
        AppError::ValidationError(format!("{} already exists", resource))
    }

    /// Create an encryption error
    pub fn encryption_error(message: &str) -> AppError {
        AppError::EncryptionError(message.to_string())
    }

    /// Create a dockerfile generation error
    pub fn dockerfile_generation_error(message: &str) -> AppError {
        AppError::DockerfileGenerationFailed(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_field_error() {
        let error = ErrorUtils::missing_field_error("username");
        match error {
            AppError::ValidationError(msg) => assert_eq!(msg, "username is required"),
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_invalid_format_error() {
        let error = ErrorUtils::invalid_format_error("email");
        match error {
            AppError::ValidationError(msg) => assert_eq!(msg, "Invalid email format"),
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_parse_uuid_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = ErrorUtils::parse_uuid(uuid_str, "user");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let uuid_str = "invalid-uuid";
        let result = ErrorUtils::parse_uuid(uuid_str, "user");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ValidationError(msg) => assert_eq!(msg, "Invalid user ID format"),
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_non_empty_valid() {
        let result = ErrorUtils::validate_non_empty("test", "name");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_non_empty_invalid() {
        let result = ErrorUtils::validate_non_empty("  ", "name");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ValidationError(msg) => assert_eq!(msg, "name is required"),
            _ => panic!("Expected ValidationError"),
        }
    }
}
