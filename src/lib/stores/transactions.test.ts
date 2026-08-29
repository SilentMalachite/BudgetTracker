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
});
