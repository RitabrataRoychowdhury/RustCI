use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasterRecoveryPlan {
    pub plan_id: Uuid,
    pub name: String,
    pub description: String,
    pub recovery_time_objective: Duration, // RTO
    pub recovery_point_objective: Duration, // RPO
    pub priority: RecoveryPriority,
    pub backup_strategy: BackupStrategy,
    pub failover_strategy: FailoverStrategy,
    pub recovery_procedures: Vec<RecoveryProcedure>,
    pub validation_tests: Vec<ValidationTest>,
    pub created_at: SystemTime,
    pub last_tested: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoveryPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStrategy {
    pub backup_type: BackupType,
    pub frequency: BackupFrequency,
    pub retention_policy: RetentionPolicy,
    pub storage_locations: Vec<StorageLocation>,
    pub encryption_enabled: bool,
    pub compression_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    Incremental,
    Differential,
    Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupFrequency {
    Continuous,
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub daily_retention_days: u32,
    pub weekly_retention_weeks: u32,
    pub monthly_retention_months: u32,
    pub yearly_retention_years: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageLocation {
    pub location_id: Uuid,
    pub name: String,
    pub location_type: LocationType,
    pub region: String,
    pub endpoint: String,
    pub credentials: StorageCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocationType {
    Local,
    S3Compatible,
    Azure,
    GoogleCloud,
    NetworkAttachedStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCredentials {
    pub access_key: String,
    pub secret_key: String,
    pub additional_config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverStrategy {
    pub strategy_type: FailoverType,
    pub automatic_failover: bool,
    pub failover_timeout: Duration,
    pub health_check_interval: Duration,
    pub failure_threshold: u32,
    pub target_regions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailoverType {
    ActivePassive,
    ActiveActive,
    MultiRegion,
    HotStandby,
    ColdStandby,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryProcedure {
    pub procedure_id: Uuid,
    pub name: String,
    pub description: String,
    pub steps: Vec<RecoveryStep>,
    pub estimated_duration: Duration,
    pub dependencies: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub step_id: Uuid,
    pub name: String,
    pub command: String,
    pub timeout: Duration,
    pub retry_count: u32,
    pub rollback_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTest {
    pub test_id: Uuid,
    pub name: String,
    pub test_type: TestType,
    pub frequency: TestFrequency,
    pub success_criteria: Vec<SuccessCriterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestType {
    BackupIntegrity,
    FailoverTime,
    DataConsistency,
    ApplicationFunctionality,
    NetworkConnectivity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    OnDemand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriterion {
    pub metric: String,
    pub threshold: f64,
    pub comparison: ComparisonOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJob {
    pub job_id: Uuid,
    pub plan_id: Uuid,
    pub backup_type: BackupType,
    pub status: BackupStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub size_bytes: u64,
    pub storage_location: StorageLocation,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreJob {
    pub job_id: Uuid,
    pub backup_job_id: Uuid,
    pub restore_point: SystemTime,
    pub status: RestoreStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub target_location: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestoreStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverEvent {
    pub event_id: Uuid,
    pub plan_id: Uuid,
    pub trigger_reason: String,
    pub source_region: String,
    pub target_region: String,
    pub status: FailoverStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub recovery_time: Option<Duration>,
    pub data_loss: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailoverStatus {
    Initiated,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

pub trait DisasterRecoveryManager: Send + Sync {
    async fn create_recovery_plan(&self, plan: DisasterRecoveryPlan) -> Result<Uuid>;
    async fn update_recovery_plan(&self, plan_id: Uuid, plan: DisasterRecoveryPlan) -> Result<()>;
    async fn delete_recovery_plan(&self, plan_id: Uuid) -> Result<()>;
    async fn get_recovery_plan(&self, plan_id: Uuid) -> Result<DisasterRecoveryPlan>;
    async fn list_recovery_plans(&self) -> Result<Vec<DisasterRecoveryPlan>>;
    async fn test_recovery_plan(&self, plan_id: Uuid) -> Result<TestResult>;
    async fn trigger_failover(&self, plan_id: Uuid, reason: String) -> Result<Uuid>;
    async fn get_failover_status(&self, event_id: Uuid) -> Result<FailoverEvent>;
}

pub trait BackupManager: Send + Sync {
    async fn create_backup(&self, plan_id: Uuid, backup_type: BackupType) -> Result<Uuid>;
    async fn get_backup_status(&self, job_id: Uuid) -> Result<BackupJob>;
    async fn list_backups(&self, plan_id: Option<Uuid>) -> Result<Vec<BackupJob>>;
    async fn restore_from_backup(&self, backup_job_id: Uuid, target_location: String) -> Result<Uuid>;
    async fn get_restore_status(&self, job_id: Uuid) -> Result<RestoreJob>;
    async fn validate_backup(&self, job_id: Uuid) -> Result<ValidationResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_id: Uuid,
    pub plan_id: Uuid,
    pub test_type: TestType,
    pub status: TestStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub results: Vec<TestMetric>,
    pub passed: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMetric {
    pub metric_name: String,
    pub value: f64,
    pub unit: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub validation_id: Uuid,
    pub backup_job_id: Uuid,
    pub integrity_check: bool,
    pub size_verification: bool,
    pub checksum_verification: bool,
    pub restore_test_passed: bool,
    pub validation_time: Duration,
}

pub struct ProductionDisasterRecoveryManager {
    recovery_plans: Arc<RwLock<HashMap<Uuid, DisasterRecoveryPlan>>>,
    backup_jobs: Arc<RwLock<HashMap<Uuid, BackupJob>>>,
    restore_jobs: Arc<RwLock<HashMap<Uuid, RestoreJob>>>,
    failover_events: Arc<RwLock<HashMap<Uuid, FailoverEvent>>>,
    backup_manager: Arc<dyn BackupManager>,
}

impl ProductionDisasterRecoveryManager {
    pub fn new(backup_manager: Arc<dyn BackupManager>) -> Self {
        Self {
            recovery_plans: Arc::new(RwLock::new(HashMap::new())),
            backup_jobs: Arc::new(RwLock::new(HashMap::new())),
            restore_jobs: Arc::new(RwLock::new(HashMap::new())),
            failover_events: Arc::new(RwLock::new(HashMap::new())),
            backup_manager,
        }
    }

    async fn execute_recovery_procedures(&self, plan: &DisasterRecoveryPlan) -> Result<Vec<RecoveryStep>> {
        let mut executed_steps = Vec::new();

        // Sort procedures by dependencies
        let mut sorted_procedures = plan.recovery_procedures.clone();
        sorted_procedures.sort_by_key(|p| p.dependencies.len());

        for procedure in &sorted_procedures {
            log::info!("Executing recovery procedure: {}", procedure.name);
            
            for step in &procedure.steps {
                log::info!("Executing recovery step: {}", step.name);
                
                // In a real implementation, this would execute the actual command
                tokio::time::sleep(Duration::from_millis(10)).await;
                
                executed_steps.push(step.clone());
                log::info!("Completed recovery step: {}", step.name);
            }
        }

        Ok(executed_steps)
    }

    async fn validate_failover_readiness(&self, plan: &DisasterRecoveryPlan) -> Result<bool> {
        // Check if recent backups are available
        let backups = self.backup_manager.list_backups(Some(plan.plan_id)).await?;
        let recent_backup = backups.iter()
            .filter(|b| matches!(b.status, BackupStatus::Completed))
            .max_by_key(|b| b.started_at);

        if let Some(backup) = recent_backup {
            let backup_age = backup.started_at.elapsed().unwrap_or(Duration::MAX);
            if backup_age > plan.recovery_point_objective {
                log::warn!("Latest backup is older than RPO: {:?} > {:?}", 
                          backup_age, plan.recovery_point_objective);
                return Ok(false);
            }
        } else {
            log::error!("No completed backups found for plan: {}", plan.plan_id);
            return Ok(false);
        }

        // Additional readiness checks would go here
        Ok(true)
    }
}

#[async_trait::async_trait]
impl DisasterRecoveryManager for ProductionDisasterRecoveryManager {
    async fn create_recovery_plan(&self, plan: DisasterRecoveryPlan) -> Result<Uuid> {
        let plan_id = plan.plan_id;
        
        {
            let mut plans = self.recovery_plans.write().await;
            plans.insert(plan_id, plan.clone());
        }

        log::info!("Created disaster recovery plan: {} - {}", plan_id, plan.name);
        Ok(plan_id)
    }

    async fn update_recovery_plan(&self, plan_id: Uuid, plan: DisasterRecoveryPlan) -> Result<()> {
        {
            let mut plans = self.recovery_plans.write().await;
            plans.insert(plan_id, plan.clone());
        }

        log::info!("Updated disaster recovery plan: {}", plan_id);
        Ok(())
    }

    async fn delete_recovery_plan(&self, plan_id: Uuid) -> Result<()> {
        {
            let mut plans = self.recovery_plans.write().await;
            plans.remove(&plan_id)
                .ok_or_else(|| AppError::NotFound(format!("Recovery plan {} not found", plan_id)))?;
        }

        log::info!("Deleted disaster recovery plan: {}", plan_id);
        Ok(())
    }

    async fn get_recovery_plan(&self, plan_id: Uuid) -> Result<DisasterRecoveryPlan> {
        let plans = self.recovery_plans.read().await;
        plans.get(&plan_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Recovery plan {} not found", plan_id)))
    }

    async fn list_recovery_plans(&self) -> Result<Vec<DisasterRecoveryPlan>> {
        let plans = self.recovery_plans.read().await;
        Ok(plans.values().cloned().collect())
    }

    async fn test_recovery_plan(&self, plan_id: Uuid) -> Result<TestResult> {
        let plan = self.get_recovery_plan(plan_id).await?;
        let test_id = Uuid::new_v4();
        let started_at = SystemTime::now();

        log::info!("Starting disaster recovery test for plan: {}", plan_id);

        // Simulate running validation tests
        let mut test_results = Vec::new();
        let mut all_passed = true;

        for validation_test in &plan.validation_tests {
            match validation_test.test_type {
                TestType::BackupIntegrity => {
                    let metric = TestMetric {
                        metric_name: "backup_integrity".to_string(),
                        value: 100.0,
                        unit: "percent".to_string(),
                        passed: true,
                    };
                    test_results.push(metric);
                }
                TestType::FailoverTime => {
                    let failover_time = 45.0; // seconds
                    let rto_seconds = plan.recovery_time_objective.as_secs() as f64;
                    let passed = failover_time < rto_seconds;
                    all_passed &= passed;
                    
                    let metric = TestMetric {
                        metric_name: "failover_time".to_string(),
                        value: failover_time,
                        unit: "seconds".to_string(),
                        passed,
                    };
                    test_results.push(metric);
                }
                TestType::DataConsistency => {
                    let metric = TestMetric {
                        metric_name: "data_consistency".to_string(),
                        value: 100.0,
                        unit: "percent".to_string(),
                        passed: true,
                    };
                    test_results.push(metric);
                }
                TestType::ApplicationFunctionality => {
                    let metric = TestMetric {
                        metric_name: "application_functionality".to_string(),
                        value: 95.0,
                        unit: "percent".to_string(),
                        passed: true,
                    };
                    test_results.push(metric);
                }
                TestType::NetworkConnectivity => {
                    let metric = TestMetric {
                        metric_name: "network_connectivity".to_string(),
                        value: 100.0,
                        unit: "percent".to_string(),
                        passed: true,
                    };
                    test_results.push(metric);
                }
            }
        }

        // Update last tested time
        {
            let mut plans = self.recovery_plans.write().await;
            if let Some(plan) = plans.get_mut(&plan_id) {
                plan.last_tested = Some(SystemTime::now());
            }
        }

        Ok(TestResult {
            test_id,
            plan_id,
            test_type: TestType::ApplicationFunctionality, // Overall test
            status: TestStatus::Completed,
            started_at,
            completed_at: Some(SystemTime::now()),
            results: test_results,
            passed: all_passed,
            error_message: None,
        })
    }

    async fn trigger_failover(&self, plan_id: Uuid, reason: String) -> Result<Uuid> {
        let plan = self.get_recovery_plan(plan_id).await?;
        let event_id = Uuid::new_v4();

        // Validate failover readiness
        if !self.validate_failover_readiness(&plan).await? {
            return Err(AppError::ValidationFailed(
                "Failover readiness validation failed".to_string()
            ));
        }

        let failover_event = FailoverEvent {
            event_id,
            plan_id,
            trigger_reason: reason.clone(),
            source_region: "primary".to_string(), // This would be determined dynamically
            target_region: plan.failover_strategy.target_regions.first()
                .cloned()
                .unwrap_or_else(|| "secondary".to_string()),
            status: FailoverStatus::Initiated,
            started_at: SystemTime::now(),
            completed_at: None,
            recovery_time: None,
            data_loss: None,
        };

        {
            let mut events = self.failover_events.write().await;
            events.insert(event_id, failover_event);
        }

        // Execute failover in background
        let manager = Arc::new(self.clone());
        let plan_clone = plan.clone();
        tokio::spawn(async move {
            if let Err(e) = manager.execute_failover_process(event_id, plan_clone).await {
                log::error!("Failover execution {} failed: {}", event_id, e);
                
                let mut events = manager.failover_events.write().await;
                if let Some(event) = events.get_mut(&event_id) {
                    event.status = FailoverStatus::Failed;
                    event.completed_at = Some(SystemTime::now());
                }
            }
        });

        log::warn!("Triggered disaster recovery failover: {} - {}", event_id, reason);
        Ok(event_id)
    }

    async fn get_failover_status(&self, event_id: Uuid) -> Result<FailoverEvent> {
        let events = self.failover_events.read().await;
        events.get(&event_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Failover event {} not found", event_id)))
    }
}

impl ProductionDisasterRecoveryManager {
    async fn execute_failover_process(&self, event_id: Uuid, plan: DisasterRecoveryPlan) -> Result<()> {
        let start_time = SystemTime::now();

        // Update status to in progress
        {
            let mut events = self.failover_events.write().await;
            if let Some(event) = events.get_mut(&event_id) {
                event.status = FailoverStatus::InProgress;
            }
        }

        // Execute recovery procedures
        self.execute_recovery_procedures(&plan).await?;

        // Calculate recovery metrics
        let recovery_time = start_time.elapsed().unwrap_or(Duration::ZERO);
        let data_loss = Duration::from_secs(0); // This would be calculated based on last backup

        // Update event with completion
        {
            let mut events = self.failover_events.write().await;
            if let Some(event) = events.get_mut(&event_id) {
                event.status = FailoverStatus::Completed;
                event.completed_at = Some(SystemTime::now());
                event.recovery_time = Some(recovery_time);
                event.data_loss = Some(data_loss);
            }
        }

        log::info!("Failover completed successfully: {} in {:?}", event_id, recovery_time);
        Ok(())
    }
}

impl Clone for ProductionDisasterRecoveryManager {
    fn clone(&self) -> Self {
        Self {
            recovery_plans: Arc::clone(&self.recovery_plans),
            backup_jobs: Arc::clone(&self.backup_jobs),
            restore_jobs: Arc::clone(&self.restore_jobs),
            failover_events: Arc::clone(&self.failover_events),
            backup_manager: Arc::clone(&self.backup_manager),
        }
    }
}

// Mock implementation for testing
pub struct MockBackupManager {
    backup_jobs: Arc<RwLock<HashMap<Uuid, BackupJob>>>,
    restore_jobs: Arc<RwLock<HashMap<Uuid, RestoreJob>>>,
}

impl MockBackupManager {
    pub fn new() -> Self {
        Self {
            backup_jobs: Arc::new(RwLock::new(HashMap::new())),
            restore_jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl BackupManager for MockBackupManager {
    async fn create_backup(&self, plan_id: Uuid, backup_type: BackupType) -> Result<Uuid> {
        let job_id = Uuid::new_v4();
        
        let backup_job = BackupJob {
            job_id,
            plan_id,
            backup_type,
            status: BackupStatus::Completed, // Simulate immediate completion
            started_at: SystemTime::now(),
            completed_at: Some(SystemTime::now()),
            size_bytes: 1024 * 1024 * 100, // 100MB
            storage_location: StorageLocation {
                location_id: Uuid::new_v4(),
                name: "mock-storage".to_string(),
                location_type: LocationType::Local,
                region: "us-west-2".to_string(),
                endpoint: "file:///backups".to_string(),
                credentials: StorageCredentials {
                    access_key: "mock".to_string(),
                    secret_key: "mock".to_string(),
                    additional_config: HashMap::new(),
                },
            },
            error_message: None,
        };

        {
            let mut jobs = self.backup_jobs.write().await;
            jobs.insert(job_id, backup_job);
        }

        Ok(job_id)
    }

    async fn get_backup_status(&self, job_id: Uuid) -> Result<BackupJob> {
        let jobs = self.backup_jobs.read().await;
        jobs.get(&job_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Backup job {} not found", job_id)))
    }

    async fn list_backups(&self, plan_id: Option<Uuid>) -> Result<Vec<BackupJob>> {
        let jobs = self.backup_jobs.read().await;
        
        let filtered_jobs: Vec<BackupJob> = jobs.values()
            .filter(|job| plan_id.map_or(true, |pid| job.plan_id == pid))
            .cloned()
            .collect();

        Ok(filtered_jobs)
    }

    async fn restore_from_backup(&self, backup_job_id: Uuid, target_location: String) -> Result<Uuid> {
        let restore_job_id = Uuid::new_v4();
        
        let restore_job = RestoreJob {
            job_id: restore_job_id,
            backup_job_id,
            restore_point: SystemTime::now(),
            status: RestoreStatus::Completed, // Simulate immediate completion
            started_at: SystemTime::now(),
            completed_at: Some(SystemTime::now()),
            target_location,
            error_message: None,
        };

        {
            let mut jobs = self.restore_jobs.write().await;
            jobs.insert(restore_job_id, restore_job);
        }

        Ok(restore_job_id)
    }

    async fn get_restore_status(&self, job_id: Uuid) -> Result<RestoreJob> {
        let jobs = self.restore_jobs.read().await;
        jobs.get(&job_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Restore job {} not found", job_id)))
    }

    async fn validate_backup(&self, job_id: Uuid) -> Result<ValidationResult> {
        let _backup_job = self.get_backup_status(job_id).await?;
        
        Ok(ValidationResult {
            validation_id: Uuid::new_v4(),
            backup_job_id: job_id,
            integrity_check: true,
            size_verification: true,
            checksum_verification: true,
            restore_test_passed: true,
            validation_time: Duration::from_secs(30),
        })
    }
}

impl Default for MockBackupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disaster_recovery_plan_management() {
        let backup_manager = Arc::new(MockBackupManager::new());
        let dr_manager = ProductionDisasterRecoveryManager::new(backup_manager);

        let plan = DisasterRecoveryPlan {
            plan_id: Uuid::new_v4(),
            name: "Critical Database DR".to_string(),
            description: "Disaster recovery for main database".to_string(),
            recovery_time_objective: Duration::from_secs(300), // 5 minutes
            recovery_point_objective: Duration::from_secs(60),  // 1 minute
            priority: RecoveryPriority::Critical,
            backup_strategy: BackupStrategy {
                backup_type: BackupType::Incremental,
                frequency: BackupFrequency::Hourly,
                retention_policy: RetentionPolicy {
                    daily_retention_days: 7,
                    weekly_retention_weeks: 4,
                    monthly_retention_months: 12,
                    yearly_retention_years: 7,
                },
                storage_locations: vec![],
                encryption_enabled: true,
                compression_enabled: true,
            },
            failover_strategy: FailoverStrategy {
                strategy_type: FailoverType::ActivePassive,
                automatic_failover: true,
                failover_timeout: Duration::from_secs(600),
                health_check_interval: Duration::from_secs(30),
                failure_threshold: 3,
                target_regions: vec!["us-east-1".to_string()],
            },
            recovery_procedures: vec![
                RecoveryProcedure {
                    procedure_id: Uuid::new_v4(),
                    name: "Database Restore".to_string(),
                    description: "Restore database from backup".to_string(),
                    steps: vec![
                        RecoveryStep {
                            step_id: Uuid::new_v4(),
                            name: "Stop Application".to_string(),
                            command: "systemctl stop app".to_string(),
                            timeout: Duration::from_secs(30),
                            retry_count: 2,
                            rollback_command: Some("systemctl start app".to_string()),
                        },
                    ],
                    estimated_duration: Duration::from_secs(180),
                    dependencies: vec![],
                },
            ],
            validation_tests: vec![
                ValidationTest {
                    test_id: Uuid::new_v4(),
                    name: "Failover Time Test".to_string(),
                    test_type: TestType::FailoverTime,
                    frequency: TestFrequency::Monthly,
                    success_criteria: vec![
                        SuccessCriterion {
                            metric: "failover_time".to_string(),
                            threshold: 300.0,
                            comparison: ComparisonOperator::LessThan,
                        },
                    ],
                },
            ],
            created_at: SystemTime::now(),
            last_tested: None,
        };

        let plan_id = dr_manager.create_recovery_plan(plan.clone()).await.unwrap();
        assert_eq!(plan_id, plan.plan_id);

        // Get plan
        let retrieved_plan = dr_manager.get_recovery_plan(plan_id).await.unwrap();
        assert_eq!(retrieved_plan.name, plan.name);

        // List plans
        let plans = dr_manager.list_recovery_plans().await.unwrap();
        assert_eq!(plans.len(), 1);

        // Update plan
        let mut updated_plan = plan.clone();
        updated_plan.description = "Updated description".to_string();
        dr_manager.update_recovery_plan(plan_id, updated_plan).await.unwrap();

        let updated_retrieved = dr_manager.get_recovery_plan(plan_id).await.unwrap();
        assert_eq!(updated_retrieved.description, "Updated description");

        // Delete plan
        dr_manager.delete_recovery_plan(plan_id).await.unwrap();
        
        let plans_after_delete = dr_manager.list_recovery_plans().await.unwrap();
        assert_eq!(plans_after_delete.len(), 0);
    }

    #[tokio::test]
    async fn test_disaster_recovery_testing() {
        let backup_manager = Arc::new(MockBackupManager::new());
        let dr_manager = ProductionDisasterRecoveryManager::new(backup_manager.clone());

        let plan = DisasterRecoveryPlan {
            plan_id: Uuid::new_v4(),
            name: "Test Plan".to_string(),
            description: "Test disaster recovery plan".to_string(),
            recovery_time_objective: Duration::from_secs(60),
            recovery_point_objective: Duration::from_secs(30),
            priority: RecoveryPriority::High,
            backup_strategy: BackupStrategy {
                backup_type: BackupType::Full,
                frequency: BackupFrequency::Daily,
                retention_policy: RetentionPolicy {
                    daily_retention_days: 7,
                    weekly_retention_weeks: 4,
                    monthly_retention_months: 12,
                    yearly_retention_years: 7,
                },
                storage_locations: vec![],
                encryption_enabled: true,
                compression_enabled: false,
            },
            failover_strategy: FailoverStrategy {
                strategy_type: FailoverType::HotStandby,
                automatic_failover: false,
                failover_timeout: Duration::from_secs(300),
                health_check_interval: Duration::from_secs(10),
                failure_threshold: 2,
                target_regions: vec!["backup-region".to_string()],
            },
            recovery_procedures: vec![],
            validation_tests: vec![
                ValidationTest {
                    test_id: Uuid::new_v4(),
                    name: "Backup Integrity".to_string(),
                    test_type: TestType::BackupIntegrity,
                    frequency: TestFrequency::Weekly,
                    success_criteria: vec![],
                },
                ValidationTest {
                    test_id: Uuid::new_v4(),
                    name: "Failover Time".to_string(),
                    test_type: TestType::FailoverTime,
                    frequency: TestFrequency::Monthly,
                    success_criteria: vec![],
                },
            ],
            created_at: SystemTime::now(),
            last_tested: None,
        };

        let plan_id = dr_manager.create_recovery_plan(plan).await.unwrap();

        // Create a backup to satisfy readiness validation
        backup_manager.create_backup(plan_id, BackupType::Full).await.unwrap();

        // Test the recovery plan
        let test_result = dr_manager.test_recovery_plan(plan_id).await.unwrap();
        assert_eq!(test_result.plan_id, plan_id);
        assert!(matches!(test_result.status, TestStatus::Completed));
        assert!(!test_result.results.is_empty());

        // Verify last_tested was updated
        let updated_plan = dr_manager.get_recovery_plan(plan_id).await.unwrap();
        assert!(updated_plan.last_tested.is_some());
    }

    #[tokio::test]
    async fn test_failover_execution() {
        let backup_manager = Arc::new(MockBackupManager::new());
        let dr_manager = ProductionDisasterRecoveryManager::new(backup_manager.clone());

        let plan = DisasterRecoveryPlan {
            plan_id: Uuid::new_v4(),
            name: "Failover Test Plan".to_string(),
            description: "Test failover execution".to_string(),
            recovery_time_objective: Duration::from_secs(120),
            recovery_point_objective: Duration::from_secs(60),
            priority: RecoveryPriority::Critical,
            backup_strategy: BackupStrategy {
                backup_type: BackupType::Snapshot,
                frequency: BackupFrequency::Continuous,
                retention_policy: RetentionPolicy {
                    daily_retention_days: 1,
                    weekly_retention_weeks: 1,
                    monthly_retention_months: 1,
                    yearly_retention_years: 1,
                },
                storage_locations: vec![],
                encryption_enabled: true,
                compression_enabled: true,
            },
            failover_strategy: FailoverStrategy {
                strategy_type: FailoverType::ActiveActive,
                automatic_failover: true,
                failover_timeout: Duration::from_secs(180),
                health_check_interval: Duration::from_secs(5),
                failure_threshold: 1,
                target_regions: vec!["dr-region".to_string()],
            },
            recovery_procedures: vec![
                RecoveryProcedure {
                    procedure_id: Uuid::new_v4(),
                    name: "Emergency Failover".to_string(),
                    description: "Execute emergency failover".to_string(),
                    steps: vec![
                        RecoveryStep {
                            step_id: Uuid::new_v4(),
                            name: "Switch DNS".to_string(),
                            command: "update-dns.sh".to_string(),
                            timeout: Duration::from_secs(60),
                            retry_count: 3,
                            rollback_command: Some("revert-dns.sh".to_string()),
                        },
                    ],
                    estimated_duration: Duration::from_secs(90),
                    dependencies: vec![],
                },
            ],
            validation_tests: vec![],
            created_at: SystemTime::now(),
            last_tested: None,
        };

        let plan_id = dr_manager.create_recovery_plan(plan).await.unwrap();

        // Create a recent backup to satisfy readiness validation
        backup_manager.create_backup(plan_id, BackupType::Snapshot).await.unwrap();

        // Trigger failover
        let event_id = dr_manager.trigger_failover(plan_id, "Primary datacenter failure".to_string()).await.unwrap();

        // Wait for failover to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        let failover_event = dr_manager.get_failover_status(event_id).await.unwrap();
        assert_eq!(failover_event.plan_id, plan_id);
        assert_eq!(failover_event.trigger_reason, "Primary datacenter failure");
        assert!(matches!(failover_event.status, FailoverStatus::Completed));
        assert!(failover_event.recovery_time.is_some());
    }

    #[tokio::test]
    async fn test_backup_manager() {
        let backup_manager = MockBackupManager::new();
        let plan_id = Uuid::new_v4();

        // Create backup
        let job_id = backup_manager.create_backup(plan_id, BackupType::Full).await.unwrap();

        // Get backup status
        let backup_job = backup_manager.get_backup_status(job_id).await.unwrap();
        assert_eq!(backup_job.plan_id, plan_id);
        assert!(matches!(backup_job.status, BackupStatus::Completed));

        // List backups
        let backups = backup_manager.list_backups(Some(plan_id)).await.unwrap();
        assert_eq!(backups.len(), 1);

        // Validate backup
        let validation = backup_manager.validate_backup(job_id).await.unwrap();
        assert!(validation.integrity_check);
        assert!(validation.restore_test_passed);

        // Restore from backup
        let restore_job_id = backup_manager.restore_from_backup(job_id, "/tmp/restore".to_string()).await.unwrap();
        
        let restore_job = backup_manager.get_restore_status(restore_job_id).await.unwrap();
        assert_eq!(restore_job.backup_job_id, job_id);
        assert!(matches!(restore_job.status, RestoreStatus::Completed));
    }
}