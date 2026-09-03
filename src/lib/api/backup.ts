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

export type PreImportSnapshot = {
  file_name: string;
  /** RFC3339 UTC, parsed from the file name's stamp. */
  created_at: string;
  size_bytes: number;
};

/** Safety copies taken right before overwrite imports, newest first. */
export function listPreImportSnapshots(): Promise<PreImportSnapshot[]> {
  return invoke<PreImportSnapshot[]>('list_pre_import_snapshots');
}

/**
 * Replace the live database with a safety copy. The current file is kept as
 * `data.db.replaced-<stamp>`; stores reload through `data:changed` events.
 */
export function restorePreImportSnapshot(fileName: string): Promise<void> {
  return invoke<void>('restore_pre_import_snapshot', { fileName });
}
