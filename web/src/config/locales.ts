import { LocaleConfig, SupportedLocale } from '@/types/locale';

/**
 * Locale configurations for all supported languages
 */
export const LOCALE_CONFIGS: Record<SupportedLocale, LocaleConfig> = {
  en: {
    code: 'en',
    name: 'English',
    nativeName: 'English',
    direction: 'ltr',
    timezone: 'Africa/Lagos',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ',',
      decimalSeparator: '.',
      groupSize: 3,
    },
  },
  fr: {
    code: 'fr',
    name: 'French',
    nativeName: 'Français',
    direction: 'ltr',
    timezone: 'Africa/Abidjan',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ' ',
      decimalSeparator: ',',
      groupSize: 3,
    },
  },
  ha: {
    code: 'ha',
    name: 'Hausa',
    nativeName: 'Hausa',
    direction: 'ltr',
    timezone: 'Africa/Lagos',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ',',
      decimalSeparator: '.',
      groupSize: 3,
    },
  },
  yo: {
    code: 'yo',
    name: 'Yoruba',
    nativeName: 'Yorùbá',
    direction: 'ltr',
    timezone: 'Africa/Lagos',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ',',
      decimalSeparator: '.',
      groupSize: 3,
    },
  },
  ig: {
    code: 'ig',
    name: 'Igbo',
    nativeName: 'Igbo',
    direction: 'ltr',
    timezone: 'Africa/Lagos',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ',',
      decimalSeparator: '.',
      groupSize: 3,
    },
  },
  sw: {
    code: 'sw',
    name: 'Swahili',
    nativeName: 'Kiswahili',
    direction: 'ltr',
    timezone: 'Africa/Nairobi',
    dateFormat: 'dd/MM/yyyy',
    timeFormat: 'HH:mm:ss',
    firstDayOfWeek: 1,
    numberGrouping: {
      groupSeparator: ',',
      decimalSeparator: '.',
      groupSize: 3,
    },
  },
};

export const DEFAULT_LOCALE: SupportedLocale = 'en';
export const SUPPORTED_LOCALES = Object.keys(LOCALE_CONFIGS) as SupportedLocale[];
