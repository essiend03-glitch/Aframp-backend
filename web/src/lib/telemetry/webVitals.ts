/**
 * Web Vitals Performance Tracking
 * Captures and reports Core Web Vitals to backend
 */

import { onCLS, onFID, onLCP, onFCP, onTTFB, type Metric } from 'web-vitals';
import { apiClient } from '@/lib/api/client';

interface VitalsPayload {
  name: string;
  value: number;
  rating: 'good' | 'needs-improvement' | 'poor';
  delta: number;
  id: string;
  pathname: string;
  userAgent: string;
  timestamp: string;
}

// ============================================================================
// Vitals Reporting
// ============================================================================

function sendToAnalytics(metric: Metric): void {
  const payload: VitalsPayload = {
    name: metric.name,
    value: metric.value,
    rating: metric.rating,
    delta: metric.delta,
    id: metric.id,
    pathname: window.location.pathname,
    userAgent: navigator.userAgent,
    timestamp: new Date().toISOString(),
  };

  // Send to backend (non-blocking)
  apiClient
    .post('/api/v1/admin/infra/profile/capture', payload)
    .catch((error) => {
      console.error('Failed to send web vitals:', error);
    });

  // Also log in development
  if (process.env.NODE_ENV === 'development') {
    console.log('[Web Vitals]', payload);
  }
}

// ============================================================================
// Initialize Tracking
// ============================================================================

export function initWebVitals(): void {
  if (typeof window === 'undefined') return;

  onCLS(sendToAnalytics);
  onFID(sendToAnalytics);
  onLCP(sendToAnalytics);
  onFCP(sendToAnalytics);
  onTTFB(sendToAnalytics);
}

// ============================================================================
// Custom Performance Marks
// ============================================================================

export function markPerformance(name: string): void {
  if (typeof window === 'undefined' || !window.performance) return;

  try {
    window.performance.mark(name);
  } catch (error) {
    console.error('Failed to mark performance:', error);
  }
}

export function measurePerformance(name: string, startMark: string, endMark?: string): void {
  if (typeof window === 'undefined' || !window.performance) return;

  try {
    const measure = window.performance.measure(name, startMark, endMark);
    
    // Report custom measurement
    apiClient
      .post('/api/v1/admin/infra/profile/capture', {
        name,
        value: measure.duration,
        type: 'custom',
        pathname: window.location.pathname,
        timestamp: new Date().toISOString(),
      })
      .catch((error) => {
        console.error('Failed to send performance measure:', error);
      });
  } catch (error) {
    console.error('Failed to measure performance:', error);
  }
}
