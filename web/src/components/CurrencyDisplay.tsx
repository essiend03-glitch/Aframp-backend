'use client';

import { useLocale } from 'next-intl';
import { currencyFormatter } from '@/lib/formatters/currency-formatter';
import { CurrencyCode } from '@/types/locale';
import { SupportedLocale } from '@/types/locale';

interface CurrencyDisplayProps {
  amount: number;
  currency: CurrencyCode;
  showCode?: boolean;
  className?: string;
}

export function CurrencyDisplay({
  amount,
  currency,
  showCode = false,
  className = '',
}: CurrencyDisplayProps) {
  const locale = useLocale() as SupportedLocale;

  const formatted = currencyFormatter.formatCurrency(
    amount,
    currency,
    locale,
    { showCode }
  );

  return (
    <span className={`currency-display ${className}`} data-currency={currency}>
      {formatted.value}
    </span>
  );
}
