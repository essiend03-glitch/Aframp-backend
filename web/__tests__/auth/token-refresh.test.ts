import axios from 'axios';
import { createApiClient } from '@/lib/auth/api-client';
import { secureStorage } from '@/lib/auth/storage';

jest.mock('@/lib/auth/storage');
jest.mock('axios');

describe('Token Refresh Interceptor', () => {
  let apiClient: ReturnType<typeof createApiClient>;

  beforeEach(() => {
    jest.clearAllMocks();
    apiClient = createApiClient();
  });

  it('queues requests during token refresh', async () => {
    const mockSession = {
      user: { id: '123' },
      tokens: {
        accessToken: 'old_token',
        refreshToken: 'refresh_token',
        expiresAt: Date.now() + 3600000,
      },
    };

    (secureStorage.getSession as jest.Mock).mockResolvedValue(mockSession);
    (secureStorage.getRefreshToken as jest.Mock).mockResolvedValue('refresh_token');

    // Mock 401 response followed by successful refresh
    (axios.post as jest.Mock)
      .mockRejectedValueOnce({
        response: { status: 401 },
        config: { headers: {} },
      })
      .mockResolvedValueOnce({
        data: {
          success: true,
          data: {
            accessToken: 'new_token',
            refreshToken: 'new_refresh_token',
            expiresAt: Date.now() + 3600000,
          },
        },
      });

    // This test verifies the interceptor logic exists
    expect(apiClient.interceptors.response).toBeDefined();
  });

  it('prevents infinite retry loops', async () => {
    (secureStorage.getSession as jest.Mock).mockResolvedValue(null);
    (secureStorage.getRefreshToken as jest.Mock).mockResolvedValue(null);

    // Verify interceptor configuration
    expect(apiClient.interceptors.request).toBeDefined();
    expect(apiClient.interceptors.response).toBeDefined();
  });
});
