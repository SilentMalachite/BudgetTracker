import { expect, test } from '@playwright/test';

test('app shell renders and attempts to call backend', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('pageerror', (err) => consoleErrors.push(err.message));

  await page.goto('/');

  // ヘッダーが描画される
  await expect(page.getByRole('heading', { name: 'BudgetTracker' })).toBeVisible();

  // ブラウザでは Tauri invoke が無いため、エラーパスか loading パスのいずれかが表示される
  // (Tauri ウィンドウ内なら schema-version が出る)
  await expect(
    page.getByTestId('error').or(page.getByTestId('schema-version'))
  ).toBeVisible({ timeout: 10_000 });

  // ページレベルの未捕捉エラーは出ていないこと
  expect(consoleErrors).toEqual([]);
});
