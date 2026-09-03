import { invoke } from '@tauri-apps/api/core';

import type { TxType } from './transactions';

export type Frequency = 'monthly' | 'weekly' | 'yearly';

export type SkipReason =
  | 'archived_account'
  | 'archived_counter_account'
  | 'archived_category'
  | 'category_type_mismatch'
  | 'malformed_rule';

export type RecurringRule = {
  id: number;
  name: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  frequency: Frequency;
  day_of_month: number | null;
  day_of_week: number | null;
  starts_on: string;
  ends_on: string | null;
  last_generated_on: string | null;
  active: boolean;
};

/** 一覧の 1 行。`next_occurrence` は表示専用で、取引は生成されていない。 */
export type RecurringRuleView = {
  rule: RecurringRule;
  next_occurrence: string | null;
};

export type RecurringRuleInput = {
  name: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  frequency: Frequency;
  day_of_month: number | null;
  day_of_week: number | null;
  starts_on: string;
  ends_on: string | null;
};

/** 保存した瞬間に何件生成されるかを事前に見せるためのもの。 */
export type OccurrencePreview = {
  backfill: string[];
  backfill_total: number;
  /**
   * backfill 全体の最後の発生日。`backfill` は limit で切られるので、その末尾は
   * 「limit 件目」でしかない。生成がどこまで届くかを名乗れるのはこちらだけ。
   */
  backfill_last: string | null;
  truncated: boolean;
  upcoming: string[];
};

export type RuleExpansion = {
  rule_id: number;
  rule_name: string;
  generated: number;
  last_generated_on: string;
};

export type SkippedRule = {
  rule_id: number;
  rule_name: string;
  reason: SkipReason;
};

export type ExpansionResult = {
  generated: number;
  rules: RuleExpansion[];
  skipped: SkippedRule[];
};

export function listRecurringRules(includeInactive: boolean): Promise<RecurringRuleView[]> {
  return invoke<RecurringRuleView[]>('list_recurring_rules', { includeInactive });
}

export function createRecurringRule(input: RecurringRuleInput): Promise<RecurringRule> {
  return invoke<RecurringRule>('create_recurring_rule', { input });
}

export function updateRecurringRule(
  id: number,
  input: RecurringRuleInput,
): Promise<RecurringRule> {
  return invoke<RecurringRule>('update_recurring_rule', { id, input });
}

export function setRecurringRuleActive(id: number, active: boolean): Promise<RecurringRule> {
  return invoke<RecurringRule>('set_recurring_rule_active', { id, active });
}

/**
 * `after` は編集中のルールの `last_generated_on`。展開が使う窓の左端 (排他) と
 * 同じものを渡すので、プレビューの件数と実際に生成される件数は一致する。
 * 新規作成は `null` (= `starts_on` から全部数える)。
 */
export function previewRecurringOccurrences(
  input: RecurringRuleInput,
  limit: number,
  after: string | null = null,
): Promise<OccurrencePreview> {
  return invoke<OccurrencePreview>('preview_recurring_occurrences', { input, limit, after });
}

export function expandDueRecurring(): Promise<ExpansionResult> {
  return invoke<ExpansionResult>('expand_due_recurring');
}
