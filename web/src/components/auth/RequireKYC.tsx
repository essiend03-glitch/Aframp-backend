'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { useAuth } from '@/lib/auth/auth-context';
import type { KYCTier } from '@/types/auth';

interface RequireKYCProps {
  children: React.ReactNode;
  level: KYCTier;
  redirectTo?: string;
  fallback?: React.ReactNode;
}

export function RequireKYC({ 
  children, 
  level, 
  redirectTo = '/kyc/verify',
  fallback 
}: RequireKYCProps) {
  const { state, checkKYCAccess } = useAuth();
  const router = useRouter();

  const hasAccess = checkKYCAccess(level);

  useEffect(() => {
    if (state.status === 'AUTHENTICATED' && !hasAccess) {
      router.push(redirectTo);
    }
  }, [state.status, hasAccess, router, redirectTo]);

  if (state.status === 'INITIALIZING') {
    return <div>Loading...</div>;
  }

  if (!hasAccess) {
    return fallback ? <>{fallback}</> : null;
  }

  return <>{children}</>;
}
