// Issue #480 — Transaction Approval Modal
// Breaks down raw XDR into readable operation summaries before user signs.
// Enforces: no signing during ongoing TX, atomic multi-op validation.

'use client';

import React, { useEffect, useState } from 'react';
import type { XDRTransactionPayload } from '../../types';
import { useWalletConnection } from '../../hooks/useWalletConnection';

interface Props {
  payload: XDRTransactionPayload | null;
  onClose: () => void;
  onSuccess: (txHash: string) => void;
}

export default function TransactionApprovalModal({ payload, onClose, onSuccess }: Props) {
  const { ctx, signAndSubmit } = useWalletConnection();
  const [error, setError] = useState<string | null>(null);
  const isBusy = ctx.state === 'SIGNING_REQUEST' || ctx.state === 'TX_SUBMITTING';

  // Prevent background scroll when modal is open
  useEffect(() => {
    if (payload) document.body.style.overflow = 'hidden';
    return () => { document.body.style.overflow = ''; };
  }, [payload]);

  if (!payload) return null;

  const handleSign = async () => {
    if (isBusy) return; // Reject out-of-sequence actions
    setError(null);
    try {
      const hash = await signAndSubmit(payload.xdr, payload.networkPassphrase);
      onSuccess(hash);
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
      style={{
        position: 'fixed', inset: 0, zIndex: 1000,
        background: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: 12, padding: 28, maxWidth: 520, width: '90%', maxHeight: '80vh', overflowY: 'auto' }}>
        <h2 id="modal-title" style={{ fontSize: 16, fontWeight: 700, marginBottom: 16, color: '#c9d1d9' }}>
          Review Transaction
        </h2>

        {/* Source account */}
        <div style={rowStyle}>
          <span style={labelStyle}>Source Account</span>
          <span style={hashStyle} title={payload.sourceAccount}>
            {payload.sourceAccount.slice(0, 8)}…{payload.sourceAccount.slice(-6)}
          </span>
        </div>

        {/* Fee */}
        <div style={rowStyle}>
          <span style={labelStyle}>Network Fee</span>
          <span style={{ color: '#c9d1d9', fontSize: 13 }}>{payload.fee} stroops</span>
        </div>

        {/* Memo */}
        {payload.memo && (
          <div style={rowStyle}>
            <span style={labelStyle}>Memo</span>
            <span style={{ color: '#c9d1d9', fontSize: 13 }}>{payload.memo}</span>
          </div>
        )}

        {/* Operations */}
        <div style={{ marginTop: 16, marginBottom: 16 }}>
          <div style={{ fontSize: 12, color: '#8b949e', textTransform: 'uppercase', marginBottom: 8 }}>
            Operations ({payload.operations.length})
          </div>
          {payload.operations.map((op, i) => (
            <div key={i} style={{ background: '#0d1117', borderRadius: 6, padding: '10px 14px', marginBottom: 8, border: '1px solid #21262d' }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: '#58a6ff', marginBottom: 6 }}>{op.type}</div>
              <div style={{ fontSize: 12, color: '#8b949e', marginBottom: 4 }}>{op.description}</div>
              {Object.entries(op.details).map(([k, v]) => (
                <div key={k} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: '#c9d1d9', marginTop: 2 }}>
                  <span style={{ color: '#8b949e' }}>{k}</span>
                  <span style={{ fontFamily: 'monospace', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{String(v)}</span>
                </div>
              ))}
            </div>
          ))}
        </div>

        {/* Network */}
        <div style={rowStyle}>
          <span style={labelStyle}>Network</span>
          <span style={{ color: '#c9d1d9', fontSize: 13 }}>{payload.networkPassphrase}</span>
        </div>

        {error && (
          <div role="alert" style={{ background: 'rgba(248,81,73,0.1)', border: '1px solid #f85149', borderRadius: 6, padding: '8px 12px', fontSize: 13, color: '#f85149', marginTop: 12 }}>
            {error}
          </div>
        )}

        <div style={{ display: 'flex', gap: 10, marginTop: 20 }}>
          <button
            onClick={handleSign}
            disabled={isBusy}
            aria-busy={isBusy}
            style={{
              flex: 1, padding: '10px 0', borderRadius: 6, border: 'none', cursor: isBusy ? 'not-allowed' : 'pointer',
              background: isBusy ? '#21262d' : '#3fb950', color: '#fff', fontWeight: 600, fontSize: 14,
            }}
          >
            {isBusy ? (ctx.state === 'SIGNING_REQUEST' ? 'Waiting for Wallet…' : 'Broadcasting…') : 'Sign & Submit'}
          </button>
          <button
            onClick={onClose}
            disabled={isBusy}
            style={{ padding: '10px 20px', borderRadius: 6, border: '1px solid #30363d', cursor: 'pointer', background: 'transparent', color: '#c9d1d9', fontSize: 14 }}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

const rowStyle: React.CSSProperties = { display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '6px 0', borderBottom: '1px solid #21262d' };
const labelStyle: React.CSSProperties = { fontSize: 12, color: '#8b949e' };
const hashStyle: React.CSSProperties = { fontFamily: 'monospace', fontSize: 12, color: '#c9d1d9', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' };
