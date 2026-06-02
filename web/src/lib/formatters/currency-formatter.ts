import { CurrencyCode, FormattedCurrency } from '@/types/locale';
import { CURRENCY_METADATA } from '@/config/currencies';

/**
 * High-precision currency formatter using Intl.NumberFormat
 * Handles both fiat (2 decimals) and crypto (up to 7 decimals)
 */
export class CurrencyFormatter {
  private formatters: Map<string, Intl.NumberFormat> = new Map();

  /**
   * Format currency value with proper regional rules
   */
  formatCurrency(
    amount: number,
    currencyCode: CurrencyCode,
    locale: string = 'en-US',
    options?: {
      showSymbol?: boolean;
      showCode?: boolean;
      minDecimals?: number;
      maxDecimals?: number;
    }
  ): FormattedCurrency {
    const metadata = CURRENCY_METADATA[currencyCode];
    if (!metadata) {
      throw new Error(`Unsupported currency: ${currencyCode}`);
    }

    const minDecimals = options?.minDecimals ?? metadata.minFractionDigits;
    const maxDecimals = options?.maxDecimals ?? metadata.maxFractionDigits;

    const formatterKey = `${locale}-${currencyCode}-${minDecimals}-${maxDecimals}`;
    
    let formatter = this.formatters.get(formatterKey);
    if (!formatter) {
      formatter = new Intl.NumberFormat(locale, {
        style: 'decimal',
        minimumFractionDigits: minDecimals,
        maximumFractionDigits: maxDecimals,
        useGrouping: true,
      });
      this.formatters.set(formatterKey, formatter);
    }

    const formattedValue = formatter.format(amount);
    
    let displayValue = formattedValue;
    if (options?.showSymbol !== false) {
      displayValue = metadata.symbolPosition === 'before'
        ? `${metadata.symbol}${formattedValue}`
        : `${formattedValue} ${metadata.symbol}`;
    }
    
    if (options?.showCode) {
      displayValue = `${displayValue} ${metadata.code}`;
    }

    return {
      value: displayValue,
      symbol: metadata.symbol,
      code: currencyCode,
      raw: amount,
    };
  }

  /**
   * Format crypto with full precision (up to 7 decimals for Stellar stroops)
   */
  formatCrypto(
    amount: number,
    currencyCode: Extract<CurrencyCode, 'USDC' | 'EURC' | 'XLM'>,
    locale: string = 'en-US'
  ): FormattedCurrency {
    return this.formatCurrency(amount, currencyCode, locale, {
      minDecimals: 2,
      maxDecimals: 7,
    });
  }

  /**
   * Format fiat with standard 2 decimal precision
   */
  formatFiat(
    amount: number,
    currencyCode: Extract<CurrencyCode, 'NGN' | 'KES' | 'GHS' | 'ZAR'>,
    locale: string = 'en-US'
  ): FormattedCurrency {
    return this.formatCurrency(amount, currencyCode, locale, {
      minDecimals: 2,
      maxDecimals: 2,
    });
  }

  /**
   * Parse localized currency string to number
   * Handles different decimal and grouping separators
   */
  parseCurrency(
    value: string,
    locale: string = 'en-US'
  ): number | null {
    if (!value || typeof value !== 'string') {
      return null;
    }

    // Remove currency symbols and codes
    let cleaned = value.trim();
    Object.values(CURRENCY_METADATA).forEach(metadata => {
      cleaned = cleaned.replace(metadata.symbol, '').replace(metadata.code, '');
    });
    cleaned = cleaned.trim();

    // Detect decimal separator based on locale
    const parts = new Intl.NumberFormat(locale).formatToParts(1234.56);
    const decimalSep = parts.find(p => p.type === 'decimal')?.value || '.';
    const groupSep = parts.find(p => p.type === 'group')?.value || ',';

    // Remove group separators
    cleaned = cleaned.split(groupSep).join('');
    
    // Normalize decimal separator to dot
    if (decimalSep !== '.') {
      cleaned = cleaned.replace(decimalSep, '.');
    }

    // Validate and parse
    const parsed = parseFloat(cleaned);
    return isNaN(parsed) ? null : parsed;
  }

  /**
   * Convert stroops (Stellar base unit) to display amount
   */
  stroopsToAmount(stroops: number): number {
    return stroops / 10000000;
  }

  /**
   * Convert display amount to stroops
   */
  amountToStroops(amount: number): number {
    return Math.round(amount * 10000000);
  }
}

// Singleton instance
export const currencyFormatter = new CurrencyFormatter();
