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
    render(Chart, {
      props: {
        type: 'bar' as const,
        data: { labels: ['a'], datasets: [{ label: 'x', data: [1] }] },
        ariaLabel: 'テスト',
      },
    });

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
