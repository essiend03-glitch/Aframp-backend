/**
 * Tab Synchronization via Broadcast Channel API
 * Propagates auth state changes across browser tabs/windows
 */

import type { AuthSession } from '@/types/auth';

// ============================================================================
// Broadcast Channel Configuration
// ============================================================================

const CHANNEL_NAME = 'aframp_auth_sync';

export type AuthSyncMessage =
  | { type: 'LOGOUT'; timestamp: number }
  | { type: 'LOGIN'; session: AuthSession; timestamp: number }
  | { type: 'TOKEN_REFRESH'; tokens: AuthSession['tokens']; timestamp: number }
  | { type: 'SESSION_EXPIRED'; timestamp: number };

// ============================================================================
// Tab Sync Manager
// ============================================================================

export class TabSyncManager {
  private channel: BroadcastChannel | null = null;
  private listeners: Map<string, (message: AuthSyncMessage) => void> = new Map();

  constructor() {
    if (typeof window !== 'undefined' && 'BroadcastChannel' in window) {
      this.initializeChannel();
    }
  }

  private initializeChannel(): void {
    try {
      this.channel = new BroadcastChannel(CHANNEL_NAME);
      
      this.channel.onmessage = (event: MessageEvent<AuthSyncMessage>) => {
        this.handleMessage(event.data);
      };

      this.channel.onmessageerror = (error) => {
        console.error('Broadcast channel message error:', error);
      };
    } catch (error) {
      console.error('Failed to initialize broadcast channel:', error);
    }
  }

  private handleMessage(message: AuthSyncMessage): void {
    // Notify all registered listeners
    this.listeners.forEach((callback) => {
      try {
        callback(message);
      } catch (error) {
        console.error('Tab sync listener error:', error);
      }
    });
  }

  /**
   * Broadcast logout event to all tabs
   */
  broadcastLogout(): void {
    this.broadcast({
      type: 'LOGOUT',
      timestamp: Date.now(),
    });
  }

  /**
   * Broadcast login event to all tabs
   */
  broadcastLogin(session: AuthSession): void {
    this.broadcast({
      type: 'LOGIN',
      session,
      timestamp: Date.now(),
    });
  }

  /**
   * Broadcast token refresh event to all tabs
   */
  broadcastTokenRefresh(tokens: AuthSession['tokens']): void {
    this.broadcast({
      type: 'TOKEN_REFRESH',
      tokens,
      timestamp: Date.now(),
    });
  }

  /**
   * Broadcast session expired event to all tabs
   */
  broadcastSessionExpired(): void {
    this.broadcast({
      type: 'SESSION_EXPIRED',
      timestamp: Date.now(),
    });
  }

  /**
   * Send message to all tabs
   */
  private broadcast(message: AuthSyncMessage): void {
    if (!this.channel) {
      console.warn('Broadcast channel not available');
      return;
    }

    try {
      this.channel.postMessage(message);
    } catch (error) {
      console.error('Failed to broadcast message:', error);
    }
  }

  /**
   * Register a listener for auth sync events
   */
  addListener(id: string, callback: (message: AuthSyncMessage) => void): void {
    this.listeners.set(id, callback);
  }

  /**
   * Remove a registered listener
   */
  removeListener(id: string): void {
    this.listeners.delete(id);
  }

  /**
   * Close the broadcast channel
   */
  close(): void {
    if (this.channel) {
      this.channel.close();
      this.channel = null;
    }
    this.listeners.clear();
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

export const tabSync = new TabSyncManager();
