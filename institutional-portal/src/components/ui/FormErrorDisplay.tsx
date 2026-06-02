'use client';
import { MultisigFormError } from '@/lib/formErrors';

export function FormErrorDisplay({ error }: { error: MultisigFormError }) {
  return (
    <div className="form-error" role="alert" aria-live="assertive">
      <p className="form-error__message">
        <strong>[{error.code}]</strong> {error.message}
      </p>
      {error.hint && <p className="form-error__hint">{error.hint}</p>}
    </div>
  );
}
