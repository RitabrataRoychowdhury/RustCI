"""
Connection management for Valkyrie Protocol
"""

import asyncio
import time
import weakref
from typing import Optional, Dict, Any, List
from dataclasses import dataclass
import structlog

from .message import Message, MessageType
from .errors import ValkyrieError, ConnectionError, TimeoutError
from .retry import RetryPolicy, RetryableOperation

logger = structlog.get_logger(__name__)


@dataclass
class ConnectionConfig:
    """Configuration for individual connections."""
    
    endpoint: str
    connect_timeout_ms: int = 5000
    request_timeout_ms: int = 30000
    max_concurrent_requests: int = 100
    auto_reconnect: bool = True
    reconnect_interval_ms: int = 1000
    keepalive_interval_ms: int = 30000


class Connection:
    """Individual connection to Valkyrie server."""
    
    def __init__(self, endpoint: str, config: 'ClientConfig'):
        self.endpoint = endpoint
        self.config = config
        self._connected = False
        self._connecting = False
        self._reader: Optional[asyncio.StreamReader] = None
        self._writer: Optional[asyncio.StreamWriter] = None
        self._pending_requests: Dict[str, asyncio.Future] = {}
        self._stats = {
            'requests_sent': 0,
            'responses_received': 0,
            'errors': 0,
            'connected_at': None,
            'last_activity': None,
        }
        
        logger.info("Connection created", endpoint=endpoint)
    
    async def connect(self) -> None:
        """Establish connection to the server."""
        if self._connected or self._connecting:
            return
        
        self._connecting = True
        
        try:
            # Parse endpoint
            if self.endpoint.startswith('tcp://'):
                host_port = self.endpoint[6:]
                if ':' in host_port:
                    host, port_str = host_port.rsplit(':', 1)
                    port = int(port_str)
                else:
                    host = host_port
                    port = 8080
                
                # Connect with timeout
                connect_timeout = self.config.connect_timeout_ms / 1000.0
                self._reader, self._writer = await asyncio.wait_for(
                    asyncio.open_connection(host, port),
                    timeout=connect_timeout
                )
                
                self._connected = True
                self._stats['connected_at'] = time.time()
                self._stats['last_activity'] = time.time()
                
                # Start message handler
                asyncio.create_task(self._handle_messages())
                
                logger.info("Connection established", endpoint=self.endpoint)
                
            else:
                raise ConnectionError(f"Unsupported endpoint format: {self.endpoint}")
                
        except asyncio.TimeoutError:
            raise ConnectionError(f"Connection timeout to {self.endpoint}")
        except Exception as e:
            raise ConnectionError(f"Failed to connect to {self.endpoint}: {e}")
        finally:
            self._connecting = False
    
    async def close(self) -> None:
        """Close the connection."""
        if not self._connected:
            return
        
        self._connected = False
        
        # Cancel pending requests
        for future in self._pending_requests.values():
            if not future.done():
                future.cancel()
        self._pending_requests.clear()
        
        # Close writer
        if self._writer:
            self._writer.close()
            try:
                await self._writer.wait_closed()
            except Exception:
                pass
        
        self._reader = None
        self._writer = None
        
        logger.info("Connection closed", endpoint=self.endpoint)
    
    async def request(self, message: Message, timeout_ms: Optional[int] = None) -> Message:
        """Send a request and wait for response."""
        if not self._connected:
            await self.connect()
        
        if message.message_type != MessageType.REQUEST:
            raise ValkyrieError("Message must be a request")
        
        # Set up response future
        response_future = asyncio.Future()
        self._pending_requests[message.id] = response_future
        
        try:
            # Send message
            await self._send_message(message)
            self._stats['requests_sent'] += 1
            
            # Wait for response
            timeout = (timeout_ms or self.config.request_timeout_ms) / 1000.0
            response = await asyncio.wait_for(response_future, timeout=timeout)
            
            self._stats['responses_received'] += 1
            return response
            
        except asyncio.TimeoutError:
            self._stats['errors'] += 1
            raise TimeoutError(f"Request timeout after {timeout_ms}ms")
        except Exception as e:
            self._stats['errors'] += 1
            raise
        finally:
            self._pending_requests.pop(message.id, None)
    
    async def notify(self, message: Message) -> None:
        """Send a notification (no response expected)."""
        if not self._connected:
            await self.connect()
        
        if message.message_type != MessageType.NOTIFICATION:
            raise ValkyrieError("Message must be a notification")
        
        await self._send_message(message)
        self._stats['requests_sent'] += 1
    
    async def _send_message(self, message: Message) -> None:
        """Send a message over the connection."""
        if not self._writer:
            raise ConnectionError("Connection not established")
        
        # Serialize message (simplified - in real implementation would use proper protocol)
        message_data = message.to_dict()
        message_json = orjson.dumps(message_data)
        
        # Send length prefix + message
        length = len(message_json)
        length_bytes = length.to_bytes(4, byteorder='big')
        
        self._writer.write(length_bytes + message_json)
        await self._writer.drain()
        
        self._stats['last_activity'] = time.time()
    
    async def _handle_messages(self) -> None:
        """Handle incoming messages."""
        try:
            while self._connected and self._reader:
                # Read message length
                length_bytes = await self._reader.readexactly(4)
                length = int.from_bytes(length_bytes, byteorder='big')
                
                # Read message data
                message_data = await self._reader.readexactly(length)
                message_dict = orjson.loads(message_data)
                
                # Parse message
                message = Message.from_dict(message_dict)
                
                # Handle response
                if message.message_type == MessageType.RESPONSE and message.correlation_id:
                    future = self._pending_requests.get(message.correlation_id)
                    if future and not future.done():
                        future.set_result(message)
                
                self._stats['last_activity'] = time.time()
                
        except asyncio.CancelledError:
            pass
        except Exception as e:
            logger.error("Message handler error", error=str(e))
            # Reconnect logic would go here
    
    async def is_connected(self) -> bool:
        """Check if connection is active."""
        return self._connected and self._writer and not self._writer.is_closing()
    
    async def get_stats(self) -> Dict[str, Any]:
        """Get connection statistics."""
        return {
            'endpoint': self.endpoint,
            'connected': await self.is_connected(),
            'requests_sent': self._stats['requests_sent'],
            'responses_received': self._stats['responses_received'],
            'errors': self._stats['errors'],
            'connected_at': self._stats['connected_at'],
            'last_activity': self._stats['last_activity'],
        }


class ConnectionPool:
    """Pool of connections for load balancing and failover."""
    
    def __init__(
        self,
        endpoint: str,
        max_connections: int,
        min_connections: int = 1,
        connect_timeout_ms: int = 5000,
        config: Optional['ClientConfig'] = None
    ):
        self.endpoint = endpoint
        self.max_connections = max_connections
        self.min_connections = min_connections
        self.connect_timeout_ms = connect_timeout_ms
        self.config = config
        
        self._connections: List[Connection] = []
        self._connection_index = 0
        self._lock = asyncio.Lock()
        self._started = False
        
        logger.info("Connection pool created", 
                   endpoint=endpoint, 
                   max_connections=max_connections)
    
    async def start(self) -> None:
        """Start the connection pool."""
        if self._started:
            return
        
        async with self._lock:
            if self._started:
                return
            
            # Create minimum connections
            for _ in range(self.min_connections):
                connection = Connection(self.endpoint, self.config)
                await connection.connect()
                self._connections.append(connection)
            
            self._started = True
            logger.info("Connection pool started", active_connections=len(self._connections))
    
    async def close(self) -> None:
        """Close all connections in the pool."""
        if not self._started:
            return
        
        async with self._lock:
            for connection in self._connections:
                await connection.close()
            
            self._connections.clear()
            self._started = False
            
            logger.info("Connection pool closed")
    
    async def request(self, message: Message, timeout_ms: Optional[int] = None) -> Message:
        """Send a request using a connection from the pool."""
        if not self._started:
            await self.start()
        
        # Get next available connection (round-robin)
        connection = await self._get_connection()
        
        # Execute with retry
        retry_policy = self.config.retry_policy if self.config else RetryPolicy()
        retryable = RetryableOperation(retry_policy)
        
        async def _send_request():
            return await connection.request(message, timeout_ms)
        
        return await retryable.execute(_send_request)
    
    async def notify(self, message: Message) -> None:
        """Send a notification using a connection from the pool."""
        if not self._started:
            await self.start()
        
        connection = await self._get_connection()
        await connection.notify(message)
    
    async def _get_connection(self) -> Connection:
        """Get the next available connection."""
        async with self._lock:
            if not self._connections:
                raise ConnectionError("No connections available")
            
            # Round-robin selection
            connection = self._connections[self._connection_index]
            self._connection_index = (self._connection_index + 1) % len(self._connections)
            
            # Check if connection is healthy
            if not await connection.is_connected():
                # Try to reconnect
                try:
                    await connection.connect()
                except Exception as e:
                    logger.warning("Failed to reconnect", error=str(e))
                    # Could implement connection replacement logic here
            
            return connection
    
    async def is_healthy(self) -> bool:
        """Check if the pool has healthy connections."""
        if not self._started:
            return False
        
        healthy_count = 0
        for connection in self._connections:
            if await connection.is_connected():
                healthy_count += 1
        
        return healthy_count > 0
    
    async def get_stats(self) -> Dict[str, Any]:
        """Get pool statistics."""
        connection_stats = []
        for connection in self._connections:
            stats = await connection.get_stats()
            connection_stats.append(stats)
        
        healthy_connections = sum(1 for stats in connection_stats if stats['connected'])
        
        return {
            'endpoint': self.endpoint,
            'max_connections': self.max_connections,
            'active_connections': len(self._connections),
            'healthy_connections': healthy_connections,
            'started': self._started,
            'connections': connection_stats,
        }


# Import orjson at the end to avoid circular imports
try:
    import orjson
except ImportError:
    import json as orjson  # Fallback to standard json