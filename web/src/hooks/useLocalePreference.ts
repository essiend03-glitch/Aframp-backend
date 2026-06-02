'use client';

import { useLocale } from 'next-intl';
import { useCallback } from 'react';
import { SupportedLocale, LocalePreference } from '@/types/locale';
import { localeTelemetry } from '@/lib/telemetry/locale-telemetry';

/**
 * Hook for managing user locale preferences
 */
export function useLocalePreference() {
  const currentLocale = useLocale() as SupportedLocale;

  const updatePreference = useCallback(
    async (preference: LocalePreference): Promise<boolean> => {
      try {
        const response = await fetch('/api/v1/users/profile/settings', {
          method: 'PATCH',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(preference),
        });

        if (!response.ok) {
          throw new Error('Failed to update locale preference');
        }

        // Track locale change
        if (preference.locale && preference.locale !== currentLocale) {
          await localeTelemetry.trackLocaleSwitch(currentLocale, preference.locale);
        }

        return true;
      } catch (error) {
        console.error('Error updating locale preference:', error);
        return false;
      }
    },
    [currentLocale]
  );

  return {
    currentLocale,
    updatePreference,
  };
}
