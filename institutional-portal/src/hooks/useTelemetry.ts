'use client';

import { useCallback, useRef } from 'react';
import { TelemetryEvent } from '@/types';

/** Push an event to the telemetry pipeline (OpenTelemetry OTLP or internal endpoint). */
async function flushEvent(event: TelemetryEvent) {
  if (typeof window === 'undefined') return;
  try {
    await fetch('/api/telemetry', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(event),
      keepalive: true,
    });
  } catch {
    // Telemetry must never break the UI
  }
}

export function useTelemetry() {
  const pendingRef = useRef<TelemetryEvent[]>([]);

  /** Record multisig approval latency from proposal creation to threshold. */
  const recordApprovalLatency = useCallback((proposalId: string, latencyMs: number) => {
    flushEvent({
      name: 'multisig_approval_latency',
      value: latencyMs / 1000, // convert to seconds for prometheus compat
      labels: { proposal_id: proposalId },
      ts: Date.now(),
    });
  }, []);

  /** Increment portal_access_denied_total when RBAC gate blocks a user. */
  const recordAccessDenied = useCallback((permission: string, role: string) => {
    flushEvent({
      name: 'portal_access_denied',
      labels: { permission, role },
      ts: Date.now(),
    });
  }, []);

  /** Record a partial signature submission event. */
  const recordPartialSignature = useCallback((proposalId: string, signerKey: string) => {
    flushEvent({
      name: 'partial_signature_submitted',
      labels: { proposal_id: proposalId, signer_key: signerKey.slice(0, 8) },
      ts: Date.now(),
    });
  }, []);

  return { recordApprovalLatency, recordAccessDenied, recordPartialSignature };
}
