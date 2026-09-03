import { render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const update = vi.fn();
const destroy = vi.fn();
const construct = vi.fn();

vi.mock('chart.js', () => {
  class FakeChart {
    data: unknown;
    options: unknown;
    update = update;
    destroy = destroy;
    constructor(_canvas: unknown, config: { data: unknown; options: unknown }) {
      construct(config);
      this.data = config.data;
      this.options = config.options;
    }
    static register = vi.fn();
  }
  return {
    Chart: FakeChart,
    ArcElement: class {},
    BarController: class {},
    BarElement: class {},
    CategoryScale: class {},
    DoughnutController: class {},
    Filler: class {},
    Legend: class {},
    LineController: class {},
    LineElement: class {},
    LinearScale: class {},
    PieController: class {},
    PointElement: class {},
    Tooltip: class {},
  };
});

const Chart = (await import('./Chart.svelte')).default;

describe('Chart.svelte', () => {
  beforeEach(() => {
    update.mockClear();
    destroy.mockClear();
    construct.mockClear();
  });

  it('builds the chart once with the given data', () => {
    const data = { labels: ['a'], datasets: [{ label: 'x', data: [1] }] };
    const options = { responsive: true };

    render(Chart, {
      props: {
        type: 'bar' as const,
        data,
        options,
        ariaLabel: 'テスト',
      },
    });

    expect(construct).toHaveBeenCalledTimes(1);
    const [config] = construct.mock.calls[0];
    expect(config.type).toBe('bar');
    expect(config.data).toEqual(data);
    expect(config.options).toEqual(options);
  });

  it('does not call update on mount, but calls it once when data changes', async () => {
    const { rerender } = render(Chart, {
      props: {
        type: 'bar' as const,
        data: { labels: ['a'], datasets: [{ label: 'x', data: [1] }] },
        ariaLabel: 'テスト',
      },
    });

    expect(update).toHaveBeenCalledTimes(0);

    await rerender({
      data: { labels: ['a', 'b'], datasets: [{ label: 'x', data: [1, 2] }] },
    });

    expect(update).toHaveBeenCalledTimes(1);
  });

  it('does not rebuild the chart when type changes after mount', async () => {
    const { rerender } = render(Chart, {
      props: {
        type: 'bar' as const,
        data: { labels: [], datasets: [] },
        ariaLabel: 'テスト',
      },
    });

    expect(construct).toHaveBeenCalledTimes(1);

    await rerender({ type: 'line' as const });

    expect(construct).toHaveBeenCalledTimes(1);
  });

  it('exposes the canvas with its aria-label and test id', () => {
    const { getByTestId } = render(Chart, {
      props: {
        type: 'line' as const,
        data: { labels: [], datasets: [] },
        ariaLabel: '純資産推移',
        testId: 'chart-net-worth',
      },
    });

    expect(getByTestId('chart-net-worth').getAttribute('aria-label')).toBe('純資産推移');
  });

  it('destroys the chart when unmounted', () => {
    const { unmount } = render(Chart, {
      props: {
        type: 'bar' as const,
        data: { labels: [], datasets: [] },
        ariaLabel: 'テスト',
      },
    });

    unmount();

    expect(destroy).toHaveBeenCalledTimes(1);
  });
});
