# Aframp Frontend Architecture

## Overview

Production-grade frontend architecture built with React 19, Next.js 15 (App Router), TypeScript, and Tailwind CSS. Designed to match the high-performance, resilient capabilities of the Aframp Rust backend.

## Architecture Goals

✅ **Strict Multi-Tenant Directory Layout** - Dynamic theme injection based on host domain  
✅ **Clean Separation of Concerns** - Presentation components isolated from financial state machines  
✅ **Cross-Border Compliance Guards** - Client-side routing guards for KYB/KYC enforcement  
✅ **Global Internationalization** - Support for regional African locales  
✅ **Type Safety** - Zero `any` types, strict TypeScript compilation  
✅ **Performance Optimized** - Sub-1.5s First Contentful Paint target  

## Project Structure

```
web/
├── src/
│   ├── app/                    # Next.js App Router pages
│   │   ├── layout.tsx          # Root layout with providers
│   │   └── globals.css         # Global styles with CSS variables
│   ├── components/             # React components
│   │   └── ErrorBoundary.tsx   # Top-level error boundary
│   ├── contexts/               # React contexts
│   │   └── ThemeContext.tsx    # Multi-tenant theme provider
│   ├── hooks/                  # Custom React hooks
│   │   ├── useAuth.ts          # Authentication state
│   │   ├── useWalletBalance.ts # Wallet balance polling
│   │   └── useFXRate.ts        # Exchange rate tracking
│   ├── lib/                    # Core utilities
│   │   ├── api/
│   │   │   └── client.ts       # API client with JWT handling
│   │   ├── formatters/
│   │   │   └── currency.ts     # Currency formatting
│   │   ├── telemetry/
│   │   │   ├── webVitals.ts    # Core Web Vitals tracking
│   │   │   └── tracking.ts     # User session analytics
│   │   ├── utils/
│   │   │   └── idempotency.ts  # Duplicate submission prevention
│   │   └── query-client.tsx    # React Query configuration
│   ├── types/                  # TypeScript definitions
│   │   └── primitives.ts       # Core platform types
│   ├── config/                 # Configuration files
│   ├── middleware.ts           # Next.js middleware for route guards
│   └── i18n.ts                 # Internationalization config
├── tailwind.config.ts          # Tailwind CSS configuration
├── tsconfig.json               # TypeScript strict configuration
├── vitest.config.ts            # Vitest test configuration
├── biome.jsonc                 # Biome linter configuration
└── package.json                # Dependencies
```

## Core Features

### 1. Type-Safe Data Models

**Location:** `src/types/primitives.ts`

Comprehensive TypeScript definitions with strict discriminated unions:

- **Transaction Types**: Deposit, Withdrawal, Transfer, Exchange
- **Wallet Types**: Personal, Business, Merchant
- **Partner Profiles**: KYB status, tier management
- **Exchange Rates**: Real-time FX tracking
- **API Responses**: Normalized error handling

**Key Features:**
- Zero `any` types allowed
- Discriminated unions for type safety
- Regional locale support (NGN, KES, GHS, USD, etc.)

### 2. Client Network Core

**Location:** `src/lib/api/client.ts`

Robust API client with:

- ✅ Automatic JWT token attachment
- ✅ Silent token refresh on 401 responses
- ✅ Request queuing during token refresh
- ✅ Normalized error payloads
- ✅ Retry logic with exponential backoff

**Usage:**
```typescript
import { apiClient } from '@/lib/api/client';

const data = await apiClient.get<User>('/api/v1/auth/me');
```

### 3. Multi-Tenant Theme System

**Location:** `src/contexts/ThemeContext.tsx`

Dynamic brand identity injection based on verified host domain:

- ✅ Domain-based theme detection
- ✅ CSS variable injection
- ✅ Dynamic color schemes
- ✅ Configurable border radius
- ✅ Custom font families

**Supported Tenants:**
- `app.aframp.com` - Default theme
- `partner.aframp.com` - Partner portal theme
- `merchant.aframp.com` - Merchant dashboard theme

### 4. State Management

**Location:** `src/lib/query-client.tsx`, `src/hooks/`

TanStack React Query v5 with optimized caching:

- **Stale Time**: 1 minute default
- **Cache Time**: 5 minutes
- **Retry Strategy**: 3 attempts with exponential backoff
- **Refetch**: On window focus and reconnect

**Custom Hooks:**
- `useAuth()` - Authentication state and mutations
- `useWalletBalance()` - Real-time balance polling (60s interval)
- `useFXRate()` - Exchange rate tracking (2min interval)

### 5. Navigation Guards

**Location:** `src/middleware.ts`

Client-side route protection with:

- ✅ Authentication enforcement
- ✅ KYC status verification
- ✅ KYB compliance for partner routes
- ✅ Automatic redirect with return URL

**Protected Route Categories:**
- Public: `/auth/*`
- KYC Required: `/dashboard`, `/wallets`, `/transactions`
- KYB Required: `/partner/*`, `/merchant/*`

### 6. Observability

#### Web Vitals Tracking
**Location:** `src/lib/telemetry/webVitals.ts`

Captures Core Web Vitals and sends to backend:
- LCP (Largest Contentful Paint)
- FID (First Input Delay)
- CLS (Cumulative Layout Shift)
- FCP (First Contentful Paint)
- TTFB (Time to First Byte)

#### User Session Tracking
**Location:** `src/lib/telemetry/tracking.ts`

Tracks:
- Page views and navigation paths
- Failed API connections
- Payment flow abandonment
- Cross-border transaction bottlenecks

### 7. Error Handling

**Location:** `src/components/ErrorBoundary.tsx`

Top-level React Error Boundary:
- ✅ Catches runtime rendering errors
- ✅ Displays localized recovery UI
- ✅ Captures error context (pathname, user ID, stack trace)
- ✅ Sends error reports to backend
- ✅ Prevents complete UI stalls

### 8. Idempotency

**Location:** `src/lib/utils/idempotency.ts`

Prevents duplicate submissions:
- Request deduplication
- Double-click protection
- Idempotency key generation

## Configuration

### TypeScript (tsconfig.json)

Strict compilation rules:
```json
{
  "strict": true,
  "noImplicitAny": true,
  "strictNullChecks": true,
  "noUnusedLocals": true,
  "noUnusedParameters": true,
  "noImplicitReturns": true
}
```

### Tailwind CSS

CSS variables for dynamic theming:
```css
:root {
  --color-primary: #10b981;
  --color-primary-hover: #059669;
  --border-radius: 0.375rem;
  --font-family: Inter, system-ui, sans-serif;
}
```

### Biome Linter

Zero-tolerance for `any` types:
```json
{
  "linter": {
    "rules": {
      "suspicious": {
        "noExplicitAny": "error"
      }
    }
  }
}
```

## Testing

### Framework: Vitest + React Testing Library

**Configuration:** `vitest.config.ts`, `vitest.setup.ts`

**Test Coverage:**
- ✅ Unit tests for currency formatters
- ✅ Integration tests for middleware guards
- ✅ Mock API state machines
- ✅ Layout provider tests

**Run Tests:**
```bash
npm run test          # Run once
npm run test:watch    # Watch mode
npm run test:ui       # UI mode
```

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| First Contentful Paint (FCP) | < 1.5s | ✅ Configured |
| Largest Contentful Paint (LCP) | < 2.5s | ✅ Monitored |
| Time to Interactive (TTI) | < 3.5s | ✅ Optimized |
| Cumulative Layout Shift (CLS) | < 0.1 | ✅ Tracked |

## Internationalization

**Supported Locales:**
- `en-NG` - English (Nigeria)
- `en-KE` - English (Kenya)
- `en-GH` - English (Ghana)
- `en-ZA` - English (South Africa)
- `fr-CI` - French (Côte d'Ivoire)
- `sw-KE` - Swahili (Kenya)
- `yo-NG` - Yoruba (Nigeria)
- `ig-NG` - Igbo (Nigeria)
- `ha-NG` - Hausa (Nigeria)

## Installation

```bash
cd web
npm install
```

## Development

```bash
npm run dev           # Start dev server
npm run build         # Production build
npm run start         # Start production server
npm run lint          # Run linter
npm run lint:fix      # Fix linting issues
npm run type-check    # TypeScript check
```

## Deployment Checklist

- [ ] Environment variables configured
- [ ] API base URL set (`NEXT_PUBLIC_API_URL`)
- [ ] SSL certificates installed
- [ ] CDN configured for static assets
- [ ] Error tracking endpoint verified
- [ ] Web vitals endpoint verified
- [ ] Multi-tenant domains configured
- [ ] Locale files complete

## Acceptance Criteria Status

### Functional & Technical Requirements

✅ **Performance**: Sub-1.5s FCP target configured with monitoring  
✅ **Multi-Tenant Isolation**: Dynamic theme switching based on host domain  
✅ **Network Idempotency**: Double-click protection at state boundary  
✅ **TypeScript Strict Mode**: Zero implicit `any` types allowed  

### Observability & Quality Assurance

✅ **Error Boundary**: Context capture (pathname, user ID, stack traces)  
✅ **Responsive Layouts**: Tailwind CSS with mobile-first approach  
✅ **Unit Tests**: Currency formatters and utility functions  
✅ **Integration Tests**: Middleware guards and auth flows  

## Next Steps

1. **Install Dependencies**: Run `npm install` in the `web/` directory
2. **Configure Environment**: Copy `.env.example` to `.env.local` and set variables
3. **Run Tests**: Execute `npm run test` to verify setup
4. **Start Development**: Run `npm run dev` to start the dev server
5. **Build Pages**: Implement specific pages in `src/app/[locale]/`

## Support

For questions or issues, refer to:
- [Next.js Documentation](https://nextjs.org/docs)
- [React Query Documentation](https://tanstack.com/query/latest)
- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
