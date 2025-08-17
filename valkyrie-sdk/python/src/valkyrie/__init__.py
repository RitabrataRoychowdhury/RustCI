"""
Valkyrie Protocol Python SDK

High-performance distributed messaging with sub-millisecond latency.
"""

from .client import ValkyrieClient, ClientConfig, ClientBuilder
from .server import ValkyrieServer, ServerConfig, MessageHandler, ServerBuilder, FunctionHandler
from .connection import ConnectionPool, Connection
from .message import Message, MessageType
from .retry import RetryPolicy, RetryableOperation, RetryPolicies
from .errors import (
    ValkyrieError,
    ConnectionError,
    TimeoutError,
    ProtocolError,
    ConfigurationError,
    AuthenticationError,
    SerializationError,
    RetryExhaustedError,
)

__version__ = "0.1.0"
__author__ = "RustCI Team"
__email__ = "team@rustci.dev"

__all__ = [
    # Core classes
    "ValkyrieClient",
    "ValkyrieServer",
    "ConnectionPool",
    "Connection",
    "Message",
    "MessageHandler",
    "FunctionHandler",
    
    # Builders
    "ClientBuilder",
    "ServerBuilder",
    
    # Configuration
    "ClientConfig",
    "ServerConfig",
    "RetryPolicy",
    "RetryableOperation",
    "RetryPolicies",
    
    # Enums
    "MessageType",
    
    # Errors
    "ValkyrieError",
    "ConnectionError",
    "TimeoutError",
    "ProtocolError",
    "ConfigurationError",
    "AuthenticationError",
    "SerializationError",
    "RetryExhaustedError",
    
    # Metadata
    "__version__",
    "__author__",
    "__email__",
]