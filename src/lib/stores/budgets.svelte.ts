import type { UnlistenFn } from '@tauri-apps/api/event';
import { listBudgetStatuses, type BudgetStatus } from '../api/budgets';
import { onDataChanged } from '../api/events';

export type BudgetsStore = {
  readonly items: BudgetStatus[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly yearMonth: string;
  load(): Promise<void>;
  setYearMonth(yearMonth: string): void;
  dispose(): Promise<void>;
};

export function createBudgetsStore(initialYearMonth: string): BudgetsStore {
  let items = $state<BudgetStatus[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let yearMonth = $state(initialYearMonth);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    try {
      const res = await listBudgetStatuses(yearMonth);
      if (id !== requestId) return;
      items = res;
    } catch (e) {
      if (id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function setYearMonth(next: string) {
    yearMonth = next;
    void load();
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'budgets' || domain === 'categories' || domain === 'transactions') {
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
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get yearMonth() {
      return yearMonth;
    },
    load,
    setYearMonth,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
