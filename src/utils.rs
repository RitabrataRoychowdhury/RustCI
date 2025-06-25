use std::env;

/// Helper function to get environment variable with a default value
pub fn get_env_or_default(key: &str, default_value: &str) -> String {
    env::var(key).unwrap_or_else(|_| default_value.to_string())
}

/// Helper function to validate required environment variables
pub fn validate_required_env_vars(vars: &[&str]) -> Result<(), String> {
    let missing_vars: Vec<&str> = vars
        .iter()
        .filter(|&&var| env::var(var).is_err())
        .copied()
        .collect();

    if !missing_vars.is_empty() {
        return Err(format!(
            "Missing required environment variables: {}",
            missing_vars.join(", ")
        ));
    }

    Ok(())
}

/// Generate a secure random string for OAuth state
pub fn generate_secure_state() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Helper to extract bearer token from Authorization header
pub fn extract_bearer_token(auth_header: &str) -> Option<String> {
    if auth_header.starts_with("Bearer ") {
        Some(auth_header[7..].to_string())
    } else {
        None
    }
}