import { invoke } from '@tauri-apps/api/core';

export type ImportMode = 'overwrite' | 'append';

export type ImportResult = {
  categories: number;
  accounts: number;
  transactions: number;
  budgets: number;
  warnings: string[];
};

export type BackupFileResult = {
  path: string;
  last_backup_at: string;
};

/** Opens the native save dialog and writes the backup. Resolves to null when cancelled. */
export function exportBackupToFile(): Promise<BackupFileResult | null> {
  return invoke<BackupFileResult | null>('export_backup_to_file');
}

export function importJson(payload: string, mode: ImportMode): Promise<ImportResult> {
  return invoke<ImportResult>('import_json', { args: { payload, mode } });
}
