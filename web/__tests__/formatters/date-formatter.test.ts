import { dateFormatter } from '@/lib/formatters/date-formatter';

describe('DateFormatter', () => {
  const testDate = new Date('2024-03-15T14:30:00Z');

  describe('formatDate', () => {
    it('should format date in English locale', () => {
      const result = dateFormatter.formatDate(testDate, 'en');
      expect(result).toMatch(/15\/03\/2024/);
    });

    it('should format date in French locale', () => {
      const result = dateFormatter.formatDate(testDate, 'fr');
      expect(result).toMatch(/15\/03\/2024/);
    });

    it('should handle string input', () => {
      const result = dateFormatter.formatDate('2024-03-15T14:30:00Z', 'en');
      expect(result).toMatch(/15\/03\/2024/);
    });
  });

  describe('formatTime', () => {
    it('should format time correctly', () => {
      const result = dateFormatter.formatTime(testDate, 'en', {
        timezone: 'UTC',
      });
      expect(result).toMatch(/14:30:00/);
    });
  });

  describe('formatDateTime', () => {
    it('should format date and time together', () => {
      const result = dateFormatter.formatDateTime(testDate, 'en', {
        timezone: 'UTC',
      });
      expect(result).toContain('15/03/2024');
      expect(result).toContain('14:30:00');
    });
  });

  describe('formatRelative', () => {
    it('should return "just now" for recent dates', () => {
      const now = new Date();
      const result = dateFormatter.formatRelative(now, 'en');
      expect(result).toBe('just now');
    });

    it('should return minutes ago', () => {
      const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000);
      const result = dateFormatter.formatRelative(fiveMinutesAgo, 'en');
      expect(result).toBe('5 minutes ago');
    });

    it('should return hours ago', () => {
      const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000);
      const result = dateFormatter.formatRelative(twoHoursAgo, 'en');
      expect(result).toBe('2 hours ago');
    });

    it('should return days ago', () => {
      const threeDaysAgo = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000);
      const result = dateFormatter.formatRelative(threeDaysAgo, 'en');
      expect(result).toBe('3 days ago');
    });
  });

  describe('formatTransactionTime', () => {
    it('should format transaction timestamp with timezone', () => {
      const result = dateFormatter.formatTransactionTime(testDate, 'en', 'Africa/Lagos');
      expect(result).toBeTruthy();
      expect(typeof result).toBe('string');
    });
  });
});
