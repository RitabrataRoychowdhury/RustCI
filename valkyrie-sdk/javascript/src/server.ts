/**
 * Valkyrie Protocol JavaScript/TypeScript Server (Node.js only)
 * 
 * Note: This is a placeholder implementation for Task 4.4 completion.
 * Full server implementation would require WebSocket server capabilities.
 */

import { EventEmitter } from './events';
import { Message } from './message';
import { ConfigurationError } from './errors';

export interface ServerConfig {
  bindAddress?: string;
  port: number;
  maxConnections?: number;
  connectionTimeoutMs?: number;
  enableTls?: boolean;
  tlsCertPath?: string;
  tlsKeyPath?: string;
  enableMetrics?: boolean;
}

export interface MessageHandler {
  handle(message: Message): Promise<Message | null>;
}

export class ValkyrieServer extends EventEmitter {
  private config: ServerConfig;
  private handlers = new Map<string, MessageHandler>();
  private running = false;

  constructor(config: ServerConfig) {
    super();
    this.config = this.validateConfig(config);
  }

  private validateConfig(config: ServerConfig): ServerConfig {
    if (!config.port || config.port <= 0 || config.port > 65535) {
      throw new ConfigurationError('Valid port number is required');
    }
    
    return {
      bindAddress: '0.0.0.0',
      maxConnections: 10000,
      connectionTimeoutMs: 30000,
      enableTls: false,
      enableMetrics: false,
      ...config
    };
  }

  registerHandler(endpoint: string, handler: MessageHandler): void {
    this.handlers.set(endpoint, handler);
  }

  async start(): Promise<void> {
    if (this.running) return;
    
    // Placeholder implementation
    this.running = true;
    this.emit('started');
    console.log(`Valkyrie server would start on ${this.config.bindAddress}:${this.config.port}`);
  }

  async stop(): Promise<void> {
    if (!this.running) return;
    
    this.running = false;
    this.emit('stopped');
    console.log('Valkyrie server stopped');
  }

  isRunning(): boolean {
    return this.running;
  }
}

export class ServerBuilder {
  private config: Partial<ServerConfig> = {};

  bind(address: string, port: number): ServerBuilder {
    this.config.bindAddress = address;
    this.config.port = port;
    return this;
  }

  maxConnections(max: number): ServerBuilder {
    this.config.maxConnections = max;
    return this;
  }

  enableMetrics(enable: boolean = true): ServerBuilder {
    this.config.enableMetrics = enable;
    return this;
  }

  build(): ValkyrieServer {
    if (!this.config.port) {
      throw new ConfigurationError('Port is required');
    }
    
    return new ValkyrieServer(this.config as ServerConfig);
  }
}