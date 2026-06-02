import { renderHook, act, waitFor } from '@testing-library/react';
import { AuthProvider, useAuth } from '@/lib/auth/auth-context';
import { apiClient } from '@/lib/auth/api-client';
import { secureStorage } from '@/lib/auth/storage';

jest.mock('@/lib/auth/api-client');
jest.mock('@/lib/auth/storage');
jest.mock('@/lib/auth/tab-sync');

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <AuthProvider>{children}</AuthProvider>
);

describe('AuthContext', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('initializes with INITIALIZING status', () => {
    const { result } = renderHook(() => useAuth(), { wrapper });
    expect(result.current.state.status).toBe('INITIALIZING');
  });

  it('transitions to UNAUTHENTICATED when no session exists', async () => {
    (secureStorage.getSession as jest.Mock).mockResolvedValue(null);

    const { result } = renderHook(() => useAuth(), { wrapper });

    await waitFor(() => {
      expect(result.current.state.status).toBe('UNAUTHENTICATED');
    });
  });

  it('successfully logs in user', async () => {
    const mockSession = {
      user: {
        id: '123',
        email: 'test@example.com',
        firstName: 'Test',
        lastName: 'User',
        kycProfile: { tier: 'KYC_Level_1' },
      },
      tokens: {
        accessToken: 'access_token',
        refreshToken: 'refresh_token',
        expiresAt: Date.now() + 3600000,
      },
    };

    (apiClient.post as jest.Mock).mockResolvedValue({
      data: { success: true, data: mockSession },
    });

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await result.current.login({
        email: 'test@example.com',
        password: 'password123',
      });
    });

    expect(result.current.state.status).toBe('AUTHENTICATED');
    expect(result.current.state.user?.email).toBe('test@example.com');
  });

  it('handles login failure', async () => {
    (apiClient.post as jest.Mock).mockRejectedValue({
      response: {
        data: {
          error: { code: 'INVALID_CREDENTIALS', message: 'Invalid credentials' },
        },
      },
    });

    const { result } = renderHook(() => useAuth(), { wrapper });

    await expect(
      act(async () => {
        await result.current.login({
          email: 'test@example.com',
          password: 'wrong_password',
        });
      })
    ).rejects.toThrow();

    expect(result.current.state.status).toBe('UNAUTHENTICATED');
  });

  it('checks KYC access correctly', async () => {
    const mockSession = {
      user: {
        id: '123',
        email: 'test@example.com',
        kycProfile: { tier: 'KYC_Level_1' },
      },
      tokens: { accessToken: 'token', expiresAt: Date.now() + 3600000 },
    };

    (secureStorage.getSession as jest.Mock).mockResolvedValue(mockSession);

    const { result } = renderHook(() => useAuth(), { wrapper });

    await waitFor(() => {
      expect(result.current.state.status).toBe('AUTHENTICATED');
    });

    expect(result.current.checkKYCAccess('Unverified')).toBe(true);
    expect(result.current.checkKYCAccess('KYC_Level_1')).toBe(true);
    expect(result.current.checkKYCAccess('KYC_Level_2')).toBe(false);
    expect(result.current.checkKYCAccess('Admin')).toBe(false);
  });
});
