import type { RouteConfig, RouteAccessLevel, KYCTier } from '@/types/auth';

export const ROUTE_CONFIGS: Record<string, RouteConfig> = {
  '/login': {
    path: '/login',
    requiredAuth: false,
    redirectUnauthenticated: undefined,
  },
  '/signup': {
    path: '/signup',
    requiredAuth: false,
    redirectUnauthenticated: undefined,
  },
  '/dashboard': {
    path: '/dashboard',
    requiredAuth: true,
    redirectUnauthenticated: '/login',
  },
  '/wallet': {
    path: '/wallet',
    requiredAuth: true,
    requiredKYC: 'KYC_Level_1',
    redirectUnauthenticated: '/login',
    redirectUnauthorized: '/kyc/verify',
  },
  '/transactions': {
    path: '/transactions',
    requiredAuth: true,
    requiredKYC: 'KYC_Level_1',
    redirectUnauthenticated: '/login',
    redirectUnauthorized: '/kyc/verify',
  },
  '/exchange': {
    path: '/exchange',
    requiredAuth: true,
    requiredKYC: 'KYC_Level_2',
    redirectUnauthenticated: '/login',
    redirectUnauthorized: '/kyc/upgrade',
  },
  '/admin': {
    path: '/admin',
    requiredAuth: true,
    requiredKYC: 'Admin',
    allowedRoles: ['admin'],
    redirectUnauthenticated: '/login',
    redirectUnauthorized: '/dashboard',
  },
};

export function getRouteConfig(path: string): RouteConfig | undefined {
  return ROUTE_CONFIGS[path] || Object.values(ROUTE_CONFIGS).find(config => 
    path.startsWith(config.path)
  );
}

export function getAccessLevel(kycTier: KYCTier): RouteAccessLevel {
  switch (kycTier) {
    case 'Admin':
      return 'admin';
    case 'KYC_Level_2':
      return 'kyc_advanced';
    case 'KYC_Level_1':
      return 'kyc_verified';
    case 'Unverified':
      return 'authenticated';
    default:
      return 'guest';
  }
}
