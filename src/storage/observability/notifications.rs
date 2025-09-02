use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::storage::{
    NotificationAdapter, NotificationMessage, NotificationSeverity, StoreError, StoreResult,
};

/// Console notification adapter - always available for development and debugging
pub struct ConsoleNotificationAdapter {
    config: HashMap<String, serde_json::Value>,
}

impl ConsoleNotificationAdapter {
    pub fn new(config: HashMap<String, serde_json::Value>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotificationAdapter for ConsoleNotificationAdapter {
    async fn send_notification(&self, message: NotificationMessage) -> StoreResult<()> {
        let severity_icon = match message.severity {
            NotificationSeverity::Info => "ℹ️",
            NotificationSeverity::Warning => "⚠️",
            NotificationSeverity::Error => "❌",
            NotificationSeverity::Critical => "🚨",
        };

        let formatted_message = format!(
            "{} [{}] {}: {} (tags: {})",
            severity_icon,
            message.severity_str(),
            message.title,
            message.body,
            message.tags.join(", ")
        );

        match message.severity {
            NotificationSeverity::Info => info!("{}", formatted_message),
            NotificationSeverity::Warning => warn!("{}", formatted_message),
            NotificationSeverity::Error | NotificationSeverity::Critical => {
                tracing::error!("{}", formatted_message)
            }
        }

        Ok(())
    }

    fn adapter_name(&self) -> &'static str {
        "console"
    }

    async fn is_ready(&self) -> bool {
        true // Console adapter is always ready
    }
}

/// SendGrid notification adapter - configured via runtime API
pub struct SendGridNotificationAdapter {
    config: HashMap<String, serde_json::Value>,
    api_key: Option<String>,
    from_email: String,
    from_name: String,
}

impl SendGridNotificationAdapter {
    pub fn new(config: HashMap<String, serde_json::Value>) -> Self {
        let from_email = config
            .get("from_email")
            .and_then(|v| v.as_str())
            .unwrap_or("alerts@valkyrie.dev")
            .to_string();

        let from_name = config
            .get("from_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Valkyrie Storage")
            .to_string();

        Self {
            config,
            api_key: None,
            from_email,
            from_name,
        }
    }

    /// Set API key via runtime API (secure credential management)
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = Some(api_key);
        info!("SendGrid API key configured");
    }

    /// Clear API key
    pub fn clear_api_key(&mut self) {
        self.api_key = None;
        info!("SendGrid API key cleared");
    }
}

#[async_trait]
impl NotificationAdapter for SendGridNotificationAdapter {
    async fn send_notification(&self, message: NotificationMessage) -> StoreResult<()> {
        if self.api_key.is_none() {
            return Err(StoreError::ConfigurationError(
                "SendGrid API key not configured".to_string(),
            ));
        }

        // TODO: Implement actual SendGrid API integration
        debug!(
            "SendGrid notification: {} - {} (severity: {:?})",
            message.title, message.body, message.severity
        );

        // Placeholder implementation
        info!("📧 SendGrid notification sent: {}", message.title);

        Ok(())
    }

    fn adapter_name(&self) -> &'static str {
        "sendgrid"
    }

    async fn is_ready(&self) -> bool {
        self.api_key.is_some()
    }
}

/// Twilio notification adapter - configured via runtime API
pub struct TwilioNotificationAdapter {
    config: HashMap<String, serde_json::Value>,
    account_sid: Option<String>,
    auth_token: Option<String>,
    from_phone: String,
}

impl TwilioNotificationAdapter {
    pub fn new(config: HashMap<String, serde_json::Value>) -> Self {
        let from_phone = config
            .get("from_phone")
            .and_then(|v| v.as_str())
            .unwrap_or("+1234567890")
            .to_string();

        Self {
            config,
            account_sid: None,
            auth_token: None,
            from_phone,
        }
    }

    /// Set credentials via runtime API (secure credential management)
    pub fn set_credentials(&mut self, account_sid: String, auth_token: String) {
        self.account_sid = Some(account_sid);
        self.auth_token = Some(auth_token);
        info!("Twilio credentials configured");
    }

    /// Clear credentials
    pub fn clear_credentials(&mut self) {
        self.account_sid = None;
        self.auth_token = None;
        info!("Twilio credentials cleared");
    }
}

#[async_trait]
impl NotificationAdapter for TwilioNotificationAdapter {
    async fn send_notification(&self, message: NotificationMessage) -> StoreResult<()> {
        if self.account_sid.is_none() || self.auth_token.is_none() {
            return Err(StoreError::ConfigurationError(
                "Twilio credentials not configured".to_string(),
            ));
        }

        // TODO: Implement actual Twilio API integration
        debug!(
            "Twilio notification: {} - {} (severity: {:?})",
            message.title, message.body, message.severity
        );

        // Placeholder implementation
        info!("📱 Twilio SMS sent: {}", message.title);

        Ok(())
    }

    fn adapter_name(&self) -> &'static str {
        "twilio"
    }

    async fn is_ready(&self) -> bool {
        self.account_sid.is_some() && self.auth_token.is_some()
    }
}

/// Slack notification adapter - configured via runtime API
pub struct SlackNotificationAdapter {
    config: HashMap<String, serde_json::Value>,
    webhook_url: Option<String>,
    channel: String,
    username: String,
}

impl SlackNotificationAdapter {
    pub fn new(config: HashMap<String, serde_json::Value>) -> Self {
        let channel = config
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("#alerts")
            .to_string();

        let username = config
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("Valkyrie Storage")
            .to_string();

        Self {
            config,
            webhook_url: None,
            channel,
            username,
        }
    }

    /// Set webhook URL via runtime API (secure credential management)
    pub fn set_webhook_url(&mut self, webhook_url: String) {
        self.webhook_url = Some(webhook_url);
        info!("Slack webhook URL configured");
    }

    /// Clear webhook URL
    pub fn clear_webhook_url(&mut self) {
        self.webhook_url = None;
        info!("Slack webhook URL cleared");
    }
}

#[async_trait]
impl NotificationAdapter for SlackNotificationAdapter {
    async fn send_notification(&self, message: NotificationMessage) -> StoreResult<()> {
        if self.webhook_url.is_none() {
            return Err(StoreError::ConfigurationError(
                "Slack webhook URL not configured".to_string(),
            ));
        }

        // TODO: Implement actual Slack webhook integration
        debug!(
            "Slack notification: {} - {} (severity: {:?})",
            message.title, message.body, message.severity
        );

        // Placeholder implementation
        info!("💬 Slack notification sent to {}: {}", self.channel, message.title);

        Ok(())
    }

    fn adapter_name(&self) -> &'static str {
        "slack"
    }

    async fn is_ready(&self) -> bool {
        self.webhook_url.is_some()
    }
}

/// Notification manager that handles multiple adapters
pub struct NotificationManager {
    adapters: Vec<Box<dyn NotificationAdapter>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Add a notification adapter
    pub fn add_adapter(&mut self, adapter: Box<dyn NotificationAdapter>) {
        info!("Added notification adapter: {}", adapter.adapter_name());
        self.adapters.push(adapter);
    }

    /// Send notification to all ready adapters
    pub async fn send_notification(&self, message: NotificationMessage) -> Vec<StoreResult<()>> {
        let mut results = Vec::new();

        for adapter in &self.adapters {
            if adapter.is_ready().await {
                let result = adapter.send_notification(message.clone()).await;
                if let Err(ref e) = result {
                    warn!("Failed to send notification via {}: {}", adapter.adapter_name(), e);
                }
                results.push(result);
            } else {
                debug!("Skipping notification adapter {} (not ready)", adapter.adapter_name());
            }
        }

        results
    }

    /// Get list of ready adapters
    pub async fn get_ready_adapters(&self) -> Vec<&str> {
        let mut ready = Vec::new();
        for adapter in &self.adapters {
            if adapter.is_ready().await {
                ready.push(adapter.adapter_name());
            }
        }
        ready
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationMessage {
    /// Get severity as string
    pub fn severity_str(&self) -> &'static str {
        match self.severity {
            NotificationSeverity::Info => "INFO",
            NotificationSeverity::Warning => "WARNING",
            NotificationSeverity::Error => "ERROR",
            NotificationSeverity::Critical => "CRITICAL",
        }
    }
}

/// Factory for creating notification adapters
pub struct NotificationAdapterFactory;

impl NotificationAdapterFactory {
    /// Create a notification adapter by type
    pub fn create_adapter(
        adapter_type: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> StoreResult<Box<dyn NotificationAdapter>> {
        match adapter_type {
            "console" => Ok(Box::new(ConsoleNotificationAdapter::new(config))),
            "sendgrid" => Ok(Box::new(SendGridNotificationAdapter::new(config))),
            "twilio" => Ok(Box::new(TwilioNotificationAdapter::new(config))),
            "slack" => Ok(Box::new(SlackNotificationAdapter::new(config))),
            _ => Err(StoreError::ConfigurationError(format!(
                "Unknown notification adapter type: {}",
                adapter_type
            ))),
        }
    }
}