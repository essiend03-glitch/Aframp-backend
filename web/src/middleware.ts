import createIntlMiddleware from 'next-intl/middleware';
import { NextRequest, NextResponse } from 'next/server';
import { SUPPORTED_LOCALES, DEFAULT_LOCALE } from '@/config/locales';

const intlMiddleware = createIntlMiddleware({
  locales: SUPPORTED_LOCALES,
  defaultLocale: DEFAULT_LOCALE,
  localeDetection: true,
  localePrefix: 'always',
});

const PUBLIC_PATHS = ['/login', '/signup', '/forgot-password', '/reset-password'];
const PROTECTED_PATHS = ['/dashboard', '/wallet', '/transactions', '/exchange', '/admin'];

export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  
  // Extract locale from pathname
  const pathnameLocale = SUPPORTED_LOCALES.find(
    locale => pathname.startsWith(`/${locale}/`) || pathname === `/${locale}`
  );
  
  // Get path without locale prefix
  const pathWithoutLocale = pathnameLocale 
    ? pathname.slice(`/${pathnameLocale}`.length) || '/'
    : pathname;

  // Check if path requires authentication
  const isProtectedPath = PROTECTED_PATHS.some(path => pathWithoutLocale.startsWith(path));
  const isPublicPath = PUBLIC_PATHS.some(path => pathWithoutLocale.startsWith(path));

  // Get session from cookie
  const sessionCookie = request.cookies.get('__aframp_session');
  const hasSession = !!sessionCookie?.value;

  // Redirect unauthenticated users from protected paths
  if (isProtectedPath && !hasSession) {
    const locale = pathnameLocale || DEFAULT_LOCALE;
    const loginUrl = new URL(`/${locale}/login`, request.url);
    loginUrl.searchParams.set('redirect', pathWithoutLocale);
    return NextResponse.redirect(loginUrl);
  }

  // Redirect authenticated users from public auth pages
  if (isPublicPath && hasSession && (pathWithoutLocale === '/login' || pathWithoutLocale === '/signup')) {
    const locale = pathnameLocale || DEFAULT_LOCALE;
    return NextResponse.redirect(new URL(`/${locale}/dashboard`, request.url));
  }

  // Apply internationalization middleware
  return intlMiddleware(request);
}

export const config = {
  matcher: ['/', '/(en|fr|ha|yo|ig|sw)/:path*'],
};
