/**
 * Client Network Core
 * Robust API client with automatic JWT handling, token refresh, and error normalization
 */

import axios, { type AxiosInstance, type AxiosError, type InternalAxiosRequestConfig } from 'axios';
import Cookies from 'js-cookie';
import type { ApiResponse, ApiErrorResponse } from '@/types/primitives';

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';
const ACCESS_TOKEN_KEY = 'aframp_access_token';
const REFRESH_TOKEN_KEY = 'aframp_refresh_token';

// ============================================================================
// Token Management
// ============================================================================

export const tokenManager = {
  getAccessToken: (): string | undefined => {
    return Cookies.get(ACCESS_TOKEN_KEY);
  },

  getRefreshToken: (): string | undefined => {
    return Cookies.get(REFRESH_TOKEN_KEY);
  },

  setTokens: (accessToken: string, refreshToken: string, expiresIn: number): void => {
    const expiryDays = expiresIn / (24 * 60 * 60);
    Cookies.set(ACCESS_TOKEN_KEY, accessToken, { 
      expires: expiryDays,
      secure: process.env.NODE_ENV === 'production',
      sameSite: 'strict'
    });
    Cookies.set(REFRESH_TOKEN_KEY, refreshToken, { 
      expires: 7, // Refresh token valid for 7 days
      secure: process.env.NODE_ENV === 'production',
      sameSite: 'strict'
    });
  },

  clearTokens: (): void => {
    Cookies.remove(ACCESS_TOKEN_KEY);
    Cookies.remove(REFRESH_TOKEN_KEY);
  },
};

// ============================================================================
// API Client Instance
// ============================================================================

class ApiClient {
  private client: AxiosInstance;
  private isRefreshing = false;
  private refreshSubscribers: Array<(token: string) => void> = [];

  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json',
      },
    });

    this.setupInterceptors();
  }

  private setupInterceptors(): void {
    // Request interceptor: Attach JWT token
    this.client.interceptors.request.use(
      (config: InternalAxiosRequestConfig) => {
        const token = tokenManager.getAccessToken();
        if (token && config.headers) {
          config.headers.Authorization = `Bearer ${token}`;
        }
        return config;
      },
      (error) => Promise.reject(error)
    );

    // Response interceptor: Handle 401 and refresh token
    this.client.interceptors.response.use(
      (response) => response,
      async (error: AxiosError) => {
        const originalRequest = error.config as InternalAxiosRequestConfig & { _retry?: boolean };

        // Handle 401 Unauthorized
        if (error.response?.status === 401 && !originalRequest._retry) {
          if (this.isRefreshing) {
            // Queue the request until token is refreshed
            return new Promise((resolve) => {
              this.refreshSubscribers.push((token: string) => {
                if (originalRequest.headers) {
                  originalRequest.headers.Authorization = `Bearer ${token}`;
                }
                resolve(this.client(originalRequest));
              });
            });
          }

          originalRequest._retry = true;
          this.isRefreshing = true;

          try {
            const newToken = await this.refreshAccessToken();
            this.isRefreshing = false;
            this.onTokenRefreshed(newToken);
            this.refreshSubscribers = [];

            if (originalRequest.headers) {
              originalRequest.headers.Authorization = `Bearer ${newToken}`;
            }
            return this.client(originalRequest);
          } catch (refreshError) {
            this.isRefreshing = false;
            this.refreshSubscribers = [];
            tokenManager.clearTokens();
            
            // Redirect to login
            if (typeof window !== 'undefined') {
              window.location.href = '/auth/login';
            }
            return Promise.reject(refreshError);
          }
        }

        return Promise.reject(this.normalizeError(error));
      }
    );
  }

  private async refreshAccessToken(): Promise<string> {
    const refreshToken = tokenManager.getRefreshToken();
    
    if (!refreshToken) {
      throw new Error('No refresh token available');
    }

    const response = await axios.post<ApiResponse<{
      accessToken: string;
      refreshToken: string;
      expiresIn: number;
    }>>(`${API_BASE_URL}/api/v1/auth/refresh`, {
      refreshToken,
    });

    if (response.data.success) {
      const { accessToken, refreshToken: newRefreshToken, expiresIn } = response.data.data;
      tokenManager.setTokens(accessToken, newRefreshToken, expiresIn);
      return accessToken;
    }

    throw new Error('Token refresh failed');
  }

  private onTokenRefreshed(token: string): void {
    this.refreshSubscribers.forEach((callback) => callback(token));
  }

  private normalizeError(error: AxiosError): ApiErrorResponse {
    if (error.response?.data) {
      const data = error.response.data as ApiErrorResponse;
      if ('error' in data) {
        return data;
      }
    }

    // Fallback error structure
    return {
      success: false,
      error: {
        code: error.code || 'NETWORK_ERROR',
        message: error.message || 'An unexpected error occurred',
        details: {
          status: error.response?.status,
          statusText: error.response?.statusText,
        },
      },
    };
  }

  // ============================================================================
  // Public API Methods
  // ============================================================================

  async get<T>(url: string, config?: Parameters<AxiosInstance['get']>[1]): Promise<T> {
    const response = await this.client.get<ApiResponse<T>>(url, config);
    if (response.data.success) {
      return response.data.data;
    }
    throw response.data;
  }

  async post<T>(url: string, data?: unknown, config?: Parameters<AxiosInstance['post']>[2]): Promise<T> {
    const response = await this.client.post<ApiResponse<T>>(url, data, config);
    if (response.data.success) {
      return response.data.data;
    }
    throw response.data;
  }

  async put<T>(url: string, data?: unknown, config?: Parameters<AxiosInstance['put']>[2]): Promise<T> {
    const response = await this.client.put<ApiResponse<T>>(url, data, config);
    if (response.data.success) {
      return response.data.data;
    }
    throw response.data;
  }

  async patch<T>(url: string, data?: unknown, config?: Parameters<AxiosInstance['patch']>[2]): Promise<T> {
    const response = await this.client.patch<ApiResponse<T>>(url, data, config);
    if (response.data.success) {
      return response.data.data;
    }
    throw response.data;
  }

  async delete<T>(url: string, config?: Parameters<AxiosInstance['delete']>[1]): Promise<T> {
    const response = await this.client.delete<ApiResponse<T>>(url, config);
    if (response.data.success) {
      return response.data.data;
    }
    throw response.data;
  }

  getRawClient(): AxiosInstance {
    return this.client;
  }
}

// Export singleton instance
export const apiClient = new ApiClient();
