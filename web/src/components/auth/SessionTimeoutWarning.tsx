'use client';

import { useEffect, useState } from 'react';
import { useAuth } from '@/lib/auth/auth-context';

export function SessionTimeoutWarning() {
  const { state, refreshSession, logout } = useAuth();
  const [showWarning, setShowWarning] = useState(false);

  useEffect(() => {
    if (state.status === 'SESSION_EXPIRED') {
      setShowWarning(true);
    } else {
      setShowWarning(false);
    }
  }, [state.status]);

  if (!showWarning) return null;

  return (
    <div style={{
      position: 'fixed',
      top: '20px',
      right: '20px',
      backgroundColor: '#fff3cd',
      border: '1px solid #ffc107',
      borderRadius: '8px',
      padding: '16px',
      boxShadow: '0 4px 6px rgba(0,0,0,0.1)',
      zIndex: 9999,
      maxWidth: '400px',
    }}>
      <h3 style={{ margin: '0 0 8px 0' }}>Session Expired</h3>
      <p style={{ margin: '0 0 16px 0' }}>Your session has expired. Please log in again.</p>
      <button
        onClick={() => logout()}
        style={{
          backgroundColor: '#007bff',
          color: 'white',
          border: 'none',
          padding: '8px 16px',
          borderRadius: '4px',
          cursor: 'pointer',
        }}
      >
        Log In Again
      </button>
    </div>
  );
}
