/**
 * Authentication Module Exports
 * Central export point for all auth-related functionality
 */

// Context & Hooks
export { AuthProvider, useAuth } from './auth-context';

// API Client
export { apiClient, createApiClient } from './api-client';

// Storage
export { secureStorage, SecureStorage } from './storage';
export type { SecureStorageAdapter } from '@/types/auth';

// Tab Synchronization
export { tabSync, TabSyncManager } from './tab-sync';
export type { AuthSyncMessage } from './tab-sync';

// Telemetry
export { emitAuthTelemetry, trackAuthPerformance, isSessionExpiringSoon } from './telemetry';

// Route Configuration
export { ROUTE_CONFIGS, getRouteConfig, getAccessLevel } from './route-config';
