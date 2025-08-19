//! JavaScript/Node.js Language Bindings for Valkyrie Protocol
//!
//! This module provides JavaScript bindings using napi-rs, enabling Node.js applications
//! to use the Valkyrie Protocol through a native JavaScript interface.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::api::valkyrie::{
    ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
    ClientStats, ValkyrieClient,
};

/// JavaScript wrapper for ValkyrieClient
#[napi]
pub struct JsValkyrieClient {
    client: Arc<ValkyrieClient>,
    runtime: Arc<Runtime>,
}

#[napi]
impl JsValkyrieClient {
    /// Create a new Valkyrie client with default configuration
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        Self::new_with_config(JsValkyrieConfig::default())
    }

    /// Create a new Valkyrie client with custom configuration
    #[napi(factory)]
    pub fn new_with_config(config: JsValkyrieConfig) -> Result<Self> {
        let runtime = Runtime::new().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Failed to create runtime: {}", e),
            )
        })?;

        let client = runtime
            .block_on(ValkyrieClient::new(config.to_rust_config()))
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to create client: {}", e),
                )
            })?;

        Ok(Self {
            client: Arc::new(client),
            runtime: Arc::new(runtime),
        })
    }

    /// Connect to a remote endpoint
    #[napi]
    pub fn connect(&self, endpoint_url: String) -> Result<String> {
        self.runtime
            .block_on(self.client.connect(&endpoint_url))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Connection failed: {}", e)))
    }

    /// Send a text message to a connection
    #[napi]
    pub fn send_text(&self, connection_id: String, text: String) -> Result<()> {
        self.runtime
            .block_on(self.client.send_text(&connection_id, &text))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Send failed: {}", e)))
    }

    /// Send binary data to a connection
    #[napi]
    pub fn send_binary(&self, connection_id: String, data: Buffer) -> Result<()> {
        let data_bytes = data.as_ref();
        self.runtime
            .block_on(self.client.send_data(&connection_id, data_bytes))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Send failed: {}", e)))
    }

    /// Send a custom message to a connection
    #[napi]
    pub fn send_message(&self, connection_id: String, message: JsValkyrieMessage) -> Result<()> {
        let rust_message = message.to_rust_message()?;
        self.runtime
            .block_on(self.client.send_message(&connection_id, rust_message))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Send failed: {}", e)))
    }

    /// Broadcast a message to multiple connections
    #[napi]
    pub fn broadcast(
        &self,
        connection_ids: Vec<String>,
        message: JsValkyrieMessage,
    ) -> Result<JsBroadcastResult> {
        let rust_message = message.to_rust_message()?;
        let result = self
            .runtime
            .block_on(self.client.broadcast(&connection_ids, rust_message))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Broadcast failed: {}", e)))?;

        Ok(JsBroadcastResult::from_rust_result(result))
    }

    /// Get client statistics
    #[napi]
    pub fn get_stats(&self) -> JsValkyrieStats {
        let stats = self.runtime.block_on(self.client.get_stats());
        JsValkyrieStats::from_rust_stats(stats)
    }

    /// Close a specific connection
    #[napi]
    pub fn close_connection(&self, connection_id: String) -> Result<()> {
        self.runtime
            .block_on(self.client.close_connection(&connection_id))
            .map_err(|e| Error::new(Status::GenericFailure, format!("Close failed: {}", e)))
    }

    /// Create a simple client with default settings
    #[napi(factory)]
    pub fn simple() -> Result<Self> {
        Self::new()
    }

    /// Create a secure client with mTLS authentication
    #[napi(factory)]
    pub fn secure(cert_path: String, key_path: String, ca_path: String) -> Result<Self> {
        let config = JsValkyrieConfig {
            enable_encryption: true,
            enable_mtls: true,
            cert_path: Some(cert_path),
            key_path: Some(key_path),
            ca_path: Some(ca_path),
            ..Default::default()
        };
        Self::new_with_config(config)
    }

    /// Create a high-performance client
    #[napi(factory)]
    pub fn high_performance() -> Result<Self> {
        let config = JsValkyrieConfig {
            max_connections: 100,
            enable_zero_copy: true,
            enable_simd: true,
            worker_threads: Some(num_cpus::get()),
            ..Default::default()
        };
        Self::new_with_config(config)
    }
}

/// JavaScript wrapper for client configuration
#[napi(object)]
#[derive(Clone)]
pub struct JsValkyrieConfig {
    pub enable_encryption: bool,
    pub enable_mtls: bool,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub connect_timeout: u32,
    pub send_timeout: u32,
    pub max_connections: u32,
    pub worker_threads: Option<u32>,
    pub enable_zero_copy: bool,
    pub enable_simd: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub ca_path: Option<String>,
}

impl Default for JsValkyrieConfig {
    fn default() -> Self {
        Self {
            enable_encryption: true,
            enable_mtls: false,
            enable_metrics: true,
            enable_tracing: false,
            connect_timeout: 10,
            send_timeout: 5,
            max_connections: 10,
            worker_threads: None,
            enable_zero_copy: true,
            enable_simd: true,
            cert_path: None,
            key_path: None,
            ca_path: None,
        }
    }
}

impl JsValkyrieConfig {
    fn to_rust_config(&self) -> ClientConfig {
        use crate::api::valkyrie::*;

        ClientConfig {
            security: ClientSecurityConfig {
                auth_method: if self.enable_mtls {
                    ClientAuthMethod::MutualTls
                } else {
                    ClientAuthMethod::None
                },
                enable_encryption: self.enable_encryption,
                enable_audit: false,
                cert_path: self.cert_path.clone(),
                key_path: self.key_path.clone(),
                ca_path: self.ca_path.clone(),
            },
            performance: ClientPerformanceConfig {
                worker_threads: self.worker_threads.map(|t| t as usize),
                max_connections_per_endpoint: self.max_connections,
                ..Default::default()
            },
            timeouts: ClientTimeoutConfig {
                connect_timeout: Duration::from_secs(self.connect_timeout as u64),
                send_timeout: Duration::from_secs(self.send_timeout as u64),
                ..Default::default()
            },
            features: ClientFeatureFlags {
                enable_metrics: self.enable_metrics,
                enable_tracing: self.enable_tracing,
                enable_zero_copy: self.enable_zero_copy,
                enable_simd: self.enable_simd,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// JavaScript wrapper for Valkyrie messages
#[napi(object)]
#[derive(Clone)]
pub struct JsValkyrieMessage {
    pub message_type: String,
    pub priority: String,
    pub data: Option<Buffer>,
    pub text: Option<String>,
    pub correlation_id: Option<String>,
    pub ttl_seconds: Option<u32>,
    pub metadata: Option<HashMap<String, String>>,
}

#[napi]
impl JsValkyrieMessage {
    /// Create a text message
    #[napi(factory)]
    pub fn text(content: String) -> Self {
        Self {
            message_type: "text".to_string(),
            priority: "normal".to_string(),
            data: None,
            text: Some(content),
            correlation_id: None,
            ttl_seconds: None,
            metadata: None,
        }
    }

    /// Create a binary message
    #[napi(factory)]
    pub fn binary(data: Buffer) -> Self {
        Self {
            message_type: "binary".to_string(),
            priority: "normal".to_string(),
            data: Some(data),
            text: None,
            correlation_id: None,
            ttl_seconds: None,
            metadata: None,
        }
    }

    /// Create a JSON message
    #[napi(factory)]
    pub fn json(data: serde_json::Value) -> Result<Self> {
        let json_str = serde_json::to_string(&data).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("JSON serialization failed: {}", e),
            )
        })?;

        Ok(Self {
            message_type: "json".to_string(),
            priority: "normal".to_string(),
            data: None,
            text: Some(json_str),
            correlation_id: None,
            ttl_seconds: None,
            metadata: None,
        })
    }

    /// Set message priority
    #[napi]
    pub fn with_priority(&mut self, priority: String) -> &mut Self {
        self.priority = priority;
        self
    }

    /// Set correlation ID
    #[napi]
    pub fn with_correlation_id(&mut self, correlation_id: String) -> &mut Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Set TTL in seconds
    #[napi]
    pub fn with_ttl(&mut self, ttl_seconds: u32) -> &mut Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }
}

impl JsValkyrieMessage {
    fn to_rust_message(&self) -> Result<ClientMessage> {
        let message_type = match self.message_type.as_str() {
            "text" => ClientMessageType::Text,
            "binary" => ClientMessageType::Binary,
            "json" => ClientMessageType::Json,
            "control" => ClientMessageType::Control,
            _ => return Err(Error::new(Status::InvalidArg, "Invalid message type")),
        };

        let priority = match self.priority.as_str() {
            "low" => ClientMessagePriority::Low,
            "normal" => ClientMessagePriority::Normal,
            "high" => ClientMessagePriority::High,
            "critical" => ClientMessagePriority::Critical,
            _ => ClientMessagePriority::Normal,
        };

        let payload = if let Some(ref text) = self.text {
            ClientPayload::Text(text.clone())
        } else if let Some(ref data) = self.data {
            ClientPayload::Binary(data.as_ref().to_vec())
        } else {
            ClientPayload::Text(String::new())
        };

        let ttl = self.ttl_seconds.map(|t| Duration::from_secs(t as u64));
        let metadata = self.metadata.clone().unwrap_or_default();

        Ok(ClientMessage {
            message_type,
            priority,
            payload,
            correlation_id: self.correlation_id.clone(),
            ttl,
            metadata,
        })
    }
}

/// JavaScript wrapper for broadcast results
#[napi(object)]
pub struct JsBroadcastResult {
    pub total: u32,
    pub successful: u32,
    pub failed: u32,
}

impl JsBroadcastResult {
    fn from_rust_result(result: crate::core::networking::valkyrie::BroadcastResult) -> Self {
        Self {
            total: result.total,
            successful: result.successful,
            failed: result.failed,
        }
    }
}

/// JavaScript wrapper for client statistics
#[napi(object)]
pub struct JsValkyrieStats {
    pub active_connections: u32,
    pub handlers_registered: u32,
    pub messages_sent: u32,
    pub messages_received: u32,
    pub bytes_sent: u32,
    pub bytes_received: u32,
}

impl JsValkyrieStats {
    fn from_rust_stats(stats: ClientStats) -> Self {
        Self {
            active_connections: stats.active_connections as u32,
            handlers_registered: stats.handlers_registered as u32,
            messages_sent: stats.engine_stats.transport.messages_sent as u32,
            messages_received: stats.engine_stats.transport.messages_received as u32,
            bytes_sent: stats.engine_stats.transport.bytes_sent as u32,
            bytes_received: stats.engine_stats.transport.bytes_received as u32,
        }
    }
}

/// Generate TypeScript definition file content
pub fn generate_typescript_definitions() -> String {
    r#"
/**
 * Valkyrie Protocol Node.js Bindings
 * 
 * High-performance distributed communication protocol for Node.js applications.
 */

export interface ValkyrieConfig {
  enableEncryption: boolean;
  enableMtls: boolean;
  enableMetrics: boolean;
  enableTracing: boolean;
  connectTimeout: number;
  sendTimeout: number;
  maxConnections: number;
  workerThreads?: number;
  enableZeroCopy: boolean;
  enableSimd: boolean;
  certPath?: string;
  keyPath?: string;
  caPath?: string;
}

export interface ValkyrieMessage {
  messageType: string;
  priority: string;
  data?: Buffer;
  text?: string;
  correlationId?: string;
  ttlSeconds?: number;
  metadata?: Record<string, string>;
}

export interface BroadcastResult {
  total: number;
  successful: number;
  failed: number;
}

export interface ValkyrieStats {
  activeConnections: number;
  handlersRegistered: number;
  messagesSent: number;
  messagesReceived: number;
  bytesSent: number;
  bytesReceived: number;
}

export declare class ValkyrieClient {
  constructor();
  
  static newWithConfig(config: ValkyrieConfig): ValkyrieClient;
  static simple(): ValkyrieClient;
  static secure(certPath: string, keyPath: string, caPath: string): ValkyrieClient;
  static highPerformance(): ValkyrieClient;
  
  connect(endpointUrl: string): string;
  sendText(connectionId: string, text: string): void;
  sendBinary(connectionId: string, data: Buffer): void;
  sendMessage(connectionId: string, message: ValkyrieMessage): void;
  broadcast(connectionIds: string[], message: ValkyrieMessage): BroadcastResult;
  getStats(): ValkyrieStats;
  closeConnection(connectionId: string): void;
}

export declare class ValkyrieMessageBuilder {
  static text(content: string): ValkyrieMessage;
  static binary(data: Buffer): ValkyrieMessage;
  static json(data: any): ValkyrieMessage;
  
  withPriority(priority: string): ValkyrieMessage;
  withCorrelationId(correlationId: string): ValkyrieMessage;
  withTtl(ttlSeconds: number): ValkyrieMessage;
}

// Convenience functions
export function createClient(): ValkyrieClient;
export function createSecureClient(certPath: string, keyPath: string, caPath: string): ValkyrieClient;
export function createHighPerformanceClient(): ValkyrieClient;

// Message priorities
export const MessagePriority = {
  LOW: 'low',
  NORMAL: 'normal',
  HIGH: 'high',
  CRITICAL: 'critical'
} as const;

// Message types
export const MessageType = {
  TEXT: 'text',
  BINARY: 'binary',
  JSON: 'json',
  CONTROL: 'control'
} as const;
"#.to_string()
}

/// Generate package.json content for npm package
pub fn generate_package_json() -> String {
    r#"
{
  "name": "@valkyrie/protocol",
  "version": "1.0.0",
  "description": "High-performance distributed communication protocol for Node.js",
  "main": "index.js",
  "types": "index.d.ts",
  "napi": {
    "name": "valkyrie-protocol",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "i686-pc-windows-msvc",
        "armv7-unknown-linux-gnueabihf",
        "aarch64-apple-darwin",
        "aarch64-pc-windows-msvc",
        "aarch64-unknown-linux-musl",
        "x86_64-unknown-freebsd"
      ]
    }
  },
  "license": "MIT",
  "devDependencies": {
    "@napi-rs/cli": "^2.16.0"
  },
  "engines": {
    "node": ">= 10"
  },
  "scripts": {
    "artifacts": "napi artifacts",
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform",
    "prepublishOnly": "napi prepublish -t npm",
    "test": "node test.js",
    "universal": "napi universal",
    "version": "napi version"
  },
  "keywords": [
    "distributed",
    "communication",
    "protocol",
    "high-performance",
    "rust",
    "native"
  ],
  "repository": {
    "type": "git",
    "url": "https://github.com/rustci/valkyrie-protocol.git"
  },
  "bugs": {
    "url": "https://github.com/rustci/valkyrie-protocol/issues"
  },
  "homepage": "https://github.com/rustci/valkyrie-protocol#readme"
}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_config_conversion() {
        let js_config = JsValkyrieConfig {
            enable_encryption: true,
            enable_mtls: true,
            max_connections: 50,
            ..Default::default()
        };

        let rust_config = js_config.to_rust_config();
        assert!(rust_config.security.enable_encryption);
        assert_eq!(rust_config.performance.max_connections_per_endpoint, 50);
    }

    #[test]
    fn test_js_message_creation() {
        let message = JsValkyrieMessage::text("Hello, JavaScript!".to_string());
        assert_eq!(message.message_type, "text");
        assert_eq!(message.text, Some("Hello, JavaScript!".to_string()));
    }
}
