import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

const listRecurringRulesMock = vi.fn();
const setRecurringRuleActiveMock = vi.fn();
const createRecurringRuleMock = vi.fn();
const updateRecurringRuleMock = vi.fn();
const previewRecurringOccurrencesMock = vi.fn();
const expandDueRecurringMock = vi.fn();

vi.mock('../lib/api/recurring', () => ({
  listRecurringRules: (...args: unknown[]) => listRecurringRulesMock(...args),
  setRecurringRuleActive: (...args: unknown[]) => setRecurringRuleActiveMock(...args),
  createRecurringRule: (...args: unknown[]) => createRecurringRuleMock(...args),
  updateRecurringRule: (...args: unknown[]) => updateRecurringRuleMock(...args),
  previewRecurringOccurrences: (...args: unknown[]) => previewRecurringOccurrencesMock(...args),
  expandDueRecurring: (...args: unknown[]) => expandDueRecurringMock(...args),
}));
vi.mock('../lib/api/accounts', () => ({
  listAccounts: async () => [],
}));
vi.mock('../lib/api/categories', () => ({
  listCategories: async () => [],
}));
vi.mock('../lib/api/events', () => ({
  onDataChanged: async () => () => {},
}));

import { recurringExpansion } from '../lib/stores/recurringExpansion.svelte';
import Recurring from './Recurring.svelte';

const ORIGINAL_TZ = process.env.TZ;

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

describe('Recurring', () => {
  beforeAll(() => {
    // 正のオフセットを持つタイムゾーンでしか再現しないバグを固定する。
    // process.env.TZ への代入で Node が tzset を呼び、V8 のキャッシュも落ちる。
    process.env.TZ = 'Asia/Tokyo';
  });

  afterAll(() => {
    process.env.TZ = ORIGINAL_TZ;
  });

  beforeEach(() => {
    listRecurringRulesMock.mockReset().mockResolvedValue([]);
    setRecurringRuleActiveMock.mockReset();
    createRecurringRuleMock.mockReset();
    updateRecurringRuleMock.mockReset();
    previewRecurringOccurrencesMock.mockReset();
    expandDueRecurringMock.mockReset().mockResolvedValue({
      generated: 0,
      rules: [],
      skipped: [],
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('seeds a new rule with the local start date, not the UTC one', async () => {
    // UTC では 2026-09-03、JST では 2026-09-04。UTC 日付を使うと 1 日ずれる。
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date('2026-09-03T15:30:00Z'));

    render(Recurring);
    await fireEvent.click(screen.getByTestId('recurring-new'));

    const startsOn = screen.getByTestId('recurring-starts-on') as HTMLInputElement;
    expect(startsOn.value).toBe('2026-09-04');
  });

  it('badges a skipped rule from the shared expansion result', async () => {
    listRecurringRulesMock.mockResolvedValue([view]);
    expandDueRecurringMock.mockResolvedValueOnce({
      generated: 0,
      rules: [],
      skipped: [{ rule_id: 1, rule_name: '家賃', reason: 'archived_account' }],
    });
    // 起動時展開に相当。App が書き込んだ結果を、この画面がそのまま読む。
    await recurringExpansion.run();

    render(Recurring);

    const badge = await screen.findByTestId('recurring-skip-badge');
    expect(badge.textContent).toContain('口座がアーカイブ済み');
  });

  it('surfaces a failed expansion and retries it from this screen', async () => {
    expandDueRecurringMock.mockRejectedValueOnce(new Error('database is locked'));
    // 起動時展開が失敗した状態。App は開いたまま、失敗はストアに残る。
    await recurringExpansion.run();

    render(Recurring);

    const banner = screen.getByTestId('recurring-expansion-error');
    expect(banner.textContent).toContain('database is locked');

    await fireEvent.click(screen.getByTestId('recurring-expansion-retry'));

    await waitFor(() => {
      expect(screen.queryByTestId('recurring-expansion-error')).toBeNull();
    });
    expect(expandDueRecurringMock).toHaveBeenCalledTimes(2);
  });
});
