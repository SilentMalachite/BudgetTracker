import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const listMock = vi.fn();
const onChangedMock = vi.fn();

vi.mock('../api/budgets', () => ({
  listBudgetStatuses: (...args: unknown[]) => listMock(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: (cb: (d: string) => void) => onChangedMock(cb),
}));

import { createBudgetsStore } from './budgets.svelte';

function status(categoryId: number, name: string) {
  return {
    category_id: categoryId,
    category_name: name,
    category_color: null,
    category_icon: null,
    budget_id: null,
    budgeted: 0,
    spent: 0,
    percent: 0,
    progress_percent: 0,
    days_left: 0,
    projected: 0,
    alert_threshold: 80,
    threshold_reached: false,
    projected_over_budget: false,
  };
}

describe('budgets store', () => {
  beforeEach(() => {
    listMock.mockReset();
    onChangedMock.mockReset();
    onChangedMock.mockImplementation(() => Promise.resolve(() => {}));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads selected month on construction', async () => {
    listMock.mockResolvedValueOnce([status(1, '食費')]);

    const store = createBudgetsStore('2026-05');

    await vi.waitFor(() => expect(store.items).toHaveLength(1));
    expect(listMock).toHaveBeenCalledWith('2026-05');
    expect(store.yearMonth).toBe('2026-05');
    await store.dispose();
  });

  it('reloads when selected month changes', async () => {
    listMock.mockResolvedValueOnce([]);
    listMock.mockResolvedValueOnce([status(2, '交通費')]);

    const store = createBudgetsStore('2026-05');
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));

    store.setYearMonth('2026-06');

    await vi.waitFor(() => expect(store.items[0]?.category_name).toBe('交通費'));
    expect(listMock).toHaveBeenLastCalledWith('2026-06');
    expect(store.yearMonth).toBe('2026-06');
    await store.dispose();
  });

  it('reloads on budgets categories and transactions changes', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);
    const store = createBudgetsStore('2026-05');
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(onChangedMock).toHaveBeenCalledTimes(1));

    trigger?.('budgets');
    trigger?.('categories');
    trigger?.('transactions');

    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(4));
    await store.dispose();
  });

  it('ignores unrelated domain changes', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);
    const store = createBudgetsStore('2026-05');
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(onChangedMock).toHaveBeenCalledTimes(1));

    trigger?.('accounts');
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(listMock).toHaveBeenCalledTimes(1);
    await store.dispose();
  });
});
