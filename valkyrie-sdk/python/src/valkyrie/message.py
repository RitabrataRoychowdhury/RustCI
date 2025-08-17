"""
Valkyrie Protocol Message Types
"""

import uuid
import time
from enum import Enum
from typing import Optional, Dict, Any, Union
from dataclasses import dataclass, field
import orjson


class MessageType(Enum):
    """Message type enumeration."""
    REQUEST = "request"
    RESPONSE = "response"
    NOTIFICATION = "notification"
    ERROR = "error"


@dataclass
class Message:
    """Valkyrie Protocol message."""
    
    # Core fields
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    message_type: MessageType = MessageType.REQUEST
    payload: bytes = field(default_factory=bytes)
    
    # Optional fields
    correlation_id: Optional[str] = None
    endpoint: Optional[str] = None
    timestamp: float = field(default_factory=time.time)
    headers: Dict[str, str] = field(default_factory=dict)
    
    # Metadata
    priority: int = 0
    ttl_ms: Optional[int] = None
    
    @classmethod
    def request(
        cls,
        payload: Union[bytes, str, Dict[str, Any]],
        endpoint: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
        priority: int = 0,
        ttl_ms: Optional[int] = None
    ) -> "Message":
        """Create a request message."""
        return cls(
            message_type=MessageType.REQUEST,
            payload=cls._serialize_payload(payload),
            endpoint=endpoint,
            headers=headers or {},
            priority=priority,
            ttl_ms=ttl_ms
        )
    
    @classmethod
    def response(
        cls,
        payload: Union[bytes, str, Dict[str, Any]],
        correlation_id: str,
        headers: Optional[Dict[str, str]] = None
    ) -> "Message":
        """Create a response message."""
        return cls(
            message_type=MessageType.RESPONSE,
            payload=cls._serialize_payload(payload),
            correlation_id=correlation_id,
            headers=headers or {}
        )
    
    @classmethod
    def notification(
        cls,
        payload: Union[bytes, str, Dict[str, Any]],
        endpoint: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
        priority: int = 0
    ) -> "Message":
        """Create a notification message."""
        return cls(
            message_type=MessageType.NOTIFICATION,
            payload=cls._serialize_payload(payload),
            endpoint=endpoint,
            headers=headers or {},
            priority=priority
        )
    
    @classmethod
    def error(
        cls,
        error_message: str,
        correlation_id: str,
        error_code: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None
    ) -> "Message":
        """Create an error message."""
        error_payload = {
            "error": error_message,
            "code": error_code,
            "timestamp": time.time()
        }
        
        return cls(
            message_type=MessageType.ERROR,
            payload=cls._serialize_payload(error_payload),
            correlation_id=correlation_id,
            headers=headers or {}
        )
    
    @staticmethod
    def _serialize_payload(payload: Union[bytes, str, Dict[str, Any]]) -> bytes:
        """Serialize payload to bytes."""
        if isinstance(payload, bytes):
            return payload
        elif isinstance(payload, str):
            return payload.encode('utf-8')
        else:
            # Use orjson for fast JSON serialization
            return orjson.dumps(payload)
    
    def payload_as_string(self) -> Optional[str]:
        """Get payload as string if possible."""
        try:
            return self.payload.decode('utf-8')
        except UnicodeDecodeError:
            return None
    
    def payload_as_json(self) -> Optional[Dict[str, Any]]:
        """Get payload as JSON object if possible."""
        try:
            return orjson.loads(self.payload)
        except (orjson.JSONDecodeError, UnicodeDecodeError):
            return None
    
    def is_expired(self) -> bool:
        """Check if message has expired based on TTL."""
        if self.ttl_ms is None:
            return False
        
        elapsed_ms = (time.time() - self.timestamp) * 1000
        return elapsed_ms > self.ttl_ms
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert message to dictionary for serialization."""
        return {
            "id": self.id,
            "message_type": self.message_type.value,
            "payload": self.payload.hex(),  # Hex encode bytes for JSON
            "correlation_id": self.correlation_id,
            "endpoint": self.endpoint,
            "timestamp": self.timestamp,
            "headers": self.headers,
            "priority": self.priority,
            "ttl_ms": self.ttl_ms
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Message":
        """Create message from dictionary."""
        return cls(
            id=data["id"],
            message_type=MessageType(data["message_type"]),
            payload=bytes.fromhex(data["payload"]),
            correlation_id=data.get("correlation_id"),
            endpoint=data.get("endpoint"),
            timestamp=data["timestamp"],
            headers=data.get("headers", {}),
            priority=data.get("priority", 0),
            ttl_ms=data.get("ttl_ms")
        )
    
    def __str__(self) -> str:
        return f"Message(id={self.id}, type={self.message_type.value}, endpoint={self.endpoint})"
    
    def __repr__(self) -> str:
        return (
            f"Message(id='{self.id}', message_type={self.message_type}, "
            f"payload_size={len(self.payload)}, endpoint='{self.endpoint}')"
        )