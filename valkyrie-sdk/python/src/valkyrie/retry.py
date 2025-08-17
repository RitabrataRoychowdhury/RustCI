"""
Retry Policy Implementation
"""

import asyncio
import random
import time
from typing import Optional, Callable, Any, Type, Union, List
from dataclasses import dataclass
from .errors import RetryExhaustedError


@dataclass
class RetryPolicy:
    """Configuration for retry behavior."""
    
    max_attempts: int = 3
    initial_delay_ms: int = 100
    max_delay_ms: int = 5000
    backoff_multiplier: float = 2.0
    jitter: bool = True
    retryable_exceptions: Optional[List[Type[Exception]]] = None
    
    def __post_init__(self):
        if self.retryable_exceptions is None:
            # Default retryable exceptions
            from .errors import ConnectionError, TimeoutError, ProtocolError
            self.retryable_exceptions = [ConnectionError, TimeoutError, ProtocolError]
    
    def is_retryable(self, exception: Exception) -> bool:
        """Check if an exception is retryable."""
        if not self.retryable_exceptions:
            return True
        
        return any(isinstance(exception, exc_type) for exc_type in self.retryable_exceptions)
    
    def calculate_delay(self, attempt: int) -> float:
        """Calculate delay for the given attempt number (0-based)."""
        if attempt == 0:
            return 0.0
        
        # Exponential backoff
        delay_ms = min(
            self.initial_delay_ms * (self.backoff_multiplier ** (attempt - 1)),
            self.max_delay_ms
        )
        
        # Add jitter to prevent thundering herd
        if self.jitter:
            jitter_range = delay_ms * 0.1  # 10% jitter
            delay_ms += random.uniform(-jitter_range, jitter_range)
        
        return max(0.0, delay_ms / 1000.0)  # Convert to seconds


class RetryableOperation:
    """Wrapper for retryable operations."""
    
    def __init__(self, policy: RetryPolicy):
        self.policy = policy
    
    async def execute(
        self,
        operation: Callable[..., Any],
        *args,
        **kwargs
    ) -> Any:
        """Execute an operation with retry logic."""
        last_exception = None
        
        for attempt in range(self.policy.max_attempts):
            try:
                # Calculate and apply delay
                delay = self.policy.calculate_delay(attempt)
                if delay > 0:
                    await asyncio.sleep(delay)
                
                # Execute the operation
                if asyncio.iscoroutinefunction(operation):
                    return await operation(*args, **kwargs)
                else:
                    return operation(*args, **kwargs)
                    
            except Exception as e:
                last_exception = e
                
                # Check if we should retry
                if not self.policy.is_retryable(e):
                    raise e
                
                # If this was the last attempt, raise RetryExhaustedError
                if attempt == self.policy.max_attempts - 1:
                    raise RetryExhaustedError(
                        f"Operation failed after {self.policy.max_attempts} attempts",
                        attempts=self.policy.max_attempts,
                        last_error=last_exception
                    )
        
        # This should never be reached, but just in case
        raise RetryExhaustedError(
            f"Operation failed after {self.policy.max_attempts} attempts",
            attempts=self.policy.max_attempts,
            last_error=last_exception
        )


def with_retry(policy: RetryPolicy):
    """Decorator for adding retry logic to functions."""
    
    def decorator(func: Callable) -> Callable:
        async def async_wrapper(*args, **kwargs):
            retryable = RetryableOperation(policy)
            return await retryable.execute(func, *args, **kwargs)
        
        def sync_wrapper(*args, **kwargs):
            # For sync functions, we can't use asyncio.sleep
            # So we implement a synchronous version
            last_exception = None
            
            for attempt in range(policy.max_attempts):
                try:
                    # Calculate and apply delay
                    delay = policy.calculate_delay(attempt)
                    if delay > 0:
                        time.sleep(delay)
                    
                    return func(*args, **kwargs)
                    
                except Exception as e:
                    last_exception = e
                    
                    if not policy.is_retryable(e):
                        raise e
                    
                    if attempt == policy.max_attempts - 1:
                        raise RetryExhaustedError(
                            f"Operation failed after {policy.max_attempts} attempts",
                            attempts=policy.max_attempts,
                            last_error=last_exception
                        )
            
            raise RetryExhaustedError(
                f"Operation failed after {policy.max_attempts} attempts",
                attempts=policy.max_attempts,
                last_error=last_exception
            )
        
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        else:
            return sync_wrapper
    
    return decorator


# Predefined retry policies
class RetryPolicies:
    """Common retry policy configurations."""
    
    @staticmethod
    def no_retry() -> RetryPolicy:
        """No retry policy."""
        return RetryPolicy(max_attempts=1)
    
    @staticmethod
    def quick_retry() -> RetryPolicy:
        """Quick retry for fast operations."""
        return RetryPolicy(
            max_attempts=3,
            initial_delay_ms=50,
            max_delay_ms=500,
            backoff_multiplier=1.5
        )
    
    @staticmethod
    def standard_retry() -> RetryPolicy:
        """Standard retry policy."""
        return RetryPolicy(
            max_attempts=3,
            initial_delay_ms=100,
            max_delay_ms=5000,
            backoff_multiplier=2.0
        )
    
    @staticmethod
    def aggressive_retry() -> RetryPolicy:
        """Aggressive retry for critical operations."""
        return RetryPolicy(
            max_attempts=5,
            initial_delay_ms=100,
            max_delay_ms=10000,
            backoff_multiplier=2.0
        )
    
    @staticmethod
    def connection_retry() -> RetryPolicy:
        """Retry policy optimized for connection failures."""
        from .errors import ConnectionError, TimeoutError
        
        return RetryPolicy(
            max_attempts=5,
            initial_delay_ms=200,
            max_delay_ms=5000,
            backoff_multiplier=1.8,
            retryable_exceptions=[ConnectionError, TimeoutError]
        )