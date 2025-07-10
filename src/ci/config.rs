use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIPipeline {
    pub id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<Trigger>,
    pub stages: Vec<Stage>,
    pub environment: HashMap<String, String>,
    pub timeout: Option<u64>, // in seconds
    pub retry_count: Option<u32>,
    pub notifications: Option<NotificationConfig>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub trigger_type: TriggerType,
    pub config: TriggerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerType {
    #[serde(rename = "webhook")]
    Webhook,
    #[serde(rename = "schedule")]
    Schedule,
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "git_push")]
    GitPush,
    #[serde(rename = "pull_request")]
    PullRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub webhook_url: Option<String>,
    pub cron_expression: Option<String>,
    pub branch_patterns: Option<Vec<String>>,
    pub repository: Option<String>,
    pub events: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub condition: Option<String>,
    pub parallel: Option<bool>,
    pub steps: Vec<Step>,
    pub environment: Option<HashMap<String, String>>,
    pub timeout: Option<u64>,
    pub retry_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub step_type: StepType,
    pub config: StepConfig,
    pub condition: Option<String>,
    pub continue_on_error: Option<bool>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepType {
    #[serde(rename = "shell")]
    Shell,
    #[serde(rename = "docker")]
    Docker,
    #[serde(rename = "kubernetes")]
    Kubernetes,
    #[serde(rename = "aws")]
    AWS,
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "gcp")]
    GCP,
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
    #[serde(rename = "custom")]
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepConfig {
    // Shell commands
    pub command: Option<String>,
    pub script: Option<String>,
    pub working_directory: Option<String>,
    
    // Docker configuration
    pub image: Option<String>,
    pub dockerfile: Option<String>,
    pub build_context: Option<String>,
    pub registry: Option<String>,
    pub tags: Option<Vec<String>>,
    
    // Kubernetes configuration
    pub namespace: Option<String>,
    pub manifest: Option<String>,
    pub helm_chart: Option<String>,
    pub values: Option<HashMap<String, serde_json::Value>>,
    
    // Cloud provider configurations
    pub region: Option<String>,
    pub service: Option<String>,
    pub action: Option<String>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    
    // Git operations
    pub repository_url: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub credentials: Option<String>,
    
    // Custom plugin configuration
    pub plugin_name: Option<String>,
    pub plugin_config: Option<HashMap<String, serde_json::Value>>,
    
    // Environment variables for this step
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub on_success: Option<Vec<NotificationTarget>>,
    pub on_failure: Option<Vec<NotificationTarget>>,
    pub on_start: Option<Vec<NotificationTarget>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTarget {
    pub target_type: NotificationType,
    pub config: NotificationTargetConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationType {
    #[serde(rename = "email")]
    Email,
    #[serde(rename = "slack")]
    Slack,
    #[serde(rename = "webhook")]
    Webhook,
    #[serde(rename = "github")]
    GitHub,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTargetConfig {
    pub email: Option<String>,
    pub webhook_url: Option<String>,
    pub slack_channel: Option<String>,
    pub slack_token: Option<String>,
    pub github_status: Option<bool>,
}

impl CIPipeline {
    #[allow(dead_code)] // Will be used when CI engine is fully implemented
    pub fn new(name: String) -> Self {
        Self {
            id: Some(Uuid::new_v4()),
            name,
            description: None,
            triggers: Vec::new(),
            stages: Vec::new(),
            environment: HashMap::new(),
            timeout: Some(3600), // 1 hour default
            retry_count: Some(0),
            notifications: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    pub fn from_yaml(yaml_content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml_content)
    }

    #[allow(dead_code)] // Will be used for exporting pipeline configurations
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Pipeline name cannot be empty".to_string());
        }

        if self.stages.is_empty() {
            return Err("Pipeline must have at least one stage".to_string());
        }

        for (i, stage) in self.stages.iter().enumerate() {
            if stage.name.is_empty() {
                return Err(format!("Stage {} name cannot be empty", i));
            }

            if stage.steps.is_empty() {
                return Err(format!("Stage '{}' must have at least one step", stage.name));
            }

            for (j, step) in stage.steps.iter().enumerate() {
                if step.name.is_empty() {
                    return Err(format!("Step {} in stage '{}' name cannot be empty", j, stage.name));
                }
            }
        }

        Ok(())
    }
}