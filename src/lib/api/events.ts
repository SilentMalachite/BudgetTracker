import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type ChangedDomain =
  | 'categories'
  | 'accounts'
  | 'transactions'
  | 'budgets'
  | 'recurring'
  | 'meta';

export type DataChangedPayload = {
  domain: ChangedDomain;
};

export async function onDataChanged(
  cb: (domain: ChangedDomain) => void,
): Promise<UnlistenFn> {
  return listen<DataChangedPayload>('data:changed', (event) => {
    cb(event.payload.domain);
  });
}
