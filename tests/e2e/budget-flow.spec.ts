import { expect, test } from '@playwright/test';

const seededCategories = [
  {
    id: 1,
    name: '食費',
    type: 'expense',
    color: '#FFAA00',
    icon: '🍱',
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
    initial_balance: 100_000,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

test('budget warning appears on budgets page and dashboard', async ({ page }) => {
  await page.addInitScript(
    ({ categories, accounts }) => {
      type ListenerPayload = { event: string; id: number; payload: { domain: string } };
      type Tx = {
        id: number;
        occurred_on: string;
        type: 'income' | 'expense' | 'transfer';
        amount: number;
        account_id: number;
        counter_account_id: number | null;
        category_id: number | null;
        description: string;
        recurring_id: null;
        created_at: string;
        updated_at: string;
      };
      type Budget = {
        id: number;
        category_id: number;
        period: 'monthly';
        amount: number;
        starts_on: string;
        ends_on: null;
        alert_threshold: number;
      };

      const state = {
        categories: structuredClone(categories),
        accounts: structuredClone(accounts),
        transactions: [] as Tx[],
        budgets: [] as Budget[],
        nextTransactionId: 100,
        nextBudgetId: 200,
      };
      (window as any).__TEST_STATE__ = state;

      let nextCallbackId = 1;
      const callbacks = new Map<number, (payload: ListenerPayload) => void>();
      const listeners = new Map<string, number[]>();

      function emitChanged(domain: string) {
        for (const id of listeners.get('data:changed') ?? []) {
          callbacks.get(id)?.({ event: 'data:changed', id, payload: { domain } });
        }
      }

      function budgetStatuses(yearMonth: string) {
        return state.categories
          .filter((category: any) => category.type === 'expense' && category.archived_at == null)
          .map((category: any) => {
            const startsOn = `${yearMonth}-01`;
            const budget = state.budgets.find(
              (item) => item.category_id === category.id && item.starts_on === startsOn,
            );
            const spent = state.transactions
              .filter(
                (tx) =>
                  tx.type === 'expense' &&
                  tx.category_id === category.id &&
                  tx.occurred_on.startsWith(yearMonth),
              )
              .reduce((sum, tx) => sum + tx.amount, 0);
            const budgeted = budget?.amount ?? 0;
            const percent = budgeted > 0 ? Math.floor((spent * 100) / budgeted) : spent > 0 ? 200 : 0;
            const progress = Math.min(percent, 100);
            const threshold = budget?.alert_threshold ?? 80;
            return {
              category_id: category.id,
              category_name: category.name,
              category_color: category.color,
              category_icon: category.icon,
              budget_id: budget?.id ?? null,
              budgeted,
              spent,
              percent,
              progress_percent: progress,
              days_left: 0,
              projected: spent,
              alert_threshold: threshold,
              threshold_reached: budgeted > 0 && percent >= threshold,
              projected_over_budget: false,
            };
          });
      }

      function balances() {
        return state.accounts.map((account: any) => {
          const balance = state.transactions.reduce((sum, tx) => {
            if (tx.type === 'income' && tx.account_id === account.id) return sum + tx.amount;
            if (tx.type === 'expense' && tx.account_id === account.id) return sum - tx.amount;
            return sum;
          }, account.initial_balance);
          return {
            account_id: account.id,
            name: account.name,
            kind: account.kind,
            initial_balance: account.initial_balance,
            balance,
            archived_at: account.archived_at,
            display_order: account.display_order,
          };
        });
      }

      (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (event: string, id: number) => {
          listeners.set(event, (listeners.get(event) ?? []).filter((listenerId) => listenerId !== id));
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
            case 'app_info':
              return { schema_version: 3, db_path: '/tmp/data.db' };
            case 'list_categories':
              return args.filter?.include_archived
                ? state.categories
                : state.categories.filter((category: any) => category.archived_at == null);
            case 'list_accounts':
              return args.includeArchived
                ? state.accounts
                : state.accounts.filter((account: any) => account.archived_at == null);
            case 'list_balances':
              return balances();
            case 'list_transactions': {
              const sorted = [...state.transactions].sort((a, b) => b.id - a.id);
              return {
                items: sorted.slice(args.page * args.pageSize, (args.page + 1) * args.pageSize),
                total: sorted.length,
              };
            }
            case 'list_budget_statuses':
              return budgetStatuses(args.yearMonth);
            case 'set_budget': {
              const startsOn = `${args.input.year_month}-01`;
              let budget = state.budgets.find(
                (item) =>
                  item.category_id === args.input.category_id && item.starts_on === startsOn,
              );
              if (!budget) {
                budget = {
                  id: state.nextBudgetId++,
                  category_id: args.input.category_id,
                  period: 'monthly',
                  amount: args.input.amount,
                  starts_on: startsOn,
                  ends_on: null,
                  alert_threshold: args.input.alert_threshold,
                };
                state.budgets.push(budget);
              } else {
                budget.amount = args.input.amount;
                budget.alert_threshold = args.input.alert_threshold;
              }
              emitChanged('budgets');
              return budget;
            }
            case 'monthly_summary': {
              const income = state.transactions
                .filter((tx) => tx.type === 'income')
                .reduce((sum, tx) => sum + tx.amount, 0);
              const expense = state.transactions
                .filter((tx) => tx.type === 'expense')
                .reduce((sum, tx) => sum + tx.amount, 0);
              return { income, expense, net: income - expense, by_category: [] };
            }
            case 'monthly_series':
              return Array.from({ length: args.months }, (_, index) => ({
                year_month: `2026-${String(index + 1).padStart(2, '0')}`,
                income: 0,
                expense: 0,
              }));
            case 'create_transaction': {
              const tx: Tx = {
                id: state.nextTransactionId++,
                occurred_on: args.input.occurred_on,
                type: args.input.type,
                amount: args.input.amount,
                account_id: args.input.account_id,
                counter_account_id: null,
                category_id: args.input.category_id,
                description: args.input.description ?? '',
                recurring_id: null,
                created_at: '2026-05-25T00:00:00Z',
                updated_at: '2026-05-25T00:00:00Z',
              };
              state.transactions.push(tx);
              emitChanged('transactions');
              return tx;
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
    { categories: seededCategories, accounts: seededAccounts },
  );

  await page.goto('/');
  await page.getByTestId('nav-budgets').click();
  await expect(page.getByTestId('page-budgets')).toBeVisible();
  await page.getByTestId('budget-month').fill('2026-05');

  await page.getByTestId('budget-edit-1').click();
  await page.getByTestId('budget-amount').fill('50000');
  await page.getByTestId('budget-threshold').fill('80');
  await page.getByRole('dialog').getByRole('button', { name: '保存', exact: true }).click();

  await expect(page.getByTestId('budget-row-1')).toContainText('¥50,000');

  await page.getByTestId('nav-transactions').click();
  await page.getByRole('button', { name: '+ 取引を追加' }).click();
  await page.getByTestId('tx-amount').fill('45000');
  await page.getByTestId('tx-description').fill('食料品');
  await page.getByRole('dialog').getByRole('button', { name: '追加', exact: true }).click();

  await page.getByTestId('nav-budgets').click();
  await expect(page.getByTestId('budget-row-1')).toContainText('90%');
  await expect(page.getByTestId('budget-alert-1')).toBeVisible();

  await page.getByTestId('nav-dashboard').click();
  await expect(page.getByTestId('dashboard-budget-widget')).toContainText('食費');
  await expect(page.getByTestId('dashboard-budget-widget')).toContainText('90%');
});
