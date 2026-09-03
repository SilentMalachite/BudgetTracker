import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  createRecurringRule,
  expandDueRecurring,
  listRecurringRules,
  previewRecurringOccurrences,
  setRecurringRuleActive,
  updateRecurringRule,
  type RecurringRuleInput,
} from './recurring';

const input: RecurringRuleInput = {
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
};

describe('recurring api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes includeInactive through to list_recurring_rules', async () => {
    invokeMock.mockResolvedValueOnce([]);

    await listRecurringRules(true);

    expect(invokeMock).toHaveBeenCalledWith('list_recurring_rules', { includeInactive: true });
  });

  it('wraps input under input for create_recurring_rule', async () => {
    invokeMock.mockResolvedValueOnce({});

    await createRecurringRule(input);

    expect(invokeMock).toHaveBeenCalledWith('create_recurring_rule', { input });
  });

  it('passes id alongside input for update_recurring_rule', async () => {
    invokeMock.mockResolvedValueOnce({});

    await updateRecurringRule(7, input);

    expect(invokeMock).toHaveBeenCalledWith('update_recurring_rule', { id: 7, input });
  });

  it('passes id and active for set_recurring_rule_active', async () => {
    invokeMock.mockResolvedValueOnce({});

    await setRecurringRuleActive(7, false);

    expect(invokeMock).toHaveBeenCalledWith('set_recurring_rule_active', { id: 7, active: false });
  });

  it('passes input and limit for preview_recurring_occurrences', async () => {
    invokeMock.mockResolvedValueOnce({ backfill: [], backfill_total: 0, truncated: false, upcoming: [] });

    await previewRecurringOccurrences(input, 100);

    expect(invokeMock).toHaveBeenCalledWith('preview_recurring_occurrences', {
      input,
      limit: 100,
      after: null,
    });
  });

  it('passes the stored watermark as after for preview_recurring_occurrences', async () => {
    invokeMock.mockResolvedValueOnce({ backfill: [], backfill_total: 0, truncated: false, upcoming: [] });

    await previewRecurringOccurrences(input, 100, '2026-04-30');

    expect(invokeMock).toHaveBeenCalledWith('preview_recurring_occurrences', {
      input,
      limit: 100,
      after: '2026-04-30',
    });
  });

  it('takes no arguments for expand_due_recurring', async () => {
    invokeMock.mockResolvedValueOnce({ generated: 0, rules: [], skipped: [] });

    await expandDueRecurring();

    expect(invokeMock).toHaveBeenCalledWith('expand_due_recurring');
  });
});
