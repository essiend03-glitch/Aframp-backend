'use client';

import { MultisigEnvelope } from '@/types';

interface SignatureProgressMonitorProps {
  envelope: MultisigEnvelope;
}

export function SignatureProgressMonitor({ envelope }: SignatureProgressMonitorProps) {
  const { signoffMatrix, requiredSignatures } = envelope;
  const { accumulatedWeight, requiredWeight, entries, thresholdMet } = signoffMatrix;

  const pct = Math.min(100, Math.round((accumulatedWeight / requiredWeight) * 100));

  return (
    <section className="sig-progress" aria-label="Signature collection progress">
      <header className="sig-progress__header">
        <h3>Signature Progress</h3>
        {thresholdMet && (
          <span className="badge badge--success" aria-live="polite">
            ✓ Threshold Met
          </span>
        )}
      </header>

      <div className="sig-progress__bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
        <div className="sig-progress__fill" style={{ width: `${pct}%` }}>
          <span className="sig-progress__label">{accumulatedWeight} / {requiredWeight}</span>
        </div>
      </div>

      <p className="sig-progress__summary" aria-live="polite">
        <strong>{entries.length}</strong> of <strong>{requiredSignatures}</strong> signatories have approved.
        {!thresholdMet && (
          <> Need <strong>{requiredWeight - accumulatedWeight}</strong> more weight.</>
        )}
      </p>

      {/* Step matrix graph — each signer entry */}
      <ol className="sig-step-matrix" role="list">
        {entries.map((entry, idx) => (
          <li key={entry.signerId} className="sig-step-matrix__item" data-testid={`signature-${idx}`}>
            <div className="sig-step-matrix__icon" aria-hidden="true">
              ✓
            </div>
            <div className="sig-step-matrix__content">
              <strong className="sig-step-matrix__signer">{entry.signerName}</strong>
              <span className="sig-step-matrix__key font-mono-xs">{entry.signerKey}</span>
              <div className="sig-step-matrix__meta">
                <span>Weight: {entry.signerWeight}</span>
                <time dateTime={entry.signedAt}>{new Date(entry.signedAt).toLocaleString()}</time>
                {entry.ipAddress && <span className="font-mono-xs">{entry.ipAddress}</span>}
              </div>
            </div>
          </li>
        ))}

        {/* Placeholder slots for remaining signers */}
        {entries.length < requiredSignatures &&
          Array.from({ length: requiredSignatures - entries.length }).map((_, idx) => (
            <li key={`pending-${idx}`} className="sig-step-matrix__item sig-step-matrix__item--pending">
              <div className="sig-step-matrix__icon" aria-hidden="true">
                ⏳
              </div>
              <div className="sig-step-matrix__content">
                <span className="sig-step-matrix__signer text-muted">Awaiting signatory</span>
              </div>
            </li>
          ))
        }
      </ol>
    </section>
  );
}
