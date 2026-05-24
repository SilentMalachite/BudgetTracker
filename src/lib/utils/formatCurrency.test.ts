import { describe, expect, it } from 'vitest';
import { formatCurrency } from './formatCurrency';

describe('formatCurrency', () => {
  it('formats positive integer yen with comma separators', () => {
    expect(formatCurrency(1234567)).toBe('¥1,234,567');
  });

  it('formats zero', () => {
    expect(formatCurrency(0)).toBe('¥0');
  });

  it('formats negative amounts with sign', () => {
    expect(formatCurrency(-500)).toBe('-¥500');
  });

  it('rejects non-integer values', () => {
    expect(() => formatCurrency(1.5)).toThrow(/integer/);
  });
});
