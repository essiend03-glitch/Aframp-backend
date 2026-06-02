/**
 * Telemetry tracking for localization events
 */

export interface LocaleTelemetryEvent {
  event: string;
  locale?: string;
  previousLocale?: string;
  missingKey?: string;
  errorType?: string;
  timestamp: string;
}

class LocaleTelemetry {
  private endpoint: string;

  constructor() {
    this.endpoint = process.env.NEXT_PUBLIC_TELEMETRY_ENDPOINT || '/api/v1/telemetry';
  }

  /**
   * Track locale switch events
   */
  async trackLocaleSwitch(fromLocale: string, toLocale: string): Promise<void> {
    await this.sendEvent({
      event: 'locale_switch_events_total',
      previousLocale: fromLocale,
      locale: toLocale,
      timestamp: new Date().toISOString(),
    });
  }

  /**
   * Track missing translation keys
   */
  async trackMissingTranslation(locale: string, key: string): Promise<void> {
    console.warn(`Missing translation key: ${key} for locale: ${locale}`);
    
    await this.sendEvent({
      event: 'missing_translation_key_exceptions',
      locale,
      missingKey: key,
      timestamp: new Date().toISOString(),
    });
  }

  /**
   * Track formatting parse errors
   */
  async trackFormattingError(
    locale: string,
    errorType: 'currency' | 'date' | 'number',
    error: Error
  ): Promise<void> {
    console.error(`Formatting error (${errorType}) for locale ${locale}:`, error);
    
    await this.sendEvent({
      event: 'formatting_parse_errors',
      locale,
      errorType,
      timestamp: new Date().toISOString(),
    });
  }

  /**
   * Send telemetry event to backend
   */
  private async sendEvent(event: LocaleTelemetryEvent): Promise<void> {
    try {
      await fetch(this.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(event),
        // Don't block on telemetry
        keepalive: true,
      });
    } catch (error) {
      // Silently fail telemetry to not impact user experience
      console.debug('Telemetry send failed:', error);
    }
  }
}

export const localeTelemetry = new LocaleTelemetry();
