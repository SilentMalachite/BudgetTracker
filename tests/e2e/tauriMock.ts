import type { Page } from '@playwright/test';

const readyBootResults: Record<string, unknown> = {
  boot_status: { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' },
  list_balances: { accounts: [], total_assets: 0 },
  list_budget_statuses: [],
  list_top_budget_statuses: [],
  monthly_summary: { income: 0, expense: 0, net: 0, by_category: [] },
  monthly_series: [],
  list_transactions: { items: [], total: 0 },
  list_categories: [],
  list_accounts: [],
  list_recurring_rules: [],
  expand_due_recurring: { generated: 0, rules: [], skipped: [] },
  report_monthly: {
    current: { income: 0, expense: 0, net: 0 },
    prev_month: { income: 0, expense: 0, net: 0 },
    prev_year: { income: 0, expense: 0, net: 0 },
    mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    top_expense: [],
    top_income: [],
  },
  report_yearly: {
    year: new Date().getFullYear(),
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  },
  report_by_category: { months: [], income: [], expense: [], series: [] },
  report_net_worth_series: { points: [] },
};

export function readyBootResult(command: string): unknown {
  return readyBootResults[command] ?? null;
}

/** Commands the ready-boot mock answers; `tests/tauri-mock.test.ts` pins each to its fixture. */
export function readyBootCommands(): string[] {
  return Object.keys(readyBootResults);
}

export async function installReadyBootMock(page: Page): Promise<void> {
  await page.addInitScript((fixtures: Record<string, unknown>) => {
    const internals = (window as any).__TAURI_INTERNALS__ ?? {};
    const previous = internals.invoke;
    internals.invoke = async (command: string, args: any) => {
      if (Object.prototype.hasOwnProperty.call(fixtures, command)) return fixtures[command];
      if (typeof previous === 'function') return previous(command, args);
      return null;
    };
    (window as any).__TAURI_INTERNALS__ = internals;
  }, readyBootResults);
}
