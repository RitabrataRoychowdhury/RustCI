use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub description: String,
    pub up_script: String,
    pub down_script: String,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

impl Migration {
    pub fn new(
        id: &str,
        name: &str,
        version: u64,
        description: &str,
        up_script: &str,
        down_script: &str,
    ) -> Self {
        let content = format!("{}{}{}", name, up_script, down_script);
        let checksum = format!("{:x}", md5::compute(content.as_bytes()));

        Self {
            id: id.to_string(),
            name: name.to_string(),
            version,
            description: description.to_string(),
            up_script: up_script.to_string(),
            down_script: down_script.to_string(),
            checksum,
            created_at: Utc::now(),
            applied_at: None,
        }
    }

    pub fn is_applied(&self) -> bool {
        self.applied_at.is_some()
    }

    pub fn mark_applied(&mut self) {
        self.applied_at = Some(Utc::now());
    }
}

#[async_trait]
pub trait MigrationRunner: Send + Sync {
    async fn run_migration(&self, migration: &Migration) -> Result<(), MigrationError>;
    async fn rollback_migration(&self, migration: &Migration) -> Result<(), MigrationError>;
    async fn get_applied_migrations(&self) -> Result<Vec<Migration>, MigrationError>;
    async fn record_migration(&self, migration: &Migration) -> Result<(), MigrationError>;
    async fn remove_migration_record(&self, migration_id: &str) -> Result<(), MigrationError>;
}

pub struct MongoMigrationRunner {
    database_url: String,
    database_name: String,
}

impl MongoMigrationRunner {
    pub fn new(database_url: &str, database_name: &str) -> Self {
        Self {
            database_url: database_url.to_string(),
            database_name: database_name.to_string(),
        }
    }
}

#[async_trait]
impl MigrationRunner for MongoMigrationRunner {
    async fn run_migration(&self, migration: &Migration) -> Result<(), MigrationError> {
        println!("Running migration: {} (v{})", migration.name, migration.version);
        self.execute_script(&migration.up_script).await?;
        self.record_migration(migration).await?;
        println!("Migration completed: {}", migration.name);
        Ok(())
    }

    async fn rollback_migration(&self, migration: &Migration) -> Result<(), MigrationError> {
        println!("Rolling back migration: {} (v{})", migration.name, migration.version);
        self.execute_script(&migration.down_script).await?;
        self.remove_migration_record(&migration.id).await?;
        println!("Migration rolled back: {}", migration.name);
        Ok(())
    }

    async fn get_applied_migrations(&self) -> Result<Vec<Migration>, MigrationError> {
        Ok(Vec::new())
    }

    async fn record_migration(&self, migration: &Migration) -> Result<(), MigrationError> {
        println!("Recording migration: {}", migration.id);
        Ok(())
    }

    async fn remove_migration_record(&self, migration_id: &str) -> Result<(), MigrationError> {
        println!("Removing migration record: {}", migration_id);
        Ok(())
    }
}

impl MongoMigrationRunner {
    async fn execute_script(&self, script: &str) -> Result<(), MigrationError> {
        println!("Executing script: {}", script);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        if script.contains("FAIL") {
            return Err(MigrationError::ExecutionFailed(
                "Simulated script execution failure".to_string()
            ));
        }
        
        Ok(())
    }
}

pub struct MigrationManager {
    runner: Box<dyn MigrationRunner>,
    migrations: Vec<Migration>,
}

impl MigrationManager {
    pub fn new(runner: Box<dyn MigrationRunner>) -> Self {
        Self {
            runner,
            migrations: Vec::new(),
        }
    }

    pub fn add_migration(&mut self, migration: Migration) {
        self.migrations.push(migration);
        self.migrations.sort_by_key(|m| m.version);
    }

    pub async fn migrate_up(&self) -> Result<(), MigrationError> {
        println!("Starting database migration...");
        
        let applied_migrations = self.runner.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<u64> = applied_migrations
            .iter()
            .map(|m| m.version)
            .collect();

        let mut migrations_to_run = Vec::new();
        for migration in &self.migrations {
            if !applied_versions.contains(&migration.version) {
                migrations_to_run.push(migration);
            }
        }

        if migrations_to_run.is_empty() {
            println!("No migrations to run - database is up to date");
            return Ok(());
        }

        println!("Found {} migrations to run", migrations_to_run.len());

        for migration in migrations_to_run {
            self.verify_migration_checksum(migration)?;
            self.runner.run_migration(migration).await?;
        }

        println!("All migrations completed successfully");
        Ok(())
    }

    pub async fn migrate_down(&self, target_version: Option<u64>) -> Result<(), MigrationError> {
        println!("Starting database rollback...");
        
        let applied_migrations = self.runner.get_applied_migrations().await?;
        let mut rollback_migrations = Vec::new();

        for migration in applied_migrations.iter().rev() {
            if let Some(target) = target_version {
                if migration.version <= target {
                    break;
                }
            }
            rollback_migrations.push(migration);
        }

        if rollback_migrations.is_empty() {
            println!("No migrations to rollback");
            return Ok(());
        }

        println!("Found {} migrations to rollback", rollback_migrations.len());

        for migration in rollback_migrations {
            self.runner.rollback_migration(migration).await?;
        }

        println!("Rollback completed successfully");
        Ok(())
    }

    pub async fn get_migration_status(&self) -> Result<MigrationStatus, MigrationError> {
        let applied_migrations = self.runner.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<u64> = applied_migrations
            .iter()
            .map(|m| m.version)
            .collect();

        let mut pending_migrations = Vec::new();
        let mut applied_migration_info = Vec::new();

        for migration in &self.migrations {
            if applied_versions.contains(&migration.version) {
                if let Some(applied) = applied_migrations.iter().find(|m| m.version == migration.version) {
                    applied_migration_info.push(applied.clone());
                }
            } else {
                pending_migrations.push(migration.clone());
            }
        }

        Ok(MigrationStatus {
            applied_migrations: applied_migration_info,
            pending_migrations,
            current_version: applied_migrations.iter().map(|m| m.version).max(),
        })
    }

    fn verify_migration_checksum(&self, migration: &Migration) -> Result<(), MigrationError> {
        let content = format!("{}{}{}", migration.name, migration.up_script, migration.down_script);
        let calculated_checksum = format!("{:x}", md5::compute(content.as_bytes()));
        
        if calculated_checksum != migration.checksum {
            return Err(MigrationError::ChecksumMismatch {
                migration_id: migration.id.clone(),
                expected: migration.checksum.clone(),
                actual: calculated_checksum,
            });
        }
        
        Ok(())
    }
}

#[derive(Debug)]
pub struct MigrationStatus {
    pub applied_migrations: Vec<Migration>,
    pub pending_migrations: Vec<Migration>,
    pub current_version: Option<u64>,
}

impl MigrationStatus {
    pub fn print_status(&self) {
        println!("Migration Status Report");
        println!("{}", "=".repeat(50));
        
        if let Some(version) = self.current_version {
            println!("Current Version: {}", version);
        } else {
            println!("Current Version: None (fresh database)");
        }
        
        println!("Applied Migrations: {}", self.applied_migrations.len());
        for migration in &self.applied_migrations {
            println!("   v{}: {} (applied: {})", 
                migration.version, 
                migration.name,
                migration.applied_at.unwrap().to_rfc3339()
            );
        }
        
        println!("Pending Migrations: {}", self.pending_migrations.len());
        for migration in &self.pending_migrations {
            println!("   v{}: {}", migration.version, migration.name);
        }
        
        println!("{}", "=".repeat(50));
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Migration execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Checksum mismatch for migration {migration_id}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        migration_id: String,
        expected: String,
        actual: String,
    },
    
    #[error("Migration not found: {0}")]
    MigrationNotFound(String),
    
    #[error("Database connection failed: {0}")]
    DatabaseError(String),
    
    #[error("Migration already applied: {0}")]
    AlreadyApplied(String),
    
    #[error("Cannot rollback migration: {0}")]
    RollbackFailed(String),
}

pub fn create_initial_schema_migration() -> Migration {
    let up_script = "// Create initial database schema";
    let down_script = "// Drop all collections";
    
    Migration::new(
        "20240101_000001_initial_schema",
        "initial_schema",
        1,
        "Create initial database schema",
        up_script,
        down_script,
    )
}

pub fn create_add_audit_fields_migration() -> Migration {
    Migration::new(
        "20240102_000001_add_audit_fields",
        "add_audit_fields", 
        2,
        "Add audit fields to all collections",
        "// TODO: Add migration script",
        "// TODO: Add rollback script",
    )
}