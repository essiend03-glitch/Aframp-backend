/**
 * Client-Side Telemetry
 * User session tracking, routing paths, and payment flow analytics
 */

import { apiClient } from '@/lib/api/client';

// ============================================================================
// Event Types
// ============================================================================

type TrackingEvent =
  | 'page_view'
  | 'session_start'
  | 'session_end'
  | 'api_error'
  | 'payment_started'
  | 'payment_completed'
  | 'payment_failed'
  | 'payment_abandoned'
  | 'exchange_started'
  | 'exchange_completed'
  | 'exchange_abandoned'
  | 'kyc_started'
  | 'kyc_completed'
  | 'kyb_started'
  | 'kyb_completed';

interface TrackingPayload {
  event: TrackingEvent;
  properties: Record<string, unknown>;
  sessionId: string;
  pathname: string;
  timestamp: string;
}

// ============================================================================
// Session Management
// ============================================================================

let sessionId: string | null = null;

function getSessionId(): string {
  if (sessionId) return sessionId;

  // Try to get from sessionStorage
  if (typeof window !== 'undefined' && window.sessionStorage) {
    const stored = window.sessionStorage.getItem('aframp_session_id');
    if (stored) {
      sessionId = stored;
      return stored;
    }
  }

  // Generate new session ID
  sessionId = generateSessionId();
  
  if (typeof window !== 'undefined' && window.sessionStorage) {
    window.sessionStorage.setItem('aframp_session_id', sessionId);
  }

  return sessionId;
}

function generateSessionId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

// ============================================================================
// Tracking Functions
// ============================================================================

export function track(event: TrackingEvent, properties: Record<string, unknown> = {}): void {
  if (typeof window === 'undefined') return;

  const payload: TrackingPayload = {
    event,
    properties,
    sessionId: getSessionId(),
    pathname: window.location.pathname,
    timestamp: new Date().toISOString(),
  };

  // Send to backend (non-blocking)
  apiClient
    .post('/api/v1/admin/infra/telemetry/track', payload)
    .catch((error) => {
      console.error('Failed to track event:', error);
    });

  // Log in development
  if (process.env.NODE_ENV === 'development') {
    console.log('[Tracking]', payload);
  }
}

export function trackPageView(): void {
  track('page_view', {
    url: window.location.href,
    referrer: document.referrer,
  });
}

export function trackApiError(error: { code: string; message: string; endpoint: string }): void {
  track('api_error', error);
}

export function trackPaymentFlow(
  stage: 'started' | 'completed' | 'failed' | 'abandoned',
  properties: Record<string, unknown> = {}
): void {
  const eventMap: Record<typeof stage, TrackingEvent> = {
    started: 'payment_started',
    completed: 'payment_completed',
    failed: 'payment_failed',
    abandoned: 'payment_abandoned',
  };

  track(eventMap[stage], properties);
}

export function trackExchangeFlow(
  stage: 'started' | 'completed' | 'abandoned',
  properties: Record<string, unknown> = {}
): void {
  const eventMap: Record<typeof stage, TrackingEvent> = {
    started: 'exchange_started',
    completed: 'exchange_completed',
    abandoned: 'exchange_abandoned',
  };

  track(eventMap[stage], properties);
}

// ============================================================================
// Session Lifecycle
// ============================================================================

export function initTracking(): void {
  if (typeof window === 'undefined') return;

  // Track session start
  track('session_start', {
    userAgent: navigator.userAgent,
    screenResolution: `${window.screen.width}x${window.screen.height}`,
    viewport: `${window.innerWidth}x${window.innerHeight}`,
  });

  // Track page views on navigation
  let lastPathname = window.location.pathname;
  const observer = new MutationObserver(() => {
    if (window.location.pathname !== lastPathname) {
      lastPathname = window.location.pathname;
      trackPageView();
    }
  });

  observer.observe(document.querySelector('body')!, {
    childList: true,
    subtree: true,
  });

  // Track session end on page unload
  window.addEventListener('beforeunload', () => {
    track('session_end', {
      duration: Date.now() - Number.parseInt(getSessionId().split('-')[0] || '0'),
    });
  });

  // Initial page view
  trackPageView();
}
