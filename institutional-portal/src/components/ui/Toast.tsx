'use client';

import React, { createContext, useCallback, useContext, useRef, useState } from 'react';

export type ToastVariant = 'info' | 'success' | 'warning' | 'danger';

export interface Toast {
  id: string;
  title: string;
  message?: string;
  variant: ToastVariant;
  /** Auto-dismiss after ms. 0 = sticky. Default 5000. */
  duration?: number;
}

interface ToastContextValue {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, 'id'>) => void;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const dismiss = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
    const timer = timers.current.get(id);
    if (timer) { clearTimeout(timer); timers.current.delete(id); }
  }, []);

  const addToast = useCallback((toast: Omit<Toast, 'id'>) => {
    const id = crypto.randomUUID();
    setToasts(prev => [{ ...toast, id }, ...prev]);

    const duration = toast.duration ?? 5000;
    if (duration > 0) {
      timers.current.set(id, setTimeout(() => dismiss(id), duration));
    }
  }, [dismiss]);

  return (
    <ToastContext.Provider value={{ toasts, addToast, dismiss }}>
      {children}
      <ToastContainer toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be within ToastProvider');
  return ctx;
}

/** Convenience wrappers */
export function useMultisigToast() {
  const { addToast } = useToast();

  return {
    notifySignatureRequired: (proposalId: string, opType: string) =>
      addToast({
        variant: 'warning',
        title: 'Signature Required',
        message: `Your key weight is needed for a ${opType} operation (ID: …${proposalId.slice(-6)}).`,
        duration: 0, // sticky — must be dismissed manually
      }),
    notifyThresholdMet: (proposalId: string) =>
      addToast({
        variant: 'success',
        title: 'Threshold Met',
        message: `Proposal …${proposalId.slice(-6)} has enough signatures and is ready to broadcast.`,
      }),
    notifyConfirmed: (txHash: string) =>
      addToast({
        variant: 'success',
        title: 'Transaction Confirmed',
        message: `On-chain hash: ${txHash.slice(0, 16)}…`,
      }),
    notifyError: (title: string, message: string) =>
      addToast({ variant: 'danger', title, message }),
  };
}

/* ─── Display ─── */

const VARIANT_ICON: Record<ToastVariant, string> = {
  info:    'ℹ',
  success: '✓',
  warning: '⚠',
  danger:  '✖',
};

function ToastContainer({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: string) => void }) {
  if (toasts.length === 0) return null;
  return (
    <div
      className="toast-container"
      role="region"
      aria-label="Notifications"
      aria-live="polite"
    >
      {toasts.map(t => (
        <div key={t.id} className={`toast toast--${t.variant}`} role="alert">
          <span className="toast__icon" aria-hidden="true">{VARIANT_ICON[t.variant]}</span>
          <div className="toast__content">
            <strong className="toast__title">{t.title}</strong>
            {t.message && <p className="toast__message">{t.message}</p>}
          </div>
          <button
            className="toast__dismiss"
            onClick={() => onDismiss(t.id)}
            aria-label="Dismiss notification"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
