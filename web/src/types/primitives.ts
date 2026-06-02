/**
 * Core Platform Type Definitions
 * Strict discriminated unions for aframp platform primitives
 */

// ============================================================================
// Currency & Regional Types
// ============================================================================

export type FiatCurrency = 'NGN' | 'KES' | 'GHS' | 'USD' | 'ZAR' | 'UGX';
export type StablecoinType = 'USDC' | 'USDT' | 'cNGN';
export type CurrencyType = FiatCurrency | StablecoinType;

export interface CurrencySpec {
  code: CurrencyType;
  symbol: string;
  decimals: number;
  locale: string;
  minAmount: string;
  maxAmount: string;
}

export type RegionalLocale = 
  | 'en-NG' // Nigeria
  | 'en-KE' // Kenya
  | 'en-GH' // Ghana
  | 'en-ZA' // South Africa
  | 'en-UG' // Uganda
  | 'fr-CI' // Côte d'Ivoire
  | 'sw-KE' // Swahili (Kenya)
  | 'yo-NG' // Yoruba (Nigeria)
  | 'ig-NG' // Igbo (Nigeria)
  | 'ha-NG'; // Hausa (Nigeria)

// ============================================================================
// Transaction Types
// ============================================================================

export type TransactionStatus =
  | 'pending'
  | 'processing'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'refunded';

export type TransactionType =
  | 'deposit'
  | 'withdrawal'
  | 'transfer'
  | 'exchange'
  | 'payment'
  | 'refund';

export interface BaseTransaction {
  id: string;
  type: TransactionType;
  status: TransactionStatus;
  amount: string;
  currency: CurrencyType;
  fee: string;
  createdAt: string;
  updatedAt: string;
  userId: string;
  metadata: Record<string, unknown>;
}

export interface DepositTransaction extends BaseTransaction {
  type: 'deposit';
  sourceAccount: string;
  destinationWalletId: string;
  paymentMethod: 'bank_transfer' | 'card' | 'mobile_money';
}

export interface WithdrawalTransaction extends BaseTransaction {
  type: 'withdrawal';
  sourceWalletId: string;
  destinationAccount: string;
  withdrawalMethod: 'bank_transfer' | 'mobile_money';
}

export interface TransferTransaction extends BaseTransaction {
  type: 'transfer';
  sourceWalletId: string;
  destinationWalletId: string;
  recipientId: string;
}

export interface ExchangeTransaction extends BaseTransaction {
  type: 'exchange';
  sourceCurrency: CurrencyType;
  destinationCurrency: CurrencyType;
  sourceAmount: string;
  destinationAmount: string;
  exchangeRate: string;
  rateId: string;
}

export type Transaction =
  | DepositTransaction
  | WithdrawalTransaction
  | TransferTransaction
  | ExchangeTransaction;

// ============================================================================
// Wallet Types
// ============================================================================

export type WalletStatus = 'active' | 'frozen' | 'suspended' | 'closed';
export type WalletType = 'personal' | 'business' | 'merchant';

export interface Wallet {
  id: string;
  userId: string;
  type: WalletType;
  currency: CurrencyType;
  balance: string;
  availableBalance: string;
  lockedBalance: string;
  status: WalletStatus;
  createdAt: string;
  updatedAt: string;
  metadata: {
    label?: string;
    isPrimary: boolean;
  };
}

export interface WalletBalance {
  walletId: string;
  currency: CurrencyType;
  balance: string;
  availableBalance: string;
  lockedBalance: string;
  lastUpdated: string;
}

// ============================================================================
// Partner & Profile Types
// ============================================================================

export type PartnerTier = 'starter' | 'growth' | 'enterprise';
export type PartnerStatus = 'pending' | 'active' | 'suspended' | 'terminated';
export type KYBStatus = 'not_started' | 'in_progress' | 'submitted' | 'approved' | 'rejected';

export interface PartnerProfile {
  id: string;
  businessName: string;
  businessType: string;
  registrationNumber: string;
  country: string;
  tier: PartnerTier;
  status: PartnerStatus;
  kybStatus: KYBStatus;
  apiKeys: {
    publicKey: string;
    hasSecretKey: boolean;
  };
  webhookUrl?: string;
  allowedOrigins: string[];
  createdAt: string;
  updatedAt: string;
}

// ============================================================================
// Exchange Rate Types
// ============================================================================

export type RateProvider = 'internal' | 'external' | 'aggregated';

export interface ExchangeRate {
  id: string;
  sourceCurrency: CurrencyType;
  destinationCurrency: CurrencyType;
  rate: string;
  inverseRate: string;
  provider: RateProvider;
  spread: string;
  validFrom: string;
  validUntil: string;
  createdAt: string;
}

export interface ExchangeQuote {
  quoteId: string;
  sourceCurrency: CurrencyType;
  destinationCurrency: CurrencyType;
  sourceAmount: string;
  destinationAmount: string;
  rate: string;
  fee: string;
  totalCost: string;
  expiresAt: string;
  createdAt: string;
}

// ============================================================================
// User & Authentication Types
// ============================================================================

export type UserRole = 'user' | 'merchant' | 'partner' | 'admin';
export type KYCStatus = 'not_started' | 'in_progress' | 'submitted' | 'approved' | 'rejected';
export type KYCTier = 'tier_0' | 'tier_1' | 'tier_2' | 'tier_3';

export interface User {
  id: string;
  email: string;
  firstName: string;
  lastName: string;
  phoneNumber?: string;
  role: UserRole;
  kycStatus: KYCStatus;
  kycTier: KYCTier;
  country: string;
  locale: RegionalLocale;
  createdAt: string;
  updatedAt: string;
}

export interface AuthTokens {
  accessToken: string;
  refreshToken: string;
  expiresIn: number;
  tokenType: 'Bearer';
}

// ============================================================================
// API Response Types
// ============================================================================

export interface ApiSuccessResponse<T> {
  success: true;
  data: T;
  meta?: {
    page?: number;
    pageSize?: number;
    total?: number;
    hasMore?: boolean;
  };
}

export interface ApiErrorResponse {
  success: false;
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
    field?: string;
  };
}

export type ApiResponse<T> = ApiSuccessResponse<T> | ApiErrorResponse;

// ============================================================================
// Pagination Types
// ============================================================================

export interface PaginationParams {
  page: number;
  pageSize: number;
  sortBy?: string;
  sortOrder?: 'asc' | 'desc';
}

export interface PaginatedResponse<T> {
  items: T[];
  pagination: {
    page: number;
    pageSize: number;
    total: number;
    totalPages: number;
    hasMore: boolean;
  };
}
