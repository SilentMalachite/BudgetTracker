import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  createTransaction,
  deleteTransaction,
  listTransactions,
  updateTransaction,
} from './transactions';

describe('transactions api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes filter/page/pageSize through to invoke', async () => {
    invokeMock.mockResolvedValueOnce({ items: [], total: 0 });
    await listTransactions({ type: 'expense' }, 2, 25);
    expect(invokeMock).toHaveBeenCalledWith('list_transactions', {
      filter: { type: 'expense' },
      page: 2,
      pageSize: 25,
    });
  });

  it('wraps input under input for create', async () => {
    invokeMock.mockResolvedValueOnce({});
    await createTransaction({
      occurred_on: '2026-05-25',
      type: 'income',
      amount: 1000,
      account_id: 1,
      category_id: 2,
      description: '',
    });
    expect(invokeMock).toHaveBeenCalledWith('create_transaction', {
      input: expect.objectContaining({ amount: 1000 }),
    });
  });

  it('wraps id and patch for update', async () => {
    invokeMock.mockResolvedValueOnce({});
    await updateTransaction(42, {
      occurred_on: '2026-05-25',
      type: 'expense',
      amount: 500,
      account_id: 1,
      category_id: 2,
      description: 'x',
    });
    expect(invokeMock).toHaveBeenCalledWith('update_transaction', {
      id: 42,
      patch: expect.objectContaining({ description: 'x' }),
    });
  });

  it('passes just id for delete', async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await deleteTransaction(7);
    expect(invokeMock).toHaveBeenCalledWith('delete_transaction', { id: 7 });
  });
});

import { createTransfer, updateTransfer } from './transactions';

describe('transfer api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('wraps input under "input" for create_transfer', async () => {
    invokeMock.mockResolvedValueOnce({});
    await createTransfer({
      occurred_on: '2026-05-25',
      amount: 50_000,
      account_id: 1,
      counter_account_id: 2,
      description: 'ATM',
    });
    expect(invokeMock).toHaveBeenCalledWith('create_transfer', {
      input: expect.objectContaining({
        account_id: 1,
        counter_account_id: 2,
        amount: 50_000,
      }),
    });
  });

  it('wraps id+patch for update_transfer', async () => {
    invokeMock.mockResolvedValueOnce({});
    await updateTransfer(42, {
      occurred_on: '2026-05-25',
      amount: 10,
      account_id: 1,
      counter_account_id: 2,
      description: '',
    });
    expect(invokeMock).toHaveBeenCalledWith('update_transfer', {
      id: 42,
      patch: expect.objectContaining({ amount: 10 }),
    });
  });
});
