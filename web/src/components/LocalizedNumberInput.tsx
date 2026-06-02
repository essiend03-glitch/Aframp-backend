'use client';

import { useLocale } from 'next-intl';
import { useState, useEffect, ChangeEvent, FocusEvent } from 'react';
import { LOCALE_CONFIGS } from '@/config/locales';
import { SupportedLocale } from '@/types/locale';
import { currencyFormatter } from '@/lib/formatters/currency-formatter';

interface LocalizedNumberInputProps {
  value: number | null;
  onChange: (value: number | null) => void;
  placeholder?: string;
  min?: number;
  max?: number;
  disabled?: boolean;
  className?: string;
  error?: string;
}

export function LocalizedNumberInput({
  value,
  onChange,
  placeholder,
  min,
  max,
  disabled,
  className = '',
  error,
}: LocalizedNumberInputProps) {
  const locale = useLocale() as SupportedLocale;
  const config = LOCALE_CONFIGS[locale];
  const [displayValue, setDisplayValue] = useState('');
  const [isFocused, setIsFocused] = useState(false);

  // Update display value when value prop changes
  useEffect(() => {
    if (value !== null && !isFocused) {
      const formatted = new Intl.NumberFormat(locale, {
        minimumFractionDigits: 0,
        maximumFractionDigits: 7,
      }).format(value);
      setDisplayValue(formatted);
    } else if (value === null && !isFocused) {
      setDisplayValue('');
    }
  }, [value, locale, isFocused]);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const input = e.target.value;
    setDisplayValue(input);

    // Parse the localized input
    const parsed = currencyFormatter.parseCurrency(input, locale);

    // Validate range
    if (parsed !== null) {
      if (min !== undefined && parsed < min) {
        onChange(null);
        return;
      }
      if (max !== undefined && parsed > max) {
        onChange(null);
        return;
      }
      onChange(parsed);
    } else {
      onChange(null);
    }
  };

  const handleFocus = (e: FocusEvent<HTMLInputElement>) => {
    setIsFocused(true);
    // Convert to raw number format for easier editing
    if (value !== null) {
      setDisplayValue(value.toString());
    }
  };

  const handleBlur = (e: FocusEvent<HTMLInputElement>) => {
    setIsFocused(false);
    // Reformat to localized format
    if (value !== null) {
      const formatted = new Intl.NumberFormat(locale, {
        minimumFractionDigits: 0,
        maximumFractionDigits: 7,
      }).format(value);
      setDisplayValue(formatted);
    }
  };

  return (
    <div className="localized-number-input">
      <input
        type="text"
        inputMode="decimal"
        value={displayValue}
        onChange={handleChange}
        onFocus={handleFocus}
        onBlur={handleBlur}
        placeholder={placeholder}
        disabled={disabled}
        className={`input ${error ? 'error' : ''} ${className}`}
        aria-invalid={!!error}
        aria-describedby={error ? 'input-error' : undefined}
      />
      {error && (
        <span id="input-error" className="error-message">
          {error}
        </span>
      )}

      <style jsx>{`
        .localized-number-input {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }

        .input {
          width: 100%;
          padding: 10px 12px;
          font-size: 16px;
          border: 1px solid #d1d5db;
          border-radius: 8px;
          transition: all 0.2s;
          background: white;
        }

        .input:focus {
          outline: none;
          border-color: #2563eb;
          box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
        }

        .input.error {
          border-color: #dc2626;
        }

        .input.error:focus {
          box-shadow: 0 0 0 3px rgba(220, 38, 38, 0.1);
        }

        .input:disabled {
          background: #f3f4f6;
          cursor: not-allowed;
          opacity: 0.6;
        }

        .error-message {
          font-size: 14px;
          color: #dc2626;
        }
      `}</style>
    </div>
  );
}
