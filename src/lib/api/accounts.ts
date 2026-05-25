import { invoke } from '@tauri-apps/api/core';

export type AccountKind = 'cash' | 'bank' | 'credit_card' | 'e_money' | 'investment';

export type Account = {
  id: number;
  name: string;
  kind: AccountKind;
  currency: string;
  initial_balance: number;
  display_order: number;
  note: string;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
};

export function listAccounts(include_archived = false): Promise<Account[]> {
  return invoke<Account[]>('list_accounts', { includeArchived: include_archived });
}

export type CreateAccountInput = {
  name: string;
  kind: AccountKind;
  initial_balance: number;
  note?: string;
};

export function createAccount(input: CreateAccountInput): Promise<Account> {
  return invoke<Account>('create_account', { input });
}

export type UpdateAccountPatch = {
  name?: string;
  kind?: AccountKind;
  initial_balance?: number;
  note?: string;
  display_order?: number;
};

export function updateAccount(id: number, patch: UpdateAccountPatch): Promise<Account> {
  return invoke<Account>('update_account', { id, patch });
}

export function archiveAccount(id: number): Promise<void> {
  return invoke('archive_account', { id });
}

export function unarchiveAccount(id: number): Promise<void> {
  return invoke('unarchive_account', { id });
}

export const ACCOUNT_KIND_LABELS: Record<AccountKind, string> = {
  cash: '現金',
  bank: '銀行',
  credit_card: 'クレジットカード',
  e_money: '電子マネー',
  investment: '投資',
};

export const ACCOUNT_KIND_ICONS: Record<AccountKind, string> = {
  cash: '💵',
  bank: '🏦',
  credit_card: '💳',
  e_money: '📱',
  investment: '📈',
};
