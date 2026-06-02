import { NextIntlClientProvider } from 'next-intl';
import { getMessages } from 'next-intl/server';
import { notFound } from 'next/navigation';
import { SUPPORTED_LOCALES, LOCALE_CONFIGS } from '@/config/locales';
import { SupportedLocale } from '@/types/locale';
import { QueryProvider } from '@/lib/query-client';
import { AuthProvider } from '@/lib/auth/auth-context';
import { SessionTimeoutWarning } from '@/components/auth/SessionTimeoutWarning';

export function generateStaticParams() {
  return SUPPORTED_LOCALES.map((locale) => ({ locale }));
}

export default async function LocaleLayout({
  children,
  params: { locale },
}: {
  children: React.ReactNode;
  params: { locale: string };
}) {
  // Validate locale
  if (!SUPPORTED_LOCALES.includes(locale as SupportedLocale)) {
    notFound();
  }

  const messages = await getMessages();
  const config = LOCALE_CONFIGS[locale as SupportedLocale];

  return (
    <html lang={locale} dir={config.direction}>
      <body>
        <QueryProvider>
          <AuthProvider>
            <NextIntlClientProvider messages={messages}>
              {children}
              <SessionTimeoutWarning />
            </NextIntlClientProvider>
          </AuthProvider>
        </QueryProvider>
      </body>
    </html>
  );
}
