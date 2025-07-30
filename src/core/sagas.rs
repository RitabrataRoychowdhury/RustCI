//! SAGA pattern implementation for distributed transaction management
//!
//! This module provides SAGA orchestration with compensation actions,
//! persistence, and proper error handling for complex business workflows.

use async_trait::async_trait;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::core::correlation::CorrelationTracker;
use crate::core::events::{DomainEvent, EventBus};
use crate::error::Result;

/// Represents a single step in a SAGA
#[async_trait]
pub trait SagaStep: Send + Sync + std::fmt::Debug {
    /// Execute the step
    async fn execute(&self, context: &SagaContext) -> Result<SagaStepResult>;

    /// Compensate for this step (rollback)
    async fn compensate(&self, context: &SagaContext) -> Result<()>;

    /// Get the step name for logging and identification
    fn step_name(&self) -> &'static str;

    /// Check if this step can be compensated
    fn can_compensate(&self) -> bool {
        true
    }

    /// Get step metadata
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Validate step before execution
    fn validate(&self, context: &SagaContext) -> Result<()> {
        let _ = context;
        Ok(())
    }
}

/// Result of executing a SAGA step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStepResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl SagaStepResult {
    pub fn success(data: Option<serde_json::Value>) -> Self {
        Self {
            success: true,
            data,
            error: None,
            metadata: HashMap::new(),
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Context passed to SAGA steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaContext {
    pub saga_id: Uuid,
    pub correlation_id: Uuid,
    pub step_data: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, String>,
    pub started_at: DateTime<Utc>,
}

impl SagaContext {
    pub fn new(saga_id: Uuid, correlation_id: Uuid) -> Self {
        Self {
            saga_id,
            correlation_id,
            step_data: HashMap::new(),
            metadata: HashMap::new(),
            started_at: Utc::now(),
        }
    }

    pub fn set_step_data(&mut self, key: String, value: serde_json::Value) {
        self.step_data.insert(key, value);
    }

    pub fn get_step_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.step_data.get(key)
    }

    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// SAGA execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SagaStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Compensating,
    Compensated,
    CompensationFailed,
}

/// SAGA execution record for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaExecution {
    pub saga_id: Uuid,
    pub correlation_id: Uuid,
    pub saga_name: String,
    pub status: SagaStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_step: usize,
    pub total_steps: usize,
    pub step_results: Vec<SagaStepExecutionRecord>,
    pub context: SagaContext,
    pub error: Option<String>,
}

/// Record of a single step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStepExecutionRecord {
    pub step_name: String,
    pub step_index: usize,
    pub status: SagaStepStatus,
    pub executed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<SagaStepResult>,
    pub compensation_executed_at: Option<DateTime<Utc>>,
    pub compensation_completed_at: Option<DateTime<Utc>>,
    pub compensation_error: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SagaStepStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Compensating,
    Compensated,
    CompensationFailed,
}

/// SAGA orchestrator that manages the execution of SAGA workflows
pub struct SagaOrchestrator {
    steps: Vec<Box<dyn SagaStep>>,
    saga_name: String,
    event_bus: Arc<EventBus>,
    correlation_tracker: Arc<CorrelationTracker>,
    persistence: Arc<dyn SagaPersistence>,
}

impl SagaOrchestrator {
    /// Create a new SAGA orchestrator
    pub fn new(
        saga_name: String,
        event_bus: Arc<EventBus>,
        correlation_tracker: Arc<CorrelationTracker>,
        persistence: Arc<dyn SagaPersistence>,
    ) -> Self {
        Self {
            steps: Vec::new(),
            saga_name,
            event_bus,
            correlation_tracker,
            persistence,
        }
    }

    /// Add a step to the SAGA
    pub fn add_step(&mut self, step: Box<dyn SagaStep>) {
        self.steps.push(step);
    }

    /// Execute the SAGA
    pub async fn execute(&self, context: SagaContext) -> Result<SagaExecution> {
        let saga_id = context.saga_id;
        let correlation_id = context.correlation_id;

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(correlation_id)
            .await;

        info!(
            saga_id = %saga_id,
            correlation_id = %correlation_id,
            saga_name = %self.saga_name,
            total_steps = self.steps.len(),
            "Starting SAGA execution"
        );

        // Create initial execution record
        let mut execution = SagaExecution {
            saga_id,
            correlation_id,
            saga_name: self.saga_name.clone(),
            status: SagaStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            current_step: 0,
            total_steps: self.steps.len(),
            step_results: Vec::new(),
            context: context.clone(),
            error: None,
        };

        // Persist initial state
        self.persistence.save_execution(&execution).await?;

        // Publish SAGA started event
        self.publish_saga_event(SagaStartedEvent {
            saga_id,
            correlation_id,
            saga_name: self.saga_name.clone(),
            occurred_at: Utc::now(),
        })
        .await?;

        // Execute steps sequentially
        let mut current_context = context;
        let mut executed_steps = Vec::new();

        for (index, step) in self.steps.iter().enumerate() {
            execution.current_step = index;

            let step_record = self
                .execute_step(step.as_ref(), &current_context, index)
                .await;

            match &step_record.result {
                Some(result) if result.success => {
                    // Step succeeded, continue
                    if let Some(data) = &result.data {
                        current_context
                            .step_data
                            .insert(format!("step_{}", index), data.clone());
                    }
                    executed_steps.push(step_record.clone());
                    execution.step_results.push(step_record);
                }
                _ => {
                    // Step failed, start compensation
                    let step_error = step_record.result.as_ref().and_then(|r| r.error.clone());

                    executed_steps.push(step_record.clone());
                    execution.step_results.push(step_record);
                    execution.status = SagaStatus::Failed;
                    execution.error = step_error;

                    // Save failed state
                    self.persistence.save_execution(&execution).await?;

                    // Start compensation
                    return self.compensate(execution, executed_steps).await;
                }
            }

            // Update execution state
            execution.step_results = executed_steps.clone();
            self.persistence.save_execution(&execution).await?;
        }

        // All steps completed successfully
        execution.status = SagaStatus::Completed;
        execution.completed_at = Some(Utc::now());
        execution.current_step = self.steps.len();

        self.persistence.save_execution(&execution).await?;

        info!(
            saga_id = %saga_id,
            correlation_id = %correlation_id,
            duration_ms = execution.completed_at.unwrap()
                .signed_duration_since(execution.started_at)
                .num_milliseconds(),
            "SAGA execution completed successfully"
        );

        // Publish SAGA completed event
        self.publish_saga_event(SagaCompletedEvent {
            saga_id,
            correlation_id,
            saga_name: self.saga_name.clone(),
            status: SagaStatus::Completed,
            occurred_at: Utc::now(),
        })
        .await?;

        Ok(execution)
    }

    /// Execute a single step
    async fn execute_step(
        &self,
        step: &dyn SagaStep,
        context: &SagaContext,
        index: usize,
    ) -> SagaStepExecutionRecord {
        let step_name = step.step_name();
        let started_at = Utc::now();

        debug!(
            saga_id = %context.saga_id,
            step_name = step_name,
            step_index = index,
            "Executing SAGA step"
        );

        // Validate step
        if let Err(e) = step.validate(context) {
            return SagaStepExecutionRecord {
                step_name: step_name.to_string(),
                step_index: index,
                status: SagaStepStatus::Failed,
                executed_at: started_at,
                completed_at: Some(Utc::now()),
                result: Some(SagaStepResult::failure(e.to_string())),
                compensation_executed_at: None,
                compensation_completed_at: None,
                compensation_error: None,
                duration_ms: Some(0),
            };
        }

        // Execute step
        let result = step.execute(context).await;
        let completed_at = Utc::now();
        let duration_ms = completed_at
            .signed_duration_since(started_at)
            .num_milliseconds() as u64;

        let (status, step_result) = match result {
            Ok(result) => {
                if result.success {
                    (SagaStepStatus::Completed, result)
                } else {
                    (SagaStepStatus::Failed, result)
                }
            }
            Err(e) => (
                SagaStepStatus::Failed,
                SagaStepResult::failure(e.to_string()),
            ),
        };

        debug!(
            saga_id = %context.saga_id,
            step_name = step_name,
            step_index = index,
            status = ?status,
            duration_ms = duration_ms,
            "SAGA step execution completed"
        );

        SagaStepExecutionRecord {
            step_name: step_name.to_string(),
            step_index: index,
            status,
            executed_at: started_at,
            completed_at: Some(completed_at),
            result: Some(step_result),
            compensation_executed_at: None,
            compensation_completed_at: None,
            compensation_error: None,
            duration_ms: Some(duration_ms),
        }
    }

    /// Compensate executed steps in reverse order
    async fn compensate(
        &self,
        mut execution: SagaExecution,
        executed_steps: Vec<SagaStepExecutionRecord>,
    ) -> Result<SagaExecution> {
        execution.status = SagaStatus::Compensating;
        self.persistence.save_execution(&execution).await?;

        info!(
            saga_id = %execution.saga_id,
            steps_to_compensate = executed_steps.len(),
            "Starting SAGA compensation"
        );

        // Compensate in reverse order
        for step_record in executed_steps.iter().rev() {
            if step_record.status == SagaStepStatus::Completed {
                let step_index = step_record.step_index;
                if let Some(step) = self.steps.get(step_index) {
                    if step.can_compensate() {
                        let compensation_result = self
                            .compensate_step(step.as_ref(), &execution.context, step_index)
                            .await;

                        // Update step record with compensation result
                        if let Some(record) = execution.step_results.get_mut(step_index) {
                            record.compensation_executed_at = Some(compensation_result.executed_at);
                            record.compensation_completed_at = compensation_result.completed_at;

                            if compensation_result.success {
                                record.status = SagaStepStatus::Compensated;
                                record.compensation_error = None;
                            } else {
                                record.status = SagaStepStatus::CompensationFailed;
                                record.compensation_error = compensation_result.error.clone();
                                execution.status = SagaStatus::CompensationFailed;
                                execution.error = compensation_result.error;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if execution.status == SagaStatus::Compensating {
            execution.status = SagaStatus::Compensated;
        }

        execution.completed_at = Some(Utc::now());
        self.persistence.save_execution(&execution).await?;

        info!(
            saga_id = %execution.saga_id,
            final_status = ?execution.status,
            "SAGA compensation completed"
        );

        // Publish SAGA compensated event
        self.publish_saga_event(SagaCompensatedEvent {
            saga_id: execution.saga_id,
            correlation_id: execution.correlation_id,
            saga_name: self.saga_name.clone(),
            status: execution.status.clone(),
            occurred_at: Utc::now(),
        })
        .await?;

        Ok(execution)
    }

    /// Compensate a single step
    async fn compensate_step(
        &self,
        step: &dyn SagaStep,
        context: &SagaContext,
        index: usize,
    ) -> CompensationResult {
        let step_name = step.step_name();
        let started_at = Utc::now();

        debug!(
            saga_id = %context.saga_id,
            step_name = step_name,
            step_index = index,
            "Compensating SAGA step"
        );

        let result = step.compensate(context).await;
        let completed_at = Utc::now();

        match result {
            Ok(()) => {
                debug!(
                    saga_id = %context.saga_id,
                    step_name = step_name,
                    step_index = index,
                    "SAGA step compensation successful"
                );

                CompensationResult {
                    success: true,
                    executed_at: started_at,
                    completed_at: Some(completed_at),
                    error: None,
                }
            }
            Err(e) => {
                error!(
                    saga_id = %context.saga_id,
                    step_name = step_name,
                    step_index = index,
                    error = %e,
                    "SAGA step compensation failed"
                );

                CompensationResult {
                    success: false,
                    executed_at: started_at,
                    completed_at: Some(completed_at),
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Publish SAGA events
    async fn publish_saga_event<T: DomainEvent>(&self, event: T) -> Result<()> {
        self.event_bus.publish(event).await
    }
}

/// Result of compensation execution
struct CompensationResult {
    success: bool,
    executed_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
}

/// Trait for SAGA persistence
#[async_trait]
pub trait SagaPersistence: Send + Sync {
    /// Save SAGA execution state
    async fn save_execution(&self, execution: &SagaExecution) -> Result<()>;

    /// Load SAGA execution by ID
    async fn load_execution(&self, saga_id: Uuid) -> Result<Option<SagaExecution>>;

    /// Find executions by status
    async fn find_by_status(&self, status: SagaStatus) -> Result<Vec<SagaExecution>>;

    /// Find executions by correlation ID
    async fn find_by_correlation_id(&self, correlation_id: Uuid) -> Result<Vec<SagaExecution>>;

    /// Delete execution record
    async fn delete_execution(&self, saga_id: Uuid) -> Result<()>;

    /// Get execution statistics
    async fn get_statistics(&self) -> Result<SagaStatistics>;
}

/// SAGA execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStatistics {
    pub total_executions: u64,
    pub completed_executions: u64,
    pub failed_executions: u64,
    pub compensated_executions: u64,
    pub average_duration_ms: f64,
    pub success_rate: f64,
}

/// SAGA domain events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStartedEvent {
    pub saga_id: Uuid,
    pub correlation_id: Uuid,
    pub saga_name: String,
    pub occurred_at: DateTime<Utc>,
}

impl DomainEvent for SagaStartedEvent {
    fn event_type(&self) -> &'static str {
        "saga.started"
    }

    fn aggregate_id(&self) -> Uuid {
        self.saga_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaCompletedEvent {
    pub saga_id: Uuid,
    pub correlation_id: Uuid,
    pub saga_name: String,
    pub status: SagaStatus,
    pub occurred_at: DateTime<Utc>,
}

impl DomainEvent for SagaCompletedEvent {
    fn event_type(&self) -> &'static str {
        "saga.completed"
    }

    fn aggregate_id(&self) -> Uuid {
        self.saga_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaCompensatedEvent {
    pub saga_id: Uuid,
    pub correlation_id: Uuid,
    pub saga_name: String,
    pub status: SagaStatus,
    pub occurred_at: DateTime<Utc>,
}

impl DomainEvent for SagaCompensatedEvent {
    fn event_type(&self) -> &'static str {
        "saga.compensated"
    }

    fn aggregate_id(&self) -> Uuid {
        self.saga_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::RwLock;

    #[derive(Debug)]
    struct TestStep {
        name: &'static str,
        should_fail: Arc<AtomicBool>,
        executed: Arc<AtomicBool>,
        compensated: Arc<AtomicBool>,
    }

    impl TestStep {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                should_fail: Arc::new(AtomicBool::new(false)),
                executed: Arc::new(AtomicBool::new(false)),
                compensated: Arc::new(AtomicBool::new(false)),
            }
        }

        fn set_should_fail(&self, should_fail: bool) {
            self.should_fail.store(should_fail, Ordering::SeqCst);
        }

        fn was_executed(&self) -> bool {
            self.executed.load(Ordering::SeqCst)
        }

        fn was_compensated(&self) -> bool {
            self.compensated.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl SagaStep for TestStep {
        async fn execute(&self, _context: &SagaContext) -> Result<SagaStepResult> {
            self.executed.store(true, Ordering::SeqCst);

            if self.should_fail.load(Ordering::SeqCst) {
                Ok(SagaStepResult::failure("Test failure".to_string()))
            } else {
                Ok(SagaStepResult::success(Some(
                    serde_json::json!({"step": self.name}),
                )))
            }
        }

        async fn compensate(&self, _context: &SagaContext) -> Result<()> {
            self.compensated.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn step_name(&self) -> &'static str {
            self.name
        }
    }

    struct InMemorySagaPersistence {
        executions: Arc<RwLock<HashMap<Uuid, SagaExecution>>>,
    }

    impl InMemorySagaPersistence {
        fn new() -> Self {
            Self {
                executions: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl SagaPersistence for InMemorySagaPersistence {
        async fn save_execution(&self, execution: &SagaExecution) -> Result<()> {
            let mut executions = self.executions.write().await;
            executions.insert(execution.saga_id, execution.clone());
            Ok(())
        }

        async fn load_execution(&self, saga_id: Uuid) -> Result<Option<SagaExecution>> {
            let executions = self.executions.read().await;
            Ok(executions.get(&saga_id).cloned())
        }

        async fn find_by_status(&self, status: SagaStatus) -> Result<Vec<SagaExecution>> {
            let executions = self.executions.read().await;
            Ok(executions
                .values()
                .filter(|e| e.status == status)
                .cloned()
                .collect())
        }

        async fn find_by_correlation_id(&self, correlation_id: Uuid) -> Result<Vec<SagaExecution>> {
            let executions = self.executions.read().await;
            Ok(executions
                .values()
                .filter(|e| e.correlation_id == correlation_id)
                .cloned()
                .collect())
        }

        async fn delete_execution(&self, saga_id: Uuid) -> Result<()> {
            let mut executions = self.executions.write().await;
            executions.remove(&saga_id);
            Ok(())
        }

        async fn get_statistics(&self) -> Result<SagaStatistics> {
            let executions = self.executions.read().await;
            let total = executions.len() as u64;
            let completed = executions
                .values()
                .filter(|e| e.status == SagaStatus::Completed)
                .count() as u64;
            let failed = executions
                .values()
                .filter(|e| e.status == SagaStatus::Failed)
                .count() as u64;
            let compensated = executions
                .values()
                .filter(|e| e.status == SagaStatus::Compensated)
                .count() as u64;

            Ok(SagaStatistics {
                total_executions: total,
                completed_executions: completed,
                failed_executions: failed,
                compensated_executions: compensated,
                average_duration_ms: 0.0,
                success_rate: if total > 0 {
                    completed as f64 / total as f64
                } else {
                    0.0
                },
            })
        }
    }

    #[tokio::test]
    async fn test_saga_successful_execution() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
        let persistence = Arc::new(InMemorySagaPersistence::new());

        let mut orchestrator = SagaOrchestrator::new(
            "test-saga".to_string(),
            event_bus,
            correlation_tracker,
            persistence.clone(),
        );

        let step1 = TestStep::new("step1");
        let step2 = TestStep::new("step2");

        orchestrator.add_step(Box::new(step1));
        orchestrator.add_step(Box::new(step2));

        let context = SagaContext::new(Uuid::new_v4(), Uuid::new_v4());
        let result = orchestrator.execute(context).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.status, SagaStatus::Completed);
        assert_eq!(execution.step_results.len(), 2);
    }

    #[tokio::test]
    async fn test_saga_compensation() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
        let persistence = Arc::new(InMemorySagaPersistence::new());

        let mut orchestrator = SagaOrchestrator::new(
            "test-saga".to_string(),
            event_bus,
            correlation_tracker,
            persistence.clone(),
        );

        // Create steps where the second one will fail
        let step1 = TestStep::new("step1");
        let step2 = TestStep::new("step2");
        step2.set_should_fail(true);

        orchestrator.add_step(Box::new(step1));
        orchestrator.add_step(Box::new(step2));

        let context = SagaContext::new(Uuid::new_v4(), Uuid::new_v4());
        let result = orchestrator.execute(context).await;

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.status, SagaStatus::Compensated);
    }
}
