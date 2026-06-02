/**
 * Authentication Hook
 * Encapsulates auth state, caching, and token management
 */

'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiClient, tokenManager } from '@/lib/api/client';
import type { User, AuthTokens } from '@/types/primitives';

// ============================================================================
// API Functions
// ============================================================================

async function fetchCurrentUser(): Promise<User> {
  return apiClient.get<User>('/api/v1/auth/me');
}

async function login(credentials: { email: string; password: string }): Promise<{ user: User; tokens: AuthTokens }> {
  return apiClient.post<{ user: User; tokens: AuthTokens }>('/api/v1/auth/login', credentials);
}

async function logout(): Promise<void> {
  return apiClient.post<void>('/api/v1/auth/logout');
}

async function register(data: {
  email: string;
  password: string;
  firstName: string;
  lastName: string;
  country: string;
}): Promise<{ user: User; tokens: AuthTokens }> {
  return apiClient.post<{ user: User; tokens: AuthTokens }>('/api/v1/auth/register', data);
}

// ============================================================================
// Auth Hook
// ============================================================================

export function useAuth() {
  const queryClient = useQueryClient();

  // Query current user
  const {
    data: user,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ['auth', 'user'],
    queryFn: fetchCurrentUser,
    enabled: !!tokenManager.getAccessToken(),
    staleTime: 5 * 60 * 1000, // 5 minutes
    retry: false,
  });

  // Login mutation
  const loginMutation = useMutation({
    mutationFn: login,
    onSuccess: (data) => {
      tokenManager.setTokens(data.tokens.accessToken, data.tokens.refreshToken, data.tokens.expiresIn);
      queryClient.setQueryData(['auth', 'user'], data.user);
    },
  });

  // Logout mutation
  const logoutMutation = useMutation({
    mutationFn: logout,
    onSuccess: () => {
      tokenManager.clearTokens();
      queryClient.setQueryData(['auth', 'user'], null);
      queryClient.clear();
    },
  });

  // Register mutation
  const registerMutation = useMutation({
    mutationFn: register,
    onSuccess: (data) => {
      tokenManager.setTokens(data.tokens.accessToken, data.tokens.refreshToken, data.tokens.expiresIn);
      queryClient.setQueryData(['auth', 'user'], data.user);
    },
  });

  return {
    user,
    isAuthenticated: !!user,
    isLoading,
    error,
    login: loginMutation.mutateAsync,
    logout: logoutMutation.mutateAsync,
    register: registerMutation.mutateAsync,
    refetch,
    isLoginLoading: loginMutation.isPending,
    isLogoutLoading: logoutMutation.isPending,
    isRegisterLoading: registerMutation.isPending,
  };
}
