/**
 * Normalize raw API or Stellar error objects into clean, actionable messages
 * suitable for display in the institutional portal UI.
 */

export interface MultisigFormError {
  field?: string;
  code: string;
  message: string;
  /** Optional remediation hint shown to operators */
  hint?: string;
}

const ERROR_CODE_MAP: Record<string, Omit<MultisigFormError, 'code'>> = {
  // Signature weight errors
  'INSUFFICIENT_WEIGHT': {
    message: 'Accumulated signer weight is below the required threshold.',
    hint: 'Ensure all designated signatories have submitted their approvals.',
  },
  'WEIGHT_OVERFLOW': {
    message: 'Combined signer weight exceeds the maximum allowed.',
    hint: 'Review quorum configuration — total assigned weight may be misconfigured.',
  },
  'SIGNER_NOT_AUTHORIZED': {
    message: 'Your Stellar key is not listed as an authorized signer for this operation.',
    hint: 'Contact a SuperAdmin to add your key with appropriate weight.',
  },
  'DUPLICATE_SIGNATURE': {
    message: 'You have already signed this proposal.',
  },

  // Sequence / ledger errors
  'BAD_SEQUENCE': {
    message: 'Transaction sequence number is out of bounds.',
    hint: "The source account's sequence may have advanced. Re-fetch the proposal and re-sign.",
  },
  'TX_TOO_LATE': {
    message: 'Transaction time-bound has expired (ledger constraint).',
    hint: 'The proposal must be re-created with a fresh time-bound. Contact the proposer.',
  },
  'TX_TOO_EARLY': {
    message: 'Transaction time-bound has not started yet.',
    hint: 'Wait for the time-lock to elapse before broadcasting.',
  },

  // Time-lock errors
  'TIME_LOCK_ACTIVE': {
    message: 'This governance change is under a time-lock and cannot be broadcast yet.',
    hint: 'The time-lock period protects against rushed governance changes. Wait for it to elapse.',
  },

  // XDR / format errors
  'XDR_DECODE_ERROR': {
    message: 'Failed to decode the transaction XDR.',
    hint: 'The XDR may be corrupted. Do not sign corrupted transactions.',
  },
  'XDR_MISMATCH': {
    message: 'The signed XDR does not match the original unsigned XDR.',
    hint: 'A potential tampering attempt was detected. Do not proceed — report to security.',
  },

  // Generic fallback
  'UNKNOWN': {
    message: 'An unexpected error occurred.',
    hint: 'Check the browser console for technical details and contact support if this persists.',
  },
};

/** Convert a raw error (API response or thrown Error) into a structured MultisigFormError. */
export function normalizeMultisigError(raw: unknown): MultisigFormError {
  if (typeof raw === 'string') {
    const mapped = ERROR_CODE_MAP[raw] ?? ERROR_CODE_MAP['UNKNOWN'];
    return { code: raw, ...mapped };
  }

  if (raw instanceof Error) {
    // Try to extract a known code embedded in the message
    for (const code of Object.keys(ERROR_CODE_MAP)) {
      if (raw.message.includes(code)) {
        return { code, ...ERROR_CODE_MAP[code] };
      }
    }
    return { code: 'UNKNOWN', message: raw.message, hint: ERROR_CODE_MAP['UNKNOWN'].hint };
  }

  if (raw && typeof raw === 'object') {
    const obj = raw as Record<string, unknown>;
    const code = (obj.code ?? obj.error_code ?? 'UNKNOWN') as string;
    const mapped = ERROR_CODE_MAP[code] ?? ERROR_CODE_MAP['UNKNOWN'];
    return {
      code,
      field: typeof obj.field === 'string' ? obj.field : undefined,
      message: typeof obj.message === 'string' ? obj.message : mapped.message,
      hint: mapped.hint,
    };
  }

  return { code: 'UNKNOWN', ...ERROR_CODE_MAP['UNKNOWN'] };
}

