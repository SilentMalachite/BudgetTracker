import { invoke } from '@tauri-apps/api/core';

export type Budget = {
  id: number;
  category_id: number;
  period: 'monthly' | 'yearly';
  amount: number;
  starts_on: string;
  ends_on: string | null;
  alert_threshold: number;
};

export type BudgetStatus = {
  category_id: number;
  category_name: string;
  category_color: string | null;
  category_icon: string | null;
  budget_id: number | null;
  budgeted: number;
  spent: number;
  percent: number;
  progress_percent: number;
  days_left: number;
  projected: number;
  alert_threshold: number;
  threshold_reached: boolean;
  projected_over_budget: boolean;
};

export type SetBudgetInput = {
  category_id: number;
  year_month: string;
  amount: number;
  alert_threshold: number;
};

export function listBudgetStatuses(yearMonth: string): Promise<BudgetStatus[]> {
  return invoke<BudgetStatus[]>('list_budget_statuses', { yearMonth });
}

export function listTopBudgetStatuses(yearMonth: string, limit: number): Promise<BudgetStatus[]> {
  return invoke<BudgetStatus[]>('list_top_budget_statuses', { yearMonth, limit });
}

export function setBudget(input: SetBudgetInput): Promise<Budget> {
  return invoke<Budget>('set_budget', { input });
}
