/**
 * Event Emitter for Valkyrie Protocol
 */

import { EventEmitter as NodeEventEmitter } from 'eventemitter3';

export class EventEmitter extends NodeEventEmitter {
  constructor() {
    super();
  }

  // Type-safe event emission
  emitTyped<T>(event: string, data: T): boolean {
    return this.emit(event, data);
  }

  // Type-safe event listening
  onTyped<T>(event: string, listener: (data: T) => void): this {
    return this.on(event, listener);
  }

  // Type-safe one-time event listening
  onceTyped<T>(event: string, listener: (data: T) => void): this {
    return this.once(event, listener);
  }
}