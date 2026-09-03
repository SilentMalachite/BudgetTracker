import type { UnlistenFn } from '@tauri-apps/api/event';

import { onDataChanged } from '../api/events';
import {
  listRecurringRules,
  setRecurringRuleActive,
  type RecurringRuleView,
} from '../api/recurring';

export type RecurringStore = {
  readonly items: RecurringRuleView[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly includeInactive: boolean;
  load(): Promise<void>;
  setIncludeInactive(next: boolean): void;
  toggleActive(id: number, active: boolean): Promise<void>;
  dispose(): Promise<void>;
};

export function createRecurringStore(): RecurringStore {
  let items = $state<RecurringRuleView[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    try {
      const res = await listRecurringRules(includeInactive);
      if (id !== requestId) return;
      items = res;
    } catch (e) {
      if (id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function setIncludeInactive(next: boolean) {
    includeInactive = next;
    void load();
  }

  async function toggleActive(id: number, active: boolean) {
    try {
      await setRecurringRuleActive(id, active);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'recurring' || domain === 'accounts' || domain === 'categories') {
          void load();
        }
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // ブラウザだけの E2E には Tauri のイベントバスが無い。初回ロードは走る。
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
    get includeInactive() {
      return includeInactive;
    },
    load,
    setIncludeInactive,
    toggleActive,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
