/**
 * Valkyrie Protocol JavaScript/TypeScript SDK
 * 
 * High-performance distributed messaging with sub-millisecond latency.
 */

// Core exports
export { ValkyrieClient, ClientBuilder } from './client';
export { ValkyrieServer, ServerBuilder } from './server';
export { ConnectionPool, Connection } from './connection';
export { Message, MessageType } from './message';
export { RetryPolicy, RetryableOperation, RetryPolicies } from './retry';

// Configuration exports
export type { ClientConfig } from './client';
export type { ServerConfig, MessageHandler } from './server';
export type { ConnectionConfig } from './connection';

// Error exports
export {
  ValkyrieError,
  ConnectionError,
  TimeoutError,
  ProtocolError,
  ConfigurationError,
  AuthenticationError,
  SerializationError,
  RetryExhaustedError
} from './errors';

// Utility exports
export { EventEmitter } from './events';
export { Logger } from './logger';

// Version information
export const VERSION = '0.1.0';
export const AUTHOR = 'RustCI Team';
export const EMAIL = 'team@rustci.dev';