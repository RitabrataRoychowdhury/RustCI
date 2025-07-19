use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::workspace::ProjectType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerfileGenerationRequest {
    pub workspace_id: Uuid,
    pub repository_id: i64,
    pub project_type: ProjectType,
    pub custom_options: HashMap<String, String>,
}

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

impl DockerfileGenerationResult {
    #[allow(dead_code)]
    pub fn new(
        workspace_id: Uuid,
        repository_id: i64,
        dockerfile_content: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            repository_id,
            dockerfile_content,
            validation_result: None,
            status: GenerationStatus::Generated,
            created_at: Utc::now(),
            approved_at: None,
            pr_url: None,
        }
    }
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
    #[allow(dead_code)]
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
    pub additional_dependencies: Vec<String>,
    pub build_args: HashMap<String, String>,
}

impl Default for ProjectInfo {
    fn default() -> Self {
        Self {
            binary_name: "app".to_string(),
            port: 8080,
            additional_dependencies: Vec::new(),
            build_args: HashMap::new(),
        }
    }
}