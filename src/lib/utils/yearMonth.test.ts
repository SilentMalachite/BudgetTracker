import { afterEach, describe, expect, it, vi } from 'vitest';
import { isoToday, monthRange } from './yearMonth';

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
