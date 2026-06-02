/**
 * Authentication Telemetry
 * Anonymized state tracking and performance monitoring
 */

import type { AuthTelemetryEvent } from '@/types/auth';

// ============================================================================
// Telemetry Configuration
// ============================================================================

const TELEMETRY_ENABLED = process.env.NEXT_PUBLIC_TELEMETRY_ENABLED === 'true';
const TELEMETRY_ENDPOINT = process.env.NEXT_PUBLIC_TELEMETRY_ENDPOINT || '/api/telemetry';

// ============================================================================
// Anonymization Utilities
// ============================================================================

/**
 * Generate anonymized user ID from actual user ID
 */
function anonymizeUserId(userId: string): string {
  // Simple hash for anonymization (in production, use a proper hashing algorithm)
  let hash = 0;
  for (let i = 0; i < userId.length; i++) {
    const char = userId.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash;
  }
  return `anon_${Math.abs(hash).toString(36)}`;
}

// ============================================================================
// Telemetry Emitter
// ============================================================================

/**
 * Emit authentication telemetry event
 */
export function emitAuthTelemetry(event: Omit<AuthTelemetryEvent, 'anonymizedUserId'>): void {
  if (!TELEMETRY_ENABLED) return;

  try {
    const telemetryEvent: AuthTelemetryEvent = {
      ...event,
      timestamp: event.timestamp || Date.now(),
    };

    // Send to telemetry endpoint (non-blocking)
    sendTelemetry(telemetryEvent);

    // Log to console in development
    if (process.env.NODE_ENV === 'development') {
      console.log('[Auth Telemetry]', telemetryEvent);
    }
  } catch (error) {
    console.error('Failed to emit telemetry:', error);
  }
}

/**
 * Send telemetry data to backend
 */
async function sendTelemetry(event: AuthTelemetryEvent): Promise<void> {
  try {
    await fetch(TELEMETRY_ENDPOINT, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(event),
      // Use keepalive to ensure delivery even if page is closing
      keepalive: true,
    });
  } catch (error) {
    // Silently fail - telemetry should not impact user experience
    console.debug('Telemetry send failed:', error);
  }
}

// ============================================================================
// Performance Tracking
// ============================================================================

/**
 * Track auth operation performance
 */
export function trackAuthPerformance(
  operation: string,
  startTime: number,
  metadata?: Record<string, unknown>
): void {
  const duration = Date.now() - startTime;

  emitAuthTelemetry({
    eventType: 'TOKEN_REFRESH',
    timestamp: Date.now(),
    duration,
    metadata: {
      operation,
      ...metadata,
    },
  });
}

// ============================================================================
// Session Timeout Warning
// ============================================================================

/**
 * Calculate time until session expires
 */
export function getTimeUntilExpiry(expiresAt: number): number {
  return Math.max(0, expiresAt - Date.now());
}

/**
 * Check if session is about to expire (within 5 minutes)
 */
export function isSessionExpiringSoon(expiresAt: number): boolean {
  const timeUntilExpiry = getTimeUntilExpiry(expiresAt);
  const fiveMinutes = 5 * 60 * 1000;
  return timeUntilExpiry > 0 && timeUntilExpiry <= fiveMinutes;
}
