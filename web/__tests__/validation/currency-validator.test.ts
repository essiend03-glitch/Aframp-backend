import { CurrencyValidator } from '@/lib/validation/currency-validator';

describe('CurrencyValidator', () => {
  describe('validateAmount', () => {
    it('should validate positive amounts', () => {
      const result = CurrencyValidator.validateAmount(100, 'NGN');
      expect(result.valid).toBe(true);
      expect(result.error).toBeUndefined();
    });

    it('should reject negative amounts', () => {
      const result = CurrencyValidator.validateAmount(-100, 'NGN');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Amount must be positive');
    });

    it('should reject NaN', () => {
      const result = CurrencyValidator.validateAmount(NaN, 'NGN');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Invalid amount');
    });

    it('should reject Infinity', () => {
      const result = CurrencyValidator.validateAmount(Infinity, 'NGN');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Invalid amount');
    });

    it('should enforce minimum amount', () => {
      const result = CurrencyValidator.validateAmount(50, 'NGN', { min: 100 });
      expect(result.valid).toBe(false);
      expect(result.error).toContain('Minimum amount');
    });

    it('should enforce maximum amount', () => {
      const result = CurrencyValidator.validateAmount(1000, 'NGN', { max: 500 });
      expect(result.valid).toBe(false);
      expect(result.error).toContain('Maximum amount');
    });

    it('should enforce decimal precision for fiat', () => {
      const result = CurrencyValidator.validateAmount(100.123, 'NGN');
      expect(result.valid).toBe(false);
      expect(result.error).toContain('decimal places');
    });

    it('should allow up to 7 decimals for crypto', () => {
      const result = CurrencyValidator.validateAmount(100.1234567, 'USDC');
      expect(result.valid).toBe(true);
    });

    it('should reject more than 7 decimals for crypto', () => {
      const result = CurrencyValidator.validateAmount(100.12345678, 'USDC');
      expect(result.valid).toBe(false);
      expect(result.error).toContain('decimal places');
    });
  });

  describe('sanitizeAmount', () => {
    it('should remove currency symbols', () => {
      const result = CurrencyValidator.sanitizeAmount('₦1,250.50');
      expect(result).toBe('1,250.50');
    });

    it('should remove letters', () => {
      const result = CurrencyValidator.sanitizeAmount('abc123.45def');
      expect(result).toBe('123.45');
    });

    it('should preserve decimal separators', () => {
      const result = CurrencyValidator.sanitizeAmount('1,250.50');
      expect(result).toBe('1,250.50');
    });

    it('should handle malicious input', () => {
      const result = CurrencyValidator.sanitizeAmount('<script>alert("xss")</script>123');
      expect(result).toBe('123');
    });
  });

  describe('isSafeAmount', () => {
    it('should accept safe numbers', () => {
      expect(CurrencyValidator.isSafeAmount(1000000)).toBe(true);
    });

    it('should accept MAX_SAFE_INTEGER', () => {
      expect(CurrencyValidator.isSafeAmount(Number.MAX_SAFE_INTEGER)).toBe(true);
    });

    it('should accept MIN_SAFE_INTEGER', () => {
      expect(CurrencyValidator.isSafeAmount(Number.MIN_SAFE_INTEGER)).toBe(true);
    });

    it('should reject numbers beyond safe range', () => {
      expect(CurrencyValidator.isSafeAmount(Number.MAX_SAFE_INTEGER + 1)).toBe(false);
    });
  });
});
