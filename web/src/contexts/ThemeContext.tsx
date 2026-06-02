/**
 * Multi-Tenant Theme Context
 * Dynamic brand identity injection based on verified host domain
 */

'use client';

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';

// ============================================================================
// Theme Types
// ============================================================================

export interface ThemeColors {
  primary: string;
  primaryHover: string;
  secondary: string;
  accent: string;
  background: string;
  surface: string;
  text: string;
  textSecondary: string;
  border: string;
  error: string;
  success: string;
  warning: string;
}

export interface ThemeConfig {
  tenantId: string;
  brandName: string;
  logo: string;
  favicon: string;
  colors: ThemeColors;
  borderRadius: 'none' | 'sm' | 'md' | 'lg' | 'xl';
  fontFamily: string;
}

// ============================================================================
// Default Themes by Domain
// ============================================================================

const DEFAULT_THEME: ThemeConfig = {
  tenantId: 'default',
  brandName: 'Aframp',
  logo: '/logos/aframp.svg',
  favicon: '/favicons/aframp.ico',
  colors: {
    primary: '#10B981',
    primaryHover: '#059669',
    secondary: '#6366F1',
    accent: '#F59E0B',
    background: '#FFFFFF',
    surface: '#F9FAFB',
    text: '#111827',
    textSecondary: '#6B7280',
    border: '#E5E7EB',
    error: '#EF4444',
    success: '#10B981',
    warning: '#F59E0B',
  },
  borderRadius: 'md',
  fontFamily: 'Inter, system-ui, sans-serif',
};

const TENANT_THEMES: Record<string, ThemeConfig> = {
  'app.aframp.com': DEFAULT_THEME,
  'partner.aframp.com': {
    ...DEFAULT_THEME,
    tenantId: 'partner',
    brandName: 'Aframp Partner',
    colors: {
      ...DEFAULT_THEME.colors,
      primary: '#6366F1',
      primaryHover: '#4F46E5',
    },
    borderRadius: 'lg',
  },
  'merchant.aframp.com': {
    ...DEFAULT_THEME.colors,
    tenantId: 'merchant',
    brandName: 'Aframp Merchant',
    logo: '/logos/aframp-merchant.svg',
    favicon: '/favicons/merchant.ico',
    colors: {
      ...DEFAULT_THEME.colors,
      primary: '#8B5CF6',
      primaryHover: '#7C3AED',
    },
    borderRadius: 'sm',
  },
  'localhost:3000': DEFAULT_THEME,
};

// ============================================================================
// Theme Context
// ============================================================================

interface ThemeContextValue {
  theme: ThemeConfig;
  setTheme: (theme: ThemeConfig) => void;
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within ThemeProvider');
  }
  return context;
}

// ============================================================================
// Theme Provider
// ============================================================================

interface ThemeProviderProps {
  children: ReactNode;
  initialTheme?: ThemeConfig;
}

export function ThemeProvider({ children, initialTheme }: ThemeProviderProps): JSX.Element {
  const [theme, setTheme] = useState<ThemeConfig>(initialTheme || DEFAULT_THEME);

  useEffect(() => {
    // Detect tenant from hostname
    if (typeof window !== 'undefined') {
      const hostname = window.location.hostname;
      const port = window.location.port;
      const hostKey = port ? `${hostname}:${port}` : hostname;
      
      const tenantTheme = TENANT_THEMES[hostKey] || DEFAULT_THEME;
      setTheme(tenantTheme);
      
      // Apply theme to CSS variables
      applyThemeVariables(tenantTheme);
    }
  }, []);

  useEffect(() => {
    applyThemeVariables(theme);
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

// ============================================================================
// Theme Application
// ============================================================================

function applyThemeVariables(theme: ThemeConfig): void {
  if (typeof document === 'undefined') return;

  const root = document.documentElement;

  // Apply color variables
  Object.entries(theme.colors).forEach(([key, value]) => {
    root.style.setProperty(`--color-${kebabCase(key)}`, value);
  });

  // Apply border radius
  const radiusMap = {
    none: '0',
    sm: '0.25rem',
    md: '0.375rem',
    lg: '0.5rem',
    xl: '0.75rem',
  };
  root.style.setProperty('--border-radius', radiusMap[theme.borderRadius]);

  // Apply font family
  root.style.setProperty('--font-family', theme.fontFamily);

  // Update favicon
  const favicon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (favicon) {
    favicon.href = theme.favicon;
  }

  // Update page title
  document.title = theme.brandName;
}

function kebabCase(str: string): string {
  return str.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
}
