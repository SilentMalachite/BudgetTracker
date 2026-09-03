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

test('a backfilling save waits for an explicit confirmation', async ({ page }) => {
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
      const state = { created: 0, expansions: 0 };
      (window as any).__created = 0;
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
            return state.created > 0
              ? [{ rule: fixtures.rule, next_occurrence: '2026-02-27' }]
              : [];
          case 'preview_recurring_occurrences':
            return {
              backfill: ['2026-01-27', '2026-02-27', '2026-03-27', '2026-04-27'],
              backfill_total: 4,
              truncated: false,
              upcoming: ['2026-05-27'],
            };
          case 'create_recurring_rule':
            state.created += 1;
            (window as any).__created = state.created;
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

  // 開始日を動かすと、さっきの件数は今のフォームのものではなくなる。
  await page.getByTestId('recurring-starts-on').fill('2026-01-20');
  await expect(page.getByTestId('recurring-preview')).toHaveCount(0);
  await page.getByTestId('recurring-starts-on').fill('2026-01-27');

  // 過去にさかのぼる保存は、承諾するまで 1 行も書かない。
  await page.getByTestId('recurring-save').click();
  await expect(page.getByTestId('recurring-backfill-confirm')).toContainText('4 件');
  await expect(page.getByTestId('recurring-row')).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__created)).toBe(0);

  await page.getByTestId('recurring-backfill-confirm-button').click();

  await expect(page.getByTestId('recurring-row')).toHaveCount(1);
  await expect(page.getByTestId('recurring-next')).toHaveText('2026-02-27');
  expect(await page.evaluate(() => (window as any).__created)).toBe(1);

  // 保存が「今すぐ生成されます」を守る: 起動時の 1 回に加えて保存直後にも展開する。
  await expect
    .poll(() => page.evaluate(() => (window as any).__expansions))
    .toBe(2);
});

test('a rule with no backfill saves in one step', async ({ page }) => {
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures) => {
      const state = { created: 0 };
      (window as any).__created = 0;
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'expand_due_recurring':
            return { generated: 0, rules: [], skipped: [] };
          case 'list_accounts':
            return fixtures.accounts;
          case 'list_categories':
            return fixtures.categories;
          case 'list_recurring_rules':
            return state.created > 0
              ? [{ rule: fixtures.rule, next_occurrence: '2026-02-27' }]
              : [];
          // 今日から始まるルールなので、生成される過去分はゼロ。
          case 'preview_recurring_occurrences':
            return { backfill: [], backfill_total: 0, truncated: false, upcoming: ['2026-02-27'] };
          case 'create_recurring_rule':
            state.created += 1;
            (window as any).__created = state.created;
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
  await page.getByTestId('nav-recurring').click();

  await page.getByTestId('recurring-new').click();
  await page.getByTestId('recurring-name').fill('家賃');
  await page.getByTestId('recurring-amount').fill('85000');
  await page.getByTestId('recurring-account').selectOption('1');
  await page.getByTestId('recurring-category').selectOption('1');
  await page.getByTestId('recurring-day-of-month').fill('27');

  // 一度きりのクリックで保存が通る。何も生成されないものに確認は挟まない。
  await page.getByTestId('recurring-save').click();

  await expect(page.getByTestId('recurring-row')).toHaveCount(1);
  await expect(page.getByTestId('recurring-backfill-confirm')).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__created)).toBe(1);
});

test('a skipped rule is flagged in the banner and on its own row', async ({ page }) => {
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures) => {
      // 参照先を直すまでは見送られ続け、直したあとの展開では見送りが消える。
      const state = { repaired: false };
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'expand_due_recurring':
            return {
              generated: 0,
              rules: [],
              skipped: state.repaired
                ? []
                : [{ rule_id: 1, rule_name: '家賃', reason: 'archived_account' }],
            };
          case 'update_recurring_rule':
            state.repaired = true;
            return fixtures.rule;
          // 見送られていたルールは watermark が進んでいない。直せば見送った期間が
          // 生成されるので、保存はその件数を見せてから書く。
          case 'preview_recurring_occurrences':
            return {
              backfill: ['2026-04-27'],
              backfill_total: 1,
              truncated: false,
              upcoming: ['2026-05-27'],
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

  // 参照先を直したら、その場でバッジが消える。再起動まで「壊れている」と出し続けない。
  await page.getByRole('button', { name: '編集' }).click();
  await page.getByTestId('recurring-save').click();
  await page.getByTestId('recurring-backfill-confirm-button').click();
  await expect(badge).toHaveCount(0);

  // アプリシェルのバナーも同じ結果を読む。バッジだけ消えてバナーが古い件数を出した
  // ままだと、直したのに「1 件見送りました」と言い続けることになる。
  await expect(page.getByTestId('recurring-skip-banner')).toHaveCount(0);
});

test('a failed startup expansion is visible and retryable from the recurring screen', async ({
  page,
}) => {
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures) => {
      // 起動時の展開だけ失敗させ、2 回目 (画面からの再試行) は成功させる。
      const state = { attempts: 0 };
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'expand_due_recurring':
            state.attempts += 1;
            if (state.attempts === 1) throw new Error('database is locked');
            return { generated: 2, rules: [], skipped: [] };
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

  // 展開に失敗してもアプリは開く。ただし黙って飲み込まない。
  await expect(page.getByTestId('nav-recurring')).toBeVisible();
  const failureBanner = page.getByTestId('recurring-expansion-error-banner');
  await expect(failureBanner).toContainText('database is locked');

  await failureBanner.getByRole('link').click();

  // 再試行はユーザーが探しに来る場所 (定期取引画面) にある。
  await page.getByTestId('recurring-expansion-retry').click();

  await expect(page.getByTestId('recurring-expansion-error')).toHaveCount(0);
  await expect(failureBanner).toHaveCount(0);
  await expect(page.getByTestId('recurring-expansion-banner')).toContainText('2 件');
});
