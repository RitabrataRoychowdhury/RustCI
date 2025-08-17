# Valkyrie Protocol JavaScript/TypeScript SDK

High-performance JavaScript/TypeScript SDK for Valkyrie Protocol - distributed messaging with sub-millisecond latency.

## Features

- **Full TypeScript Support**: Complete type definitions and IntelliSense support
- **Promise-based API**: Modern async/await support with Promise-based operations
- **Connection Pooling**: Automatic connection management and pooling
- **Retry Logic**: Configurable retry policies with exponential backoff
- **Multiple Transports**: WebSocket, TCP (Node.js), and HTTP fallback
- **Event-driven Architecture**: EventEmitter-based for reactive programming
- **Browser & Node.js**: Works in both browser and Node.js environments
- **Zero Dependencies**: Minimal dependencies for small bundle size

## Installation

```bash
npm install @valkyrie-protocol/sdk
```

### For TypeScript projects:
```bash
npm install @valkyrie-protocol/sdk
npm install --save-dev @types/node  # If using Node.js features
```

## Quick Start

### Client Usage (TypeScript)

```typescript
import { ValkyrieClient, ClientConfig } from '@valkyrie-protocol/sdk';

async function main() {
  // Create client with configuration
  const config: ClientConfig = {
    endpoint: 'ws://localhost:8080',
    connectTimeoutMs: 5000,
    requestTimeoutMs: 30000,
    enablePooling: true,
    maxConnections: 10
  };
  
  const client = new ValkyrieClient(config);
  
  try {
    // Connect to server
    await client.connect();
    
    // Send a request
    const response = await client.request({ action: 'ping' });
    console.log('Response:', response);
    
    // Send a notification
    await client.notify({ event: 'user_login', userId: 123 });
    
  } finally {
    await client.disconnect();
  }
}

main().catch(console.error);
```

### Client Usage (JavaScript)

```javascript
const { ValkyrieClient } = require('@valkyrie-protocol/sdk');

async function main() {
  const client = new ValkyrieClient({
    endpoint: 'ws://localhost:8080',
    connectTimeoutMs: 5000,
    requestTimeoutMs: 30000
  });
  
  try {
    await client.connect();
    
    const response = await client.request({ action: 'ping' });
    console.log('Response:', response);
    
  } finally {
    await client.disconnect();
  }
}

main().catch(console.error);
```

### Server Usage (Node.js only)

```typescript
import { ValkyrieServer, ServerConfig, MessageHandler } from '@valkyrie-protocol/sdk';

class PingHandler implements MessageHandler {
  async handle(message: any): Promise<any> {
    return { status: 'pong', timestamp: Date.now() };
  }
}

async function main() {
  const config: ServerConfig = {
    bindAddress: '0.0.0.0',
    port: 8080,
    maxConnections: 10000
  };
  
  const server = new ValkyrieServer(config);
  server.registerHandler('/ping', new PingHandler());
  
  await server.start();
  console.log('Server started on port 8080');
  
  // Graceful shutdown
  process.on('SIGINT', async () => {
    await server.stop();
    process.exit(0);
  });
}

main().catch(console.error);
```

## Configuration

### Client Configuration

```typescript
interface ClientConfig {
  endpoint: string;                    // Server endpoint (ws://, wss://, tcp://)
  connectTimeoutMs?: number;           // Connection timeout (default: 5000)
  requestTimeoutMs?: number;           // Request timeout (default: 30000)
  maxConnections?: number;             // Max connections in pool (default: 10)
  enablePooling?: boolean;             // Enable connection pooling (default: true)
  retryPolicy?: RetryPolicy;           // Retry configuration
  enableMetrics?: boolean;             // Enable metrics collection (default: false)
  headers?: Record<string, string>;    // Custom headers for WebSocket
}
```

### Server Configuration (Node.js only)

```typescript
interface ServerConfig {
  bindAddress?: string;                // Bind address (default: '0.0.0.0')
  port: number;                        // Port to listen on
  maxConnections?: number;             // Max concurrent connections (default: 10000)
  connectionTimeoutMs?: number;        // Connection timeout (default: 30000)
  enableTls?: boolean;                 // Enable TLS (default: false)
  tlsCertPath?: string;                // TLS certificate path
  tlsKeyPath?: string;                 // TLS private key path
  enableMetrics?: boolean;             // Enable metrics (default: false)
}
```

## Advanced Usage

### Connection Pooling

```typescript
import { ValkyrieClient, ConnectionPool } from '@valkyrie-protocol/sdk';

// Manual pool management
const pool = new ConnectionPool({
  endpoint: 'ws://localhost:8080',
  maxConnections: 20,
  connectTimeoutMs: 5000
});

await pool.start();

// Use pool for requests
const response = await pool.request({ action: 'getData' });

await pool.close();
```

### Custom Message Handlers

```typescript
import { MessageHandler } from '@valkyrie-protocol/sdk';

class CustomHandler implements MessageHandler {
  async handle(message: any): Promise<any> {
    const { action } = message;
    
    switch (action) {
      case 'processData':
        return await this.processData(message.data);
      default:
        throw new Error(`Unknown action: ${action}`);
    }
  }
  
  private async processData(data: any): Promise<any> {
    // Your custom processing logic
    return { processed: true, data };
  }
}
```

### Event Handling

```typescript
import { ValkyrieClient } from '@valkyrie-protocol/sdk';

const client = new ValkyrieClient({ endpoint: 'ws://localhost:8080' });

// Listen for connection events
client.on('connected', () => {
  console.log('Connected to server');
});

client.on('disconnected', () => {
  console.log('Disconnected from server');
});

client.on('error', (error) => {
  console.error('Client error:', error);
});

client.on('message', (message) => {
  console.log('Received message:', message);
});

await client.connect();
```

### Retry Policies

```typescript
import { ValkyrieClient, RetryPolicy } from '@valkyrie-protocol/sdk';

const retryPolicy: RetryPolicy = {
  maxAttempts: 3,
  initialDelayMs: 100,
  maxDelayMs: 5000,
  backoffMultiplier: 2.0,
  jitter: true
};

const client = new ValkyrieClient({
  endpoint: 'ws://localhost:8080',
  retryPolicy
});
```

## Browser Usage

The SDK works in browsers using WebSocket transport:

```html
<!DOCTYPE html>
<html>
<head>
  <script src="https://unpkg.com/@valkyrie-protocol/sdk/dist/browser.js"></script>
</head>
<body>
  <script>
    async function main() {
      const client = new ValkyrieSDK.ValkyrieClient({
        endpoint: 'wss://your-server.com:8080'
      });
      
      await client.connect();
      const response = await client.request({ action: 'ping' });
      console.log('Response:', response);
    }
    
    main().catch(console.error);
  </script>
</body>
</html>
```

## CLI Tools

The SDK includes command-line tools for testing and development:

```bash
# Install globally
npm install -g @valkyrie-protocol/sdk

# Send a request
valkyrie-client request --endpoint ws://localhost:8080 --data '{"action": "ping"}'

# Send a notification
valkyrie-client notify --endpoint ws://localhost:8080 --data '{"event": "test"}'

# Benchmark performance
valkyrie-client benchmark --endpoint ws://localhost:8080 --requests 10000 --concurrency 100

# Start a test server
valkyrie-server start --port 8080 --handlers ./handlers.js
```

## Performance

The JavaScript SDK is optimized for high performance:

- **Sub-millisecond latency** for local connections
- **50K+ requests/second** throughput in Node.js
- **Efficient connection pooling** with automatic load balancing
- **Minimal memory overhead** with object pooling
- **Fast JSON serialization** with native JSON support

## Error Handling

```typescript
import { 
  ValkyrieError, 
  ConnectionError, 
  TimeoutError, 
  ProtocolError 
} from '@valkyrie-protocol/sdk';

try {
  const response = await client.request({ action: 'test' });
} catch (error) {
  if (error instanceof TimeoutError) {
    console.log('Request timed out');
  } else if (error instanceof ConnectionError) {
    console.log('Connection failed');
  } else if (error instanceof ValkyrieError) {
    console.log('Valkyrie error:', error.message);
  } else {
    console.log('Unknown error:', error);
  }
}
```

## Testing

```bash
# Run tests
npm test

# Run tests in watch mode
npm run test:watch

# Run specific test
npm test -- --testNamePattern="Client"
```

## Building from Source

```bash
# Clone repository
git clone https://github.com/rustci/valkyrie-protocol.git
cd valkyrie-protocol/valkyrie-sdk/javascript

# Install dependencies
npm install

# Build TypeScript
npm run build

# Run tests
npm test
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