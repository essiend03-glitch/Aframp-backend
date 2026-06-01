'use client';

import { useEffect } from 'react';
import { useRouter, usePathname } from 'next/navigation';
import { useAuth } from '@/lib/auth/auth-context';

interface RequireAuthProps {
  children: React.ReactNode;
  redirectTo?: string;
}

export function RequireAuth({ children, redirectTo = '/login' }: RequireAuthProps) {
  const { state } = useAuth();
  const router = useRouter();
  const pathname = usePathname();

  useEffect(() => {
    if (state.status === 'UNAUTHENTICATED' || state.status === 'SESSION_EXPIRED') {
      const url = `${redirectTo}?redirect=${encodeURIComponent(pathname)}`;
      router.push(url);
    }
  }, [state.status, router, redirectTo, pathname]);

  if (state.status === 'INITIALIZING') {
    return <div>Loading...</div>;
  }

  if (state.status !== 'AUTHENTICATED') {
    return null;
  }

  return <>{children}</>;
}
