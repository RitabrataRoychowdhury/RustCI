#![allow(dead_code)]

use async_trait::async_trait;
use bollard::Docker;
use bollard::image::BuildImageOptions;
use bollard::container::{CreateContainerOptions, Config, StartContainerOptions, WaitContainerOptions};
use futures_util::stream::TryStreamExt;

use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::{
    error::{AppError, Result},
    models::ValidationResult,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ValidatorType {
    Local,
    Remote,
    Mock,
}

#[async_trait]
pub trait DockerfileValidator: Send + Sync {
    async fn validate(&self, dockerfile_content: &str, context_path: Option<&str>) -> Result<ValidationResult>;
    fn validator_type(&self) -> ValidatorType;
    fn supports_build_test(&self) -> bool;
    fn supports_run_test(&self) -> bool;
}

pub struct DockerfileValidatorContext {
    strategy: Box<dyn DockerfileValidator>,
}

impl DockerfileValidatorContext {
    pub fn new(strategy: Box<dyn DockerfileValidator>) -> Self {
        Self { strategy }
    }
    
    pub fn set_strategy(&mut self, strategy: Box<dyn DockerfileValidator>) {
        self.strategy = strategy;
    }
    
    pub async fn validate(&self, dockerfile_content: &str, context_path: Option<&str>) -> Result<ValidationResult> {
        self.strategy.validate(dockerfile_content, context_path).await
    }
    
    pub fn get_validator_type(&self) -> ValidatorType {
        self.strategy.validator_type()
    }
}

// Local Docker Validator (uses local Docker daemon)
pub struct LocalDockerValidator {
    docker_client: Arc<Docker>,
    cleanup_images: bool,
    test_timeout_seconds: u64,
}

impl LocalDockerValidator {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_socket_defaults()
            .map_err(|e| AppError::DockerValidationError(format!("Failed to connect to Docker: {}", e)))?;
        
        Ok(Self {
            docker_client: Arc::new(docker),
            cleanup_images: true,
            test_timeout_seconds: 300, // 5 minutes
        })
    }
    
    pub fn with_cleanup(mut self, cleanup: bool) -> Self {
        self.cleanup_images = cleanup;
        self
    }
    
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.test_timeout_seconds = timeout_seconds;
        self
    }
    
    async fn create_build_context(&self, dockerfile_content: &str, context_path: Option<&str>) -> Result<TempDir> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| AppError::DockerValidationError(format!("Failed to create temp directory: {}", e)))?;
        
        // Write Dockerfile
        let dockerfile_path = temp_dir.path().join("Dockerfile");
        let mut dockerfile_file = fs::File::create(&dockerfile_path).await
            .map_err(|e| AppError::DockerValidationError(format!("Failed to create Dockerfile: {}", e)))?;
        
        dockerfile_file.write_all(dockerfile_content.as_bytes()).await
            .map_err(|e| AppError::DockerValidationError(format!("Failed to write Dockerfile: {}", e)))?;
        
        // If context_path is provided, copy files from there
        if let Some(_context) = context_path {
            // For now, we'll create a simple context with just the Dockerfile
            // In a real implementation, you'd copy the actual project files
            let readme_path = temp_dir.path().join("README.md");
            fs::write(&readme_path, "# Test Project\nThis is a test context for Dockerfile validation.").await
                .map_err(|e| AppError::DockerValidationError(format!("Failed to create context file: {}", e)))?;
        }
        
        Ok(temp_dir)
    }
    
    async fn build_image(&self, build_context: &TempDir) -> Result<(String, Vec<String>)> {
        let image_tag = format!("rustci-validation-{}", uuid::Uuid::new_v4());
        let mut build_logs = Vec::new();
        
        let build_options = BuildImageOptions {
            dockerfile: "Dockerfile".to_string(),
            t: image_tag.clone(),
            rm: true,
            pull: true,
            ..Default::default()
        };
        
        let build_context_tar = self.create_tar_archive(build_context.path()).await?;
        
        let mut build_stream = self.docker_client.build_image(
            build_options,
            None,
            Some(build_context_tar.into()),
        );
        
        while let Some(build_info) = build_stream.try_next().await
            .map_err(|e| AppError::DockerValidationError(format!("Build stream error: {}", e)))? {
            
            if let Some(stream) = build_info.stream {
                build_logs.push(stream.trim().to_string());
            }
            
            if let Some(error) = build_info.error {
                return Err(AppError::DockerValidationError(format!("Build failed: {}", error)));
            }
        }
        
        Ok((image_tag, build_logs))
    }
    
    async fn test_container_run(&self, image_tag: &str) -> Result<Vec<String>> {
        let container_name = format!("rustci-test-{}", uuid::Uuid::new_v4());
        let mut run_logs = Vec::new();
        
        // Create container
        let config = Config {
            image: Some(image_tag.to_string()),
            cmd: Some(vec!["echo".to_string(), "Container started successfully".to_string()]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            platform: None,
        };
        
        let container = self.docker_client
            .create_container(Some(create_options), config)
            .await
            .map_err(|e| AppError::DockerValidationError(format!("Failed to create container: {}", e)))?;
        
        // Start container
        self.docker_client
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| AppError::DockerValidationError(format!("Failed to start container: {}", e)))?;
        
        // Wait for container to finish (with timeout)
        let wait_options = WaitContainerOptions {
            condition: "not-running".to_string(),
        };
        
        let mut wait_stream = self.docker_client.wait_container(&container.id, Some(wait_options));
        
        // Use timeout to prevent hanging
        let wait_result = tokio::time::timeout(
            std::time::Duration::from_secs(self.test_timeout_seconds),
            wait_stream.try_next()
        ).await;
        
        match wait_result {
            Ok(Ok(Some(wait_info))) => {
                let exit_code = wait_info.status_code;
                run_logs.push(format!("Container exited with code: {}", exit_code));
                if exit_code != 0 {
                    run_logs.push("Container failed to run successfully".to_string());
                } else {
                    run_logs.push("Container ran successfully".to_string());
                }
            }
            Ok(Ok(None)) => {
                run_logs.push("Container wait stream ended unexpectedly".to_string());
            }
            Ok(Err(e)) => {
                run_logs.push(format!("Container wait error: {}", e));
            }
            Err(_) => {
                run_logs.push("Container run test timed out".to_string());
            }
        }
        
        // Clean up container
        let _ = self.docker_client.remove_container(&container.id, None).await;
        
        Ok(run_logs)
    }
    
    async fn cleanup_image(&self, image_tag: &str) -> Result<()> {
        if self.cleanup_images {
            let _ = self.docker_client.remove_image(image_tag, None, None).await;
        }
        Ok(())
    }
    
    async fn create_tar_archive(&self, dir_path: &std::path::Path) -> Result<Vec<u8>> {
        
        
        let mut tar_data = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut tar_data);
            tar.append_dir_all(".", dir_path)
                .map_err(|e| AppError::DockerValidationError(format!("Failed to create tar archive: {}", e)))?;
            tar.finish()
                .map_err(|e| AppError::DockerValidationError(format!("Failed to finish tar archive: {}", e)))?;
        }
        Ok(tar_data)
    }
}

#[async_trait]
impl DockerfileValidator for LocalDockerValidator {
    async fn validate(&self, dockerfile_content: &str, context_path: Option<&str>) -> Result<ValidationResult> {
        // Create build context
        let build_context = self.create_build_context(dockerfile_content, context_path).await?;
        
        // Build the image
        let (image_tag, build_logs) = match self.build_image(&build_context).await {
            Ok((tag, logs)) => (tag, logs),
            Err(e) => {
                return Ok(ValidationResult {
                    success: false,
                    build_logs: vec![],
                    run_logs: vec![],
                    errors: vec![e.to_string()],
                    warnings: vec![],
                });
            }
        };
        
        // Test container run
        let run_logs = match self.test_container_run(&image_tag).await {
            Ok(logs) => logs,
            Err(e) => {
                // Clean up image even if run test fails
                let _ = self.cleanup_image(&image_tag).await;
                return Ok(ValidationResult {
                    success: false,
                    build_logs,
                    run_logs: vec![],
                    errors: vec![format!("Run test failed: {}", e)],
                    warnings: vec![],
                });
            }
        };
        
        // Clean up
        let _ = self.cleanup_image(&image_tag).await;
        
        // Determine success based on logs
        let success = !build_logs.iter().any(|log| log.to_lowercase().contains("error")) &&
                     !run_logs.iter().any(|log| log.contains("failed"));
        
        let mut warnings = Vec::new();
        if build_logs.iter().any(|log| log.to_lowercase().contains("warning")) {
            warnings.push("Build produced warnings".to_string());
        }
        
        Ok(ValidationResult {
            success,
            build_logs,
            run_logs,
            errors: vec![],
            warnings,
        })
    }
    
    fn validator_type(&self) -> ValidatorType {
        ValidatorType::Local
    }
    
    fn supports_build_test(&self) -> bool {
        true
    }
    
    fn supports_run_test(&self) -> bool {
        true
    }
}

// Mock Validator (for testing)
pub struct MockDockerValidator {
    should_succeed: bool,
    mock_build_logs: Vec<String>,
    mock_run_logs: Vec<String>,
    mock_errors: Vec<String>,
    mock_warnings: Vec<String>,
}

impl MockDockerValidator {
    pub fn new_success() -> Self {
        Self {
            should_succeed: true,
            mock_build_logs: vec![
                "Step 1/5 : FROM ubuntu:20.04".to_string(),
                "Successfully built abc123def456".to_string(),
                "Successfully tagged test-image:latest".to_string(),
            ],
            mock_run_logs: vec![
                "Container started successfully".to_string(),
                "Container exited with code: 0".to_string(),
            ],
            mock_errors: vec![],
            mock_warnings: vec![],
        }
    }
    
    pub fn new_failure() -> Self {
        Self {
            should_succeed: false,
            mock_build_logs: vec![
                "Step 1/5 : FROM ubuntu:20.04".to_string(),
                "ERROR: Failed to build image".to_string(),
            ],
            mock_run_logs: vec![],
            mock_errors: vec!["Build failed due to invalid Dockerfile syntax".to_string()],
            mock_warnings: vec![],
        }
    }
    
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.mock_warnings = warnings;
        self
    }
}

#[async_trait]
impl DockerfileValidator for MockDockerValidator {
    async fn validate(&self, _dockerfile_content: &str, _context_path: Option<&str>) -> Result<ValidationResult> {
        // Simulate some processing time
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        Ok(ValidationResult {
            success: self.should_succeed,
            build_logs: self.mock_build_logs.clone(),
            run_logs: self.mock_run_logs.clone(),
            errors: self.mock_errors.clone(),
            warnings: self.mock_warnings.clone(),
        })
    }
    
    fn validator_type(&self) -> ValidatorType {
        ValidatorType::Mock
    }
    
    fn supports_build_test(&self) -> bool {
        true
    }
    
    fn supports_run_test(&self) -> bool {
        true
    }
}

// Remote Validator (placeholder for future implementation)
pub struct RemoteDockerValidator {
    endpoint: String,
    api_key: Option<String>,
}

impl RemoteDockerValidator {
    pub fn new(endpoint: String, api_key: Option<String>) -> Self {
        Self { endpoint, api_key }
    }
}

#[async_trait]
impl DockerfileValidator for RemoteDockerValidator {
    async fn validate(&self, _dockerfile_content: &str, _context_path: Option<&str>) -> Result<ValidationResult> {
        // Placeholder for remote validation
        // In a real implementation, this would send the Dockerfile to a remote service
        Err(AppError::DockerValidationError("Remote validation not yet implemented".to_string()))
    }
    
    fn validator_type(&self) -> ValidatorType {
        ValidatorType::Remote
    }
    
    fn supports_build_test(&self) -> bool {
        false
    }
    
    fn supports_run_test(&self) -> bool {
        false
    }
}

// Factory for creating validators
pub struct DockerfileValidatorFactory;

impl DockerfileValidatorFactory {
    pub fn create_local_validator() -> Result<Box<dyn DockerfileValidator>> {
        let validator = LocalDockerValidator::new()?;
        Ok(Box::new(validator))
    }
    
    pub fn create_mock_validator(should_succeed: bool) -> Box<dyn DockerfileValidator> {
        if should_succeed {
            Box::new(MockDockerValidator::new_success())
        } else {
            Box::new(MockDockerValidator::new_failure())
        }
    }
    
    pub fn create_remote_validator(endpoint: String, api_key: Option<String>) -> Box<dyn DockerfileValidator> {
        Box::new(RemoteDockerValidator::new(endpoint, api_key))
    }
    
    pub fn create_validator_by_type(validator_type: ValidatorType) -> Result<Box<dyn DockerfileValidator>> {
        match validator_type {
            ValidatorType::Local => Self::create_local_validator(),
            ValidatorType::Mock => Ok(Self::create_mock_validator(true)),
            ValidatorType::Remote => Ok(Self::create_remote_validator(
                "https://api.example.com/validate".to_string(),
                None,
            )),
        }
    }
}

// Service for managing Dockerfile validation with different strategies
pub struct DockerfileValidationService {
    default_validator: ValidatorType,
    fallback_validator: ValidatorType,
}

impl DockerfileValidationService {
    pub fn new() -> Self {
        Self {
            default_validator: ValidatorType::Local,
            fallback_validator: ValidatorType::Mock,
        }
    }
    
    pub fn with_default_validator(mut self, validator_type: ValidatorType) -> Self {
        self.default_validator = validator_type;
        self
    }
    
    pub fn with_fallback_validator(mut self, validator_type: ValidatorType) -> Self {
        self.fallback_validator = validator_type;
        self
    }
    
    pub async fn validate_dockerfile(
        &self,
        dockerfile_content: &str,
        context_path: Option<&str>,
        preferred_validator: Option<ValidatorType>,
    ) -> Result<ValidationResult> {
        let validator_type = preferred_validator.unwrap_or(self.default_validator.clone());
        
        // Try primary validator
        match DockerfileValidatorFactory::create_validator_by_type(validator_type) {
            Ok(validator) => {
                let context = DockerfileValidatorContext::new(validator);
                match context.validate(dockerfile_content, context_path).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("Primary validator failed: {}, trying fallback", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create primary validator: {}, trying fallback", e);
            }
        }
        
        // Try fallback validator
        let fallback_validator = DockerfileValidatorFactory::create_validator_by_type(self.fallback_validator.clone())?;
        let context = DockerfileValidatorContext::new(fallback_validator);
        context.validate(dockerfile_content, context_path).await
    }
    
    pub async fn validate_with_multiple_strategies(
        &self,
        dockerfile_content: &str,
        context_path: Option<&str>,
        strategies: Vec<ValidatorType>,
    ) -> Result<Vec<(ValidatorType, Result<ValidationResult>)>> {
        let mut results = Vec::new();
        
        for strategy in strategies {
            let validator_result = DockerfileValidatorFactory::create_validator_by_type(strategy.clone());
            
            let validation_result = match validator_result {
                Ok(validator) => {
                    let context = DockerfileValidatorContext::new(validator);
                    context.validate(dockerfile_content, context_path).await
                }
                Err(e) => Err(e),
            };
            
            results.push((strategy, validation_result));
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_validator_success() {
        let validator = MockDockerValidator::new_success();
        let result = validator.validate("FROM ubuntu:20.04", None).await.unwrap();
        
        assert!(result.success);
        assert!(!result.build_logs.is_empty());
        assert!(!result.run_logs.is_empty());
        assert!(result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_mock_validator_failure() {
        let validator = MockDockerValidator::new_failure();
        let result = validator.validate("INVALID DOCKERFILE", None).await.unwrap();
        
        assert!(!result.success);
        assert!(!result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_validator_context() {
        let validator = MockDockerValidator::new_success();
        let mut context = DockerfileValidatorContext::new(Box::new(validator));
        
        let result = context.validate("FROM ubuntu:20.04", None).await.unwrap();
        assert!(result.success);
        assert_eq!(context.get_validator_type(), ValidatorType::Mock);
        
        // Change strategy
        let new_validator = MockDockerValidator::new_failure();
        context.set_strategy(Box::new(new_validator));
        
        let result2 = context.validate("FROM ubuntu:20.04", None).await.unwrap();
        assert!(!result2.success);
    }
    
    #[tokio::test]
    async fn test_validation_service() {
        let service = DockerfileValidationService::new()
            .with_default_validator(ValidatorType::Mock)
            .with_fallback_validator(ValidatorType::Mock);
        
        let result = service.validate_dockerfile("FROM ubuntu:20.04", None, None).await.unwrap();
        assert!(result.success);
    }
    
    #[tokio::test]
    async fn test_multiple_strategies() {
        let service = DockerfileValidationService::new();
        let strategies = vec![ValidatorType::Mock];
        
        let results = service.validate_with_multiple_strategies(
            "FROM ubuntu:20.04",
            None,
            strategies,
        ).await.unwrap();
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, ValidatorType::Mock);
        assert!(results[0].1.is_ok());
    }
    
    #[test]
    fn test_validator_factory() {
        let mock_validator = DockerfileValidatorFactory::create_mock_validator(true);
        assert_eq!(mock_validator.validator_type(), ValidatorType::Mock);
        assert!(mock_validator.supports_build_test());
        assert!(mock_validator.supports_run_test());
        
        let remote_validator = DockerfileValidatorFactory::create_remote_validator(
            "https://example.com".to_string(),
            Some("api_key".to_string()),
        );
        assert_eq!(remote_validator.validator_type(), ValidatorType::Remote);
    }
    
    // Note: Local Docker validator tests would require Docker to be running
    // These would typically be integration tests run in a CI environment
    #[tokio::test]
    #[ignore] // Ignore by default since it requires Docker
    async fn test_local_docker_validator() {
        if let Ok(validator) = LocalDockerValidator::new() {
            let simple_dockerfile = r#"
FROM alpine:latest
RUN echo "Hello, World!"
CMD ["echo", "Container is running"]
"#;
            
            let result = validator.validate(simple_dockerfile, None).await.unwrap();
            // This test would only pass if Docker is available and working
            println!("Validation result: {:?}", result);
        }
    }
}