<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { YearlyReport } from '../../lib/api/reports';

  let {
    report,
    onYearChange,
    loading,
    error,
  }: {
    report: YearlyReport | null;
    onYearChange: (year: number) => void;
    loading: boolean;
    error: string | null;
  } = $props();

  const chartData = $derived({
    labels: (report?.months ?? []).map((bucket) => bucket.year_month),
    datasets: [
      {
        label: '収入',
        data: (report?.months ?? []).map((b) => b.income),
        backgroundColor: '#4facfe',
      },
      {
        label: '支出',
        data: (report?.months ?? []).map((b) => b.expense),
        backgroundColor: '#e53e3e',
      },
    ],
  });

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { x: { stacked: true }, y: { stacked: true, beginAtZero: true } },
  };
</script>

<div class="yearly">
  <div class="year-picker">
    <label for="report-year">対象年</label>
    <input
      id="report-year"
      type="number"
      min="1000"
      max="9999"
      value={report?.year ?? new Date().getFullYear()}
      data-testid="report-year"
      onchange={(event) => {
        const next = Number((event.currentTarget as HTMLInputElement).value);
        if (Number.isInteger(next) && next >= 1000 && next <= 9999) onYearChange(next);
      }}
    />
  </div>

  <Card>
    {#snippet children()}
      <h2>月別の収入 / 支出</h2>
      <div class="chart-box">
        <Chart
          type="bar"
          data={chartData}
          options={chartOptions}
          ariaLabel="年間の収入と支出の積み上げ"
          testId="chart-reports-yearly"
        />
      </div>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>年間サマリー</h2>
      {#if report === null}
        {#if loading}
          <EmptyState title="読み込み中" hint="集計を取得しています" />
        {:else if error}
          <EmptyState title="読み込みに失敗しました" hint={error} />
        {:else}
          <EmptyState title="読み込み中" hint="集計を取得しています" />
        {/if}
      {:else}
        <dl class="summary" data-testid="yearly-summary">
          <div>
            <dt>年間収入</dt>
            <dd>{formatCurrency(report.total_income)}</dd>
          </div>
          <div>
            <dt>年間支出</dt>
            <dd>{formatCurrency(report.total_expense)}</dd>
          </div>
          <div>
            <dt>12ヶ月平均支出</dt>
            <dd data-testid="yearly-avg-expense">{formatCurrency(report.avg_expense)}</dd>
          </div>
          <div>
            <dt>最大支出月</dt>
            <dd data-testid="yearly-max-month">{report.max_expense_month ?? '—'}</dd>
          </div>
        </dl>
      {/if}
    {/snippet}
  </Card>
</div>

<style>
  .yearly {
    display: grid;
    gap: var(--space-5);
  }

  .year-picker {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    color: white;
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .chart-box {
    height: 320px;
    min-width: 0;
  }

  .summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-4);
    margin: 0;
  }

  .summary dt {
    color: var(--muted);
    font-weight: 700;
  }

  .summary dd {
    margin: var(--space-2) 0 0;
    font-size: 1.3rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
