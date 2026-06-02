/**
 * Minimal client-side XDR inspection utilities.
 * Full XDR decoding is delegated to the Stellar SDK (stellar-sdk) at runtime.
 * This module provides a safe wrapper that works in SSR-safe environments.
 */

export interface ParsedXdrOperation {
  type: string;
  body: Record<string, unknown>;
}

export interface ParsedXdrTransaction {
  sourceAccount: string;
  fee: number;
  seqNum: string;
  operations: ParsedXdrOperation[];
  memo: string;
  timeBoundsMinLedger?: number;
  timeBoundsMaxLedger?: number;
}

/**
 * Parse a base64-encoded Stellar TransactionEnvelope XDR into a
 * human-readable structure using the browser-available stellar-sdk.
 *
 * Falls back to a "raw" representation when stellar-sdk is unavailable.
 */
export async function parseXdr(xdrBase64: string): Promise<ParsedXdrTransaction> {
  try {
    // Dynamic import — stellar-sdk ships a browser bundle
    const { TransactionBuilder, Networks } = await import('@stellar/stellar-sdk');
    const tx = TransactionBuilder.fromXDR(xdrBase64, Networks.PUBLIC);

    return {
      sourceAccount: tx.source,
      fee: parseInt(tx.fee, 10),
      seqNum: (tx as any).sequence ?? '',
      memo: tx.memo?.value?.toString() ?? tx.memo?.type ?? 'none',
      operations: (tx.operations ?? []).map(op => ({
        type: op.type,
        body: opToRecord(op),
      })),
    };
  } catch {
    // Graceful fallback — show raw base64 length
    return {
      sourceAccount: 'unknown',
      fee: 0,
      seqNum: '?',
      memo: 'none',
      operations: [{ type: 'raw', body: { xdr: xdrBase64.slice(0, 64) + '…' } }],
    };
  }
}

function opToRecord(op: Record<string, unknown>): Record<string, unknown> {
  const { type: _type, ...rest } = op;
  // Serialize BigInt / asset objects for display
  return JSON.parse(JSON.stringify(rest, (_key, val) =>
    typeof val === 'bigint' ? val.toString() : val
  ));
}

/**
 * Calculate the SHA-256 digest of the raw XDR bytes.
 * Used to confirm identity before signing.
 */
export async function xdrDigest(xdrBase64: string): Promise<string> {
  const bytes = Uint8Array.from(atob(xdrBase64), c => c.charCodeAt(0));
  const buf = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(buf)).map(b => b.toString(16).padStart(2, '0')).join('');
}
