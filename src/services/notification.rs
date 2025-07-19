#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{ValidationResult, ProjectType, GitHubPullRequest},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    DockerfileGenerated,
    DockerfileValidated,
    PullRequestCreated,
    WorkspaceCreated,
    WorkspaceUpdated,
    RepositoryLinked,
    ValidationStarted,
    ValidationCompleted,
    BuildStarted,
    BuildCompleted,
    DeploymentStarted,
    DeploymentCompleted,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub workspace_id: Uuid,
    pub repository_id: Option<i64>,
    pub user_id: Option<Uuid>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: EventData,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventData {
    DockerfileGenerated {
        project_type: ProjectType,
        dockerfile_content: String,
        generation_id: Uuid,
    },
    DockerfileValidated {
        validation_result: ValidationResult,
        dockerfile_content: String,
        generation_id: Uuid,
    },
    PullRequestCreated {
        pull_request: GitHubPullRequest,
        dockerfile_content: String,
    },
    WorkspaceCreated {
        workspace_id: Uuid,
        github_username: String,
    },
    WorkspaceUpdated {
        workspace_id: Uuid,
        changes: Vec<String>,
    },
    RepositoryLinked {
        repository_name: String,
        repository_id: i64,
        project_type: Option<ProjectType>,
    },
    ValidationStarted {
        generation_id: Uuid,
        validator_type: String,
    },
    ValidationCompleted {
        generation_id: Uuid,
        success: bool,
        duration_ms: u64,
    },
    BuildStarted {
        build_id: Uuid,
        dockerfile_path: String,
    },
    BuildCompleted {
        build_id: Uuid,
        success: bool,
        duration_ms: u64,
        image_tag: Option<String>,
    },
    DeploymentStarted {
        deployment_id: Uuid,
        target_environment: String,
    },
    DeploymentCompleted {
        deployment_id: Uuid,
        success: bool,
        endpoint: Option<String>,
    },
    Error {
        error_message: String,
        error_code: Option<String>,
        context: std::collections::HashMap<String, String>,
    },
}

impl Event {
    pub fn new(event_type: EventType, workspace_id: Uuid, data: EventData) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            workspace_id,
            repository_id: None,
            user_id: None,
            timestamp: chrono::Utc::now(),
            data,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_repository(mut self, repository_id: i64) -> Self {
        self.repository_id = Some(repository_id);
        self
    }
    
    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[async_trait]
pub trait Observer: Send + Sync {
    async fn notify(&self, event: &Event) -> Result<()>;
    fn event_types(&self) -> Vec<EventType>;
    fn observer_id(&self) -> String;
    fn is_enabled(&self) -> bool;
}

pub struct EventPublisher {
    observers: Vec<Arc<dyn Observer>>,
    event_history: std::sync::RwLock<Vec<Event>>,
    max_history_size: usize,
}

impl EventPublisher {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
            event_history: std::sync::RwLock::new(Vec::new()),
            max_history_size: 1000,
        }
    }
    
    pub fn with_max_history_size(mut self, size: usize) -> Self {
        self.max_history_size = size;
        self
    }
    
    pub fn subscribe(&mut self, observer: Arc<dyn Observer>) {
        tracing::info!("Subscribing observer: {}", observer.observer_id());
        self.observers.push(observer);
    }
    
    pub fn unsubscribe(&mut self, observer_id: &str) {
        self.observers.retain(|observer| observer.observer_id() != observer_id);
        tracing::info!("Unsubscribed observer: {}", observer_id);
    }
    
    pub async fn publish(&self, event: Event) -> Result<()> {
        tracing::info!("Publishing event: {:?} for workspace: {}", event.event_type, event.workspace_id);
        
        // Store event in history
        self.store_event_in_history(event.clone());
        
        // Notify all relevant observers
        let mut notification_errors = Vec::new();
        
        for observer in &self.observers {
            if observer.is_enabled() && observer.event_types().contains(&event.event_type) {
                tracing::debug!("Notifying observer: {} for event: {:?}", observer.observer_id(), event.event_type);
                
                if let Err(e) = observer.notify(&event).await {
                    tracing::error!("Observer {} failed to handle event: {}", observer.observer_id(), e);
                    notification_errors.push(format!("{}: {}", observer.observer_id(), e));
                }
            }
        }
        
        if !notification_errors.is_empty() {
            tracing::warn!("Some observers failed to handle event: {}", notification_errors.join(", "));
            // Don't fail the entire publish operation if some observers fail
        }
        
        tracing::debug!("Event published successfully: {}", event.id);
        Ok(())
    }
    
    pub async fn publish_multiple(&self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }
    
    pub fn get_event_history(&self, limit: Option<usize>) -> Vec<Event> {
        if let Ok(history) = self.event_history.read() {
            let limit = limit.unwrap_or(history.len());
            history.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn get_events_by_type(&self, event_type: EventType, limit: Option<usize>) -> Vec<Event> {
        if let Ok(history) = self.event_history.read() {
            let filtered: Vec<Event> = history
                .iter()
                .filter(|event| event.event_type == event_type)
                .rev()
                .take(limit.unwrap_or(history.len()))
                .cloned()
                .collect();
            filtered
        } else {
            Vec::new()
        }
    }
    
    pub fn get_events_by_workspace(&self, workspace_id: Uuid, limit: Option<usize>) -> Vec<Event> {
        if let Ok(history) = self.event_history.read() {
            let filtered: Vec<Event> = history
                .iter()
                .filter(|event| event.workspace_id == workspace_id)
                .rev()
                .take(limit.unwrap_or(history.len()))
                .cloned()
                .collect();
            filtered
        } else {
            Vec::new()
        }
    }
    
    pub fn clear_history(&self) {
        if let Ok(mut history) = self.event_history.write() {
            history.clear();
            tracing::info!("Event history cleared");
        }
    }
    
    pub fn observer_count(&self) -> usize {
        self.observers.len()
    }
    
    pub fn active_observer_count(&self) -> usize {
        self.observers.iter().filter(|o| o.is_enabled()).count()
    }
    
    fn store_event_in_history(&self, event: Event) {
        if let Ok(mut history) = self.event_history.write() {
            history.push(event);
            
            // Trim history if it exceeds max size
            if history.len() > self.max_history_size {
                let excess = history.len() - self.max_history_size;
                history.drain(0..excess);
            }
        }
    }
}

impl Default for EventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

// Specific Observer Implementations

pub struct DockerfileValidationObserver {
    id: String,
    enabled: bool,
}

impl DockerfileValidationObserver {
    pub fn new() -> Self {
        Self {
            id: "dockerfile_validation_observer".to_string(),
            enabled: true,
        }
    }
    
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[async_trait]
impl Observer for DockerfileValidationObserver {
    async fn notify(&self, event: &Event) -> Result<()> {
        match &event.data {
            EventData::DockerfileValidated { validation_result, generation_id, .. } => {
                if validation_result.success {
                    tracing::info!("✅ Dockerfile validation successful for generation: {}", generation_id);
                    // Could send success notification to user, update database, etc.
                } else {
                    tracing::warn!("❌ Dockerfile validation failed for generation: {}", generation_id);
                    tracing::warn!("Errors: {:?}", validation_result.errors);
                    // Could send failure notification, create issue, etc.
                }
            }
            EventData::ValidationStarted { generation_id, validator_type } => {
                tracing::info!("🔄 Dockerfile validation started for generation: {} using {}", generation_id, validator_type);
            }
            EventData::ValidationCompleted { generation_id, success, duration_ms } => {
                tracing::info!("✅ Dockerfile validation completed for generation: {} in {}ms (success: {})", 
                    generation_id, duration_ms, success);
            }
            _ => {
                tracing::debug!("DockerfileValidationObserver received unhandled event type: {:?}", event.event_type);
            }
        }
        Ok(())
    }
    
    fn event_types(&self) -> Vec<EventType> {
        vec![
            EventType::DockerfileValidated,
            EventType::ValidationStarted,
            EventType::ValidationCompleted,
        ]
    }
    
    fn observer_id(&self) -> String {
        self.id.clone()
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

pub struct PullRequestObserver {
    id: String,
    enabled: bool,
}

impl PullRequestObserver {
    pub fn new() -> Self {
        Self {
            id: "pull_request_observer".to_string(),
            enabled: true,
        }
    }
    
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[async_trait]
impl Observer for PullRequestObserver {
    async fn notify(&self, event: &Event) -> Result<()> {
        match &event.data {
            EventData::PullRequestCreated { pull_request, .. } => {
                tracing::info!("🎉 Pull request created: {} (#{}) - {}", 
                    pull_request.title, pull_request.number, pull_request.html_url);
                
                // Could send notification to team, update project management tools, etc.
                // For now, just log the success
            }
            _ => {
                tracing::debug!("PullRequestObserver received unhandled event type: {:?}", event.event_type);
            }
        }
        Ok(())
    }
    
    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::PullRequestCreated]
    }
    
    fn observer_id(&self) -> String {
        self.id.clone()
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

pub struct WorkspaceObserver {
    id: String,
    enabled: bool,
}

impl WorkspaceObserver {
    pub fn new() -> Self {
        Self {
            id: "workspace_observer".to_string(),
            enabled: true,
        }
    }
    
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[async_trait]
impl Observer for WorkspaceObserver {
    async fn notify(&self, event: &Event) -> Result<()> {
        match &event.data {
            EventData::WorkspaceCreated { workspace_id, github_username } => {
                tracing::info!("🏗️ Workspace created for user: {} (ID: {})", github_username, workspace_id);
                // Could initialize default settings, send welcome message, etc.
            }
            EventData::WorkspaceUpdated { workspace_id, changes } => {
                tracing::info!("🔄 Workspace updated: {} - Changes: {:?}", workspace_id, changes);
                // Could log changes, notify team members, etc.
            }
            EventData::RepositoryLinked { repository_name, repository_id, project_type } => {
                tracing::info!("🔗 Repository linked: {} (ID: {}) - Type: {:?}", 
                    repository_name, repository_id, project_type);
                // Could trigger initial analysis, setup webhooks, etc.
            }
            _ => {
                tracing::debug!("WorkspaceObserver received unhandled event type: {:?}", event.event_type);
            }
        }
        Ok(())
    }
    
    fn event_types(&self) -> Vec<EventType> {
        vec![
            EventType::WorkspaceCreated,
            EventType::WorkspaceUpdated,
            EventType::RepositoryLinked,
        ]
    }
    
    fn observer_id(&self) -> String {
        self.id.clone()
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

pub struct ErrorObserver {
    id: String,
    enabled: bool,
}

impl ErrorObserver {
    pub fn new() -> Self {
        Self {
            id: "error_observer".to_string(),
            enabled: true,
        }
    }
    
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[async_trait]
impl Observer for ErrorObserver {
    async fn notify(&self, event: &Event) -> Result<()> {
        match &event.data {
            EventData::Error { error_message, error_code, context } => {
                tracing::error!("🚨 Error event: {} (Code: {:?})", error_message, error_code);
                tracing::error!("Context: {:?}", context);
                
                // Could send alerts, create incidents, notify on-call team, etc.
                // For now, just ensure it's logged at ERROR level
            }
            _ => {
                tracing::debug!("ErrorObserver received unhandled event type: {:?}", event.event_type);
            }
        }
        Ok(())
    }
    
    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::Error]
    }
    
    fn observer_id(&self) -> String {
        self.id.clone()
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// Factory for creating and configuring observers
pub struct ObserverFactory;

impl ObserverFactory {
    pub fn create_dockerfile_validation_observer() -> Arc<dyn Observer> {
        Arc::new(DockerfileValidationObserver::new())
    }
    
    pub fn create_pull_request_observer() -> Arc<dyn Observer> {
        Arc::new(PullRequestObserver::new())
    }
    
    pub fn create_workspace_observer() -> Arc<dyn Observer> {
        Arc::new(WorkspaceObserver::new())
    }
    
    pub fn create_error_observer() -> Arc<dyn Observer> {
        Arc::new(ErrorObserver::new())
    }
    
    pub fn create_all_observers() -> Vec<Arc<dyn Observer>> {
        vec![
            Self::create_dockerfile_validation_observer(),
            Self::create_pull_request_observer(),
            Self::create_workspace_observer(),
            Self::create_error_observer(),
        ]
    }
    
    pub fn create_default_publisher() -> EventPublisher {
        let mut publisher = EventPublisher::new();
        
        for observer in Self::create_all_observers() {
            publisher.subscribe(observer);
        }
        
        publisher
    }
}

// Service for managing notifications across the application
pub struct NotificationService {
    publisher: Arc<std::sync::RwLock<EventPublisher>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            publisher: Arc::new(std::sync::RwLock::new(ObserverFactory::create_default_publisher())),
        }
    }
    
    pub fn with_publisher(publisher: EventPublisher) -> Self {
        Self {
            publisher: Arc::new(std::sync::RwLock::new(publisher)),
        }
    }
    
    pub async fn notify_dockerfile_generated(
        &self,
        workspace_id: Uuid,
        repository_id: i64,
        project_type: ProjectType,
        dockerfile_content: String,
        generation_id: Uuid,
    ) -> Result<()> {
        let event = Event::new(
            EventType::DockerfileGenerated,
            workspace_id,
            EventData::DockerfileGenerated {
                project_type,
                dockerfile_content,
                generation_id,
            },
        ).with_repository(repository_id);
        
        if let Ok(publisher) = self.publisher.read() {
            publisher.publish(event).await
        } else {
            Err(AppError::InternalServerError("Failed to acquire publisher lock".to_string()))
        }
    }
    
    pub async fn notify_dockerfile_validated(
        &self,
        workspace_id: Uuid,
        repository_id: i64,
        validation_result: ValidationResult,
        dockerfile_content: String,
        generation_id: Uuid,
    ) -> Result<()> {
        let event = Event::new(
            EventType::DockerfileValidated,
            workspace_id,
            EventData::DockerfileValidated {
                validation_result,
                dockerfile_content,
                generation_id,
            },
        ).with_repository(repository_id);
        
        if let Ok(publisher) = self.publisher.read() {
            publisher.publish(event).await
        } else {
            Err(AppError::InternalServerError("Failed to acquire publisher lock".to_string()))
        }
    }
    
    pub async fn notify_pull_request_created(
        &self,
        workspace_id: Uuid,
        repository_id: i64,
        pull_request: GitHubPullRequest,
        dockerfile_content: String,
    ) -> Result<()> {
        let event = Event::new(
            EventType::PullRequestCreated,
            workspace_id,
            EventData::PullRequestCreated {
                pull_request,
                dockerfile_content,
            },
        ).with_repository(repository_id);
        
        if let Ok(publisher) = self.publisher.read() {
            publisher.publish(event).await
        } else {
            Err(AppError::InternalServerError("Failed to acquire publisher lock".to_string()))
        }
    }
    
    pub async fn notify_error(
        &self,
        workspace_id: Uuid,
        error_message: String,
        error_code: Option<String>,
        context: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let event = Event::new(
            EventType::Error,
            workspace_id,
            EventData::Error {
                error_message,
                error_code,
                context,
            },
        );
        
        if let Ok(publisher) = self.publisher.read() {
            publisher.publish(event).await
        } else {
            Err(AppError::InternalServerError("Failed to acquire publisher lock".to_string()))
        }
    }
    
    pub fn get_event_history(&self, limit: Option<usize>) -> Vec<Event> {
        if let Ok(publisher) = self.publisher.read() {
            publisher.get_event_history(limit)
        } else {
            Vec::new()
        }
    }
    
    pub fn subscribe_observer(&self, observer: Arc<dyn Observer>) -> Result<()> {
        if let Ok(mut publisher) = self.publisher.write() {
            publisher.subscribe(observer);
            Ok(())
        } else {
            Err(AppError::InternalServerError("Failed to acquire publisher lock".to_string()))
        }
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_event_creation() {
        let workspace_id = Uuid::new_v4();
        let generation_id = Uuid::new_v4();
        
        let event = Event::new(
            EventType::DockerfileGenerated,
            workspace_id,
            EventData::DockerfileGenerated {
                project_type: ProjectType::Rust,
                dockerfile_content: "FROM rust:1.75".to_string(),
                generation_id,
            },
        ).with_repository(123)
          .with_user(Uuid::new_v4())
          .with_metadata("source".to_string(), "test".to_string());
        
        assert_eq!(event.event_type, EventType::DockerfileGenerated);
        assert_eq!(event.workspace_id, workspace_id);
        assert_eq!(event.repository_id, Some(123));
        assert!(event.user_id.is_some());
        assert_eq!(event.metadata.get("source"), Some(&"test".to_string()));
    }
    
    #[tokio::test]
    async fn test_event_publisher() {
        let mut publisher = EventPublisher::new();
        let observer = Arc::new(DockerfileValidationObserver::new());
        
        publisher.subscribe(observer);
        assert_eq!(publisher.observer_count(), 1);
        assert_eq!(publisher.active_observer_count(), 1);
        
        let event = Event::new(
            EventType::DockerfileValidated,
            Uuid::new_v4(),
            EventData::DockerfileValidated {
                validation_result: ValidationResult {
                    success: true,
                    build_logs: vec!["Build successful".to_string()],
                    run_logs: vec!["Run successful".to_string()],
                    errors: vec![],
                    warnings: vec![],
                },
                dockerfile_content: "FROM rust:1.75".to_string(),
                generation_id: Uuid::new_v4(),
            },
        );
        
        let result = publisher.publish(event).await;
        assert!(result.is_ok());
        
        let history = publisher.get_event_history(Some(10));
        assert_eq!(history.len(), 1);
    }
    
    #[tokio::test]
    async fn test_observer_filtering() {
        let mut publisher = EventPublisher::new();
        let validation_observer = Arc::new(DockerfileValidationObserver::new());
        let pr_observer = Arc::new(PullRequestObserver::new());
        
        publisher.subscribe(validation_observer);
        publisher.subscribe(pr_observer);
        
        // This event should only be handled by validation observer
        let validation_event = Event::new(
            EventType::DockerfileValidated,
            Uuid::new_v4(),
            EventData::DockerfileValidated {
                validation_result: ValidationResult {
                    success: true,
                    build_logs: vec![],
                    run_logs: vec![],
                    errors: vec![],
                    warnings: vec![],
                },
                dockerfile_content: "FROM rust:1.75".to_string(),
                generation_id: Uuid::new_v4(),
            },
        );
        
        let result = publisher.publish(validation_event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_notification_service() {
        let service = NotificationService::new();
        let workspace_id = Uuid::new_v4();
        let generation_id = Uuid::new_v4();
        
        let result = service.notify_dockerfile_generated(
            workspace_id,
            123,
            ProjectType::Rust,
            "FROM rust:1.75".to_string(),
            generation_id,
        ).await;
        
        assert!(result.is_ok());
        
        let history = service.get_event_history(Some(10));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, EventType::DockerfileGenerated);
    }
    
    #[tokio::test]
    async fn test_observer_factory() {
        let observers = ObserverFactory::create_all_observers();
        assert_eq!(observers.len(), 4);
        
        let publisher = ObserverFactory::create_default_publisher();
        assert_eq!(publisher.observer_count(), 4);
        assert_eq!(publisher.active_observer_count(), 4);
    }
    
    #[test]
    fn test_event_filtering() {
        let publisher = EventPublisher::new();
        
        // Add some test events to history (this would normally be done through publish)
        let workspace_id = Uuid::new_v4();
        let event1 = Event::new(
            EventType::DockerfileGenerated,
            workspace_id,
            EventData::DockerfileGenerated {
                project_type: ProjectType::Rust,
                dockerfile_content: "FROM rust:1.75".to_string(),
                generation_id: Uuid::new_v4(),
            },
        );
        
        let event2 = Event::new(
            EventType::DockerfileValidated,
            workspace_id,
            EventData::DockerfileValidated {
                validation_result: ValidationResult {
                    success: true,
                    build_logs: vec![],
                    run_logs: vec![],
                    errors: vec![],
                    warnings: vec![],
                },
                dockerfile_content: "FROM rust:1.75".to_string(),
                generation_id: Uuid::new_v4(),
            },
        );
        
        // Store events in history manually for testing
        publisher.store_event_in_history(event1);
        publisher.store_event_in_history(event2);
        
        let generated_events = publisher.get_events_by_type(EventType::DockerfileGenerated, None);
        assert_eq!(generated_events.len(), 1);
        
        let workspace_events = publisher.get_events_by_workspace(workspace_id, None);
        assert_eq!(workspace_events.len(), 2);
    }
}