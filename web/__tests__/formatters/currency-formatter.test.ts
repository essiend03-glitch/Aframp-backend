import { currencyFormatter } from '@/lib/formatters/currency-formatter';

describe('CurrencyFormatter', () => {
  describe('formatFiat', () => {
    it('should format NGN correctly', () => {
      const result = currencyFormatter.formatFiat(1250.5, 'NGN', 'en-NG');
      expect(result.code).toBe('NGN');
      expect(result.symbol).toBe('₦');
      expect(result.raw).toBe(1250.5);
      expect(result.value).toContain('1,250.50');
    });

    it('should format KES correctly', () => {
      const result = currencyFormatter.formatFiat(1250.5, 'KES', 'en-KE');
      expect(result.code).toBe('KES');
      expect(result.symbol).toBe('KSh');
      expect(result.value).toContain('1,250.50');
    });

    it('should format GHS correctly', () => {
      const result = currencyFormatter.formatFiat(1250.5, 'GHS', 'en-GH');
      expect(result.code).toBe('GHS');
      expect(result.symbol).toBe('GH₵');
    });

    it('should format ZAR correctly', () => {
      const result = currencyFormatter.formatFiat(1250.5, 'ZAR', 'en-ZA');
      expect(result.code).toBe('ZAR');
      expect(result.symbol).toBe('R');
    });
  });

  describe('formatCrypto', () => {
    it('should format USDC with up to 7 decimals', () => {
      const result = currencyFormatter.formatCrypto(123.4567891, 'USDC', 'en-US');
      expect(result.code).toBe('USDC');
      expect(result.value).toContain('123.4567891');
    });

    it('should format EURC with up to 7 decimals', () => {
      const result = currencyFormatter.formatCrypto(100.123, 'EURC', 'en-US');
      expect(result.code).toBe('EURC');
      expect(result.raw).toBe(100.123);
    });

    it('should format XLM with up to 7 decimals', () => {
      const result = currencyFormatter.formatCrypto(50.1234567, 'XLM', 'en-US');
      expect(result.code).toBe('XLM');
      expect(result.value).toContain('50.1234567');
    });
  });

  describe('parseCurrency', () => {
    it('should parse English format correctly', () => {
      const result = currencyFormatter.parseCurrency('1,250.50', 'en-US');
      expect(result).toBe(1250.5);
    });

    it('should parse French format correctly', () => {
      const result = currencyFormatter.parseCurrency('1 250,50', 'fr-FR');
      expect(result).toBe(1250.5);
    });

    it('should handle currency symbols', () => {
      const result = currencyFormatter.parseCurrency('₦1,250.50', 'en-NG');
      expect(result).toBe(1250.5);
    });

    it('should return null for invalid input', () => {
      const result = currencyFormatter.parseCurrency('invalid', 'en-US');
      expect(result).toBeNull();
    });

    it('should return null for empty input', () => {
      const result = currencyFormatter.parseCurrency('', 'en-US');
      expect(result).toBeNull();
    });
  });

  describe('stroops conversion', () => {
    it('should convert stroops to amount correctly', () => {
      const result = currencyFormatter.stroopsToAmount(10000000);
      expect(result).toBe(1);
    });

    it('should convert amount to stroops correctly', () => {
      const result = currencyFormatter.amountToStroops(1.5);
      expect(result).toBe(15000000);
    });

    it('should handle precision correctly', () => {
      const stroops = currencyFormatter.amountToStroops(0.0000001);
      expect(stroops).toBe(1);
      const amount = currencyFormatter.stroopsToAmount(stroops);
      expect(amount).toBe(0.0000001);
    });
  });
});
