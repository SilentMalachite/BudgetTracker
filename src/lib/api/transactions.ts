import { invoke } from '@tauri-apps/api/core';

export type TxType = 'income' | 'expense' | 'transfer';

export type Transaction = {
  id: number;
  occurred_on: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  recurring_id: number | null;
  created_at: string;
  updated_at: string;
};

export type ListTransactionFilter = {
  from?: string;
  to?: string;
  type?: 'income' | 'expense';
  category_id?: number;
  account_id?: number;
  search?: string;
};

export type ListTransactionResult = {
  items: Transaction[];
  total: number;
};

export function listTransactions(
  filter: ListTransactionFilter,
  page: number,
  pageSize: number,
): Promise<ListTransactionResult> {
  return invoke<ListTransactionResult>('list_transactions', {
    filter,
    page,
    pageSize,
  });
}

export type CreateTransactionInput = {
  occurred_on: string;
  type: 'income' | 'expense';
  amount: number;
  account_id: number;
  category_id: number;
  description?: string;
};

export function createTransaction(input: CreateTransactionInput): Promise<Transaction> {
  return invoke<Transaction>('create_transaction', { input });
}

export type UpdateTransactionPatch = CreateTransactionInput;

export function updateTransaction(
  id: number,
  patch: UpdateTransactionPatch,
): Promise<Transaction> {
  return invoke<Transaction>('update_transaction', { id, patch });
}

export function deleteTransaction(id: number): Promise<void> {
  return invoke('delete_transaction', { id });
}
