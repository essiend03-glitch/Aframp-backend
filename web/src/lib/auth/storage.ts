/**
 * Secure Storage Layer for Authentication
 * Implements high-security client-side storage with HttpOnly cookies via server actions
 */

import type { AuthSession, SecureStorageAdapter, TokenContainer } from '@/types/auth';

const SESSION_COOKIE_NAME = '__aframp_session';
const REFRESH_TOKEN_COOKIE_NAME = '__aframp_refresh';

// ============================================================================
// Server Actions for Secure Cookie Management
// ============================================================================

/**
 * Server action to set session cookie (HttpOnly, Secure, SameSite=Strict)
 */
export async function setSessionCookie(session: AuthSession): Promise<void> {
  'use server';
  const { cookies } = await import('next/headers');
  const cookieStore = await cookies();
  
  cookieStore.set(SESSION_COOKIE_NAME, JSON.stringify(session), {
    httpOnly: true,
    secure: process.env.NODE_ENV === 'production',
    sameSite: 'strict',
    maxAge: 60 * 60 * 24 * 7, // 7 days
    path: '/',
  });
}

/**
 * Server action to get session from cookie
 */
export async function getSessionCookie(): Promise<AuthSession | null> {
  'use server';
  const { cookies } = await import('next/headers');
  const cookieStore = await cookies();
  
  const sessionCookie = cookieStore.get(SESSION_COOKIE_NAME);
  if (!sessionCookie?.value) return null;
  
  try {
    return JSON.parse(sessionCookie.value) as AuthSession;
  } catch {
    return null;
  }
}

/**
 * Server action to clear session cookie
 */
export async function clearSessionCookie(): Promise<void> {
  'use server';
  const { cookies } = await import('next/headers');
  const cookieStore = await cookies();
  
  cookieStore.delete(SESSION_COOKIE_NAME);
  cookieStore.delete(REFRESH_TOKEN_COOKIE_NAME);
}

/**
 * Server action to set refresh token cookie
 */
export async function setRefreshTokenCookie(token: string): Promise<void> {
  'use server';
  const { cookies } = await import('next/headers');
  const cookieStore = await cookies();
  
  cookieStore.set(REFRESH_TOKEN_COOKIE_NAME, token, {
    httpOnly: true,
    secure: process.env.NODE_ENV === 'production',
    sameSite: 'strict',
    maxAge: 60 * 60 * 24 * 30, // 30 days
    path: '/',
  });
}

/**
 * Server action to get refresh token from cookie
 */
export async function getRefreshTokenCookie(): Promise<string | null> {
  'use server';
  const { cookies } = await import('next/headers');
  const cookieStore = await cookies();
  
  const tokenCookie = cookieStore.get(REFRESH_TOKEN_COOKIE_NAME);
  return tokenCookie?.value || null;
}

// ============================================================================
// Client-Side Storage Adapter
// ============================================================================

/**
 * Secure storage adapter for client-side auth state management
 * Uses server actions for HttpOnly cookie operations
 */
export class SecureStorage implements SecureStorageAdapter {
  private memoryCache: {
    session: AuthSession | null;
    refreshToken: string | null;
  } = {
    session: null,
    refreshToken: null,
  };

  async getSession(): Promise<AuthSession | null> {
    // Return from memory cache if available
    if (this.memoryCache.session) {
      return this.memoryCache.session;
    }

    // Fetch from server-side cookie
    try {
      const session = await getSessionCookie();
      this.memoryCache.session = session;
      return session;
    } catch (error) {
      console.error('Failed to retrieve session:', error);
      return null;
    }
  }

  async setSession(session: AuthSession): Promise<void> {
    this.memoryCache.session = session;
    
    try {
      await setSessionCookie(session);
    } catch (error) {
      console.error('Failed to store session:', error);
      throw new Error('Session storage failed');
    }
  }

  async clearSession(): Promise<void> {
    this.memoryCache.session = null;
    this.memoryCache.refreshToken = null;
    
    try {
      await clearSessionCookie();
    } catch (error) {
      console.error('Failed to clear session:', error);
    }
  }

  async getRefreshToken(): Promise<string | null> {
    if (this.memoryCache.refreshToken) {
      return this.memoryCache.refreshToken;
    }

    try {
      const token = await getRefreshTokenCookie();
      this.memoryCache.refreshToken = token;
      return token;
    } catch (error) {
      console.error('Failed to retrieve refresh token:', error);
      return null;
    }
  }

  async setRefreshToken(token: string): Promise<void> {
    this.memoryCache.refreshToken = token;
    
    try {
      await setRefreshTokenCookie(token);
    } catch (error) {
      console.error('Failed to store refresh token:', error);
      throw new Error('Refresh token storage failed');
    }
  }

  /**
   * Clear memory cache (useful for tab synchronization)
   */
  clearMemoryCache(): void {
    this.memoryCache.session = null;
    this.memoryCache.refreshToken = null;
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

export const secureStorage = new SecureStorage();
