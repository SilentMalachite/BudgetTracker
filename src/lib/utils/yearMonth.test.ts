import { afterEach, describe, expect, it, vi } from 'vitest';
import { isYearMonth, isoToday, monthRange } from './yearMonth';

describe('isYearMonth', () => {
  it('accepts YYYY-MM with a month between 01 and 12', () => {
    expect(isYearMonth('2026-01')).toBe(true);
    expect(isYearMonth('2026-12')).toBe(true);
  });

  it('rejects a cleared or partially typed month input', () => {
    expect(isYearMonth('')).toBe(false);
    expect(isYearMonth('2026')).toBe(false);
    expect(isYearMonth('2026-')).toBe(false);
    expect(isYearMonth('2026-1')).toBe(false);
    expect(isYearMonth('2026-00')).toBe(false);
    expect(isYearMonth('2026-13')).toBe(false);
    expect(isYearMonth('2026-02-01')).toBe(false);
  });
});

describe('isoToday', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns local YYYY-MM-DD and does not use toISOString for the date', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 5, 1, 0, 30, 0)); // local 2026-06-01 00:30
    const isoSpy = vi.spyOn(Date.prototype, 'toISOString');
    expect(isoToday()).toBe('2026-06-01');
    expect(isoSpy).not.toHaveBeenCalled();
  });
});

describe('monthRange', () => {
  it('returns inclusive local month bounds', () => {
    expect(monthRange(2026, 2)).toEqual({ from: '2026-02-01', to: '2026-02-28' });
    expect(monthRange(2024, 2)).toEqual({ from: '2024-02-01', to: '2024-02-29' });
  });
});
