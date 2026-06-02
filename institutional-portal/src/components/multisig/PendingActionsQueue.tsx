'use client';

import { MultisigEnvelope, MultisigStatus, MultiSigOpType } from '@/types';

const STATUS_LABELS: Record<MultisigStatus, string> = {
  PENDING_SIGNATURES: 'Pending Signatures',
  THRESHOLD_MET:      'Threshold Met',
  BROADCASTING:       'Broadcasting',
  SUCCESS:            'Confirmed',
  FAILED:             'Failed',
  EXPIRED:            'Expired',
};

const OP_LABELS: Record<MultiSigOpType, string> = {
  mint:             'Mint cNGN',
  burn:             'Burn cNGN',
  set_options:      'Set Options',
  add_signer:       'Add Signer',
  remove_signer:    'Remove Signer',
  change_threshold: 'Change Threshold',
};

function timeRemaining(expiresAt: string): string {
  const diff = new Date(expiresAt).getTime() - Date.now();
  if (diff <= 0) return 'Expired';
  const h = Math.floor(diff / 3_600_000);
  const m = Math.floor((diff % 3_600_000) / 60_000);
  return `${h}h ${m}m`;
}

interface PendingActionsQueueProps {
  envelopes: MultisigEnvelope[];
  onSelect: (envelope: MultisigEnvelope) => void;
  selectedId?: string;
}

export function PendingActionsQueue({ envelopes, onSelect, selectedId }: PendingActionsQueueProps) {
  const active = envelopes.filter(e =>
    e.status === 'PENDING_SIGNATURES' || e.status === 'THRESHOLD_MET' || e.status === 'BROADCASTING'
  );

  return (
    <section className="pending-queue" aria-label="Pending multi-sig actions">
      <header className="pending-queue__header">
        <h2>Pending Actions</h2>
        <span className="badge badge--warning" aria-live="polite">{active.length} active</span>
      </header>

      {active.length === 0 ? (
        <p className="empty-state">No pending actions. All transactions are settled.</p>
      ) : (
        <ul className="pending-queue__list" role="list">
          {active.map(env => {
            const pct = Math.min(
              100,
              Math.round((env.signoffMatrix.accumulatedWeight / env.signoffMatrix.requiredWeight) * 100)
            );

            return (
              <li
                key={env.id}
                className={`pending-queue__item${selectedId === env.id ? ' pending-queue__item--selected' : ''}`}
                onClick={() => onSelect(env)}
                role="button"
                tabIndex={0}
                onKeyDown={e => e.key === 'Enter' && onSelect(env)}
                aria-selected={selectedId === env.id}
                aria-label={`${OP_LABELS[env.opType]}: ${env.description}`}
              >
                <div className="pending-queue__row">
                  <span className="pending-queue__op-type">{OP_LABELS[env.opType]}</span>
                  <StatusBadge status={env.status} />
                </div>
                <p className="pending-queue__desc">{env.description}</p>

                {/* Mini signature progress bar */}
                <div className="sig-mini-bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
                  <div className="sig-mini-bar__fill" style={{ width: `${pct}%` }} />
                </div>
                <div className="pending-queue__meta">
                  <span>
                    {env.signoffMatrix.accumulatedWeight}/{env.signoffMatrix.requiredWeight} weight
                  </span>
                  <span>Expires: {timeRemaining(env.expiresAt)}</span>
                </div>

                {env.timeLockRemainingSeconds != null && env.timeLockRemainingSeconds > 0 && (
                  <p className="pending-queue__timelock" aria-label="Time lock active">
                    ⏳ Time-lock: {Math.ceil(env.timeLockRemainingSeconds / 3600)}h remaining
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function StatusBadge({ status }: { status: MultisigStatus }) {
  const cls: Record<MultisigStatus, string> = {
    PENDING_SIGNATURES: 'badge--warning',
    THRESHOLD_MET:      'badge--info',
    BROADCASTING:       'badge--info',
    SUCCESS:            'badge--success',
    FAILED:             'badge--danger',
    EXPIRED:            'badge--muted',
  };
  return <span className={`badge ${cls[status]}`}>{STATUS_LABELS[status]}</span>;
}
