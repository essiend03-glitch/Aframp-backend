import { CurrencyCode } from '@/types/locale';
import { CURRENCY_METADATA } from '@/config/currencies';

/**
 * Validation utilities for currency inputs
 */
export class CurrencyValidator {
  /**
   * Validate amount against currency constraints
   */
  static validateAmount(
    amount: number,
    currency: CurrencyCode,
    options?: {
      min?: number;
      max?: number;
    }
  ): { valid: boolean; error?: string } {
    const metadata = CURRENCY_METADATA[currency];

    if (!metadata) {
      return { valid: false, error: 'Invalid currency' };
    }

    if (isNaN(amount) || !isFinite(amount)) {
      return { valid: false, error: 'Invalid amount' };
    }

    if (amount < 0) {
      return { valid: false, error: 'Amount must be positive' };
    }

    if (options?.min !== undefined && amount < options.min) {
      return { valid: false, error: `Minimum amount is ${options.min}` };
    }

    if (options?.max !== undefined && amount > options.max) {
      return { valid: false, error: `Maximum amount is ${options.max}` };
    }

    // Check decimal precision
    const decimalPlaces = (amount.toString().split('.')[1] || '').length;
    if (decimalPlaces > metadata.maxFractionDigits) {
      return {
        valid: false,
        error: `Maximum ${metadata.maxFractionDigits} decimal places allowed`,
      };
    }

    return { valid: true };
  }

  /**
   * Sanitize amount to prevent injection attacks
   */
  static sanitizeAmount(input: string): string {
    // Remove all non-numeric characters except decimal separators
    return input.replace(/[^\d.,\s]/g, '');
  }

  /**
   * Check if amount is within safe JavaScript number range
   */
  static isSafeAmount(amount: number): boolean {
    return amount <= Number.MAX_SAFE_INTEGER && amount >= Number.MIN_SAFE_INTEGER;
  }
}
