use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
    pub access_token: String,
}

impl User {
    pub fn new(
        id: u64,
        login: String,
        name: Option<String>,
        email: Option<String>,
        avatar_url: String,
        html_url: String,
        access_token: String,
    ) -> Self {
        Self {
            id,
            login,
            name,
            email,
            avatar_url,
            html_url,
            access_token,
        }
    }

    pub fn has_repo_access(&self) -> bool {
        // For now, assume all authenticated users have basic repo access
        // Later, we can implement more granular permissions
        !self.access_token.is_empty()
    }
}