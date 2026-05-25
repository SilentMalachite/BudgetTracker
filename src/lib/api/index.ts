import { invoke } from '@tauri-apps/api/core';

export type AppInfo = {
  schema_version: number;
  db_path: string;
};

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info');
}

export * from './events';
export * from './categories';
export * from './accounts';
export * from './transactions';
export * from './reports';
export * from './backup';
export * from './settings';
export * from './balances';
export * from './budgets';
