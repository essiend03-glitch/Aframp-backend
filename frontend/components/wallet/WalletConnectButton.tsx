// Issue #480 — Wallet Connect Button + Trustline Setup
// Displays connection state, provider selection, and one-click trustline setup.

'use client';

import React, { useState } from 'react';
import { useWalletConnection } from '../../hooks/useWalletConnection';
import type { WalletProvider } from '../../types';

const PROVIDERS: { id: WalletProvider; label: string; icon: string }[] = [
  { id: 'freighter', label: 'Freighter', icon: '🚀' },
  { id: 'albedo', label: 'Albedo', icon: '🌟' },
  { id: 'lobstr', label: 'Lobstr', icon: '🦞' },
];

export default function WalletConnectButton() {
  const { ctx, connect, disconnect, setupTrustline, toastMessage } = useWalletConnection();
  const [showPicker, setShowPicker] = useState(false);

  const isConnected = ctx.state === 'CONNECTED';
  const isBusy = ctx.state === 'CONNECTING' || ctx.state === 'SIGNING_REQUEST' || ctx.state === 'TX_SUBMITTING';

  return (
    <div style={{ position: 'relative', display: 'inline-block' }}>
      {/* Toast overlay */}
      {toastMessage && (
        <div
          role="status"
          aria-live="polite"
          style={{
            position: 'fixed', bottom: 24, right: 24, zIndex: 2000,
            background: '#161b22', border: '1px solid #30363d', borderRadius: 8,
            padding: '12px 18px', fontSize: 13, color: '#c9d1d9', maxWidth: 320,
            boxShadow: '0 4px 20px rgba(0,0,0,0.4)',
          }}
        >
          {toastMessage}
        </div>
      )}

      {isConnected ? (
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: 8, padding: '8px 14px', fontSize: 13 }}>
            <span style={{ color: '#3fb950', marginRight: 6 }}>●</span>
            <span style={{ fontFamily: 'monospace', color: '#c9d1d9' }}>
              {ctx.publicKey?.slice(0, 6)}…{ctx.publicKey?.slice(-4)}
            </span>
            <span style={{ color: '#8b949e', marginLeft: 8, fontSize: 11 }}>{ctx.provider}</span>
          </div>
          {!ctx.hasCNGNTrustline && (
            <button
              onClick={setupTrustline}
              disabled={isBusy}
              aria-label="Add cNGN trustline"
              style={{ padding: '8px 14px', borderRadius: 8, border: '1px solid #f0883e', background: 'rgba(240,136,62,0.1)', color: '#f0883e', cursor: 'pointer', fontSize: 13 }}
            >
              + Add cNGN Trustline
            </button>
          )}
          <button
            onClick={disconnect}
            aria-label="Disconnect wallet"
            style={{ padding: '8px 14px', borderRadius: 8, border: '1px solid #30363d', background: 'transparent', color: '#8b949e', cursor: 'pointer', fontSize: 13 }}
          >
            Disconnect
          </button>
        </div>
      ) : (
        <>
          <button
            onClick={() => setShowPicker((v) => !v)}
            disabled={isBusy}
            aria-expanded={showPicker}
            aria-haspopup="listbox"
            style={{
              padding: '8px 18px', borderRadius: 8, border: 'none',
              background: isBusy ? '#21262d' : '#3fb950', color: '#fff',
              cursor: isBusy ? 'not-allowed' : 'pointer', fontWeight: 600, fontSize: 14,
            }}
          >
            {isBusy ? 'Connecting…' : 'Connect Wallet'}
          </button>
          {showPicker && (
            <div
              role="listbox"
              aria-label="Select wallet provider"
              style={{
                position: 'absolute', top: '110%', right: 0, zIndex: 100,
                background: '#161b22', border: '1px solid #30363d', borderRadius: 8,
                padding: 8, minWidth: 180, boxShadow: '0 4px 20px rgba(0,0,0,0.4)',
              }}
            >
              {PROVIDERS.map((p) => (
                <button
                  key={p.id}
                  role="option"
                  aria-selected={false}
                  onClick={() => { setShowPicker(false); connect(p.id); }}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 10, width: '100%',
                    padding: '10px 14px', background: 'none', border: 'none',
                    cursor: 'pointer', color: '#c9d1d9', fontSize: 14, borderRadius: 6,
                    textAlign: 'left',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = '#21262d')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                >
                  <span>{p.icon}</span>
                  <span>{p.label}</span>
                </button>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
