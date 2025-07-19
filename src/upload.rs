use crate::error::{AppError, Result};
use axum::extract::Multipart;
// Removed redundant import
use tracing::info;

/// Configuration for file upload handling
#[derive(Debug, Clone)]
pub struct FileUploadConfig {
    pub max_file_size: usize,
    pub allowed_extensions: Vec<String>,
}

impl Default for FileUploadConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB
            allowed_extensions: vec!["yaml".to_string(), "yml".to_string()],
        }
    }
}

/// Handler for processing multipart file uploads
pub struct FileUploadHandler {
    config: FileUploadConfig,
}

impl FileUploadHandler {
    pub fn new(config: FileUploadConfig) -> Self {
        Self { config }
    }

    #[allow(dead_code)]
    pub fn with_default_config() -> Self {
        Self::new(FileUploadConfig::default())
    }

    /// Handle YAML file upload from multipart form data
    pub async fn handle_yaml_upload(&self, mut multipart: Multipart) -> Result<String> {
        info!("📁 Processing multipart file upload");

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            AppError::FileUploadError(format!("Failed to read multipart field: {}", e))
        })? {
            let field_name = field.name().unwrap_or("unknown").to_string();

            // Look for the pipeline file field
            if field_name == "pipeline" || field_name == "file" || field_name == "yaml" {
                return self.process_yaml_field(field).await;
            }
        }

        Err(AppError::FileUploadError(
            "No pipeline file found in multipart data. Expected field name: 'pipeline', 'file', or 'yaml'".to_string()
        ))
    }

    /// Process a single multipart field containing YAML content
    async fn process_yaml_field(
        &self,
        field: axum::extract::multipart::Field<'_>,
    ) -> Result<String> {
        let file_name = field.file_name().unwrap_or("pipeline.yaml").to_string();

        info!("📄 Processing uploaded file: {}", file_name);

        // Validate file extension
        self.validate_file_extension(&file_name)?;

        // Read file content
        let file_data = field
            .bytes()
            .await
            .map_err(|e| AppError::FileUploadError(format!("Failed to read file data: {}", e)))?;

        // Validate file size
        self.validate_file_size(file_data.len())?;

        // Convert to string
        let yaml_content = String::from_utf8(file_data.to_vec())
            .map_err(|e| AppError::FileUploadError(format!("File is not valid UTF-8: {}", e)))?;

        // Validate YAML content
        self.validate_yaml_content(&yaml_content)?;

        info!(
            "✅ Successfully processed YAML file: {} ({} bytes)",
            file_name,
            yaml_content.len()
        );

        Ok(yaml_content)
    }

    /// Validate file extension
    fn validate_file_extension(&self, file_name: &str) -> Result<()> {
        let extension = file_name.split('.').next_back().unwrap_or("").to_lowercase();

        if !self.config.allowed_extensions.contains(&extension) {
            return Err(AppError::UnsupportedFileType(format!(
                "File extension '{}' not allowed. Supported extensions: {}",
                extension,
                self.config.allowed_extensions.join(", ")
            )));
        }

        Ok(())
    }

    /// Validate file size
    fn validate_file_size(&self, size: usize) -> Result<()> {
        if size > self.config.max_file_size {
            return Err(AppError::FileSizeExceeded {
                actual: size,
                limit: self.config.max_file_size,
            });
        }

        if size == 0 {
            return Err(AppError::FileUploadError("File is empty".to_string()));
        }

        Ok(())
    }

    /// Validate YAML content structure
    pub fn validate_yaml_content(&self, content: &str) -> Result<()> {
        // First, check if it's valid YAML
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(content)
            .map_err(|e| AppError::ValidationError(format!("Invalid YAML syntax: {}", e)))?;

        // Check if it's a mapping (object)
        let yaml_map = yaml_value.as_mapping().ok_or_else(|| {
            AppError::ValidationError("YAML must be an object/mapping".to_string())
        })?;

        // Validate required fields for pipeline configuration
        self.validate_pipeline_structure(yaml_map)?;

        Ok(())
    }

    /// Validate pipeline-specific YAML structure
    fn validate_pipeline_structure(&self, yaml_map: &serde_yaml::Mapping) -> Result<()> {
        let required_fields = ["name", "stages"];
        let mut missing_fields = Vec::new();

        for field in &required_fields {
            if !yaml_map.contains_key(&serde_yaml::Value::String(field.to_string())) {
                missing_fields.push(field.to_string());
            }
        }

        if !missing_fields.is_empty() {
            return Err(AppError::ValidationError(format!(
                "Missing required fields in pipeline YAML: {}. Required fields: {}",
                missing_fields.join(", "),
                required_fields.join(", ")
            )));
        }

        // Validate stages structure
        if let Some(stages) = yaml_map.get(&serde_yaml::Value::String("stages".to_string())) {
            let stages_array = stages.as_sequence().ok_or_else(|| {
                AppError::ValidationError("'stages' must be an array".to_string())
            })?;

            if stages_array.is_empty() {
                return Err(AppError::ValidationError(
                    "Pipeline must have at least one stage".to_string(),
                ));
            }

            // Validate each stage has required fields
            for (i, stage) in stages_array.iter().enumerate() {
                let stage_map = stage.as_mapping().ok_or_else(|| {
                    AppError::ValidationError(format!("Stage {} must be an object", i + 1))
                })?;

                if !stage_map.contains_key(&serde_yaml::Value::String("name".to_string())) {
                    return Err(AppError::ValidationError(format!(
                        "Stage {} missing required 'name' field",
                        i + 1
                    )));
                }

                if !stage_map.contains_key(&serde_yaml::Value::String("steps".to_string())) {
                    return Err(AppError::ValidationError(format!(
                        "Stage {} missing required 'steps' field",
                        i + 1
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get upload configuration
    #[allow(dead_code)]
    pub fn get_config(&self) -> &FileUploadConfig {
        &self.config
    }
}

/// Helper function to create a file upload handler with environment-based configuration
pub fn create_upload_handler() -> FileUploadHandler {
    let max_size = std::env::var("MAX_UPLOAD_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10 * 1024 * 1024); // Default 10MB

    let config = FileUploadConfig {
        max_file_size: max_size,
        allowed_extensions: vec!["yaml".to_string(), "yml".to_string()],
    };

    FileUploadHandler::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_extension() {
        let handler = FileUploadHandler::with_default_config();

        assert!(handler.validate_file_extension("pipeline.yaml").is_ok());
        assert!(handler.validate_file_extension("config.yml").is_ok());
        assert!(handler.validate_file_extension("test.YAML").is_ok()); // Case insensitive

        assert!(handler.validate_file_extension("config.json").is_err());
        assert!(handler.validate_file_extension("script.sh").is_err());
    }

    #[test]
    fn test_validate_file_size() {
        let handler = FileUploadHandler::with_default_config();

        assert!(handler.validate_file_size(1024).is_ok()); // 1KB
        assert!(handler.validate_file_size(5 * 1024 * 1024).is_ok()); // 5MB

        assert!(handler.validate_file_size(0).is_err()); // Empty file
        assert!(handler.validate_file_size(15 * 1024 * 1024).is_err()); // 15MB > 10MB limit
    }

    #[test]
    fn test_validate_yaml_content() {
        let handler = FileUploadHandler::with_default_config();

        let valid_yaml = r#"
name: "Test Pipeline"
description: "A test pipeline"
stages:
  - name: "Build"
    steps:
      - name: "build-step"
        step_type: shell
        config:
          command: "echo Building..."
environment: {}
timeout: 3600
retry_count: 0
"#;

        assert!(handler.validate_yaml_content(valid_yaml).is_ok());

        // Test invalid YAML
        let invalid_yaml = "name: test\n  invalid: yaml: structure";
        assert!(handler.validate_yaml_content(invalid_yaml).is_err());

        // Test missing required fields
        let incomplete_yaml = r#"
description: "Missing name and stages"
timeout: 3600
"#;
        assert!(handler.validate_yaml_content(incomplete_yaml).is_err());
    }
}
