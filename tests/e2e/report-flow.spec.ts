import { expect, test } from '@playwright/test';

import { installReadyBootMock, readyBootCommands, readyBootResult } from './tauriMock';

const categoryReport = {
  months: ['2026-04', '2026-05'],
  income: [{ category_id: 2, name: '給与', type: 'income', amount: 640_000 }],
  expense: [
    { category_id: 1, name: '食費', type: 'expense', amount: 281_000 },
    { category_id: 3, name: '交通費', type: 'expense', amount: 12_000 },
  ],
  series: [
    { category_id: 2, name: '給与', type: 'income', points: [320_000, 320_000] },
    { category_id: 1, name: '食費', type: 'expense', points: [132_400, 148_600] },
    { category_id: 3, name: '交通費', type: 'expense', points: [6_000, 6_000] },
  ],
};

const monthlyReport = {
  current: { income: 320_000, expense: 148_600, net: 171_400 },
  prev_month: { income: 320_000, expense: 132_400, net: 187_600 },
  prev_year: { income: 300_000, expense: 0, net: 300_000 },
  mom: { income_diff: 0, expense_diff: 16_200, net_diff: -16_200, expense_percent: 12 },
  yoy: { income_diff: 20_000, expense_diff: 148_600, net_diff: -128_600, expense_percent: null },
  top_expense: [{ category_id: 1, name: '食費', type: 'expense', amount: 148_600 }],
  top_income: [{ category_id: 2, name: '給与', type: 'income', amount: 320_000 }],
};

test('the report tabs render and the category trend narrows on a click', async ({ page }) => {
  // アプリは Dashboard (`/`) で起動し、そこで list_balances / monthly_summary /
  // monthly_series / list_transactions / list_top_budget_statuses を叩く。
  // installReadyBootMock がそれらを空の形で返すので初期描画は落ちない。この
  // init script はその上に重ねて、このシナリオが読む値だけを差し替える。
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures: Record<string, unknown>) => {
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        if (Object.prototype.hasOwnProperty.call(fixtures, command)) return fixtures[command];
        if (typeof previous === 'function') return previous(command, args);
        return null;
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    {
      report_monthly: monthlyReport,
      report_by_category: categoryReport,
    },
  );

  await page.goto('/reports');
  await expect(page.getByTestId('page-reports')).toBeVisible();

  // 月次タブ: 割合は Rust が出した値をそのまま出す。前年同月は分母 0 なので "—"。
  await expect(page.getByTestId('compare-mom')).toContainText('12%');
  await expect(page.getByTestId('compare-yoy')).toContainText('—');
  await expect(page.getByTestId('top-expense')).toContainText('食費');

  await page.getByTestId('reports-tab-category').click();
  await expect(page.getByTestId('selected-category')).toHaveText('支出上位5カテゴリ');

  await page.getByTestId('legend-category-3').click();
  await expect(page.getByTestId('selected-category')).toHaveText('交通費');

  // もう一度押すと選択が外れて初期表示に戻る。
  await page.getByTestId('legend-category-3').click();
  await expect(page.getByTestId('selected-category')).toHaveText('支出上位5カテゴリ');

  await page.getByTestId('reports-tab-trend').click();
  await expect(page.getByTestId('chart-net-worth')).toBeHidden();
  // report_net_worth_series isn't overridden above, so it falls back to
  // installReadyBootMock's `{ points: [] }`: both trend cards (net worth and
  // net/moving-average) render their own "データがありません" EmptyState, so the
  // brief's original getByText(...) here was ambiguous under Playwright's strict
  // mode (2 matches). .first() keeps the brief's intent — the trend tab shows an
  // empty state, not a chart — without asserting which of the two identical cards.
  await expect(page.getByText('データがありません').first()).toBeVisible();
});

// --- Task 7 extra coverage (controller ruling): Task 6's final fix round changed the
// loading/empty/error split in all four tab components and added the data:changed
// subscription on the monthly bar chart, but shipped with no automated test of either.
// The two tests below close that gap at the E2E layer.

test('a failed initial load shows the error state, not a false-empty state', async ({ page }) => {
  // Before Task 6's fix (commit bae007b), each tab collapsed "still loading" and
  // "failed" into a single `report === null` branch:
  //   - Monthly/Yearly's comparison cards always rendered "読み込み中", forever, on
  //     failure (see git show bae007b -- src/routes/reports/MonthlyTab.svelte).
  //   - ByCategory/Trend instead fell through to their *empty-data* branch
  //     (`{#if expense.length === 0}` / `{#if points.length === 0}`, which reads
  //     true when report is null) and claimed "支出がありません" / "収入がありません" /
  //     "データがありません".
  // This test rejects report_monthly, which makes the whole store.load() Promise.all
  // reject, so all four tabs stay in the null/error branch. It fails against the
  // pre-fix components exactly as described above (see task-7-report.md for the
  // git-show evidence) and passes against the current ones.
  await installReadyBootMock(page);

  const failureMessage = 'report_monthly failed';
  await page.addInitScript(
    (message: string) => {
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        if (command === 'report_monthly') throw new Error(message);
        return typeof previous === 'function' ? previous(command, args) : null;
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    failureMessage,
  );

  await page.goto('/reports');
  await expect(page.getByTestId('page-reports')).toBeVisible();
  await expect(page.getByTestId('reports-error')).toContainText(failureMessage);

  // 月次タブ: 前は「読み込み中」に留まり続けた。
  const monthlyCompareCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: /月の比較$/ }) });
  await expect(monthlyCompareCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(monthlyCompareCard.getByText('読み込み中')).toHaveCount(0);

  // 年次タブも同じ構造で同じバグを持っていた。
  await page.getByTestId('reports-tab-yearly').click();
  const yearlySummaryCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: '年間サマリー' }) });
  await expect(yearlySummaryCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(yearlySummaryCard.getByText('読み込み中')).toHaveCount(0);

  // カテゴリ別タブ: 前は「支出がありません」「収入がありません」という偽の空表示だった。
  await page.getByTestId('reports-tab-category').click();
  const expenseCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: '支出の内訳' }) });
  const incomeCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: '収入の内訳' }) });
  await expect(expenseCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(expenseCard.getByText('支出がありません')).toHaveCount(0);
  await expect(incomeCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(incomeCard.getByText('収入がありません')).toHaveCount(0);

  // トレンドタブ: 前は「データがありません」という偽の空表示だった。
  await page.getByTestId('reports-tab-trend').click();
  const netWorthCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: '純資産推移' }) });
  const netCard = page
    .locator('.card')
    .filter({ has: page.getByRole('heading', { name: '月次収支と3ヶ月移動平均' }) });
  await expect(netWorthCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(netWorthCard.getByText('データがありません')).toHaveCount(0);
  await expect(netCard.getByText('読み込みに失敗しました')).toBeVisible();
  await expect(netCard.getByText('データがありません')).toHaveCount(0);
});

test('a data:changed event on the reports page refreshes the monthly bar chart', async ({ page }) => {
  // Task 6 gave Reports.svelte's standalone loadSeries() (the monthly bar chart's
  // data source) a data:changed subscription and a request-id staleness guard, same
  // shape as the reports store's, but nothing exercised the subscription. This test
  // fully replaces __TAURI_INTERNALS__ (rather than chaining through
  // installReadyBootMock) so it can implement the plugin:event bus itself, following
  // the pattern in tests/e2e/transaction-flow.spec.ts's emitDataChanged /
  // transformCallback plumbing. It fails against a build that dropped the
  // subscription (monthly_series would only ever be called once) and passes against
  // the current one (called again after the event fires).
  const defaults = Object.fromEntries(
    readyBootCommands().map((command) => [command, readyBootResult(command)]),
  );
  const seriesSteps = [
    [{ year_month: '2026-05', income: 320_000, expense: 132_400 }],
    [{ year_month: '2026-05', income: 320_000, expense: 148_600 }],
  ];

  await page.addInitScript(
    ({ defaults, seriesSteps }: { defaults: Record<string, unknown>; seriesSteps: unknown[] }) => {
      type ListenerPayload = { event: string; id: number; payload: { domain: string } };

      const state = { monthlySeriesCalls: 0 };
      (window as any).__monthlySeriesCalls = 0;
      let nextCallbackId = 1;
      const callbacks = new Map<number, (payload: ListenerPayload) => void>();
      const listeners = new Map<string, number[]>();

      (window as any).__emitDataChanged = (domain: string) => {
        for (const id of listeners.get('data:changed') ?? []) {
          callbacks.get(id)?.({ event: 'data:changed', id, payload: { domain } });
        }
      };

      (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (event: string, id: number) => {
          listeners.set(
            event,
            (listeners.get(event) ?? []).filter((listenerId) => listenerId !== id),
          );
          callbacks.delete(id);
        },
      };

      (window as any).__TAURI_INTERNALS__ = {
        transformCallback: (callback: (payload: ListenerPayload) => void) => {
          const id = nextCallbackId++;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        invoke: async (command: string, args: any) => {
          if (command === 'plugin:event|listen') {
            listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
            return args.handler;
          }
          if (command === 'plugin:event|unlisten') {
            (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(
              args.event,
              args.eventId,
            );
            return null;
          }
          if (command === 'monthly_series') {
            const step = Math.min(state.monthlySeriesCalls, seriesSteps.length - 1);
            state.monthlySeriesCalls += 1;
            (window as any).__monthlySeriesCalls = state.monthlySeriesCalls;
            return seriesSteps[step];
          }
          if (Object.prototype.hasOwnProperty.call(defaults, command)) return defaults[command];
          return null;
        },
      };
    },
    { defaults, seriesSteps },
  );

  await page.goto('/reports');
  await expect(page.getByTestId('page-reports')).toBeVisible();
  await expect(page.getByTestId('chart-reports-monthly')).toBeVisible();

  await expect
    .poll(() => page.evaluate(() => (window as any).__monthlySeriesCalls))
    .toBe(1);

  await page.evaluate(() => (window as any).__emitDataChanged('transactions'));

  await expect
    .poll(() => page.evaluate(() => (window as any).__monthlySeriesCalls))
    .toBe(2);
});
