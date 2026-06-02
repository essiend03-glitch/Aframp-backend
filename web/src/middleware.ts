/**
 * Next.js Middleware
 * Client-side navigation guards for KYB/KYC enforcement and route protection
 */

import { NextResponse, type NextRequest } from 'next/server';
import createMiddleware from 'next-intl/middleware';
import { locales, defaultLocale } from './config/locales';

// ============================================================================
// Route Protection Configuration
// ============================================================================

const PUBLIC_ROUTES = [
  '/auth/login',
  '/auth/register',
  '/auth/forgot-password',
  '/auth/reset-password',
];

const KYC_REQUIRED_ROUTES = [
  '/dashboard',
  '/wallets',
  '/transactions',
  '/exchange',
  '/send',
  '/receive',
];

const KYB_REQUIRED_ROUTES = [
  '/partner',
  '/merchant',
  '/api-keys',
  '/webhooks',
];

// ============================================================================
// Internationalization Middleware
// ============================================================================

const intlMiddleware = createMiddleware({
  locales,
  defaultLocale,
  localePrefix: 'as-needed',
});

// ============================================================================
// Main Middleware
// ============================================================================

export default async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // Apply internationalization
  const response = intlMiddleware(request);

  // Extract locale from pathname
  const pathnameLocale = locales.find(
    (locale) => pathname.startsWith(`/${locale}/`) || pathname === `/${locale}`
  );
  const pathWithoutLocale = pathnameLocale
    ? pathname.slice(`/${pathnameLocale}`.length) || '/'
    : pathname;

  // Check authentication
  const accessToken = request.cookies.get('aframp_access_token')?.value;
  const isAuthenticated = !!accessToken;

  // Public routes - allow access
  if (PUBLIC_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    return response;
  }

  // Protected routes - require authentication
  if (!isAuthenticated) {
    const loginUrl = new URL(
      pathnameLocale ? `/${pathnameLocale}/auth/login` : '/auth/login',
      request.url
    );
    loginUrl.searchParams.set('redirect', pathname);
    return NextResponse.redirect(loginUrl);
  }

  // KYC enforcement
  if (KYC_REQUIRED_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    const kycStatus = request.cookies.get('aframp_kyc_status')?.value;
    
    if (kycStatus !== 'approved') {
      const kycUrl = new URL(
        pathnameLocale ? `/${pathnameLocale}/onboarding/kyc` : '/onboarding/kyc',
        request.url
      );
      return NextResponse.redirect(kycUrl);
    }
  }

  // KYB enforcement for partner/merchant routes
  if (KYB_REQUIRED_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    const kybStatus = request.cookies.get('aframp_kyb_status')?.value;
    
    if (kybStatus !== 'approved') {
      const kybUrl = new URL(
        pathnameLocale ? `/${pathnameLocale}/onboarding/kyb` : '/onboarding/kyb',
        request.url
      );
      return NextResponse.redirect(kybUrl);
    }
  }

  return response;
}

export const config = {
  matcher: [
    // Match all pathnames except for
    // - … if they start with `/api`, `/_next` or `/_vercel`
    // - … the ones containing a dot (e.g. `favicon.ico`)
    '/((?!api|_next|_vercel|.*\\..*).*)',
  ],
};
