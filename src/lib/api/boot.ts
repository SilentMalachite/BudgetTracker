import { invoke } from '@tauri-apps/api/core';
import type { ImportResult } from './backup';

export type RecoveryReason =
  | 'key_missing'
  | 'decrypt_failed'
  | 'key_corrupt'
  | 'keychain_error';

export type BootStatus = {
  state: 'ready' | 'recovery';
  recovery_reason: RecoveryReason | null;
  db_path: string;
};

export function bootStatus(): Promise<BootStatus> {
  return invoke('boot_status');
}

export function recoverImportJson(payload: string): Promise<ImportResult> {
  return invoke('recover_import_json', { payload });
}

export function recoverStartEmpty(): Promise<void> {
  return invoke('recover_start_empty');
}
