import { invoke } from '@tauri-apps/api/core';

export function getLastBackupAt(): Promise<string | null> {
  return invoke<string | null>('get_last_backup_at');
}

export function getDbPath(): Promise<string> {
  return invoke<string>('get_db_path');
}
