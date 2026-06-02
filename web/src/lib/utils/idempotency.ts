/**
 * Idempotency Utilities
 * Prevent duplicate API calls and double-submission
 */

const pendingRequests = new Map<string, Promise<unknown>>();

/**
 * Ensures only one request with the same key is in flight at a time
 * Subsequent calls with the same key will return the same promise
 */
export async function idempotentRequest<T>(
  key: string,
  requestFn: () => Promise<T>
): Promise<T> {
  // Check if request is already in flight
  if (pendingRequests.has(key)) {
    return pendingRequests.get(key) as Promise<T>;
  }

  // Execute request and store promise
  const promise = requestFn()
    .finally(() => {
      // Clean up after request completes
      pendingRequests.delete(key);
    });

  pendingRequests.set(key, promise);
  return promise;
}

/**
 * Generate idempotency key for transaction submissions
 */
export function generateIdempotencyKey(
  userId: string,
  action: string,
  params: Record<string, unknown>
): string {
  const paramsStr = JSON.stringify(params, Object.keys(params).sort());
  return `${userId}:${action}:${paramsStr}`;
}

/**
 * Hook for preventing double-click submissions
 */
export function useIdempotentSubmit<T>(
  submitFn: () => Promise<T>
): [() => Promise<T>, boolean] {
  let isSubmitting = false;

  const idempotentSubmit = async (): Promise<T> => {
    if (isSubmitting) {
      throw new Error('Submission already in progress');
    }

    isSubmitting = true;
    try {
      return await submitFn();
    } finally {
      isSubmitting = false;
    }
  };

  return [idempotentSubmit, isSubmitting];
}
