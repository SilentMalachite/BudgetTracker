import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  listTransactions,
  type ListTransactionFilter,
  type Transaction,
} from '../api/transactions';
import { onDataChanged } from '../api/events';

export type TransactionsStore = {
  readonly items: Transaction[];
  readonly total: number;
  readonly loading: boolean;
  readonly error: string | null;
  readonly page: number;
  readonly pageSize: number;
  readonly filter: ListTransactionFilter;
  load(): Promise<void>;
  setFilter(filter: ListTransactionFilter): void;
  setPage(page: number): void;
  dispose(): Promise<void>;
};

export function createTransactionsStore(
  initialFilter: ListTransactionFilter = {},
  initialPageSize = 50,
): TransactionsStore {
  let items = $state<Transaction[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let page = $state(0);
  let pageSize = $state(initialPageSize);
  let filter = $state<ListTransactionFilter>(initialFilter);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    try {
      const res = await listTransactions(filter, page, pageSize);
      if (id !== requestId) return;
      items = res.items;
      total = res.total;
    } catch (e) {
      if (id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function setFilter(next: ListTransactionFilter) {
    filter = next;
    page = 0;
    void load();
  }

  function setPage(next: number) {
    page = Math.max(0, next);
    void load();
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
          void load();
        }
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
    get total() {
      return total;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get page() {
      return page;
    },
    get pageSize() {
      return pageSize;
    },
    get filter() {
      return filter;
    },
    load,
    setFilter,
    setPage,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
