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
    PieController,
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
    PieController,
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
  // onMount と $effect の初回実行は同じマウントの中で両方走る (Svelte はどちらも
  // 同じ component_context.e に積んで宣言順に flush するため、onMount が先に
  // 必ず終わる)。このフラグで $effect の「初回だけ」を無視し、生成直後の
  // 冗長な chart.update() を防ぐ。
  let justCreated = false;

  onMount(() => {
    if (!canvas) return;
    chart = new ChartJS(canvas, { type, data, options });
    justCreated = true;
  });

  $effect(() => {
    // data / options を読むことで、差し替えのたびにこの effect が動く。
    const nextData = data;
    const nextOptions = options;
    if (!chart) return;
    if (justCreated) {
      // onMount が今まさにこのデータで生成したばかり。反映済みなので何もしない。
      justCreated = false;
      return;
    }
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
