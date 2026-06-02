// Issue #479 — Unit Tests: chart aggregation, date formatting, string masking

// ── Date formatting ───────────────────────────────────────────────────────────

function formatTimestamp(iso: string, locale = 'en-NG'): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso));
}

describe('formatTimestamp', () => {
  it('formats ISO string to locale date', () => {
    const result = formatTimestamp('2026-06-01T12:00:00Z', 'en-US');
    expect(result).toMatch(/Jun/);
    expect(result).toMatch(/2026/);
  });

  it('handles different locales without throwing', () => {
    expect(() => formatTimestamp('2026-01-15T08:30:00Z', 'fr-FR')).not.toThrow();
  });
});

// ── Numeric precision ─────────────────────────────────────────────────────────

function formatAmount(value: number, decimals = 2): string {
  return value.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

describe('formatAmount', () => {
  it('formats with correct decimal places', () => {
    expect(formatAmount(1234567.891, 2)).toBe('1,234,567.89');
  });

  it('handles zero', () => {
    expect(formatAmount(0)).toBe('0.00');
  });

  it('handles large numbers', () => {
    expect(formatAmount(1_000_000_000)).toBe('1,000,000,000.00');
  });
});

// ── String masking (hash truncation) ─────────────────────────────────────────

function maskHash(hash: string, prefixLen = 8, suffixLen = 6): string {
  if (hash.length <= prefixLen + suffixLen) return hash;
  return `${hash.slice(0, prefixLen)}…${hash.slice(-suffixLen)}`;
}

describe('maskHash', () => {
  it('truncates long hashes', () => {
    const hash = 'a'.repeat(64);
    const result = maskHash(hash);
    expect(result).toBe('aaaaaaaa…aaaaaa');
    expect(result.length).toBeLessThan(hash.length);
  });

  it('returns short hashes unchanged', () => {
    expect(maskHash('abc123')).toBe('abc123');
  });
});

// ── Volume aggregation ────────────────────────────────────────────────────────

interface DataPoint { timestamp: string; volume: number }

function aggregateVolume(points: DataPoint[]): number {
  return points.reduce((sum, p) => sum + p.volume, 0);
}

function averageVolume(points: DataPoint[]): number {
  if (points.length === 0) return 0;
  return aggregateVolume(points) / points.length;
}

describe('aggregateVolume', () => {
  const data: DataPoint[] = [
    { timestamp: '2026-06-01', volume: 1000 },
    { timestamp: '2026-06-02', volume: 2000 },
    { timestamp: '2026-06-03', volume: 3000 },
  ];

  it('sums all volumes', () => {
    expect(aggregateVolume(data)).toBe(6000);
  });

  it('computes average', () => {
    expect(averageVolume(data)).toBe(2000);
  });

  it('handles empty array', () => {
    expect(aggregateVolume([])).toBe(0);
    expect(averageVolume([])).toBe(0);
  });
});

// ── Filter logic ──────────────────────────────────────────────────────────────

interface Tx { type: string; status: string; amount: number; createdAt: string }

function applyFilters(txs: Tx[], type: string, status: string, minAmt: number, maxAmt: number): Tx[] {
  return txs.filter((t) => {
    if (type && t.type !== type) return false;
    if (status && t.status !== status) return false;
    if (t.amount < minAmt || t.amount > maxAmt) return false;
    return true;
  });
}

describe('applyFilters', () => {
  const txs: Tx[] = [
    { type: 'inbound_deposit', status: 'settled', amount: 500, createdAt: '2026-06-01' },
    { type: 'outbound_payout', status: 'pending', amount: 1500, createdAt: '2026-06-02' },
    { type: 'stellar_swap', status: 'settled', amount: 3000, createdAt: '2026-06-03' },
  ];

  it('filters by type', () => {
    expect(applyFilters(txs, 'inbound_deposit', '', 0, Infinity)).toHaveLength(1);
  });

  it('filters by status', () => {
    expect(applyFilters(txs, '', 'settled', 0, Infinity)).toHaveLength(2);
  });

  it('filters by amount range', () => {
    expect(applyFilters(txs, '', '', 1000, 2000)).toHaveLength(1);
  });

  it('returns all when no filters', () => {
    expect(applyFilters(txs, '', '', 0, Infinity)).toHaveLength(3);
  });
});
