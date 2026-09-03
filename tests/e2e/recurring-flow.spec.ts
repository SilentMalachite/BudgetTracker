import { expect, test } from '@playwright/test';

import { installReadyBootMock } from './tauriMock';

const seededAccounts = [
  {
    id: 1,
    name: '現金',
    kind: 'cash',
    currency: 'JPY',
    initial_balance: 100_000,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

const seededCategories = [
  {
    id: 1,
    name: '家賃',
    type: 'expense',
    color: null,
    icon: null,
    display_order: 0,
    archived_at: null,
  },
];

const createdRule = {
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
};

test('a new rule appears in the list with its next occurrence', async ({ page }) => {
  // The app boots on Dashboard (`/`) before this test ever navigates to
  // `/recurring`, and Dashboard fans out to list_balances, monthly_summary,
  // monthly_series, list_transactions and list_top_budget_statuses on mount.
  // installReadyBootMock answers all of those with their empty-state shape so
  // the initial render never throws; this init script layers on top of it
  // (chaining through the `previous` invoke, same pattern the mock itself
  // uses) to take over just the commands this scenario drives.
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures) => {
      const state = { created: false, expansions: 0 };
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'expand_due_recurring':
            state.expansions += 1;
            (window as any).__expansions = state.expansions;
            return { generated: 3, rules: [], skipped: [] };
          case 'list_accounts':
            return fixtures.accounts;
          case 'list_categories':
            return fixtures.categories;
          case 'list_recurring_rules':
            return state.created
              ? [{ rule: fixtures.rule, next_occurrence: '2026-02-27' }]
              : [];
          case 'preview_recurring_occurrences':
            return { backfill: [], backfill_total: 4, truncated: false, upcoming: ['2026-02-27'] };
          case 'create_recurring_rule':
            state.created = true;
            return fixtures.rule;
          default:
            return typeof previous === 'function' ? previous(command, args) : null;
        }
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    { accounts: seededAccounts, categories: seededCategories, rule: createdRule },
  );

  await page.goto('/');

  // 起動時展開の結果がバナーに出る。
  await expect(page.getByTestId('recurring-expansion-banner')).toContainText('3 件');

  await page.getByTestId('nav-recurring').click();
  await expect(page.getByTestId('recurring-row')).toHaveCount(0);

  await page.getByTestId('recurring-new').click();
  await page.getByTestId('recurring-name').fill('家賃');
  await page.getByTestId('recurring-amount').fill('85000');
  await page.getByTestId('recurring-account').selectOption('1');
  await page.getByTestId('recurring-category').selectOption('1');
  await page.getByTestId('recurring-day-of-month').fill('27');
  await page.getByTestId('recurring-starts-on').fill('2026-01-27');

  // 保存前に生成件数が見える。
  await page.getByTestId('recurring-preview-button').click();
  await expect(page.getByTestId('recurring-preview')).toContainText('4 件');

  await page.getByTestId('recurring-save').click();

  await expect(page.getByTestId('recurring-row')).toHaveCount(1);
  await expect(page.getByTestId('recurring-next')).toHaveText('2026-02-27');

  // 保存が「今すぐ生成されます」を守る: 起動時の 1 回に加えて保存直後にも展開する。
  await expect
    .poll(() => page.evaluate(() => (window as any).__expansions))
    .toBe(2);
});

test('a skipped rule is flagged in the banner and on its own row', async ({ page }) => {
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures) => {
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'expand_due_recurring':
            return {
              generated: 0,
              rules: [],
              skipped: [{ rule_id: 1, rule_name: '家賃', reason: 'archived_account' }],
            };
          case 'list_accounts':
            return fixtures.accounts;
          case 'list_categories':
            return fixtures.categories;
          case 'list_recurring_rules':
            return [{ rule: fixtures.rule, next_occurrence: '2026-02-27' }];
          default:
            return typeof previous === 'function' ? previous(command, args) : null;
        }
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    { accounts: seededAccounts, categories: seededCategories, rule: createdRule },
  );

  await page.goto('/');

  await expect(page.getByTestId('recurring-skip-banner')).toContainText('1 件');
  await page.getByTestId('recurring-skip-banner').getByRole('link').click();

  // どのルールがなぜ見送られたかが、その行で分かる (spec §5.4)。
  const badge = page.getByTestId('recurring-skip-badge');
  await expect(badge).toHaveCount(1);
  await expect(badge).toContainText('口座がアーカイブ済み');
});
