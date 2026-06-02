'use client';

import { AuditEntry, AuditEventType } from '@/types';

const EVENT_LABELS: Record<AuditEventType, string> = {
  proposal_created:        'Proposal Created',
  signature_submitted:     'Signature Submitted',
  threshold_met:           'Threshold Met',
  time_lock_started:       'Time-lock Started',
  time_lock_elapsed:       'Time-lock Elapsed',
  transaction_submitted:   'Transaction Submitted to Stellar',
  transaction_confirmed:   'On-chain Confirmation',
  proposal_rejected:       'Proposal Rejected',
  proposal_expired:        'Proposal Expired',
};

const EVENT_ICON: Record<AuditEventType, string> = {
  proposal_created:        '📝',
  signature_submitted:     '✍',
  threshold_met:           '✅',
  time_lock_started:       '⏳',
  time_lock_elapsed:       '✔',
  transaction_submitted:   '🚀',
  transaction_confirmed:   '⛓',
  proposal_rejected:       '✖',
  proposal_expired:        '⌛',
};

const EVENT_VARIANT: Record<AuditEventType, string> = {
  proposal_created:        'neutral',
  signature_submitted:     'info',
  threshold_met:           'success',
  time_lock_started:       'warning',
  time_lock_elapsed:       'info',
  transaction_submitted:   'info',
  transaction_confirmed:   'success',
  proposal_rejected:       'danger',
  proposal_expired:        'muted',
};

interface ComplianceTimelineProps {
  entries: AuditEntry[];
  /** If provided, only show entries for this proposal */
  proposalId?: string;
}

export function ComplianceTimeline({ entries, proposalId }: ComplianceTimelineProps) {
  const filtered = proposalId
    ? entries.filter(e => e.proposalId === proposalId)
    : entries;

  // Chronological order
  const sorted = [...filtered].sort(
    (a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
  );

  return (
    <section className="compliance-timeline" aria-label="Compliance audit timeline">
      <header className="compliance-timeline__header">
        <h2>Compliance Audit Trail</h2>
        <span className="text-muted">{sorted.length} events</span>
      </header>

      {sorted.length === 0 ? (
        <p className="empty-state">No audit entries found.</p>
      ) : (
        <ol className="timeline" role="list">
          {sorted.map((entry, idx) => (
            <li
              key={entry.id}
              className={`timeline__event timeline__event--${EVENT_VARIANT[entry.eventType]}`}
              data-testid={`audit-event-${idx}`}
            >
              <div className="timeline__connector" aria-hidden="true" />
              <div className="timeline__dot" aria-hidden="true">
                {EVENT_ICON[entry.eventType]}
              </div>

              <div className="timeline__body">
                <div className="timeline__meta">
                  <strong className="timeline__event-type">
                    {EVENT_LABELS[entry.eventType]}
                  </strong>
                  <time className="timeline__time" dateTime={entry.createdAt}>
                    {new Date(entry.createdAt).toLocaleString()}
                  </time>
                </div>

                {entry.actorName && (
                  <p className="timeline__actor">
                    Actor: <span className="font-mono-sm">{entry.actorName}</span>
                  </p>
                )}
                {entry.actorKey && (
                  <p className="timeline__actor-key font-mono-xs">{entry.actorKey}</p>
                )}

                {/* Expandable payload */}
                {Object.keys(entry.payload).length > 0 && (
                  <details className="timeline__payload">
                    <summary>View payload</summary>
                    <pre>{JSON.stringify(entry.payload, null, 2)}</pre>
                  </details>
                )}

                {/* Chain integrity */}
                <div className="timeline__hash">
                  <span className="font-mono-xs text-muted">
                    Hash: {entry.currentHash.slice(0, 16)}…
                  </span>
                  {idx > 0 && entry.previousHash && (
                    <span
                      className="timeline__chain-link font-mono-xs text-muted"
                      title={`Links to: ${entry.previousHash}`}
                      aria-label="Linked to previous entry"
                    >
                      ← {entry.previousHash.slice(0, 8)}
                    </span>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ol>
      )}

      {/* Stellar anchor link for final confirmation */}
      {sorted.some(e => e.eventType === 'transaction_confirmed') && (
        <div className="compliance-timeline__anchor">
          <StellarExplorerLink entries={sorted} />
        </div>
      )}
    </section>
  );
}

function StellarExplorerLink({ entries }: { entries: AuditEntry[] }) {
  const confirmEntry = entries.find(e => e.eventType === 'transaction_confirmed');
  const txHash = confirmEntry?.payload?.tx_hash as string | undefined;

  if (!txHash) return null;
  return (
    <a
      href={`https://stellar.expert/explorer/public/tx/${txHash}`}
      target="_blank"
      rel="noopener noreferrer"
      className="btn btn--ghost btn--sm"
    >
      View on Stellar Expert ↗
    </a>
  );
}
