<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import ReportState from '../../lib/components/ReportState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { YearlyReport } from '../../lib/api/reports';

  let {
    report,
    year,
    onYearChange,
    error,
  }: {
    report: YearlyReport | null;
    /** ストアが持つ唯一の正。`report?.year` は非同期で遅れるので入力欄には使わない。 */
    year: number;
    onYearChange: (year: number) => void;
    error: string | null;
  } = $props();

  let yearError = $state<string | null>(null);

  function handleYearChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const next = Number(input.value);
    if (Number.isInteger(next) && next >= 1000 && next <= 9999) {
      yearError = null;
      onYearChange(next);
      return;
    }
    // 拒否したことを画面に残す。value はストアの year に固定したままなので、
    // 入力欄自体もここで正しい値に戻しておく（黙って古い/不正な表示のまま
    // にしない）。
    yearError = `"${input.value}" は年として使えません。1000〜9999の整数を入力してください`;
    input.value = String(year);
  }

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
      value={year}
      data-testid="report-year"
      onchange={handleYearChange}
    />
  </div>
  {#if yearError}
    <p class="year-error" data-testid="report-year-error">{yearError}</p>
  {/if}

  <Card>
    {#snippet children()}
      <h2>月別の収入 / 支出</h2>
      {#if report === null}
        <ReportState {error} />
      {:else}
        <div class="chart-box">
          <Chart
            type="bar"
            data={chartData}
            options={chartOptions}
            ariaLabel="年間の収入と支出の積み上げ"
            testId="chart-reports-yearly"
          />
        </div>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>年間サマリー</h2>
      {#if report === null}
        <ReportState {error} />
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

  .year-error {
    color: var(--danger);
    font-weight: 700;
    margin: calc(-1 * var(--space-3)) 0 0;
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
