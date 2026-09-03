import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

// chart.js は jsdom に canvas コンテキストが無いので描画ごと差し替える。
vi.mock('../../lib/components/Chart.svelte', async () => {
  const Stub = (await import('../../lib/components/__stubs__/ChartStub.svelte')).default;
  return { default: Stub };
});

const YearlyTab = (await import('./YearlyTab.svelte')).default;

describe('YearlyTab year input', () => {
  it('rejects an out-of-range year: no onYearChange call, a visible message, and the box snaps back', async () => {
    const onYearChange = vi.fn();
    render(YearlyTab, { props: { report: null, year: 2026, onYearChange, error: null } });

    const input = screen.getByTestId('report-year') as HTMLInputElement;
    expect(input.value).toBe('2026');

    await fireEvent.input(input, { target: { value: '99999' } });
    await fireEvent.change(input, { target: { value: '99999' } });

    // Before this fix: the rejected value stayed in the box forever (value was bound
    // to `report?.year`, which never changes on rejection) with no indication anything
    // was wrong, while the data below kept describing whatever year was last accepted.
    expect(onYearChange).not.toHaveBeenCalled();
    expect(screen.getByTestId('report-year-error')).toBeTruthy();
    expect(input.value).toBe('2026');
  });

  it('accepts a valid year, calls onYearChange once, and clears a previous rejection message', async () => {
    const onYearChange = vi.fn();
    render(YearlyTab, { props: { report: null, year: 2026, onYearChange, error: null } });

    const input = screen.getByTestId('report-year') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: '99999' } });
    await fireEvent.change(input, { target: { value: '99999' } });
    expect(screen.getByTestId('report-year-error')).toBeTruthy();

    await fireEvent.input(input, { target: { value: '2027' } });
    await fireEvent.change(input, { target: { value: '2027' } });

    expect(onYearChange).toHaveBeenCalledTimes(1);
    expect(onYearChange).toHaveBeenCalledWith(2027);
    expect(screen.queryByTestId('report-year-error')).toBeNull();
  });

  it('binds the input to the store-owned `year` prop, not to `report?.year`', () => {
    // FIX 2/4: the box must show the store's single source of truth even when the
    // report describing a *different* year is what's currently loaded (e.g. mid-flight
    // between setYear and the new report arriving).
    render(YearlyTab, {
      props: {
        report: {
          year: 2020,
          months: [],
          total_income: 0,
          total_expense: 0,
          net: 0,
          avg_income: 0,
          avg_expense: 0,
          max_expense_month: null,
        },
        year: 2026,
        onYearChange: vi.fn(),
        error: null,
      },
    });

    expect((screen.getByTestId('report-year') as HTMLInputElement).value).toBe('2026');
  });
});
