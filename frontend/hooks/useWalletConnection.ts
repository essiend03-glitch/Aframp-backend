// Issue #480 — Non-Custodial Wallet Integration
// Universal Wallet Abstraction Layer + React context hook.
// Supports Freighter, Albedo, Lobstr. Private keys never touch this layer.

import { useState, useCallback, useContext, createContext, useEffect } from 'react';
import type { WalletConnectionContext, WalletProvider, TrustlineStatus } from '../types';

// ── Horizon error map (issue #480 §3) ─────────────────────────────────────────
const HORIZON_ERRORS: Record<string, string> = {
  tx_bad_seq: 'Transaction sequence number is out of order. Please refresh and try again.',
  op_low_reserve: 'Insufficient XLM reserve. Add at least 0.5 XLM to your account.',
  op_no_trust: 'No cNGN trustline found. Please add the trustline first.',
  tx_insufficient_fee: 'Transaction fee too low. Please increase the fee and retry.',
  op_underfunded: 'Insufficient balance for this operation.',
  tx_bad_auth: 'Invalid signature. Please re-sign the transaction.',
};

export function mapHorizonError(code: string): string {
  return HORIZON_ERRORS[code] ?? `Transaction failed: ${code}`;
}

// ── Wallet Abstraction Layer (issue #480 §2) ──────────────────────────────────

interface WalletAdapter {
  connect(): Promise<{ publicKey: string; network: 'mainnet' | 'testnet' }>;
  signXDR(xdr: string, networkPassphrase: string): Promise<string>;
  disconnect?(): void;
}

function getFreighterAdapter(): WalletAdapter {
  return {
    async connect() {
      const { getPublicKey, getNetwork } = await import('@stellar/freighter-api');
      const publicKey = await getPublicKey();
      const network = (await getNetwork()) === 'TESTNET' ? 'testnet' : 'mainnet';
      return { publicKey, network };
    },
    async signXDR(xdr, networkPassphrase) {
      const { signTransaction } = await import('@stellar/freighter-api');
      return signTransaction(xdr, { networkPassphrase });
    },
  };
}

function getAlbedoAdapter(): WalletAdapter {
  return {
    async connect() {
      const albedo = (await import('albedo.link/src/index')).default;
      const result = await albedo.publicKey({});
      return { publicKey: result.pubkey, network: 'mainnet' };
    },
    async signXDR(xdr, networkPassphrase) {
      const albedo = (await import('albedo.link/src/index')).default;
      const result = await albedo.tx({ xdr, network: networkPassphrase.includes('Test') ? 'testnet' : 'public' });
      return result.signed_envelope_xdr;
    },
  };
}

function getLobstrAdapter(): WalletAdapter {
  return {
    async connect() {
      const kit = (await import('@creit.tech/stellar-wallets-kit')).default;
      // Lobstr uses the StellarWalletsKit interface
      const publicKey = await (kit as any).getPublicKey();
      return { publicKey, network: 'mainnet' };
    },
    async signXDR(xdr, networkPassphrase) {
      const kit = (await import('@creit.tech/stellar-wallets-kit')).default;
      const { signedXDR } = await (kit as any).sign({ xdr, publicKey: '' });
      return signedXDR;
    },
  };
}

function getAdapter(provider: WalletProvider): WalletAdapter {
  switch (provider) {
    case 'freighter': return getFreighterAdapter();
    case 'albedo': return getAlbedoAdapter();
    case 'lobstr': return getLobstrAdapter();
    default: throw new Error(`Unsupported provider: ${provider}`);
  }
}

// ── Trustline check (issue #480 §3) ──────────────────────────────────────────

const HORIZON_URL = process.env.NEXT_PUBLIC_HORIZON_URL ?? 'https://horizon.stellar.org';
const CNGN_ISSUER = process.env.NEXT_PUBLIC_CNGN_ISSUER ?? '';

export async function checkCNGNTrustline(publicKey: string): Promise<TrustlineStatus> {
  const res = await fetch(`${HORIZON_URL}/accounts/${publicKey}`);
  if (!res.ok) return { exists: false };
  const data = await res.json();
  const trustline = data.balances?.find(
    (b: { asset_code?: string; asset_issuer?: string }) =>
      b.asset_code === 'cNGN' && b.asset_issuer === CNGN_ISSUER
  );
  return trustline
    ? { exists: true, limit: trustline.limit, balance: trustline.balance }
    : { exists: false };
}

// ── React Context (issue #480 §2) ─────────────────────────────────────────────

const WalletContext = createContext<{
  ctx: WalletConnectionContext;
  connect: (provider: WalletProvider) => Promise<void>;
  disconnect: () => void;
  signAndSubmit: (xdr: string, networkPassphrase: string) => Promise<string>;
  setupTrustline: () => Promise<void>;
  toastMessage: string | null;
} | null>(null);

export function WalletConnectionProvider({ children }: { children: React.ReactNode }) {
  const [ctx, setCtx] = useState<WalletConnectionContext>({
    state: 'DISCONNECTED',
    publicKey: null,
    provider: null,
    network: null,
    hasCNGNTrustline: false,
  });
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  const toast = (msg: string) => {
    setToastMessage(msg);
    setTimeout(() => setToastMessage(null), 4000);
  };

  const connect = useCallback(async (provider: WalletProvider) => {
    setCtx((c) => ({ ...c, state: 'CONNECTING', provider }));
    try {
      const adapter = getAdapter(provider);
      const { publicKey, network } = await adapter.connect();
      const trustlineStatus = await checkCNGNTrustline(publicKey);
      setCtx({ state: 'CONNECTED', publicKey, provider, network, hasCNGNTrustline: trustlineStatus.exists });
      toast('Wallet connected successfully.');
    } catch (err: unknown) {
      setCtx((c) => ({ ...c, state: 'DISCONNECTED' }));
      toast(`Connection failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, []);

  const disconnect = useCallback(() => {
    setCtx({ state: 'DISCONNECTED', publicKey: null, provider: null, network: null, hasCNGNTrustline: false });
  }, []);

  const signAndSubmit = useCallback(async (xdr: string, networkPassphrase: string): Promise<string> => {
    if (ctx.state !== 'CONNECTED' || !ctx.provider) throw new Error('Wallet not connected');
    setCtx((c) => ({ ...c, state: 'SIGNING_REQUEST' }));
    toast('Waiting for Wallet Approval...');
    try {
      const adapter = getAdapter(ctx.provider);
      const signedXDR = await adapter.signXDR(xdr, networkPassphrase);
      setCtx((c) => ({ ...c, state: 'TX_SUBMITTING' }));
      toast('Broadcasting Transaction to Stellar Horizon...');
      const res = await fetch(`${HORIZON_URL}/transactions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: `tx=${encodeURIComponent(signedXDR)}`,
      });
      const data = await res.json();
      if (!res.ok) {
        const code = data?.extras?.result_codes?.transaction ?? 'unknown';
        throw new Error(mapHorizonError(code));
      }
      setCtx((c) => ({ ...c, state: 'CONNECTED' }));
      toast('Settlement Confirmed!');
      return data.hash;
    } catch (err: unknown) {
      setCtx((c) => ({ ...c, state: 'CONNECTED' }));
      const msg = err instanceof Error ? err.message : String(err);
      toast(msg);
      throw err;
    }
  }, [ctx.state, ctx.provider]);

  const setupTrustline = useCallback(async () => {
    if (!ctx.publicKey || !ctx.provider) return;
    // Build ChangeTrust XDR via backend endpoint
    const res = await fetch('/api/v1/wallet/trustline/build', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ publicKey: ctx.publicKey }),
    });
    const { xdr, networkPassphrase } = await res.json();
    await signAndSubmit(xdr, networkPassphrase);
    setCtx((c) => ({ ...c, hasCNGNTrustline: true }));
  }, [ctx.publicKey, ctx.provider, signAndSubmit]);

  return (
    <WalletContext.Provider value={{ ctx, connect, disconnect, signAndSubmit, setupTrustline, toastMessage }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWalletConnection() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error('useWalletConnection must be used inside WalletConnectionProvider');
  return ctx;
}
