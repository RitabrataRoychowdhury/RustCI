# Valkyrie Protocol Python SDK

High-performance Python SDK for Valkyrie Protocol - distributed messaging with sub-millisecond latency.

## Features

- **Async/Await Support**: Built on asyncio with full async/await support
- **Type Safety**: Complete type hints and Pydantic models
- **Connection Pooling**: Automatic connection management and pooling
- **Retry Logic**: Configurable retry policies with exponential backoff
- **Multiple Transports**: TCP, QUIC, Unix sockets, WebSockets
- **Observability**: Built-in metrics, tracing, and structured logging
- **Security**: TLS, mTLS, JWT authentication support

## Installation

```bash
pip install valkyrie-protocol
```

### Development Installation

```bash
git clone https://github.com/rustci/valkyrie-protocol.git
cd valkyrie-protocol/valkyrie-sdk/python
pip install -e ".[dev]"
```

## Quick Start

### Client Usage

```python
import asyncio
from valkyrie import ValkyrieClient, ClientConfig

async def main():
    # Create client with configuration
    config = ClientConfig(
        endpoint="tcp://localhost:8080",
        connect_timeout_ms=5000,
        request_timeout_ms=30000,
        enable_pooling=True,
        max_connections=10
    )
    
    async with ValkyrieClient(config) as client:
        # Send a request
        response = await client.request({"action": "ping"})
        print(f"Response: {response}")
        
        # Send a notification
        await client.notify({"event": "user_login", "user_id": 123})

if __name__ == "__main__":
    asyncio.run(main())
```

### Server Usage

```python
import asyncio
from valkyrie import ValkyrieServer, ServerConfig, MessageHandler

class PingHandler(MessageHandler):
    async def handle(self, message: dict) -> dict:
        return {"status": "pong", "timestamp": time.time()}

async def main():
    config = ServerConfig(
        bind_address="0.0.0.0",
        port=8080,
        max_connections=10000
    )
    
    server = ValkyrieServer(config)
    server.register_handler("/ping", PingHandler())
    
    await server.start()
    print("Server started on port 8080")
    
    # Keep server running
    try:
        await asyncio.Event().wait()
    except KeyboardInterrupt:
        await server.stop()

if __name__ == "__main__":
    asyncio.run(main())
```

## Configuration

### Client Configuration

```python
from valkyrie import ClientConfig, RetryPolicy

config = ClientConfig(
    endpoint="tcp://localhost:8080",
    connect_timeout_ms=5000,
    request_timeout_ms=30000,
    max_connections=10,
    enable_pooling=True,
    retry_policy=RetryPolicy(
        max_attempts=3,
        initial_delay_ms=100,
        max_delay_ms=5000,
        backoff_multiplier=2.0
    ),
    enable_metrics=True
)
```

### Server Configuration

```python
from valkyrie import ServerConfig

config = ServerConfig(
    bind_address="0.0.0.0",
    port=8080,
    max_connections=10000,
    connection_timeout_ms=30000,
    enable_tls=True,
    tls_cert_path="/path/to/cert.pem",
    tls_key_path="/path/to/key.pem",
    enable_metrics=True
)
```

## Advanced Usage

### Connection Pooling

```python
from valkyrie import ValkyrieClient, ConnectionPool

# Manual pool management
pool = ConnectionPool(
    endpoint="tcp://localhost:8080",
    max_connections=20,
    connect_timeout_ms=5000
)

await pool.start()

# Use pool for requests
response = await pool.request({"action": "get_data"})

await pool.close()
```

### Custom Message Handlers

```python
from valkyrie import MessageHandler
from typing import Any, Dict

class CustomHandler(MessageHandler):
    async def handle(self, message: Dict[str, Any]) -> Dict[str, Any]:
        # Process the message
        action = message.get("action")
        
        if action == "process_data":
            result = await self.process_data(message["data"])
            return {"status": "success", "result": result}
        
        return {"status": "error", "message": "Unknown action"}
    
    async def process_data(self, data: Any) -> Any:
        # Your custom processing logic
        return {"processed": True, "data": data}
```

### Observability

```python
from valkyrie import ValkyrieClient, ClientConfig
from valkyrie.observability import PrometheusMetrics, StructuredLogger

# Enable metrics
config = ClientConfig(
    endpoint="tcp://localhost:8080",
    enable_metrics=True
)

# Configure structured logging
logger = StructuredLogger(
    level="INFO",
    format="json",
    include_trace_id=True
)

client = ValkyrieClient(config, logger=logger)

# Metrics will be automatically collected
async with client:
    response = await client.request({"action": "test"})
```

## CLI Tools

### Client CLI

```bash
# Send a request
valkyrie-client request --endpoint tcp://localhost:8080 --data '{"action": "ping"}'

# Send a notification
valkyrie-client notify --endpoint tcp://localhost:8080 --data '{"event": "test"}'

# Benchmark performance
valkyrie-client benchmark --endpoint tcp://localhost:8080 --requests 10000 --concurrency 100
```

### Server CLI

```bash
# Start a test server
valkyrie-server start --bind 0.0.0.0:8080 --handlers examples/handlers.py

# Start with TLS
valkyrie-server start --bind 0.0.0.0:8443 --tls-cert cert.pem --tls-key key.pem
```

## Performance

The Python SDK is optimized for high performance:

- **Sub-millisecond latency** for local connections
- **100K+ requests/second** throughput
- **Minimal memory overhead** with connection pooling
- **Efficient serialization** with orjson and msgpack support

## Error Handling

```python
from valkyrie import ValkyrieError, ConnectionError, TimeoutError

try:
    response = await client.request({"action": "test"})
except TimeoutError:
    print("Request timed out")
except ConnectionError:
    print("Connection failed")
except ValkyrieError as e:
    print(f"Valkyrie error: {e}")
```

## Testing

```bash
# Run tests
pytest

# Run with coverage
pytest --cov=valkyrie --cov-report=html

# Run specific test
pytest tests/test_client.py::test_basic_request
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run the test suite
6. Submit a pull request

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.