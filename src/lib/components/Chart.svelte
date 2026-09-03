<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import {
    ArcElement,
    BarController,
    BarElement,
    CategoryScale,
    Chart as ChartJS,
    DoughnutController,
    Filler,
    Legend,
    LineController,
    LineElement,
    LinearScale,
    PointElement,
    Tooltip,
  } from 'chart.js';
  import type { ChartData, ChartOptions, ChartType } from 'chart.js';

  // 4タブが使う controller / element をまとめて一度だけ登録する。
  ChartJS.register(
    ArcElement,
    BarController,
    BarElement,
    CategoryScale,
    DoughnutController,
    Filler,
    Legend,
    LineController,
    LineElement,
    LinearScale,
    PointElement,
    Tooltip,
  );

  let {
    type,
    data,
    options,
    ariaLabel,
    testId,
  }: {
    /** 生成時に一度だけ読む。切り替えたいときは別の Chart を置くこと。 */
    type: ChartType;
    data: ChartData;
    options?: ChartOptions;
    ariaLabel: string;
    testId?: string;
  } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let chart: ChartJS | null = null;

  onMount(() => {
    if (!canvas) return;
    chart = new ChartJS(canvas, { type, data, options });
  });

  $effect(() => {
    // data / options を読むことで、差し替えのたびにこの effect が動く。
    const nextData = data;
    const nextOptions = options;
    if (!chart) return;
    chart.data = nextData;
    if (nextOptions) chart.options = nextOptions;
    chart.update();
  });

  onDestroy(() => {
    chart?.destroy();
    chart = null;
  });
</script>

<canvas bind:this={canvas} aria-label={ariaLabel} data-testid={testId}></canvas>

<style>
  canvas {
    max-width: 100%;
  }
</style>
