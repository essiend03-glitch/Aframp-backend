/**
 * Locale configuration types for aframp platform
 */

export type SupportedLocale = 'en' | 'fr' | 'ha' | 'yo' | 'ig' | 'sw';

export type TextDirection = 'ltr' | 'rtl';

export interface LocaleConfig {
  code: SupportedLocale;
  name: string;
  nativeName: string;
  direction: TextDirection;
  timezone: string;
  dateFormat: string;
  timeFormat: string;
  firstDayOfWeek: 0 | 1 | 2 | 3 | 4 | 5 | 6;
  numberGrouping: {
    groupSeparator: string;
    decimalSeparator: string;
    groupSize: number;
  };
}

export type CurrencyCode = 'NGN' | 'KES' | 'GHS' | 'ZAR' | 'USDC' | 'EURC' | 'XLM';

export interface CurrencyMetadata {
  code: CurrencyCode;
  symbol: string;
  name: string;
  subUnit: string;
  subUnitDivision: number;
  minFractionDigits: number;
  maxFractionDigits: number;
  symbolPosition: 'before' | 'after';
  isCrypto: boolean;
}

export interface FormattedCurrency {
  value: string;
  symbol: string;
  code: CurrencyCode;
  raw: number;
}

export interface LocalePreference {
  locale: SupportedLocale;
  timezone?: string;
  currency?: CurrencyCode;
}
