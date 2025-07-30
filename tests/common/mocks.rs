use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Mock external services
pub struct MockExternalServices {
    pub github: MockGitHubService,
    pub docker: MockDockerService,
    pub kubernetes: MockKubernetesService,
    pub notification: MockNotificationService,
}

impl MockExternalServices {
    pub fn new() -> Self {
        Self {
            github: MockGitHubService::new(),
            docker: MockDockerService::new(),
            kubernetes: MockKubernetesService::new(),
            notification: MockNotificationService::new(),
        }
    }

    pub async fn reset(&self) {
        self.github.reset().await;
        self.docker.reset().await;
        self.kubernetes.reset().await;
        self.notification.reset().await;
    }
}

/// Mock GitHub service
pub struct MockGitHubService {
    responses: Arc<RwLock<HashMap<String, Value>>>,
    call_count: Arc<RwLock<HashMap<String, usize>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl MockGitHubService {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(HashMap::new())),
            call_count: Arc::new(RwLock::new(HashMap::new())),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn set_response(&self, endpoint: &str, response: Value) {
        self.responses
            .write()
            .await
            .insert(endpoint.to_string(), response);
    }

    pub async fn get_call_count(&self, endpoint: &str) -> usize {
        self.call_count
            .read()
            .await
            .get(endpoint)
            .copied()
            .unwrap_or(0)
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }

    pub async fn reset(&self) {
        self.responses.write().await.clear();
        self.call_count.write().await.clear();
        *self.should_fail.write().await = false;
    }

    pub async fn mock_call(&self, endpoint: &str) -> Result<Value, MockError> {
        // Increment call count
        {
            let mut counts = self.call_count.write().await;
            let count = counts.entry(endpoint.to_string()).or_insert(0);
            *count += 1;
        }

        // Check if should fail
        if *self.should_fail.read().await {
            return Err(MockError::ServiceUnavailable);
        }

        // Return mock response
        self.responses
            .read()
            .await
            .get(endpoint)
            .cloned()
            .ok_or(MockError::EndpointNotMocked)
    }
}

/// Mock Docker service
pub struct MockDockerService {
    containers: Arc<RwLock<HashMap<String, MockContainer>>>,
    images: Arc<RwLock<HashMap<String, MockImage>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl MockDockerService {
    pub fn new() -> Self {
        Self {
            containers: Arc::new(RwLock::new(HashMap::new())),
            images: Arc::new(RwLock::new(HashMap::new())),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn add_container(&self, id: &str, container: MockContainer) {
        self.containers
            .write()
            .await
            .insert(id.to_string(), container);
    }

    pub async fn add_image(&self, name: &str, image: MockImage) {
        self.images.write().await.insert(name.to_string(), image);
    }

    pub async fn get_container(&self, id: &str) -> Option<MockContainer> {
        self.containers.read().await.get(id).cloned()
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }

    pub async fn reset(&self) {
        self.containers.write().await.clear();
        self.images.write().await.clear();
        *self.should_fail.write().await = false;
    }
}

/// Mock Kubernetes service
pub struct MockKubernetesService {
    pods: Arc<RwLock<HashMap<String, MockPod>>>,
    deployments: Arc<RwLock<HashMap<String, MockDeployment>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl MockKubernetesService {
    pub fn new() -> Self {
        Self {
            pods: Arc::new(RwLock::new(HashMap::new())),
            deployments: Arc::new(RwLock::new(HashMap::new())),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn add_pod(&self, name: &str, pod: MockPod) {
        self.pods.write().await.insert(name.to_string(), pod);
    }

    pub async fn add_deployment(&self, name: &str, deployment: MockDeployment) {
        self.deployments
            .write()
            .await
            .insert(name.to_string(), deployment);
    }

    pub async fn get_pod(&self, name: &str) -> Option<MockPod> {
        self.pods.read().await.get(name).cloned()
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }

    pub async fn reset(&self) {
        self.pods.write().await.clear();
        self.deployments.write().await.clear();
        *self.should_fail.write().await = false;
    }
}

/// Mock notification service
pub struct MockNotificationService {
    sent_notifications: Arc<RwLock<Vec<MockNotification>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl MockNotificationService {
    pub fn new() -> Self {
        Self {
            sent_notifications: Arc::new(RwLock::new(Vec::new())),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn send_notification(&self, notification: MockNotification) -> Result<(), MockError> {
        if *self.should_fail.read().await {
            return Err(MockError::ServiceUnavailable);
        }

        self.sent_notifications.write().await.push(notification);
        Ok(())
    }

    pub async fn get_sent_notifications(&self) -> Vec<MockNotification> {
        self.sent_notifications.read().await.clone()
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }

    pub async fn reset(&self) {
        self.sent_notifications.write().await.clear();
        *self.should_fail.write().await = false;
    }
}

/// Mock message bus for testing event-driven components
pub struct TestMessageBus {
    events: Arc<RwLock<Vec<TestEvent>>>,
    subscribers: Arc<RwLock<HashMap<String, Vec<mpsc::UnboundedSender<TestEvent>>>>>,
}

impl TestMessageBus {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn publish(&self, event: TestEvent) {
        // Store event
        self.events.write().await.push(event.clone());

        // Send to subscribers
        let subscribers = self.subscribers.read().await;
        if let Some(senders) = subscribers.get(&event.event_type) {
            for sender in senders {
                let _ = sender.send(event.clone());
            }
        }
    }

    pub async fn subscribe(&self, event_type: &str) -> mpsc::UnboundedReceiver<TestEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        self.subscribers
            .write()
            .await
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(tx);

        rx
    }

    pub async fn get_events(&self) -> Vec<TestEvent> {
        self.events.read().await.clone()
    }

    pub async fn get_events_by_type(&self, event_type: &str) -> Vec<TestEvent> {
        self.events
            .read()
            .await
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    pub async fn cleanup(&self) {
        self.events.write().await.clear();
        self.subscribers.write().await.clear();
    }
}

// Mock data structures
#[derive(Debug, Clone)]
pub struct MockContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MockImage {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct MockPod {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub containers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MockDeployment {
    pub name: String,
    pub namespace: String,
    pub replicas: u32,
    pub ready_replicas: u32,
}

#[derive(Debug, Clone)]
pub struct MockNotification {
    pub id: Uuid,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub notification_type: String,
}

#[derive(Debug, Clone)]
pub struct TestEvent {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_id: Uuid,
    pub data: Value,
    pub correlation_id: Uuid,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

/// Mock error types
#[derive(Debug, thiserror::Error)]
pub enum MockError {
    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Endpoint not mocked: {0}")]
    EndpointNotMocked,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Resource not found: {0}")]
    NotFound(String),
}

/// Mock factory for creating test services
pub struct MockServiceFactory {
    mocks: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl MockServiceFactory {
    pub fn new() -> Self {
        Self {
            mocks: HashMap::new(),
        }
    }

    pub fn register<T: 'static + Send + Sync>(&mut self, name: &str, mock: T) {
        self.mocks.insert(name.to_string(), Box::new(mock));
    }

    pub fn get<T: 'static>(&self, name: &str) -> Option<&T> {
        self.mocks
            .get(name)
            .and_then(|mock| mock.downcast_ref::<T>())
    }
}
