#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfiguration;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};
    use tokio::time::{sleep, Duration};

    fn create_test_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "test_secret_that_is_long_enough_for_validation".to_string();
        config.security.encryption.key = "dGVzdF9lbmNyeXB0aW9uX2tleV90aGF0X2lzXzMyX2J5dGVz".to_string();
        config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
        config
    }

    fn create_hot_reload_config() -> HotReloadConfig {
        HotReloadConfig {
            enabled: true,
            validation_timeout_seconds: 10,
            max_backup_configs: 5,
            debounce_delay_ms: 100,
            auto_rollback_on_failure: true,
            notification_channels: vec![
                NotificationChannel {
                    name: "test_log".to_string(),
                    channel_type: NotificationChannelType::Log,
                    config: HashMap::new(),
                    events: vec![HotReloadEvent::ConfigChanged, HotReloadEvent::ValidationFailed],
                }
            ],
            validation_rules: ValidationRules {
                require_validation_before_apply: true,
                fail_on_warnings: false,
                custom_validation_scripts: Vec::new(),
                schema_validation_enabled: true,
                environment_specific_validation: true,
            },
            file_watch_patterns: vec!["*.yaml".to_string()],
            environment_overrides: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_production_hot_reload_manager_creation() {
        let hot_reload_config = create_hot_reload_config();
        let manager = ProductionHotReloadManager::new("test", Some(hot_reload_config));
        
        let config = manager.get_config().await;
        assert_eq!(config.server.port, 8000); // Default port
    }

    #[tokio::test]
    async fn test_load_and_validate() {
        let hot_reload_config = create_hot_reload_config();
        let mut manager = ProductionHotReloadManager::new("test", Some(hot_reload_config));
        
        // This will use environment variables and defaults
        let validation_report = manager.load_and_validate().await.unwrap();
        
        // Should have created at least one backup
        let backups = manager.get_backups().await;
        assert!(!backups.is_empty());
        assert_eq!(backups[0].description, "Initial configuration load");
    }

    #[tokio::test]
    async fn test_config_file_parsing() {
        // Test YAML parsing
        let yaml_content = r#"
server:
  host: "127.0.0.1"
  port: 9000
database:
  mongodb_uri: "mongodb://test:27017/testdb"
security:
  jwt:
    secret: "test_secret_that_is_long_enough_for_validation"
"#;
        
        let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        let config_path = temp_file.path().to_path_buf();
        
        let config = ProductionHotReloadManager::parse_config_content(yaml_content, &config_path).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.database.mongodb_uri, "mongodb://test:27017/testdb");

        // Test JSON parsing
        let json_content = r#"
{
  "server": {
    "host": "0.0.0.0",
    "port": 8080
  },
  "database": {
    "mongodb_uri": "mongodb://json:27017/jsondb"
  },
  "security": {
    "jwt": {
      "secret": "json_test_secret_that_is_long_enough_for_validation"
    }
  }
}
"#;
        
        let temp_json_file = NamedTempFile::with_suffix(".json").unwrap();
        let json_config_path = temp_json_file.path().to_path_buf();
        
        let json_config = ProductionHotReloadManager::parse_config_content(json_content, &json_config_path).unwrap();
        assert_eq!(json_config.server.host, "0.0.0.0");
        assert_eq!(json_config.server.port, 8080);
        assert_eq!(json_config.database.mongodb_uri, "mongodb://json:27017/jsondb");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = create_test_config();
        
        // Valid configuration should pass
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_ok());
        
        // Invalid configurations should fail
        config.security.jwt.secret = "".to_string();
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_err());
        
        config.security.jwt.secret = "valid_secret".to_string();
        config.database.mongodb_uri = "".to_string();
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_err());
        
        config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
        config.server.port = 0;
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_err());
    }

    #[tokio::test]
    async fn test_config_hash_calculation() {
        let config1 = create_test_config();
        let config2 = create_test_config();
        let mut config3 = create_test_config();
        config3.server.port = 9000;

        let hash1 = ProductionHotReloadManager::calculate_config_hash(&config1);
        let hash2 = ProductionHotReloadManager::calculate_config_hash(&config2);
        let hash3 = ProductionHotReloadManager::calculate_config_hash(&config3);

        assert_eq!(hash1, hash2); // Same configs should have same hash
        assert_ne!(hash1, hash3); // Different configs should have different hashes
    }

    #[tokio::test]
    async fn test_validation_cache() {
        let validation_cache = Arc::new(RwLock::new(ValidationCache {
            entries: HashMap::new(),
        }));
        
        let config = create_test_config();
        let config_hash = ProductionHotReloadManager::calculate_config_hash(&config);
        let validation_report = ProductionValidationReport::new("test");

        // Initially no cache
        let cached = ProductionHotReloadManager::check_validation_cache(&validation_cache, config_hash).await;
        assert!(cached.is_none());

        // Cache the result
        ProductionHotReloadManager::cache_validation_result(&validation_cache, config_hash, &validation_report).await;

        // Should now find cached result
        let cached = ProductionHotReloadManager::check_validation_cache(&validation_cache, config_hash).await;
        assert!(cached.is_some());
        
        let cached_result = cached.unwrap();
        assert_eq!(cached_result.config_hash, config_hash);
    }

    #[tokio::test]
    async fn test_change_detection_with_impact() {
        let change_detector = ChangeDetector::new();
        
        let old_config = create_test_config();
        let mut new_config = old_config.clone();
        
        // Make some changes
        new_config.server.port = 9000; // Should be critical impact
        new_config.security.jwt.secret = "new_secret_that_is_long_enough_for_validation".to_string(); // Should be high impact
        new_config.observability.logging.level = "debug".to_string(); // Should be low impact

        let changes = change_detector.detect_changes_with_impact(&old_config, &new_config).unwrap();
        
        assert!(!changes.is_empty());
        
        // Find the port change
        let port_change = changes.iter().find(|c| c.field_path == "server.port");
        assert!(port_change.is_some());
        let port_change = port_change.unwrap();
        assert!(matches!(port_change.impact_assessment.severity, ImpactSeverity::Critical));
        assert!(port_change.impact_assessment.requires_restart);
        
        // Find the JWT secret change
        let jwt_change = changes.iter().find(|c| c.field_path == "security.jwt.secret");
        assert!(jwt_change.is_some());
        let jwt_change = jwt_change.unwrap();
        assert!(matches!(jwt_change.impact_assessment.severity, ImpactSeverity::High));
    }

    #[tokio::test]
    async fn test_config_backup_creation() {
        let hot_reload_config = create_hot_reload_config();
        let manager = ProductionHotReloadManager::new("test", Some(hot_reload_config));
        
        let config = create_test_config();
        let validation_report = ProductionValidationReport::new("test");
        
        manager.create_backup(
            config.clone(),
            validation_report,
            ConfigChangeSource::Automatic,
            "Test backup".to_string(),
        ).await.unwrap();
        
        let backups = manager.get_backups().await;
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].description, "Test backup");
        assert_eq!(backups[0].config.server.port, config.server.port);
    }

    #[tokio::test]
    async fn test_rollback_manager() {
        let rollback_manager = RollbackManager::new(5, true);
        
        // Create some test backups
        let backup_configs = Arc::new(RwLock::new(Vec::new()));
        let config_arc = Arc::new(RwLock::new(create_test_config()));
        let (change_sender, _) = broadcast::channel(100);
        
        // Add a backup
        let backup = ConfigBackup {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            config: {
                let mut config = create_test_config();
                config.server.port = 9000;
                config
            },
            validation_report: ProductionValidationReport::new("test"),
            source: ConfigChangeSource::Automatic,
            description: "Test backup".to_string(),
            tags: HashMap::new(),
        };
        
        let backup_id = backup.id;
        {
            let mut backups = backup_configs.write().await;
            backups.push(backup);
        }
        
        // Test rollback to specific backup
        rollback_manager.rollback_to_backup(
            backup_id,
            &backup_configs,
            &config_arc,
            &change_sender,
        ).await.unwrap();
        
        // Verify the configuration was rolled back
        let current_config = config_arc.read().await;
        assert_eq!(current_config.server.port, 9000);
    }

    #[tokio::test]
    async fn test_notification_system() {
        let channels = vec![
            NotificationChannel {
                name: "test_log".to_string(),
                channel_type: NotificationChannelType::Log,
                config: HashMap::new(),
                events: vec![HotReloadEvent::ConfigChanged],
            }
        ];
        
        let notification_system = NotificationSystem::new(channels);
        
        // This should not fail
        let result = notification_system.notify(
            HotReloadEvent::ConfigChanged,
            "Test notification message",
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_change_event_subscription() {
        let hot_reload_config = create_hot_reload_config();
        let manager = ProductionHotReloadManager::new("test", Some(hot_reload_config));
        
        let mut change_receiver = manager.subscribe_to_changes();
        
        // This test would require actually triggering a config change
        // For now, we just verify that we can create a subscription
        assert!(change_receiver.try_recv().is_err()); // Should be empty initially
    }

    #[tokio::test]
    async fn test_hot_reload_config_defaults() {
        let config = HotReloadConfig::default();
        
        assert!(config.enabled);
        assert_eq!(config.validation_timeout_seconds, 30);
        assert_eq!(config.max_backup_configs, 10);
        assert_eq!(config.debounce_delay_ms, 500);
        assert!(config.auto_rollback_on_failure);
        assert!(!config.notification_channels.is_empty());
        assert!(config.validation_rules.require_validation_before_apply);
        assert!(!config.validation_rules.fail_on_warnings);
    }

    #[tokio::test]
    async fn test_impact_rule_matching() {
        let change_detector = ChangeDetector::new();
        
        // Test pattern matching
        assert!(change_detector.matches_pattern("security.jwt.secret", "security.*"));
        assert!(change_detector.matches_pattern("database.mongodb_uri", "database.*"));
        assert!(change_detector.matches_pattern("server.port", "server.port"));
        assert!(!change_detector.matches_pattern("server.host", "server.port"));
        assert!(!change_detector.matches_pattern("logging.level", "security.*"));
    }

    #[tokio::test]
    async fn test_config_change_metadata() {
        let metadata = ConfigChangeMetadata {
            hostname: "test-host".to_string(),
            process_id: 12345,
            environment: "test".to_string(),
            git_commit: Some("abc123".to_string()),
            deployment_id: Some("deploy-456".to_string()),
            correlation_id: uuid::Uuid::new_v4(),
        };
        
        assert_eq!(metadata.hostname, "test-host");
        assert_eq!(metadata.process_id, 12345);
        assert_eq!(metadata.environment, "test");
        assert_eq!(metadata.git_commit, Some("abc123".to_string()));
        assert_eq!(metadata.deployment_id, Some("deploy-456".to_string()));
    }

    #[tokio::test]
    async fn test_change_types() {
        let change_detector = ChangeDetector::new();
        
        let old_config = create_test_config();
        let mut new_config = old_config.clone();
        
        // Test different change types
        new_config.server.port = 9000; // Modified
        new_config.server.host = "new-host".to_string(); // Modified
        
        let changes = change_detector.detect_changes_with_impact(&old_config, &new_config).unwrap();
        
        for change in &changes {
            match change.field_path.as_str() {
                "server.port" | "server.host" => {
                    assert!(matches!(change.change_type, ChangeType::Modified));
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_value_type_detection() {
        use serde_json::Value;
        
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Bool(true).type_name(), "boolean");
        assert_eq!(Value::Number(serde_json::Number::from(42)).type_name(), "number");
        assert_eq!(Value::String("test".to_string()).type_name(), "string");
        assert_eq!(Value::Array(vec![]).type_name(), "array");
        assert_eq!(Value::Object(serde_json::Map::new()).type_name(), "object");
    }
}