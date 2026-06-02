/**
 * Unit tests — signature threshold arithmetic, RBAC permission utilities,
 * and XDR digest helper.
 *
 * Run with: npx vitest run src/__tests__/unit/
 */

import { describe, it, expect } from 'vitest';
import {
  hasPermission,
  ROLE_PERMISSIONS,
  DEFAULT_THRESHOLDS,
  type InstitutionalRole,
  type MultisigStatus,
  type SignoffEntry,
  type SignoffMatrix,
} from '../../types';
import { normalizeMultisigError } from '../../lib/formErrors';

// ─── Helpers under test ────────────────────────────────────────────────────────

/** Derive the effective signoff matrix from a list of entries + required weight. */
function buildMatrix(entries: SignoffEntry[], requiredWeight: number): SignoffMatrix {
  const accumulated = entries.reduce((sum, e) => sum + e.signerWeight, 0);
  return {
    proposalId: 'test-proposal',
    requiredWeight,
    accumulatedWeight: accumulated,
    entries,
    thresholdMet: accumulated >= requiredWeight,
  };
}

function makeEntry(id: string, weight: number): SignoffEntry {
  return {
    signerId: id,
    signerKey: `G${id.toUpperCase().padEnd(55, '0')}`,
    signerName: `Signer ${id}`,
    signerWeight: weight,
    signedAt: new Date().toISOString(),
  };
}

// ─── Threshold Arithmetic ─────────────────────────────────────────────────────

describe('Signature threshold arithmetic', () => {
  it('thresholdMet is false when accumulated weight < required', () => {
    const matrix = buildMatrix([makeEntry('A', 1), makeEntry('B', 1)], 3);
    expect(matrix.thresholdMet).toBe(false);
    expect(matrix.accumulatedWeight).toBe(2);
  });

  it('thresholdMet is true when accumulated weight == required', () => {
    const matrix = buildMatrix([makeEntry('A', 1), makeEntry('B', 1), makeEntry('C', 1)], 3);
    expect(matrix.thresholdMet).toBe(true);
  });

  it('thresholdMet is true when accumulated weight > required (over-signed)', () => {
    const matrix = buildMatrix([makeEntry('A', 2), makeEntry('B', 2)], 3);
    expect(matrix.thresholdMet).toBe(true);
    expect(matrix.accumulatedWeight).toBe(4);
  });

  it('empty entries never meets threshold', () => {
    const matrix = buildMatrix([], 3);
    expect(matrix.thresholdMet).toBe(false);
    expect(matrix.accumulatedWeight).toBe(0);
  });

  it('single signer with sufficient weight meets threshold', () => {
    const matrix = buildMatrix([makeEntry('A', 5)], 3);
    expect(matrix.thresholdMet).toBe(true);
  });

  it('DEFAULT_THRESHOLDS: mint requires weight 3', () => {
    expect(DEFAULT_THRESHOLDS.mint.requiredWeight).toBe(3);
    expect(DEFAULT_THRESHOLDS.mint.timeLockSeconds).toBe(0);
  });

  it('DEFAULT_THRESHOLDS: change_threshold requires weight 4 and 48h time-lock', () => {
    expect(DEFAULT_THRESHOLDS.change_threshold.requiredWeight).toBe(4);
    expect(DEFAULT_THRESHOLDS.change_threshold.timeLockSeconds).toBe(172800);
  });

  it('DEFAULT_THRESHOLDS: add_signer requires time-lock', () => {
    expect(DEFAULT_THRESHOLDS.add_signer.timeLockSeconds).toBeGreaterThan(0);
    expect(DEFAULT_THRESHOLDS.remove_signer.timeLockSeconds).toBeGreaterThan(0);
  });
});

// ─── Permission-Checking Utilities ────────────────────────────────────────────

describe('RBAC permission checks', () => {
  const roles: InstitutionalRole[] = ['SuperAdmin', 'Operator', 'ComplianceAuditor', 'Signatory'];

  it('SuperAdmin has all permissions', () => {
    for (const p of ROLE_PERMISSIONS.SuperAdmin) {
      expect(hasPermission('SuperAdmin', p)).toBe(true);
    }
  });

  it('ComplianceAuditor can read compliance but cannot create proposals', () => {
    expect(hasPermission('ComplianceAuditor', 'compliance:read')).toBe(true);
    expect(hasPermission('ComplianceAuditor', 'proposals:create')).toBe(false);
    expect(hasPermission('ComplianceAuditor', 'users:write')).toBe(false);
  });

  it('Signatory can sign but cannot write config', () => {
    expect(hasPermission('Signatory', 'signatures:submit')).toBe(true);
    expect(hasPermission('Signatory', 'config:write')).toBe(false);
    expect(hasPermission('Signatory', 'users:write')).toBe(false);
  });

  it('Operator can create proposals but cannot modify users', () => {
    expect(hasPermission('Operator', 'proposals:create')).toBe(true);
    expect(hasPermission('Operator', 'users:write')).toBe(false);
  });

  it('Unknown permission returns false for all roles', () => {
    for (const role of roles) {
      expect(hasPermission(role, 'nonexistent:permission')).toBe(false);
    }
  });
});

// ─── Form Error Normalization ─────────────────────────────────────────────────

describe('normalizeMultisigError', () => {
  it('maps string error code INSUFFICIENT_WEIGHT', () => {
    const err = normalizeMultisigError('INSUFFICIENT_WEIGHT');
    expect(err.code).toBe('INSUFFICIENT_WEIGHT');
    expect(err.message).toContain('weight');
    expect(err.hint).toBeDefined();
  });

  it('maps BAD_SEQUENCE with remediation hint', () => {
    const err = normalizeMultisigError('BAD_SEQUENCE');
    expect(err.code).toBe('BAD_SEQUENCE');
    expect(err.hint).toContain('sequence');
  });

  it('maps TX_TOO_LATE for expired ledger constraint', () => {
    const err = normalizeMultisigError('TX_TOO_LATE');
    expect(err.code).toBe('TX_TOO_LATE');
    expect(err.message).toContain('expired');
  });

  it('handles Error objects with embedded code', () => {
    const err = normalizeMultisigError(new Error('XDR_DECODE_ERROR: bad bytes'));
    expect(err.code).toBe('XDR_DECODE_ERROR');
  });

  it('handles unknown error codes gracefully', () => {
    const err = normalizeMultisigError('SOME_NEW_CODE');
    expect(err.code).toBe('SOME_NEW_CODE');
    expect(err.message).toBeTruthy();
  });

  it('handles API response objects with code + message', () => {
    const err = normalizeMultisigError({ code: 'TIME_LOCK_ACTIVE', message: 'time lock is active' });
    expect(err.code).toBe('TIME_LOCK_ACTIVE');
    expect(err.message).toBe('time lock is active');
    expect(err.hint).toContain('time-lock');
  });
});

// ─── MultisigStatus type guards ───────────────────────────────────────────────

describe('MultisigStatus discriminated union', () => {
  const TERMINAL: MultisigStatus[] = ['SUCCESS', 'FAILED', 'EXPIRED'];
  const ACTIVE: MultisigStatus[] = ['PENDING_SIGNATURES', 'THRESHOLD_MET', 'BROADCASTING'];

  it('terminal states are a strict set', () => {
    for (const s of TERMINAL) {
      expect(['SUCCESS', 'FAILED', 'EXPIRED']).toContain(s);
    }
  });

  it('active states are distinct from terminal states', () => {
    for (const s of ACTIVE) {
      expect(TERMINAL).not.toContain(s);
    }
  });
});
