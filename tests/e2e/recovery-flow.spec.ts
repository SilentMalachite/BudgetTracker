import { expect, test } from '@playwright/test';

test('recovery screen is shown when boot_status is recovery', async ({ page }) => {
  await page.addInitScript(() => {
    const internals = (window as any).__TAURI_INTERNALS__ ?? {};
    internals.invoke = async (command: string) => {
      if (command === 'boot_status') {
        return {
          state: 'recovery',
          recovery_reason: 'decrypt_failed',
          db_path: '/tmp/data.db',
        };
      }
      if (command === 'recover_import_json') {
        return {
          categories: 0,
          accounts: 1,
          recurring_rules: 0,
          transactions: 0,
          budgets: 0,
          warnings: [],
        };
      }
      if (command === 'recover_start_empty') {
        return null;
      }
      return null;
    };
    (window as any).__TAURI_INTERNALS__ = internals;
  });

  await page.goto('/');

  await expect(page.getByRole('heading', { name: 'データベースを開けません' })).toBeVisible();
  await expect(page.getByTestId('page-recovery')).toBeVisible();
  await expect(page.getByTestId('recovery-import-file')).toBeVisible();
  await expect(page.getByRole('button', { name: '空の家計簿で始める' })).toBeVisible();
  await expect(page.getByTestId('recovery-db-path')).toHaveText('/tmp/data.db');
  await expect(page.getByTestId('nav-dashboard')).toHaveCount(0);
});
