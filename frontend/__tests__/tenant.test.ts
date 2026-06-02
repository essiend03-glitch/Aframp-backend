// Issue #482 — Unit Tests: domain parsing, hex validation, theme fallbacks

// ── Domain / host parsing ─────────────────────────────────────────────────────

function extractSubdomain(host: string): string {
  const clean = host.split(':')[0];
  const parts = clean.split('.');
  if (parts.length >= 3) return parts[0];
  return 'default';
}

describe('extractSubdomain', () => {
  it('extracts subdomain from three-part host', () => {
    expect(extractSubdomain('zenith.aframp.io')).toBe('zenith');
  });

  it('returns default for two-part host', () => {
    expect(extractSubdomain('aframp.io')).toBe('default');
  });

  it('strips port before parsing', () => {
    expect(extractSubdomain('uba.aframp.io:3000')).toBe('uba');
  });
});

// ── Hex color validation ──────────────────────────────────────────────────────

function isValidHex(color: string): boolean {
  return /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(color);
}

describe('isValidHex', () => {
  it('accepts 6-digit hex', () => {
    expect(isValidHex('#3fb950')).toBe(true);
  });

  it('accepts 3-digit hex', () => {
    expect(isValidHex('#fff')).toBe(true);
  });

  it('accepts 8-digit hex with alpha', () => {
    expect(isValidHex('#3fb95080')).toBe(true);
  });

  it('rejects invalid hex', () => {
    expect(isValidHex('not-a-color')).toBe(false);
    expect(isValidHex('#gg0000')).toBe(false);
    expect(isValidHex('')).toBe(false);
  });
});

// ── Theme fallback ────────────────────────────────────────────────────────────

interface Theme { primaryColor: string; fontFamily: string }

function safeColor(val: string, fallback: string): string {
  return isValidHex(val) || val.startsWith('rgba(') || val.startsWith('rgb(') ? val : fallback;
}

describe('safeColor', () => {
  it('returns valid hex as-is', () => {
    expect(safeColor('#3fb950', '#000')).toBe('#3fb950');
  });

  it('returns fallback for invalid color', () => {
    expect(safeColor('not-a-color', '#000')).toBe('#000');
  });

  it('accepts rgba values', () => {
    expect(safeColor('rgba(63,185,80,0.5)', '#000')).toBe('rgba(63,185,80,0.5)');
  });
});

// ── Feature flag isolation ────────────────────────────────────────────────────

interface FeatureFlags { enableStellarSettlement: boolean; enableFiatDeposit: boolean }

function isFeatureEnabled(flags: FeatureFlags, key: keyof FeatureFlags): boolean {
  return flags[key] === true;
}

describe('isFeatureEnabled', () => {
  const flags: FeatureFlags = { enableStellarSettlement: true, enableFiatDeposit: false };

  it('returns true for enabled feature', () => {
    expect(isFeatureEnabled(flags, 'enableStellarSettlement')).toBe(true);
  });

  it('returns false for disabled feature', () => {
    expect(isFeatureEnabled(flags, 'enableFiatDeposit')).toBe(false);
  });
});

// ── Custom copy substitution ──────────────────────────────────────────────────

function substituteText(template: string, copy: Record<string, string>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => copy[key] ?? `{{${key}}}`);
}

describe('substituteText', () => {
  it('replaces known keys', () => {
    expect(substituteText('Welcome to {{platformName}}', { platformName: 'ZenithPay' })).toBe('Welcome to ZenithPay');
  });

  it('leaves unknown keys as-is', () => {
    expect(substituteText('Hello {{unknown}}', {})).toBe('Hello {{unknown}}');
  });

  it('handles multiple substitutions', () => {
    const result = substituteText('{{a}} and {{b}}', { a: 'foo', b: 'bar' });
    expect(result).toBe('foo and bar');
  });
});
