//! Universal YAML Parser for CI/CD Pipelines
//! 
//! Supports all major CI/CD YAML formats with automatic detection and conversion

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use crate::error::{AppError, Result};

/// Supported CI/CD platforms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlatformType {
    GitHubActions,
    GitLabCI,
    Jenkins,
    AzureDevOps,
    CircleCI,
    TravisCI,
    BitbucketPipelines,
    RustCI,
}

/// Universal pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalPipeline {
    pub name: String,
    pub version: String,
    pub platform: PlatformType,
    pub triggers: Vec<Trigger>,
    pub variables: HashMap<String, String>,
    pub stages: Vec<Stage>,
    pub notifications: Option<Notifications>,
    pub cleanup: Option<CleanupConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub event: String,
    pub branches: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
    pub conditions: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub runner: String,
    pub image: Option<String>,
    pub condition: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub parallel: Option<bool>,
    pub matrix: Option<HashMap<String, Vec<String>>>,
    pub steps: Vec<Step>,
    pub artifacts: Option<Vec<Artifact>>,
    pub environment: Option<Environment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub action: Option<String>,
    pub run: Option<String>,
    pub with: Option<HashMap<String, Value>>,
    pub condition: Option<String>,
    pub timeout: Option<String>,
    pub retry: Option<u32>,
    pub allow_failure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub name: Option<String>,
    pub retention: Option<String>,
    pub artifact_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub name: String,
    pub url: Option<String>,
    pub variables: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notifications {
    pub slack: Option<SlackNotification>,
    pub email: Option<EmailNotification>,
    pub webhook: Option<WebhookNotification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackNotification {
    pub webhook: String,
    pub channel: Option<String>,
    pub on_success: Option<bool>,
    pub on_failure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotification {
    pub recipients: Vec<String>,
    pub on_success: Option<bool>,
    pub on_failure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookNotification {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    pub always: bool,
    pub on_success: Option<bool>,
    pub on_failure: Option<bool>,
}

/// Universal YAML parser with auto-detection
pub struct UniversalYamlParser;

impl UniversalYamlParser {
    /// Parse YAML with automatic platform detection
    pub fn parse(yaml_content: &str) -> Result<UniversalPipeline> {
        let platform = Self::detect_platform(yaml_content)?;
        
        match platform {
            PlatformType::GitHubActions => Self::parse_github_actions(yaml_content),
            PlatformType::GitLabCI => Self::parse_gitlab_ci(yaml_content),
            PlatformType::Jenkins => Self::parse_jenkins(yaml_content),
            PlatformType::AzureDevOps => Self::parse_azure_devops(yaml_content),
            PlatformType::CircleCI => Self::parse_circleci(yaml_content),
            PlatformType::TravisCI => Self::parse_travis_ci(yaml_content),
            PlatformType::BitbucketPipelines => Self::parse_bitbucket(yaml_content),
            PlatformType::RustCI => Self::parse_rustci(yaml_content),
        }
    }
    
    /// Detect CI/CD platform from YAML content
    fn detect_platform(yaml_content: &str) -> Result<PlatformType> {
        let yaml: Value = serde_yaml::from_str(yaml_content)
            .map_err(|e| AppError::ValidationError(format!("Invalid YAML: {}", e)))?;
        
        // GitHub Actions detection
        if yaml.get("on").is_some() || yaml.get("jobs").is_some() {
            return Ok(PlatformType::GitHubActions);
        }
        
        // GitLab CI detection
        if yaml.get("stages").is_some() || yaml.get("before_script").is_some() {
            return Ok(PlatformType::GitLabCI);
        }
        
        // Jenkins detection
        if yaml.get("pipeline").is_some() || yaml.get("agent").is_some() {
            return Ok(PlatformType::Jenkins);
        }
        
        // Azure DevOps detection
        if yaml.get("trigger").is_some() || yaml.get("pool").is_some() {
            return Ok(PlatformType::AzureDevOps);
        }
        
        // CircleCI detection
        if yaml.get("version").is_some() && yaml.get("workflows").is_some() {
            return Ok(PlatformType::CircleCI);
        }
        
        // Travis CI detection
        if yaml.get("language").is_some() || yaml.get("script").is_some() {
            return Ok(PlatformType::TravisCI);
        }
        
        // Bitbucket Pipelines detection
        if yaml.get("pipelines").is_some() {
            return Ok(PlatformType::BitbucketPipelines);
        }
        
        // RustCI detection (default)
        Ok(PlatformType::RustCI)
    }
    
    /// Parse GitHub Actions workflow
    fn parse_github_actions(yaml_content: &str) -> Result<UniversalPipeline> {
        let yaml: Value = serde_yaml::from_str(yaml_content)?;
        
        let name = yaml.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("GitHub Actions Pipeline")
            .to_string();
        
        // Parse triggers
        let mut triggers = Vec::new();
        if let Some(on_value) = yaml.get("on") {
            triggers = Self::parse_github_triggers(on_value)?;
        }
        
        // Parse environment variables
        let variables = yaml.get("env")
            .and_then(|v| v.as_mapping())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        Some((k.as_str()?.to_string(), v.as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        
        // Parse jobs as stages
        let stages = if let Some(jobs) = yaml.get("jobs").and_then(|v| v.as_mapping()) {
            Self::parse_github_jobs(jobs)?
        } else {
            Vec::new()
        };
        
        Ok(UniversalPipeline {
            name,
            version: "1.0".to_string(),
            platform: PlatformType::GitHubActions,
            triggers,
            variables,
            stages,
            notifications: None,
            cleanup: None,
        })
    }
    
    fn parse_github_triggers(on_value: &Value) -> Result<Vec<Trigger>> {
        let mut triggers = Vec::new();
        
        match on_value {
            Value::String(event) => {
                triggers.push(Trigger {
                    event: event.clone(),
                    branches: None,
                    paths: None,
                    conditions: None,
                });
            }
            Value::Sequence(events) => {
                for event in events {
                    if let Some(event_str) = event.as_str() {
                        triggers.push(Trigger {
                            event: event_str.to_string(),
                            branches: None,
                            paths: None,
                            conditions: None,
                        });
                    }
                }
            }
            Value::Mapping(events_map) => {
                for (event, config) in events_map {
                    if let Some(event_str) = event.as_str() {
                        let mut trigger = Trigger {
                            event: event_str.to_string(),
                            branches: None,
                            paths: None,
                            conditions: None,
                        };
                        
                        if let Some(config_map) = config.as_mapping() {
                            if let Some(branches) = config_map.get("branches") {
                                trigger.branches = Self::parse_string_array(branches);
                            }
                            if let Some(paths) = config_map.get("paths") {
                                trigger.paths = Self::parse_string_array(paths);
                            }
                        }
                        
                        triggers.push(trigger);
                    }
                }
            }
            _ => {}
        }
        
        Ok(triggers)
    }
    
    fn parse_github_jobs(jobs: &serde_yaml::Mapping) -> Result<Vec<Stage>> {
        let mut stages = Vec::new();
        
        for (job_name, job_config) in jobs {
            if let (Some(name), Some(config_map)) = (job_name.as_str(), job_config.as_mapping()) {
                let mut stage = Stage {
                    name: name.to_string(),
                    runner: "ubuntu-latest".to_string(),
                    image: None,
                    condition: None,
                    depends_on: None,
                    parallel: None,
                    matrix: None,
                    steps: Vec::new(),
                    artifacts: None,
                    environment: None,
                };
                
                // Parse runner
                if let Some(runs_on) = config_map.get("runs-on").and_then(|v| v.as_str()) {
                    stage.runner = runs_on.to_string();
                }
                
                // Parse needs (dependencies)
                if let Some(needs) = config_map.get("needs") {
                    stage.depends_on = Self::parse_string_array(needs);
                }
                
                // Parse strategy matrix
                if let Some(strategy) = config_map.get("strategy").and_then(|v| v.as_mapping()) {
                    if let Some(matrix) = strategy.get("matrix").and_then(|v| v.as_mapping()) {
                        let mut matrix_map = HashMap::new();
                        for (key, values) in matrix {
                            if let (Some(key_str), Some(values_array)) = (key.as_str(), Self::parse_string_array(values)) {
                                matrix_map.insert(key_str.to_string(), values_array);
                            }
                        }
                        if !matrix_map.is_empty() {
                            stage.matrix = Some(matrix_map);
                        }
                    }
                }
                
                // Parse steps
                if let Some(steps) = config_map.get("steps").and_then(|v| v.as_sequence()) {
                    stage.steps = Self::parse_github_steps(steps)?;
                }
                
                stages.push(stage);
            }
        }
        
        Ok(stages)
    }
    
    fn parse_github_steps(steps: &Vec<Value>) -> Result<Vec<Step>> {
        let mut parsed_steps = Vec::new();
        
        for step in steps {
            if let Some(step_map) = step.as_mapping() {
                let mut parsed_step = Step {
                    name: step_map.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed step")
                        .to_string(),
                    action: None,
                    run: None,
                    with: None,
                    condition: None,
                    timeout: None,
                    retry: None,
                    allow_failure: None,
                };
                
                // Parse uses (action)
                if let Some(uses) = step_map.get("uses").and_then(|v| v.as_str()) {
                    parsed_step.action = Some(uses.to_string());
                }
                
                // Parse run command
                if let Some(run) = step_map.get("run").and_then(|v| v.as_str()) {
                    parsed_step.run = Some(run.to_string());
                }
                
                // Parse with parameters
                if let Some(with) = step_map.get("with").and_then(|v| v.as_mapping()) {
                    let mut with_map = HashMap::new();
                    for (key, value) in with {
                        if let Some(key_str) = key.as_str() {
                            with_map.insert(key_str.to_string(), value.clone());
                        }
                    }
                    parsed_step.with = Some(with_map);
                }
                
                // Parse if condition
                if let Some(condition) = step_map.get("if").and_then(|v| v.as_str()) {
                    parsed_step.condition = Some(condition.to_string());
                }
                
                parsed_steps.push(parsed_step);
            }
        }
        
        Ok(parsed_steps)
    }
    
    /// Parse GitLab CI configuration
    fn parse_gitlab_ci(yaml_content: &str) -> Result<UniversalPipeline> {
        let yaml: Value = serde_yaml::from_str(yaml_content)?;
        
        let name = "GitLab CI Pipeline".to_string();
        
        // Parse variables
        let variables = yaml.get("variables")
            .and_then(|v| v.as_mapping())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        Some((k.as_str()?.to_string(), v.as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        
        // Parse stages
        let stage_names = yaml.get("stages")
            .and_then(|v| v.as_sequence())
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        
        // Parse jobs
        let mut stages = Vec::new();
        if let Some(yaml_map) = yaml.as_mapping() {
            for (key, value) in yaml_map {
                if let Some(key_str) = key.as_str() {
                    // Skip special keys
                    if ["stages", "variables", "before_script", "after_script", "image"].contains(&key_str) {
                        continue;
                    }
                    
                    if let Some(job_config) = value.as_mapping() {
                        let stage = Self::parse_gitlab_job(key_str, job_config)?;
                        stages.push(stage);
                    }
                }
            }
        }
        
        Ok(UniversalPipeline {
            name,
            version: "1.0".to_string(),
            platform: PlatformType::GitLabCI,
            triggers: vec![Trigger {
                event: "push".to_string(),
                branches: None,
                paths: None,
                conditions: None,
            }],
            variables,
            stages,
            notifications: None,
            cleanup: None,
        })
    }
    
    fn parse_gitlab_job(job_name: &str, job_config: &serde_yaml::Mapping) -> Result<Stage> {
        let mut stage = Stage {
            name: job_name.to_string(),
            runner: "docker".to_string(),
            image: None,
            condition: None,
            depends_on: None,
            parallel: None,
            matrix: None,
            steps: Vec::new(),
            artifacts: None,
            environment: None,
        };
        
        // Parse image
        if let Some(image) = job_config.get("image").and_then(|v| v.as_str()) {
            stage.image = Some(image.to_string());
        }
        
        // Parse script as steps
        if let Some(script) = job_config.get("script") {
            let scripts = match script {
                Value::String(s) => vec![s.clone()],
                Value::Sequence(seq) => seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => Vec::new(),
            };
            
            for (i, script_line) in scripts.iter().enumerate() {
                stage.steps.push(Step {
                    name: format!("Script {}", i + 1),
                    action: None,
                    run: Some(script_line.clone()),
                    with: None,
                    condition: None,
                    timeout: None,
                    retry: None,
                    allow_failure: None,
                });
            }
        }
        
        Ok(stage)
    }
    
    /// Parse native RustCI configuration
    fn parse_rustci(yaml_content: &str) -> Result<UniversalPipeline> {
        serde_yaml::from_str(yaml_content)
            .map_err(|e| AppError::ValidationError(format!("Invalid RustCI YAML: {}", e)))
    }
    
    /// Placeholder parsers for other platforms
    fn parse_jenkins(_yaml_content: &str) -> Result<UniversalPipeline> {
        // TODO: Implement Jenkins pipeline parsing
        Err(AppError::ValidationError("Jenkins parsing not yet implemented".to_string()))
    }
    
    fn parse_azure_devops(_yaml_content: &str) -> Result<UniversalPipeline> {
        // TODO: Implement Azure DevOps parsing
        Err(AppError::ValidationError("Azure DevOps parsing not yet implemented".to_string()))
    }
    
    fn parse_circleci(_yaml_content: &str) -> Result<UniversalPipeline> {
        // TODO: Implement CircleCI parsing
        Err(AppError::ValidationError("CircleCI parsing not yet implemented".to_string()))
    }
    
    fn parse_travis_ci(_yaml_content: &str) -> Result<UniversalPipeline> {
        // TODO: Implement Travis CI parsing
        Err(AppError::ValidationError("Travis CI parsing not yet implemented".to_string()))
    }
    
    fn parse_bitbucket(_yaml_content: &str) -> Result<UniversalPipeline> {
        // TODO: Implement Bitbucket Pipelines parsing
        Err(AppError::ValidationError("Bitbucket Pipelines parsing not yet implemented".to_string()))
    }
    
    /// Helper function to parse string arrays from YAML values
    fn parse_string_array(value: &Value) -> Option<Vec<String>> {
        match value {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Sequence(seq) => {
                let strings: Vec<String> = seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if strings.is_empty() { None } else { Some(strings) }
            }
            _ => None,
        }
    }
    
    /// Convert universal pipeline to RustCI format
    pub fn to_rustci_yaml(pipeline: &UniversalPipeline) -> Result<String> {
        serde_yaml::to_string(pipeline)
            .map_err(|e| AppError::ValidationError(format!("Failed to serialize to YAML: {}", e)))
    }
    
    /// Validate pipeline configuration
    pub fn validate(pipeline: &UniversalPipeline) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // Check for empty stages
        if pipeline.stages.is_empty() {
            warnings.push("Pipeline has no stages defined".to_string());
        }
        
        // Check for stages without steps
        for stage in &pipeline.stages {
            if stage.steps.is_empty() {
                warnings.push(format!("Stage '{}' has no steps defined", stage.name));
            }
        }
        
        // Check for circular dependencies
        // TODO: Implement dependency cycle detection
        
        Ok(warnings)
    }
}