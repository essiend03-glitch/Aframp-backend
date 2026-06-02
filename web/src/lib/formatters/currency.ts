/**
 * Currency Formatting Utilities
 * Localized currency formatting for regional fiat specifications
 */

import type { CurrencyType, RegionalLocale } from '@/types/primitives';

// ============================================================================
// Currency Specifications
// ============================================================================

const CURRENCY_SPECS: Record<CurrencyType, { symbol: string; decimals: number }> = {
  NGN: { symbol: '₦', decimals: 2 },
  KES: { symbol: 'KSh', decimals: 2 },
  GHS: { symbol: 'GH₵', decimals: 2 },
  USD: { symbol: '$', decimals: 2 },
  ZAR: { symbol: 'R', decimals: 2 },
  UGX: { symbol: 'USh', decimals: 0 },
  USDC: { symbol: 'USDC', decimals: 6 },
  USDT: { symbol: 'USDT', decimals: 6 },
  cNGN: { symbol: 'cNGN', decimals: 2 },
};

// ============================================================================
// Formatting Functions
// ============================================================================

export function formatCurrency(
  amount: string | number,
  currency: CurrencyType,
  locale: RegionalLocale | string = 'en-US'
): string {
  const spec = CURRENCY_SPECS[currency];
  const numAmount = typeof amount === 'string' ? parseFloat(amount) : amount;

  if (Number.isNaN(numAmount)) {
    return `${spec.symbol} 0.${'0'.repeat(spec.decimals)}`;
  }

  const formatted = new Intl.NumberFormat(locale, {
    minimumFractionDigits: spec.decimals,
    maximumFractionDigits: spec.decimals,
  }).format(numAmount);

  // Handle symbol placement based on currency
  if (currency === 'KES' || currency === 'UGX') {
    return `${spec.symbol} ${formatted}`;
  }

  return `${spec.symbol}${formatted}`;
}

export function parseCurrencyInput(input: string): string {
  // Remove all non-numeric characters except decimal point
  const cleaned = input.replace(/[^\d.]/g, '');
  
  // Validate the result
  if (cleaned === '' || Number.isNaN(parseFloat(cleaned))) {
    return '';
  }

  return cleaned;
}

export function formatCompactCurrency(
  amount: string | number,
  currency: CurrencyType,
  locale: RegionalLocale | string = 'en-US'
): string {
  const spec = CURRENCY_SPECS[currency];
  const numAmount = typeof amount === 'string' ? parseFloat(amount) : amount;

  if (Number.isNaN(numAmount)) {
    return `${spec.symbol}0`;
  }

  const formatted = new Intl.NumberFormat(locale, {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(numAmount);

  return `${spec.symbol}${formatted}`;
}
