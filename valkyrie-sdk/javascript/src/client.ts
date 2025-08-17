/**
 * Valkyrie Protocol JavaScript/TypeScript Client
 */

import { EventEmitter } from './events';
import { Message, MessageType } from './message';
import { RetryPolicy, RetryableOperation, RetryPolicies } from './retry';
import { ValkyrieError, ConnectionError, TimeoutError, ConfigurationError } from './errors';
import { Logger, LogLevel } from './logger';

export interface ClientConfig {
  endpoint: string;
  connectTimeoutMs?: number;
  requestTimeoutMs?: number;
  maxConnections?: number;
  enablePooling?: boolean;
  retryPolicy?: RetryPolicy;
  enableMetrics?: boolean;
  headers?: Record<string, string>;
  enableTls?: boolean;
  authToken?: string;
  authMethod?: 'none' | 'jwt' | 'api_key';
}

export interface ClientStats {
  connected: boolean;
  endpoint: string;
  poolingEnabled: boolean;
  requestsSent: number;
  responsesReceived: number;
  errors: number;
  connectedAt?: number;
  lastActivity?: number;
}

export class ValkyrieClient extends EventEmitter {
  private config: Required<ClientConfig>;
  private connection?: WebSocket;
  private connected = false;
  private connecting = false;
  private pendingRequests = new Map<string, {
    resolve: (message: Message) => void;
    reject: (error: Error) => void;
    timeout: NodeJS.Timeout;
  }>();
  private stats: ClientStats;
  private logger: Logger;

  constructor(config: ClientConfig) {
    super();
    
    this.config = this.validateAndNormalizeConfig(config);
    this.logger = new Logger({ 
      level: LogLevel.INFO, 
      prefix: 'ValkyrieClient' 
    });
    
    this.stats = {
      connected: false,
      endpoint: this.config.endpoint,
      poolingEnabled: this.config.enablePooling,
      requestsSent: 0,
      responsesReceived: 0,
      errors: 0
    };

    this.logger.info('Valkyrie client initialized', { endpoint: config.endpoint });
  }

  private validateAndNormalizeConfig(config: ClientConfig): Required<ClientConfig> {
    if (!config.endpoint) {
      throw new ConfigurationError('Endpoint is required');
    }

    if (!config.endpoint.startsWith('ws://') && !config.endpoint.startsWith('wss://')) {
      throw new ConfigurationError('Only WebSocket endpoints (ws:// or wss://) are supported in browser/Node.js');
    }

    return {
      endpoint: config.endpoint,
      connectTimeoutMs: config.connectTimeoutMs ?? 5000,
      requestTimeoutMs: config.requestTimeoutMs ?? 30000,
      maxConnections: config.maxConnections ?? 10,
      enablePooling: config.enablePooling ?? false, // Simplified for initial implementation
      retryPolicy: config.retryPolicy ?? RetryPolicies.standardRetry(),
      enableMetrics: config.enableMetrics ?? false,
      headers: config.headers ?? {},
      enableTls: config.enableTls ?? false,
      authToken: config.authToken ?? '',
      authMethod: config.authMethod ?? 'none'
    };
  }

  async connect(): Promise<void> {
    if (this.connected || this.connecting) {
      return;
    }

    this.connecting = true;

    try {
      await this.establishConnection();
      this.connected = true;
      this.stats.connected = true;
      this.stats.connectedAt = Date.now();
      this.stats.lastActivity = Date.now();
      
      this.emit('connected');
      this.logger.info('Connected to server');
    } catch (error) {
      this.connecting = false;
      this.emit('error', error);
      throw error;
    } finally {
      this.connecting = false;
    }
  }

  async disconnect(): Promise<void> {
    if (!this.connected) {
      return;
    }

    this.connected = false;
    this.stats.connected = false;

    // Cancel all pending requests
    for (const [id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(new ConnectionError('Client disconnected'));
    }
    this.pendingRequests.clear();

    // Close WebSocket connection
    if (this.connection) {
      this.connection.close();
      this.connection = undefined;
    }

    this.emit('disconnected');
    this.logger.info('Disconnected from server');
  }

  async request<T = any>(data: any, endpoint?: string, timeoutMs?: number): Promise<T> {
    if (!this.connected) {
      await this.connect();
    }

    const message = Message.request(data, endpoint);
    const timeout = timeoutMs ?? this.config.requestTimeoutMs;

    // Execute with retry
    const retryable = new RetryableOperation(this.config.retryPolicy);
    
    return retryable.execute(async () => {
      return this.sendRequest<T>(message, timeout);
    });
  }

  async notify(data: any, endpoint?: string): Promise<void> {
    if (!this.connected) {
      await this.connect();
    }

    const message = Message.notification(data, endpoint);
    
    // Execute with retry
    const retryable = new RetryableOperation(this.config.retryPolicy);
    
    return retryable.execute(async () => {
      return this.sendNotification(message);
    });
  }

  async ping(timeoutMs: number = 5000): Promise<number> {
    const startTime = Date.now();
    
    try {
      await this.request({ action: 'ping', timestamp: startTime }, '/ping', timeoutMs);
      const endTime = Date.now();
      const rttMs = endTime - startTime;
      
      this.logger.debug('Ping successful', { rttMs });
      return rttMs;
    } catch (error) {
      this.logger.error('Ping failed', { error });
      throw error;
    }
  }

  isConnected(): boolean {
    return this.connected && this.connection?.readyState === WebSocket.OPEN;
  }

  getStats(): ClientStats {
    return { ...this.stats };
  }

  private async establishConnection(): Promise<void> {
    return new Promise((resolve, reject) => {
      const connectTimeout = setTimeout(() => {
        reject(new ConnectionError(`Connection timeout after ${this.config.connectTimeoutMs}ms`));
      }, this.config.connectTimeoutMs);

      try {
        // Create WebSocket connection
        const headers: Record<string, string> = { ...this.config.headers };
        
        // Add authentication if configured
        if (this.config.authMethod === 'jwt' && this.config.authToken) {
          headers['Authorization'] = `Bearer ${this.config.authToken}`;
        } else if (this.config.authMethod === 'api_key' && this.config.authToken) {
          headers['X-API-Key'] = this.config.authToken;
        }

        // Note: In browser, headers are limited. In Node.js with 'ws' library, we can pass headers
        this.connection = new WebSocket(this.config.endpoint);

        this.connection.onopen = () => {
          clearTimeout(connectTimeout);
          this.setupMessageHandling();
          resolve();
        };

        this.connection.onerror = (event) => {
          clearTimeout(connectTimeout);
          reject(new ConnectionError(`WebSocket connection failed: ${event}`));
        };

        this.connection.onclose = () => {
          this.handleDisconnection();
        };

      } catch (error) {
        clearTimeout(connectTimeout);
        reject(new ConnectionError(`Failed to create WebSocket connection: ${error}`));
      }
    });
  }

  private setupMessageHandling(): void {
    if (!this.connection) return;

    this.connection.onmessage = (event) => {
      try {
        const messageData = JSON.parse(event.data);
        const message = Message.fromJSON(messageData);
        
        this.stats.lastActivity = Date.now();
        this.handleMessage(message);
      } catch (error) {
        this.logger.error('Failed to parse message', { error });
        this.emit('error', new ValkyrieError('Failed to parse incoming message'));
      }
    };
  }

  private handleMessage(message: Message): void {
    this.emit('message', message);

    // Handle responses
    if (message.messageType === MessageType.RESPONSE && message.correlationId) {
      const pending = this.pendingRequests.get(message.correlationId);
      if (pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(message.correlationId);
        this.stats.responsesReceived++;
        pending.resolve(message);
      }
    }
  }

  private handleDisconnection(): void {
    if (this.connected) {
      this.connected = false;
      this.stats.connected = false;
      this.emit('disconnected');
      this.logger.warn('Connection lost');
    }
  }

  private async sendRequest<T>(message: Message, timeoutMs: number): Promise<T> {
    return new Promise((resolve, reject) => {
      if (!this.connection || this.connection.readyState !== WebSocket.OPEN) {
        reject(new ConnectionError('Connection not available'));
        return;
      }

      // Set up timeout
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(message.id);
        this.stats.errors++;
        reject(new TimeoutError(`Request timeout after ${timeoutMs}ms`, timeoutMs));
      }, timeoutMs);

      // Store pending request
      this.pendingRequests.set(message.id, {
        resolve: (response: Message) => {
          if (response.messageType === MessageType.ERROR) {
            const errorData = response.payloadAsJson();
            reject(new ValkyrieError(
              errorData?.error || 'Server returned error',
              errorData?.code
            ));
          } else {
            const result = response.payloadAsJson<T>();
            resolve(result ?? response.payload);
          }
        },
        reject,
        timeout
      });

      // Send message
      try {
        this.connection.send(JSON.stringify(message.toJSON()));
        this.stats.requestsSent++;
        this.stats.lastActivity = Date.now();
      } catch (error) {
        clearTimeout(timeout);
        this.pendingRequests.delete(message.id);
        this.stats.errors++;
        reject(new ConnectionError(`Failed to send message: ${error}`));
      }
    });
  }

  private async sendNotification(message: Message): Promise<void> {
    if (!this.connection || this.connection.readyState !== WebSocket.OPEN) {
      throw new ConnectionError('Connection not available');
    }

    try {
      this.connection.send(JSON.stringify(message.toJSON()));
      this.stats.requestsSent++;
      this.stats.lastActivity = Date.now();
      this.logger.debug('Notification sent', { endpoint: message.endpoint });
    } catch (error) {
      this.stats.errors++;
      throw new ConnectionError(`Failed to send notification: ${error}`);
    }
  }
}

export class ClientBuilder {
  private config: Partial<ClientConfig> = {};

  endpoint(endpoint: string): ClientBuilder {
    this.config.endpoint = endpoint;
    return this;
  }

  timeouts(connectMs: number, requestMs: number): ClientBuilder {
    this.config.connectTimeoutMs = connectMs;
    this.config.requestTimeoutMs = requestMs;
    return this;
  }

  pooling(enabled: boolean, maxConnections: number = 10): ClientBuilder {
    this.config.enablePooling = enabled;
    this.config.maxConnections = maxConnections;
    return this;
  }

  retryPolicy(policy: RetryPolicy): ClientBuilder {
    this.config.retryPolicy = policy;
    return this;
  }

  auth(method: 'none' | 'jwt' | 'api_key', token?: string): ClientBuilder {
    this.config.authMethod = method;
    this.config.authToken = token;
    return this;
  }

  headers(headers: Record<string, string>): ClientBuilder {
    this.config.headers = headers;
    return this;
  }

  enableMetrics(enable: boolean = true): ClientBuilder {
    this.config.enableMetrics = enable;
    return this;
  }

  build(): ValkyrieClient {
    if (!this.config.endpoint) {
      throw new ConfigurationError('Endpoint is required');
    }
    
    return new ValkyrieClient(this.config as ClientConfig);
  }
}