// Issue #482 — Next.js Edge Middleware: Tenant Resolution
// Extracts tenant_id from host header or X-Partner-Domain and injects it
// into request headers for downstream consumption. Runs at the edge — zero FOUC.

import { NextRequest, NextResponse } from 'next/server';

const TENANT_MAP: Record<string, string> = {
  'app.aframp.io': 'aframp',
  'pay.zenithbank.com': 'zenith',
  'remit.uba.africa': 'uba',
  // Additional tenants loaded from KV store in production
};

function resolveTenantId(req: NextRequest): string {
  // 1. Explicit partner header (B2B API calls)
  const partnerDomain = req.headers.get('x-partner-domain');
  if (partnerDomain && TENANT_MAP[partnerDomain]) return TENANT_MAP[partnerDomain];

  // 2. Host-based resolution
  const host = req.headers.get('host') ?? '';
  const cleanHost = host.split(':')[0]; // strip port
  if (TENANT_MAP[cleanHost]) return TENANT_MAP[cleanHost];

  // 3. Subdomain extraction: zenith.aframp.io → zenith
  const subdomain = cleanHost.split('.')[0];
  if (subdomain && subdomain !== 'www' && subdomain !== 'app') return subdomain;

  return 'default';
}

export function middleware(req: NextRequest) {
  const tenantId = resolveTenantId(req);
  const res = NextResponse.next();
  // Inject tenant_id for server components and API routes
  res.headers.set('x-tenant-id', tenantId);
  return res;
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
