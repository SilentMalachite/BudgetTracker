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


export type RangePreset = 'last6' | 'last12' | 'last24' | 'thisYear';

const PRESET_MONTHS: Record<Exclude<RangePreset, 'thisYear'>, number> = {
  last6: 6,
  last12: 12,
  last24: 24,
};

function yearMonthKey(year: number, month: number): string {
  return `${year}-${pad2(month)}`;
}

/**
 * プリセットを `YYYY-MM` の閉区間に開く。区間の意味づけ（何ヶ月ぶんか）は
 * ここだけが持ち、集計そのものは Rust 側が行う。
 */
export function presetRange(
  preset: RangePreset,
  today: Date = new Date(),
): { from: string; to: string } {
  const year = today.getFullYear();
  const month = today.getMonth() + 1;

  if (preset === 'thisYear') {
    return { from: yearMonthKey(year, 1), to: yearMonthKey(year, 12) };
  }

  const span = PRESET_MONTHS[preset];
  const startIndex = year * 12 + (month - 1) - (span - 1);
  return {
    from: yearMonthKey(Math.floor(startIndex / 12), (startIndex % 12) + 1),
    to: yearMonthKey(year, month),
  };
}
