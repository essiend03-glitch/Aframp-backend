/**
 * API Client with Token Refresh Interceptors
 * Handles automatic token refresh, request queuing, and 401 error recovery
 */

import axios, { AxiosInstance, AxiosError, InternalAxiosRequestConfig } from 'axios';
import type { AuthSession, RefreshResponse, TokenContainer } from '@/types/auth';
import { secureStorage } from './storage';

// ============================================================================
// Configuration
// ============================================================================

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';
const REFRESH_ENDPOINT = '/api/v1/auth/refresh';

// ============================================================================
// Request Queue for Token Refresh
// ============================================================================

interface QueuedRequest {
  resolve: (token: string) => void;
  reject: (error: Error) => void;
}

class TokenRefreshQueue {
  private isRefreshing = false;
  private queue: QueuedRequest[] = [];

  async addToQueue(): Promise<string> {
    return new Promise((resolve, reject) => {
      this.queue.push({ resolve, reject });
    });
  }

  setRefreshing(status: boolean): void {
    this.isRefreshing = status;
  }

  isCurrentlyRefreshing(): boolean {
    return this.isRefreshing;
  }

  resolveQueue(token: string): void {
    this.queue.forEach(({ resolve }) => resolve(token));
    this.queue = [];
  }

  rejectQueue(error: Error): void {
    this.queue.forEach(({ reject }) => reject(error));
    this.queue = [];
  }
}

const refreshQueue = new TokenRefreshQueue();

// ============================================================================
// Token Refresh Logic
// ============================================================================

async function refreshAccessToken(): Promise<string> {
  const refreshToken = await secureStorage.getRefreshToken();
  
  if (!refreshToken) {
    throw new Error('No refresh token available');
  }

  try {
    const response = await axios.post<RefreshResponse>(
      `${API_BASE_URL}${REFRESH_ENDPOINT}`,
      { refreshToken },
      {
        headers: { 'Content-Type': 'application/json' },
      }
    );

    if (!response.data.success || !response.data.data) {
      throw new Error('Token refresh failed');
    }

    const newTokens = response.data.data;
    
    // Update stored session with new tokens
    const session = await secureStorage.getSession();
    if (session) {
      session.tokens = newTokens;
      await secureStorage.setSession(session);
    }

    return newTokens.accessToken;
  } catch (error) {
    await secureStorage.clearSession();
    throw error;
  }
}

// ============================================================================
// API Client Factory
// ============================================================================

export function createApiClient(): AxiosInstance {
  const client = axios.create({
    baseURL: API_BASE_URL,
    timeout: 30000,
    headers: {
      'Content-Type': 'application/json',
    },
  });

  // ============================================================================
  // Request Interceptor - Inject Access Token
  // ============================================================================

  client.interceptors.request.use(
    async (config: InternalAxiosRequestConfig) => {
      const session = await secureStorage.getSession();
      
      if (session?.tokens.accessToken) {
        config.headers.Authorization = `Bearer ${session.tokens.accessToken}`;
      }

      return config;
    },
    (error) => Promise.reject(error)
  );

  // ============================================================================
  // Response Interceptor - Handle 401 and Token Refresh
  // ============================================================================

  client.interceptors.response.use(
    (response) => response,
    async (error: AxiosError) => {
      const originalRequest = error.config as InternalAxiosRequestConfig & { _retry?: boolean };

      // Only handle 401 errors
      if (error.response?.status !== 401 || !originalRequest) {
        return Promise.reject(error);
      }

      // Prevent infinite retry loops
      if (originalRequest._retry) {
        await secureStorage.clearSession();
        window.location.href = '/login?session=expired';
        return Promise.reject(error);
      }

      originalRequest._retry = true;

      // If already refreshing, queue this request
      if (refreshQueue.isCurrentlyRefreshing()) {
        try {
          const newToken = await refreshQueue.addToQueue();
          originalRequest.headers.Authorization = `Bearer ${newToken}`;
          return client(originalRequest);
        } catch (refreshError) {
          return Promise.reject(refreshError);
        }
      }

      // Start token refresh process
      refreshQueue.setRefreshing(true);

      try {
        const newToken = await refreshAccessToken();
        refreshQueue.resolveQueue(newToken);
        refreshQueue.setRefreshing(false);

        // Retry original request with new token
        originalRequest.headers.Authorization = `Bearer ${newToken}`;
        return client(originalRequest);
      } catch (refreshError) {
        refreshQueue.rejectQueue(refreshError as Error);
        refreshQueue.setRefreshing(false);
        
        await secureStorage.clearSession();
        window.location.href = '/login?session=expired';
        return Promise.reject(refreshError);
      }
    }
  );

  return client;
}

// ============================================================================
// Singleton Instance
// ============================================================================

export const apiClient = createApiClient();
