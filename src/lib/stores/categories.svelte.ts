import type { UnlistenFn } from '@tauri-apps/api/event';
import { listCategories, type Category, type ListCategoryFilter } from '../api/categories';
import { onDataChanged } from '../api/events';

export type CategoriesStore = {
  readonly items: Category[];
  readonly loading: boolean;
  readonly error: string | null;
  load(): Promise<void>;
  setFilter(filter: ListCategoryFilter): void;
  dispose(): Promise<void>;
};

export function createCategoriesStore(
  initialFilter: ListCategoryFilter = {},
): CategoriesStore {
  let items = $state<Category[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let filter = $state<ListCategoryFilter>(initialFilter);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;

  async function load() {
    loading = true;
    error = null;
    try {
      items = await listCategories(filter);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function setFilter(next: ListCategoryFilter) {
    filter = next;
    void load();
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'categories') void load();
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
    setFilter,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
