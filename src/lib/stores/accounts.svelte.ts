import type { UnlistenFn } from '@tauri-apps/api/event';
import { listAccounts, type Account } from '../api/accounts';
import { onDataChanged } from '../api/events';

export type AccountsStore = {
  readonly items: Account[];
  readonly loading: boolean;
  readonly error: string | null;
  load(): Promise<void>;
  setIncludeArchived(value: boolean): void;
  dispose(): Promise<void>;
};

export function createAccountsStore(initialIncludeArchived = false): AccountsStore {
  let items = $state<Account[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let includeArchived = $state(initialIncludeArchived);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;

  async function load() {
    loading = true;
    error = null;
    try {
      items = await listAccounts(includeArchived);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function setIncludeArchived(value: boolean) {
    includeArchived = value;
    void load();
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'accounts') void load();
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus; initial load should still run.
    }
  })();

  return {
    get items() {
      return items;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    load,
    setIncludeArchived,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
