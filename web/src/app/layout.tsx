/**
 * Root Layout
 * Global providers and configuration
 */

import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import { QueryProvider } from '@/lib/query-client';
import { ThemeProvider } from '@/contexts/ThemeContext';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { initWebVitals } from '@/lib/telemetry/webVitals';
import { initTracking } from '@/lib/telemetry/tracking';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Aframp - Cross-Border Payments Platform',
  description: 'High-performance cross-border payment infrastructure for Africa',
  viewport: 'width=device-width, initial-scale=1',
  themeColor: '#10B981',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}): JSX.Element {
  // Initialize telemetry on client side
  if (typeof window !== 'undefined') {
    initWebVitals();
    initTracking();
  }

  return (
    <html lang="en" suppressHydrationWarning>
      <body className={inter.className}>
        <ErrorBoundary>
          <QueryProvider>
            <ThemeProvider>
              {children}
            </ThemeProvider>
          </QueryProvider>
        </ErrorBoundary>
      </body>
    </html>
  );
}
