import { getRequestConfig } from 'next-intl/server';
import { notFound } from 'next/navigation';
import { SUPPORTED_LOCALES } from '@/config/locales';

export default getRequestConfig(async ({ locale }) => {
  // Validate that the incoming `locale` parameter is valid
  if (!SUPPORTED_LOCALES.includes(locale as any)) {
    notFound();
  }

  return {
    messages: (await import(`./messages/${locale}.json`)).default,
  };
});
