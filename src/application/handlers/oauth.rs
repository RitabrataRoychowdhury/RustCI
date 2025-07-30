use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Re-export auth functions for OAuth endpoints
// Removed unused re-exports - these can be imported directly from auth module when needed

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthConfig {
    pub provider: String,
    pub client_id: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
}
