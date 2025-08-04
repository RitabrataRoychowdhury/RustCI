//! Enhanced command pattern implementation with CQRS, queuing, batching, and authorization
//!
//! This module provides the command side of CQRS pattern with proper
//! validation, error handling, audit trail support, queuing, batching,
//! authorization, and retry mechanisms.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::networking::security::{Permission, SecurityContext};
use crate::error::{AppError, Result};

/// Core trait that all commands must implement
#[async_trait]
pub trait Command: Send + Sync + std::fmt::Debug + Clone {
    type Result: Send + Sync + Clone;

    /// Execute the command
    async fn execute(&self) -> Result<Self::Result>;

    /// Compensate for this command (for SAGA pattern)
    async fn compensate(&self) -> Result<()> {
        warn!("Compensation not implemented for command: {:?}", self);
        Ok(())
    }

    /// Get the command ID
    fn command_id(&self) -> Uuid;

    /// Get the correlation ID for tracing
    fn correlation_id(&self) -> Uuid;

    /// Get the command name for logging and metrics
    fn command_name(&self) -> &'static str;

    /// Check if this command supports compensation
    fn supports_compensation(&self) -> bool {
        false
    }

    /// Validate the command before execution
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Get command metadata
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Get required permissions for authorization
    fn required_permissions(&self) -> Vec<Permission> {
        vec![]
    }

    /// Get command priority for queuing
    fn priority(&self) -> CommandPriority {
        CommandPriority::Normal
    }

    /// Check if command can be batched with others
    fn can_batch(&self) -> bool {
        false
    }

    /// Get batch key for grouping similar commands
    fn batch_key(&self) -> Option<String> {
        None
    }

    /// Get retry configuration
    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }
}

/// Handler for commands
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    /// Handle the command
    async fn handle(&self, command: C) -> Result<C::Result>;

    /// Get handler name for logging and debugging
    fn handler_name(&self) -> &'static str;

    /// Validate the command before handling
    async fn validate(&self, command: &C) -> Result<()> {
        command.validate()
    }
}

/// Command priority for queue ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CommandPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Retry configuration for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub retry_on_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            retry_on_errors: vec![
                "DatabaseError".to_string(),
                "NetworkError".to_string(),
                "TemporaryError".to_string(),
            ],
        }
    }
}

/// Command execution record for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecutionRecord {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub command_name: String,
    pub handler_name: String,
    pub executed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: CommandExecutionStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
    pub metadata: HashMap<String, String>,
    pub retry_count: u32,
    pub priority: CommandPriority,
    pub batch_id: Option<Uuid>,
    pub user_id: Option<String>,
    pub permissions_checked: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandExecutionStatus {
    Queued,
    Pending,
    Executing,
    Completed,
    Failed,
    Compensated,
    Retrying,
    Cancelled,
}

/// Queued command wrapper
#[derive(Debug, Clone)]
pub struct QueuedCommand {
    pub command_data: Vec<u8>, // Serialized command
    pub command_type: String,
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub priority: CommandPriority,
    pub queued_at: DateTime<Utc>,
    pub retry_count: u32,
    pub batch_id: Option<Uuid>,
    pub user_id: Option<String>,
    pub security_context: Option<SecurityContext>,
}

/// Batch execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExecutionResult {
    pub batch_id: Uuid,
    pub command_count: usize,
    pub successful_count: usize,
    pub failed_count: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// Enhanced command dispatcher with queuing, batching, and authorization
pub struct CommandDispatcher {
    handlers: Arc<RwLock<HashMap<String, Box<dyn CommandHandlerWrapper>>>>,
    execution_log: Arc<RwLock<Vec<CommandExecutionRecord>>>,
    correlation_tracker: Arc<crate::core::CorrelationTracker>,
    command_queue: Arc<Mutex<VecDeque<QueuedCommand>>>,
    batch_queue: Arc<Mutex<HashMap<String, Vec<QueuedCommand>>>>,
    queue_processor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    batch_processor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    queue_sender: Arc<Mutex<Option<mpsc::UnboundedSender<QueuedCommand>>>>,
    queue_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<QueuedCommand>>>>,
    config: CommandDispatcherConfig,
}

/// Configuration for command dispatcher
#[derive(Debug, Clone)]
pub struct CommandDispatcherConfig {
    pub max_queue_size: usize,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub worker_count: usize,
    pub enable_authorization: bool,
    pub enable_batching: bool,
    pub enable_retries: bool,
}

impl Default for CommandDispatcherConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            batch_size: 100,
            batch_timeout_ms: 1000,
            worker_count: 4,
            enable_authorization: true,
            enable_batching: true,
            enable_retries: true,
        }
    }
}

/// Wrapper trait to handle different command types
#[async_trait]
trait CommandHandlerWrapper: Send + Sync {
    async fn handle_command_bytes(
        &self,
        command_data: &[u8],
        security_context: Option<&SecurityContext>,
    ) -> Result<Vec<u8>>;
    fn handler_name(&self) -> &'static str;
    fn command_type(&self) -> &'static str;
}

/// Concrete wrapper implementation
struct ConcreteCommandHandlerWrapper<C: Command + 'static> {
    handler: Box<dyn CommandHandler<C>>,
    _phantom: std::marker::PhantomData<C>,
}

#[async_trait]
impl<C: Command + 'static> CommandHandlerWrapper for ConcreteCommandHandlerWrapper<C>
where
    C: for<'de> Deserialize<'de> + Serialize,
    C::Result: Serialize + for<'de> Deserialize<'de>,
{
    async fn handle_command_bytes(
        &self,
        command_data: &[u8],
        security_context: Option<&SecurityContext>,
    ) -> Result<Vec<u8>> {
        // Deserialize command
        let command: C = serde_json::from_slice(command_data).map_err(|e| {
            AppError::ValidationError(format!("Failed to deserialize command: {}", e))
        })?;

        // Check authorization if security context is provided
        if let Some(ctx) = security_context {
            let required_permissions = command.required_permissions();
            for permission in required_permissions {
                if !ctx.has_permission(&permission) {
                    return Err(AppError::AuthorizationError(format!(
                        "Missing required permission: {:?}",
                        permission
                    )));
                }
            }
        }

        // Execute command
        let result = self.handler.handle(command).await?;

        // Serialize result
        serde_json::to_vec(&result).map_err(|e| {
            AppError::InternalServerError(format!("Failed to serialize result: {}", e))
        })
    }

    fn handler_name(&self) -> &'static str {
        self.handler.handler_name()
    }

    fn command_type(&self) -> &'static str {
        std::any::type_name::<C>()
    }
}

impl CommandDispatcher {
    /// Create a new command dispatcher
    pub fn new(
        correlation_tracker: Arc<crate::core::CorrelationTracker>,
        config: Option<CommandDispatcherConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            execution_log: Arc::new(RwLock::new(Vec::new())),
            correlation_tracker,
            command_queue: Arc::new(Mutex::new(VecDeque::new())),
            batch_queue: Arc::new(Mutex::new(HashMap::new())),
            queue_processor_handle: Arc::new(Mutex::new(None)),
            batch_processor_handle: Arc::new(Mutex::new(None)),
            queue_sender: Arc::new(Mutex::new(Some(sender))),
            queue_receiver: Arc::new(Mutex::new(Some(receiver))),
            config,
        }
    }

    /// Start the command processing workers
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting command dispatcher with {} workers",
            self.config.worker_count
        );

        // Start queue processor
        let queue_handle = self.start_queue_processor().await?;
        *self.queue_processor_handle.lock().await = Some(queue_handle);

        // Start batch processor if enabled
        if self.config.enable_batching {
            let batch_handle = self.start_batch_processor().await?;
            *self.batch_processor_handle.lock().await = Some(batch_handle);
        }

        Ok(())
    }

    /// Stop the command dispatcher
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping command dispatcher");

        // Stop queue processor
        if let Some(handle) = self.queue_processor_handle.lock().await.take() {
            handle.abort();
        }

        // Stop batch processor
        if let Some(handle) = self.batch_processor_handle.lock().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Start the queue processor
    async fn start_queue_processor(&self) -> Result<tokio::task::JoinHandle<()>> {
        let handlers = Arc::clone(&self.handlers);
        let execution_log = Arc::clone(&self.execution_log);
        let correlation_tracker = Arc::clone(&self.correlation_tracker);
        let config = self.config.clone();
        let mut receiver = self.queue_receiver.lock().await.take().ok_or_else(|| {
            AppError::InternalServerError("Queue receiver already taken".to_string())
        })?;

        let handle = tokio::spawn(async move {
            info!("Queue processor started");

            while let Some(queued_command) = receiver.recv().await {
                let handlers_guard = handlers.read().await;
                if let Some(handler) = handlers_guard.get(&queued_command.command_type) {
                    let start_time = std::time::Instant::now();

                    // Create execution record
                    let mut record = CommandExecutionRecord {
                        command_id: queued_command.command_id,
                        correlation_id: queued_command.correlation_id,
                        command_name: queued_command.command_type.clone(),
                        handler_name: handler.handler_name().to_string(),
                        executed_at: Utc::now(),
                        completed_at: None,
                        status: CommandExecutionStatus::Executing,
                        result: None,
                        error: None,
                        duration_ms: None,
                        metadata: HashMap::new(),
                        retry_count: queued_command.retry_count,
                        priority: queued_command.priority,
                        batch_id: queued_command.batch_id,
                        user_id: queued_command.user_id.clone(),
                        permissions_checked: vec![],
                    };

                    // Set correlation context
                    correlation_tracker
                        .set_correlation_id(queued_command.correlation_id)
                        .await;

                    // Execute command with retry logic
                    let result = if config.enable_retries {
                        Self::execute_with_retry(
                            handler.as_ref(),
                            &queued_command.command_data,
                            queued_command.security_context.as_ref(),
                            queued_command.retry_count,
                        )
                        .await
                    } else {
                        handler
                            .handle_command_bytes(
                                &queued_command.command_data,
                                queued_command.security_context.as_ref(),
                            )
                            .await
                    };

                    let duration = start_time.elapsed();
                    record.completed_at = Some(Utc::now());
                    record.duration_ms = Some(duration.as_millis() as u64);

                    match result {
                        Ok(_) => {
                            record.status = CommandExecutionStatus::Completed;
                            record.result = Some("Success".to_string());

                            info!(
                                command_id = %queued_command.command_id,
                                duration_ms = duration.as_millis(),
                                "Command executed successfully"
                            );
                        }
                        Err(e) => {
                            record.status = CommandExecutionStatus::Failed;
                            record.error = Some(e.to_string());

                            error!(
                                command_id = %queued_command.command_id,
                                error = %e,
                                duration_ms = duration.as_millis(),
                                "Command execution failed"
                            );
                        }
                    }

                    // Log execution
                    let mut log = execution_log.write().await;
                    log.push(record);

                    // Keep only the last 10000 records to prevent memory issues
                    if log.len() > 10000 {
                        let len = log.len();
                        log.drain(0..len - 10000);
                    }
                } else {
                    warn!(
                        "No handler found for command type: {}",
                        queued_command.command_type
                    );
                }
            }

            info!("Queue processor stopped");
        });

        Ok(handle)
    }

    /// Start the batch processor
    async fn start_batch_processor(&self) -> Result<tokio::task::JoinHandle<()>> {
        let batch_queue = Arc::clone(&self.batch_queue);
        let handlers = Arc::clone(&self.handlers);
        let execution_log = Arc::clone(&self.execution_log);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            info!("Batch processor started");

            let mut interval =
                tokio::time::interval(Duration::from_millis(config.batch_timeout_ms));

            loop {
                interval.tick().await;

                let mut batches_to_process = Vec::new();
                {
                    let mut batch_queue_guard = batch_queue.lock().await;
                    for (batch_key, commands) in batch_queue_guard.drain() {
                        if !commands.is_empty() {
                            batches_to_process.push((batch_key, commands));
                        }
                    }
                }

                for (batch_key, commands) in batches_to_process {
                    if commands.len() >= config.batch_size {
                        Self::process_batch(&handlers, &execution_log, batch_key, commands).await;
                    } else {
                        // Put back small batches
                        let mut batch_queue_guard = batch_queue.lock().await;
                        batch_queue_guard.insert(batch_key, commands);
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Execute command with retry logic
    async fn execute_with_retry(
        handler: &dyn CommandHandlerWrapper,
        command_data: &[u8],
        security_context: Option<&SecurityContext>,
        _current_retry: u32,
    ) -> Result<Vec<u8>> {
        let retry_config = RetryConfig::default(); // In practice, this would come from the command
        let mut last_error = None;

        for attempt in 0..retry_config.max_attempts {
            match handler
                .handle_command_bytes(command_data, security_context)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.clone());

                    // Check if error is retryable
                    let error_name = format!("{:?}", e);
                    if !retry_config
                        .retry_on_errors
                        .iter()
                        .any(|retry_error| error_name.contains(retry_error))
                    {
                        return Err(e);
                    }

                    if attempt < retry_config.max_attempts - 1 {
                        let delay = std::cmp::min(
                            retry_config.initial_delay_ms
                                * (retry_config.backoff_multiplier.powi(attempt as i32) as u64),
                            retry_config.max_delay_ms,
                        );

                        warn!(
                            attempt = attempt + 1,
                            max_attempts = retry_config.max_attempts,
                            delay_ms = delay,
                            error = %e,
                            "Command execution failed, retrying"
                        );

                        sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| AppError::InternalServerError("Unknown retry error".to_string())))
    }

    /// Process a batch of commands
    async fn process_batch(
        handlers: &Arc<RwLock<HashMap<String, Box<dyn CommandHandlerWrapper>>>>,
        execution_log: &Arc<RwLock<Vec<CommandExecutionRecord>>>,
        batch_key: String,
        commands: Vec<QueuedCommand>,
    ) {
        let batch_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();
        let mut successful_count = 0;
        let mut failed_count = 0;
        let mut errors = Vec::new();

        info!(
            batch_id = %batch_id,
            batch_key = batch_key,
            command_count = commands.len(),
            "Processing command batch"
        );

        let handlers_guard = handlers.read().await;

        for command in commands {
            if let Some(handler) = handlers_guard.get(&command.command_type) {
                match handler
                    .handle_command_bytes(&command.command_data, command.security_context.as_ref())
                    .await
                {
                    Ok(_) => successful_count += 1,
                    Err(e) => {
                        failed_count += 1;
                        errors.push(format!("Command {}: {}", command.command_id, e));
                    }
                }
            } else {
                failed_count += 1;
                errors.push(format!(
                    "No handler for command type: {}",
                    command.command_type
                ));
            }
        }

        let duration = start_time.elapsed();
        let batch_result = BatchExecutionResult {
            batch_id,
            command_count: successful_count + failed_count,
            successful_count,
            failed_count,
            duration_ms: duration.as_millis() as u64,
            errors,
        };

        info!(
            batch_id = %batch_id,
            successful = successful_count,
            failed = failed_count,
            duration_ms = duration.as_millis(),
            "Batch processing completed"
        );

        // Log batch execution (simplified)
        let mut log = execution_log.write().await;
        let record = CommandExecutionRecord {
            command_id: batch_id,
            correlation_id: batch_id,
            command_name: format!("Batch[{}]", batch_key),
            handler_name: "BatchProcessor".to_string(),
            executed_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: if failed_count == 0 {
                CommandExecutionStatus::Completed
            } else {
                CommandExecutionStatus::Failed
            },
            result: Some(format!(
                "Processed {} commands",
                successful_count + failed_count
            )),
            error: if !batch_result.errors.is_empty() {
                Some(batch_result.errors.join("; "))
            } else {
                None
            },
            duration_ms: Some(duration.as_millis() as u64),
            metadata: HashMap::new(),
            retry_count: 0,
            priority: CommandPriority::Normal,
            batch_id: Some(batch_id),
            user_id: None,
            permissions_checked: vec![],
        };
        log.push(record);
    }

    /// Register a command handler
    pub async fn register_handler<C>(&self, handler: Box<dyn CommandHandler<C>>) -> Result<()>
    where
        C: Command + 'static + for<'de> Deserialize<'de> + Serialize,
        C::Result: Serialize + for<'de> Deserialize<'de>,
    {
        let command_name = std::any::type_name::<C>();
        let wrapper = ConcreteCommandHandlerWrapper {
            handler,
            _phantom: std::marker::PhantomData,
        };

        let mut handlers = self.handlers.write().await;
        handlers.insert(command_name.to_string(), Box::new(wrapper));

        info!("Registered command handler for: {}", command_name);
        Ok(())
    }

    /// Dispatch a command to its handler
    pub async fn dispatch<C: Command + 'static>(&self, command: C) -> Result<C::Result> {
        let command_name = command.command_name();
        let command_id = command.command_id();
        let correlation_id = command.correlation_id();

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(correlation_id)
            .await;

        info!(
            command_id = %command_id,
            correlation_id = %correlation_id,
            command_name = command_name,
            "Dispatching command"
        );

        // Create execution record
        let mut record = CommandExecutionRecord {
            command_id,
            correlation_id,
            command_name: command_name.to_string(),
            handler_name: "unknown".to_string(),
            executed_at: Utc::now(),
            completed_at: None,
            status: CommandExecutionStatus::Pending,
            result: None,
            error: None,
            duration_ms: None,
            metadata: command.metadata(),
            retry_count: 0,
            priority: command.priority(),
            batch_id: None,
            user_id: None,
            permissions_checked: vec![],
        };

        let start_time = std::time::Instant::now();

        // Validate command
        if let Err(e) = command.validate() {
            record.status = CommandExecutionStatus::Failed;
            record.error = Some(e.to_string());
            record.completed_at = Some(Utc::now());
            record.duration_ms = Some(start_time.elapsed().as_millis() as u64);

            self.log_execution(record).await;
            return Err(e);
        }

        record.status = CommandExecutionStatus::Executing;

        // Execute command
        let result = command.execute().await;

        let duration = start_time.elapsed();
        record.completed_at = Some(Utc::now());
        record.duration_ms = Some(duration.as_millis() as u64);

        match result {
            Ok(res) => {
                record.status = CommandExecutionStatus::Completed;
                record.result = Some("Success".to_string());

                info!(
                    command_id = %command_id,
                    duration_ms = duration.as_millis(),
                    "Command executed successfully"
                );

                self.log_execution(record).await;
                Ok(res)
            }
            Err(e) => {
                record.status = CommandExecutionStatus::Failed;
                record.error = Some(e.to_string());

                error!(
                    command_id = %command_id,
                    error = %e,
                    duration_ms = duration.as_millis(),
                    "Command execution failed"
                );

                self.log_execution(record).await;
                Err(e)
            }
        }
    }

    /// Log command execution
    async fn log_execution(&self, record: CommandExecutionRecord) {
        let mut log = self.execution_log.write().await;
        log.push(record);

        // Keep only the last 1000 records to prevent memory issues
        if log.len() > 1000 {
            let len = log.len();
            log.drain(0..len - 1000);
        }
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> HashMap<String, usize> {
        let log = self.execution_log.read().await;
        let mut stats = HashMap::new();

        for record in log.iter() {
            let key = format!("{}_{:?}", record.command_name, record.status);
            *stats.entry(key).or_insert(0) += 1;
        }

        stats
    }

    /// Get recent execution records
    pub async fn get_recent_executions(&self, limit: usize) -> Vec<CommandExecutionRecord> {
        let log = self.execution_log.read().await;
        log.iter().rev().take(limit).cloned().collect()
    }

    /// Clear execution log (useful for testing)
    pub async fn clear_execution_log(&self) {
        let mut log = self.execution_log.write().await;
        log.clear();
        info!("Cleared command execution log");
    }

    /// Queue a command for asynchronous processing
    pub async fn queue_command<C>(
        &self,
        command: C,
        security_context: Option<SecurityContext>,
    ) -> Result<()>
    where
        C: Command + 'static + for<'de> Deserialize<'de> + Serialize,
    {
        if self.config.enable_authorization && security_context.is_none() {
            return Err(AppError::AuthorizationError(
                "Security context required for queued commands".to_string(),
            ));
        }

        // Check authorization if enabled
        if let Some(ctx) = &security_context {
            let required_permissions = command.required_permissions();
            for permission in required_permissions {
                if !ctx.has_permission(&permission) {
                    return Err(AppError::AuthorizationError(format!(
                        "Missing required permission: {:?}",
                        permission
                    )));
                }
            }
        }

        // Check queue size
        {
            let queue = self.command_queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                return Err(AppError::ResourceExhausted(
                    "Command queue is full".to_string(),
                ));
            }
        }

        // Serialize command
        let command_data = serde_json::to_vec(&command).map_err(|e| {
            AppError::ValidationError(format!("Failed to serialize command: {}", e))
        })?;

        let queued_command = QueuedCommand {
            command_data,
            command_type: std::any::type_name::<C>().to_string(),
            command_id: command.command_id(),
            correlation_id: command.correlation_id(),
            priority: command.priority(),
            queued_at: Utc::now(),
            retry_count: 0,
            batch_id: if self.config.enable_batching && command.can_batch() {
                command.batch_key().map(|_| Uuid::new_v4())
            } else {
                None
            },
            user_id: security_context.as_ref().map(|ctx| ctx.user_id.to_string()),
            security_context,
        };

        // Add to appropriate queue
        if self.config.enable_batching && command.can_batch() {
            if let Some(batch_key) = command.batch_key() {
                let mut batch_queue = self.batch_queue.lock().await;
                batch_queue
                    .entry(batch_key.clone())
                    .or_insert_with(Vec::new)
                    .push(queued_command);

                info!(
                    command_id = %command.command_id(),
                    batch_key = batch_key,
                    "Command added to batch queue"
                );
            } else {
                // Send to regular queue
                if let Some(sender) = self.queue_sender.lock().await.as_ref() {
                    sender.send(queued_command).map_err(|_| {
                        AppError::InternalServerError("Failed to queue command".to_string())
                    })?;
                }
            }
        } else {
            // Send to regular queue
            if let Some(sender) = self.queue_sender.lock().await.as_ref() {
                sender.send(queued_command).map_err(|_| {
                    AppError::InternalServerError("Failed to queue command".to_string())
                })?;
            }
        }

        Ok(())
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        let queue = self.command_queue.lock().await;
        stats.insert("queued_commands".to_string(), queue.len());

        let batch_queue = self.batch_queue.lock().await;
        let total_batched: usize = batch_queue.values().map(|v| v.len()).sum();
        stats.insert("batched_commands".to_string(), total_batched);
        stats.insert("batch_groups".to_string(), batch_queue.len());

        stats
    }

    /// Get command history for audit purposes
    pub async fn get_command_history(
        &self,
        user_id: Option<String>,
        command_name: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Vec<CommandExecutionRecord> {
        let log = self.execution_log.read().await;

        let mut filtered: Vec<CommandExecutionRecord> = log
            .iter()
            .filter(|record| {
                if let Some(ref uid) = user_id {
                    if record.user_id.as_ref() != Some(uid) {
                        return false;
                    }
                }
                if let Some(ref cmd_name) = command_name {
                    if &record.command_name != cmd_name {
                        return false;
                    }
                }
                if let Some(start) = start_time {
                    if record.executed_at < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if record.executed_at > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by execution time (newest first)
        filtered.sort_by(|a, b| b.executed_at.cmp(&a.executed_at));

        // Apply limit
        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        filtered
    }

    /// Force process batch queue (useful for testing)
    pub async fn force_process_batches(&self) -> Result<Vec<BatchExecutionResult>> {
        let mut results = Vec::new();
        let mut batches_to_process = Vec::new();

        {
            let mut batch_queue = self.batch_queue.lock().await;
            for (batch_key, commands) in batch_queue.drain() {
                if !commands.is_empty() {
                    batches_to_process.push((batch_key, commands));
                }
            }
        }

        for (_batch_key, commands) in batches_to_process {
            let batch_id = Uuid::new_v4();
            let start_time = std::time::Instant::now();
            let mut successful_count = 0;
            let mut failed_count = 0;
            let mut errors = Vec::new();

            let handlers = self.handlers.read().await;

            for command in commands {
                if let Some(handler) = handlers.get(&command.command_type) {
                    match handler
                        .handle_command_bytes(
                            &command.command_data,
                            command.security_context.as_ref(),
                        )
                        .await
                    {
                        Ok(_) => successful_count += 1,
                        Err(e) => {
                            failed_count += 1;
                            errors.push(format!("Command {}: {}", command.command_id, e));
                        }
                    }
                } else {
                    failed_count += 1;
                    errors.push(format!(
                        "No handler for command type: {}",
                        command.command_type
                    ));
                }
            }

            let duration = start_time.elapsed();
            let batch_result = BatchExecutionResult {
                batch_id,
                command_count: successful_count + failed_count,
                successful_count,
                failed_count,
                duration_ms: duration.as_millis() as u64,
                errors,
            };

            results.push(batch_result);
        }

        Ok(results)
    }
}

/// Example command implementations
#[derive(Debug, Clone)]
pub struct CreatePipelineCommand {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub stages: Vec<String>,
}

#[async_trait]
impl Command for CreatePipelineCommand {
    type Result = Uuid;

    async fn execute(&self) -> Result<Self::Result> {
        // This would typically delegate to a service or repository
        debug!("Executing CreatePipelineCommand for: {}", self.name);
        Ok(Uuid::new_v4())
    }

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn command_name(&self) -> &'static str {
        "CreatePipeline"
    }

    fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(AppError::ValidationError(
                "Pipeline name cannot be empty".to_string(),
            ));
        }
        if self.stages.is_empty() {
            return Err(AppError::ValidationError(
                "Pipeline must have at least one stage".to_string(),
            ));
        }
        Ok(())
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("pipeline_name".to_string(), self.name.clone());
        metadata.insert("stage_count".to_string(), self.stages.len().to_string());
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommandHandler;

    #[async_trait]
    impl CommandHandler<CreatePipelineCommand> for TestCommandHandler {
        async fn handle(&self, command: CreatePipelineCommand) -> Result<Uuid> {
            command.execute().await
        }

        fn handler_name(&self) -> &'static str {
            "TestCommandHandler"
        }
    }

    #[tokio::test]
    async fn test_command_validation() {
        let command = CreatePipelineCommand {
            command_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            name: "".to_string(),
            description: None,
            stages: vec![],
        };

        assert!(command.validate().is_err());

        let valid_command = CreatePipelineCommand {
            command_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            name: "test-pipeline".to_string(),
            description: None,
            stages: vec!["build".to_string()],
        };

        assert!(valid_command.validate().is_ok());
    }

    #[tokio::test]
    async fn test_command_execution() {
        let command = CreatePipelineCommand {
            command_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            name: "test-pipeline".to_string(),
            description: None,
            stages: vec!["build".to_string()],
        };

        let result = command.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_command_dispatcher() {
        let correlation_tracker = Arc::new(crate::core::CorrelationTracker::new());
        let dispatcher = CommandDispatcher::new(correlation_tracker, None);

        let command = CreatePipelineCommand {
            command_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            name: "test-pipeline".to_string(),
            description: None,
            stages: vec!["build".to_string()],
        };

        let result = dispatcher.dispatch(command).await;
        assert!(result.is_ok());

        let stats = dispatcher.get_execution_stats().await;
        assert!(!stats.is_empty());
    }
}
