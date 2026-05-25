import { invoke } from '@tauri-apps/api/core';

export type ImportMode = 'overwrite' | 'append';

export type ImportResult = {
  categories: number;
  accounts: number;
  transactions: number;
  budgets: number;
  warnings: string[];
};

export function exportJson(): Promise<string> {
  return invoke<string>('export_json');
}

export function importJson(payload: string, mode: ImportMode): Promise<ImportResult> {
  return invoke<ImportResult>('import_json', { args: { payload, mode } });
}
