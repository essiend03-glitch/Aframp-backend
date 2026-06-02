'use client';

import { useState, useEffect } from 'react';
import { MultisigEnvelope } from '@/types';
import { parseXdr, xdrDigest, ParsedXdrTransaction } from '@/lib/xdrParser';

declare global {
  interface Window {
    freighter?: {
      isConnected: () => Promise<boolean>;
      getPublicKey: () => Promise<string>;
      signTransaction: (xdr: string, options?: { networkPassphrase: string }) => Promise<string>;
    };
  }
}

interface XDRSigningPanelProps {
  envelope: MultisigEnvelope;
  onSign: (proposalId: string, signedXdr: string, signerKey: string) => Promise<void>;
  /** If false, the panel is read-only (e.g., non-signatory role) */
  canSign: boolean;
}

export function XDRSigningPanel({ envelope, onSign, canSign }: XDRSigningPanelProps) {
  const [parsedTx, setParsedTx] = useState<ParsedXdrTransaction | null>(null);
  const [digest, setDigest] = useState<string>('');
  const [walletConnected, setWalletConnected] = useState(false);
  const [walletKey, setWalletKey] = useState<string | null>(null);
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    parseXdr(envelope.unsignedXdr).then(setParsedTx);
    xdrDigest(envelope.unsignedXdr).then(setDigest);
    checkWallet();
  }, [envelope.unsignedXdr]);

  async function checkWallet() {
    if (!window.freighter) return;
    try {
      const connected = await window.freighter.isConnected();
      if (connected) {
        const pk = await window.freighter.getPublicKey();
        setWalletKey(pk);
        setWalletConnected(true);
      }
    } catch { /* ignore */ }
  }

  async function handleSign() {
    if (!window.freighter || !walletKey) {
      setError('Freighter wallet not connected');
      return;
    }

    setSigning(true);
    setError(null);
    try {
      // Freighter signs the XDR and returns a decorated signature envelope
      const signedXdr = await window.freighter.signTransaction(envelope.unsignedXdr, {
        networkPassphrase: 'Public Global Stellar Network ; September 2015',
      });

      await onSign(envelope.id, signedXdr, walletKey);
    } catch (e: unknown) {
      setError((e as Error).message ?? 'Signing failed');
    } finally {
      setSigning(false);
    }
  }

  if (!parsedTx) return <p className="text-muted">Decoding XDR…</p>;

  return (
    <section className="xdr-signing" aria-label="XDR transaction details and signing">
      <header className="xdr-signing__header">
        <h3>Transaction Details</h3>
        <button
          className="btn btn--xs btn--ghost"
          onClick={() => navigator.clipboard.writeText(envelope.unsignedXdr)}
          aria-label="Copy raw XDR to clipboard"
        >
          Copy XDR
        </button>
      </header>

      <dl className="xdr-details">
        <div>
          <dt>Source Account</dt>
          <dd className="font-mono-sm">{parsedTx.sourceAccount}</dd>
        </div>
        <div>
          <dt>Fee (stroops)</dt>
          <dd>{parsedTx.fee.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Sequence</dt>
          <dd>{parsedTx.seqNum}</dd>
        </div>
        <div>
          <dt>Memo</dt>
          <dd>{parsedTx.memo}</dd>
        </div>
        <div>
          <dt>XDR Digest (SHA-256)</dt>
          <dd className="font-mono-xs">{digest}</dd>
        </div>
      </dl>

      <section className="xdr-operations">
        <h4>Operations ({parsedTx.operations.length})</h4>
        {parsedTx.operations.map((op, idx) => (
          <details key={idx} className="xdr-operation" open={idx === 0}>
            <summary className="xdr-operation__summary">
              <strong>{idx + 1}.</strong> {op.type}
            </summary>
            <pre className="xdr-operation__body">{JSON.stringify(op.body, null, 2)}</pre>
          </details>
        ))}
      </section>

      {canSign && (
        <footer className="xdr-signing__footer">
          {!walletConnected ? (
            <div className="alert alert--warning">
              Freighter wallet not detected. Install the extension and connect your account.
            </div>
          ) : (
            <>
              <p className="text-sm">
                Connected: <span className="font-mono-sm">{walletKey}</span>
              </p>
              <button
                className="btn btn--primary btn--lg"
                onClick={handleSign}
                disabled={signing}
                aria-busy={signing}
              >
                {signing ? 'Signing…' : 'Sign with Freighter'}
              </button>
            </>
          )}
          {error && <p className="alert alert--danger" role="alert">{error}</p>}
        </footer>
      )}
    </section>
  );
}
