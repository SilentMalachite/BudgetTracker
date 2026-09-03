import { expect, test } from '@playwright/test';

import monthlySummaryFixture from '../fixtures/responses/monthly_summary.json' with { type: 'json' };

const seededCategories = [
  {
    id: 1,
    name: '食費',
    type: 'expense',
    color: null,
    icon: null,
    display_order: 0,
    archived_at: null,
  },
  {
    id: 2,
    name: '給与',
    type: 'income',
    color: null,
    icon: null,
    display_order: 0,
    archived_at: null,
  },
];

const seededAccounts = [
  {
    id: 1,
    name: '現金',
    kind: 'cash',
    currency: 'JPY',
    initial_balance: 0,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

// `monthly_summary` for each step of the scenario, keyed by how many transactions
// exist. Shapes are spread from the Rust-generated fixture; the numbers are this
// test's expectations (income/expense are plain sums, `net = income - expense`):
//   0 tx: income 0, expense 0, net 0
//   1 tx: expense 1,500 in 食費 -> income 0, expense 1,500, net 0 - 1,500 = -1,500
const monthlySummaryByTxCount = [
  { ...monthlySummaryFixture, income: 0, expense: 0, net: 0, by_category: [] },
  {
    ...monthlySummaryFixture,
    income: 0,
    expense: 1_500,
    net: -1_500,
    by_category: [
      {
        ...monthlySummaryFixture.by_category[0],
        category_id: 1,
        name: '食費',
        type: 'expense',
        amount: 1_500,
      },
    ],
  },
];

test('happy path: add transaction and see dashboard total update', async ({ page }) => {
  await page.addInitScript(
    ({ categories, accounts, monthlySummaryByTxCount }) => {
      type ListenerPayload = { event: string; id: number; payload: { domain: string } };
      type Transaction = {
        id: number;
        occurred_on: string;
        type: 'income' | 'expense';
        amount: number;
        account_id: number;
        category_id: number;
        description?: string;
        counter_account_id: null;
        recurring_id: null;
        created_at: string;
        updated_at: string;
      };

      const state = {
        categories: structuredClone(categories),
        accounts: structuredClone(accounts),
        transactions: [] as Transaction[],
        nextId: 100,
      };
      (window as any).__TEST_STATE__ = state;
      let nextCallbackId = 1;
      const callbacks = new Map<number, (payload: ListenerPayload) => void>();
      const listeners = new Map<string, number[]>();

      function emitDataChanged(domain: string) {
        for (const id of listeners.get('data:changed') ?? []) {
          callbacks.get(id)?.({ event: 'data:changed', id, payload: { domain } });
        }
      }

      // Static per-step responses: the mock never re-derives Rust aggregates.
      function stepResponse<T>(steps: T[], step: number): T {
        const response = steps[step];
        if (response === undefined) {
          throw new Error('no fixture for step ' + step + ' (' + steps.length + ' steps defined)');
        }
        return response;
      }

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

          switch (command) {
            case 'boot_status':
              return { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' };
            case 'app_info':
              return { schema_version: 1, db_path: '/tmp/data.db' };
            case 'list_categories':
              return state.categories;
            case 'list_accounts':
              return state.accounts;
            case 'list_balances':
              return { accounts: [], total_assets: 0 };
            case 'list_budget_statuses':
              return [];
            case 'list_top_budget_statuses':
              return [];
            case 'list_transactions': {
              const sorted = [...state.transactions].sort((a, b) => b.id - a.id);
              return {
                items: sorted.slice(args.page * args.pageSize, (args.page + 1) * args.pageSize),
                total: sorted.length,
              };
            }
            case 'monthly_summary':
              return stepResponse(monthlySummaryByTxCount, state.transactions.length);
            case 'monthly_series':
              return Array.from({ length: args.months }, (_, index) => ({
                year_month: `2026-${String(index + 1).padStart(2, '0')}`,
                income: 0,
                expense: 0,
              }));
            case 'create_transaction': {
              const transaction = {
                id: state.nextId++,
                ...args.input,
                counter_account_id: null,
                recurring_id: null,
                created_at: '2026-05-25T00:00:00Z',
                updated_at: '2026-05-25T00:00:00Z',
              };
              state.transactions.push(transaction);
              emitDataChanged('transactions');
              return transaction;
            }
            case 'get_db_path':
              return '/tmp/data.db';
            case 'get_last_backup_at':
              return null;
            default:
              return null;
          }
        },
      };
    },
    { categories: seededCategories, accounts: seededAccounts, monthlySummaryByTxCount },
  );

  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'ダッシュボード' })).toBeVisible();
  await expect(page.getByTestId('card-income')).toBeVisible();

  await page.getByTestId('nav-transactions').click();
  await expect(
    page.getByTestId('tx-table').or(page.getByText('該当する取引がありません')),
  ).toBeVisible();

  await page.getByRole('button', { name: '+ 取引を追加' }).click();
  await page.getByTestId('tx-amount').fill('1500');
  await page.getByTestId('tx-description').fill('テスト');
  await page.getByRole('dialog').getByRole('button', { name: '追加', exact: true }).click();

  await expect
    .poll(() => page.evaluate(() => (window as any).__TEST_STATE__.transactions.length))
    .toBe(1);
  await expect(page.getByText('テスト')).toBeVisible();

  await page.getByTestId('nav-dashboard').click();
  await expect(page.getByTestId('card-expense')).toContainText('1,500');
});
