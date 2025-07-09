use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub mongodb_uri: String,
    pub mongodb_database: String,
    pub jwt_secret: String,
    pub jwt_expires_in: String,
    pub jwt_maxage: i32,
    
    pub github_oauth_client_id: String,
    pub github_oauth_client_secret: String,
    pub github_oauth_redirect_url: String,
    
    pub client_origin: String,
    pub port: u16,
}

impl Config {
    pub fn init() -> Config {
        let mongodb_uri = std::env::var("MONGODB_URI")
            .expect("MONGODB_URI must be set");
        let mongodb_database = std::env::var("MONGODB_DATABASE")
            .expect("MONGODB_DATABASE must be set");
        
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set");
        let jwt_expires_in = std::env::var("JWT_EXPIRED_IN")
            .unwrap_or_else(|_| "60m".to_owned());
        let jwt_maxage = std::env::var("JWT_MAXAGE")
            .unwrap_or_else(|_| "60".to_owned())
            .parse::<i32>()
            .expect("JWT_MAXAGE must be a number");

        let github_oauth_client_id = std::env::var("GITHUB_OAUTH_CLIENT_ID")
            .expect("GITHUB_OAUTH_CLIENT_ID must be set");
        let github_oauth_client_secret = std::env::var("GITHUB_OAUTH_CLIENT_SECRET")
            .expect("GITHUB_OAUTH_CLIENT_SECRET must be set");
        let github_oauth_redirect_url = std::env::var("GITHUB_OAUTH_REDIRECT_URL")
            .expect("GITHUB_OAUTH_REDIRECT_URL must be set");

        let client_origin = std::env::var("CLIENT_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".to_owned());
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8000".to_owned())
            .parse::<u16>()
            .expect("PORT must be a number");

        Config {
            mongodb_uri,
            mongodb_database,
            jwt_secret,
            jwt_expires_in,
            jwt_maxage,
            github_oauth_client_id,
            github_oauth_client_secret,
            github_oauth_redirect_url,
            client_origin,
            port,
        }
    }
}