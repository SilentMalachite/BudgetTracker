<script lang="ts">
  import type { ChartData } from 'chart.js';

  // Chart.js を jsdom で描画できないので描画そのものを差し替える。data だけは
  // JSON にして data-chart に載せる: 凡例と推移グラフで同じカテゴリに同じ色が
  // 入っているかをテストから確認するのに必要 (色は data.datasets[].borderColor /
  // backgroundColor に載る)。options は関数を含み得るのでシリアライズしない。
  let {
    data,
    ariaLabel,
    testId,
  }: { data?: ChartData; ariaLabel: string; testId?: string } = $props();
</script>

<div
  aria-label={ariaLabel}
  data-testid={testId}
  data-chart={data ? JSON.stringify(data) : undefined}
></div>
