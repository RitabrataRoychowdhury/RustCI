//! Tests for File Upload and Storage Management System

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use uuid::Uuid;

use rustci::core::resources::file_storage::{
    FileStorageManager, FileStorageType, FileMetadata, StorageQuota, ScanResult,
    VirusScanner, StorageBackend, LocalFileSystemBackend, MockVirusScanner,
};

/// Test virus scanner that can be configured for different behaviors
struct ConfigurableVirusScanner {
    name: String,
    scan_results: HashMap<String, ScanResult>,
    default_result: ScanResult,
}

impl ConfigurableVirusScanner {
    fn new(name: &str, default_result: ScanResult) -> Self {
        Self {
            name: name.to_string(),
            scan_results: HashMap::new(),
            default_result,
        }
    }

    fn add_scan_result(&mut self, filename_pattern: &str, result: ScanResult) {
        self.scan_results.insert(filename_pattern.to_string(), result);
    }
}

#[async_trait::async_trait]
impl VirusScanner for ConfigurableVirusScanner {
    async fn scan_file(&self, file_path: &Path) -> Result<ScanResult, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
            for (pattern, result) in &self.scan_results {
                if filename.contains(pattern) {
                    return Ok(result.clone());
                }
            }
        }
        Ok(self.default_result.clone())
    }

    fn get_scanner_name(&self) -> &str {
        &self.name
    }
}

#[tokio::test]
async fn test_file_storage_manager_creation() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    let stats = manager.get_storage_stats().await;
    
    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_size_bytes, 0);
    assert!(stats.files_by_type.is_empty());
    assert!(stats.size_by_type.is_empty());
}

#[tokio::test]
async fn test_file_upload_and_retrieval() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"Hello, World! This is test file content.".to_vec();
    let mut tags = HashMap::new();
    tags.insert("category".to_string(), "test".to_string());
    tags.insert("version".to_string(), "1.0".to_string());
    
    let file_id = manager.upload_file(
        file_data.clone(),
        "test_document.txt".to_string(),
        FileStorageType::Temporary,
        Some("text/plain".to_string()),
        Some("user123".to_string()),
        Some(tags.clone()),
        None,
    ).await.unwrap();
    
    // Verify file can be retrieved
    let retrieved_data = manager.retrieve_file(file_id).await.unwrap();
    assert_eq!(retrieved_data, file_data);
    
    // Verify metadata
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.id, file_id);
    assert_eq!(metadata.original_name, "test_document.txt");
    assert_eq!(metadata.file_type, FileStorageType::Temporary);
    assert_eq!(metadata.size_bytes, file_data.len() as u64);
    assert_eq!(metadata.mime_type, Some("text/plain".to_string()));
    assert_eq!(metadata.owner_id, Some("user123".to_string()));
    assert_eq!(metadata.tags.get("category"), Some(&"test".to_string()));
    assert_eq!(metadata.tags.get("version"), Some(&"1.0".to_string()));
    assert_eq!(metadata.scan_result, Some(ScanResult::Clean));
    assert_eq!(metadata.access_count, 1); // Should be incremented after retrieval
}

#[tokio::test]
async fn test_file_upload_with_expiration() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    let expires_at = SystemTime::now() + Duration::from_secs(3600); // 1 hour from now
    let file_data = b"Expiring file content".to_vec();
    
    let file_id = manager.upload_file(
        file_data,
        "expiring_file.txt".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        Some(expires_at),
    ).await.unwrap();
    
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.expires_at, Some(expires_at));
}

#[tokio::test]
async fn test_virus_scanning_clean_file() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let mut scanner = ConfigurableVirusScanner::new("test_scanner", ScanResult::Clean);
    let scanner = Arc::new(scanner);
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"Clean file content".to_vec();
    let file_id = manager.upload_file(
        file_data,
        "clean_file.txt".to_string(),
        FileStorageType::Permanent,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.scan_result, Some(ScanResult::Clean));
}

#[tokio::test]
async fn test_virus_scanning_malicious_file() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", false));
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"Malicious content".to_vec();
    let result = manager.upload_file(
        file_data,
        "virus_file.txt".to_string(), // MockVirusScanner treats files with "virus" as malicious
        FileStorageType::Upload,
        None,
        None,
        None,
        None,
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("malicious"));
}

#[tokio::test]
async fn test_virus_scanning_suspicious_file() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", false));
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"Potentially suspicious content".to_vec();
    let file_id = manager.upload_file(
        file_data,
        "suspicious_file.txt".to_string(), // MockVirusScanner treats files with "suspicious" as suspicious
        FileStorageType::Upload,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.scan_result, Some(ScanResult::Suspicious));
}

#[tokio::test]
async fn test_storage_quota_file_size_limit() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Set quota with small file size limit
    let quota = StorageQuota {
        storage_type: FileStorageType::Upload,
        max_total_size: 1024 * 1024, // 1MB
        max_file_count: 100,
        max_file_size: 50, // 50 bytes
        retention_period: Duration::from_secs(3600),
        cleanup_interval: Duration::from_secs(300),
    };
    manager.set_storage_quota(quota).await;
    
    // Try to upload file that exceeds size limit
    let large_data = vec![0u8; 100]; // 100 bytes
    let result = manager.upload_file(
        large_data,
        "large_file.bin".to_string(),
        FileStorageType::Upload,
        None,
        None,
        None,
        None,
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("size") && result.unwrap_err().to_string().contains("exceeds"));
}

#[tokio::test]
async fn test_storage_quota_file_count_limit() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Set quota with small file count limit
    let quota = StorageQuota {
        storage_type: FileStorageType::Cache,
        max_total_size: 1024 * 1024,
        max_file_count: 2, // Only 2 files allowed
        max_file_size: 1024,
        retention_period: Duration::from_secs(3600),
        cleanup_interval: Duration::from_secs(300),
    };
    manager.set_storage_quota(quota).await;
    
    // Upload first file
    let file_data1 = b"File 1".to_vec();
    let _id1 = manager.upload_file(
        file_data1,
        "file1.txt".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Upload second file
    let file_data2 = b"File 2".to_vec();
    let _id2 = manager.upload_file(
        file_data2,
        "file2.txt".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Try to upload third file (should fail)
    let file_data3 = b"File 3".to_vec();
    let result3 = manager.upload_file(
        file_data3,
        "file3.txt".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        None,
    ).await;
    
    assert!(result3.is_err());
    assert!(result3.unwrap_err().to_string().contains("count") && result3.unwrap_err().to_string().contains("exceeds"));
}

#[tokio::test]
async fn test_storage_quota_total_size_limit() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Set quota with small total size limit
    let quota = StorageQuota {
        storage_type: FileStorageType::Backup,
        max_total_size: 100, // 100 bytes total
        max_file_count: 10,
        max_file_size: 60,
        retention_period: Duration::from_secs(3600),
        cleanup_interval: Duration::from_secs(300),
    };
    manager.set_storage_quota(quota).await;
    
    // Upload first file
    let file_data1 = vec![0u8; 50]; // 50 bytes
    let _id1 = manager.upload_file(
        file_data1,
        "file1.bin".to_string(),
        FileStorageType::Backup,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Try to upload second file that would exceed total size
    let file_data2 = vec![0u8; 60]; // 60 bytes (50 + 60 = 110 > 100)
    let result2 = manager.upload_file(
        file_data2,
        "file2.bin".to_string(),
        FileStorageType::Backup,
        None,
        None,
        None,
        None,
    ).await;
    
    assert!(result2.is_err());
    assert!(result2.unwrap_err().to_string().contains("Total size"));
}

#[tokio::test]
async fn test_file_deletion() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"File to be deleted".to_vec();
    let file_id = manager.upload_file(
        file_data,
        "delete_me.txt".to_string(),
        FileStorageType::Temporary,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Verify file exists
    assert!(manager.get_file_metadata(file_id).await.is_some());
    
    // Delete file
    manager.delete_file(file_id).await.unwrap();
    
    // Verify file is gone
    assert!(manager.get_file_metadata(file_id).await.is_none());
    
    // Try to retrieve deleted file
    let retrieve_result = manager.retrieve_file(file_id).await;
    assert!(retrieve_result.is_err());
}

#[tokio::test]
async fn test_file_deletion_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let nonexistent_id = Uuid::new_v4();
    let result = manager.delete_file(nonexistent_id).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_list_files_by_type() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Upload files of different types
    let _temp_id = manager.upload_file(
        b"Temp file".to_vec(),
        "temp.txt".to_string(),
        FileStorageType::Temporary,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let _perm_id = manager.upload_file(
        b"Permanent file".to_vec(),
        "perm.txt".to_string(),
        FileStorageType::Permanent,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let _cache_id = manager.upload_file(
        b"Cache file".to_vec(),
        "cache.txt".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // List files by type
    let temp_files = manager.list_files_by_type(&FileStorageType::Temporary).await;
    let perm_files = manager.list_files_by_type(&FileStorageType::Permanent).await;
    let cache_files = manager.list_files_by_type(&FileStorageType::Cache).await;
    
    assert_eq!(temp_files.len(), 1);
    assert_eq!(perm_files.len(), 1);
    assert_eq!(cache_files.len(), 1);
    
    assert_eq!(temp_files[0].original_name, "temp.txt");
    assert_eq!(perm_files[0].original_name, "perm.txt");
    assert_eq!(cache_files[0].original_name, "cache.txt");
}

#[tokio::test]
async fn test_list_files_by_owner() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Upload files with different owners
    let _user1_file1 = manager.upload_file(
        b"User 1 file 1".to_vec(),
        "user1_file1.txt".to_string(),
        FileStorageType::Upload,
        None,
        Some("user1".to_string()),
        None,
        None,
    ).await.unwrap();
    
    let _user1_file2 = manager.upload_file(
        b"User 1 file 2".to_vec(),
        "user1_file2.txt".to_string(),
        FileStorageType::Upload,
        None,
        Some("user1".to_string()),
        None,
        None,
    ).await.unwrap();
    
    let _user2_file = manager.upload_file(
        b"User 2 file".to_vec(),
        "user2_file.txt".to_string(),
        FileStorageType::Upload,
        None,
        Some("user2".to_string()),
        None,
        None,
    ).await.unwrap();
    
    // List files by owner
    let user1_files = manager.list_files_by_owner("user1").await;
    let user2_files = manager.list_files_by_owner("user2").await;
    let user3_files = manager.list_files_by_owner("user3").await;
    
    assert_eq!(user1_files.len(), 2);
    assert_eq!(user2_files.len(), 1);
    assert_eq!(user3_files.len(), 0);
    
    assert!(user1_files.iter().all(|f| f.owner_id.as_ref() == Some(&"user1".to_string())));
    assert!(user2_files.iter().all(|f| f.owner_id.as_ref() == Some(&"user2".to_string())));
}

#[tokio::test]
async fn test_storage_statistics() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    // Upload files of different types and sizes
    let _temp_id = manager.upload_file(
        vec![0u8; 100],
        "temp.bin".to_string(),
        FileStorageType::Temporary,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let _cache_id = manager.upload_file(
        vec![0u8; 200],
        "cache.bin".to_string(),
        FileStorageType::Cache,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let _upload_id = manager.upload_file(
        vec![0u8; 150],
        "upload.bin".to_string(),
        FileStorageType::Upload,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let stats = manager.get_storage_stats().await;
    
    assert_eq!(stats.total_files, 3);
    assert_eq!(stats.total_size_bytes, 450);
    
    assert_eq!(stats.files_by_type.get(&FileStorageType::Temporary), Some(&1));
    assert_eq!(stats.files_by_type.get(&FileStorageType::Cache), Some(&1));
    assert_eq!(stats.files_by_type.get(&FileStorageType::Upload), Some(&1));
    
    assert_eq!(stats.size_by_type.get(&FileStorageType::Temporary), Some(&100));
    assert_eq!(stats.size_by_type.get(&FileStorageType::Cache), Some(&200));
    assert_eq!(stats.size_by_type.get(&FileStorageType::Upload), Some(&150));
}

#[tokio::test]
async fn test_file_access_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    let scanner = Arc::new(MockVirusScanner::new("test_scanner", true));
    manager.register_virus_scanner(scanner).await;
    
    let file_data = b"Access tracking test".to_vec();
    let file_id = manager.upload_file(
        file_data,
        "access_test.txt".to_string(),
        FileStorageType::Temporary,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Initial access count should be 0
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.access_count, 0);
    
    // Retrieve file multiple times
    for _ in 0..3 {
        let _ = manager.retrieve_file(file_id).await.unwrap();
    }
    
    // Access count should be incremented
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.access_count, 3);
}

#[tokio::test]
async fn test_multiple_virus_scanners() {
    let temp_dir = TempDir::new().unwrap();
    let backend = Arc::new(LocalFileSystemBackend::new(temp_dir.path().to_path_buf()));
    let manager = FileStorageManager::new(temp_dir.path().to_path_buf(), backend).await.unwrap();
    
    // Register multiple scanners
    let scanner1 = Arc::new(MockVirusScanner::new("scanner1", true));
    let scanner2 = Arc::new(MockVirusScanner::new("scanner2", true));
    manager.register_virus_scanner(scanner1).await;
    manager.register_virus_scanner(scanner2).await;
    
    let file_data = b"Multi-scanner test".to_vec();
    let file_id = manager.upload_file(
        file_data,
        "multi_scan.txt".to_string(),
        FileStorageType::Temporary,
        None,
        None,
        None,
        None,
    ).await.unwrap();
    
    let metadata = manager.get_file_metadata(file_id).await.unwrap();
    assert_eq!(metadata.scan_result, Some(ScanResult::Clean));
}