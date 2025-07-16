use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub mongo_id: Option<ObjectId>,
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_user_id: i64,
    pub github_username: String,
    pub organization: Option<String>,
    pub encrypted_github_token: String,
    pub repositories: Vec<RepositoryMetadata>,
    pub shared_secrets: HashMap<String, String>, // Encrypted values
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Workspace {
    pub fn new(
        user_id: Uuid,
        github_user_id: i64,
        github_username: String,
        encrypted_github_token: String,
    ) -> Self {
        Self {
            mongo_id: None,
            id: Uuid::new_v4(),
            user_id,
            github_user_id,
            github_username,
            organization: None,
            encrypted_github_token,
            repositories: Vec::new(),
            shared_secrets: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadata {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
    pub has_dockerfile: bool,
    pub project_type: Option<ProjectType>,
    pub linked_at: DateTime<Utc>,
    pub last_dockerfile_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Java,
    Go,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceUpdate {
    pub organization: Option<String>,
    pub shared_secrets: Option<HashMap<String, String>>,
    pub updated_at: DateTime<Utc>,
}

impl WorkspaceUpdate {
    pub fn new() -> Self {
        Self {
            organization: None,
            shared_secrets: None,
            updated_at: Utc::now(),
        }
    }
}