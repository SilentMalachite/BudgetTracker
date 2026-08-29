import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { listBalances } from './balances';

describe('balances api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('invokes list_balances with no args', async () => {
    invokeMock.mockResolvedValueOnce({ accounts: [], total_assets: 0 });
    await listBalances();
    expect(invokeMock).toHaveBeenCalledWith('list_balances');
  });

  it('returns the rust payload unchanged', async () => {
    const payload = {
      accounts: [
        {
          account_id: 1,
          name: 'cash',
          kind: 'cash',
          initial_balance: 1000,
          balance: 1500,
          archived_at: null,
          display_order: 0,
        },
      ],
      total_assets: 1500,
    };
    invokeMock.mockResolvedValueOnce(payload);
    const result = await listBalances();
    expect(result).toEqual(payload);
  });
});
