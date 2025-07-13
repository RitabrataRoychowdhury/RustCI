use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use tracing::{info, debug, error, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub build_type: BuildType,
    pub build_commands: Vec<String>,
    pub environment_variables: HashMap<String, String>,
    pub build_directory: Option<String>,
    pub output_directory: Option<String>,
    pub cache_enabled: bool,
    pub parallel_jobs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildType {
    NodeJs,
    Python,
    Rust,
    Java,
    Go,
    DotNet,
    Static,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub build_type: BuildType,
    pub success: bool,
    pub build_time: chrono::DateTime<chrono::Utc>,
    pub duration_seconds: u64,
    pub artifacts: Vec<BuildArtifact>,
    pub logs: Vec<String>,
    pub output_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArtifact {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub artifact_type: ArtifactType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Executable,
    Library,
    Archive,
    StaticFiles,
    Documentation,
    Configuration,
}

pub struct ProjectBuilder {
    cache_directory: PathBuf,
}

impl ProjectBuilder {
    pub fn new(cache_directory: PathBuf) -> Self {
        Self { cache_directory }
    }

    pub async fn build_project(
        &self,
        workspace_path: &Path,
        config: Option<BuildConfig>,
    ) -> Result<BuildResult> {
        let start_time = chrono::Utc::now();
        info!("🔨 Starting build process in: {}", workspace_path.display());

        // Auto-detect project type if not specified
        let build_config = if let Some(config) = config {
            config
        } else {
            self.auto_detect_build_config(workspace_path).await?
        };

        info!("🔍 Detected build type: {:?}", build_config.build_type);

        let mut result = BuildResult {
            build_type: build_config.build_type.clone(),
            success: false,
            build_time: start_time,
            duration_seconds: 0,
            artifacts: Vec::new(),
            logs: Vec::new(),
            output_directory: None,
        };

        // Execute build based on project type
        match build_config.build_type {
            BuildType::NodeJs => self.build_nodejs(workspace_path, &build_config, &mut result).await?,
            BuildType::Python => self.build_python(workspace_path, &build_config, &mut result).await?,
            BuildType::Rust => self.build_rust(workspace_path, &build_config, &mut result).await?,
            BuildType::Java => self.build_java(workspace_path, &build_config, &mut result).await?,
            BuildType::Go => self.build_go(workspace_path, &build_config, &mut result).await?,
            BuildType::DotNet => self.build_dotnet(workspace_path, &build_config, &mut result).await?,
            BuildType::Static => self.build_static(workspace_path, &build_config, &mut result).await?,
            BuildType::Custom => self.build_custom(workspace_path, &build_config, &mut result).await?,
        }

        let end_time = chrono::Utc::now();
        result.duration_seconds = (end_time - start_time).num_seconds() as u64;

        if result.success {
            info!("✅ Build completed successfully in {} seconds", result.duration_seconds);
        } else {
            error!("❌ Build failed after {} seconds", result.duration_seconds);
        }

        Ok(result)
    }

    async fn auto_detect_build_config(&self, workspace_path: &Path) -> Result<BuildConfig> {
        let build_type = if workspace_path.join("package.json").exists() {
            BuildType::NodeJs
        } else if workspace_path.join("requirements.txt").exists() || workspace_path.join("pyproject.toml").exists() {
            BuildType::Python
        } else if workspace_path.join("Cargo.toml").exists() {
            BuildType::Rust
        } else if workspace_path.join("pom.xml").exists() || workspace_path.join("build.gradle").exists() {
            BuildType::Java
        } else if workspace_path.join("go.mod").exists() {
            BuildType::Go
        } else if workspace_path.join("*.csproj").exists() || workspace_path.join("*.sln").exists() {
            BuildType::DotNet
        } else if workspace_path.join("index.html").exists() {
            BuildType::Static
        } else {
            return Err(AppError::ValidationError("Unable to detect project type".to_string()));
        };

        let build_commands = self.get_default_build_commands(&build_type);

        Ok(BuildConfig {
            build_type,
            build_commands,
            environment_variables: HashMap::new(),
            build_directory: None,
            output_directory: None,
            cache_enabled: true,
            parallel_jobs: None,
        })
    }

    fn get_default_build_commands(&self, build_type: &BuildType) -> Vec<String> {
        match build_type {
            BuildType::NodeJs => vec![
                "npm ci".to_string(),
                "npm run build".to_string(),
            ],
            BuildType::Python => vec![
                "pip install -r requirements.txt".to_string(),
                "python setup.py build".to_string(),
            ],
            BuildType::Rust => vec![
                "cargo build --release".to_string(),
            ],
            BuildType::Java => vec![
                "mvn clean package -DskipTests".to_string(),
            ],
            BuildType::Go => vec![
                "go mod download".to_string(),
                "go build -o app".to_string(),
            ],
            BuildType::DotNet => vec![
                "dotnet restore".to_string(),
                "dotnet build --configuration Release".to_string(),
            ],
            BuildType::Static => vec![
                "echo 'No build required for static files'".to_string(),
            ],
            BuildType::Custom => vec![],
        }
    }

    async fn build_nodejs(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🟢 Building Node.js project");

        // Check if package.json exists
        let package_json_path = workspace_path.join("package.json");
        if !package_json_path.exists() {
            return Err(AppError::ValidationError("package.json not found".to_string()));
        }

        // Read package.json to understand the project
        let package_json_content = fs::read_to_string(&package_json_path).await?;
        let package_json: serde_json::Value = serde_json::from_str(&package_json_content)
            .map_err(|e| AppError::ValidationError(format!("Invalid package.json: {}", e)))?;

        // Install dependencies
        let install_command = if workspace_path.join("package-lock.json").exists() {
            "npm ci"
        } else if workspace_path.join("yarn.lock").exists() {
            "yarn install --frozen-lockfile"
        } else {
            "npm install"
        };

        self.execute_command(install_command, workspace_path, &config.environment_variables, result).await?;

        // Run build command if available
        if let Some(scripts) = package_json.get("scripts").and_then(|s| s.as_object()) {
            if scripts.contains_key("build") {
                self.execute_command("npm run build", workspace_path, &config.environment_variables, result).await?;
                
                // Look for common output directories
                let output_dirs = ["dist", "build", "out", "public"];
                for dir in &output_dirs {
                    let output_path = workspace_path.join(dir);
                    if output_path.exists() {
                        result.output_directory = Some(output_path.clone());
                        self.collect_artifacts(&output_path, result).await?;
                        break;
                    }
                }
            }
        }

        result.success = true;
        Ok(())
    }

    async fn build_python(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🐍 Building Python project");

        // Create virtual environment
        self.execute_command("python -m venv venv", workspace_path, &config.environment_variables, result).await?;

        // Activate virtual environment and install dependencies
        let pip_command = if workspace_path.join("requirements.txt").exists() {
            "venv/bin/pip install -r requirements.txt"
        } else if workspace_path.join("pyproject.toml").exists() {
            "venv/bin/pip install ."
        } else {
            return Err(AppError::ValidationError("No requirements.txt or pyproject.toml found".to_string()));
        };

        self.execute_command(pip_command, workspace_path, &config.environment_variables, result).await?;

        // Run setup.py build if available
        if workspace_path.join("setup.py").exists() {
            self.execute_command("venv/bin/python setup.py build", workspace_path, &config.environment_variables, result).await?;
        }

        result.success = true;
        Ok(())
    }

    async fn build_rust(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🦀 Building Rust project");

        // Check if Cargo.toml exists
        if !workspace_path.join("Cargo.toml").exists() {
            return Err(AppError::ValidationError("Cargo.toml not found".to_string()));
        }

        // Build the project
        let build_command = if config.build_commands.is_empty() {
            "cargo build --release"
        } else {
            &config.build_commands[0]
        };

        self.execute_command(build_command, workspace_path, &config.environment_variables, result).await?;

        // Collect artifacts from target directory
        let target_dir = workspace_path.join("target/release");
        if target_dir.exists() {
            result.output_directory = Some(target_dir.clone());
            self.collect_artifacts(&target_dir, result).await?;
        }

        result.success = true;
        Ok(())
    }

    async fn build_java(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("☕ Building Java project");

        if workspace_path.join("pom.xml").exists() {
            // Maven project
            self.execute_command("mvn clean package -DskipTests", workspace_path, &config.environment_variables, result).await?;
            
            let target_dir = workspace_path.join("target");
            if target_dir.exists() {
                result.output_directory = Some(target_dir.clone());
                self.collect_artifacts(&target_dir, result).await?;
            }
        } else if workspace_path.join("build.gradle").exists() {
            // Gradle project
            self.execute_command("./gradlew build", workspace_path, &config.environment_variables, result).await?;
            
            let build_dir = workspace_path.join("build");
            if build_dir.exists() {
                result.output_directory = Some(build_dir.clone());
                self.collect_artifacts(&build_dir, result).await?;
            }
        } else {
            return Err(AppError::ValidationError("No pom.xml or build.gradle found".to_string()));
        }

        result.success = true;
        Ok(())
    }

    async fn build_go(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🐹 Building Go project");

        if !workspace_path.join("go.mod").exists() {
            return Err(AppError::ValidationError("go.mod not found".to_string()));
        }

        // Download dependencies
        self.execute_command("go mod download", workspace_path, &config.environment_variables, result).await?;

        // Build the project
        self.execute_command("go build -o app", workspace_path, &config.environment_variables, result).await?;

        // Collect the built binary
        let app_path = workspace_path.join("app");
        if app_path.exists() {
            let metadata = fs::metadata(&app_path).await?;
            result.artifacts.push(BuildArtifact {
                name: "app".to_string(),
                path: app_path,
                size_bytes: metadata.len(),
                artifact_type: ArtifactType::Executable,
            });
        }

        result.success = true;
        Ok(())
    }

    async fn build_dotnet(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🔷 Building .NET project");

        // Restore dependencies
        self.execute_command("dotnet restore", workspace_path, &config.environment_variables, result).await?;

        // Build the project
        self.execute_command("dotnet build --configuration Release", workspace_path, &config.environment_variables, result).await?;

        // Publish the project
        self.execute_command("dotnet publish --configuration Release --output ./publish", workspace_path, &config.environment_variables, result).await?;

        let publish_dir = workspace_path.join("publish");
        if publish_dir.exists() {
            result.output_directory = Some(publish_dir.clone());
            self.collect_artifacts(&publish_dir, result).await?;
        }

        result.success = true;
        Ok(())
    }

    async fn build_static(
        &self,
        workspace_path: &Path,
        _config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("📄 Processing static files");

        // For static files, just collect all files as artifacts
        result.output_directory = Some(workspace_path.to_path_buf());
        self.collect_artifacts(workspace_path, result).await?;
        result.success = true;
        Ok(())
    }

    async fn build_custom(
        &self,
        workspace_path: &Path,
        config: &BuildConfig,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🔧 Running custom build commands");

        for command in &config.build_commands {
            self.execute_command(command, workspace_path, &config.environment_variables, result).await?;
        }

        result.success = true;
        Ok(())
    }

    async fn execute_command(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: &HashMap<String, String>,
        result: &mut BuildResult,
    ) -> Result<()> {
        info!("🔧 Executing: {}", command);

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(working_dir);

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let output = cmd.output().await
            .map_err(|e| AppError::InternalServerError(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        result.logs.push(format!("Command: {}", command));
        if !stdout.is_empty() {
            result.logs.push(format!("STDOUT: {}", stdout));
        }
        if !stderr.is_empty() {
            result.logs.push(format!("STDERR: {}", stderr));
        }

        if !output.status.success() {
            result.success = false;
            return Err(AppError::InternalServerError(format!("Command failed: {}", command)));
        }

        Ok(())
    }

    async fn collect_artifacts(&self, directory: &Path, result: &mut BuildResult) -> Result<()> {
        let mut entries = fs::read_dir(directory).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                let artifact_type = self.determine_artifact_type(&path);
                result.artifacts.push(BuildArtifact {
                    name: path.file_name().unwrap().to_string_lossy().to_string(),
                    path: path.clone(),
                    size_bytes: metadata.len(),
                    artifact_type,
                });
            } else if metadata.is_dir() {
                // Recursively collect artifacts from subdirectories
                self.collect_artifacts(&path, result).await?;
            }
        }

        Ok(())
    }

    fn determine_artifact_type(&self, path: &Path) -> ArtifactType {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            match extension.to_lowercase().as_str() {
                "exe" | "bin" => ArtifactType::Executable,
                "so" | "dll" | "dylib" | "a" => ArtifactType::Library,
                "tar" | "gz" | "zip" | "rar" | "7z" => ArtifactType::Archive,
                "html" | "css" | "js" | "png" | "jpg" | "svg" => ArtifactType::StaticFiles,
                "json" | "yaml" | "yml" | "toml" | "ini" | "conf" => ArtifactType::Configuration,
                "md" | "txt" | "pdf" => ArtifactType::Documentation,
                _ => ArtifactType::StaticFiles,
            }
        } else {
            // Check if file is executable
            if path.file_name().and_then(|name| name.to_str()).map_or(false, |name| !name.contains('.')) {
                ArtifactType::Executable
            } else {
                ArtifactType::StaticFiles
            }
        }
    }
}