// Structured logging with correlation IDs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::{ObservabilityError, CorrelationId};

/// Structured logger with correlation ID support
pub struct StructuredLogger {
    /// Log entries storage
    logs: Arc<RwLock<Vec<LogEntry>>>,
    /// Retention period in seconds
    retention_seconds: u64,
    /// Background cleanup task handle
    cleanup_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Log configuration
    config: LogConfig,
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

/// Individual log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique log entry ID
    pub id: Uuid,
    /// Timestamp in seconds since epoch
    pub timestamp: u64,
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Correlation ID for tracing
    pub correlation_id: Option<CorrelationId>,
    /// Additional context data
    pub context: HashMap<String, serde_json::Value>,
    /// Source component
    pub component: Option<String>,
    /// Thread/task identifier
    pub thread_id: Option<String>,
}

/// Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Minimum log level to record
    pub min_level: LogLevel,
    /// Maximum number of log entries to keep in memory
    pub max_entries: usize,
    /// Enable console output
    pub console_output: bool,
    /// Enable structured JSON output
    pub json_output: bool,
    /// Include stack traces for errors
    pub include_stack_traces: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            max_entries: 10000,
            console_output: true,
            json_output: false,
            include_stack_traces: true,
        }
    }
}

/// Log query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub level: Option<LogLevel>,
    pub correlation_id: Option<CorrelationId>,
    pub component: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub limit: Option<usize>,
    pub search_text: Option<String>,
}

impl StructuredLogger {
    /// Create a new structured logger
    pub fn new(retention_seconds: u64) -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            retention_seconds,
            cleanup_handle: Arc::new(tokio::sync::Mutex::new(None)),
            running: Arc::new(RwLock::new(false)),
            config: LogConfig::default(),
        }
    }

    /// Create a new structured logger with configuration
    pub fn with_config(retention_seconds: u64, config: LogConfig) -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            retention_seconds,
            cleanup_handle: Arc::new(tokio::sync::Mutex::new(None)),
            running: Arc::new(RwLock::new(false)),
            config,
        }
    }

    /// Start the structured logger
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        *running = true;
        drop(running);

        // Start background cleanup task
        let logs = self.logs.clone();
        let retention_seconds = self.retention_seconds;
        let max_entries = self.config.max_entries;
        let running_flag = self.running.clone();

        *self.cleanup_handle.lock().await = Some(tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(30)); // Cleanup every 30 seconds

            while *running_flag.read().await {
                cleanup_interval.tick().await;
                
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let mut logs_guard = logs.write().await;
                
                // Remove old entries
                logs_guard.retain(|entry| {
                    current_time - entry.timestamp < retention_seconds
                });

                // Limit total entries
                if logs_guard.len() > max_entries {
                    let excess = logs_guard.len() - max_entries;
                    logs_guard.drain(0..excess);
                }
            }
        }));

        Ok(())
    }

    /// Stop the structured logger
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        if let Some(handle) = self.cleanup_handle.lock().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Log a message
    pub async fn log(
        &self,
        level: LogLevel,
        message: &str,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<(), ObservabilityError> {
        if level < self.config.min_level {
            return Ok(());
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Logging(format!("Time error: {}", e)))?
            .as_secs();

        let entry = LogEntry {
            id: Uuid::new_v4(),
            timestamp,
            level,
            message: message.to_string(),
            correlation_id: context.get("correlation_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok()),
            context,
            component: None,
            thread_id: Some(format!("{:?}", std::thread::current().id())),
        };

        // Output to console if enabled
        if self.config.console_output {
            self.output_to_console(&entry);
        }

        // Store in memory
        let mut logs = self.logs.write().await;
        logs.push(entry);

        Ok(())
    }

    /// Log with correlation ID
    pub async fn log_with_correlation(
        &self,
        correlation_id: CorrelationId,
        level: LogLevel,
        message: &str,
        mut context: HashMap<String, serde_json::Value>,
    ) -> Result<(), ObservabilityError> {
        context.insert(
            "correlation_id".to_string(),
            serde_json::Value::String(correlation_id.to_string()),
        );
        self.log(level, message, context).await
    }

    /// Log with component information
    pub async fn log_with_component(
        &self,
        component: &str,
        level: LogLevel,
        message: &str,
        mut context: HashMap<String, serde_json::Value>,
    ) -> Result<(), ObservabilityError> {
        context.insert(
            "component".to_string(),
            serde_json::Value::String(component.to_string()),
        );
        self.log(level, message, context).await
    }

    /// Convenience methods for different log levels
    pub async fn trace(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Trace, message, HashMap::new()).await
    }

    pub async fn debug(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Debug, message, HashMap::new()).await
    }

    pub async fn info(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Info, message, HashMap::new()).await
    }

    pub async fn warn(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Warn, message, HashMap::new()).await
    }

    pub async fn error(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Error, message, HashMap::new()).await
    }

    pub async fn fatal(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LogLevel::Fatal, message, HashMap::new()).await
    }

    /// Query log entries
    pub async fn query(&self, query: LogQuery) -> Result<Vec<LogEntry>, ObservabilityError> {
        let logs = self.logs.read().await;
        let mut results = Vec::new();

        for entry in logs.iter() {
            // Filter by level
            if let Some(level) = query.level {
                if entry.level < level {
                    continue;
                }
            }

            // Filter by correlation ID
            if let Some(correlation_id) = query.correlation_id {
                if entry.correlation_id != Some(correlation_id) {
                    continue;
                }
            }

            // Filter by component
            if let Some(ref component) = query.component {
                if entry.component.as_ref() != Some(component) {
                    continue;
                }
            }

            // Filter by time range
            if let Some(start) = query.start_time {
                if entry.timestamp < start {
                    continue;
                }
            }
            if let Some(end) = query.end_time {
                if entry.timestamp > end {
                    continue;
                }
            }

            // Filter by search text
            if let Some(ref search_text) = query.search_text {
                if !entry.message.contains(search_text) {
                    continue;
                }
            }

            results.push(entry.clone());
        }

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Get log count
    pub async fn count(&self) -> usize {
        let logs = self.logs.read().await;
        logs.len()
    }

    /// Get recent logs
    pub async fn recent(&self, count: usize) -> Vec<LogEntry> {
        let logs = self.logs.read().await;
        let start = if logs.len() > count {
            logs.len() - count
        } else {
            0
        };
        logs[start..].to_vec()
    }

    /// Get logs by correlation ID
    pub async fn by_correlation(&self, correlation_id: CorrelationId) -> Vec<LogEntry> {
        let query = LogQuery {
            correlation_id: Some(correlation_id),
            level: None,
            component: None,
            start_time: None,
            end_time: None,
            limit: None,
            search_text: None,
        };
        self.query(query).await.unwrap_or_default()
    }

    /// Output log entry to console
    fn output_to_console(&self, entry: &LogEntry) {
        let level_str = match entry.level {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        };

        let timestamp = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M:%S");

        let correlation_part = entry.correlation_id
            .map(|id| format!(" [{}]", id))
            .unwrap_or_default();

        if self.config.json_output {
            if let Ok(json) = serde_json::to_string(entry) {
                println!("{}", json);
            }
        } else {
            println!(
                "{} [{}]{} {}",
                timestamp,
                level_str,
                correlation_part,
                entry.message
            );
        }
    }

    /// Get logging statistics
    pub async fn statistics(&self) -> LogStatistics {
        let logs = self.logs.read().await;
        let mut level_counts = HashMap::new();
        let mut component_counts = HashMap::new();
        let mut correlation_counts = HashMap::new();

        for entry in logs.iter() {
            *level_counts.entry(entry.level).or_insert(0) += 1;
            
            if let Some(ref component) = entry.component {
                *component_counts.entry(component.clone()).or_insert(0) += 1;
            }
            
            if let Some(correlation_id) = entry.correlation_id {
                *correlation_counts.entry(correlation_id).or_insert(0) += 1;
            }
        }

        LogStatistics {
            total_entries: logs.len(),
            level_counts,
            component_counts,
            correlation_counts,
            retention_seconds: self.retention_seconds,
        }
    }
}

/// Logging statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStatistics {
    pub total_entries: usize,
    pub level_counts: HashMap<LogLevel, usize>,
    pub component_counts: HashMap<String, usize>,
    pub correlation_counts: HashMap<CorrelationId, usize>,
    pub retention_seconds: u64,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(LogLevel::Trace),
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            "FATAL" => Ok(LogLevel::Fatal),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}