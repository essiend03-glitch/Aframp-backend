import { format, formatInTimeZone } from 'date-fns-tz';
import { parseISO } from 'date-fns';
import { LOCALE_CONFIGS } from '@/config/locales';
import { SupportedLocale } from '@/types/locale';

/**
 * Date and time formatter with timezone support
 */
export class DateFormatter {
  /**
   * Format date according to locale configuration
   */
  formatDate(
    date: Date | string,
    locale: SupportedLocale = 'en',
    options?: {
      timezone?: string;
      formatString?: string;
    }
  ): string {
    const config = LOCALE_CONFIGS[locale];
    const dateObj = typeof date === 'string' ? parseISO(date) : date;
    const tz = options?.timezone || config.timezone;
    const formatStr = options?.formatString || config.dateFormat;

    return formatInTimeZone(dateObj, tz, formatStr);
  }

  /**
   * Format time according to locale configuration
   */
  formatTime(
    date: Date | string,
    locale: SupportedLocale = 'en',
    options?: {
      timezone?: string;
      formatString?: string;
    }
  ): string {
    const config = LOCALE_CONFIGS[locale];
    const dateObj = typeof date === 'string' ? parseISO(date) : date;
    const tz = options?.timezone || config.timezone;
    const formatStr = options?.formatString || config.timeFormat;

    return formatInTimeZone(dateObj, tz, formatStr);
  }

  /**
   * Format date and time together
   */
  formatDateTime(
    date: Date | string,
    locale: SupportedLocale = 'en',
    options?: {
      timezone?: string;
    }
  ): string {
    const config = LOCALE_CONFIGS[locale];
    const dateObj = typeof date === 'string' ? parseISO(date) : date;
    const tz = options?.timezone || config.timezone;
    const formatStr = `${config.dateFormat} ${config.timeFormat}`;

    return formatInTimeZone(dateObj, tz, formatStr);
  }

  /**
   * Format relative time (e.g., "2 hours ago")
   */
  formatRelative(
    date: Date | string,
    locale: SupportedLocale = 'en'
  ): string {
    const dateObj = typeof date === 'string' ? parseISO(date) : date;
    const now = new Date();
    const diffMs = now.getTime() - dateObj.getTime();
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffSecs < 60) return 'just now';
    if (diffMins < 60) return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
    if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    if (diffDays < 7) return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
    
    return this.formatDate(dateObj, locale);
  }

  /**
   * Format transaction timestamp with timezone
   */
  formatTransactionTime(
    timestamp: Date | string,
    locale: SupportedLocale = 'en',
    timezone?: string
  ): string {
    return this.formatDateTime(timestamp, locale, { timezone });
  }
}

// Singleton instance
export const dateFormatter = new DateFormatter();
