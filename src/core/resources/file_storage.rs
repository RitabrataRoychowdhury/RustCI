//! File Upload and Storage Management System
//! 
//! Provides comprehensive file lifecycle management with automatic cleanup,
//! file storage quotas and monitoring, and file virus scanning and security validation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use sha2::{Sha256, Digest};

/// File storage types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileStorageType {
    Temporary,
    Permanent,
    Cache,
    Upload,
    Backup,
    Log,
}

/// File security scan results
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanResult {
    Clean,
    Suspicious,
    Malicious,
    Error(String),
}

/// File metadata for tracking and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: Uuid,
    pub original_name: String,
    pub stored_path: PathBuf,
    pub file_type: FileStorageType,
    pub size_bytes: u64,
    pub mime_type: Option<String>,
    pub checksum: String,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub owner_id: Option<String>,
    pub tags: HashMap<String, String>,
    pub scan_result: Option<ScanResult>,
    pub access_count: u64,
}

/// File storage quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageQuota {
    pub storage_type: FileStorageType,
    pub max_total_size: u64,
    pub max_file_count: usize,
    pub max_file_size: u64,
    pub retention_period: Duration,
    pub cleanup_interval: Duration,
}

impl Default for StorageQuota {
    fn default() -> Self {
        Self {
            storage_type: FileStorageType::Temporary,
            max_total_size: 1024 * 1024 * 1024, // 1GB
            max_file_count: 10000,
            max_file_size: 100 * 1024 * 1024, // 100MB
            retention_period: Duration::from_secs(3600), // 1 hour
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// File storage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub files_by_type: HashMap<FileStorageType, usize>,
    pub size_by_type: HashMap<FileStorageType, u64>,
    pub files_cleaned: u64,
    pub bytes_freed: u64,
    pub scan_results: HashMap<ScanResult, u64>,
}

/// Trait for file virus scanners
#[async_trait::async_trait]
pub trait VirusScanner: Send + Sync {
    async fn scan_file(&self, file_path: &Path) -> Result<ScanResult, Box<dyn std::error::Error + Send + Sync>>;
    fn get_scanner_name(&self) -> &str;
}

/// Trait for file storage backends
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn store_file(&self, file_data: &[u8], metadata: &FileMetadata) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>>;
    async fn retrieve_file(&self, file_path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
    async fn delete_file(&self, file_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn get_file_size(&self, file_path: &Path) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
    fn get_backend_name(&self) -> &str;
}

/// File upload and storage manager
pub struct FileStorageManager {
    base_path: PathBuf,
    files: Arc<RwLock<HashMap<Uuid, FileMetadata>>>,
    quotas: Arc<RwLock<HashMap<FileStorageType, StorageQuota>>>,
    virus_scanners: Arc<RwLock<Vec<Arc<dyn VirusScanner>>>>,
    storage_backend: Arc<dyn StorageBackend>,
    stats: Arc<Mutex<StorageStats>>,
    is_running: Arc<Mutex<bool>>,
}

impl FileStorageManager {
    /// Create a new file storage manager
    pub async fn new(
        base_path: PathBuf,
        storage_backend: Arc<dyn StorageBackend>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Ensure base directory exists
        fs::create_dir_all(&base_path).await?;

        Ok(Self {
            base_path,
            files: Arc::new(RwLock::new(HashMap::new())),
            quotas: Arc::new(RwLock::new(HashMap::new())),
            virus_scanners: Arc::new(RwLock::new(Vec::new())),
            storage_backend,
            stats: Arc::new(Mutex::new(StorageStats::default())),
            is_running: Arc::new(Mutex::new(false)),
        })
    }

    /// Register a virus scanner
    pub async fn register_virus_scanner(&self, scanner: Arc<dyn VirusScanner>) {
        let mut scanners = self.virus_scanners.write().await;
        scanners.push(scanner);
    }

    /// Set storage quota for a file type
    pub async fn set_storage_quota(&self, quota: StorageQuota) {
        let mut quotas = self.quotas.write().await;
        quotas.insert(quota.storage_type.clone(), quota);
    }

    /// Upload and store a file
    pub async fn upload_file(
        &self,
        file_data: Vec<u8>,
        original_name: String,
        file_type: FileStorageType,
        mime_type: Option<String>,
        owner_id: Option<String>,
        tags: Option<HashMap<String, String>>,
        expires_at: Option<SystemTime>,
    ) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        // Check quota before upload
        self.check_quota(&file_type, file_data.len() as u64).await?;

        // Generate file ID and calculate checksum
        let file_id = Uuid::new_v4();
        let checksum = self.calculate_checksum(&file_data);

        // Create metadata
        let metadata = FileMetadata {
            id: file_id,
            original_name,
            stored_path: PathBuf::new(), // Will be set by storage backend
            file_type: file_type.clone(),
            size_bytes: file_data.len() as u64,
            mime_type,
            checksum,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at,
            owner_id,
            tags: tags.unwrap_or_default(),
            scan_result: None,
            access_count: 0,
        };

        // Store file using backend
        let stored_path = self.storage_backend.store_file(&file_data, &metadata).await?;
        
        let mut final_metadata = metadata;
        final_metadata.stored_path = stored_path.clone();

        // Scan file for viruses
        let scan_result = self.scan_file(&stored_path).await;
        final_metadata.scan_result = Some(scan_result.clone());

        // If file is malicious, delete it immediately
        if scan_result == ScanResult::Malicious {
            let _ = self.storage_backend.delete_file(&stored_path).await;
            return Err("File failed virus scan - malicious content detected".into());
        }

        let original_name = final_metadata.original_name.clone();
        
        // Store metadata
        {
            let mut files = self.files.write().await;
            files.insert(file_id, final_metadata);
        }

        // Update statistics
        self.update_stats_on_upload(&file_type, file_data.len() as u64, &scan_result).await;

        info!("Uploaded file {} ({} bytes) as {}", file_id, file_data.len(), original_name);
        Ok(file_id)
    }

    /// Retrieve a file
    pub async fn retrieve_file(&self, file_id: Uuid) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = {
            let mut files = self.files.write().await;
            if let Some(metadata) = files.get_mut(&file_id) {
                metadata.last_accessed = SystemTime::now();
                metadata.access_count += 1;
                metadata.stored_path.clone()
            } else {
                return Err("File not found".into());
            }
        };

        self.storage_backend.retrieve_file(&file_path).await
    }

    /// Get file metadata
    pub async fn get_file_metadata(&self, file_id: Uuid) -> Option<FileMetadata> {
        let files = self.files.read().await;
        files.get(&file_id).cloned()
    }

    /// Delete a file
    pub async fn delete_file(&self, file_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let metadata = {
            let mut files = self.files.write().await;
            files.remove(&file_id)
        };

        if let Some(metadata) = metadata {
            // Delete from storage backend
            self.storage_backend.delete_file(&metadata.stored_path).await?;
            
            // Update statistics
            self.update_stats_on_delete(&metadata.file_type, metadata.size_bytes).await;
            
            info!("Deleted file {} ({})", file_id, metadata.original_name);
            Ok(())
        } else {
            Err("File not found".into())
        }
    }

    /// List files by type
    pub async fn list_files_by_type(&self, file_type: &FileStorageType) -> Vec<FileMetadata> {
        let files = self.files.read().await;
        files.values()
            .filter(|f| &f.file_type == file_type)
            .cloned()
            .collect()
    }

    /// List files by owner
    pub async fn list_files_by_owner(&self, owner_id: &str) -> Vec<FileMetadata> {
        let files = self.files.read().await;
        files.values()
            .filter(|f| f.owner_id.as_ref() == Some(&owner_id.to_string()))
            .cloned()
            .collect()
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> StorageStats {
        let mut stats = self.stats.lock().await;
        
        // Update current file counts and sizes
        let files = self.files.read().await;
        stats.total_files = files.len();
        stats.total_size_bytes = files.values().map(|f| f.size_bytes).sum();
        
        stats.files_by_type.clear();
        stats.size_by_type.clear();
        
        for file in files.values() {
            *stats.files_by_type.entry(file.file_type.clone()).or_insert(0) += 1;
            *stats.size_by_type.entry(file.file_type.clone()).or_insert(0) += file.size_bytes;
        }
        
        stats.clone()
    }

    /// Start automatic cleanup process
    pub async fn start_cleanup_process(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        let files = Arc::clone(&self.files);
        let quotas = Arc::clone(&self.quotas);
        let storage_backend = Arc::clone(&self.storage_backend);
        let stats = Arc::clone(&self.stats);
        let running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::cleanup_loop(files, quotas, storage_backend, stats, running).await;
        });

        info!("Started file storage cleanup process");
        Ok(())
    }

    /// Stop automatic cleanup process
    pub async fn stop_cleanup_process(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("Stopped file storage cleanup process");
    }

    /// Check if upload would exceed quota
    async fn check_quota(&self, file_type: &FileStorageType, file_size: u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let quotas = self.quotas.read().await;
        if let Some(quota) = quotas.get(file_type) {
            // Check file size limit
            if file_size > quota.max_file_size {
                return Err(format!("File size {} exceeds limit {}", file_size, quota.max_file_size).into());
            }

            // Check total size and count limits
            let files = self.files.read().await;
            let current_files: Vec<_> = files.values().filter(|f| &f.file_type == file_type).collect();
            let current_count = current_files.len();
            let current_size: u64 = current_files.iter().map(|f| f.size_bytes).sum();

            if current_count >= quota.max_file_count {
                return Err(format!("File count {} exceeds limit {}", current_count, quota.max_file_count).into());
            }

            if current_size + file_size > quota.max_total_size {
                return Err(format!("Total size {} + {} exceeds limit {}", current_size, file_size, quota.max_total_size).into());
            }
        }

        Ok(())
    }

    /// Calculate file checksum
    fn calculate_checksum(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Scan file for viruses
    async fn scan_file(&self, file_path: &Path) -> ScanResult {
        let scanners = self.virus_scanners.read().await;
        
        if scanners.is_empty() {
            return ScanResult::Clean; // No scanners configured
        }

        for scanner in scanners.iter() {
            match scanner.scan_file(file_path).await {
                Ok(result) => {
                    if result != ScanResult::Clean {
                        return result;
                    }
                }
                Err(e) => {
                    warn!("Virus scan failed with {}: {}", scanner.get_scanner_name(), e);
                    return ScanResult::Error(e.to_string());
                }
            }
        }

        ScanResult::Clean
    }

    /// Update statistics on file upload
    async fn update_stats_on_upload(&self, file_type: &FileStorageType, size: u64, scan_result: &ScanResult) {
        let mut stats = self.stats.lock().await;
        *stats.scan_results.entry(scan_result.clone()).or_insert(0) += 1;
    }

    /// Update statistics on file deletion
    async fn update_stats_on_delete(&self, file_type: &FileStorageType, size: u64) {
        let mut stats = self.stats.lock().await;
        stats.files_cleaned += 1;
        stats.bytes_freed += size;
    }

    /// Cleanup loop for removing expired files
    async fn cleanup_loop(
        files: Arc<RwLock<HashMap<Uuid, FileMetadata>>>,
        quotas: Arc<RwLock<HashMap<FileStorageType, StorageQuota>>>,
        storage_backend: Arc<dyn StorageBackend>,
        stats: Arc<Mutex<StorageStats>>,
        is_running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(60)); // Check every minute
        
        while {
            let running = is_running.lock().await;
            *running
        } {
            cleanup_interval.tick().await;
            
            let now = SystemTime::now();
            let quotas_map = quotas.read().await;
            let mut files_to_cleanup = Vec::new();
            
            // Identify files that need cleanup
            {
                let files_map = files.read().await;
                for (id, file) in files_map.iter() {
                    let should_cleanup = if let Some(expires_at) = file.expires_at {
                        now > expires_at
                    } else if let Some(quota) = quotas_map.get(&file.file_type) {
                        if let Ok(age) = now.duration_since(file.created_at) {
                            age > quota.retention_period
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if should_cleanup {
                        files_to_cleanup.push((*id, file.clone()));
                    }
                }
            }

            // Perform cleanup
            let mut cleanup_count = 0;
            let mut bytes_freed = 0;

            for (id, file) in files_to_cleanup {
                // Delete from storage backend
                if let Err(e) = storage_backend.delete_file(&file.stored_path).await {
                    error!("Failed to delete file {}: {}", id, e);
                    continue;
                }

                // Remove from files map
                {
                    let mut files_map = files.write().await;
                    files_map.remove(&id);
                }

                cleanup_count += 1;
                bytes_freed += file.size_bytes;
                debug!("Cleaned up expired file {} ({})", id, file.original_name);
            }

            // Update statistics
            if cleanup_count > 0 {
                let mut stats_guard = stats.lock().await;
                stats_guard.files_cleaned += cleanup_count;
                stats_guard.bytes_freed += bytes_freed;
                
                info!("Cleanup cycle completed: {} files cleaned, {} bytes freed", cleanup_count, bytes_freed);
            }
        }
    }
}

/// Local filesystem storage backend
pub struct LocalFileSystemBackend {
    base_path: PathBuf,
}

impl LocalFileSystemBackend {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }
}

#[async_trait::async_trait]
impl StorageBackend for LocalFileSystemBackend {
    async fn store_file(&self, file_data: &[u8], metadata: &FileMetadata) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        // Create type-specific directory
        let type_dir = self.base_path.join(format!("{:?}", metadata.file_type).to_lowercase());
        fs::create_dir_all(&type_dir).await?;
        
        // Generate unique filename
        let filename = format!("{}_{}", metadata.id, metadata.original_name);
        let file_path = type_dir.join(filename);
        
        // Write file
        let mut file = File::create(&file_path).await?;
        file.write_all(file_data).await?;
        file.sync_all().await?;
        
        Ok(file_path)
    }

    async fn retrieve_file(&self, file_path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut file = File::open(file_path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;
        Ok(contents)
    }

    async fn delete_file(&self, file_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::remove_file(file_path).await?;
        Ok(())
    }

    async fn get_file_size(&self, file_path: &Path) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let metadata = fs::metadata(file_path).await?;
        Ok(metadata.len())
    }

    fn get_backend_name(&self) -> &str {
        "LocalFileSystem"
    }
}

/// Mock virus scanner for testing
pub struct MockVirusScanner {
    name: String,
    always_clean: bool,
}

impl MockVirusScanner {
    pub fn new(name: &str, always_clean: bool) -> Self {
        Self {
            name: name.to_string(),
            always_clean,
        }
    }
}

#[async_trait::async_trait]
impl VirusScanner for MockVirusScanner {
    async fn scan_file(&self, _file_path: &Path) -> Result<ScanResult, Box<dyn std::error::Error + Send + Sync>> {
        if self.always_clean {
            Ok(ScanResult::Clean)
        } else {
            // Simple heuristic: files with "virus" in name are malicious
            if let Some(filename) = _file_path.file_name().and_then(|n| n.to_str()) {
                if filename.to_lowercase().contains("virus") {
                    Ok(ScanResult::Malicious)
                } else if filename.to_lowercase().contains("suspicious") {
                    Ok(ScanResult::Suspicious)
                } else {
                    Ok(ScanResult::Clean)
                }
            } else {
                Ok(ScanResult::Clean)
            }
        }
    }

    fn get_scanner_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_storage_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
        
        let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
        let stats = manager.get_storage_stats().await;
        
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size_bytes, 0);
    }

    #[tokio::test]
    async fn test_file_upload_and_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
        let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
        
        let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
        manager.register_virus_scanner(scanner).await;
        
        let file_data = b"Hello, World!".to_vec();
        let file_id = manager.upload_file(
            file_data.clone(),
            "test.txt".to_string(),
            FileStorageType::Temporary,
            Some("text/plain".to_string()),
            Some("user123".to_string()),
            None,
            None,
        ).await.unwrap();
        
        let retrieved_data = manager.retrieve_file(file_id).await.unwrap();
        assert_eq!(retrieved_data, file_data);
        
        let metadata = manager.get_file_metadata(file_id).await.unwrap();
        assert_eq!(metadata.original_name, "test.txt");
        assert_eq!(metadata.size_bytes, 13);
        assert_eq!(metadata.owner_id, Some("user123".to_string()));
    }

    #[tokio::test]
    async fn test_virus_scanning() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
        let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
        
        let scanner = Arc::new(MockVirusScanner::new("test_scanner", false));
        manager.register_virus_scanner(scanner).await;
        
        // Upload clean file
        let clean_data = b"Clean file content".to_vec();
        let clean_id = manager.upload_file(
            clean_data,
            "clean.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await.unwrap();
        
        let clean_metadata = manager.get_file_metadata(clean_id).await.unwrap();
        assert_eq!(clean_metadata.scan_result, Some(ScanResult::Clean));
        
        // Try to upload malicious file
        let virus_data = b"Virus content".to_vec();
        let virus_result = manager.upload_file(
            virus_data,
            "virus.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await;
        
        assert!(virus_result.is_err());
        assert!(virus_result.unwrap_err().to_string().contains("malicious"));
    }

    #[tokio::test]
    async fn test_storage_quotas() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
        let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
        
        let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
        manager.register_virus_scanner(scanner).await;
        
        // Set restrictive quota
        let quota = StorageQuota {
            storage_type: FileStorageType::Temporary,
            max_total_size: 100,
            max_file_count: 2,
            max_file_size: 50,
            retention_period: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(10),
        };
        manager.set_storage_quota(quota).await;
        
        // Upload file within limits
        let small_data = b"Small".to_vec();
        let _id1 = manager.upload_file(
            small_data,
            "small1.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await.unwrap();
        
        // Upload another file within limits
        let small_data2 = b"Small2".to_vec();
        let _id2 = manager.upload_file(
            small_data2,
            "small2.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await.unwrap();
        
        // Try to upload file that exceeds count limit
        let small_data3 = b"Small3".to_vec();
        let result3 = manager.upload_file(
            small_data3,
            "small3.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await;
        
        assert!(result3.is_err());
        assert!(result3.unwrap_err().to_string().contains("count"));
        
        // Try to upload file that exceeds size limit
        let large_data = vec![0u8; 60];
        let result_large = manager.upload_file(
            large_data,
            "large.txt".to_string(),
            FileStorageType::Temporary,
            None,
            None,
            None,
            None,
        ).await;
        
        assert!(result_large.is_err());
        assert!(result_large.unwrap_err().to_string().contains("size"));
    }
}