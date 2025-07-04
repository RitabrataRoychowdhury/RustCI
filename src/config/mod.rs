use dotenv::dotenv;
use std::env;
use tracing::{info, debug, warn};

#[derive(Debug, Clone)]
pub struct Config {
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_uri: String,
    pub server_url: String,
    pub server_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file if it exists
        match dotenv() {
            Ok(_) => info!("✅ Loaded .env file"),
            Err(_) => debug!("No .env file found, using system environment variables"),
        }
        
        // Validate required environment variables
        let required_vars = ["GITHUB_CLIENT_ID", "GITHUB_CLIENT_SECRET"];
        let missing_vars: Vec<&str> = required_vars
            .iter()
            .filter(|&&var| env::var(var).is_err())
            .copied()
            .collect();

        if !missing_vars.is_empty() {
            return Err(format!(
                "Missing required environment variables: {}. Please check your .env file or environment.",
                missing_vars.join(", ")
            ).into());
        }

        let github_client_id = env::var("GITHUB_CLIENT_ID")
            .map_err(|_| "GITHUB_CLIENT_ID must be set")?;
        
        let github_client_secret = env::var("GITHUB_CLIENT_SECRET")
            .map_err(|_| "GITHUB_CLIENT_SECRET must be set")?;

        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .map_err(|_| "SERVER_PORT must be a valid number")?;

        let server_url = env::var("SERVER_URL")
            .unwrap_or_else(|_| format!("http://localhost:{}", server_port));

        let github_redirect_uri = env::var("GITHUB_REDIRECT_URI")
            .unwrap_or_else(|_| format!("{}/auth/github/callback", server_url));

        // Log configuration (without secrets)
        info!("📋 Configuration loaded:");
        info!("  - GitHub Client ID: {}...", &github_client_id[..8]);
        info!("  - Server URL: {}", server_url);
        info!("  - Server Port: {}", server_port);
        info!("  - Redirect URI: {}", github_redirect_uri);

        // Validate GitHub client ID format
        if github_client_id.len() < 20 {
            warn!("⚠️  GitHub Client ID seems too short. Make sure it's correct.");
        }

        if github_client_secret.len() < 40 {
            warn!("⚠️  GitHub Client Secret seems too short. Make sure it's correct.");
        }
        
        Ok(Config {
            github_client_id,
            github_client_secret,
            github_redirect_uri,
            server_url,
            server_port,
        })
    }

    /// Validate that all required configuration is present and valid
    pub fn validate(&self) -> Result<(), String> {
        if self.github_client_id.is_empty() {
            return Err("GitHub Client ID cannot be empty".to_string());
        }

        if self.github_client_secret.is_empty() {
            return Err("GitHub Client Secret cannot be empty".to_string());
        }

        if self.server_port == 0 {
            return Err("Server port must be greater than 0".to_string());
        }

        if !self.github_redirect_uri.starts_with("http") {
            return Err("GitHub redirect URI must be a valid HTTP(S) URL".to_string());
        }

        Ok(())
    }
}