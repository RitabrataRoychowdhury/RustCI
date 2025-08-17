/**
 * Valkyrie Protocol Message Types
 */

import { v4 as uuidv4 } from 'uuid';

export enum MessageType {
  REQUEST = 'request',
  RESPONSE = 'response',
  NOTIFICATION = 'notification',
  ERROR = 'error'
}

export interface MessageData {
  id: string;
  messageType: MessageType;
  payload: any;
  correlationId?: string;
  endpoint?: string;
  timestamp: number;
  headers: Record<string, string>;
  priority: number;
  ttlMs?: number;
}

export class Message {
  public readonly id: string;
  public readonly messageType: MessageType;
  public readonly payload: any;
  public readonly correlationId?: string;
  public readonly endpoint?: string;
  public readonly timestamp: number;
  public readonly headers: Record<string, string>;
  public readonly priority: number;
  public readonly ttlMs?: number;

  constructor(data: Partial<MessageData> & { messageType: MessageType; payload: any }) {
    this.id = data.id || uuidv4();
    this.messageType = data.messageType;
    this.payload = data.payload;
    this.correlationId = data.correlationId;
    this.endpoint = data.endpoint;
    this.timestamp = data.timestamp || Date.now();
    this.headers = data.headers || {};
    this.priority = data.priority || 0;
    this.ttlMs = data.ttlMs;
  }

  static request(
    payload: any,
    endpoint?: string,
    headers?: Record<string, string>,
    priority: number = 0,
    ttlMs?: number
  ): Message {
    return new Message({
      messageType: MessageType.REQUEST,
      payload,
      endpoint,
      headers,
      priority,
      ttlMs
    });
  }

  static response(payload: any, correlationId: string, headers?: Record<string, string>): Message {
    return new Message({
      messageType: MessageType.RESPONSE,
      payload,
      correlationId,
      headers
    });
  }

  static notification(
    payload: any,
    endpoint?: string,
    headers?: Record<string, string>,
    priority: number = 0
  ): Message {
    return new Message({
      messageType: MessageType.NOTIFICATION,
      payload,
      endpoint,
      headers,
      priority
    });
  }

  static error(
    errorMessage: string,
    correlationId: string,
    errorCode?: string,
    headers?: Record<string, string>
  ): Message {
    const errorPayload = {
      error: errorMessage,
      code: errorCode,
      timestamp: Date.now()
    };

    return new Message({
      messageType: MessageType.ERROR,
      payload: errorPayload,
      correlationId,
      headers
    });
  }

  payloadAsString(): string | null {
    if (typeof this.payload === 'string') {
      return this.payload;
    }
    
    if (this.payload instanceof Buffer) {
      try {
        return this.payload.toString('utf-8');
      } catch {
        return null;
      }
    }
    
    if (typeof this.payload === 'object') {
      try {
        return JSON.stringify(this.payload);
      } catch {
        return null;
      }
    }
    
    return String(this.payload);
  }

  payloadAsJson<T = any>(): T | null {
    try {
      if (typeof this.payload === 'object' && this.payload !== null) {
        return this.payload as T;
      }
      
      const stringPayload = this.payloadAsString();
      if (stringPayload) {
        return JSON.parse(stringPayload) as T;
      }
      
      return null;
    } catch {
      return null;
    }
  }

  isExpired(): boolean {
    if (!this.ttlMs) {
      return false;
    }

    const elapsedMs = Date.now() - this.timestamp;
    return elapsedMs > this.ttlMs;
  }

  toJSON(): MessageData {
    return {
      id: this.id,
      messageType: this.messageType,
      payload: this.payload,
      correlationId: this.correlationId,
      endpoint: this.endpoint,
      timestamp: this.timestamp,
      headers: this.headers,
      priority: this.priority,
      ttlMs: this.ttlMs
    };
  }

  static fromJSON(data: MessageData): Message {
    return new Message(data);
  }

  toString(): string {
    return `Message(id=${this.id}, type=${this.messageType}, endpoint=${this.endpoint})`;
  }
}