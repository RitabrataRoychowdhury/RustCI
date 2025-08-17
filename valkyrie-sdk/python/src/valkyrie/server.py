"""
Valkyrie Protocol Python Server
"""

import asyncio
import time
from typing import Optional, Dict, Any, Callable, Awaitable
from dataclasses import dataclass
from abc import ABC, abstractmethod
import structlog

from .message import Message, MessageType
from .errors import ValkyrieError, ConnectionError
from .connection import Connection

logger = structlog.get_logger(__name__)


@dataclass
class ServerConfig:
    """Configuration for Valkyrie server."""
    
    # Server settings
    bind_address: str = "0.0.0.0"
    port: int = 8080
    max_connections: int = 10000
    connection_timeout_ms: int = 30000
    
    # Security settings
    enable_tls: bool = False
    tls_cert_path: Optional[str] = None
    tls_key_path: Optional[str] = None
    
    # Performance settings
    backlog: int = 100
    reuse_port: bool = True
    
    # Observability
    enable_metrics: bool = False
    
    def validate(self) -> None:
        """Validate server configuration."""
        if self.port <= 0 or self.port > 65535:
            raise ValkyrieError("Port must be between 1 and 65535")
        
        if self.max_connections <= 0:
            raise ValkyrieError("Max connections must be positive")
        
        if self.enable_tls and not (self.tls_cert_path and self.tls_key_path):
            raise ValkyrieError("TLS cert and key paths required when TLS is enabled")


class MessageHandler(ABC):
    """Abstract base class for message handlers."""
    
    @abstractmethod
    async def handle(self, message: Message) -> Optional[Message]:
        """Handle a message and optionally return a response."""
        pass


class FunctionHandler(MessageHandler):
    """Handler that wraps a simple function."""
    
    def __init__(self, handler_func: Callable[[Dict[str, Any]], Awaitable[Dict[str, Any]]]):
        self.handler_func = handler_func
    
    async def handle(self, message: Message) -> Optional[Message]:
        """Handle message using the wrapped function."""
        try:
            # Parse input
            input_data = message.payload_as_json()
            if input_data is None:
                input_data = {"data": message.payload_as_string() or ""}
            
            # Call handler function
            result = await self.handler_func(input_data)
            
            # Create response if this was a request
            if message.message_type == MessageType.REQUEST:
                return Message.response(result, message.id)
            
            return None
            
        except Exception as e:
            logger.error("Handler error", error=str(e), message_id=message.id)
            
            if message.message_type == MessageType.REQUEST:
                return Message.error(str(e), message.id)
            
            raise


class ClientConnection:
    """Represents a client connection to the server."""
    
    def __init__(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter, server: 'ValkyrieServer'):
        self.reader = reader
        self.writer = writer
        self.server = server
        self.client_address = writer.get_extra_info('peername')
        self.connected_at = time.time()
        self.last_activity = time.time()
        self.messages_handled = 0
        self._closed = False
        
        logger.debug("Client connected", address=self.client_address)
    
    async def handle_client(self) -> None:
        """Handle messages from this client."""
        try:
            while not self._closed:
                # Read message length
                length_bytes = await self.reader.readexactly(4)
                length = int.from_bytes(length_bytes, byteorder='big')
                
                # Read message data
                message_data = await self.reader.readexactly(length)
                message_dict = orjson.loads(message_data)
                
                # Parse message
                message = Message.from_dict(message_dict)
                
                # Update activity
                self.last_activity = time.time()
                self.messages_handled += 1
                
                # Handle message
                response = await self.server._handle_message(message)
                
                # Send response if needed
                if response:
                    await self._send_message(response)
                
        except asyncio.IncompleteReadError:
            # Client disconnected
            pass
        except Exception as e:
            logger.error("Client handler error", error=str(e), address=self.client_address)
        finally:
            await self.close()
    
    async def _send_message(self, message: Message) -> None:
        """Send a message to the client."""
        if self._closed:
            return
        
        try:
            # Serialize message
            message_data = message.to_dict()
            message_json = orjson.dumps(message_data)
            
            # Send length prefix + message
            length = len(message_json)
            length_bytes = length.to_bytes(4, byteorder='big')
            
            self.writer.write(length_bytes + message_json)
            await self.writer.drain()
            
        except Exception as e:
            logger.error("Failed to send message", error=str(e))
            await self.close()
    
    async def close(self) -> None:
        """Close the client connection."""
        if self._closed:
            return
        
        self._closed = True
        
        try:
            self.writer.close()
            await self.writer.wait_closed()
        except Exception:
            pass
        
        logger.debug("Client disconnected", address=self.client_address)
    
    def get_stats(self) -> Dict[str, Any]:
        """Get connection statistics."""
        return {
            'address': self.client_address,
            'connected_at': self.connected_at,
            'last_activity': self.last_activity,
            'messages_handled': self.messages_handled,
            'connected': not self._closed,
        }


class ValkyrieServer:
    """High-level Valkyrie Protocol server."""
    
    def __init__(self, config: ServerConfig):
        """Initialize server with configuration."""
        config.validate()
        self.config = config
        self._server: Optional[asyncio.Server] = None
        self._handlers: Dict[str, MessageHandler] = {}
        self._clients: Dict[str, ClientConnection] = {}
        self._running = False
        self._stats = {
            'started_at': None,
            'connections_accepted': 0,
            'messages_handled': 0,
            'errors': 0,
        }
        
        logger.info("Valkyrie server initialized", 
                   bind_address=config.bind_address, 
                   port=config.port)
    
    def register_handler(self, endpoint: str, handler: MessageHandler) -> None:
        """Register a message handler for an endpoint."""
        self._handlers[endpoint] = handler
        logger.info("Handler registered", endpoint=endpoint)
    
    def register_function(
        self, 
        endpoint: str, 
        handler_func: Callable[[Dict[str, Any]], Awaitable[Dict[str, Any]]]
    ) -> None:
        """Register a function as a message handler."""
        self.register_handler(endpoint, FunctionHandler(handler_func))
    
    async def start(self) -> None:
        """Start the server."""
        if self._running:
            return
        
        logger.info("Starting Valkyrie server", 
                   bind_address=self.config.bind_address, 
                   port=self.config.port)
        
        # Create server
        self._server = await asyncio.start_server(
            self._handle_client,
            self.config.bind_address,
            self.config.port,
            backlog=self.config.backlog,
            reuse_port=self.config.reuse_port
        )
        
        self._running = True
        self._stats['started_at'] = time.time()
        
        logger.info("Valkyrie server started", 
                   addresses=[str(sock.getsockname()) for sock in self._server.sockets])
    
    async def stop(self) -> None:
        """Stop the server."""
        if not self._running:
            return
        
        logger.info("Stopping Valkyrie server")
        
        self._running = False
        
        # Close all client connections
        for client in list(self._clients.values()):
            await client.close()
        self._clients.clear()
        
        # Close server
        if self._server:
            self._server.close()
            await self._server.wait_closed()
            self._server = None
        
        logger.info("Valkyrie server stopped")
    
    async def serve_forever(self) -> None:
        """Start the server and serve forever."""
        await self.start()
        
        if self._server:
            async with self._server:
                await self._server.serve_forever()
    
    async def _handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        """Handle a new client connection."""
        client_address = writer.get_extra_info('peername')
        
        # Check connection limit
        if len(self._clients) >= self.config.max_connections:
            logger.warning("Connection limit reached", address=client_address)
            writer.close()
            await writer.wait_closed()
            return
        
        # Create client connection
        client = ClientConnection(reader, writer, self)
        client_id = f"{client_address[0]}:{client_address[1]}:{time.time()}"
        self._clients[client_id] = client
        
        self._stats['connections_accepted'] += 1
        
        try:
            # Handle client messages
            await client.handle_client()
        finally:
            # Clean up
            self._clients.pop(client_id, None)
    
    async def _handle_message(self, message: Message) -> Optional[Message]:
        """Handle a message using registered handlers."""
        try:
            self._stats['messages_handled'] += 1
            
            # Find handler
            endpoint = message.endpoint or "/"
            handler = self._handlers.get(endpoint)
            
            if not handler:
                # Try default handler
                handler = self._handlers.get("/")
            
            if not handler:
                if message.message_type == MessageType.REQUEST:
                    return Message.error(f"No handler for endpoint: {endpoint}", message.id)
                return None
            
            # Handle message
            response = await handler.handle(message)
            return response
            
        except Exception as e:
            self._stats['errors'] += 1
            logger.error("Message handling error", error=str(e), message_id=message.id)
            
            if message.message_type == MessageType.REQUEST:
                return Message.error(f"Internal server error: {e}", message.id)
            
            return None
    
    async def is_running(self) -> bool:
        """Check if server is running."""
        return self._running and self._server is not None
    
    async def get_stats(self) -> Dict[str, Any]:
        """Get server statistics."""
        active_connections = len(self._clients)
        client_stats = [client.get_stats() for client in self._clients.values()]
        
        return {
            'running': self._running,
            'bind_address': self.config.bind_address,
            'port': self.config.port,
            'max_connections': self.config.max_connections,
            'active_connections': active_connections,
            'started_at': self._stats['started_at'],
            'connections_accepted': self._stats['connections_accepted'],
            'messages_handled': self._stats['messages_handled'],
            'errors': self._stats['errors'],
            'handlers': list(self._handlers.keys()),
            'clients': client_stats,
        }


class ServerBuilder:
    """Builder for creating Valkyrie servers."""
    
    def __init__(self):
        self.config = ServerConfig()
        self.handlers: Dict[str, MessageHandler] = {}
    
    def bind(self, address: str, port: int) -> 'ServerBuilder':
        """Set bind address and port."""
        self.config.bind_address = address
        self.config.port = port
        return self
    
    def max_connections(self, max_conn: int) -> 'ServerBuilder':
        """Set maximum connections."""
        self.config.max_connections = max_conn
        return self
    
    def tls(self, cert_path: str, key_path: str) -> 'ServerBuilder':
        """Enable TLS with certificate and key files."""
        self.config.enable_tls = True
        self.config.tls_cert_path = cert_path
        self.config.tls_key_path = key_path
        return self
    
    def handler(self, endpoint: str, handler: MessageHandler) -> 'ServerBuilder':
        """Add a message handler."""
        self.handlers[endpoint] = handler
        return self
    
    def function(
        self, 
        endpoint: str, 
        handler_func: Callable[[Dict[str, Any]], Awaitable[Dict[str, Any]]]
    ) -> 'ServerBuilder':
        """Add a function handler."""
        self.handlers[endpoint] = FunctionHandler(handler_func)
        return self
    
    def enable_metrics(self, enable: bool = True) -> 'ServerBuilder':
        """Enable metrics collection."""
        self.config.enable_metrics = enable
        return self
    
    def build(self) -> ValkyrieServer:
        """Build the server."""
        server = ValkyrieServer(self.config)
        
        # Register all handlers
        for endpoint, handler in self.handlers.items():
            server.register_handler(endpoint, handler)
        
        return server


# Import orjson at the end to avoid circular imports
try:
    import orjson
except ImportError:
    import json as orjson  # Fallback to standard json