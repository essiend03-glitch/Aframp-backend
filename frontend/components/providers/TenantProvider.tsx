// Issue #482 — TenantProvider: distributes brand metadata, feature flags,
// and compliance rules across the component tree.

'use client';

import React, { createContext, useContext } from 'react';
import { useTenantConfig } from '../../hooks/useTenantConfig';
import type { TenantConfig } from '../../types';
import { DEFAULT_TENANT_CONFIG } from '../../types';

const TenantContext = createContext<TenantConfig>(DEFAULT_TENANT_CONFIG);

export function TenantProvider({ children }: { children: React.ReactNode }) {
  const { config } = useTenantConfig();
  return <TenantContext.Provider value={config}>{children}</TenantContext.Provider>;
}

export function useTenant() {
  return useContext(TenantContext);
}

/** Feature flag gate — renders children only when flag is enabled */
export function FeatureGate({ flag, children }: { flag: keyof TenantConfig['features']; children: React.ReactNode }) {
  const { features } = useTenant();
  return features[flag] ? <>{children}</> : null;
}
