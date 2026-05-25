import { expect, test } from '@playwright/test';

test('app shell renders with dashboard route', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('pageerror', (err) => consoleErrors.push(err.message));

  await page.goto('/');

  await expect(page.getByText('BudgetTracker')).toBeVisible();
  await expect(page.getByTestId('nav-dashboard')).toBeVisible();
  await expect(page.getByTestId('page-dashboard')).toBeVisible();
  await page.getByTestId('nav-categories').click();
  await expect(page.getByRole('heading', { name: 'カテゴリ' })).toBeVisible();

  // ページレベルの未捕捉エラーは出ていないこと
  expect(consoleErrors).toEqual([]);
});
