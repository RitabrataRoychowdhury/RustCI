"""
Valkyrie Protocol Error Types
"""

from typing import Optional, Any


class ValkyrieError(Exception):
    """Base exception for all Valkyrie Protocol errors."""
    
    def __init__(self, message: str, code: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message)
        self.message = message
        self.code = code
        self.details = details
    
    def __str__(self) -> str:
        if self.code:
            return f"[{self.code}] {self.message}"
        return self.message
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(message='{self.message}', code='{self.code}')"


class ConnectionError(ValkyrieError):
    """Raised when connection operations fail."""
    
    def __init__(self, message: str, endpoint: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message, "CONNECTION_ERROR", details)
        self.endpoint = endpoint


class TimeoutError(ValkyrieError):
    """Raised when operations timeout."""
    
    def __init__(self, message: str, timeout_ms: Optional[int] = None, details: Optional[Any] = None):
        super().__init__(message, "TIMEOUT_ERROR", details)
        self.timeout_ms = timeout_ms


class ProtocolError(ValkyrieError):
    """Raised when protocol-level errors occur."""
    
    def __init__(self, message: str, protocol_code: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message, "PROTOCOL_ERROR", details)
        self.protocol_code = protocol_code


class ConfigurationError(ValkyrieError):
    """Raised when configuration is invalid."""
    
    def __init__(self, message: str, field: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message, "CONFIGURATION_ERROR", details)
        self.field = field


class AuthenticationError(ValkyrieError):
    """Raised when authentication fails."""
    
    def __init__(self, message: str, auth_method: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message, "AUTHENTICATION_ERROR", details)
        self.auth_method = auth_method


class SerializationError(ValkyrieError):
    """Raised when serialization/deserialization fails."""
    
    def __init__(self, message: str, data_type: Optional[str] = None, details: Optional[Any] = None):
        super().__init__(message, "SERIALIZATION_ERROR", details)
        self.data_type = data_type


class RetryExhaustedError(ValkyrieError):
    """Raised when retry attempts are exhausted."""
    
    def __init__(self, message: str, attempts: int, last_error: Optional[Exception] = None):
        super().__init__(message, "RETRY_EXHAUSTED", {"attempts": attempts, "last_error": str(last_error)})
        self.attempts = attempts
        self.last_error = last_error