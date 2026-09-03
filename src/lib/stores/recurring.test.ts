import { beforeEach, describe, expect, it, vi } from 'vitest';

const listRecurringRulesMock = vi.fn();
const setRecurringRuleActiveMock = vi.fn();
vi.mock('../api/recurring', () => ({
  listRecurringRules: (...args: unknown[]) => listRecurringRulesMock(...args),
  setRecurringRuleActive: (...args: unknown[]) => setRecurringRuleActiveMock(...args),
}));
vi.mock('../api/events', () => ({
  onDataChanged: async () => () => {},
}));

import { createRecurringStore } from './recurring.svelte';

const view = {
  rule: {
    id: 1,
    name: '家賃',
    type: 'expense',
    amount: 85_000,
    account_id: 1,
    counter_account_id: null,
    category_id: 1,
    description: '',
    frequency: 'monthly',
    day_of_month: 27,
    day_of_week: null,
    starts_on: '2026-01-27',
    ends_on: null,
    last_generated_on: null,
    active: true,
  },
  next_occurrence: '2026-02-27',
};

describe('recurring store', () => {
  beforeEach(() => {
    listRecurringRulesMock.mockReset();
    setRecurringRuleActiveMock.mockReset();
  });

  it('loads active rules by default', async () => {
    listRecurringRulesMock.mockResolvedValue([view]);

    const store = createRecurringStore();
    await store.load();

    expect(listRecurringRulesMock).toHaveBeenCalledWith(false);
    expect(store.items).toEqual([view]);
    expect(store.error).toBeNull();
  });

  it('reloads with inactive rules when asked', async () => {
    listRecurringRulesMock.mockResolvedValue([]);

    const store = createRecurringStore();
    store.setIncludeInactive(true);
    await store.load();

    expect(listRecurringRulesMock).toHaveBeenLastCalledWith(true);
  });

  it('surfaces a load failure as an error message', async () => {
    listRecurringRulesMock.mockRejectedValue(new Error('boom'));

    const store = createRecurringStore();
    await store.load();

    expect(store.error).toBe('boom');
    expect(store.items).toEqual([]);
  });

  it('reloads after toggling a rule', async () => {
    listRecurringRulesMock.mockResolvedValue([view]);
    setRecurringRuleActiveMock.mockResolvedValue(view.rule);

    const store = createRecurringStore();
    await store.load();
    listRecurringRulesMock.mockClear();
    await store.toggleActive(1, false);

    expect(setRecurringRuleActiveMock).toHaveBeenCalledWith(1, false);
    expect(listRecurringRulesMock).toHaveBeenCalled();
  });
});
