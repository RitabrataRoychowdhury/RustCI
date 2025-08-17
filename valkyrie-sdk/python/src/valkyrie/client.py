"""
Valkyrie Protocol Python Client
"""

import asyncio
import time
from typing import Optional, Dict, Any, Union
from dataclasses import dataclass, field
import structlog

from .message import Message, MessageType
from .connection import ConnectionPool, Connection
from .retry import RetryPolicy, RetryableOperation
from .errors import (
    ValkyrieError,
    ConnectionError,
    TimeoutError,
    ConfigurationError,
    SerializationError
)

logger = structlog.get_logger(__name__)


@dataclass
class ClientConfig:
    """Configuration for Valkyrie client."""
    
    # Connection settings
    endpoint: str = "tcp://localhost:8080"
    connect_timeout_ms: int = 5000
    request_timeout_ms: int = 30000
    
    # Pooling settings
    enable_pooling: bool = True
    max_connections: int = 10
    min_connections: int = 1
    
    # Retry settings
    retry_policy: RetryPolicy = field(default_factory=RetryPolicy)
    
    # Security settings
    enable_tls: bool = False
    tls_cert_path: Optional[str] = None
    tls_key_path: Optional[str] = None
    tls_ca_path: Optional[str] = None
    
    # Authentication
    auth_token: Optional[str] = None
    auth_method: str = "none"  # none, jwt, api_key
    
    # Performance settings
    enable_compression: bool = False
    compression_threshold: int = 1024
    
    # Observability
    enable_metrics: bool = False
    enable_tracing: bool = False
    
    def validate(self) -> None:
        """Validate configuration."""
        if not self.endpoint:
            raise ConfigurationError("Endpoint is required")
        
        if self.connect_timeout_ms <= 0:
            raise ConfigurationError("Connect timeout must be positive")
        
        if self.request_timeout_ms <= 0:
            raise ConfigurationError("Request timeout must be positive")
        
        if self.max_connections <= 0:
            raise ConfigurationError("Max connections must be positive")
        
        if self.min_connections < 0:
            raise ConfigurationError("Min connections cannot be negative")
        
        if self.min_connections > self.max_connections:
            raise ConfigurationError("Min connections cannot exceed max connections")
        
        if self.enable_tls and not (self.tls_cert_path and self.tls_key_path):
            raise ConfigurationError("TLS cert and key paths required when TLS is enabled")


class ValkyrieClient:
    """High-level Valkyrie Protocol client."""
    
    def __init__(self, config: ClientConfig):
        """Initialize client with configuration."""
        config.validate()
        self.config = config
        self.pool: Optional[ConnectionPool] = None
        self.connection: Optional[Connection] = None
        self._closed = False
        
        logger.info("Valkyrie client initialized", endpoint=config.endpoint)
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
    
    async def connect(self) -> None:
        """Establish connection(s) to the server."""
        if self._closed:
            raise ValkyrieError("Client is closed")
        
        try:
            if self.config.enable_pooling:
                self.pool = ConnectionPool(
                    endpoint=self.config.endpoint,
                    max_connections=self.config.max_connections,
                    min_connections=self.config.min_connections,
                    connect_timeout_ms=self.config.connect_timeout_ms,
                    config=self.config
                )
                await self.pool.start()
                logger.info("Connection pool started", max_connections=self.config.max_connections)
            else:
                self.connection = Connection(self.config.endpoint, self.config)
                await self.connection.connect()
                logger.info("Single connection established")
                
        except Exception as e:
            logger.error("Failed to connect", error=str(e))
            raise ConnectionError(f"Failed to connect to {self.config.endpoint}: {e}")
    
    async def close(self) -> None:
        """Close all connections."""
        if self._closed:
            return
        
        self._closed = True
        
        try:
            if self.pool:
                await self.pool.close()
                self.pool = None
                logger.info("Connection pool closed")
            
            if self.connection:
                await self.connection.close()
                self.connection = None
                logger.info("Connection closed")
                
        except Exception as e:
            logger.warning("Error during close", error=str(e))
    
    async def request(
        self,
        data: Union[Dict[str, Any], str, bytes],
        endpoint: Optional[str] = None,
        timeout_ms: Optional[int] = None,
        headers: Optional[Dict[str, str]] = None,
        priority: int = 0
    ) -> Dict[str, Any]:
        """Send a request and wait for response."""
        if self._closed:
            raise ValkyrieError("Client is closed")
        
        # Create request message
        message = Message.request(
            payload=data,
            endpoint=endpoint,
            headers=headers or {},
            priority=priority,
            ttl_ms=timeout_ms or self.config.request_timeout_ms
        )
        
        # Execute with retry
        retryable = RetryableOperation(self.config.retry_policy)
        
        async def _send_request():
            if self.pool:
                return await self.pool.request(message, timeout_ms)
            elif self.connection:
                return await self.connection.request(message, timeout_ms)
            else:
                raise ConnectionError("No connection available")
        
        try:
            response = await retryable.execute(_send_request)
            
            # Parse response
            if response.message_type == MessageType.ERROR:
                error_data = response.payload_as_json()
                if error_data:
                    raise ValkyrieError(
                        error_data.get("error", "Unknown error"),
                        code=error_data.get("code")
                    )
                else:
                    raise ValkyrieError("Server returned error response")
            
            # Return parsed JSON or raw string
            json_data = response.payload_as_json()
            if json_data is not None:
                return json_data
            
            string_data = response.payload_as_string()
            if string_data is not None:
                return {"data": string_data}
            
            return {"data": response.payload.hex()}
            
        except Exception as e:
            logger.error("Request failed", error=str(e), endpoint=endpoint)
            raise
    
    async def notify(
        self,
        data: Union[Dict[str, Any], str, bytes],
        endpoint: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
        priority: int = 0
    ) -> None:
        """Send a notification (no response expected)."""
        if self._closed:
            raise ValkyrieError("Client is closed")
        
        # Create notification message
        message = Message.notification(
            payload=data,
            endpoint=endpoint,
            headers=headers or {},
            priority=priority
        )
        
        # Execute with retry
        retryable = RetryableOperation(self.config.retry_policy)
        
        async def _send_notification():
            if self.pool:
                await self.pool.notify(message)
            elif self.connection:
                await self.connection.notify(message)
            else:
                raise ConnectionError("No connection available")
        
        try:
            await retryable.execute(_send_notification)
            logger.debug("Notification sent", endpoint=endpoint)
            
        except Exception as e:
            logger.error("Notification failed", error=str(e), endpoint=endpoint)
            raise
    
    async def ping(self, timeout_ms: Optional[int] = None) -> float:
        """Send a ping request and return round-trip time in milliseconds."""
        start_time = time.time()
        
        try:
            await self.request(
                {"action": "ping", "timestamp": start_time},
                endpoint="/ping",
                timeout_ms=timeout_ms or 5000
            )
            
            end_time = time.time()
            rtt_ms = (end_time - start_time) * 1000
            
            logger.debug("Ping successful", rtt_ms=rtt_ms)
            return rtt_ms
            
        except Exception as e:
            logger.error("Ping failed", error=str(e))
            raise
    
    async def is_connected(self) -> bool:
        """Check if client is connected."""
        if self._closed:
            return False
        
        try:
            if self.pool:
                return await self.pool.is_healthy()
            elif self.connection:
                return await self.connection.is_connected()
            else:
                return False
        except Exception:
            return False
    
    async def get_stats(self) -> Dict[str, Any]:
        """Get client statistics."""
        stats = {
            "connected": await self.is_connected(),
            "endpoint": self.config.endpoint,
            "pooling_enabled": self.config.enable_pooling,
            "closed": self._closed
        }
        
        if self.pool:
            pool_stats = await self.pool.get_stats()
            stats.update(pool_stats)
        elif self.connection:
            connection_stats = await self.connection.get_stats()
            stats.update(connection_stats)
        
        return stats


class ClientBuilder:
    """Builder for creating Valkyrie clients."""
    
    def __init__(self):
        self.config = ClientConfig()
    
    def endpoint(self, endpoint: str) -> "ClientBuilder":
        """Set the server endpoint."""
        self.config.endpoint = endpoint
        return self
    
    def timeouts(self, connect_ms: int, request_ms: int) -> "ClientBuilder":
        """Set connection and request timeouts."""
        self.config.connect_timeout_ms = connect_ms
        self.config.request_timeout_ms = request_ms
        return self
    
    def pooling(self, enabled: bool, max_connections: int = 10, min_connections: int = 1) -> "ClientBuilder":
        """Configure connection pooling."""
        self.config.enable_pooling = enabled
        self.config.max_connections = max_connections
        self.config.min_connections = min_connections
        return self
    
    def retry_policy(self, policy: RetryPolicy) -> "ClientBuilder":
        """Set retry policy."""
        self.config.retry_policy = policy
        return self
    
    def tls(self, cert_path: str, key_path: str, ca_path: Optional[str] = None) -> "ClientBuilder":
        """Enable TLS with certificate paths."""
        self.config.enable_tls = True
        self.config.tls_cert_path = cert_path
        self.config.tls_key_path = key_path
        self.config.tls_ca_path = ca_path
        return self
    
    def auth(self, method: str, token: Optional[str] = None) -> "ClientBuilder":
        """Configure authentication."""
        self.config.auth_method = method
        self.config.auth_token = token
        return self
    
    def compression(self, enabled: bool, threshold: int = 1024) -> "ClientBuilder":
        """Configure compression."""
        self.config.enable_compression = enabled
        self.config.compression_threshold = threshold
        return self
    
    def observability(self, metrics: bool = False, tracing: bool = False) -> "ClientBuilder":
        """Configure observability features."""
        self.config.enable_metrics = metrics
        self.config.enable_tracing = tracing
        return self
    
    def build(self) -> ValkyrieClient:
        """Build the client."""
        return ValkyrieClient(self.config)