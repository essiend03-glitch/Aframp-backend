// Issue #480 — Unit Tests: XDR parsing, wallet state mapping, error normalization

import { mapHorizonError } from '../hooks/useWalletConnection';

// ── Horizon error mapping ─────────────────────────────────────────────────────

describe('mapHorizonError', () => {
  it('maps tx_bad_seq to human-readable message', () => {
    const msg = mapHorizonError('tx_bad_seq');
    expect(msg).toContain('sequence number');
  });

  it('maps op_no_trust to trustline message', () => {
    const msg = mapHorizonError('op_no_trust');
    expect(msg).toContain('trustline');
  });

  it('maps op_low_reserve to reserve message', () => {
    const msg = mapHorizonError('op_low_reserve');
    expect(msg).toContain('XLM');
  });

  it('returns generic message for unknown codes', () => {
    const msg = mapHorizonError('unknown_error_xyz');
    expect(msg).toContain('unknown_error_xyz');
  });
});

// ── Wallet state machine ──────────────────────────────────────────────────────

type WalletState = 'DISCONNECTED' | 'CONNECTING' | 'CONNECTED' | 'SIGNING_REQUEST' | 'TX_SUBMITTING';

function canSign(state: WalletState): boolean {
  return state === 'CONNECTED';
}

function isLocked(state: WalletState): boolean {
  return state === 'SIGNING_REQUEST' || state === 'TX_SUBMITTING';
}

describe('wallet state machine', () => {
  it('allows signing only when CONNECTED', () => {
    expect(canSign('CONNECTED')).toBe(true);
    expect(canSign('DISCONNECTED')).toBe(false);
    expect(canSign('SIGNING_REQUEST')).toBe(false);
    expect(canSign('TX_SUBMITTING')).toBe(false);
  });

  it('locks transactional fields during signing/submitting', () => {
    expect(isLocked('SIGNING_REQUEST')).toBe(true);
    expect(isLocked('TX_SUBMITTING')).toBe(true);
    expect(isLocked('CONNECTED')).toBe(false);
    expect(isLocked('DISCONNECTED')).toBe(false);
  });
});

// ── XDR validation helpers ────────────────────────────────────────────────────

function isValidBase64XDR(xdr: string): boolean {
  if (!xdr || typeof xdr !== 'string') return false;
  // Base64 pattern check
  return /^[A-Za-z0-9+/]+=*$/.test(xdr) && xdr.length > 0;
}

describe('isValidBase64XDR', () => {
  it('accepts valid base64 strings', () => {
    expect(isValidBase64XDR('AAAAAQAAAA==')).toBe(true);
  });

  it('rejects empty strings', () => {
    expect(isValidBase64XDR('')).toBe(false);
  });

  it('rejects non-base64 characters', () => {
    expect(isValidBase64XDR('not-valid-xdr!')).toBe(false);
  });
});

// ── Public key validation ─────────────────────────────────────────────────────

function isValidStellarPublicKey(key: string): boolean {
  return /^G[A-Z2-7]{55}$/.test(key);
}

describe('isValidStellarPublicKey', () => {
  it('accepts valid Stellar public key', () => {
    expect(isValidStellarPublicKey('GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX')).toBe(true);
  });

  it('rejects keys not starting with G', () => {
    expect(isValidStellarPublicKey('SCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX')).toBe(false);
  });

  it('rejects short keys', () => {
    expect(isValidStellarPublicKey('GABC')).toBe(false);
  });
});
