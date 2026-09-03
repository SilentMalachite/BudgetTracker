function pad2(n: number): string {
  return String(n).padStart(2, '0');
}

export function isoToday(): string {
  const now = new Date();
  return `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
}

const YEAR_MONTH = /^\d{4}-(0[1-9]|1[0-2])$/;

/** True for a `YYYY-MM` string with month 01-12 (the value shape of `<input type="month">`). */
export function isYearMonth(value: string): boolean {
  return YEAR_MONTH.test(value);
}

export function monthRange(year: number, month: number): { from: string; to: string } {
  const m = pad2(month);
  const last = new Date(year, month, 0).getDate();
  return {
    from: `${year}-${m}-01`,
    to: `${year}-${m}-${pad2(last)}`,
  };
}
