// Issue #479 — Financial Dashboard & Real-Time Ledger Visualization
// Main dashboard layout: metrics grid, live balance cards, transaction table,
// area charts, sparklines, filter pane, and CSV/JSON export.

'use client';

import React, { useState, useMemo, useCallback, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip,
  ResponsiveContainer, LineChart, Line,
} from 'recharts';
import { FixedSizeList as VirtualList } from 'react-window';
import { useLedgerStream } from '../../hooks/useLedgerStream';
import type { LedgerTransaction, WalletBalance, VolumeDataPoint, ConversionRate, DashboardMetrics } from '../../types';

// ── API fetchers ──────────────────────────────────────────────────────────────

const api = (path: string) => fetch(`/api/v1${path}`).then((r) => r.json());

// ── Utility: export ───────────────────────────────────────────────────────────

function exportCSV(rows: LedgerTransaction[]) {
  const start = performance.now();
  const header = 'id,type,status,amount,currency,counterparty,createdAt,stellarTxHash\n';
  const body = rows.map((r) =>
    [r.id, r.type, r.status, r.amount, r.currency, r.counterparty, r.createdAt, r.stellarTxHash ?? ''].join(',')
  ).join('\n');
  const blob = new Blob([header + body], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a'); a.href = url; a.download = 'transactions.csv'; a.click();
  URL.revokeObjectURL(url);
  console.info(`[telemetry] export_csv duration_ms=${(performance.now() - start).toFixed(1)}`);
}

function exportJSON(rows: LedgerTransaction[]) {
  const blob = new Blob([JSON.stringify(rows, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a'); a.href = url; a.download = 'transactions.json'; a.click();
  URL.revokeObjectURL(url);
}

// ── Sub-components ────────────────────────────────────────────────────────────

function ConnectionDot({ state }: { state: string }) {
  if (state === 'open') return null;
  return (
    <span
      role="status"
      aria-live="polite"
      aria-label="WebSocket reconnecting"
      style={{ display: 'inline-flex', alignItems: 'center', gap: 4, fontSize: 12, color: '#f85149' }}
    >
      <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#f85149', display: 'inline-block' }} />
      {state === 'reconnecting' ? 'reconnecting...' : 'disconnected'}
    </span>
  );
}

function BalanceCard({ balance, flash }: { balance: WalletBalance; flash: boolean }) {
  return (
    <div
      role="region"
      aria-label={`${balance.currency} balance`}
      aria-live="polite"
      style={{
        padding: '16px 20px',
        borderRadius: 8,
        background: flash ? 'rgba(63,185,80,0.15)' : '#161b22',
        border: '1px solid #30363d',
        transition: 'background 0.4s ease',
      }}
    >
      <div style={{ fontSize: 12, color: '#8b949e', marginBottom: 4 }}>{balance.currency}</div>
      <div style={{ fontSize: 22, fontWeight: 700, color: '#c9d1d9' }}>
        {balance.available.toLocaleString()}
      </div>
      <div style={{ fontSize: 11, color: '#8b949e', marginTop: 4 }}>
        Pending: {balance.pending.toLocaleString()}
      </div>
    </div>
  );
}

const TX_TYPE_LABELS: Record<string, string> = {
  inbound_deposit: 'Inbound Deposit',
  outbound_payout: 'Outbound Payout',
  stellar_swap: 'Stellar Swap',
};

const TX_TYPE_COLORS: Record<string, string> = {
  inbound_deposit: '#3fb950',
  outbound_payout: '#f85149',
  stellar_swap: '#58a6ff',
};

function TxRow({ tx, flash, style }: { tx: LedgerTransaction; flash: boolean; style: React.CSSProperties }) {
  const [copied, setCopied] = useState(false);

  const copyHash = useCallback(() => {
    if (!tx.stellarTxHash) return;
    navigator.clipboard.writeText(tx.stellarTxHash);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [tx.stellarTxHash]);

  return (
    <div
      role="row"
      style={{
        ...style,
        display: 'grid',
        gridTemplateColumns: '140px 1fr 100px 80px 160px',
        alignItems: 'center',
        padding: '0 16px',
        borderBottom: '1px solid #21262d',
        background: flash ? 'rgba(88,166,255,0.1)' : 'transparent',
        transition: 'background 0.4s ease',
        fontSize: 13,
        color: '#c9d1d9',
      }}
    >
      <span style={{ color: TX_TYPE_COLORS[tx.type] ?? '#c9d1d9' }}>
        {TX_TYPE_LABELS[tx.type] ?? tx.type}
      </span>
      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {tx.counterparty}
      </span>
      <span>{tx.amount.toLocaleString()} {tx.currency}</span>
      <span style={{ color: tx.status === 'settled' ? '#3fb950' : '#f85149' }}>{tx.status}</span>
      <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        {tx.stellarTxHash ? (
          <button
            onClick={copyHash}
            title="Click to copy transaction hash"
            aria-label={`Copy transaction hash ${tx.stellarTxHash}`}
            style={{
              background: 'none', border: 'none', cursor: 'pointer', color: '#58a6ff',
              fontFamily: 'monospace', fontSize: 12, padding: 0,
              maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
            }}
          >
            {tx.stellarTxHash.slice(0, 8)}…{tx.stellarTxHash.slice(-6)}
          </button>
        ) : '—'}
        {copied && <span style={{ fontSize: 10, color: '#3fb950' }}>Copied!</span>}
      </span>
    </div>
  );
}

// ── Filter Pane (issue #479 §3) ───────────────────────────────────────────────

interface Filters {
  type: string;
  status: string;
  minAmount: string;
  maxAmount: string;
  dateFrom: string;
  dateTo: string;
  corridor: string;
}

const EMPTY_FILTERS: Filters = { type: '', status: '', minAmount: '', maxAmount: '', dateFrom: '', dateTo: '', corridor: '' };

function FilterPane({ filters, onChange }: { filters: Filters; onChange: (f: Filters) => void }) {
  const set = (key: keyof Filters) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
    onChange({ ...filters, [key]: e.target.value });

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginBottom: 12 }}>
      <select value={filters.type} onChange={set('type')} aria-label="Filter by type" style={selectStyle}>
        <option value="">All Types</option>
        <option value="inbound_deposit">Inbound Deposit</option>
        <option value="outbound_payout">Outbound Payout</option>
        <option value="stellar_swap">Stellar Swap</option>
      </select>
      <select value={filters.status} onChange={set('status')} aria-label="Filter by status" style={selectStyle}>
        <option value="">All Statuses</option>
        <option value="settled">Settled</option>
        <option value="pending">Pending</option>
        <option value="failed">Failed</option>
      </select>
      <input type="number" placeholder="Min amount" value={filters.minAmount} onChange={set('minAmount')} aria-label="Minimum amount" style={inputStyle} />
      <input type="number" placeholder="Max amount" value={filters.maxAmount} onChange={set('maxAmount')} aria-label="Maximum amount" style={inputStyle} />
      <input type="date" value={filters.dateFrom} onChange={set('dateFrom')} aria-label="Date from" style={inputStyle} />
      <input type="date" value={filters.dateTo} onChange={set('dateTo')} aria-label="Date to" style={inputStyle} />
      <input type="text" placeholder="Corridor (e.g. NGN/cNGN)" value={filters.corridor} onChange={set('corridor')} aria-label="Filter by corridor" style={inputStyle} />
      <button onClick={() => onChange(EMPTY_FILTERS)} style={btnStyle}>Clear</button>
    </div>
  );
}

const selectStyle: React.CSSProperties = { background: '#161b22', color: '#c9d1d9', border: '1px solid #30363d', borderRadius: 6, padding: '4px 8px', fontSize: 12 };
const inputStyle: React.CSSProperties = { ...selectStyle, width: 130 };
const btnStyle: React.CSSProperties = { ...selectStyle, cursor: 'pointer' };

// ── Main Dashboard ────────────────────────────────────────────────────────────

export default function FinancialDashboard() {
  const { readyState, flashIds } = useLedgerStream();
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS);

  const { data: metrics } = useQuery<DashboardMetrics>({ queryKey: ['metrics'], queryFn: () => api('/dashboard/metrics') });
  const { data: transactions = [] } = useQuery<LedgerTransaction[]>({ queryKey: ['transactions'], queryFn: () => api('/transactions?limit=1000') });
  const { data: balances = [] } = useQuery<WalletBalance[]>({ queryKey: ['balances'], queryFn: () => api('/wallet/balances') });
  const { data: volumeData = [] } = useQuery<VolumeDataPoint[]>({ queryKey: ['volume'], queryFn: () => api('/analytics/volume?range=30d') });
  const { data: rates = [] } = useQuery<ConversionRate[]>({ queryKey: ['rates'], queryFn: () => api('/rates/corridors') });

  // Apply filters — sub-5ms for thousands of records via simple array filter
  const filtered = useMemo(() => {
    const minAmt = filters.minAmount ? parseFloat(filters.minAmount) : -Infinity;
    const maxAmt = filters.maxAmount ? parseFloat(filters.maxAmount) : Infinity;
    return transactions.filter((tx) => {
      if (filters.type && tx.type !== filters.type) return false;
      if (filters.status && tx.status !== filters.status) return false;
      if (tx.amount < minAmt || tx.amount > maxAmt) return false;
      if (filters.dateFrom && tx.createdAt < filters.dateFrom) return false;
      if (filters.dateTo && tx.createdAt > filters.dateTo + 'T23:59:59') return false;
      if (filters.corridor && tx.corridor !== filters.corridor) return false;
      return true;
    });
  }, [transactions, filters]);

  const ROW_HEIGHT = 44;

  return (
    <div style={{ fontFamily: 'var(--font-family, Inter, system-ui, sans-serif)', color: 'var(--color-text, #c9d1d9)', padding: 24 }}>
      {/* Connection status */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
        <ConnectionDot state={readyState} />
      </div>

      {/* Metrics grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16, marginBottom: 24 }}>
        <MetricCard label="Net Wallet Equity" value={metrics?.netWalletEquity} />
        <MetricCard label="Active Liquidity Limit" value={metrics?.activeLiquidityLimit} />
        <MetricCard label="Pending Transactions" value={metrics?.pendingTransactions} integer />
        <MetricCard label="Settled Today" value={metrics?.settledToday} integer />
      </div>

      {/* Balance cards */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: 12, marginBottom: 24 }}>
        {balances.map((b) => (
          <BalanceCard key={b.currency} balance={b} flash={flashIds.has(`balance-${b.currency}`)} />
        ))}
      </div>

      {/* Area chart — transaction volume */}
      <section aria-label="Transaction volume chart" style={cardStyle}>
        <h2 style={h2Style}>Transaction Volume (30d)</h2>
        <ResponsiveContainer width="100%" height={200}>
          <AreaChart data={volumeData}>
            <defs>
              <linearGradient id="volGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#3fb950" stopOpacity={0.3} />
                <stop offset="95%" stopColor="#3fb950" stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="#21262d" />
            <XAxis dataKey="timestamp" tick={{ fill: '#8b949e', fontSize: 11 }} tickFormatter={(v) => v.slice(5, 10)} />
            <YAxis tick={{ fill: '#8b949e', fontSize: 11 }} />
            <Tooltip contentStyle={{ background: '#161b22', border: '1px solid #30363d', color: '#c9d1d9' }} />
            <Area type="monotone" dataKey="volume" stroke="#3fb950" fill="url(#volGrad)" strokeWidth={1.5} />
          </AreaChart>
        </ResponsiveContainer>
      </section>

      {/* Sparkline matrix — conversion rates */}
      <section aria-label="Conversion rate sparklines" style={cardStyle}>
        <h2 style={h2Style}>Live Conversion Rates</h2>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12 }}>
          {rates.map((r) => (
            <div key={r.pair} style={{ background: '#161b22', borderRadius: 6, padding: '10px 14px', border: '1px solid #30363d' }}>
              <div style={{ fontSize: 12, color: '#8b949e', marginBottom: 4 }}>{r.pair}</div>
              <div style={{ fontSize: 18, fontWeight: 700 }}>{r.rate.toFixed(4)}</div>
              <ResponsiveContainer width="100%" height={40}>
                <LineChart data={[{ v: r.open }, { v: r.high }, { v: r.low }, { v: r.close }]}>
                  <Line type="monotone" dataKey="v" stroke="#58a6ff" dot={false} strokeWidth={1.5} />
                </LineChart>
              </ResponsiveContainer>
            </div>
          ))}
        </div>
      </section>

      {/* Transaction table with virtualization */}
      <section aria-label="Transaction ledger" style={cardStyle}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <h2 style={{ ...h2Style, margin: 0 }}>Ledger ({filtered.length.toLocaleString()} records)</h2>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={() => exportCSV(filtered)} style={btnStyle} aria-label="Export as CSV">Export CSV</button>
            <button onClick={() => exportJSON(filtered)} style={btnStyle} aria-label="Export as JSON">Export JSON</button>
          </div>
        </div>
        <FilterPane filters={filters} onChange={setFilters} />
        {/* Table header */}
        <div role="rowgroup" style={{ display: 'grid', gridTemplateColumns: '140px 1fr 100px 80px 160px', padding: '8px 16px', borderBottom: '1px solid #30363d', fontSize: 11, color: '#8b949e', textTransform: 'uppercase' }}>
          <span>Type</span><span>Counterparty</span><span>Amount</span><span>Status</span><span>Tx Hash</span>
        </div>
        {/* Virtualized rows — maintains 60fps under thousands of records */}
        <VirtualList
          height={Math.min(filtered.length * ROW_HEIGHT, 480)}
          itemCount={filtered.length}
          itemSize={ROW_HEIGHT}
          width="100%"
          aria-label="Transaction rows"
        >
          {({ index, style }) => {
            const tx = filtered[index];
            return <TxRow key={tx.id} tx={tx} flash={flashIds.has(tx.id)} style={style} />;
          }}
        </VirtualList>
      </section>
    </div>
  );
}

function MetricCard({ label, value, integer }: { label: string; value?: number; integer?: boolean }) {
  return (
    <div style={{ background: '#161b22', border: '1px solid #30363d', borderRadius: 8, padding: '16px 20px' }}>
      <div style={{ fontSize: 11, color: '#8b949e', textTransform: 'uppercase', marginBottom: 6 }}>{label}</div>
      <div style={{ fontSize: 24, fontWeight: 700 }}>
        {value == null ? '—' : integer ? value.toLocaleString() : value.toLocaleString(undefined, { maximumFractionDigits: 2 })}
      </div>
    </div>
  );
}

const cardStyle: React.CSSProperties = { background: '#0d1117', border: '1px solid #30363d', borderRadius: 8, padding: 20, marginBottom: 20 };
const h2Style: React.CSSProperties = { fontSize: 15, fontWeight: 600, marginBottom: 16, color: '#c9d1d9' };
