import type { Page } from '@playwright/test';

export async function installReadyBootMock(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const internals = (window as any).__TAURI_INTERNALS__ ?? {};
    const previous = internals.invoke;
    internals.invoke = async (command: string, args: any) => {
      if (command === 'boot_status') {
        return { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' };
      }
      // Dashboard mounts on `/` and `/` is the smoke landing page.
      if (command === 'list_balances') {
        return [];
      }
      if (command === 'list_budget_statuses' || command === 'list_top_budget_statuses') {
        return [];
      }
      if (command === 'monthly_summary') {
        return { income: 0, expense: 0, net: 0, by_category: [] };
      }
      if (command === 'monthly_series') {
        return [];
      }
      if (command === 'list_transactions') {
        return { items: [], total: 0 };
      }
      if (command === 'list_categories' || command === 'list_accounts') {
        return [];
      }
      if (typeof previous === 'function') return previous(command, args);
      return null;
    };
    (window as any).__TAURI_INTERNALS__ = internals;
  });
}
