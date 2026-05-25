import { expect, test } from '@playwright/test';

const seededAccounts = [
  {
    id: 1,
    name: '現金',
    kind: 'cash',
    currency: 'JPY',
    initial_balance: 50_000,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
  {
    id: 2,
    name: '銀行',
    kind: 'bank',
    currency: 'JPY',
    initial_balance: 200_000,
    display_order: 1,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

test('transfer moves money between accounts without changing total assets', async ({ page }) => {
  await page.addInitScript((accounts) => {
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

    const state = {
      categories: [] as any[],
      accounts: structuredClone(accounts),
      transactions: [] as Tx[],
      nextId: 100,
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
    (window as any).__TEST_EMIT_CHANGED__ = emitChanged;

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        listeners.set(event, (listeners.get(event) ?? []).filter((x) => x !== id));
        callbacks.delete(id);
      },
    };

    function balances() {
      return state.accounts.map((a: any) => {
        let bal = a.initial_balance;
        for (const t of state.transactions) {
          if (t.type === 'income' && t.account_id === a.id) bal += t.amount;
          else if (t.type === 'expense' && t.account_id === a.id) bal -= t.amount;
          else if (t.type === 'transfer' && t.account_id === a.id) bal -= t.amount;
          else if (t.type === 'transfer' && t.counter_account_id === a.id) bal += t.amount;
        }
        return {
          account_id: a.id,
          name: a.name,
          kind: a.kind,
          initial_balance: a.initial_balance,
          balance: bal,
          archived_at: a.archived_at,
          display_order: a.display_order,
        };
      });
    }

    (window as any).__TAURI_INTERNALS__ = {
      transformCallback: (cb: (p: ListenerPayload) => void) => {
        const id = nextCallbackId++;
        callbacks.set(id, cb);
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
            return { schema_version: 2, db_path: '/tmp/data.db' };
          case 'list_categories':
            return state.categories;
          case 'list_accounts':
            return args.includeArchived
              ? state.accounts
              : state.accounts.filter((account: any) => account.archived_at == null);
          case 'list_transactions': {
            const sorted = [...state.transactions].sort((a, b) => b.id - a.id);
            return {
              items: sorted.slice(args.page * args.pageSize, (args.page + 1) * args.pageSize),
              total: sorted.length,
            };
          }
          case 'list_balances':
            return balances();
          case 'monthly_summary': {
            const inc = state.transactions
              .filter((t) => t.type === 'income')
              .reduce((s, t) => s + t.amount, 0);
            const exp = state.transactions
              .filter((t) => t.type === 'expense')
              .reduce((s, t) => s + t.amount, 0);
            return { income: inc, expense: exp, net: inc - exp, by_category: [] };
          }
          case 'monthly_series':
            return Array.from({ length: args.months }, (_, i) => ({
              year_month: `2026-${String(i + 1).padStart(2, '0')}`,
              income: 0,
              expense: 0,
            }));
          case 'create_transfer': {
            const tx: Tx = {
              id: state.nextId++,
              occurred_on: args.input.occurred_on,
              type: 'transfer',
              amount: args.input.amount,
              account_id: args.input.account_id,
              counter_account_id: args.input.counter_account_id,
              category_id: null,
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
  }, seededAccounts);

  await page.goto('/');

  // Total before any transactions = 50,000 + 200,000 = 250,000
  await expect(page.getByTestId('card-total-assets')).toContainText('250,000');
  await expect(page.getByTestId('balance-1')).toContainText('50,000');
  await expect(page.getByTestId('balance-2')).toContainText('200,000');

  // Create a transfer 30,000 from 銀行 (id=2) -> 現金 (id=1).
  await page.getByTestId('nav-transactions').click();
  await page.getByRole('button', { name: '+ 取引を追加' }).click();
  await page.getByTestId('tx-type').selectOption('transfer');
  await page.getByTestId('tx-amount').fill('30000');
  await page.getByTestId('tx-account').selectOption('2');           // source = 銀行
  await page.getByTestId('tx-counter-account').selectOption('1');   // dest   = 現金
  await page.getByTestId('tx-description').fill('ATM入金');
  await page.getByRole('dialog').getByRole('button', { name: '追加', exact: true }).click();

  await expect
    .poll(() => page.evaluate(() => (window as any).__TEST_STATE__.transactions.length))
    .toBe(1);

  // Transfer row renders as `銀行 → 現金` in the table.
  await expect(page.getByTestId('tx-row-transfer')).toContainText('銀行 → 現金');

  // Back on the Dashboard: total unchanged at 250,000, per-account moved.
  await page.getByTestId('nav-dashboard').click();
  await expect(page.getByTestId('card-total-assets')).toContainText('250,000');
  await expect(page.getByTestId('balance-1')).toContainText('80,000');  // 50,000 + 30,000
  await expect(page.getByTestId('balance-2')).toContainText('170,000'); // 200,000 - 30,000
  await expect(page.getByTestId('recent-list')).toContainText('ATM入金');
  await expect(page.getByTestId('recent-list')).not.toContainText(/\+.*30,000/);

  // Historical transaction labels should keep archived account names.
  await page.getByTestId('nav-transactions').click();
  await page.evaluate(() => {
    const state = (window as any).__TEST_STATE__;
    state.accounts.find((account: any) => account.id === 2).archived_at = '2026-05-26T00:00:00Z';
    (window as any).__TEST_EMIT_CHANGED__('accounts');
  });
  await expect(page.getByTestId('tx-row-transfer')).toContainText('銀行 → 現金');
});

test('accounts page surfaces balance loading errors', async ({ page }) => {
  await page.addInitScript((accounts) => {
    type ListenerPayload = { event: string; id: number; payload: { domain: string } };
    let nextCallbackId = 1;
    const callbacks = new Map<number, (payload: ListenerPayload) => void>();
    const listeners = new Map<string, number[]>();

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        listeners.set(event, (listeners.get(event) ?? []).filter((x) => x !== id));
        callbacks.delete(id);
      },
    };

    (window as any).__TAURI_INTERNALS__ = {
      transformCallback: (cb: (p: ListenerPayload) => void) => {
        const id = nextCallbackId++;
        callbacks.set(id, cb);
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
            return { schema_version: 2, db_path: '/tmp/data.db' };
          case 'list_categories':
            return [];
          case 'list_accounts':
            return args.includeArchived
              ? accounts
              : (accounts as any[]).filter((account) => account.archived_at == null);
          case 'list_transactions':
            return { items: [], total: 0 };
          case 'list_balances':
            throw new Error('balance boom');
          case 'monthly_summary':
            return { income: 0, expense: 0, net: 0, by_category: [] };
          case 'monthly_series':
            return Array.from({ length: args.months }, (_, i) => ({
              year_month: `2026-${String(i + 1).padStart(2, '0')}`,
              income: 0,
              expense: 0,
            }));
          case 'get_db_path':
            return '/tmp/data.db';
          case 'get_last_backup_at':
            return null;
          default:
            return null;
        }
      },
    };
  }, seededAccounts);

  await page.goto('/');
  await page.getByTestId('nav-accounts').click();
  await expect(page.getByText(/残高を取得できません/)).toBeVisible();
  await expect(page.getByText(/balance boom/)).toBeVisible();
});
