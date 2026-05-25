<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    BarController,
    BarElement,
    CategoryScale,
    Chart,
    Legend,
    LinearScale,
    Tooltip,
  } from 'chart.js';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import Card from '../lib/components/Card.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import { listBudgetStatuses, type BudgetStatus } from '../lib/api/budgets';
  import { onDataChanged } from '../lib/api/events';
  import { monthlySeries, monthlySummary, type MonthlyBucket, type MonthlySummary } from '../lib/api/reports';
  import { listTransactions, type Transaction } from '../lib/api/transactions';
  import { createBalancesStore } from '../lib/stores/balances.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';

  Chart.register(BarController, BarElement, CategoryScale, LinearScale, Tooltip, Legend);

  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });
  const today = new Date();
  const currentYear = today.getFullYear();
  const currentMonth = today.getMonth() + 1;
  const currentYearMonth = `${currentYear}-${String(currentMonth).padStart(2, '0')}`;
  const catStore = createCategoriesStore({ include_archived: true });
  const balancesStore = createBalancesStore();

  let summary = $state<MonthlySummary | null>(null);
  let series = $state<MonthlyBucket[]>([]);
  let recent = $state<Transaction[]>([]);
  let budgetStatuses = $state<BudgetStatus[]>([]);
  let error = $state<string | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let chart: Chart<'bar'> | null = null;
  let unlisten: UnlistenFn | null = null;

  const categoryById = $derived(new Map(catStore.items.map((item) => [item.id, item])));

  async function reload() {
    error = null;
    try {
      const [nextSummary, nextSeries, nextRecent, nextBudgets] = await Promise.all([
        monthlySummary(currentYear, currentMonth),
        monthlySeries(12),
        listTransactions({}, 0, 10),
        listBudgetStatuses(currentYearMonth),
      ]);
      summary = nextSummary;
      series = nextSeries;
      recent = nextRecent.items;
      budgetStatuses = nextBudgets;
      drawChart();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      series = [];
      recent = [];
      budgetStatuses = [];
      drawChart();
    }
  }

  function drawChart() {
    if (!canvas) return;
    const labels = series.map((bucket) => bucket.year_month);
    const income = series.map((bucket) => bucket.income);
    const expense = series.map((bucket) => bucket.expense);

    if (chart) {
      chart.data.labels = labels;
      chart.data.datasets[0].data = income;
      chart.data.datasets[1].data = expense;
      chart.update();
      return;
    }

    chart = new Chart(canvas, {
      type: 'bar',
      data: {
        labels,
        datasets: [
          { label: '収入', data: income, backgroundColor: '#4facfe' },
          { label: '支出', data: expense, backgroundColor: '#e53e3e' },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        scales: { y: { beginAtZero: true } },
      },
    });
  }

  onMount(() => {
    void (async () => {
      await reload();
      try {
        unlisten = await onDataChanged((domain) => {
          if (
            domain === 'transactions' ||
            domain === 'categories' ||
            domain === 'accounts' ||
            domain === 'budgets'
          ) {
            void reload();
          }
        });
      } catch {
        // Browser-only E2E has no Tauri event bus; the dashboard can still render.
      }
    })();
  });

  onDestroy(() => {
    chart?.destroy();
    chart = null;
    unlisten?.();
    unlisten = null;
    void catStore.dispose();
    void balancesStore.dispose();
  });

  const topBudgetStatuses = $derived(
    [...budgetStatuses]
      .filter((status) => status.budget_id != null)
      .sort((a: BudgetStatus, b: BudgetStatus) => {
        const percentOrder = b.percent - a.percent;
        if (percentOrder !== 0) return percentOrder;
        const projectedOrder = Number(b.projected_over_budget) - Number(a.projected_over_budget);
        if (projectedOrder !== 0) return projectedOrder;
        return a.category_name.localeCompare(b.category_name, 'ja');
      })
      .slice(0, 3),
  );
</script>

<section data-testid="page-dashboard">
  <h1>ダッシュボード</h1>

  {#if error}
    <p class="error">エラー: {error}</p>
  {/if}

  <div class="assets-section">
    <Card>
      {#snippet children()}
        <div class="assets-card">
          <div class="assets-head">
            <small>総資産</small>
            <strong data-testid="card-total-assets">{yen.format(balancesStore.totalAssets)}</strong>
            {#if balancesStore.error}
              <small class="error">エラー: {balancesStore.error}</small>
            {/if}
          </div>
          {#if balancesStore.items.length === 0 && !balancesStore.loading}
            <EmptyState title="口座がありません" hint="口座ページから追加してください" />
          {:else}
            <ul class="assets-list" data-testid="assets-list">
              {#each balancesStore.items.filter((b) => b.archived_at == null).slice(0, 8) as account (account.account_id)}
                <li>
                  <span class="acct-name">{account.name}</span>
                  <span class="acct-balance" data-testid={`balance-${account.account_id}`}>
                    {yen.format(account.balance)}
                  </span>
                </li>
              {/each}
              {#if balancesStore.items.filter((b) => b.archived_at == null).length > 8}
                <li class="more">
                  他 {balancesStore.items.filter((b) => b.archived_at == null).length - 8} 件
                </li>
              {/if}
            </ul>
          {/if}
        </div>
      {/snippet}
    </Card>
  </div>

  <div class="summary-grid">
    <Card>
      {#snippet children()}
        <small>{currentYear}年{currentMonth}月の収入</small>
        <strong class="income" data-testid="card-income">
          {summary ? yen.format(summary.income) : '---'}
        </strong>
      {/snippet}
    </Card>
    <Card>
      {#snippet children()}
        <small>{currentYear}年{currentMonth}月の支出</small>
        <strong class="expense" data-testid="card-expense">
          {summary ? yen.format(summary.expense) : '---'}
        </strong>
      {/snippet}
    </Card>
    <Card>
      {#snippet children()}
        <small>当月の収支</small>
        <strong
          class:positive={(summary?.net ?? 0) >= 0}
          class:negative={(summary?.net ?? 0) < 0}
          data-testid="card-net"
        >
          {summary ? yen.format(summary.net) : '---'}
        </strong>
      {/snippet}
    </Card>
  </div>

  <div class="dashboard-grid">
    <Card>
      {#snippet children()}
        <h2>月別収支</h2>
        <div class="chart-box">
          <canvas bind:this={canvas} aria-label="直近12ヶ月の収入と支出"></canvas>
        </div>
      {/snippet}
    </Card>

    <Card>
      {#snippet children()}
        <h2>直近の取引</h2>
        {#if recent.length === 0}
          <EmptyState title="まだ取引がありません" hint="取引ページから記録できます" />
        {:else}
          <ul class="recent-list" data-testid="recent-list">
            {#each recent as transaction (transaction.id)}
              <li>
                <span class="date">{transaction.occurred_on}</span>
                <span class="category">
                  {#if transaction.category_id != null}
                    {categoryById.get(transaction.category_id)?.name ?? '-'}
                  {:else}
                    振替
                  {/if}
                </span>
                <span class="description">{transaction.description || '-'}</span>
                <span
                  class="amount"
                  class:income={transaction.type === 'income'}
                  class:expense={transaction.type === 'expense'}
                >
                  {#if transaction.type === 'expense'}-{:else if transaction.type === 'income'}+{/if}{yen.format(transaction.amount)}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      {/snippet}
    </Card>

    <Card>
      {#snippet children()}
        <h2>予算進捗</h2>
        {#if topBudgetStatuses.length === 0}
          <EmptyState title="予算が未設定です" hint="予算ページで月別予算を設定してください" />
        {:else}
          <ul class="budget-widget" data-testid="dashboard-budget-widget">
            {#each topBudgetStatuses as status (status.category_id)}
              <li data-testid={`dashboard-budget-row-${status.category_id}`}>
                <div class="budget-widget-head">
                  <strong>{status.category_name}</strong>
                  <span>{status.percent}%</span>
                </div>
                <div class="budget-widget-bar">
                  <span style={`width: ${status.progress_percent}%`}></span>
                </div>
                <small>
                  {yen.format(status.spent)} / {yen.format(status.budgeted)}
                  {#if status.threshold_reached}<em>警告</em>{/if}
                  {#if status.projected_over_budget}<em>予測超過</em>{/if}
                </small>
              </li>
            {/each}
          </ul>
        {/if}
      {/snippet}
    </Card>
  </div>
</section>

<style>
  h1 {
    color: white;
    margin: 0 0 var(--space-5);
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .summary-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }

  small {
    color: var(--muted);
    font-weight: 700;
  }

  strong {
    display: block;
    margin-top: var(--space-2);
    font-size: 1.8rem;
    font-variant-numeric: tabular-nums;
    line-height: 1.2;
  }

  .income,
  .positive {
    color: var(--success);
  }

  .expense,
  .negative {
    color: var(--danger);
  }

  .dashboard-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(320px, 0.65fr);
    gap: var(--space-5);
  }

  .chart-box {
    height: 280px;
    min-width: 0;
  }

  .recent-list {
    display: grid;
    gap: var(--space-2);
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .recent-list li {
    display: grid;
    grid-template-columns: 6rem 7rem minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    border-bottom: 1px solid var(--border);
    padding: var(--space-2) 0;
  }

  .recent-list li:last-child {
    border-bottom: 0;
  }

  .budget-widget {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: var(--space-4);
  }

  .budget-widget li {
    display: grid;
    gap: var(--space-2);
    border-bottom: 1px solid var(--border);
    padding-bottom: var(--space-3);
  }

  .budget-widget li:last-child {
    border-bottom: 0;
    padding-bottom: 0;
  }

  .budget-widget-head {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
    align-items: baseline;
  }

  .budget-widget-head strong {
    margin: 0;
    font-size: 1rem;
  }

  .budget-widget-head span {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .budget-widget-bar {
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.08);
  }

  .budget-widget-bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }

  .budget-widget em {
    margin-left: var(--space-2);
    color: var(--danger);
    font-style: normal;
    font-weight: 700;
  }

  .date,
  .description {
    color: var(--muted);
  }

  .category {
    font-weight: 700;
  }

  .description {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .amount {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    white-space: nowrap;
  }

  .error {
    color: var(--danger);
    font-weight: 700;
  }

  @media (max-width: 980px) {
    .summary-grid,
    .dashboard-grid {
      grid-template-columns: 1fr;
    }

    .recent-list li {
      grid-template-columns: 5.5rem minmax(0, 1fr) auto;
    }

    .description {
      display: none;
    }
  }

  .assets-card {
    display: grid;
    gap: var(--space-4);
  }
  .assets-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .assets-head strong {
    font-size: 2rem;
    margin: 0;
  }
  .assets-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--space-2) var(--space-4);
  }
  .assets-list li {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    border-bottom: 1px dashed var(--border);
    padding: var(--space-2) 0;
  }
  .assets-list li.more {
    color: var(--muted);
    justify-content: center;
    grid-column: 1 / -1;
  }
  .acct-name {
    color: var(--muted);
  }
  .acct-balance {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .assets-section {
    margin-bottom: var(--space-5);
  }
</style>
