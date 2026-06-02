# Authentication System - Quick Start Guide

## Installation

1. **Install dependencies:**
```bash
cd web
npm install
```

2. **Configure environment:**
```bash
cp .env.example .env.local
```

Edit `.env.local`:
```env
NEXT_PUBLIC_API_URL=http://localhost:8080
NEXT_PUBLIC_TELEMETRY_ENABLED=false
NODE_ENV=development
```

3. **Run development server:**
```bash
npm run dev
```

Visit `http://localhost:3000`

## Basic Usage

### 1. Protect a Page

```typescript
// app/[locale]/protected/page.tsx
'use client';

import { RequireAuth } from '@/components/auth/RequireAuth';

export default function ProtectedPage() {
  return (
    <RequireAuth>
      <div>This content requires authentication</div>
    </RequireAuth>
  );
}
```

### 2. Protect with KYC Level

```typescript
// app/[locale]/advanced/page.tsx
'use client';

import { RequireAuth } from '@/components/auth/RequireAuth';
import { RequireKYC } from '@/components/auth/RequireKYC';

export default function AdvancedPage() {
  return (
    <RequireAuth>
      <RequireKYC level="KYC_Level_2">
        <div>This requires KYC Level 2 verification</div>
      </RequireKYC>
    </RequireAuth>
  );
}
```

### 3. Use Auth State

```typescript
'use client';

import { useAuth } from '@/lib/auth/auth-context';

export default function MyComponent() {
  const { state, logout } = useAuth();

  if (state.status === 'AUTHENTICATED') {
    return (
      <div>
        <p>Welcome, {state.user.firstName}!</p>
        <p>KYC Status: {state.user.kycProfile.tier}</p>
        <button onClick={logout}>Logout</button>
      </div>
    );
  }

  return <div>Please log in</div>;
}
```

### 4. Login Form

```typescript
'use client';

import { useLoginMutation } from '@/hooks/useAuthMutations';
import { useState } from 'react';

export default function LoginForm() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const loginMutation = useLoginMutation();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await loginMutation.mutateAsync({ email, password });
      // Redirect handled automatically
    } catch (error) {
      console.error('Login failed:', error);
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
      />
      <input
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
      />
      <button type="submit" disabled={loginMutation.isPending}>
        {loginMutation.isPending ? 'Logging in...' : 'Login'}
      </button>
    </form>
  );
}
```

### 5. Make Authenticated API Calls

```typescript
import { apiClient } from '@/lib/auth/api-client';

// Token is automatically injected
async function fetchWalletBalance() {
  const response = await apiClient.get('/api/v1/wallet/balance');
  return response.data;
}

// Automatic token refresh on 401
async function makeTransaction(data: any) {
  const response = await apiClient.post('/api/v1/transactions', data);
  return response.data;
}
```

### 6. Check KYC Access

```typescript
'use client';

import { useAuth } from '@/lib/auth/auth-context';

export default function ConditionalFeature() {
  const { checkKYCAccess } = useAuth();

  const canAccessExchange = checkKYCAccess('KYC_Level_2');

  return (
    <div>
      {canAccessExchange ? (
        <button>Access Exchange</button>
      ) : (
        <div>
          <p>Upgrade to KYC Level 2 to access exchange</p>
          <a href="/kyc/upgrade">Upgrade Now</a>
        </div>
      )}
    </div>
  );
}
```

## Testing

### Run All Tests
```bash
npm test
```

### Run Specific Test Suite
```bash
npm test -- auth-context
```

### Run Integration Tests
```bash
npm test -- integration
```

### Watch Mode
```bash
npm run test:watch
```

## Common Patterns

### Loading States

```typescript
const { state, isLoading } = useAuth();

if (state.status === 'INITIALIZING' || isLoading) {
  return <LoadingSpinner />;
}
```

### Error Handling

```typescript
const loginMutation = useLoginMutation();

try {
  await loginMutation.mutateAsync(credentials);
} catch (error: any) {
  if (error.code === 'INVALID_CREDENTIALS') {
    setError('Invalid email or password');
  } else if (error.code === 'ACCOUNT_LOCKED') {
    setError('Your account has been locked');
  } else {
    setError('An error occurred. Please try again.');
  }
}
```

### Conditional Rendering

```typescript
const { state } = useAuth();

return (
  <>
    {state.status === 'AUTHENTICATED' && (
      <UserMenu user={state.user} />
    )}
    {state.status === 'UNAUTHENTICATED' && (
      <LoginButton />
    )}
  </>
);
```

## Route Configuration

Edit `src/lib/auth/route-config.ts` to add protected routes:

```typescript
export const ROUTE_CONFIGS: Record<string, RouteConfig> = {
  '/my-new-page': {
    path: '/my-new-page',
    requiredAuth: true,
    requiredKYC: 'KYC_Level_1',
    redirectUnauthenticated: '/login',
    redirectUnauthorized: '/kyc/verify',
  },
};
```

## Troubleshooting

### Session Not Persisting
- Check that cookies are enabled in browser
- Verify `NEXT_PUBLIC_API_URL` is correct
- Check browser console for errors

### Token Refresh Failing
- Verify backend `/api/v1/auth/refresh` endpoint is working
- Check refresh token is being stored correctly
- Review network tab for 401 responses

### Multi-Tab Sync Not Working
- Broadcast Channel API requires same origin
- Check browser compatibility
- Verify no errors in console

### Tests Failing
- Run `npm install` to ensure all dependencies are installed
- Clear jest cache: `npm test -- --clearCache`
- Check mock implementations are correct

## API Endpoints Required

Your backend must implement these endpoints:

- `POST /api/v1/auth/login` - User login
- `POST /api/v1/auth/signup` - User registration  
- `POST /api/v1/auth/logout` - User logout
- `POST /api/v1/auth/refresh` - Token refresh
- `POST /api/v1/auth/password-reset` - Request password reset
- `POST /api/v1/auth/password-reset/confirm` - Confirm password reset

## Security Checklist

- [ ] HTTPS enabled in production
- [ ] CORS configured correctly
- [ ] Rate limiting on auth endpoints
- [ ] Strong password requirements
- [ ] Account lockout after failed attempts
- [ ] Email verification enabled
- [ ] MFA available for sensitive operations
- [ ] Session timeout configured
- [ ] Audit logging enabled

## Production Deployment

1. **Environment Variables:**
```env
NEXT_PUBLIC_API_URL=https://api.aframp.com
NEXT_PUBLIC_TELEMETRY_ENABLED=true
NODE_ENV=production
```

2. **Build:**
```bash
npm run build
```

3. **Start:**
```bash
npm start
```

4. **Verify:**
- Test login/logout flow
- Verify token refresh works
- Check multi-tab synchronization
- Test route protection
- Validate KYC access control

## Support

For issues or questions:
1. Check `AUTH_IMPLEMENTATION.md` for detailed documentation
2. Review `IMPLEMENTATION_STATUS.md` for feature status
3. Check test files for usage examples
4. Review browser console for errors

## Next Steps

1. Integrate with your backend API
2. Customize UI components
3. Add additional auth methods (OAuth, biometric)
4. Implement MFA flow
5. Build KYC verification pages
6. Add analytics and monitoring
