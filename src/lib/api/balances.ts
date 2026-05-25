import { invoke } from '@tauri-apps/api/core';
import type { AccountKind } from './accounts';

export type AccountBalance = {
  account_id: number;
  name: string;
  kind: AccountKind;
  initial_balance: number;
  balance: number;
  archived_at: string | null;
  display_order: number;
};

export function listBalances(): Promise<AccountBalance[]> {
  return invoke<AccountBalance[]>('list_balances');
}
