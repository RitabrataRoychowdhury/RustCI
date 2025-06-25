use dotenv::dotenv;
use std::env;

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
        dotenv().ok(); // Load .env file if it exists
        
        Ok(Config {
            github_client_id: env::var("GITHUB_CLIENT_ID")
                .map_err(|_| "GITHUB_CLIENT_ID must be set")?,
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .map_err(|_| "GITHUB_CLIENT_SECRET must be set")?,
            github_redirect_uri: env::var("GITHUB_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:3000/auth/github/callback".to_string()),
            server_url: env::var("SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| "SERVER_PORT must be a valid number")?,
        })
    }
}