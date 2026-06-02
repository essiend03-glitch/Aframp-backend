// Issue #479 — Event Router: intercepts server events and pushes into React Query cache.
// Handles cNGN.transaction.settled and wallet.balance.updated events.

import { useCallback, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useWebSocket, WSReadyState } from './useWebSocket';
import type { LedgerTransaction, WalletBalance } from '../types';

const WS_URL = process.env.NEXT_PUBLIC_WS_URL ?? 'ws://localhost:8000/api/v1/streams/events';

interface StreamEvent {
  type: string;
  payload: unknown;
}

/** IDs of recently updated rows — used to trigger flash-highlight CSS class */
export function useLedgerStream() {
  const qc = useQueryClient();
  const [flashIds, setFlashIds] = useState<Set<string>>(new Set());

  const flash = useCallback((id: string) => {
    setFlashIds((prev) => new Set(prev).add(id));
    setTimeout(() => setFlashIds((prev) => { const s = new Set(prev); s.delete(id); return s; }), 1200);
  }, []);

  const handleMessage = useCallback((evt: MessageEvent) => {
    let event: StreamEvent;
    try { event = JSON.parse(evt.data); } catch { return; }

    switch (event.type) {
      case 'cNGN.transaction.settled': {
        const tx = event.payload as LedgerTransaction;
        // Prepend to cached transaction list
        qc.setQueryData<LedgerTransaction[]>(['transactions'], (old = []) => [tx, ...old]);
        flash(tx.id);
        break;
      }
      case 'wallet.balance.updated': {
        const balance = event.payload as WalletBalance;
        qc.setQueryData<WalletBalance[]>(['balances'], (old = []) =>
          old.map((b) => (b.currency === balance.currency ? balance : b))
        );
        flash(`balance-${balance.currency}`);
        break;
      }
      default:
        break;
    }
  }, [qc, flash]);

  const { readyState } = useWebSocket({ url: WS_URL, onMessage: handleMessage });

  return { readyState, flashIds };
}
