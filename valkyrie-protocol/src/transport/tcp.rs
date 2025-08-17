//! TCP transport implementation

use crate::{Message, Result, ValkyrieError, transport::{Transport, TransportCapabilities}};
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex;

/// TCP transport implementation
pub struct TcpTransport {
    stream: Arc<Mutex<TcpStream>>,
    capabilities: TransportCapabilities,
    connected: bool,
}

impl TcpTransport {
    /// Connect to a TCP endpoint
    pub async fn connect(url: &url::Url) -> Result<Self> {
        let host = url.host_str().ok_or_else(|| ValkyrieError::configuration("Missing host"))?;
        let port = url.port().unwrap_or(8080);
        
        let stream = TcpStream::connect((host, port)).await
            .map_err(|e| ValkyrieError::transport(format!("Failed to connect to {}:{}: {}", host, port, e)))?;
        
        let capabilities = TransportCapabilities {
            max_message_size: 64 * 1024 * 1024, // 64MB
            supports_encryption: false, // TLS would be handled at a higher layer
            supports_compression: false,
            supports_multiplexing: false,
            connection_oriented: true,
        };
        
        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            capabilities,
            connected: true,
        })
    }
    
    /// Create from existing TcpStream
    pub fn from_stream(stream: TcpStream) -> Self {
        let capabilities = TransportCapabilities {
            max_message_size: 64 * 1024 * 1024, // 64MB
            supports_encryption: false,
            supports_compression: false,
            supports_multiplexing: false,
            connection_oriented: true,
        };
        
        Self {
            stream: Arc::new(Mutex::new(stream)),
            capabilities,
            connected: true,
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    fn name(&self) -> &str {
        "tcp"
    }
    
    fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }
    
    async fn send(&self, message: Message) -> Result<()> {
        if !self.connected {
            return Err(ValkyrieError::transport("Transport not connected"));
        }
        
        let data = message.to_bytes()
            .map_err(|e| ValkyrieError::protocol(format!("Serialization failed: {}", e)))?;
        
        let mut stream = self.stream.lock().await;
        
        // Send message length first (4 bytes, big-endian)
        let len = data.len() as u32;
        stream.write_all(&len.to_be_bytes()).await
            .map_err(|e| ValkyrieError::transport(format!("Failed to send message length: {}", e)))?;
        
        // Send message data
        stream.write_all(&data).await
            .map_err(|e| ValkyrieError::transport(format!("Failed to send message data: {}", e)))?;
        
        stream.flush().await
            .map_err(|e| ValkyrieError::transport(format!("Failed to flush stream: {}", e)))?;
        
        Ok(())
    }
    
    async fn recv(&self) -> Result<Message> {
        if !self.connected {
            return Err(ValkyrieError::transport("Transport not connected"));
        }
        
        let mut stream = self.stream.lock().await;
        
        // Read message length (4 bytes, big-endian)
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await
            .map_err(|e| ValkyrieError::transport(format!("Failed to read message length: {}", e)))?;
        
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        if len > self.capabilities.max_message_size {
            return Err(ValkyrieError::transport(format!("Message too large: {} bytes", len)));
        }
        
        // Read message data
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await
            .map_err(|e| ValkyrieError::transport(format!("Failed to read message data: {}", e)))?;
        
        // Deserialize message
        Message::from_bytes(&data)
            .map_err(|e| ValkyrieError::protocol(format!("Deserialization failed: {}", e)))
    }
    
    async fn try_recv(&self) -> Result<Option<Message>> {
        // For TCP, we'll use a timeout to make it non-blocking
        match tokio::time::timeout(std::time::Duration::from_millis(1), self.recv()).await {
            Ok(result) => result.map(Some),
            Err(_) => Ok(None), // Timeout means no message available
        }
    }
    
    async fn close(&self) -> Result<()> {
        // TCP streams are automatically closed when dropped
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
}