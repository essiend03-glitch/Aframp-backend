'use client';

import React, { createContext, useContext } from 'react';
import { InstitutionalRole, hasPermission } from '@/types';

interface RbacContextValue {
  role: InstitutionalRole;
  userId: string;
  userName: string;
}

const RbacContext = createContext<RbacContextValue | null>(null);

export function RbacProvider({
  role,
  userId,
  userName,
  children,
}: RbacContextValue & { children: React.ReactNode }) {
  return (
    <RbacContext.Provider value={{ role, userId, userName }}>
      {children}
    </RbacContext.Provider>
  );
}

export function useRbac() {
  const ctx = useContext(RbacContext);
  if (!ctx) throw new Error('useRbac must be used within RbacProvider');
  return ctx;
}

interface RbacGateProps {
  permission: string;
  /** Rendered when permission is denied. Default: null (hidden) */
  fallback?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * <RbacGate permission="proposals:create">
 *   <CreateProposalButton />
 * </RbacGate>
 *
 * Renders children only if the current user's role grants the given permission.
 */
export function RbacGate({ permission, fallback = null, children }: RbacGateProps) {
  const { role } = useRbac();
  if (!hasPermission(role, permission)) return <>{fallback}</>;
  return <>{children}</>;
}
