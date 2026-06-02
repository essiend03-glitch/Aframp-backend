import { renderHook, act, waitFor } from '@testing-library/react';
import { AuthProvider, useAuth } from '@/lib/auth/auth-context';
import { apiClient } from '@/lib/auth/api-client';
import { secureStorage } from '@/lib/auth/storage';
import { tabSync } from '@/lib/auth/tab-sync';

jest.mock('@/lib/auth/api-client');
jest.mock('@/lib/auth/storage');
jest.mock('@/lib/auth/tab-sync');

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <AuthProvider>{children}</AuthProvider>
);

describe('Authentication Flow Integration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('completes full login to logout flow', async () => {
    const mockSession = {
      user: {
        id: '123',
        email: 'test@example.com',
        firstName: 'Test',
        lastName: 'User',
        phoneNumber: null,
        kycProfile: {
          tier: 'KYC_Level_1',
          verifiedAt: new Date().toISOString(),
          expiresAt: null,
          documentStatus: {
            idVerified: true,
            addressVerified: false,
            selfieVerified: true,
          },
          limits: {
            dailyTransactionLimit: 1000000,
            monthlyTransactionLimit: 5000000,
            singleTransactionLimit: 500000,
          },
          restrictions: [],
        },
        marketAccess: [],
        preferences: {
          locale: 'en',
          currency: 'NGN',
          timezone: 'Africa/Lagos',
          notifications: { email: true, sms: false, push: true },
        },
        createdAt: new Date().toISOString(),
        lastLoginAt: new Date().toISOString(),
      },
      tokens: {
        accessToken: 'access_token_123',
        refreshToken: 'refresh_token_123',
        expiresAt: Date.now() + 3600000,
        tokenType: 'Bearer' as const,
      },
      metadata: {
        sessionId: 'session_123',
        deviceId: 'device_123',
        ipAddress: '127.0.0.1',
        userAgent: 'test',
        createdAt: Date.now(),
        lastActivityAt: Date.now(),
      },
    };

    (secureStorage.getSession as jest.Mock).mockResolvedValue(null);
    (apiClient.post as jest.Mock).mockResolvedValue({
      data: { success: true, data: mockSession },
    });

    const { result } = renderHook(() => useAuth(), { wrapper });

    // Wait for initialization
    await waitFor(() => {
      expect(result.current.state.status).toBe('UNAUTHENTICATED');
    });

    // Login
    await act(async () => {
      await result.current.login({
        email: 'test@example.com',
        password: 'password123',
      });
    });

    expect(result.current.state.status).toBe('AUTHENTICATED');
    expect(result.current.state.user?.email).toBe('test@example.com');
    expect(secureStorage.setSession).toHaveBeenCalledWith(mockSession);
    expect(tabSync.broadcastLogin).toHaveBeenCalledWith(mockSession);

    // Logout
    (apiClient.post as jest.Mock).mockResolvedValue({ data: { success: true } });

    await act(async () => {
      await result.current.logout();
    });

    expect(result.current.state.status).toBe('UNAUTHENTICATED');
    expect(secureStorage.clearSession).toHaveBeenCalled();
    expect(tabSync.broadcastLogout).toHaveBeenCalled();
  });

  it('handles multi-tab logout synchronization', async () => {
    const mockSession = {
      user: { id: '123', email: 'test@example.com' },
      tokens: { accessToken: 'token', expiresAt: Date.now() + 3600000 },
    };

    (secureStorage.getSession as jest.Mock).mockResolvedValue(mockSession);

    const { result } = renderHook(() => useAuth(), { wrapper });

    await waitFor(() => {
      expect(result.current.state.status).toBe('AUTHENTICATED');
    });

    // Simulate logout from another tab
    const logoutMessage = { type: 'LOGOUT' as const, timestamp: Date.now() };
    
    act(() => {
      const listener = (tabSync.addListener as jest.Mock).mock.calls[0][1];
      listener(logoutMessage);
    });

    expect(result.current.state.status).toBe('UNAUTHENTICATED');
  });

  it('handles session expiry and refresh', async () => {
    const expiredSession = {
      user: { id: '123' },
      tokens: {
        accessToken: 'old_token',
        refreshToken: 'refresh_token',
        expiresAt: Date.now() - 1000, // Expired
      },
    };

    const newTokens = {
      accessToken: 'new_token',
      refreshToken: 'new_refresh_token',
      expiresAt: Date.now() + 3600000,
    };

    (secureStorage.getSession as jest.Mock).mockResolvedValue(expiredSession);
    (secureStorage.getRefreshToken as jest.Mock).mockResolvedValue('refresh_token');
    (apiClient.post as jest.Mock).mockResolvedValue({
      data: { success: true, data: newTokens },
    });

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await result.current.refreshSession();
    });

    expect(apiClient.post).toHaveBeenCalledWith('/api/v1/auth/refresh', {
      refreshToken: 'refresh_token',
    });
    expect(tabSync.broadcastTokenRefresh).toHaveBeenCalledWith(newTokens);
  });
});
