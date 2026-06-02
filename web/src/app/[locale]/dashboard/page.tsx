'use client';

import { useAuth } from '@/lib/auth/auth-context';
import { useLogoutMutation } from '@/hooks/useAuthMutations';
import { RequireAuth } from '@/components/auth/RequireAuth';

function DashboardContent() {
  const { state } = useAuth();
  const logoutMutation = useLogoutMutation();

  const handleLogout = async () => {
    await logoutMutation.mutateAsync();
  };

  if (state.status !== 'AUTHENTICATED') {
    return null;
  }

  return (
    <div style={{ padding: '40px', maxWidth: '1200px', margin: '0 auto' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '32px' }}>
        <h1>Dashboard</h1>
        <button
          onClick={handleLogout}
          style={{
            padding: '8px 16px',
            backgroundColor: '#dc3545',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
          }}
        >
          Logout
        </button>
      </div>

      <div style={{ backgroundColor: '#f8f9fa', padding: '24px', borderRadius: '8px', marginBottom: '24px' }}>
        <h2>Welcome, {state.user.firstName} {state.user.lastName}</h2>
        <p>Email: {state.user.email}</p>
        <p>KYC Status: <strong>{state.user.kycProfile.tier}</strong></p>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: '16px' }}>
        <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '8px', border: '1px solid #dee2e6' }}>
          <h3>Wallet</h3>
          <p>View your balances and transactions</p>
        </div>
        <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '8px', border: '1px solid #dee2e6' }}>
          <h3>Exchange</h3>
          <p>Convert between currencies</p>
        </div>
        <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '8px', border: '1px solid #dee2e6' }}>
          <h3>Transactions</h3>
          <p>View transaction history</p>
        </div>
      </div>
    </div>
  );
}

export default function DashboardPage() {
  return (
    <RequireAuth>
      <DashboardContent />
    </RequireAuth>
  );
}
