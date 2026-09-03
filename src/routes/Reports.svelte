<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ByCategoryTab from './reports/ByCategoryTab.svelte';
  import MonthlyTab from './reports/MonthlyTab.svelte';
  import TrendTab from './reports/TrendTab.svelte';
  import YearlyTab from './reports/YearlyTab.svelte';
  import { monthlySeries, type MonthlyBucket } from '../lib/api/reports';
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

  // 月次タブの棒グラフは既存コマンドを使う（系列を二重に持たない）。
  let series = $state<MonthlyBucket[]>([]);
  let seriesError = $state<string | null>(null);

  onMount(() => {
    void store.load();
    void monthlySeries(12)
      .then((next) => {
        series = next;
      })
      .catch((e) => {
        seriesError = e instanceof Error ? e.message : String(e);
      });
  });

  onDestroy(() => {
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
    <MonthlyTab report={store.monthly} {series} year={new Date().getFullYear()} month={store.month} />
  {:else if tab === 'yearly'}
    <YearlyTab report={store.yearly} onYearChange={(year) => void store.setYear(year)} />
  {:else if tab === 'category'}
    <ByCategoryTab report={store.byCategory} />
  {:else}
    <TrendTab report={store.netWorth} />
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
