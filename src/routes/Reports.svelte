<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import ByCategoryTab from './reports/ByCategoryTab.svelte';
  import MonthlyTab from './reports/MonthlyTab.svelte';
  import TrendTab from './reports/TrendTab.svelte';
  import YearlyTab from './reports/YearlyTab.svelte';
  import { monthlySeries, type MonthlyBucket } from '../lib/api/reports';
  import { onDataChanged } from '../lib/api/events';
  import { createReportsStore } from '../lib/stores/reports.svelte';
  import type { RangePreset } from '../lib/utils/yearMonth';

  type Tab = 'monthly' | 'yearly' | 'category' | 'trend';

  const tabs: { id: Tab; label: string }[] = [
    { id: 'monthly', label: '月次' },
    { id: 'yearly', label: '年次' },
    { id: 'category', label: 'カテゴリ別' },
    { id: 'trend', label: 'トレンド' },
  ];

  const presets: { id: RangePreset; label: string }[] = [
    { id: 'last6', label: '直近6ヶ月' },
    { id: 'last12', label: '直近12ヶ月' },
    { id: 'last24', label: '直近24ヶ月' },
    { id: 'thisYear', label: '今年' },
  ];

  const store = createReportsStore();

  // 月次タブの棒グラフは既存コマンドを使う（系列を二重に持たない）。ストアの4本と
  // 同じ data:changed 信号で再取得し、同じ「古い応答は無視する」ガードをかける。
  // そうしないと、画面を開いたまま他画面で取引を編集したときこのグラフだけ古くなる。
  let series = $state<MonthlyBucket[]>([]);
  let seriesError = $state<string | null>(null);
  let seriesUnlisten: UnlistenFn | null = null;
  let seriesDisposed = false;
  let seriesRequestId = 0;

  async function loadSeries() {
    const id = ++seriesRequestId;
    try {
      const next = await monthlySeries(12);
      if (seriesDisposed || id !== seriesRequestId) return;
      series = next;
      seriesError = null;
    } catch (e) {
      if (seriesDisposed || id !== seriesRequestId) return;
      seriesError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    void store.load();
    void loadSeries();
    void (async () => {
      try {
        const nextUnlisten = await onDataChanged((domain) => {
          if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
            void loadSeries();
          }
        });
        if (seriesDisposed) nextUnlisten();
        else seriesUnlisten = nextUnlisten;
      } catch {
        // Browser-only E2E has no Tauri event bus; the explicit loadSeries() above already ran.
      }
    })();
  });

  onDestroy(() => {
    seriesDisposed = true;
    if (seriesUnlisten) {
      seriesUnlisten();
      seriesUnlisten = null;
    }
    void store.dispose();
  });

  let tab = $state<Tab>('monthly');
</script>

<section data-testid="page-reports">
  <h1>レポート</h1>

  {#if store.error}
    <p class="error" data-testid="reports-error">エラー: {store.error}</p>
  {/if}
  {#if seriesError}
    <p class="error" data-testid="reports-series-error">エラー: {seriesError}</p>
  {/if}

  <div class="controls">
    <div class="tabs" role="tablist" aria-label="レポートの種類">
      {#each tabs as item (item.id)}
        <button
          type="button"
          role="tab"
          aria-selected={tab === item.id}
          class:active={tab === item.id}
          data-testid={`reports-tab-${item.id}`}
          onclick={() => (tab = item.id)}
        >
          {item.label}
        </button>
      {/each}
    </div>

    {#if tab === 'category' || tab === 'trend'}
      <div class="presets" data-testid="reports-presets">
        {#each presets as item (item.id)}
          <button
            type="button"
            class:active={store.preset === item.id}
            data-testid={`reports-preset-${item.id}`}
            onclick={() => void store.setPreset(item.id)}
          >
            {item.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if tab === 'monthly'}
    <MonthlyTab
      report={store.monthly}
      {series}
      year={store.currentYear}
      month={store.month}
      error={store.error}
      {seriesError}
    />
  {:else if tab === 'yearly'}
    <YearlyTab
      report={store.yearly}
      year={store.year}
      onYearChange={(year) => void store.setYear(year)}
      error={store.error}
    />
  {:else if tab === 'category'}
    <ByCategoryTab report={store.byCategory} error={store.error} />
  {:else}
    <TrendTab report={store.netWorth} error={store.error} />
  {/if}
</section>

<style>
  h1 {
    color: white;
    margin: 0 0 var(--space-5);
  }

  .controls {
    display: flex;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }

  .tabs,
  .presets {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  button {
    padding: var(--space-2) var(--space-4);
    border: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    color: white;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
  }

  button:hover {
    background: rgba(255, 255, 255, 0.24);
  }

  button.active {
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }

  .error {
    color: var(--danger);
    font-weight: 700;
  }
</style>
