<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { CategoryReport } from '../../lib/api/reports';

  let { report }: { report: CategoryReport | null } = $props();

  const PALETTE = ['#667eea', '#764ba2', '#4facfe', '#00f2fe', '#f6ad55', '#e53e3e', '#38b2ac'];
  const TREND_LIMIT = 5;

  let selectedCategoryId = $state<number | null>(null);

  const expense = $derived(report?.expense ?? []);
  const income = $derived(report?.income ?? []);

  /** 初期は支出上位5本。1つ選ぶとその1本だけ。Rust が降順で返すので先頭を取るだけ。 */
  const visibleSeries = $derived.by(() => {
    const all = report?.series ?? [];
    if (selectedCategoryId !== null) {
      return all.filter((s) => s.category_id === selectedCategoryId);
    }
    const topIds = new Set(expense.slice(0, TREND_LIMIT).map((a) => a.category_id));
    return all.filter((s) => topIds.has(s.category_id));
  });

  const selectedName = $derived(
    selectedCategoryId === null
      ? '支出上位5カテゴリ'
      : ([...expense, ...income].find((a) => a.category_id === selectedCategoryId)?.name ?? '—'),
  );

  function toggle(categoryId: number) {
    selectedCategoryId = selectedCategoryId === categoryId ? null : categoryId;
  }

  function pieData(aggregates: typeof expense) {
    return {
      labels: aggregates.map((a) => a.name),
      datasets: [
        {
          data: aggregates.map((a) => a.amount),
          backgroundColor: aggregates.map((_, i) => PALETTE[i % PALETTE.length]),
        },
      ],
    };
  }

  function pieOptions(aggregates: typeof expense) {
    return {
      responsive: true,
      maintainAspectRatio: false,
      onClick: (_event: unknown, elements: { index: number }[]) => {
        const hit = elements[0];
        if (hit && aggregates[hit.index]) toggle(aggregates[hit.index].category_id);
      },
    };
  }

  const expensePie = $derived(pieData(expense));
  const expensePieOptions = $derived(pieOptions(expense));
  const incomePie = $derived(pieData(income));
  const incomePieOptions = $derived(pieOptions(income));

  const trendData = $derived({
    labels: report?.months ?? [],
    datasets: visibleSeries.map((series, i) => ({
      label: series.name,
      data: series.points,
      borderColor: PALETTE[i % PALETTE.length],
      backgroundColor: PALETTE[i % PALETTE.length],
      tension: 0.25,
    })),
  });

  const trendOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { y: { beginAtZero: true } },
  };
</script>

<div class="by-category">
  <Card>
    {#snippet children()}
      <h2>支出の内訳</h2>
      {#if expense.length === 0}
        <EmptyState title="支出がありません" hint="期間を広げるか取引を記録してください" />
      {:else}
        <div class="pie-box">
          <Chart
            type="doughnut"
            data={expensePie}
            options={expensePieOptions}
            ariaLabel="支出のカテゴリ内訳"
            testId="chart-category-expense"
          />
        </div>
        <ul class="legend" data-testid="legend-expense">
          {#each expense as aggregate, i (aggregate.category_id)}
            <li>
              <button
                type="button"
                class:selected={selectedCategoryId === aggregate.category_id}
                data-testid={`legend-category-${aggregate.category_id}`}
                onclick={() => toggle(aggregate.category_id)}
              >
                <span class="swatch" style={`background: ${PALETTE[i % PALETTE.length]}`}></span>
                <span class="legend-name">{aggregate.name}</span>
                <span class="legend-amount">{formatCurrency(aggregate.amount)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>収入の内訳</h2>
      {#if income.length === 0}
        <EmptyState title="収入がありません" hint="期間を広げるか取引を記録してください" />
      {:else}
        <div class="pie-box">
          <Chart
            type="doughnut"
            data={incomePie}
            options={incomePieOptions}
            ariaLabel="収入のカテゴリ内訳"
            testId="chart-category-income"
          />
        </div>
        <ul class="legend" data-testid="legend-income">
          {#each income as aggregate, i (aggregate.category_id)}
            <li>
              <button
                type="button"
                class:selected={selectedCategoryId === aggregate.category_id}
                data-testid={`legend-category-${aggregate.category_id}`}
                onclick={() => toggle(aggregate.category_id)}
              >
                <span class="swatch" style={`background: ${PALETTE[i % PALETTE.length]}`}></span>
                <span class="legend-name">{aggregate.name}</span>
                <span class="legend-amount">{formatCurrency(aggregate.amount)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <div class="trend-head">
        <h2>カテゴリ別の推移</h2>
        <span data-testid="selected-category">{selectedName}</span>
      </div>
      <div class="chart-box">
        <Chart
          type="line"
          data={trendData}
          options={trendOptions}
          ariaLabel="カテゴリ別の月次推移"
          testId="chart-category-trend"
        />
      </div>
    {/snippet}
  </Card>
</div>

<style>
  .by-category {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--space-5);
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .pie-box {
    height: 240px;
    min-width: 0;
  }

  .chart-box {
    height: 300px;
    min-width: 0;
  }

  .trend-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-3);
  }

  .trend-head span {
    color: var(--muted);
    font-weight: 700;
  }

  .legend {
    list-style: none;
    margin: var(--space-4) 0 0;
    padding: 0;
    display: grid;
    gap: var(--space-1);
  }

  .legend button {
    width: 100%;
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    cursor: pointer;
    font: inherit;
    color: inherit;
    text-align: left;
  }

  .legend button:hover,
  .legend button.selected {
    background: rgba(0, 0, 0, 0.06);
  }

  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }

  .legend-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .legend-amount {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }
</style>
