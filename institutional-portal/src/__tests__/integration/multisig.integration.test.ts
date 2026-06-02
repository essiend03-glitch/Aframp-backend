/**
 * Integration tests — three-signatory cNGN transfer workflow.
 *
 * Simulates:
 *   1. Three independent signatories logging in sequentially
 *   2. Reviewing a high-volume cNGN transfer request
 *   3. Each approving with independent simulated keys
 *   4. System aggregating state changes up to final on-chain submission readiness
 *
 * Run with: npx vitest run src/__tests__/integration/
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  MultisigEnvelope,
  MultisigStatus,
  SignoffEntry,
  DEFAULT_THRESHOLDS,
} from '../../types';
import { normalizeMultisigError } from '../../lib/formErrors';

// ─── Mock XDR ─────────────────────────────────────────────────────────────────

const MOCK_XDR_B64 =
  'AAAAAgAAAABiZnVtcGxhY2Vob2xkZXIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' +
  'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==';

// ─── Mock signatory pool ──────────────────────────────────────────────────────

interface MockSignatory {
  id: string;
  name: string;
  key: string;
  weight: number;
}

const SIGNATORIES: MockSignatory[] = [
  { id: 'sig-1', name: 'Alice Treasury', key: 'GALICE000000000000000000000000000000000000000000000000000', weight: 1 },
  { id: 'sig-2', name: 'Bob Compliance', key: 'GBOB0000000000000000000000000000000000000000000000000000000', weight: 1 },
  { id: 'sig-3', name: 'Carol Operations', key: 'GCAROL00000000000000000000000000000000000000000000000000000', weight: 1 },
];

// ─── In-memory multisig coordinator ──────────────────────────────────────────

interface CoordinatorState {
  envelope: MultisigEnvelope;
  signatures: SignoffEntry[];
}

function createEnvelope(): MultisigEnvelope {
  const threshold = DEFAULT_THRESHOLDS.mint;
  return {
    id: 'proposal-cnGN-001',
    opType: 'mint',
    description: 'High-volume cNGN transfer: 5,000,000 NGN',
    unsignedXdr: MOCK_XDR_B64,
    signedXdr: null,
    stellarTxHash: null,
    requiredSignatures: 3,
    totalSigners: 3,
    signoffMatrix: {
      proposalId: 'proposal-cnGN-001',
      requiredWeight: threshold.requiredWeight,
      accumulatedWeight: 0,
      entries: [],
      thresholdMet: false,
    },
    timeLockUntil: null,
    timeLockRemainingSeconds: null,
    status: 'PENDING_SIGNATURES',
    failureReason: null,
    proposedBy: 'operator-001',
    proposedByKey: 'GOPERATOR0000000000000000000000000000000000000000000000000',
    expiresAt: new Date(Date.now() + 86_400_000).toISOString(),
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}

/** Simulates the backend coordinator that aggregates signatures. */
class MultisigCoordinator {
  state: CoordinatorState;

  constructor() {
    this.state = { envelope: createEnvelope(), signatures: [] };
  }

  submitSignature(signatory: MockSignatory, signedXdr: string): { ok: boolean; error?: string } {
    const { envelope, signatures } = this.state;

    if (envelope.status === 'SUCCESS' || envelope.status === 'EXPIRED') {
      return { ok: false, error: 'INVALID_STATE' };
    }
    if (signatures.some(s => s.signerId === signatory.id)) {
      return { ok: false, error: 'DUPLICATE_SIGNATURE' };
    }

    const entry: SignoffEntry = {
      signerId: signatory.id,
      signerKey: signatory.key,
      signerName: signatory.name,
      signerWeight: signatory.weight,
      signedAt: new Date().toISOString(),
    };

    signatures.push(entry);

    const accumulated = signatures.reduce((sum, s) => sum + s.signerWeight, 0);
    const thresholdMet = accumulated >= envelope.signoffMatrix.requiredWeight;

    this.state.envelope = {
      ...envelope,
      signedXdr: signedXdr,
      updatedAt: new Date().toISOString(),
      status: thresholdMet ? 'THRESHOLD_MET' : 'PENDING_SIGNATURES',
      signoffMatrix: {
        ...envelope.signoffMatrix,
        accumulatedWeight: accumulated,
        entries: [...signatures],
        thresholdMet,
      },
    };

    return { ok: true };
  }

  broadcast(): { ok: boolean; txHash?: string; error?: string } {
    if (this.state.envelope.status !== 'THRESHOLD_MET') {
      return { ok: false, error: 'INSUFFICIENT_WEIGHT' };
    }
    const txHash = 'abc123def456' + Math.random().toString(36).slice(2);
    this.state.envelope = {
      ...this.state.envelope,
      status: 'SUCCESS',
      stellarTxHash: txHash,
    };
    return { ok: true, txHash };
  }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe('Three-signatory cNGN transfer integration', () => {
  let coordinator: MultisigCoordinator;

  beforeEach(() => {
    coordinator = new MultisigCoordinator();
  });

  it('creates proposal in PENDING_SIGNATURES state', () => {
    const { envelope } = coordinator.state;
    expect(envelope.status).toBe<MultisigStatus>('PENDING_SIGNATURES');
    expect(envelope.signoffMatrix.accumulatedWeight).toBe(0);
    expect(envelope.signoffMatrix.thresholdMet).toBe(false);
  });

  it('rejects broadcast before threshold is met', () => {
    const result = coordinator.broadcast();
    expect(result.ok).toBe(false);
    const err = normalizeMultisigError(result.error!);
    expect(err.code).toBe('INSUFFICIENT_WEIGHT');
  });

  it('first signatory submits — still PENDING_SIGNATURES', () => {
    const result = coordinator.submitSignature(SIGNATORIES[0], 'signed-xdr-1');
    expect(result.ok).toBe(true);

    const { envelope } = coordinator.state;
    expect(envelope.status).toBe<MultisigStatus>('PENDING_SIGNATURES');
    expect(envelope.signoffMatrix.entries).toHaveLength(1);
    expect(envelope.signoffMatrix.accumulatedWeight).toBe(1);
    expect(envelope.signoffMatrix.thresholdMet).toBe(false);
  });

  it('second signatory submits — still PENDING_SIGNATURES (need weight=3)', () => {
    coordinator.submitSignature(SIGNATORIES[0], 'signed-xdr-1');
    coordinator.submitSignature(SIGNATORIES[1], 'signed-xdr-2');

    const { envelope } = coordinator.state;
    expect(envelope.status).toBe<MultisigStatus>('PENDING_SIGNATURES');
    expect(envelope.signoffMatrix.accumulatedWeight).toBe(2);
    expect(envelope.signoffMatrix.thresholdMet).toBe(false);
  });

  it('third signatory submits — transitions to THRESHOLD_MET', () => {
    for (const [idx, sig] of SIGNATORIES.entries()) {
      coordinator.submitSignature(sig, `signed-xdr-${idx}`);
    }

    const { envelope } = coordinator.state;
    expect(envelope.status).toBe<MultisigStatus>('THRESHOLD_MET');
    expect(envelope.signoffMatrix.accumulatedWeight).toBe(3);
    expect(envelope.signoffMatrix.thresholdMet).toBe(true);
    expect(envelope.signoffMatrix.entries).toHaveLength(3);
  });

  it('duplicate signature is rejected', () => {
    coordinator.submitSignature(SIGNATORIES[0], 'signed-xdr-1');
    const result = coordinator.submitSignature(SIGNATORIES[0], 'signed-xdr-1-dupe');

    expect(result.ok).toBe(false);
    const err = normalizeMultisigError(result.error!);
    expect(err.code).toBe('DUPLICATE_SIGNATURE');
  });

  it('broadcast succeeds after all three signatures — reaches SUCCESS', () => {
    for (const [idx, sig] of SIGNATORIES.entries()) {
      coordinator.submitSignature(sig, `signed-xdr-${idx}`);
    }
    const result = coordinator.broadcast();

    expect(result.ok).toBe(true);
    expect(result.txHash).toBeTruthy();
    expect(coordinator.state.envelope.status).toBe<MultisigStatus>('SUCCESS');
    expect(coordinator.state.envelope.stellarTxHash).toBe(result.txHash);
  });

  it('full workflow aggregates state transitions correctly', () => {
    const states: MultisigStatus[] = [];

    // Initial state
    states.push(coordinator.state.envelope.status);

    // Three sequential signatories
    for (const [idx, sig] of SIGNATORIES.entries()) {
      coordinator.submitSignature(sig, `signed-xdr-${idx}`);
      states.push(coordinator.state.envelope.status);
    }

    // Broadcast
    coordinator.broadcast();
    states.push(coordinator.state.envelope.status);

    expect(states).toEqual([
      'PENDING_SIGNATURES',
      'PENDING_SIGNATURES', // after sig 1
      'PENDING_SIGNATURES', // after sig 2
      'THRESHOLD_MET',      // after sig 3
      'SUCCESS',
    ]);
  });

  it('reviews XDR description before signing (non-custodial check)', () => {
    const { envelope } = coordinator.state;
    expect(envelope.description).toContain('cNGN');
    expect(envelope.unsignedXdr).toBeTruthy();
    expect(typeof envelope.unsignedXdr).toBe('string');
  });
});

// ─── Concurrent duplicate prevention ─────────────────────────────────────────

describe('Concurrent signature guards', () => {
  it('prevents same signer from submitting while threshold is already met', () => {
    const coord = new MultisigCoordinator();
    for (const [idx, sig] of SIGNATORIES.entries()) {
      coord.submitSignature(sig, `signed-xdr-${idx}`);
    }
    // Try a 4th from same signer as first
    const result = coord.submitSignature(SIGNATORIES[0], 'signed-xdr-extra');
    expect(result.ok).toBe(false);
  });
});
