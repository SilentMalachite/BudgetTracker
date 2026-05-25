import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { listBudgetStatuses, setBudget } from './budgets';

describe('budgets api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes yearMonth through to list_budget_statuses', async () => {
    invokeMock.mockResolvedValueOnce([]);

    await listBudgetStatuses('2026-05');

    expect(invokeMock).toHaveBeenCalledWith('list_budget_statuses', {
      yearMonth: '2026-05',
    });
  });

  it('wraps input under input for set_budget', async () => {
    invokeMock.mockResolvedValueOnce({});

    await setBudget({
      category_id: 1,
      year_month: '2026-05',
      amount: 50_000,
      alert_threshold: 80,
    });

    expect(invokeMock).toHaveBeenCalledWith('set_budget', {
      input: expect.objectContaining({
        category_id: 1,
        amount: 50_000,
      }),
    });
  });
});
