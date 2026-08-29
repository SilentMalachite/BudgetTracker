import type { UnlistenFn } from '@tauri-apps/api/event';
import { listBalances, type AccountBalance } from '../api/balances';
import { onDataChanged } from '../api/events';

export type BalancesStore = {
  readonly items: AccountBalance[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly totalAssets: number;
  load(): Promise<void>;
  dispose(): Promise<void>;
};

export function createBalancesStore(): BalancesStore {
  let items = $state<AccountBalance[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let totalAssets = $state(0);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    try {
      const res = await listBalances();
      if (id !== requestId) return;
      items = res.accounts;
      totalAssets = res.total_assets;
    } catch (e) {
      if (id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'transactions' || domain === 'accounts') void load();
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus; initial load still ran.
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
    get totalAssets() {
      return totalAssets;
    },
    load,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
