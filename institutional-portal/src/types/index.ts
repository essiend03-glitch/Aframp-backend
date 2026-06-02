// ─── Institutional Roles ──────────────────────────────────────────────────────

export type InstitutionalRole =
  | 'SuperAdmin'
  | 'Operator'
  | 'ComplianceAuditor'
  | 'Signatory';

export interface InstitutionalUser {
  id: string;
  name: string;
  email: string;
  role: InstitutionalRole;
  stellarPublicKey?: string;
  /** Stellar signer weight (1–20); only meaningful for Signatory role */
  signerWeight: number;
  ipWhitelist: string[];
  isActive: boolean;
  createdAt: string;
  lastLoginAt?: string;
}

// ─── Permission Matrix ────────────────────────────────────────────────────────

export const ROLE_PERMISSIONS: Record<InstitutionalRole, string[]> = {
  SuperAdmin: [
    'proposals:create', 'proposals:read', 'proposals:reject',
    'signatures:submit',
    'config:read', 'config:write',
    'compliance:read',
    'users:read', 'users:write',
  ],
  Operator: [
    'proposals:create', 'proposals:read',
    'config:read',
  ],
  ComplianceAuditor: [
    'proposals:read',
    'compliance:read',
    'config:read',
  ],
  Signatory: [
    'proposals:read',
    'signatures:submit',
  ],
};

export function hasPermission(role: InstitutionalRole, permission: string): boolean {
  return ROLE_PERMISSIONS[role]?.includes(permission) ?? false;
}

// ─── Multi-Sig Operation Types ────────────────────────────────────────────────

export type MultiSigOpType =
  | 'mint'
  | 'burn'
  | 'set_options'
  | 'add_signer'
  | 'remove_signer'
  | 'change_threshold';

// ─── Execution Status (discriminated union) ───────────────────────────────────

export type MultisigStatus =
  | 'PENDING_SIGNATURES'
  | 'THRESHOLD_MET'
  | 'BROADCASTING'
  | 'SUCCESS'
  | 'FAILED'
  | 'EXPIRED';

// ─── Threshold Configuration ──────────────────────────────────────────────────

export type ThresholdTier = 'low' | 'medium' | 'high';

export interface ThresholdConfig {
  tier: ThresholdTier;
  /** Minimum accumulated signer weight required */
  requiredWeight: number;
  /** Human label, e.g. "3-of-5" */
  label: string;
  timeLockSeconds: number;
}

export const DEFAULT_THRESHOLDS: Record<MultiSigOpType, ThresholdConfig> = {
  mint:             { tier: 'medium', requiredWeight: 3, label: '3-of-5', timeLockSeconds: 0 },
  burn:             { tier: 'medium', requiredWeight: 3, label: '3-of-5', timeLockSeconds: 0 },
  set_options:      { tier: 'medium', requiredWeight: 3, label: '3-of-5', timeLockSeconds: 0 },
  add_signer:       { tier: 'high',   requiredWeight: 3, label: '3-of-5', timeLockSeconds: 172800 },
  remove_signer:    { tier: 'high',   requiredWeight: 3, label: '3-of-5', timeLockSeconds: 172800 },
  change_threshold: { tier: 'high',   requiredWeight: 4, label: '4-of-5', timeLockSeconds: 172800 },
};

// ─── Signoff Matrix ───────────────────────────────────────────────────────────

export interface SignoffEntry {
  signerId: string;
  signerKey: string;
  signerName: string;
  signerWeight: number;
  signedAt: string;
  ipAddress?: string;
}

export interface SignoffMatrix {
  proposalId: string;
  requiredWeight: number;
  accumulatedWeight: number;
  entries: SignoffEntry[];
  /** true when accumulatedWeight >= requiredWeight */
  thresholdMet: boolean;
}

// ─── Multi-Sig Envelope ───────────────────────────────────────────────────────

export interface MultisigEnvelope {
  id: string;
  opType: MultiSigOpType;
  description: string;
  /** Base64-encoded unsigned Stellar XDR */
  unsignedXdr: string;
  /** Base64-encoded partially/fully signed XDR (null until first sig) */
  signedXdr: string | null;
  stellarTxHash: string | null;
  requiredSignatures: number;
  totalSigners: number;
  signoffMatrix: SignoffMatrix;
  timeLockUntil: string | null;
  timeLockRemainingSeconds: number | null;
  status: MultisigStatus;
  failureReason: string | null;
  proposedBy: string;
  proposedByKey: string;
  expiresAt: string;
  createdAt: string;
  updatedAt: string;
}

// ─── Compliance / Audit Trail ─────────────────────────────────────────────────

export type AuditEventType =
  | 'proposal_created'
  | 'signature_submitted'
  | 'threshold_met'
  | 'time_lock_started'
  | 'time_lock_elapsed'
  | 'transaction_submitted'
  | 'transaction_confirmed'
  | 'proposal_rejected'
  | 'proposal_expired';

export interface AuditEntry {
  id: string;
  proposalId: string | null;
  eventType: AuditEventType;
  actorKey: string | null;
  actorId: string | null;
  actorName?: string;
  payload: Record<string, unknown>;
  /** SHA-256 of (previousHash + payload) — append-only chain */
  currentHash: string;
  previousHash: string | null;
  createdAt: string;
}

// ─── Telemetry Vectors ────────────────────────────────────────────────────────

export interface TelemetryEvent {
  name: 'multisig_approval_latency' | 'portal_access_denied' | 'partial_signature_submitted';
  value?: number;
  labels: Record<string, string>;
  ts: number;
}

// ─── API Response wrapper ─────────────────────────────────────────────────────

export interface ApiResponse<T> {
  data: T;
  error?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}
