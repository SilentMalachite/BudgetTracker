import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const listMock = vi.fn();
const onChangedMock = vi.fn();

vi.mock('../api/transactions', () => ({
  listTransactions: (...args: unknown[]) => listMock(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: (cb: (d: string) => void) => onChangedMock(cb),
}));

import { createTransactionsStore } from './transactions.svelte';

describe('transactions store', () => {
  beforeEach(() => {
    listMock.mockReset();
    onChangedMock.mockReset();
    onChangedMock.mockImplementation(() => Promise.resolve(() => {}));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('ignores a stale list response', async () => {
    let resolveOld: (value: unknown) => void = () => {};
    const oldPromise = new Promise((resolve) => {
      resolveOld = resolve;
    });
    listMock.mockImplementationOnce(() => oldPromise);
    listMock.mockResolvedValueOnce({
      items: [{ id: 2, occurred_on: '2026-05-02', type: 'expense', amount: 2, account_id: 1, counter_account_id: null, category_id: 1, description: 'new', recurring_id: null, created_at: '', updated_at: '' }],
      total: 1,
    });

    const store = createTransactionsStore({}, 50);
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));

    store.setPage(1);
    await vi.waitFor(() => expect(store.items[0]?.id).toBe(2));

    resolveOld({
      items: [{ id: 1, occurred_on: '2026-05-01', type: 'expense', amount: 1, account_id: 1, counter_account_id: null, category_id: 1, description: 'old', recurring_id: null, created_at: '', updated_at: '' }],
      total: 1,
    });
    await new Promise((r) => setTimeout(r, 20));
    expect(store.items[0]?.id).toBe(2);
    await store.dispose();
  });

  it('falls back to the last non-empty page when the current page becomes empty', async () => {
    const row = (id: number) => ({
      id,
      occurred_on: '2026-05-01',
      type: 'expense',
      amount: id,
      account_id: 1,
      counter_account_id: null,
      category_id: 1,
      description: `row ${id}`,
      recurring_id: null,
      created_at: '',
      updated_at: '',
    });

    // Page size 2: page 0 = rows 1,2 / page 1 = row 3. After row 3 is deleted
    // (simulated via data:changed), page 1 is empty but total is still > 0.
    let deleted = false;
    listMock.mockImplementation((_filter: unknown, page: number) => {
      if (!deleted) {
        return Promise.resolve(
          page === 0 ? { items: [row(1), row(2)], total: 3 } : { items: [row(3)], total: 3 },
        );
      }
      return Promise.resolve(
        page === 0 ? { items: [row(1), row(2)], total: 2 } : { items: [], total: 2 },
      );
    });

    const store = createTransactionsStore({}, 2);
    await vi.waitFor(() => expect(onChangedMock).toHaveBeenCalledTimes(1));
    const onChanged = onChangedMock.mock.calls[0]?.[0] as (domain: string) => void;

    store.setPage(1);
    await vi.waitFor(() => expect(store.items.map((t) => t.id)).toEqual([3]));
    expect(store.page).toBe(1);

    deleted = true;
    onChanged('transactions');
    await vi.waitFor(() => expect(store.items.map((t) => t.id)).toEqual([1, 2]));
    expect(store.page).toBe(0);
    expect(store.total).toBe(2);
    expect(store.loading).toBe(false);
    await store.dispose();
  });

  it('keeps page 0 when the list is empty and total is 0', async () => {
    listMock.mockResolvedValue({ items: [], total: 0 });

    const store = createTransactionsStore({}, 2);
    await vi.waitFor(() => expect(store.loading).toBe(false));
    expect(store.page).toBe(0);
    expect(store.items).toEqual([]);
    expect(listMock).toHaveBeenCalledTimes(1);
    await store.dispose();
  });
});
