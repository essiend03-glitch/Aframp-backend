/**
 * Authentication Type Definitions
 * Comprehensive TypeScript types for user sessions, auth payloads, and multi-tenant contexts
 */

// ============================================================================
// Auth State Machine Types
// ============================================================================

export type AuthStatus = 
  | 'INITIALIZING'
  | 'AUTHENTICATED'
  | 'UNAUTHENTICATED'
  | 'SESSION_EXPIRED';

export type AuthState = 
  | { status: 'INITIALIZING'; user: null; error: null }
  | { status: 'AUTHENTICATED'; user: AuthenticatedUser; error: null }
  | { status: 'UNAUTHENTICATED'; user: null; error: null }
  | { status: 'SESSION_EXPIRED'; user: null; error: AuthError };

// ============================================================================
// KYC Verification Tiers
// ============================================================================

export type KYCTier = 
  | 'Unverified'
  | 'KYC_Level_1'
  | 'KYC_Level_2'
  | 'Admin';

export interface KYCComplianceProfile {
  tier: KYCTier;
  verifiedAt: string | null;
  expiresAt: string | null;
  documentStatus: {
    idVerified: boolean;
    addressVerified: boolean;
    selfieVerified: boolean;
  };
  limits: {
    dailyTransactionLimit: number;
    monthlyTransactionLimit: number;
    singleTransactionLimit: number;
  };
  restrictions: string[];
}

// ============================================================================
// User & Session Types
// ============================================================================

export interface AuthenticatedUser {
  id: string;
  email: string;
  phoneNumber: string | null;
  firstName: string;
  lastName: string;
  kycProfile: KYCComplianceProfile;
  marketAccess: MarketAccessScope[];
  preferences: UserPreferences;
  createdAt: string;
  lastLoginAt: string;
}

export interface MarketAccessScope {
  marketId: string;
  marketCode: string; // e.g., 'NGN', 'KES', 'GHS'
  enabled: boolean;
  permissions: string[];
}

export interface UserPreferences {
  locale: string;
  currency: string;
  timezone: string;
  notifications: {
    email: boolean;
    sms: boolean;
    push: boolean;
  };
}

// ============================================================================
// Token & Session Management
// ============================================================================

export interface TokenContainer {
  accessToken: string;
  refreshToken: string;
  expiresAt: number; // Unix timestamp
  tokenType: 'Bearer';
}

export interface SessionMetadata {
  sessionId: string;
  deviceId: string;
  ipAddress: string;
  userAgent: string;
  createdAt: number;
  lastActivityAt: number;
}

export interface AuthSession {
  user: AuthenticatedUser;
  tokens: TokenContainer;
  metadata: SessionMetadata;
}

// ============================================================================
// Auth Payloads & Responses
// ============================================================================

export interface LoginPayload {
  email: string;
  password: string;
  deviceId?: string;
  rememberMe?: boolean;
}

export interface SignupPayload {
  email: string;
  password: string;
  firstName: string;
  lastName: string;
  phoneNumber?: string;
  marketCode: string;
  acceptedTerms: boolean;
}

export interface RefreshTokenPayload {
  refreshToken: string;
  deviceId: string;
}

export interface PasswordResetPayload {
  email: string;
}

export interface PasswordResetConfirmPayload {
  token: string;
  newPassword: string;
}

export interface MFASetupPayload {
  method: 'totp' | 'sms';
}

export interface MFAVerifyPayload {
  code: string;
  method: 'totp' | 'sms';
}

export interface AuthResponse {
  success: boolean;
  data?: AuthSession;
  error?: AuthError;
}

export interface RefreshResponse {
  success: boolean;
  data?: TokenContainer;
  error?: AuthError;
}

// ============================================================================
// Error Types
// ============================================================================

export interface AuthError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
  timestamp: number;
}

export type AuthErrorCode =
  | 'INVALID_CREDENTIALS'
  | 'SESSION_EXPIRED'
  | 'TOKEN_INVALID'
  | 'TOKEN_REFRESH_FAILED'
  | 'ACCOUNT_LOCKED'
  | 'MFA_REQUIRED'
  | 'INSUFFICIENT_KYC'
  | 'NETWORK_ERROR'
  | 'UNKNOWN_ERROR';

// ============================================================================
// Route Protection Types
// ============================================================================

export interface RouteConfig {
  path: string;
  requiredAuth: boolean;
  requiredKYC?: KYCTier;
  allowedRoles?: string[];
  redirectUnauthenticated?: string;
  redirectUnauthorized?: string;
}

export type RouteAccessLevel = 
  | 'guest'           // No auth required
  | 'authenticated'   // Any authenticated user
  | 'kyc_verified'    // KYC Level 1 or higher
  | 'kyc_advanced'    // KYC Level 2 or higher
  | 'admin';          // Admin only

// ============================================================================
// Auth Context Types
// ============================================================================

export interface AuthContextValue {
  state: AuthState;
  login: (payload: LoginPayload) => Promise<void>;
  signup: (payload: SignupPayload) => Promise<void>;
  logout: () => Promise<void>;
  refreshSession: () => Promise<void>;
  resetPassword: (payload: PasswordResetPayload) => Promise<void>;
  confirmPasswordReset: (payload: PasswordResetConfirmPayload) => Promise<void>;
  setupMFA: (payload: MFASetupPayload) => Promise<void>;
  verifyMFA: (payload: MFAVerifyPayload) => Promise<void>;
  checkKYCAccess: (requiredTier: KYCTier) => boolean;
  isLoading: boolean;
}

// ============================================================================
// Storage Types
// ============================================================================

export interface SecureStorageAdapter {
  getSession: () => Promise<AuthSession | null>;
  setSession: (session: AuthSession) => Promise<void>;
  clearSession: () => Promise<void>;
  getRefreshToken: () => Promise<string | null>;
  setRefreshToken: (token: string) => Promise<void>;
}

// ============================================================================
// Telemetry Types
// ============================================================================

export interface AuthTelemetryEvent {
  eventType: 'SESSION_START' | 'SESSION_END' | 'TOKEN_REFRESH' | 'AUTH_ERROR';
  timestamp: number;
  duration?: number;
  metadata: Record<string, unknown>;
  anonymizedUserId?: string;
}
