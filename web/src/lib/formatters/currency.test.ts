/**
 * Currency Formatter Tests
 * Unit tests for localized currency formatting
 */

import { describe, it, expect } from 'vitest';
import { formatCurrency, parseCurrencyInput } from './currency';

describe('formatCurrency', () => {
  it('formats NGN correctly', () => {
    expect(formatCurrency('1000.50', 'NGN', 'en-NG')).toBe('₦1,000.50');
  });

  it('formats USD correctly', () => {
    expect(formatCurrency('1234.56', 'USD', 'en-US')).toBe('$1,234.56');
  });

  it('formats KES correctly', () => {
    expect(formatCurrency('5000', 'KES', 'en-KE')).toBe('KSh 5,000.00');
  });

  it('handles zero values', () => {
    expect(formatCurrency('0', 'NGN', 'en-NG')).toBe('₦0.00');
  });

  it('handles large numbers', () => {
    expect(formatCurrency('1000000', 'USD', 'en-US')).toBe('$1,000,000.00');
  });

  it('handles decimal precision', () => {
    expect(formatCurrency('10.123456', 'USD', 'en-US')).toBe('$10.12');
  });
});

describe('parseCurrencyInput', () => {
  it('parses formatted currency string', () => {
    expect(parseCurrencyInput('₦1,000.50')).toBe('1000.50');
  });

  it('removes currency symbols', () => {
    expect(parseCurrencyInput('$1,234.56')).toBe('1234.56');
  });

  it('handles plain numbers', () => {
    expect(parseCurrencyInput('1000')).toBe('1000');
  });

  it('handles decimal inputs', () => {
    expect(parseCurrencyInput('10.50')).toBe('10.50');
  });

  it('returns empty string for invalid input', () => {
    expect(parseCurrencyInput('abc')).toBe('');
  });
});
