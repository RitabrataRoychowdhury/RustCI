//! Python Language Bindings for Valkyrie Protocol
//!
//! This module provides Python bindings using PyO3, enabling Python applications
//! to use the Valkyrie Protocol through a native Python interface.

use pyo3::exceptions::{PyConnectionError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::api::valkyrie::{
    ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
    ClientStats, ValkyrieClient,
};

/// Python wrapper for ValkyrieClient
#[pyclass(name = "ValkyrieClient")]
pub struct PyValkyrieClient {
    client: Arc<ValkyrieClient>,
    runtime: Arc<Runtime>,
}

#[pymethods]
impl PyValkyrieClient {
    /// Create a new Valkyrie client with default configuration
    #[new]
    fn new() -> PyResult<Self> {
        Self::new_with_config(PyValkyrieConfig::default())
    }

    /// Create a new Valkyrie client with custom configuration
    #[staticmethod]
    fn new_with_config(config: PyValkyrieConfig) -> PyResult<Self> {
        let runtime = Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let client = runtime
            .block_on(ValkyrieClient::new(config.to_rust_config()))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create client: {}", e)))?;

        Ok(Self {
            client: Arc::new(client),
            runtime: Arc::new(runtime),
        })
    }

    /// Connect to a remote endpoint
    fn connect(&self, endpoint_url: &str) -> PyResult<String> {
        self.runtime
            .block_on(self.client.connect(endpoint_url))
            .map_err(|e| PyConnectionError::new_err(format!("Connection failed: {}", e)))
    }

    /// Send a text message to a connection
    fn send_text(&self, connection_id: &str, text: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.send_text(connection_id, text))
            .map_err(|e| PyRuntimeError::new_err(format!("Send failed: {}", e)))
    }

    /// Send binary data to a connection
    fn send_binary(&self, connection_id: &str, data: &PyBytes) -> PyResult<()> {
        let data_bytes = data.as_bytes();
        self.runtime
            .block_on(self.client.send_data(connection_id, data_bytes))
            .map_err(|e| PyRuntimeError::new_err(format!("Send failed: {}", e)))
    }

    /// Send a custom message to a connection
    fn send_message(&self, connection_id: &str, message: PyValkyrieMessage) -> PyResult<()> {
        let rust_message = message.to_rust_message()?;
        self.runtime
            .block_on(self.client.send_message(connection_id, rust_message))
            .map_err(|e| PyRuntimeError::new_err(format!("Send failed: {}", e)))
    }

    /// Broadcast a message to multiple connections
    fn broadcast(
        &self,
        connection_ids: Vec<String>,
        message: PyValkyrieMessage,
    ) -> PyResult<PyBroadcastResult> {
        let rust_message = message.to_rust_message()?;
        let result = self
            .runtime
            .block_on(self.client.broadcast(&connection_ids, rust_message))
            .map_err(|e| PyRuntimeError::new_err(format!("Broadcast failed: {}", e)))?;

        Ok(PyBroadcastResult::from_rust_result(result))
    }

    /// Get client statistics
    fn get_stats(&self) -> PyResult<PyValkyrieStats> {
        let stats = self.runtime.block_on(self.client.get_stats());
        Ok(PyValkyrieStats::from_rust_stats(stats))
    }

    /// Close a specific connection
    fn close_connection(&self, connection_id: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.close_connection(connection_id))
            .map_err(|e| PyRuntimeError::new_err(format!("Close failed: {}", e)))
    }

    /// Shutdown the client
    fn shutdown(&mut self) -> PyResult<()> {
        // Note: This is a simplified shutdown for Python bindings
        // In a real implementation, we'd need to handle the mutable reference properly
        Ok(())
    }

    /// Create a simple client with default settings
    #[staticmethod]
    fn simple() -> PyResult<Self> {
        Self::new()
    }

    /// Create a secure client with mTLS authentication
    #[staticmethod]
    fn secure(cert_path: &str, key_path: &str, ca_path: &str) -> PyResult<Self> {
        let config = PyValkyrieConfig {
            enable_encryption: true,
            enable_mtls: true,
            cert_path: Some(cert_path.to_string()),
            key_path: Some(key_path.to_string()),
            ca_path: Some(ca_path.to_string()),
            ..Default::default()
        };
        Self::new_with_config(config)
    }

    /// Create a high-performance client
    #[staticmethod]
    fn high_performance() -> PyResult<Self> {
        let config = PyValkyrieConfig {
            max_connections: 100,
            enable_zero_copy: true,
            enable_simd: true,
            worker_threads: Some(num_cpus::get()),
            ..Default::default()
        };
        Self::new_with_config(config)
    }
}

/// Python wrapper for client configuration
#[pyclass(name = "ValkyrieConfig")]
#[derive(Clone)]
pub struct PyValkyrieConfig {
    #[pyo3(get, set)]
    pub enable_encryption: bool,
    #[pyo3(get, set)]
    pub enable_mtls: bool,
    #[pyo3(get, set)]
    pub enable_metrics: bool,
    #[pyo3(get, set)]
    pub enable_tracing: bool,
    #[pyo3(get, set)]
    pub connect_timeout: u64,
    #[pyo3(get, set)]
    pub send_timeout: u64,
    #[pyo3(get, set)]
    pub max_connections: u32,
    #[pyo3(get, set)]
    pub worker_threads: Option<usize>,
    #[pyo3(get, set)]
    pub enable_zero_copy: bool,
    #[pyo3(get, set)]
    pub enable_simd: bool,
    #[pyo3(get, set)]
    pub cert_path: Option<String>,
    #[pyo3(get, set)]
    pub key_path: Option<String>,
    #[pyo3(get, set)]
    pub ca_path: Option<String>,
}

impl Default for PyValkyrieConfig {
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

#[pymethods]
impl PyValkyrieConfig {
    #[new]
    fn new() -> Self {
        Self::default()
    }
}

impl PyValkyrieConfig {
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
                worker_threads: self.worker_threads,
                max_connections_per_endpoint: self.max_connections,
                ..Default::default()
            },
            timeouts: ClientTimeoutConfig {
                connect_timeout: Duration::from_secs(self.connect_timeout),
                send_timeout: Duration::from_secs(self.send_timeout),
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

/// Python wrapper for Valkyrie messages
#[pyclass(name = "ValkyrieMessage")]
#[derive(Clone)]
pub struct PyValkyrieMessage {
    #[pyo3(get, set)]
    pub message_type: String,
    #[pyo3(get, set)]
    pub priority: String,
    #[pyo3(get, set)]
    pub data: Option<Vec<u8>>,
    #[pyo3(get, set)]
    pub text: Option<String>,
    #[pyo3(get, set)]
    pub correlation_id: Option<String>,
    #[pyo3(get, set)]
    pub ttl_seconds: Option<u64>,
    #[pyo3(get, set)]
    pub metadata: Option<HashMap<String, String>>,
}

#[pymethods]
impl PyValkyrieMessage {
    #[new]
    fn new() -> Self {
        Self {
            message_type: "text".to_string(),
            priority: "normal".to_string(),
            data: None,
            text: None,
            correlation_id: None,
            ttl_seconds: None,
            metadata: None,
        }
    }

    /// Create a text message
    #[staticmethod]
    fn text(content: &str) -> Self {
        Self {
            message_type: "text".to_string(),
            priority: "normal".to_string(),
            data: None,
            text: Some(content.to_string()),
            correlation_id: None,
            ttl_seconds: None,
            metadata: None,
        }
    }

    /// Create a binary message
    #[staticmethod]
    fn binary(data: Vec<u8>) -> Self {
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
    #[staticmethod]
    fn json(data: &PyDict) -> PyResult<Self> {
        let json_str = serde_json::to_string(&data)
            .map_err(|e| PyValueError::new_err(format!("JSON serialization failed: {}", e)))?;

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
    fn with_priority(mut slf: PyRefMut<Self>, priority: &str) -> PyRefMut<Self> {
        slf.priority = priority.to_string();
        slf
    }

    /// Set correlation ID
    fn with_correlation_id(mut slf: PyRefMut<Self>, correlation_id: &str) -> PyRefMut<Self> {
        slf.correlation_id = Some(correlation_id.to_string());
        slf
    }

    /// Set TTL in seconds
    fn with_ttl(mut slf: PyRefMut<Self>, ttl_seconds: u64) -> PyRefMut<Self> {
        slf.ttl_seconds = Some(ttl_seconds);
        slf
    }
}

impl PyValkyrieMessage {
    fn to_rust_message(&self) -> PyResult<ClientMessage> {
        let message_type = match self.message_type.as_str() {
            "text" => ClientMessageType::Text,
            "binary" => ClientMessageType::Binary,
            "json" => ClientMessageType::Json,
            "control" => ClientMessageType::Control,
            _ => return Err(PyValueError::new_err("Invalid message type")),
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
            ClientPayload::Binary(data.clone())
        } else {
            ClientPayload::Text(String::new())
        };

        let ttl = self.ttl_seconds.map(Duration::from_secs);
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

/// Python wrapper for broadcast results
#[pyclass(name = "BroadcastResult")]
pub struct PyBroadcastResult {
    #[pyo3(get)]
    pub total: u32,
    #[pyo3(get)]
    pub successful: u32,
    #[pyo3(get)]
    pub failed: u32,
}

impl PyBroadcastResult {
    fn from_rust_result(result: crate::core::networking::valkyrie::BroadcastResult) -> Self {
        Self {
            total: result.total,
            successful: result.successful,
            failed: result.failed,
        }
    }
}

/// Python wrapper for client statistics
#[pyclass(name = "ValkyrieStats")]
pub struct PyValkyrieStats {
    #[pyo3(get)]
    pub active_connections: usize,
    #[pyo3(get)]
    pub handlers_registered: usize,
    #[pyo3(get)]
    pub messages_sent: u64,
    #[pyo3(get)]
    pub messages_received: u64,
    #[pyo3(get)]
    pub bytes_sent: u64,
    #[pyo3(get)]
    pub bytes_received: u64,
}

impl PyValkyrieStats {
    fn from_rust_stats(stats: ClientStats) -> Self {
        Self {
            active_connections: stats.active_connections,
            handlers_registered: stats.handlers_registered,
            messages_sent: stats.engine_stats.transport.messages_sent,
            messages_received: stats.engine_stats.transport.messages_received,
            bytes_sent: stats.engine_stats.transport.bytes_sent,
            bytes_received: stats.engine_stats.transport.bytes_received,
        }
    }
}

/// Python module initialization
#[pymodule]
fn valkyrie_protocol(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyValkyrieClient>()?;
    m.add_class::<PyValkyrieConfig>()?;
    m.add_class::<PyValkyrieMessage>()?;
    m.add_class::<PyBroadcastResult>()?;
    m.add_class::<PyValkyrieStats>()?;

    // Add module-level convenience functions
    m.add_function(wrap_pyfunction!(create_client, m)?)?;
    m.add_function(wrap_pyfunction!(create_secure_client, m)?)?;
    m.add_function(wrap_pyfunction!(create_high_performance_client, m)?)?;

    Ok(())
}

/// Create a simple client (convenience function)
#[pyfunction]
fn create_client() -> PyResult<PyValkyrieClient> {
    PyValkyrieClient::simple()
}

/// Create a secure client (convenience function)
#[pyfunction]
fn create_secure_client(
    cert_path: &str,
    key_path: &str,
    ca_path: &str,
) -> PyResult<PyValkyrieClient> {
    PyValkyrieClient::secure(cert_path, key_path, ca_path)
}

/// Create a high-performance client (convenience function)
#[pyfunction]
fn create_high_performance_client() -> PyResult<PyValkyrieClient> {
    PyValkyrieClient::high_performance()
}

/// Generate Python stub file content for type hints
pub fn generate_python_stubs() -> String {
    r#"
"""
Valkyrie Protocol Python Bindings

This module provides Python bindings for the Valkyrie Protocol,
enabling high-performance distributed communication in Python applications.
"""

from typing import Optional, Dict, List, Union
import asyncio

class ValkyrieClient:
    """High-level Valkyrie Protocol client for Python applications."""
    
    def __init__(self) -> None:
        """Create a new Valkyrie client with default configuration."""
        ...
    
    @staticmethod
    def new_with_config(config: 'ValkyrieConfig') -> 'ValkyrieClient':
        """Create a new Valkyrie client with custom configuration."""
        ...
    
    def connect(self, endpoint_url: str) -> str:
        """Connect to a remote endpoint and return connection ID."""
        ...
    
    def send_text(self, connection_id: str, text: str) -> None:
        """Send a text message to a connection."""
        ...
    
    def send_binary(self, connection_id: str, data: bytes) -> None:
        """Send binary data to a connection."""
        ...
    
    def send_message(self, connection_id: str, message: 'ValkyrieMessage') -> None:
        """Send a custom message to a connection."""
        ...
    
    def broadcast(self, connection_ids: List[str], message: 'ValkyrieMessage') -> 'BroadcastResult':
        """Broadcast a message to multiple connections."""
        ...
    
    def get_stats(self) -> 'ValkyrieStats':
        """Get client statistics."""
        ...
    
    def close_connection(self, connection_id: str) -> None:
        """Close a specific connection."""
        ...
    
    def shutdown(self) -> None:
        """Shutdown the client."""
        ...
    
    @staticmethod
    def simple() -> 'ValkyrieClient':
        """Create a simple client with default settings."""
        ...
    
    @staticmethod
    def secure(cert_path: str, key_path: str, ca_path: str) -> 'ValkyrieClient':
        """Create a secure client with mTLS authentication."""
        ...
    
    @staticmethod
    def high_performance() -> 'ValkyrieClient':
        """Create a high-performance client."""
        ...

class ValkyrieConfig:
    """Configuration for Valkyrie Protocol client."""
    
    enable_encryption: bool
    enable_mtls: bool
    enable_metrics: bool
    enable_tracing: bool
    connect_timeout: int
    send_timeout: int
    max_connections: int
    worker_threads: Optional[int]
    enable_zero_copy: bool
    enable_simd: bool
    cert_path: Optional[str]
    key_path: Optional[str]
    ca_path: Optional[str]
    
    def __init__(self) -> None:
        """Create default configuration."""
        ...

class ValkyrieMessage:
    """Message structure for Valkyrie Protocol."""
    
    message_type: str
    priority: str
    data: Optional[bytes]
    text: Optional[str]
    correlation_id: Optional[str]
    ttl_seconds: Optional[int]
    metadata: Optional[Dict[str, str]]
    
    def __init__(self) -> None:
        """Create a new message."""
        ...
    
    @staticmethod
    def text(content: str) -> 'ValkyrieMessage':
        """Create a text message."""
        ...
    
    @staticmethod
    def binary(data: bytes) -> 'ValkyrieMessage':
        """Create a binary message."""
        ...
    
    @staticmethod
    def json(data: Dict) -> 'ValkyrieMessage':
        """Create a JSON message."""
        ...
    
    def with_priority(self, priority: str) -> 'ValkyrieMessage':
        """Set message priority."""
        ...
    
    def with_correlation_id(self, correlation_id: str) -> 'ValkyrieMessage':
        """Set correlation ID."""
        ...
    
    def with_ttl(self, ttl_seconds: int) -> 'ValkyrieMessage':
        """Set TTL in seconds."""
        ...

class BroadcastResult:
    """Result of a broadcast operation."""
    
    total: int
    successful: int
    failed: int

class ValkyrieStats:
    """Client statistics."""
    
    active_connections: int
    handlers_registered: int
    messages_sent: int
    messages_received: int
    bytes_sent: int
    bytes_received: int

# Convenience functions
def create_client() -> ValkyrieClient:
    """Create a simple client."""
    ...

def create_secure_client(cert_path: str, key_path: str, ca_path: str) -> ValkyrieClient:
    """Create a secure client."""
    ...

def create_high_performance_client() -> ValkyrieClient:
    """Create a high-performance client."""
    ...
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_config_conversion() {
        let py_config = PyValkyrieConfig {
            enable_encryption: true,
            enable_mtls: true,
            max_connections: 50,
            ..Default::default()
        };

        let rust_config = py_config.to_rust_config();
        assert!(rust_config.security.enable_encryption);
        assert_eq!(rust_config.performance.max_connections_per_endpoint, 50);
    }

    #[test]
    fn test_python_message_creation() {
        let message = PyValkyrieMessage::text("Hello, Python!");
        assert_eq!(message.message_type, "text");
        assert_eq!(message.text, Some("Hello, Python!".to_string()));
    }
}
