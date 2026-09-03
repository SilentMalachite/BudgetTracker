<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import ReportState from '../../lib/components/ReportState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { Delta, MonthlyBucket, MonthlyReport } from '../../lib/api/reports';

  let {
    report,
    series,
    year,
    month,
    error,
    seriesError,
  }: {
    report: MonthlyReport | null;
    series: MonthlyBucket[];
    year: number;
    month: number;
    /** 比較・Top5 カードが読む、4本レポートの取得失敗。 */
    error: string | null;
    /** 棒グラフが読む、`monthlySeries` 単独の取得失敗（4本とは別物、規約はブリーフ参照）。 */
    seriesError: string | null;
  } = $props();

  const chartData = $derived({
    labels: series.map((bucket) => bucket.year_month),
    datasets: [
      { label: '収入', data: series.map((b) => b.income), backgroundColor: '#4facfe' },
      { label: '支出', data: series.map((b) => b.expense), backgroundColor: '#e53e3e' },
    ],
  });

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { y: { beginAtZero: true } },
  };

  /** 割合は Rust 側が出す。ここは表示だけ（null は比較対象が 0 のとき）。 */
  function percentLabel(delta: Delta): string {
    return delta.expense_percent === null ? '—' : `${delta.expense_percent}%`;
  }
</script>

<div class="monthly">
  <Card>
    {#snippet children()}
      <h2>月別収支</h2>
      {#if series.length === 0}
        <ReportState error={seriesError} />
      {:else}
        <div class="chart-box">
          <Chart
            type="bar"
            data={chartData}
            options={chartOptions}
            ariaLabel="直近12ヶ月の収入と支出"
            testId="chart-reports-monthly"
          />
        </div>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>{year}年{month}月の比較</h2>
      {#if report === null}
        <ReportState {error} />
      {:else}
        <dl class="compare" data-testid="monthly-compare">
          <div>
            <dt>今月の支出</dt>
            <dd data-testid="compare-current-expense">{formatCurrency(report.current.expense)}</dd>
          </div>
          <div>
            <dt>前月比</dt>
            <dd data-testid="compare-mom">
              {formatCurrency(report.mom.expense_diff)} ({percentLabel(report.mom)})
            </dd>
          </div>
          <div>
            <dt>前年同月比</dt>
            <dd data-testid="compare-yoy">
              {formatCurrency(report.yoy.expense_diff)} ({percentLabel(report.yoy)})
            </dd>
          </div>
        </dl>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>支出 Top5</h2>
      {#if report === null}
        <ReportState {error} />
      {:else if report.top_expense.length === 0}
        <EmptyState title="支出がありません" hint="取引ページから記録できます" />
      {:else}
        <ol class="ranking" data-testid="top-expense">
          {#each report.top_expense as aggregate (aggregate.category_id)}
            <li>
              <span>{aggregate.name}</span>
              <strong>{formatCurrency(aggregate.amount)}</strong>
            </li>
          {/each}
        </ol>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>収入 Top5</h2>
      {#if report === null}
        <ReportState {error} />
      {:else if report.top_income.length === 0}
        <EmptyState title="収入がありません" hint="取引ページから記録できます" />
      {:else}
        <ol class="ranking" data-testid="top-income">
          {#each report.top_income as aggregate (aggregate.category_id)}
            <li>
              <span>{aggregate.name}</span>
              <strong>{formatCurrency(aggregate.amount)}</strong>
            </li>
          {/each}
        </ol>
      {/if}
    {/snippet}
  </Card>
</div>

<style>
  .monthly {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--space-5);
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .chart-box {
    height: 280px;
    min-width: 0;
  }

  .compare {
    display: grid;
    gap: var(--space-3);
    margin: 0;
  }

  .compare div {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .compare dt {
    color: var(--muted);
    font-weight: 700;
  }

  .compare dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }

  .ranking {
    margin: 0;
    padding-left: var(--space-5);
    display: grid;
    gap: var(--space-2);
  }

  .ranking li {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .ranking strong {
    font-variant-numeric: tabular-nums;
  }
</style>
