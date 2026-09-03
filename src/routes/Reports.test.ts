import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../lib/api/reports', () => ({
  monthlySeries: vi.fn().mockResolvedValue([]),
  reportMonthly: vi.fn().mockResolvedValue({
    current: { income: 0, expense: 0, net: 0 },
    prev_month: { income: 0, expense: 0, net: 0 },
    prev_year: { income: 0, expense: 0, net: 0 },
    mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    top_expense: [],
    top_income: [],
  }),
  reportYearly: vi.fn().mockResolvedValue({
    year: 2026,
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  }),
  reportByCategory: vi.fn().mockResolvedValue({
    months: [],
    income: [],
    expense: [],
    series: [],
  }),
  reportNetWorthSeries: vi.fn().mockResolvedValue({ points: [] }),
}));

vi.mock('../lib/api/events', () => ({
  onDataChanged: () => Promise.reject(new Error('no tauri event bus')),
}));

vi.mock('../lib/components/Chart.svelte', async () => {
  // chart.js は jsdom に canvas コンテキストが無いので描画ごと差し替える。
  const Stub = (await import('../lib/components/__stubs__/ChartStub.svelte')).default;
  return { default: Stub };
});

const Reports = (await import('./Reports.svelte')).default;

describe('Reports', () => {
  it('opens on the monthly tab and switches to the trend tab', async () => {
    render(Reports);

    expect(screen.getByTestId('page-reports')).toBeTruthy();
    expect(screen.getByTestId('reports-tab-monthly').getAttribute('aria-selected')).toBe('true');

    screen.getByTestId('reports-tab-trend').click();
    await Promise.resolve();

    expect(screen.getByTestId('reports-tab-trend').getAttribute('aria-selected')).toBe('true');
  });

  it('shows the range presets only on the range-driven tabs', async () => {
    render(Reports);

    expect(screen.queryByTestId('reports-presets')).toBeNull();

    screen.getByTestId('reports-tab-category').click();
    await Promise.resolve();

    expect(screen.getByTestId('reports-presets')).toBeTruthy();
  });
});
