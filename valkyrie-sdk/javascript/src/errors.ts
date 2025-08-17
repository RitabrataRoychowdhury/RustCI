/**
 * Valkyrie Protocol Error Types
 */

export class ValkyrieError extends Error {
  public readonly code?: string;
  public readonly details?: any;

  constructor(message: string, code?: string, details?: any) {
    super(message);
    this.name = 'ValkyrieError';
    this.code = code;
    this.details = details;
    
    // Maintain proper stack trace for where our error was thrown (only available on V8)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, ValkyrieError);
    }
  }

  toString(): string {
    if (this.code) {
      return `[${this.code}] ${this.message}`;
    }
    return this.message;
  }
}

export class ConnectionError extends ValkyrieError {
  public readonly endpoint?: string;

  constructor(message: string, endpoint?: string, details?: any) {
    super(message, 'CONNECTION_ERROR', details);
    this.name = 'ConnectionError';
    this.endpoint = endpoint;
  }
}

export class TimeoutError extends ValkyrieError {
  public readonly timeoutMs?: number;

  constructor(message: string, timeoutMs?: number, details?: any) {
    super(message, 'TIMEOUT_ERROR', details);
    this.name = 'TimeoutError';
    this.timeoutMs = timeoutMs;
  }
}

export class ProtocolError extends ValkyrieError {
  public readonly protocolCode?: string;

  constructor(message: string, protocolCode?: string, details?: any) {
    super(message, 'PROTOCOL_ERROR', details);
    this.name = 'ProtocolError';
    this.protocolCode = protocolCode;
  }
}

export class ConfigurationError extends ValkyrieError {
  public readonly field?: string;

  constructor(message: string, field?: string, details?: any) {
    super(message, 'CONFIGURATION_ERROR', details);
    this.name = 'ConfigurationError';
    this.field = field;
  }
}

export class AuthenticationError extends ValkyrieError {
  public readonly authMethod?: string;

  constructor(message: string, authMethod?: string, details?: any) {
    super(message, 'AUTHENTICATION_ERROR', details);
    this.name = 'AuthenticationError';
    this.authMethod = authMethod;
  }
}

export class SerializationError extends ValkyrieError {
  public readonly dataType?: string;

  constructor(message: string, dataType?: string, details?: any) {
    super(message, 'SERIALIZATION_ERROR', details);
    this.name = 'SerializationError';
    this.dataType = dataType;
  }
}

export class RetryExhaustedError extends ValkyrieError {
  public readonly attempts: number;
  public readonly lastError?: Error;

  constructor(message: string, attempts: number, lastError?: Error) {
    super(message, 'RETRY_EXHAUSTED', { attempts, lastError: lastError?.message });
    this.name = 'RetryExhaustedError';
    this.attempts = attempts;
    this.lastError = lastError;
  }
}