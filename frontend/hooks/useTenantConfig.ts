// Issue #482 — Multi-Tenant Whitelabel Core
// Client-side tenant config hook with aggressive caching and CSS variable injection.

import { useEffect, useState } from 'react';
import type { TenantConfig, WhitelabelTheme } from '../types';
import { DEFAULT_TENANT_CONFIG } from '../types';

const CACHE_KEY = 'aframp_tenant_cfg';
const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

interface CachedConfig { config: TenantConfig; cachedAt: number }

function isValidHex(color: string): boolean {
  return /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(color);
}

function isValidColor(color: string): boolean {
  return isValidHex(color) || color.startsWith('rgba(') || color.startsWith('rgb(');
}

/** Inject theme tokens as CSS custom properties on :root */
function injectTheme(theme: WhitelabelTheme) {
  const root = document.documentElement;
  const safe = (val: string, fallback: string) => (isValidColor(val) ? val : fallback);

  root.style.setProperty('--color-primary', safe(theme.primaryColor, '#3fb950'));
  root.style.setProperty('--color-secondary', safe(theme.secondaryColor, '#58a6ff'));
  root.style.setProperty('--color-accent', safe(theme.accentColor, '#d2a8ff'));
  root.style.setProperty('--color-bg', safe(theme.backgroundColor, '#0d1117'));
  root.style.setProperty('--color-text', safe(theme.textColor, '#c9d1d9'));
  root.style.setProperty('--font-family', theme.fontFamily || 'Inter, system-ui, sans-serif');
}

function readCache(): TenantConfig | null {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const { config, cachedAt }: CachedConfig = JSON.parse(raw);
    if (Date.now() - cachedAt > CACHE_TTL_MS) return null;
    return config;
  } catch { return null; }
}

function writeCache(config: TenantConfig) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ config, cachedAt: Date.now() }));
  } catch { /* storage quota exceeded — ignore */ }
}

export function useTenantConfig() {
  const [config, setConfig] = useState<TenantConfig>(DEFAULT_TENANT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const cached = readCache();
    if (cached) {
      setConfig(cached);
      injectTheme(cached.theme);
      setLoading(false);
      return;
    }

    fetch('/api/v1/tenant/config')
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<TenantConfig>;
      })
      .then((data) => {
        writeCache(data);
        setConfig(data);
        injectTheme(data.theme);
      })
      .catch((err) => {
        setError(err.message);
        // Graceful fallback — use defaults so UI never breaks
        injectTheme(DEFAULT_TENANT_CONFIG.theme);
      })
      .finally(() => setLoading(false));
  }, []);

  return { config, loading, error };
}
