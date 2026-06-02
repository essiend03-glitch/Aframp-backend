/**
 * Wallet Balance Hook
 * Real-time wallet balance tracking with optimized polling
 */

'use client';

import { useQuery } from '@tanstack/react-query';
import { apiClient } from '@/lib/api/client';
import type { WalletBalance } from '@/types/primitives';

async function fetchWalletBalance(walletId: string): Promise<WalletBalance> {
  return apiClient.get<WalletBalance>(`/api/v1/wallets/${walletId}/balance`);
}

async function fetchAllWalletBalances(userId: string): Promise<WalletBalance[]> {
  return apiClient.get<WalletBalance[]>(`/api/v1/users/${userId}/wallets/balances`);
}

export function useWalletBalance(walletId: string, options?: { enabled?: boolean; refetchInterval?: number }) {
  return useQuery({
    queryKey: ['wallet', 'balance', walletId],
    queryFn: () => fetchWalletBalance(walletId),
    enabled: options?.enabled ?? true,
    staleTime: 30 * 1000, // 30 seconds
    refetchInterval: options?.refetchInterval ?? 60 * 1000, // Poll every 60 seconds
    refetchOnWindowFocus: true,
  });
}

export function useAllWalletBalances(userId: string, options?: { enabled?: boolean }) {
  return useQuery({
    queryKey: ['wallets', 'balances', userId],
    queryFn: () => fetchAllWalletBalances(userId),
    enabled: options?.enabled ?? true,
    staleTime: 30 * 1000,
    refetchInterval: 60 * 1000,
    refetchOnWindowFocus: true,
  });
}
