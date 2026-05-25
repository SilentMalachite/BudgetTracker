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
