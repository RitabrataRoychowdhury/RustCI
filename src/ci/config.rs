use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use utoipa::ToSchema;

/// Pipeline type enum for multi-tier system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PipelineType {
    Minimal,
    Simple,
    Standard,
    Advanced,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CIPipeline {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub mongo_id: Option<ObjectId>,
    pub id: Option<Uuid>,
    
    // Core fields (preserved for backward compatibility)
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub environment: HashMap<String, String>,
    pub timeout: Option<u64>, // in seconds
    pub retry_count: Option<u32>,
    pub notifications: Option<NotificationConfig>,
    
    // Multi-tier system fields
    pub pipeline_type: Option<PipelineType>,
    
    // Minimal pipeline fields
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub server: Option<ServerConfig>,
    
    // Simple pipeline fields
    pub steps: Option<Vec<SimpleStep>>,
    
    // Advanced pipeline fields
    pub variables: Option<HashMap<String, String>>,
    pub jobs: Option<HashMap<String, PipelineJob>>,
    pub matrix: Option<MatrixConfig>,
    pub cache: Option<CacheConfig>,
    pub include: Option<Vec<IncludeConfig>>,
    
    // Timestamps
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub trigger_type: TriggerType,
    pub config: TriggerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Webhook,
    Schedule,
    Manual,
    GitPush,
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
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Shell,
    Docker,
    Kubernetes,
    AWS,
    Azure,
    GCP,
    GitHub,
    GitLab,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    // Deployment configuration
    pub disable_deployment_detection: Option<bool>,
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
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    Email,
    Slack,
    Webhook,
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

/// Server configuration for minimal pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ServerConfig {
    /// Simple string: "user@host" or "host"
    Simple(String),
    /// Detailed configuration
    Detailed {
        host: String,
        port: Option<u16>,
        user: Option<String>,
        key: Option<String>,
    },
}

/// Simple step for linear pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum SimpleStep {
    /// Just a command string
    Command(String),
    /// Detailed step
    Detailed {
        run: String,
        name: Option<String>,
        working_directory: Option<String>,
    },
}

/// Job definition for standard/advanced pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum PipelineJob {
    /// Simple script string or array
    Simple(JobScript),
    /// Full job definition
    Detailed {
        stage: String,
        script: JobScript,
        matrix: Option<MatrixConfig>,
        cache: Option<CacheConfig>,
    },
}

/// Job script can be single command or multiple commands
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum JobScript {
    Single(String),
    Multiple(Vec<String>),
}

/// Matrix configuration for advanced pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MatrixConfig {
    #[serde(flatten)]
    pub variables: HashMap<String, Vec<String>>,
}

/// Cache configuration for advanced pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheConfig {
    pub paths: Vec<String>,
    pub key: Option<String>,
    pub policy: Option<String>, // pull, push, pull-push
}

/// Include configuration for advanced pipelines
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncludeConfig {
    pub local: Option<String>,
    pub remote: Option<String>,
    pub template: Option<String>,
}

/// Pipeline type detector for auto-detection
pub struct PipelineTypeDetector;

impl PipelineTypeDetector {
    /// Detect pipeline type based on YAML structure
    pub fn detect_type(yaml_value: &serde_yaml::Value) -> PipelineType {
        if let Some(mapping) = yaml_value.as_mapping() {
            // Check for explicit type
            if let Some(type_value) = mapping.get("type") {
                if let Some(type_str) = type_value.as_str() {
                    return match type_str {
                        "minimal" => PipelineType::Minimal,
                        "simple" => PipelineType::Simple,
                        "standard" => PipelineType::Standard,
                        "advanced" => PipelineType::Advanced,
                        _ => Self::auto_detect(mapping),
                    };
                }
            }
            
            Self::auto_detect(mapping)
        } else {
            PipelineType::Minimal
        }
    }
    
    /// Auto-detect pipeline type based on structure
    fn auto_detect(mapping: &serde_yaml::Mapping) -> PipelineType {
        // Advanced features detection
        if mapping.contains_key("matrix") || 
           mapping.contains_key("include") || 
           mapping.contains_key("cache") ||
           mapping.contains_key("variables") {
            return PipelineType::Advanced;
        }
        
        // Standard features detection
        if mapping.contains_key("stages") || mapping.contains_key("jobs") {
            return PipelineType::Standard;
        }
        
        // Simple features detection
        if mapping.contains_key("steps") {
            return PipelineType::Simple;
        }
        
        // Default to minimal
        PipelineType::Minimal
    }
}

impl CIPipeline {
    #[allow(dead_code)] // Will be used when CI engine is fully implemented
    pub fn new(name: String) -> Self {
        Self {
            mongo_id: None,
            id: Some(Uuid::new_v4()),
            name,
            description: None,
            triggers: Vec::new(),
            stages: Vec::new(),
            environment: HashMap::new(),
            timeout: Some(3600), // 1 hour default
            retry_count: Some(0),
            notifications: None,
            pipeline_type: None,
            repo: None,
            branch: None,
            server: None,
            steps: None,
            variables: None,
            jobs: None,
            matrix: None,
            cache: None,
            include: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    /// Parse YAML with automatic pipeline type detection
    pub fn from_yaml(yaml_content: &str) -> Result<Self, serde_yaml::Error> {
        // First parse into a generic Value for type detection
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
        
        // Detect pipeline type
        let detected_type = PipelineTypeDetector::detect_type(&yaml_value);
        
        // Parse into CIPipeline struct
        let mut pipeline: Self = serde_yaml::from_str(yaml_content)?;
        
        // Set the detected type if not explicitly specified
        if pipeline.pipeline_type.is_none() {
            pipeline.pipeline_type = Some(detected_type);
        }
        
        // Apply defaults based on pipeline type
        pipeline.apply_type_defaults();
        
        Ok(pipeline)
    }

    /// Apply defaults based on pipeline type
    fn apply_type_defaults(&mut self) {
        match self.pipeline_type.as_ref().unwrap_or(&PipelineType::Standard) {
            PipelineType::Minimal => {
                // For minimal pipelines, ensure we have a name
                if self.name.is_empty() {
                    self.name = "Minimal Pipeline".to_string();
                }
                // Default branch to main if not specified
                if self.branch.is_none() && self.repo.is_some() {
                    self.branch = Some("main".to_string());
                }
            }
            PipelineType::Simple => {
                if self.name.is_empty() {
                    self.name = "Simple Pipeline".to_string();
                }
            }
            PipelineType::Standard => {
                if self.name.is_empty() {
                    self.name = "Standard Pipeline".to_string();
                }
            }
            PipelineType::Advanced => {
                if self.name.is_empty() {
                    self.name = "Advanced Pipeline".to_string();
                }
            }
        }
    }

    #[allow(dead_code)] // Will be used for exporting pipeline configurations
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Get the pipeline type, defaulting to Standard for backward compatibility
    pub fn get_pipeline_type(&self) -> PipelineType {
        self.pipeline_type.clone().unwrap_or(PipelineType::Standard)
    }

    /// Validate pipeline based on its type
    pub fn validate(&self) -> Result<(), String> {
        let pipeline_type = self.get_pipeline_type();
        
        match pipeline_type {
            PipelineType::Minimal => self.validate_minimal(),
            PipelineType::Simple => self.validate_simple(),
            PipelineType::Standard => self.validate_standard(),
            PipelineType::Advanced => self.validate_advanced(),
        }
    }
    
    /// Validate minimal pipeline
    fn validate_minimal(&self) -> Result<(), String> {
        if self.repo.is_none() {
            return Err("Minimal pipeline requires 'repo' field".to_string());
        }
        
        if self.server.is_none() {
            return Err("Minimal pipeline requires 'server' field".to_string());
        }
        
        Ok(())
    }
    
    /// Validate simple pipeline
    fn validate_simple(&self) -> Result<(), String> {
        if let Some(steps) = &self.steps {
            if steps.is_empty() {
                return Err("Simple pipeline must have at least one step".to_string());
            }
        } else {
            return Err("Simple pipeline requires 'steps' field".to_string());
        }
        
        Ok(())
    }
    
    /// Validate standard pipeline (existing validation logic)
    fn validate_standard(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Pipeline name cannot be empty".to_string());
        }

        if self.stages.is_empty() {
            return Err("Standard pipeline must have at least one stage".to_string());
        }

        for (i, stage) in self.stages.iter().enumerate() {
            if stage.name.is_empty() {
                return Err(format!("Stage {} name cannot be empty", i));
            }

            if stage.steps.is_empty() {
                return Err(format!(
                    "Stage '{}' must have at least one step",
                    stage.name
                ));
            }

            for (j, step) in stage.steps.iter().enumerate() {
                if step.name.is_empty() {
                    return Err(format!(
                        "Step {} in stage '{}' name cannot be empty",
                        j, stage.name
                    ));
                }
            }
        }

        Ok(())
    }
    
    /// Validate advanced pipeline
    fn validate_advanced(&self) -> Result<(), String> {
        // Advanced pipelines accept all fields, so we just do basic validation
        if self.name.is_empty() {
            return Err("Pipeline name cannot be empty".to_string());
        }
        
        // If jobs are defined, validate them
        if let Some(jobs) = &self.jobs {
            if jobs.is_empty() {
                return Err("Advanced pipeline with jobs must have at least one job".to_string());
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_pipeline_parsing() {
        let yaml = r#"
name: "Test Minimal Pipeline"
repo: https://github.com/user/repo.git
server: user@deploy.example.com
"#;
        
        let pipeline = CIPipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.get_pipeline_type(), PipelineType::Minimal);
        assert_eq!(pipeline.repo, Some("https://github.com/user/repo.git".to_string()));
        assert!(pipeline.server.is_some());
        assert_eq!(pipeline.name, "Test Minimal Pipeline");
    }

    #[test]
    fn test_simple_pipeline_parsing() {
        let yaml = r#"
name: "Test Simple Pipeline"
repo: https://github.com/user/repo.git
steps:
  - run: cargo build --release
  - run: cargo test
"#;
        
        let pipeline = CIPipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.get_pipeline_type(), PipelineType::Simple);
        assert!(pipeline.steps.is_some());
        assert_eq!(pipeline.steps.as_ref().unwrap().len(), 2);
        assert_eq!(pipeline.name, "Test Simple Pipeline");
    }

    #[test]
    fn test_standard_pipeline_parsing() {
        let yaml = r#"
name: "Test Standard Pipeline"
repo: https://github.com/user/repo.git
stages:
  - name: build
    steps: []
  - name: test
    steps: []
jobs:
  build:
    stage: build
    script: cargo build --release
"#;
        
        let pipeline = CIPipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.get_pipeline_type(), PipelineType::Standard);
        assert!(pipeline.jobs.is_some());
        assert_eq!(pipeline.jobs.as_ref().unwrap().len(), 1);
        assert_eq!(pipeline.name, "Test Standard Pipeline");
    }

    #[test]
    fn test_advanced_pipeline_parsing() {
        let yaml = r#"
name: "Test Advanced Pipeline"
repo: https://github.com/user/repo.git
variables:
  RUST_VERSION: "1.70"
stages:
  - name: build
    steps: []
jobs:
  build:
    stage: build
    script: cargo build --release
cache:
  paths: [target/]
"#;
        
        let pipeline = CIPipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.get_pipeline_type(), PipelineType::Advanced);
        assert!(pipeline.variables.is_some());
        assert!(pipeline.cache.is_some());
        assert_eq!(pipeline.name, "Test Advanced Pipeline");
    }

    #[test]
    fn test_explicit_type_override() {
        let yaml = r#"
name: "Test Override Pipeline"
type: simple
repo: https://github.com/user/repo.git
stages:
  - name: build
    steps: []
jobs:
  build:
    stage: build
    script: cargo build --release
"#;
        
        let pipeline = CIPipeline::from_yaml(yaml).unwrap();
        // Should respect explicit type even if structure suggests otherwise
        assert_eq!(pipeline.get_pipeline_type(), PipelineType::Simple);
    }

    #[test]
    fn test_pipeline_validation() {
        // Test minimal pipeline validation
        let mut minimal = CIPipeline::new("test".to_string());
        minimal.pipeline_type = Some(PipelineType::Minimal);
        minimal.repo = Some("https://github.com/user/repo.git".to_string());
        minimal.server = Some(ServerConfig::Simple("user@host".to_string()));
        assert!(minimal.validate().is_ok());

        // Test minimal pipeline missing server
        minimal.server = None;
        assert!(minimal.validate().is_err());

        // Test simple pipeline validation
        let mut simple = CIPipeline::new("test".to_string());
        simple.pipeline_type = Some(PipelineType::Simple);
        simple.steps = Some(vec![SimpleStep::Command("cargo build".to_string())]);
        assert!(simple.validate().is_ok());

        // Test simple pipeline with empty steps
        simple.steps = Some(vec![]);
        assert!(simple.validate().is_err());
    }

    #[test]
    fn test_type_detection() {
        // Test minimal detection
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(r#"
repo: https://github.com/user/repo.git
server: user@host
"#).unwrap();
        assert_eq!(PipelineTypeDetector::detect_type(&yaml_value), PipelineType::Minimal);

        // Test simple detection
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(r#"
steps:
  - run: cargo build
"#).unwrap();
        assert_eq!(PipelineTypeDetector::detect_type(&yaml_value), PipelineType::Simple);

        // Test standard detection
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(r#"
stages: [build]
jobs:
  build:
    stage: build
    script: cargo build
"#).unwrap();
        assert_eq!(PipelineTypeDetector::detect_type(&yaml_value), PipelineType::Standard);

        // Test advanced detection
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(r#"
variables:
  TEST: value
stages: [build]
"#).unwrap();
        assert_eq!(PipelineTypeDetector::detect_type(&yaml_value), PipelineType::Advanced);
    }
}
