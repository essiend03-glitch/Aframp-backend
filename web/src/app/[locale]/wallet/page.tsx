'use client';

import { RequireAuth } from '@/components/auth/RequireAuth';
import { RequireKYC } from '@/components/auth/RequireKYC';

function WalletContent() {
  return (
    <div style={{ padding: '40px', maxWidth: '1200px', margin: '0 auto' }}>
      <h1>Wallet</h1>
      <p>This page requires KYC Level 1 verification.</p>
      
      <div style={{ marginTop: '32px' }}>
        <h2>Your Balances</h2>
        <div style={{ backgroundColor: '#f8f9fa', padding: '24px', borderRadius: '8px' }}>
          <p>NGN: ₦0.00</p>
          <p>USD: $0.00</p>
          <p>cNGN: 0.00</p>
        </div>
      </div>
    </div>
  );
}

export default function WalletPage() {
  return (
    <RequireAuth>
      <RequireKYC level="KYC_Level_1">
        <WalletContent />
      </RequireKYC>
    </RequireAuth>
  );
}
