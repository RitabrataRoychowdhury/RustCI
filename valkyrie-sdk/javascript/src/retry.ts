/**
 * Retry Policy Implementation
 */

import { RetryExhaustedError } from './errors';

export interface RetryPolicy {
  maxAttempts: number;
  initialDelayMs: number;
  maxDelayMs: number;
  backoffMultiplier: number;
  jitter: boolean;
  retryableErrors?: (new (...args: any[]) => Error)[];
}

export class RetryableOperation {
  constructor(private policy: RetryPolicy) {}

  async execute<T>(operation: () => Promise<T>): Promise<T> {
    let lastError: Error | undefined;

    for (let attempt = 0; attempt < this.policy.maxAttempts; attempt++) {
      try {
        // Apply delay for retry attempts
        if (attempt > 0) {
          const delay = this.calculateDelay(attempt);
          await this.sleep(delay);
        }

        // Execute the operation
        return await operation();
      } catch (error) {
        lastError = error as Error;

        // Check if we should retry this error
        if (!this.isRetryable(error as Error)) {
          throw error;
        }

        // If this was the last attempt, throw RetryExhaustedError
        if (attempt === this.policy.maxAttempts - 1) {
          throw new RetryExhaustedError(
            `Operation failed after ${this.policy.maxAttempts} attempts`,
            this.policy.maxAttempts,
            lastError
          );
        }
      }
    }

    // This should never be reached, but just in case
    throw new RetryExhaustedError(
      `Operation failed after ${this.policy.maxAttempts} attempts`,
      this.policy.maxAttempts,
      lastError
    );
  }

  private isRetryable(error: Error): boolean {
    if (!this.policy.retryableErrors || this.policy.retryableErrors.length === 0) {
      return true; // Retry all errors by default
    }

    return this.policy.retryableErrors.some(ErrorClass => error instanceof ErrorClass);
  }

  private calculateDelay(attempt: number): number {
    // Exponential backoff
    let delayMs = Math.min(
      this.policy.initialDelayMs * Math.pow(this.policy.backoffMultiplier, attempt - 1),
      this.policy.maxDelayMs
    );

    // Add jitter to prevent thundering herd
    if (this.policy.jitter) {
      const jitterRange = delayMs * 0.1; // 10% jitter
      delayMs += (Math.random() - 0.5) * 2 * jitterRange;
    }

    return Math.max(0, delayMs);
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

export function withRetry<T extends (...args: any[]) => Promise<any>>(
  policy: RetryPolicy
): (target: any, propertyKey: string, descriptor: TypedPropertyDescriptor<T>) => void {
  return function (target: any, propertyKey: string, descriptor: TypedPropertyDescriptor<T>) {
    const originalMethod = descriptor.value;
    if (!originalMethod) return;

    descriptor.value = async function (this: any, ...args: any[]) {
      const retryable = new RetryableOperation(policy);
      return retryable.execute(() => originalMethod.apply(this, args));
    } as T;
  };
}

export class RetryPolicies {
  static noRetry(): RetryPolicy {
    return {
      maxAttempts: 1,
      initialDelayMs: 0,
      maxDelayMs: 0,
      backoffMultiplier: 1,
      jitter: false
    };
  }

  static quickRetry(): RetryPolicy {
    return {
      maxAttempts: 3,
      initialDelayMs: 50,
      maxDelayMs: 500,
      backoffMultiplier: 1.5,
      jitter: true
    };
  }

  static standardRetry(): RetryPolicy {
    return {
      maxAttempts: 3,
      initialDelayMs: 100,
      maxDelayMs: 5000,
      backoffMultiplier: 2.0,
      jitter: true
    };
  }

  static aggressiveRetry(): RetryPolicy {
    return {
      maxAttempts: 5,
      initialDelayMs: 100,
      maxDelayMs: 10000,
      backoffMultiplier: 2.0,
      jitter: true
    };
  }

  static connectionRetry(): RetryPolicy {
    return {
      maxAttempts: 5,
      initialDelayMs: 200,
      maxDelayMs: 5000,
      backoffMultiplier: 1.8,
      jitter: true,
      retryableErrors: [Error] // Will be replaced with specific connection errors
    };
  }
}