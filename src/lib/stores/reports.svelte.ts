import type { UnlistenFn } from '@tauri-apps/api/event';
import { onDataChanged } from '../api/events';
import {
  reportByCategory,
  reportMonthly,
  reportNetWorthSeries,
  reportYearly,
  type CategoryReport,
  type MonthlyReport,
  type NetWorthReport,
  type YearlyReport,
} from '../api/reports';
import { presetRange, type RangePreset } from '../utils/yearMonth';

export type ReportsStore = {
  readonly monthly: MonthlyReport | null;
  readonly yearly: YearlyReport | null;
  readonly byCategory: CategoryReport | null;
  readonly netWorth: NetWorthReport | null;
  readonly preset: RangePreset;
  readonly year: number;
  readonly month: number;
  /**
   * 月次レポートを実際に取得した年（`today` の年で固定、`year` のように
   * `setYear` で動かない）。UI 側が「今年」を独自に `new Date()` で読み直す
   * 必要をなくすための、ストアが持つ唯一の正。
   */
  readonly currentYear: number;
  readonly loading: boolean;
  readonly error: string | null;
  setPreset(preset: RangePreset): Promise<void>;
  setYear(year: number): Promise<void>;
  load(): Promise<void>;
  dispose(): Promise<void>;
};

/**
 * 4本のレポートコマンドをまとめて取得する。集計はすべて Rust 側にあるので、
 * ここは取得と失敗の保持しかしない。
 */
export function createReportsStore(today: Date = new Date()): ReportsStore {
  let monthly = $state<MonthlyReport | null>(null);
  let yearly = $state<YearlyReport | null>(null);
  let byCategory = $state<CategoryReport | null>(null);
  let netWorth = $state<NetWorthReport | null>(null);
  let preset = $state<RangePreset>('last12');
  let year = $state(today.getFullYear());
  const month = today.getMonth() + 1;
  const currentYear = today.getFullYear();
  let loading = $state(false);
  let error = $state<string | null>(null);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    const { from, to } = presetRange(preset, today);
    try {
      const [nextMonthly, nextYearly, nextCategory, nextNetWorth] = await Promise.all([
        reportMonthly(currentYear, month),
        reportYearly(year),
        reportByCategory(from, to),
        reportNetWorthSeries(from, to),
      ]);
      if (disposed || id !== requestId) return;
      monthly = nextMonthly;
      yearly = nextYearly;
      byCategory = nextCategory;
      netWorth = nextNetWorth;
    } catch (e) {
      if (disposed || id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (!disposed && id === requestId) loading = false;
    }
  }

  void (async () => {
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
          void load();
        }
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus; explicit load() still runs.
    }
  })();

  return {
    get monthly() {
      return monthly;
    },
    get yearly() {
      return yearly;
    },
    get byCategory() {
      return byCategory;
    },
    get netWorth() {
      return netWorth;
    },
    get preset() {
      return preset;
    },
    get year() {
      return year;
    },
    get month() {
      return month;
    },
    get currentYear() {
      return currentYear;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    async setPreset(next) {
      preset = next;
      await load();
    },
    async setYear(next) {
      year = next;
      await load();
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
