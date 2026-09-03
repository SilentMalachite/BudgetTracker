import { describe, expect, it, vi } from 'vitest';

const reportMonthly = vi.fn();
const reportYearly = vi.fn();
const reportByCategory = vi.fn();
const reportNetWorthSeries = vi.fn();

vi.mock('../api/reports', () => ({
  reportMonthly: (...args: unknown[]) => reportMonthly(...args),
  reportYearly: (...args: unknown[]) => reportYearly(...args),
  reportByCategory: (...args: unknown[]) => reportByCategory(...args),
  reportNetWorthSeries: (...args: unknown[]) => reportNetWorthSeries(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: () => Promise.reject(new Error('no tauri event bus')),
}));

const { createReportsStore } = await import('./reports.svelte');

const emptyMonthly = {
  current: { income: 0, expense: 0, net: 0 },
  prev_month: { income: 0, expense: 0, net: 0 },
  prev_year: { income: 0, expense: 0, net: 0 },
  mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
  yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
  top_expense: [],
  top_income: [],
};

function stubAll() {
  reportMonthly.mockResolvedValue(emptyMonthly);
  reportYearly.mockResolvedValue({
    year: 2026,
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  });
  reportByCategory.mockResolvedValue({ months: [], income: [], expense: [], series: [] });
  reportNetWorthSeries.mockResolvedValue({ points: [] });
}

describe('reports store', () => {
  it('loads all four reports and passes the preset range through', async () => {
    stubAll();
    const store = createReportsStore(new Date(2026, 4, 15));

    await store.load();

    expect(reportByCategory).toHaveBeenCalledWith('2025-06', '2026-05');
    expect(reportNetWorthSeries).toHaveBeenCalledWith('2025-06', '2026-05');
    expect(reportMonthly).toHaveBeenCalledWith(2026, 5);
    expect(reportYearly).toHaveBeenCalledWith(2026);
    expect(store.netWorth).toEqual({ points: [] });
    expect(store.error).toBeNull();
    await store.dispose();
  });

  it('reloads with the new range when the preset changes', async () => {
    stubAll();
    const store = createReportsStore(new Date(2026, 4, 15));
    await store.load();
    reportByCategory.mockClear();

    await store.setPreset('last6');

    expect(reportByCategory).toHaveBeenCalledWith('2025-12', '2026-05');
    await store.dispose();
  });

  it('keeps the message when a command rejects', async () => {
    stubAll();
    reportMonthly.mockRejectedValue(new Error('invalid argument: month out of range: 13'));
    const store = createReportsStore(new Date(2026, 4, 15));

    await store.load();

    expect(store.error).toBe('invalid argument: month out of range: 13');
    await store.dispose();
  });
});
