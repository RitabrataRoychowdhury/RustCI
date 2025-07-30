use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info};
use uuid::Uuid;

#[allow(dead_code)] // Will be used when CI engine executes pipelines
#[derive(Debug, Clone)]
pub struct Workspace {
    #[allow(dead_code)] // Will be used for workspace identification
    pub id: Uuid,
    pub path: PathBuf,
    #[allow(dead_code)] // Will be used for tracking which execution owns this workspace
    pub execution_id: Uuid,
}

/// Enhanced workspace context that provides path injection and environment setup
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    pub workspace_path: PathBuf,
    pub execution_id: Uuid,
    pub environment_variables: HashMap<String, String>,
    pub working_directory: PathBuf,
    pub source_directory: PathBuf,
    pub build_directory: PathBuf,
    pub artifacts_directory: PathBuf,
}

impl Workspace {
    pub fn new(execution_id: Uuid, base_path: &Path) -> Self {
        let workspace_id = Uuid::new_v4();
        let path = base_path.join(format!("workspace-{}", workspace_id));

        Self {
            id: workspace_id,
            path,
            execution_id,
        }
    }

    pub async fn create(&self) -> Result<()> {
        debug!("📁 Creating workspace: {:?}", self.path);

        fs::create_dir_all(&self.path).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create workspace: {}", e))
        })?;

        info!("✅ Workspace created: {:?}", self.path);
        Ok(())
    }

    #[allow(dead_code)] // Will be used for workspace cleanup
    pub async fn cleanup(&self) -> Result<()> {
        debug!("🗑️ Cleaning up workspace: {:?}", self.path);

        if self.path.exists() {
            fs::remove_dir_all(&self.path).await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to cleanup workspace: {}", e))
            })?;
        }

        info!("✅ Workspace cleaned up: {:?}", self.path);
        Ok(())
    }

    #[allow(dead_code)] // Will be used for file path resolution
    pub fn get_file_path(&self, relative_path: &str) -> PathBuf {
        self.path.join(relative_path)
    }

    #[allow(dead_code)] // Will be used for writing files to workspace
    pub async fn write_file(&self, relative_path: &str, content: &str) -> Result<()> {
        let file_path = self.get_file_path(relative_path);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to create directory: {}", e))
            })?;
        }

        fs::write(&file_path, content)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to write file: {}", e)))?;

        debug!("📝 File written: {:?}", file_path);
        Ok(())
    }

    #[allow(dead_code)] // Will be used for reading files from workspace
    pub async fn read_file(&self, relative_path: &str) -> Result<String> {
        let file_path = self.get_file_path(relative_path);

        fs::read_to_string(&file_path)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to read file: {}", e)))
    }

    #[allow(dead_code)] // Will be used for checking file existence
    pub async fn file_exists(&self, relative_path: &str) -> bool {
        self.get_file_path(relative_path).exists()
    }
}

#[allow(dead_code)] // Will be used for workspace management
#[derive(Debug)]
pub struct WorkspaceManager {
    base_path: PathBuf,
}

impl WorkspaceContext {
    /// Inject workspace paths into command strings
    pub fn inject_into_command(&self, command: &str) -> String {
        command
            .replace("${WORKSPACE}", &self.workspace_path.to_string_lossy())
            .replace("${WORKING_DIR}", &self.working_directory.to_string_lossy())
            .replace("${SOURCE_DIR}", &self.source_directory.to_string_lossy())
            .replace("${BUILD_DIR}", &self.build_directory.to_string_lossy())
            .replace(
                "${ARTIFACTS_DIR}",
                &self.artifacts_directory.to_string_lossy(),
            )
            // Fix hardcoded paths from pipeline.yaml
            .replace("/tmp/rustci", &self.source_directory.to_string_lossy())
    }

    /// Get environment variables with workspace paths
    pub fn get_environment_variables(&self) -> HashMap<String, String> {
        let mut env = self.environment_variables.clone();
        env.insert(
            "WORKSPACE_PATH".to_string(),
            self.workspace_path.to_string_lossy().to_string(),
        );
        env.insert(
            "WORKING_DIR".to_string(),
            self.working_directory.to_string_lossy().to_string(),
        );
        env.insert(
            "SOURCE_DIR".to_string(),
            self.source_directory.to_string_lossy().to_string(),
        );
        env.insert(
            "BUILD_DIR".to_string(),
            self.build_directory.to_string_lossy().to_string(),
        );
        env.insert(
            "ARTIFACTS_DIR".to_string(),
            self.artifacts_directory.to_string_lossy().to_string(),
        );
        env.insert("EXECUTION_ID".to_string(), self.execution_id.to_string());
        env
    }
}

impl WorkspaceManager {
    #[allow(dead_code)] // Will be used when CI engine is initialized
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub async fn create_workspace(&self, execution_id: Uuid) -> Result<Workspace> {
        // Ensure base directory exists
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).await.map_err(|e| {
                AppError::InternalServerError(format!(
                    "Failed to create base workspace directory: {}",
                    e
                ))
            })?;
        }

        let workspace = Workspace::new(execution_id, &self.base_path);
        workspace.create().await?;

        Ok(workspace)
    }

    /// Create workspace with enhanced context for path injection
    pub async fn create_workspace_with_context(
        &self,
        execution_id: Uuid,
    ) -> Result<WorkspaceContext> {
        // Ensure base directory exists with proper permissions
        self.ensure_base_directory().await?;

        let workspace = Workspace::new(execution_id, &self.base_path);
        workspace.create().await?;

        // Create subdirectories for different purposes
        let source_dir = workspace.path.join("source");
        let build_dir = workspace.path.join("build");
        let artifacts_dir = workspace.path.join("artifacts");

        fs::create_dir_all(&source_dir).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create source directory: {}", e))
        })?;
        fs::create_dir_all(&build_dir).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create build directory: {}", e))
        })?;
        fs::create_dir_all(&artifacts_dir).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to create artifacts directory: {}", e))
        })?;

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "SOURCE_DIR".to_string(),
            source_dir.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "BUILD_DIR".to_string(),
            build_dir.to_string_lossy().to_string(),
        );
        env_vars.insert(
            "ARTIFACTS_DIR".to_string(),
            artifacts_dir.to_string_lossy().to_string(),
        );

        info!(
            "✅ Enhanced workspace created with context: {:?}",
            workspace.path
        );
        debug!("📁 Source directory: {:?}", source_dir);
        debug!("📁 Build directory: {:?}", build_dir);
        debug!("📁 Artifacts directory: {:?}", artifacts_dir);

        Ok(WorkspaceContext {
            workspace_path: workspace.path,
            execution_id,
            environment_variables: env_vars,
            working_directory: source_dir.clone(), // Default working directory
            source_directory: source_dir,
            build_directory: build_dir,
            artifacts_directory: artifacts_dir,
        })
    }

    async fn ensure_base_directory(&self) -> Result<()> {
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).await.map_err(|e| {
                AppError::InternalServerError(format!(
                    "Failed to create base workspace directory {}: {}",
                    self.base_path.display(),
                    e
                ))
            })?;
        }

        // Verify permissions
        let metadata = fs::metadata(&self.base_path).await.map_err(|e| {
            AppError::InternalServerError(format!(
                "Failed to read workspace directory metadata: {}",
                e
            ))
        })?;

        if metadata.permissions().readonly() {
            return Err(AppError::InternalServerError(
                "Workspace directory is read-only".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn cleanup_workspace(&self, execution_id: Uuid) -> Result<()> {
        let _ = execution_id;
        // Find workspace by execution_id and clean it up
        let _workspace_pattern = "workspace-*".to_string();
        let mut entries = fs::read_dir(&self.base_path).await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to read workspace directory: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if dir_name.starts_with("workspace-") {
                        // For now, clean up all workspaces
                        // TODO: Implement proper workspace tracking by execution_id
                        if let Err(e) = fs::remove_dir_all(&path).await {
                            error!("⚠️ Failed to cleanup workspace {:?}: {}", path, e);
                        } else {
                            debug!("🗑️ Cleaned up workspace: {:?}", path);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)] // Will be used for workspace retrieval
    pub async fn get_workspace(&self, _execution_id: Uuid) -> Result<Option<Workspace>> {
        // TODO: Implement proper workspace lookup by execution_id
        // For now, this is a placeholder
        Ok(None)
    }
}
