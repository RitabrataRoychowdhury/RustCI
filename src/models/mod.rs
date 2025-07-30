// This file declares all model modules and re-exports their contents
// This allows other parts of the code to use `use crate::models::User`
// instead of `use crate::models::user::User`

pub mod dockerfile;
pub mod github;
pub mod user;
pub mod workspace;

// Re-export all public items from the modules
// This makes them available at the models module level
// pub use dockerfile::*; // Unused import
pub use github::*;
pub use user::*;
pub use workspace::*;

// Additional models for integration testing
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerfileGenerationResult {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub repository_id: i64,
    pub dockerfile_content: String,
    pub validation_result: Option<ValidationResult>,
    pub status: GenerationStatus,
    pub created_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenerationStatus {
    Generated,
    Validated,
    ValidationFailed,
    Approved,
    PrCreated,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub success: bool,
    pub build_logs: Vec<String>,
    pub run_logs: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn success(build_logs: Vec<String>, run_logs: Vec<String>) -> Self {
        Self {
            success: true,
            build_logs,
            run_logs,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
    
    #[allow(dead_code)]
    pub fn failure(errors: Vec<String>) -> Self {
        Self {
            success: false,
            build_logs: Vec::new(),
            run_logs: Vec::new(),
            errors,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub binary_name: String,
    pub port: u16,
    pub dependencies: Vec<String>,
    pub build_command: Option<String>,
    pub run_command: Option<String>,
}