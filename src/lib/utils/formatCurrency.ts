/**
 * Format an integer yen amount. Throws if the input is not an integer because
 * monetary amounts must be stored as integers throughout the app (CLAUDE.md §1).
 */
export function formatCurrency(amount: number): string {
  if (!Number.isInteger(amount)) {
    throw new Error(`formatCurrency expects an integer yen amount, got ${amount}`);
  }
  const sign = amount < 0 ? '-' : '';
  const abs = Math.abs(amount);
  return `${sign}¥${abs.toLocaleString('en-US')}`;
}
