import { TabSyncManager } from '@/lib/auth/tab-sync';

describe('TabSyncManager', () => {
  let tabSync: TabSyncManager;

  beforeEach(() => {
    // Mock BroadcastChannel
    global.BroadcastChannel = jest.fn().mockImplementation(() => ({
      postMessage: jest.fn(),
      close: jest.fn(),
      onmessage: null,
      onmessageerror: null,
    })) as any;

    tabSync = new TabSyncManager();
  });

  afterEach(() => {
    tabSync.close();
  });

  it('broadcasts logout event', () => {
    const broadcastSpy = jest.spyOn(tabSync as any, 'broadcast');
    tabSync.broadcastLogout();

    expect(broadcastSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'LOGOUT',
        timestamp: expect.any(Number),
      })
    );
  });

  it('broadcasts login event', () => {
    const mockSession = {
      user: { id: '123', email: 'test@example.com' },
      tokens: { accessToken: 'token', expiresAt: Date.now() + 3600000 },
    } as any;

    const broadcastSpy = jest.spyOn(tabSync as any, 'broadcast');
    tabSync.broadcastLogin(mockSession);

    expect(broadcastSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'LOGIN',
        session: mockSession,
        timestamp: expect.any(Number),
      })
    );
  });

  it('registers and removes listeners', () => {
    const callback = jest.fn();

    tabSync.addListener('test-listener', callback);
    expect((tabSync as any).listeners.has('test-listener')).toBe(true);

    tabSync.removeListener('test-listener');
    expect((tabSync as any).listeners.has('test-listener')).toBe(false);
  });
});
