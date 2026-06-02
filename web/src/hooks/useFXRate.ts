/**
 * Foreign Exchange Rate Hook
 * Real-time FX rate tracking with caching
 */

'use client';

import { useQuery } from '@tanstack/react-query';
import { apiClient } from '@/lib/api/client';
import type { ExchangeRate, ExchangeQuote, CurrencyType } from '@/types/primitives';

async function fetchExchangeRate(
  sourceCurrency: CurrencyType,
  destinationCurrency: CurrencyType
): Promise<ExchangeRate> {
  return apiClient.get<ExchangeRate>(
    `/api/v1/exchange/rates?from=${sourceCurrency}&to=${destinationCurrency}`
  );
}

async function fetchExchangeQuote(
  sourceCurrency: CurrencyType,
  destinationCurrency: CurrencyType,
  amount: string
): Promise<ExchangeQuote> {
  return apiClient.post<ExchangeQuote>('/api/v1/exchange/quote', {
    sourceCurrency,
    destinationCurrency,
    sourceAmount: amount,
  });
}

export function useFXRate(sourceCurrency: CurrencyType, destinationCurrency: CurrencyType) {
  return useQuery({
    queryKey: ['exchange', 'rate', sourceCurrency, destinationCurrency],
    queryFn: () => fetchExchangeRate(sourceCurrency, destinationCurrency),
    staleTime: 60 * 1000, // 1 minute
    refetchInterval: 2 * 60 * 1000, // Refetch every 2 minutes
    enabled: sourceCurrency !== destinationCurrency,
  });
}

export function useExchangeQuote(
  sourceCurrency: CurrencyType,
  destinationCurrency: CurrencyType,
  amount: string,
  options?: { enabled?: boolean }
) {
  return useQuery({
    queryKey: ['exchange', 'quote', sourceCurrency, destinationCurrency, amount],
    queryFn: () => fetchExchangeQuote(sourceCurrency, destinationCurrency, amount),
    staleTime: 30 * 1000, // 30 seconds
    enabled: (options?.enabled ?? true) && !!amount && parseFloat(amount) > 0,
  });
}
