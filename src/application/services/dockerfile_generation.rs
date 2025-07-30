#![allow(dead_code)]

use crate::{
    domain::entities::{ProjectInfo, ProjectType},
    error::{AppError, Result},
};

pub trait DockerfileGenerator: Send + Sync {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String>;
    fn get_template(&self) -> &str;
    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String;
    fn get_default_port(&self) -> u16;
    fn get_default_binary_name(&self, project_name: &str) -> String;
}

pub struct DockerfileGeneratorFactory;

impl DockerfileGeneratorFactory {
    pub fn create_generator(project_type: ProjectType) -> Box<dyn DockerfileGenerator> {
        match project_type {
            ProjectType::Rust => Box::new(RustDockerfileGenerator::new()),
            ProjectType::Node => Box::new(NodeDockerfileGenerator::new()),
            ProjectType::Python => Box::new(PythonDockerfileGenerator::new()),
            ProjectType::Java => Box::new(JavaDockerfileGenerator::new()),
            ProjectType::Go => Box::new(GoDockerfileGenerator::new()),
            ProjectType::Unknown => Box::new(GenericDockerfileGenerator::new()),
        }
    }

    pub fn generate_dockerfile(
        project_type: ProjectType,
        project_info: &ProjectInfo,
    ) -> Result<String> {
        let generator = Self::create_generator(project_type);
        generator.generate(project_info)
    }
}

// Rust Dockerfile Generator
pub struct RustDockerfileGenerator;

impl Default for RustDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl RustDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for RustDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated Dockerfile for Rust project
# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/main.rs

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/{{binary_name}} ./app

# Create non-root user
RUN useradd -r -s /bin/false appuser
RUN chown appuser:appuser ./app
USER appuser

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Run the application
CMD ["./app"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        8080
    }

    fn get_default_binary_name(&self, project_name: &str) -> String {
        project_name.replace("-", "_")
    }
}

// Node.js Dockerfile Generator
pub struct NodeDockerfileGenerator;

impl Default for NodeDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for NodeDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated Dockerfile for Node.js project
# Build stage
FROM node:18-alpine as builder

WORKDIR /app

# Copy package files
COPY package*.json ./
COPY yarn.lock* ./

# Install dependencies
RUN if [ -f yarn.lock ]; then yarn install --frozen-lockfile; \
    else npm ci --only=production; fi

# Copy source code
COPY . .

# Build the application (if build script exists)
RUN if grep -q '"build"' package.json; then \
    if [ -f yarn.lock ]; then yarn build; else npm run build; fi; \
    fi

# Runtime stage
FROM node:18-alpine

# Install dumb-init for proper signal handling
RUN apk add --no-cache dumb-init

WORKDIR /app

# Create non-root user
RUN addgroup -g 1001 -S nodejs
RUN adduser -S nextjs -u 1001

# Copy built application
COPY --from=builder --chown=nextjs:nodejs /app ./

USER nextjs

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Run the application
ENTRYPOINT ["dumb-init", "--"]
CMD ["node", "{{binary_name}}"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        3000
    }

    fn get_default_binary_name(&self, _project_name: &str) -> String {
        "index.js".to_string()
    }
}

// Python Dockerfile Generator
pub struct PythonDockerfileGenerator;

impl Default for PythonDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for PythonDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated Dockerfile for Python project
FROM python:3.11-slim

# Set environment variables
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements first for better caching
COPY requirements*.txt ./
COPY pyproject.toml* ./
COPY setup.py* ./

# Install Python dependencies
RUN if [ -f requirements.txt ]; then pip install --no-cache-dir -r requirements.txt; \
    elif [ -f pyproject.toml ]; then pip install --no-cache-dir .; \
    elif [ -f setup.py ]; then pip install --no-cache-dir .; \
    fi

# Copy source code
COPY . .

# Create non-root user
RUN useradd -r -s /bin/false appuser
RUN chown -R appuser:appuser /app
USER appuser

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Run the application
CMD ["python", "{{binary_name}}"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        8000
    }

    fn get_default_binary_name(&self, _project_name: &str) -> String {
        "main.py".to_string()
    }
}

// Java Dockerfile Generator
pub struct JavaDockerfileGenerator;

impl Default for JavaDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for JavaDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated Dockerfile for Java project
# Build stage
FROM maven:3.9-openjdk-17 as builder

WORKDIR /app

# Copy pom.xml first for better caching
COPY pom.xml ./
COPY src ./src

# Build the application
RUN mvn clean package -DskipTests

# Runtime stage
FROM openjdk:17-jre-slim

# Install curl for health checks
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the JAR file from builder stage
COPY --from=builder /app/target/*.jar ./{{binary_name}}.jar

# Create non-root user
RUN useradd -r -s /bin/false appuser
RUN chown appuser:appuser ./{{binary_name}}.jar
USER appuser

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Run the application
CMD ["java", "-jar", "./{{binary_name}}.jar"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        8080
    }

    fn get_default_binary_name(&self, project_name: &str) -> String {
        format!("{}-app", project_name)
    }
}

// Go Dockerfile Generator
pub struct GoDockerfileGenerator;

impl Default for GoDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl GoDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for GoDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated Dockerfile for Go project
# Build stage
FROM golang:1.21-alpine as builder

# Install git for go modules
RUN apk add --no-cache git

WORKDIR /app

# Copy go mod files
COPY go.mod go.sum ./

# Download dependencies
RUN go mod download

# Copy source code
COPY . .

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o {{binary_name}} .

# Runtime stage
FROM alpine:latest

# Install ca-certificates and curl
RUN apk --no-cache add ca-certificates curl

WORKDIR /root/

# Copy the binary from builder stage
COPY --from=builder /app/{{binary_name}} ./

# Create non-root user
RUN adduser -D -s /bin/sh appuser
RUN chown appuser:appuser ./{{binary_name}}
USER appuser

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Run the application
CMD ["./{{binary_name}}"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        8080
    }

    fn get_default_binary_name(&self, project_name: &str) -> String {
        project_name.to_string()
    }
}

// Generic Dockerfile Generator (fallback)
pub struct GenericDockerfileGenerator;

impl Default for GenericDockerfileGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericDockerfileGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DockerfileGenerator for GenericDockerfileGenerator {
    fn generate(&self, project_info: &ProjectInfo) -> Result<String> {
        let template = self.get_template();
        let customized = self.customize_for_project(template, project_info);
        Ok(customized)
    }

    fn get_template(&self) -> &str {
        r#"# Auto-generated generic Dockerfile
FROM ubuntu:22.04

# Install basic dependencies
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy all files
COPY . .

# Create non-root user
RUN useradd -r -s /bin/false appuser
RUN chown -R appuser:appuser /app
USER appuser

# Expose port
EXPOSE {{port}}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:{{port}}/health || exit 1

# Default command (should be customized)
CMD ["echo", "Please customize this Dockerfile for your specific application"]
"#
    }

    fn customize_for_project(&self, template: &str, project_info: &ProjectInfo) -> String {
        template
            .replace("{{binary_name}}", &project_info.binary_name)
            .replace("{{port}}", &project_info.port.to_string())
    }

    fn get_default_port(&self) -> u16 {
        8080
    }

    fn get_default_binary_name(&self, project_name: &str) -> String {
        project_name.to_string()
    }
}

// Helper function to create project info with smart defaults
pub fn create_project_info_with_defaults(
    project_type: ProjectType,
    project_name: &str,
    custom_port: Option<u16>,
    custom_binary_name: Option<String>,
) -> ProjectInfo {
    let generator = DockerfileGeneratorFactory::create_generator(project_type);

    let port = custom_port.unwrap_or_else(|| generator.get_default_port());
    let binary_name =
        custom_binary_name.unwrap_or_else(|| generator.get_default_binary_name(project_name));

    ProjectInfo {
        binary_name,
        port,
        dependencies: Vec::new(),
        build_command: None,
        run_command: None,
    }
}

// Service for managing Dockerfile generation with caching and validation
pub struct DockerfileGenerationService {
    cache: std::sync::RwLock<std::collections::HashMap<String, String>>,
}

impl Default for DockerfileGenerationService {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerfileGenerationService {
    pub fn new() -> Self {
        Self {
            cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn generate_dockerfile(
        &self,
        project_type: ProjectType,
        project_info: &ProjectInfo,
        use_cache: bool,
    ) -> Result<String> {
        let cache_key = format!(
            "{:?}_{}_{}_{}",
            project_type,
            project_info.binary_name,
            project_info.port,
            project_info.dependencies.join(",")
        );

        if use_cache {
            if let Ok(cache) = self.cache.read() {
                if let Some(cached_dockerfile) = cache.get(&cache_key) {
                    return Ok(cached_dockerfile.clone());
                }
            }
        }

        let dockerfile =
            DockerfileGeneratorFactory::generate_dockerfile(project_type, project_info)?;

        // Cache the result
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, dockerfile.clone());
        }

        Ok(dockerfile)
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn validate_dockerfile_syntax(&self, dockerfile_content: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Basic syntax validation
        if !dockerfile_content.contains("FROM") {
            return Err(AppError::DockerfileGenerationFailed(
                "Dockerfile must contain a FROM instruction".to_string(),
            ));
        }

        // Check for best practices
        if !dockerfile_content.contains("USER") {
            warnings.push("Consider adding a USER instruction for security".to_string());
        }

        if !dockerfile_content.contains("HEALTHCHECK") {
            warnings.push("Consider adding a HEALTHCHECK instruction".to_string());
        }

        if dockerfile_content.contains("ADD") && !dockerfile_content.contains("COPY") {
            warnings.push("Consider using COPY instead of ADD when possible".to_string());
        }

        if dockerfile_content.lines().any(|line| {
            line.trim().starts_with("RUN apt-get update")
                && !line.contains("rm -rf /var/lib/apt/lists/*")
        }) {
            warnings.push("Consider cleaning apt cache after apt-get update".to_string());
        }

        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_project_info() -> ProjectInfo {
        ProjectInfo {
            binary_name: "test-app".to_string(),
            port: 8080,
            dependencies: Vec::new(),
            build_command: None,
            run_command: None,
        }
    }

    #[test]
    fn test_rust_dockerfile_generation() {
        let generator = RustDockerfileGenerator::new();
        let project_info = create_test_project_info();

        let dockerfile = generator.generate(&project_info).unwrap();

        assert!(dockerfile.contains("FROM rust:1.75"));
        assert!(dockerfile.contains("test-app"));
        assert!(dockerfile.contains("8080"));
        assert!(dockerfile.contains("HEALTHCHECK"));
        assert!(dockerfile.contains("USER appuser"));
    }

    #[test]
    fn test_node_dockerfile_generation() {
        let generator = NodeDockerfileGenerator::new();
        let project_info = create_test_project_info();

        let dockerfile = generator.generate(&project_info).unwrap();

        assert!(dockerfile.contains("FROM node:18-alpine"));
        assert!(dockerfile.contains("test-app"));
        assert!(dockerfile.contains("8080"));
        assert!(dockerfile.contains("dumb-init"));
    }

    #[test]
    fn test_python_dockerfile_generation() {
        let generator = PythonDockerfileGenerator::new();
        let project_info = create_test_project_info();

        let dockerfile = generator.generate(&project_info).unwrap();

        assert!(dockerfile.contains("FROM python:3.11-slim"));
        assert!(dockerfile.contains("test-app"));
        assert!(dockerfile.contains("8080"));
        assert!(dockerfile.contains("PYTHONUNBUFFERED"));
    }

    #[test]
    fn test_java_dockerfile_generation() {
        let generator = JavaDockerfileGenerator::new();
        let project_info = create_test_project_info();

        let dockerfile = generator.generate(&project_info).unwrap();

        assert!(dockerfile.contains("FROM maven:3.9-openjdk-17"));
        assert!(dockerfile.contains("test-app"));
        assert!(dockerfile.contains("8080"));
        assert!(dockerfile.contains("mvn clean package"));
    }

    #[test]
    fn test_go_dockerfile_generation() {
        let generator = GoDockerfileGenerator::new();
        let project_info = create_test_project_info();

        let dockerfile = generator.generate(&project_info).unwrap();

        assert!(dockerfile.contains("FROM golang:1.21-alpine"));
        assert!(dockerfile.contains("test-app"));
        assert!(dockerfile.contains("8080"));
        assert!(dockerfile.contains("go mod download"));
    }

    #[test]
    fn test_factory_pattern() {
        let rust_generator = DockerfileGeneratorFactory::create_generator(ProjectType::Rust);
        let node_generator = DockerfileGeneratorFactory::create_generator(ProjectType::Node);

        assert_eq!(rust_generator.get_default_port(), 8080);
        assert_eq!(node_generator.get_default_port(), 3000);
    }

    #[test]
    fn test_dockerfile_generation_service() {
        let service = DockerfileGenerationService::new();
        let project_info = create_test_project_info();

        let dockerfile1 = service
            .generate_dockerfile(ProjectType::Rust, &project_info, true)
            .unwrap();
        let dockerfile2 = service
            .generate_dockerfile(ProjectType::Rust, &project_info, true)
            .unwrap();

        // Should be the same due to caching
        assert_eq!(dockerfile1, dockerfile2);

        service.clear_cache();
    }

    #[test]
    fn test_dockerfile_validation() {
        let service = DockerfileGenerationService::new();

        let valid_dockerfile =
            "FROM ubuntu:20.04\nUSER appuser\nHEALTHCHECK CMD curl -f http://localhost:8080/health";
        let warnings = service
            .validate_dockerfile_syntax(valid_dockerfile)
            .unwrap();
        assert!(warnings.is_empty());

        let invalid_dockerfile = "RUN echo hello";
        let result = service.validate_dockerfile_syntax(invalid_dockerfile);
        assert!(result.is_err());

        let dockerfile_with_warnings =
            "FROM ubuntu:20.04\nRUN apt-get update && apt-get install -y curl";
        let warnings = service
            .validate_dockerfile_syntax(dockerfile_with_warnings)
            .unwrap();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_create_project_info_with_defaults() {
        let project_info =
            create_project_info_with_defaults(ProjectType::Node, "my-app", None, None);

        assert_eq!(project_info.port, 3000); // Node.js default
        assert_eq!(project_info.binary_name, "index.js"); // Node.js default

        let custom_project_info = create_project_info_with_defaults(
            ProjectType::Rust,
            "my-rust-app",
            Some(9000),
            Some("custom-binary".to_string()),
        );

        assert_eq!(custom_project_info.port, 9000);
        assert_eq!(custom_project_info.binary_name, "custom-binary");
    }
}
