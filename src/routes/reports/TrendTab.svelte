<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import ReportState from '../../lib/components/ReportState.svelte';
  import type { NetWorthReport } from '../../lib/api/reports';

  let {
    report,
    error,
  }: {
    report: NetWorthReport | null;
    error: string | null;
  } = $props();

  const points = $derived(report?.points ?? []);
  const labels = $derived(points.map((point) => point.year_month));

  const netWorthData = $derived({
    labels,
    datasets: [
      {
        label: '純資産',
        data: points.map((point) => point.net_worth),
        borderColor: '#667eea',
        backgroundColor: 'rgba(102, 126, 234, 0.25)',
        fill: true,
        tension: 0.25,
      },
    ],
  });

  const netData = $derived({
    labels,
    datasets: [
      {
        type: 'bar' as const,
        label: '月次収支',
        data: points.map((point) => point.net),
        backgroundColor: '#4facfe',
      },
      {
        type: 'line' as const,
        label: '3ヶ月移動平均',
        // 窓が埋まらない先頭2点は null のまま渡し、線を描かせない。
        data: points.map((point) => point.net_moving_avg),
        borderColor: '#e53e3e',
        backgroundColor: '#e53e3e',
        spanGaps: false,
        tension: 0.25,
      },
    ],
  });

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { y: { beginAtZero: false } },
  };
</script>

<div class="trend">
  <Card>
    {#snippet children()}
      <h2>純資産推移</h2>
      {#if report === null}
        <ReportState {error} />
      {:else if points.length === 0}
        <EmptyState title="データがありません" hint="口座と取引を登録すると表示されます" />
      {:else}
        <div class="chart-box">
          <Chart
            type="line"
            data={netWorthData}
            {options}
            ariaLabel="非アーカイブ口座合計の純資産推移"
            testId="chart-net-worth"
          />
        </div>
        <p class="note">非アーカイブ口座の合計。振替の出入りを含みます。</p>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>月次収支と3ヶ月移動平均</h2>
      {#if report === null}
        <ReportState {error} />
      {:else if points.length === 0}
        <EmptyState title="データがありません" hint="取引ページから記録できます" />
      {:else}
        <div class="chart-box">
          <Chart
            type="bar"
            data={netData}
            {options}
            ariaLabel="月次収支と3ヶ月移動平均"
            testId="chart-net-moving-average"
          />
        </div>
        <p class="note">月次収支は振替を除いた収入 − 支出です。</p>
      {/if}
    {/snippet}
  </Card>
</div>

<style>
  .trend {
    display: grid;
    gap: var(--space-5);
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .chart-box {
    height: 300px;
    min-width: 0;
  }

  .note {
    margin: var(--space-3) 0 0;
    color: var(--muted);
  }
</style>
