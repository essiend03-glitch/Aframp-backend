/**
 * Middleware Tests
 * Integration tests for navigation guards and route protection
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { NextRequest, NextResponse } from 'next/server';
import middleware from './middleware';

describe('Middleware - Route Protection', () => {
  it('allows access to public routes without authentication', async () => {
    const request = new NextRequest(new URL('http://localhost:3000/auth/login'));
    const response = await middleware(request);
    
    expect(response.status).not.toBe(307); // Not redirected
  });

  it('redirects unauthenticated users from protected routes', async () => {
    const request = new NextRequest(new URL('http://localhost:3000/dashboard'));
    const response = await middleware(request);
    
    expect(response.status).toBe(307); // Redirected
    expect(response.headers.get('location')).toContain('/auth/login');
  });

  it('allows authenticated users to access protected routes', async () => {
    const request = new NextRequest(new URL('http://localhost:3000/dashboard'));
    request.cookies.set('aframp_access_token', 'mock-token');
    request.cookies.set('aframp_kyc_status', 'approved');
    
    const response = await middleware(request);
    
    expect(response.status).not.toBe(307);
  });

  it('enforces KYC for KYC-required routes', async () => {
    const request = new NextRequest(new URL('http://localhost:3000/wallets'));
    request.cookies.set('aframp_access_token', 'mock-token');
    request.cookies.set('aframp_kyc_status', 'pending');
    
    const response = await middleware(request);
    
    expect(response.status).toBe(307);
    expect(response.headers.get('location')).toContain('/onboarding/kyc');
  });

  it('enforces KYB for partner routes', async () => {
    const request = new NextRequest(new URL('http://localhost:3000/partner/dashboard'));
    request.cookies.set('aframp_access_token', 'mock-token');
    request.cookies.set('aframp_kyb_status', 'pending');
    
    const response = await middleware(request);
    
    expect(response.status).toBe(307);
    expect(response.headers.get('location')).toContain('/onboarding/kyb');
  });
});
