import { fireEvent, render, screen } from '@testing-library/svelte';
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

import Recurring from './Recurring.svelte';

const ORIGINAL_TZ = process.env.TZ;

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
});
