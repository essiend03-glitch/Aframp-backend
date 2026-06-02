'use client';

/**
 * Authentication Context Provider
 * Centralized auth state management with React Context
 */

import React, { createContext, useContext, useEffect, useState, useCallback, useRef } from 'react';
import type {
  AuthState,
  AuthContextValue,
  LoginPayload,
  SignupPayload,
  PasswordResetPayload,
  PasswordResetConfirmPayload,
  MFASetupPayload,
  MFAVerifyPayload,
  KYCTier,
  AuthSession,
  AuthResponse,
} from '@/types/auth';
import { apiClient } from './api-client';
import { secureStorage } from './storage';
import { tabSync, type AuthSyncMessage } from './tab-sync';
import { emitAuthTelemetry } from './telemetry';

// ============================================================================
// Context Definition
// ============================================================================

const AuthContext = createContext<AuthContextValue | null>(null);

// ============================================================================
// KYC Tier Hierarchy
// ============================================================================

const KYC_TIER_HIERARCHY: Record<KYCTier, number> = {
  'Unverified': 0,
  'KYC_Level_1': 1,
  'KYC_Level_2': 2,
  'Admin': 3,
};

// ============================================================================
// Auth Provider Component
// ============================================================================

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<AuthState>({
    status: 'INITIALIZING',
    user: null,
    error: null,
  });
  const [isLoading, setIsLoading] = useState(false);
  const initializationRef = useRef(false);

  // ============================================================================
  // Session Initialization
  // ============================================================================

  const initializeSession = useCallback(async () => {
    if (initializationRef.current) return;
    initializationRef.current = true;

    try {
      const session = await secureStorage.getSession();

      if (!session) {
        setState({ status: 'UNAUTHENTICATED', user: null, error: null });
        return;
      }

      // Check if token is expired
      const now = Date.now();
      if (session.tokens.expiresAt < now) {
        // Attempt silent refresh
        try {
          await refreshSession();
        } catch {
          setState({ status: 'UNAUTHENTICATED', user: null, error: null });
        }
        return;
      }

      setState({ status: 'AUTHENTICATED', user: session.user, error: null });
      emitAuthTelemetry({
        eventType: 'SESSION_START',
        timestamp: now,
        metadata: { source: 'initialization' },
      });
    } catch (error) {
      console.error('Session initialization failed:', error);
      setState({ status: 'UNAUTHENTICATED', user: null, error: null });
    }
  }, []);

  useEffect(() => {
    initializeSession();
  }, [initializeSession]);

  // ============================================================================
  // Tab Synchronization
  // ============================================================================

  useEffect(() => {
    const handleSyncMessage = (message: AuthSyncMessage) => {
      switch (message.type) {
        case 'LOGOUT':
          secureStorage.clearMemoryCache();
          setState({ status: 'UNAUTHENTICATED', user: null, error: null });
          break;

        case 'LOGIN':
          setState({ status: 'AUTHENTICATED', user: message.session.user, error: null });
          break;

        case 'TOKEN_REFRESH':
          // Update tokens in current tab
          secureStorage.getSession().then((session) => {
            if (session) {
              session.tokens = message.tokens;
              secureStorage.setSession(session);
            }
          });
          break;

        case 'SESSION_EXPIRED':
          secureStorage.clearMemoryCache();
          setState({
            status: 'SESSION_EXPIRED',
            user: null,
            error: {
              code: 'SESSION_EXPIRED',
              message: 'Your session has expired',
              timestamp: message.timestamp,
            },
          });
          break;
      }
    };

    tabSync.addListener('auth-provider', handleSyncMessage);

    return () => {
      tabSync.removeListener('auth-provider');
    };
  }, []);

  // ============================================================================
  // Authentication Methods
  // ============================================================================

  const login = useCallback(async (payload: LoginPayload) => {
    setIsLoading(true);
    const startTime = Date.now();

    try {
      const response = await apiClient.post<AuthResponse>('/api/v1/auth/login', payload);

      if (!response.data.success || !response.data.data) {
        throw new Error(response.data.error?.message || 'Login failed');
      }

      const session = response.data.data;
      await secureStorage.setSession(session);
      await secureStorage.setRefreshToken(session.tokens.refreshToken);

      setState({ status: 'AUTHENTICATED', user: session.user, error: null });
      tabSync.broadcastLogin(session);

      emitAuthTelemetry({
        eventType: 'SESSION_START',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
        metadata: { method: 'login' },
      });
    } catch (error: any) {
      const authError = {
        code: error.response?.data?.error?.code || 'UNKNOWN_ERROR',
        message: error.response?.data?.error?.message || 'Login failed',
        timestamp: Date.now(),
      };

      setState({ status: 'UNAUTHENTICATED', user: null, error: null });

      emitAuthTelemetry({
        eventType: 'AUTH_ERROR',
        timestamp: Date.now(),
        metadata: { error: authError.code, method: 'login' },
      });

      throw authError;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const signup = useCallback(async (payload: SignupPayload) => {
    setIsLoading(true);

    try {
      const response = await apiClient.post<AuthResponse>('/api/v1/auth/signup', payload);

      if (!response.data.success || !response.data.data) {
        throw new Error(response.data.error?.message || 'Signup failed');
      }

      const session = response.data.data;
      await secureStorage.setSession(session);
      await secureStorage.setRefreshToken(session.tokens.refreshToken);

      setState({ status: 'AUTHENTICATED', user: session.user, error: null });
      tabSync.broadcastLogin(session);
    } catch (error: any) {
      const authError = {
        code: error.response?.data?.error?.code || 'UNKNOWN_ERROR',
        message: error.response?.data?.error?.message || 'Signup failed',
        timestamp: Date.now(),
      };

      throw authError;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const logout = useCallback(async () => {
    setIsLoading(true);
    const startTime = Date.now();

    try {
      await apiClient.post('/api/v1/auth/logout');
    } catch (error) {
      console.error('Logout API call failed:', error);
    } finally {
      await secureStorage.clearSession();
      setState({ status: 'UNAUTHENTICATED', user: null, error: null });
      tabSync.broadcastLogout();

      emitAuthTelemetry({
        eventType: 'SESSION_END',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
        metadata: { method: 'logout' },
      });

      setIsLoading(false);
    }
  }, []);

  const refreshSession = useCallback(async () => {
    const startTime = Date.now();

    try {
      const refreshToken = await secureStorage.getRefreshToken();
      if (!refreshToken) {
        throw new Error('No refresh token available');
      }

      const response = await apiClient.post('/api/v1/auth/refresh', { refreshToken });

      if (!response.data.success || !response.data.data) {
        throw new Error('Token refresh failed');
      }

      const newTokens = response.data.data;
      const session = await secureStorage.getSession();

      if (session) {
        session.tokens = newTokens;
        await secureStorage.setSession(session);
        tabSync.broadcastTokenRefresh(newTokens);
      }

      emitAuthTelemetry({
        eventType: 'TOKEN_REFRESH',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
        metadata: { success: true },
      });
    } catch (error) {
      await secureStorage.clearSession();
      setState({
        status: 'SESSION_EXPIRED',
        user: null,
        error: {
          code: 'TOKEN_REFRESH_FAILED',
          message: 'Session refresh failed',
          timestamp: Date.now(),
        },
      });
      tabSync.broadcastSessionExpired();

      throw error;
    }
  }, []);

  const resetPassword = useCallback(async (payload: PasswordResetPayload) => {
    setIsLoading(true);

    try {
      await apiClient.post('/api/v1/auth/password-reset', payload);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const confirmPasswordReset = useCallback(async (payload: PasswordResetConfirmPayload) => {
    setIsLoading(true);

    try {
      await apiClient.post('/api/v1/auth/password-reset/confirm', payload);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const setupMFA = useCallback(async (payload: MFASetupPayload) => {
    setIsLoading(true);

    try {
      await apiClient.post('/api/v1/auth/mfa/setup', payload);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const verifyMFA = useCallback(async (payload: MFAVerifyPayload) => {
    setIsLoading(true);

    try {
      await apiClient.post('/api/v1/auth/mfa/verify', payload);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const checkKYCAccess = useCallback(
    (requiredTier: KYCTier): boolean => {
      if (state.status !== 'AUTHENTICATED' || !state.user) {
        return false;
      }

      const userTierLevel = KYC_TIER_HIERARCHY[state.user.kycProfile.tier];
      const requiredTierLevel = KYC_TIER_HIERARCHY[requiredTier];

      return userTierLevel >= requiredTierLevel;
    },
    [state]
  );

  // ============================================================================
  // Context Value
  // ============================================================================

  const value: AuthContextValue = {
    state,
    login,
    signup,
    logout,
    refreshSession,
    resetPassword,
    confirmPasswordReset,
    setupMFA,
    verifyMFA,
    checkKYCAccess,
    isLoading,
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

// ============================================================================
// Hook
// ============================================================================

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);

  if (!context) {
    throw new Error('useAuth must be used within AuthProvider');
  }

  return context;
}
