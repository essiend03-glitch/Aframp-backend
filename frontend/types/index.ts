// Shared TypeScript interfaces for Issues #479, #480, #482

// ── Issue #479: Financial Dashboard Types ─────────────────────────────────────

export type TransactionType = 'inbound_deposit' | 'outbound_payout' | 'stellar_swap';
export type TransactionStatus = 'pending' | 'settled' | 'failed' | 'processing';

export interface LedgerTransaction {
  id: string;
  type: TransactionType;
  status: TransactionStatus;
  amount: number;
  currency: string;
  counterparty: string;
  stellarTxHash?: string;
  createdAt: string;
  settledAt?: string;
  fee?: number;
  corridor?: string; // e.g. "NGN/cNGN"
}

export interface WalletBalance {
  currency: string;
  available: number;
  pending: number;
  total: number;
  updatedAt: string;
}

export interface VolumeDataPoint {
  timestamp: string;
  volume: number;
  txCount: number;
}

export interface ConversionRate {
  pair: string;   // e.g. "NGN/cNGN"
  rate: number;
  open: number;
  high: number;
  low: number;
  close: number;
  timestamp: string;
}

export interface DashboardMetrics {
  netWalletEquity: number;
  activeLiquidityLimit: number;
  pendingTransactions: number;
  settledToday: number;
  balances: WalletBalance[];
}

// ── Issue #480: Non-Custodial Wallet Types ────────────────────────────────────

export type WalletProvider = 'freighter' | 'albedo' | 'lobstr' | 'ledger';

export type WalletConnectionState =
  | 'DISCONNECTED'
  | 'CONNECTING'
  | 'CONNECTED'
  | 'SIGNING_REQUEST'
  | 'TX_SUBMITTING';

export interface WalletConnectionContext {
  state: WalletConnectionState;
  publicKey: string | null;
  provider: WalletProvider | null;
  network: 'mainnet' | 'testnet' | null;
  hasCNGNTrustline: boolean;
}

export interface XDRTransactionPayload {
  xdr: string;           // base64-encoded unsigned XDR
  networkPassphrase: string;
  operations: ParsedOperation[];
  sourceAccount: string;
  fee: number;
  memo?: string;
}

export interface ParsedOperation {
  type: string;
  description: string;
  details: Record<string, string | number>;
}

export interface HorizonErrorMap {
  [code: string]: string;
}

export interface TrustlineStatus {
  exists: boolean;
  limit?: string;
  balance?: string;
}

// ── Issue #482: Multi-Tenant Whitelabel Types ─────────────────────────────────

export interface WhitelabelTheme {
  tenantId: string;
  tenantName: string;
  primaryColor: string;       // hex/rgba
  secondaryColor: string;
  accentColor: string;
  backgroundColor: string;
  textColor: string;
  fontFamily: string;
  logoUri: string;
  faviconUri: string;
  supportEmail: string;
  supportUrl?: string;
}

export interface FeatureFlagConfig {
  enableStellarSettlement: boolean;
  enableFiatDeposit: boolean;
  enableCryptoWithdrawal: boolean;
  enableMultiCurrency: boolean;
  enableLedgerWallet: boolean;
  enableAnalyticsDashboard: boolean;
}

export interface TenantConfig {
  theme: WhitelabelTheme;
  features: FeatureFlagConfig;
  complianceRules: Record<string, unknown>;
  customCopy: Record<string, string>;
}

export const DEFAULT_TENANT_CONFIG: TenantConfig = {
  theme: {
    tenantId: 'default',
    tenantName: 'Aframp',
    primaryColor: '#3fb950',
    secondaryColor: '#58a6ff',
    accentColor: '#d2a8ff',
    backgroundColor: '#0d1117',
    textColor: '#c9d1d9',
    fontFamily: 'Inter, system-ui, sans-serif',
    logoUri: '/assets/logo.svg',
    faviconUri: '/favicon.ico',
    supportEmail: 'support@aframp.io',
  },
  features: {
    enableStellarSettlement: true,
    enableFiatDeposit: true,
    enableCryptoWithdrawal: true,
    enableMultiCurrency: true,
    enableLedgerWallet: false,
    enableAnalyticsDashboard: true,
  },
  complianceRules: {},
  customCopy: {},
};
