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

  /** 新規モーダルを開いて、保存できる最小限を埋める。 */
  async function openNewRuleForm() {
    render(Recurring);
    await fireEvent.click(screen.getByTestId('recurring-new'));
    await fireEvent.input(screen.getByTestId('recurring-name'), { target: { value: '家賃' } });
    await fireEvent.input(screen.getByTestId('recurring-amount'), { target: { value: '85000' } });
  }

  const previewOf = (total: number) => ({
    backfill: [],
    backfill_total: total,
    backfill_last: total > 0 ? '2026-04-27' : null,
    truncated: false,
    upcoming: ['2026-02-27'],
  });

  /** 数え直しを開いたままにする。解決する関数を返す。 */
  function holdTheRecount() {
    let release!: (value: unknown) => void;
    previewRecurringOccurrencesMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );
    return (value: unknown) => release(value);
  }

  it('drops a fetched preview as soon as a schedule field changes', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(4));
    await openNewRuleForm();

    await fireEvent.click(screen.getByTestId('recurring-preview-button'));
    const shown = await screen.findByTestId('recurring-preview');
    expect(shown.textContent).toContain('4 件');

    // 開始日を動かした瞬間、さっきの件数は今のフォームの件数ではなくなる。
    await fireEvent.input(screen.getByTestId('recurring-starts-on'), {
      target: { value: '2020-01-01' },
    });

    await waitFor(() => {
      expect(screen.queryByTestId('recurring-preview')).toBeNull();
    });
  });

  it('recounts at save and writes nothing until the backfill is confirmed', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(4));
    await openNewRuleForm();

    await fireEvent.click(screen.getByTestId('recurring-save'));

    const confirmBlock = await screen.findByTestId('recurring-backfill-confirm');
    expect(confirmBlock.textContent).toContain('4 件');
    expect(createRecurringRuleMock).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByTestId('recurring-backfill-confirm-button'));

    await waitFor(() => {
      expect(createRecurringRuleMock).toHaveBeenCalledTimes(1);
    });
  });

  it('saves in one step when nothing would be backfilled', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(0));
    await openNewRuleForm();

    await fireEvent.click(screen.getByTestId('recurring-save'));

    await waitFor(() => {
      expect(createRecurringRuleMock).toHaveBeenCalledTimes(1);
    });
    expect(screen.queryByTestId('recurring-backfill-confirm')).toBeNull();
  });

  it('never writes an input that was not the one counted and confirmed', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(4));
    await openNewRuleForm();

    // 2026-01-27 の 4 件を承諾する。
    await fireEvent.click(screen.getByTestId('recurring-save'));
    await screen.findByTestId('recurring-backfill-confirm');

    // 承諾後の数え直しを IPC 往復の途中で止め、その隙にフォームを 10 年戻す。
    const release = holdTheRecount();
    void fireEvent.click(screen.getByTestId('recurring-backfill-confirm-button'));
    await fireEvent.input(screen.getByTestId('recurring-starts-on'), {
      target: { value: '2016-01-01' },
    });
    release(previewOf(4));
    await waitFor(() => {
      expect(previewRecurringOccurrencesMock).toHaveBeenCalledTimes(2);
    });
    await new Promise((resolve) => setTimeout(resolve, 0));

    // 承諾したのは 2026-01-27 の 4 件。2016-01-01 のルールは書かせない。
    expect(createRecurringRuleMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ starts_on: '2016-01-01' }),
    );
    expect(createRecurringRuleMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('page-error')).toBeTruthy();
  });

  it('says so instead of doing nothing when the form moves during the count', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(4));
    await openNewRuleForm();

    const release = holdTheRecount();
    void fireEvent.click(screen.getByTestId('recurring-save'));
    await fireEvent.input(screen.getByTestId('recurring-starts-on'), {
      target: { value: '2016-01-01' },
    });
    release(previewOf(4));

    // 古い鍵で確認待ちにすると確認ブロックは出ず、押しても何も起きない画面になる。
    await waitFor(() => {
      expect(screen.getByTestId('page-error')).toBeTruthy();
    });
    expect(screen.queryByTestId('recurring-backfill-confirm')).toBeNull();
    expect(createRecurringRuleMock).not.toHaveBeenCalled();
  });

  it('does not name a last date it cannot know when the list is truncated', async () => {
    previewRecurringOccurrencesMock.mockResolvedValue({
      // limit で切られた一覧。末尾は 100 件目であって最後の発生日ではない。
      backfill: ['2016-01-01', '2024-04-01'],
      backfill_total: 129,
      backfill_last: '2026-09-01',
      truncated: true,
      upcoming: ['2026-10-01'],
    });
    await openNewRuleForm();

    await fireEvent.click(screen.getByTestId('recurring-save'));

    const confirmBlock = await screen.findByTestId('recurring-backfill-confirm');
    expect(confirmBlock.textContent).toContain('129 件');
    expect(confirmBlock.textContent).toContain('2026-09-01');
    expect(confirmBlock.textContent).not.toContain('2024-04-01');
  });

  it('counts an edited rule from its stored watermark, not from starts_on', async () => {
    listRecurringRulesMock.mockResolvedValue([
      { ...view, rule: { ...view.rule, last_generated_on: '2026-04-30' } },
    ]);
    previewRecurringOccurrencesMock.mockResolvedValue(previewOf(0));

    render(Recurring);
    await fireEvent.click(await screen.findByText('編集'));
    await fireEvent.click(screen.getByTestId('recurring-preview-button'));

    await waitFor(() => {
      expect(previewRecurringOccurrencesMock).toHaveBeenCalledWith(
        expect.objectContaining({ starts_on: '2026-01-27' }),
        100,
        '2026-04-30',
      );
    });
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
