import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { CategoryReport } from '../../lib/api/reports';

// chart.js は jsdom に canvas コンテキストが無いので描画ごと差し替える。ChartStub は
// data を data-chart 属性に JSON で載せる (このテストが読みたいのは datasets の
// borderColor / backgroundColor)。
vi.mock('../../lib/components/Chart.svelte', async () => {
  const Stub = (await import('../../lib/components/__stubs__/ChartStub.svelte')).default;
  return { default: Stub };
});

const ByCategoryTab = (await import('./ByCategoryTab.svelte')).default;

type StubChartData = { datasets: { label?: string; borderColor?: string }[] };

function trendChartData(): StubChartData {
  const raw = screen.getByTestId('chart-category-trend').getAttribute('data-chart');
  if (!raw) throw new Error('chart-category-trend has no data-chart attribute');
  return JSON.parse(raw) as StubChartData;
}

/** jsdom normalizes an inline `background: #667eea` to `rgb(102, 126, 234)` on read;
 *  run the trend dataset's raw hex through the same CSSOM round-trip so both sides
 *  compare in the same representation. */
function normalizeColor(color: string): string {
  const probe = document.createElement('div');
  probe.style.background = color;
  return probe.style.background;
}

function legendSwatchColor(categoryId: number): string {
  const button = screen.getByTestId(`legend-category-${categoryId}`);
  const swatch = button.querySelector<HTMLElement>('.swatch');
  if (!swatch) throw new Error(`legend-category-${categoryId} has no swatch element`);
  return swatch.style.background;
}

// 支出2件・収入1件。selectedCategoryId が null のときの visibleSeries は上位5件の
// 支出全部 (ここでは食費・交通費の2件とも)。交通費 (category_id 3) を選ぶと
// visibleSeries が1件に絞られ、series 配列内での位置が [1件目] に変わる。
// ByCategoryTab.svelte の categoryColor マップ (category_id → 色) が配列の位置で
// 色を割り当てていた場合、この絞り込みで凡例と推移線の色がずれる。
const report: CategoryReport = {
  months: ['2026-04', '2026-05'],
  income: [{ category_id: 2, name: '給与', type: 'income', amount: 640_000 }],
  expense: [
    { category_id: 1, name: '食費', type: 'expense', amount: 281_000 },
    { category_id: 3, name: '交通費', type: 'expense', amount: 12_000 },
  ],
  series: [
    { category_id: 2, name: '給与', type: 'income', points: [320_000, 320_000] },
    { category_id: 1, name: '食費', type: 'expense', points: [132_400, 148_600] },
    { category_id: 3, name: '交通費', type: 'expense', points: [6_000, 6_000] },
  ],
};

// FIX 3 用: 軸 (months) が別期間になり、選んでいたカテゴリ (交通費, id 3) がもう
// 存在しない期間。選択を引きずると visibleSeries が空になり selectedName が "—" の
// まま取り残される（凡例からもそのボタンが消えるので外す手段が無い）。
const otherRangeReport: CategoryReport = {
  months: ['2025-01', '2025-02'],
  income: [],
  expense: [{ category_id: 5, name: '娯楽費', type: 'expense', amount: 4_000 }],
  series: [{ category_id: 5, name: '娯楽費', type: 'expense', points: [2_000, 2_000] }],
};

describe('ByCategoryTab', () => {
  it('resets the selected category when the report months axis changes', async () => {
    const { rerender } = render(ByCategoryTab, { props: { report, error: null } });

    screen.getByTestId('legend-category-3').click();
    await Promise.resolve();
    expect(screen.getByTestId('selected-category').textContent).toBe('交通費');

    // 期間 (プリセット) を変えて新しいレポートが届いた想定。
    await rerender({ report: otherRangeReport });

    expect(screen.getByTestId('selected-category').textContent).toBe('支出上位5カテゴリ');
  });

  it('keeps the selection when the same-range report is refetched (e.g. a data:changed reload)', async () => {
    const { rerender } = render(ByCategoryTab, { props: { report, error: null } });

    screen.getByTestId('legend-category-3').click();
    await Promise.resolve();
    expect(screen.getByTestId('selected-category').textContent).toBe('交通費');

    // 同じ months のまま中身の金額だけ変わって再取得された想定（新しいオブジェクト
    // 参照だが軸は同じで、選んでいたカテゴリ(3)もまだ存在する）。ユーザーの選択を
    // 裏で勝手に外してはいけない。
    await rerender({
      report: {
        ...report,
        expense: report.expense.map((a) =>
          a.category_id === 1 ? { ...a, amount: a.amount + 1 } : a,
        ),
      },
    });

    expect(screen.getByTestId('selected-category').textContent).toBe('交通費');
  });

  it('colors a category the same in the legend and the trend line, before and after narrowing', async () => {
    render(ByCategoryTab, { props: { report, error: null } });

    // 絞り込み前: 食費・交通費の両方が推移に出ている。凡例の色と一致すること。
    let trend = trendChartData();
    for (const [categoryId, name] of [
      [1, '食費'],
      [3, '交通費'],
    ] as const) {
      const dataset = trend.datasets.find((d) => d.label === name);
      expect(dataset).toBeDefined();
      expect(normalizeColor(dataset!.borderColor!)).toBe(legendSwatchColor(categoryId));
    }

    const beforeSelectionColor = legendSwatchColor(3);

    // 交通費だけに絞り込む。壊れた実装ではここで visibleSeries の配列位置がずれ、
    // 推移線の色が凡例の色と食い違う。
    screen.getByTestId('legend-category-3').click();
    await Promise.resolve();

    trend = trendChartData();
    expect(trend.datasets).toHaveLength(1);
    expect(trend.datasets[0]?.label).toBe('交通費');
    expect(normalizeColor(trend.datasets[0]!.borderColor!)).toBe(beforeSelectionColor);
    expect(normalizeColor(trend.datasets[0]!.borderColor!)).toBe(legendSwatchColor(3));
  });
});
