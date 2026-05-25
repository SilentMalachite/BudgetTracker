import { invoke } from '@tauri-apps/api/core';

export function getLastBackupAt(): Promise<string | null> {
  return invoke<string | null>('get_last_backup_at');
}

export function setLastBackupAt(iso: string): Promise<void> {
  return invoke('set_last_backup_at', { iso });
}

export function getDbPath(): Promise<string> {
  return invoke<string>('get_db_path');
}
