import { invoke } from '@tauri-apps/api/core';

export type CategoryAggregate = {
  category_id: number;
  name: string;
  type: 'income' | 'expense';
  amount: number;
};

export type MonthlySummary = {
  income: number;
  expense: number;
  net: number;
  by_category: CategoryAggregate[];
};

export function monthlySummary(year: number, month: number): Promise<MonthlySummary> {
  return invoke<MonthlySummary>('monthly_summary', { year, month });
}

export type MonthlyBucket = {
  year_month: string;
  income: number;
  expense: number;
};

export function monthlySeries(months: number): Promise<MonthlyBucket[]> {
  return invoke<MonthlyBucket[]>('monthly_series', { months });
}

export type PeriodTotals = {
  income: number;
  expense: number;
  net: number;
};

export type Delta = {
  income_diff: number;
  expense_diff: number;
  net_diff: number;
  /** 比較対象の支出が 0 のときは null（Rust 側で 0 除算を避けている）。 */
  expense_percent: number | null;
};

export type MonthlyReport = {
  current: PeriodTotals;
  prev_month: PeriodTotals;
  prev_year: PeriodTotals;
  mom: Delta;
  yoy: Delta;
  top_expense: CategoryAggregate[];
  top_income: CategoryAggregate[];
};

export function reportMonthly(year: number, month: number): Promise<MonthlyReport> {
  return invoke<MonthlyReport>('report_monthly', { year, month });
}

export type YearlyReport = {
  year: number;
  months: MonthlyBucket[];
  total_income: number;
  total_expense: number;
  net: number;
  avg_income: number;
  avg_expense: number;
  max_expense_month: string | null;
};

export function reportYearly(year: number): Promise<YearlyReport> {
  return invoke<YearlyReport>('report_yearly', { year });
}

export type CategorySeries = {
  category_id: number;
  name: string;
  type: 'income' | 'expense';
  /** months と同じ長さ。取引の無い月は 0。 */
  points: number[];
};

export type CategoryReport = {
  months: string[];
  /**
   * 金額の降順、同額なら category_id の昇順（Rust の `category_totals_between` の
   * SQL `ORDER BY` をそのまま素通しする）。上位 N 件を出したいだけなら先頭から
   * slice すればよく、呼び直しは不要。
   */
  income: CategoryAggregate[];
  /** `income` と同じ順序保証。 */
  expense: CategoryAggregate[];
  series: CategorySeries[];
};

export function reportByCategory(
  fromYearMonth: string,
  toYearMonth: string,
): Promise<CategoryReport> {
  return invoke<CategoryReport>('report_by_category', {
    fromYearMonth,
    toYearMonth,
  });
}

export type NetWorthPoint = {
  year_month: string;
  net_worth: number;
  net: number;
  /** 3ヶ月移動平均。窓が埋まらない先頭2点は null。 */
  net_moving_avg: number | null;
};

export type NetWorthReport = {
  points: NetWorthPoint[];
};

export function reportNetWorthSeries(
  fromYearMonth: string,
  toYearMonth: string,
): Promise<NetWorthReport> {
  return invoke<NetWorthReport>('report_net_worth_series', {
    fromYearMonth,
    toYearMonth,
  });
}
