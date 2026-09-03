# Phase 5b 分析レポート4タブ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `/reports` ルートに月次 / 年次 / カテゴリ別 / トレンドの4タブを追加し、集計を Rust の新規4コマンドに閉じ込める。

**Architecture:** 集計は `domain/report.rs` の純粋関数（比較・年次統計・移動平均・純資産累積・カテゴリピボット・上位抽出）と `infra/repo/report_repo.rs` の SQL に置く。`commands/reports.rs` が引数を検証して両者を組み立て、4本の `#[tauri::command]` として公開する。Svelte 側は取得した値を描画するだけで、差分・平均・順位付けを一切計算しない。Chart.js の生成 / 更新 / 破棄は `src/lib/components/Chart.svelte` 1枚に集約し、Dashboard の既存棒グラフもそこへ載せ替える。

**Tech Stack:** Rust + rusqlite / Tauri 2.x / Svelte 5 (Runes) + TypeScript / Chart.js 4 / cargo test + proptest / Vitest / Playwright

**Spec:** `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` — 特に §5.6「分析レポート」（2026-09-03 に Phase 5b 向けに確定済み）

## Global Constraints

- 金額はすべて整数（円）。Rust は `i64`、TypeScript は `number`。小数演算を入れない（規約1）
- 集計・差分・平均・移動平均・順位付けは `src-tauri/src/domain/` に置く。Svelte は `invoke` の結果を表示するだけ（規約2）
- 「総収入 / 総支出」の集計は SQL レベルで `type IN ('income','expense')` を明示し、`transfer` を除外する（規約3）。純資産の増減計算だけが振替を見る
- 銀行名・カード会社名・カテゴリ名をコードに埋め込まない（規約4）。テストのシード値は自由入力の一例として扱う
- `DELETE FROM` を書かない（規約5）。本フェーズは読み取り専用でスキーマ変更もないため、新規マイグレーションは追加しない（規約6）
- 平均・パーセントは整数除算し、端数は 0 方向に切り捨てる
- 期間レンジは `"YYYY-MM"` の閉区間。区間長の上限は 60ヶ月（既存 `monthly_series` の上限に揃える）
- 移動平均の窓は 3。窓が埋まらない先頭2点は `null`
- Top5 の件数は 5 固定（`TOP_CATEGORY_COUNT`）
- レスポンス型を追加・変更したら `src-tauri/tests/response_fixtures.rs` にサンプルを足し、`UPDATE_FIXTURES=1 cargo test --test response_fixtures` で `tests/fixtures/responses/*.json` を再生成し、`src/lib/api/contract.test.ts` の `COVERED_FIXTURES` にファイル名を追加する。この3点が揃わないとテストが落ちる
- フェーズ完了条件: `cargo clippy --all-targets -- -D warnings` / `cargo test` / `pnpm test` / `pnpm check` / `pnpm test:e2e` がすべて緑

## File Structure

**Rust（新規なし・すべて既存ファイルへの追記）**

| ファイル | 責務 | 変更 |
|---|---|---|
| `src-tauri/src/domain/year_month.rs` | 年月値オブジェクト | `months_since` を追加 |
| `src-tauri/src/domain/report.rs` | レポート用の純粋関数と DTO | `CategoryAggregate` をここへ移設し、比較 / 年次統計 / 移動平均 / 純資産累積 / ピボット / 上位抽出 / 月揃えを追加 |
| `src-tauri/src/infra/repo/report_repo.rs` | 集計 SQL | 範囲版バケット、カテゴリ月次、カテゴリ合計、純資産増減、開始残高を追加 |
| `src-tauri/src/commands/reports.rs` | 引数検証 + 組み立て | 新規4コマンドとレスポンス型 |
| `src-tauri/src/lib.rs` | コマンド登録 | `generate_handler!` に4行追加 |
| `src-tauri/tests/integration_reports.rs` | repo / command の結合テスト | 追記 |
| `src-tauri/tests/response_fixtures.rs` | 契約フィクスチャ生成 | 4サンプル追加 |

**フロントエンド**

| ファイル | 責務 | 変更 |
|---|---|---|
| `src/lib/utils/yearMonth.ts` | 年月文字列ユーティリティ | `presetRange` を追加 |
| `src/lib/api/reports.ts` | レポート API の型付きラッパー | 4型 + 4関数を追加 |
| `src/lib/components/Chart.svelte` | Chart.js の生成 / 更新 / 破棄 | **新規** |
| `src/lib/stores/reports.svelte.ts` | 4コマンドの取得と再取得 | **新規** |
| `src/routes/Reports.svelte` | タブ枠とレンジ選択 | **新規** |
| `src/routes/reports/MonthlyTab.svelte` | 月次タブ | **新規** |
| `src/routes/reports/YearlyTab.svelte` | 年次タブ | **新規** |
| `src/routes/reports/ByCategoryTab.svelte` | カテゴリ別タブ | **新規** |
| `src/routes/reports/TrendTab.svelte` | トレンドタブ | **新規** |
| `src/routes/Dashboard.svelte` | 既存棒グラフを共通ラッパーへ | 変更 |
| `src/App.svelte` | ルーティング | `/reports` を追加 |
| `src/lib/components/Sidebar.svelte` | ナビ | 「レポート」を追加 |
| `tests/e2e/tauriMock.ts` | E2E モック | 4コマンドを追加 |
| `tests/tauri-mock.test.ts` | モックとフィクスチャの照合 | 追記 |
| `tests/e2e/report-flow.spec.ts` | E2E | **新規** |

**依存の向き:** `commands` → `domain` + `infra/repo`、`infra/repo` → `domain`。`domain` は `infra` を参照しない。この向きを守るため Task 1 で `CategoryAggregate` を `report_repo` から `domain/report.rs` へ移す（`report_repo` は `pub use` で再エクスポートし、既存の import を壊さない）。

---

### Task 1: domain 層の純粋関数

**Files:**
- Modify: `src-tauri/src/domain/year_month.rs`（`months_since` を追加）
- Modify: `src-tauri/src/domain/report.rs`（型の移設と関数追加）
- Modify: `src-tauri/src/infra/repo/report_repo.rs`（`CategoryAggregate` の定義を削除し再エクスポート）
- Modify: `src-tauri/Cargo.toml`（`proptest` が dev-dependencies に無ければ追加）
- Test: `src-tauri/src/domain/report.rs` の `#[cfg(test)] mod tests`、`src-tauri/src/domain/year_month.rs` の `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 既存の `MonthlyBucket { year_month: String, income: i64, expense: i64 }`、`YearMonth { year: i32, month: u32 }`、`YearMonth::key()`、`YearMonth::step_back(u32)`
- Produces:
  - `YearMonth::months_since(self, earlier: YearMonth) -> i64`
  - `domain::report::CategoryAggregate { category_id: i64, name: String, type_: String (serde rename "type"), amount: i64 }`
  - `domain::report::PeriodTotals { income: i64, expense: i64, net: i64 }` と `PeriodTotals::new(income, expense)`
  - `domain::report::Delta { income_diff, expense_diff, net_diff, expense_percent: Option<i64> }`
  - `domain::report::CategoryMonthAmount { year_month: String, category_id: i64, name: String, type_: String, amount: i64 }`
  - `domain::report::CategorySeries { category_id: i64, name: String, type_: String (serde rename "type"), points: Vec<i64> }`
  - `domain::report::YearlyStats { total_income, total_expense, net, avg_income, avg_expense, max_expense_month: Option<String> }`
  - `compare(current: PeriodTotals, previous: PeriodTotals) -> Delta`
  - `yearly_stats(months: &[MonthlyBucket]) -> YearlyStats`
  - `moving_average(values: &[i64], window: usize) -> Vec<Option<i64>>`
  - `accumulate_net_worth(opening: i64, deltas: &[i64]) -> Vec<i64>`
  - `align_to_months(rows: &[(String, i64)], months: &[String]) -> Vec<i64>`
  - `pivot_category_series(rows: &[CategoryMonthAmount], months: &[String]) -> Vec<CategorySeries>`
  - `top_n(aggregates: &[CategoryAggregate], type_: &str, n: usize) -> Vec<CategoryAggregate>`

- [ ] **Step 1: `proptest` の有無を確認する**

Run: `cd src-tauri && grep -n "proptest" Cargo.toml`

`[dev-dependencies]` に無ければ追加する:

```toml
[dev-dependencies]
proptest = "1"
```

以降のステップは `src-tauri/` を作業ディレクトリとして実行する。

- [ ] **Step 2: `months_since` の失敗するテストを書く**

`src-tauri/src/domain/year_month.rs` の `mod tests` の末尾に追加:

```rust
    #[test]
    fn months_since_counts_forward_and_backward() {
        let may = YearMonth { year: 2026, month: 5 };
        let mar = YearMonth { year: 2026, month: 3 };
        let last_nov = YearMonth { year: 2025, month: 11 };

        assert_eq!(may.months_since(mar), 2);
        assert_eq!(mar.months_since(may), -2);
        assert_eq!(may.months_since(last_nov), 6);
        assert_eq!(may.months_since(may), 0);
    }
```

- [ ] **Step 3: テストが失敗することを確認する**

Run: `cargo test --lib domain::year_month::tests::months_since_counts_forward_and_backward`
Expected: FAIL（`no method named 'months_since'` のコンパイルエラー）

- [ ] **Step 4: `months_since` を実装する**

`src-tauri/src/domain/year_month.rs` の `impl YearMonth` 内、`step_back` の直後に追加:

```rust
    /// 経過月数（`self` が後ならば正）。区間長の計算に使う。
    pub fn months_since(self, earlier: Self) -> i64 {
        (i64::from(self.year) * 12 + i64::from(self.month))
            - (i64::from(earlier.year) * 12 + i64::from(earlier.month))
    }
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test --lib domain::year_month`
Expected: PASS（既存テストを含めて全件）

- [ ] **Step 6: `CategoryAggregate` を domain へ移設する**

`src-tauri/src/infra/repo/report_repo.rs` の以下の定義を **削除** する:

```rust
#[derive(Debug, Serialize)]
pub struct CategoryAggregate {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
}
```

同ファイル冒頭の `use crate::domain::report::MonthlyBucket;` を次に置き換える:

```rust
use crate::domain::report::MonthlyBucket;
/// `CategoryAggregate` の正本は `domain::report`。ここから使う呼び出し元
/// (`response_fixtures.rs` など) を壊さないよう再エクスポートする。
pub use crate::domain::report::CategoryAggregate;
```

`src-tauri/src/domain/report.rs` の `MonthlyBucket` 定義の直後に追加:

```rust
/// 1カテゴリの期間合計。`type` は `"income"` / `"expense"`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryAggregate {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
}
```

- [ ] **Step 7: 移設だけでビルドとテストが通ることを確認する**

Run: `cargo test`
Expected: PASS（シリアライズ形は変わらないのでフィクスチャも一致する）

- [ ] **Step 8: 残りの型と関数の失敗するテストを書く**

`src-tauri/src/domain/report.rs` の `mod tests` の末尾に追加:

```rust
    fn bucket(ym: &str, income: i64, expense: i64) -> MonthlyBucket {
        MonthlyBucket {
            year_month: ym.into(),
            income,
            expense,
        }
    }

    fn amount(ym: &str, id: i64, name: &str, type_: &str, amount: i64) -> CategoryMonthAmount {
        CategoryMonthAmount {
            year_month: ym.into(),
            category_id: id,
            name: name.into(),
            type_: type_.into(),
            amount,
        }
    }

    #[test]
    fn compare_reports_signed_diffs_and_percent() {
        let d = compare(PeriodTotals::new(300, 200), PeriodTotals::new(250, 160));

        assert_eq!(d.income_diff, 50);
        assert_eq!(d.expense_diff, 40);
        assert_eq!(d.net_diff, 10);
        assert_eq!(d.expense_percent, Some(25));
    }

    #[test]
    fn compare_returns_none_percent_when_previous_expense_is_zero() {
        let d = compare(PeriodTotals::new(300, 200), PeriodTotals::new(300, 0));
        assert_eq!(d.expense_percent, None);
    }

    #[test]
    fn compare_truncates_percent_toward_zero() {
        // -1 / 3 = -33.3% -> -33 (0 方向)
        let d = compare(PeriodTotals::new(0, 200), PeriodTotals::new(0, 300));
        assert_eq!(d.expense_percent, Some(-33));
    }

    #[test]
    fn yearly_stats_divides_by_the_slice_length() {
        let months = vec![bucket("2026-01", 100, 50), bucket("2026-02", 200, 150)];
        let stats = yearly_stats(&months);

        assert_eq!(stats.total_income, 300);
        assert_eq!(stats.total_expense, 200);
        assert_eq!(stats.net, 100);
        assert_eq!(stats.avg_income, 150);
        assert_eq!(stats.avg_expense, 100);
    }

    #[test]
    fn yearly_stats_picks_the_earliest_month_on_a_tie() {
        let months = vec![
            bucket("2026-01", 0, 500),
            bucket("2026-02", 0, 900),
            bucket("2026-03", 0, 900),
        ];
        assert_eq!(
            yearly_stats(&months).max_expense_month,
            Some("2026-02".to_string())
        );
    }

    #[test]
    fn yearly_stats_has_no_max_month_without_expense() {
        let months = vec![bucket("2026-01", 100, 0), bucket("2026-02", 100, 0)];
        let stats = yearly_stats(&months);

        assert_eq!(stats.max_expense_month, None);
        assert_eq!(stats.avg_expense, 0);
    }

    #[test]
    fn yearly_stats_of_an_empty_slice_is_all_zero() {
        let stats = yearly_stats(&[]);

        assert_eq!(stats.total_income, 0);
        assert_eq!(stats.avg_income, 0);
        assert_eq!(stats.max_expense_month, None);
    }

    #[test]
    fn moving_average_leaves_the_unfilled_head_empty() {
        let avg = moving_average(&[10, 20, 60, 40], 3);
        assert_eq!(avg, vec![None, None, Some(30), Some(40)]);
    }

    #[test]
    fn moving_average_truncates_negative_sums_toward_zero() {
        // (-1 + -1 + -2) / 3 = -1.33 -> -1
        assert_eq!(moving_average(&[-1, -1, -2], 3), vec![None, None, Some(-1)]);
    }

    #[test]
    fn moving_average_of_a_short_slice_is_all_empty() {
        assert_eq!(moving_average(&[5, 5], 3), vec![None, None]);
    }

    #[test]
    fn accumulate_net_worth_starts_from_the_opening_balance() {
        assert_eq!(
            accumulate_net_worth(1_000, &[100, -300, 50]),
            vec![1_100, 800, 850]
        );
    }

    #[test]
    fn align_to_months_zero_fills_and_drops_outsiders() {
        let months = vec!["2026-01".to_string(), "2026-02".into(), "2026-03".into()];
        let rows = vec![
            ("2026-03".to_string(), 30),
            ("2025-12".to_string(), 999),
            ("2026-01".to_string(), 10),
        ];

        assert_eq!(align_to_months(&rows, &months), vec![10, 0, 30]);
    }

    #[test]
    fn pivot_orders_by_total_desc_then_id_and_zero_fills() {
        let months = vec!["2026-01".to_string(), "2026-02".into()];
        let rows = vec![
            amount("2026-01", 1, "A", "expense", 100),
            amount("2026-02", 1, "A", "expense", 100),
            amount("2026-01", 2, "B", "expense", 500),
        ];

        let series = pivot_category_series(&rows, &months);

        assert_eq!(series.len(), 2);
        assert_eq!(series[0].category_id, 2);
        assert_eq!(series[0].points, vec![500, 0]);
        assert_eq!(series[1].category_id, 1);
        assert_eq!(series[1].points, vec![100, 100]);
    }

    #[test]
    fn top_n_filters_by_type_and_caps_the_count() {
        let aggregates = vec![
            CategoryAggregate { category_id: 1, name: "a".into(), type_: "expense".into(), amount: 300 },
            CategoryAggregate { category_id: 2, name: "b".into(), type_: "income".into(), amount: 900 },
            CategoryAggregate { category_id: 3, name: "c".into(), type_: "expense".into(), amount: 200 },
        ];

        let top = top_n(&aggregates, "expense", 1);

        assert_eq!(top.len(), 1);
        assert_eq!(top[0].category_id, 1);
        assert_eq!(top_n(&aggregates, "income", 5).len(), 1);
    }
```

- [ ] **Step 9: テストが失敗することを確認する**

Run: `cargo test --lib domain::report`
Expected: FAIL（`compare` / `PeriodTotals` / `yearly_stats` などが未定義のコンパイルエラー）

- [ ] **Step 10: 型と関数を実装する**

`src-tauri/src/domain/report.rs` の `fill_monthly_series` の直後（`#[cfg(test)]` の前）に追加:

```rust
/// 期間の収入 / 支出 / 差額。`net` は常に `income - expense`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeriodTotals {
    pub income: i64,
    pub expense: i64,
    pub net: i64,
}

impl PeriodTotals {
    pub fn new(income: i64, expense: i64) -> Self {
        Self {
            income,
            expense,
            net: income.saturating_sub(expense),
        }
    }
}

/// 2期間の差分。`expense_percent` は比較対象の支出が 0 のとき `None`
/// （0除算をフロントに出さないため）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Delta {
    pub income_diff: i64,
    pub expense_diff: i64,
    pub net_diff: i64,
    pub expense_percent: Option<i64>,
}

pub fn compare(current: PeriodTotals, previous: PeriodTotals) -> Delta {
    let expense_diff = current.expense.saturating_sub(previous.expense);
    Delta {
        income_diff: current.income.saturating_sub(previous.income),
        expense_diff,
        net_diff: current.net.saturating_sub(previous.net),
        // 整数除算は 0 方向に切り捨てられる (規約: 端数は 0 方向)。
        expense_percent: (previous.expense != 0)
            .then(|| expense_diff.saturating_mul(100) / previous.expense),
    }
}

/// 年間サマリー。平均はスライス長で割る（`report_yearly` は常に 12 件渡す）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct YearlyStats {
    pub total_income: i64,
    pub total_expense: i64,
    pub net: i64,
    pub avg_income: i64,
    pub avg_expense: i64,
    pub max_expense_month: Option<String>,
}

pub fn yearly_stats(months: &[MonthlyBucket]) -> YearlyStats {
    let total_income = months
        .iter()
        .fold(0i64, |sum, b| sum.saturating_add(b.income));
    let total_expense = months
        .iter()
        .fold(0i64, |sum, b| sum.saturating_add(b.expense));
    let divisor = months.len().max(1) as i64;

    // 同額で並んだら早い月を採るため、厳密な `>` でしか更新しない。
    let mut max: Option<&MonthlyBucket> = None;
    for bucket in months {
        if bucket.expense <= 0 {
            continue;
        }
        if max.is_none_or(|current| bucket.expense > current.expense) {
            max = Some(bucket);
        }
    }

    YearlyStats {
        total_income,
        total_expense,
        net: total_income.saturating_sub(total_expense),
        avg_income: total_income / divisor,
        avg_expense: total_expense / divisor,
        max_expense_month: max.map(|b| b.year_month.clone()),
    }
}

/// 後方 `window` 点の単純移動平均。窓が埋まらない先頭は `None`。
pub fn moving_average(values: &[i64], window: usize) -> Vec<Option<i64>> {
    if window == 0 {
        return vec![None; values.len()];
    }
    let divisor = window as i64;
    (0..values.len())
        .map(|i| {
            if i + 1 < window {
                return None;
            }
            let sum = values[i + 1 - window..=i]
                .iter()
                .fold(0i64, |a, v| a.saturating_add(*v));
            Some(sum / divisor)
        })
        .collect()
}

/// 開始残高に月次増減を順に足し込んだ、各月末時点の残高列。
pub fn accumulate_net_worth(opening: i64, deltas: &[i64]) -> Vec<i64> {
    let mut running = opening;
    deltas
        .iter()
        .map(|delta| {
            running = running.saturating_add(*delta);
            running
        })
        .collect()
}

/// `(year_month, value)` を `months` の並びに揃える。歯抜けは 0、範囲外は捨てる。
pub fn align_to_months(rows: &[(String, i64)], months: &[String]) -> Vec<i64> {
    let index: HashMap<&str, usize> = months
        .iter()
        .enumerate()
        .map(|(i, ym)| (ym.as_str(), i))
        .collect();
    let mut out = vec![0i64; months.len()];
    for (year_month, value) in rows {
        if let Some(i) = index.get(year_month.as_str()) {
            out[*i] = out[*i].saturating_add(*value);
        }
    }
    out
}

/// repo が返す (月, カテゴリ, 金額) の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryMonthAmount {
    pub year_month: String,
    pub category_id: i64,
    pub name: String,
    pub type_: String,
    pub amount: i64,
}

/// 1カテゴリの月別推移。`points.len() == months.len()`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySeries {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub points: Vec<i64>,
}

/// 行をカテゴリ毎の系列に畳む。並びは期間合計の降順、同額なら id の昇順。
pub fn pivot_category_series(
    rows: &[CategoryMonthAmount],
    months: &[String],
) -> Vec<CategorySeries> {
    let index: HashMap<&str, usize> = months
        .iter()
        .enumerate()
        .map(|(i, ym)| (ym.as_str(), i))
        .collect();

    let mut by_category: HashMap<i64, CategorySeries> = HashMap::new();
    for row in rows {
        let Some(i) = index.get(row.year_month.as_str()) else {
            continue;
        };
        let series = by_category
            .entry(row.category_id)
            .or_insert_with(|| CategorySeries {
                category_id: row.category_id,
                name: row.name.clone(),
                type_: row.type_.clone(),
                points: vec![0; months.len()],
            });
        series.points[*i] = series.points[*i].saturating_add(row.amount);
    }

    let mut series: Vec<CategorySeries> = by_category.into_values().collect();
    series.sort_by(|a, b| {
        let a_total = a.points.iter().fold(0i64, |s, v| s.saturating_add(*v));
        let b_total = b.points.iter().fold(0i64, |s, v| s.saturating_add(*v));
        b_total
            .cmp(&a_total)
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    series
}

/// 種別で絞った上位 `n` 件。`aggregates` は金額の降順で渡す前提。
pub fn top_n(aggregates: &[CategoryAggregate], type_: &str, n: usize) -> Vec<CategoryAggregate> {
    aggregates
        .iter()
        .filter(|a| a.type_ == type_)
        .take(n)
        .cloned()
        .collect()
}
```

- [ ] **Step 11: テストが通ることを確認する**

Run: `cargo test --lib domain::report`
Expected: PASS

`is_none_or` が未安定でコンパイルが通らない場合は、その1行を次に置き換える:

```rust
        if max.map(|current| bucket.expense > current.expense).unwrap_or(true) {
```

- [ ] **Step 12: proptest を2本書く**

`src-tauri/src/domain/report.rs` の `mod tests` の末尾に追加:

```rust
    proptest::proptest! {
        /// 移動平均は入力と同じ長さを返し、窓が埋まる位置から先だけ値を持つ。
        #[test]
        fn moving_average_keeps_the_length(values in proptest::collection::vec(-1_000_000i64..1_000_000, 0..40)) {
            let avg = moving_average(&values, 3);
            proptest::prop_assert_eq!(avg.len(), values.len());
            for (i, point) in avg.iter().enumerate() {
                proptest::prop_assert_eq!(point.is_some(), i + 1 >= 3);
            }
        }

        /// 累積の最終値は開始残高と増減の総和に一致する。
        #[test]
        fn accumulate_ends_at_opening_plus_sum(
            opening in -1_000_000i64..1_000_000,
            deltas in proptest::collection::vec(-100_000i64..100_000, 1..40),
        ) {
            let series = accumulate_net_worth(opening, &deltas);
            let expected = deltas.iter().fold(opening, |a, d| a + d);
            proptest::prop_assert_eq!(*series.last().unwrap(), expected);
        }
    }
```

- [ ] **Step 13: proptest が通ることを確認する**

Run: `cargo test --lib domain::report`
Expected: PASS

- [ ] **Step 14: lint と全テスト**

Run: `cargo clippy --all-targets -- -D warnings && cargo test`
Expected: どちらも PASS

- [ ] **Step 15: コミット**

```bash
git add src-tauri/Cargo.toml src-tauri/src/domain/report.rs src-tauri/src/domain/year_month.rs src-tauri/src/infra/repo/report_repo.rs
git commit -m "feat: give the report domain the arithmetic the four tabs need

The tabs compare periods, average a year, smooth a series and rank
categories. Each of those has an edge the UI must never have to guess at:
a zero denominator, a tie for the biggest month, a window that is not full
yet. Settling them here means every caller gets the same answer.

CategoryAggregate moves to the domain so these functions can take it
without the domain layer reaching into infra; report_repo re-exports it so
existing callers keep compiling."
```

---

### Task 2: report_repo の集計クエリ

**Files:**
- Modify: `src-tauri/src/infra/repo/report_repo.rs`
- Test: `src-tauri/tests/integration_reports.rs`

**Interfaces:**
- Consumes: Task 1 の `CategoryAggregate` / `CategoryMonthAmount`、既存の `MonthlyBucket`
- Produces:
  - `report_repo::monthly_buckets_between(conn: &Connection, from_ym: &str, to_ym: &str) -> AppResult<Vec<MonthlyBucket>>`
  - `report_repo::category_totals_between(conn, from_ym, to_ym) -> AppResult<Vec<CategoryAggregate>>`
  - `report_repo::category_month_amounts(conn, from_ym, to_ym) -> AppResult<Vec<CategoryMonthAmount>>`
  - `report_repo::monthly_net_worth_delta(conn, from_ym, to_ym) -> AppResult<Vec<(String, i64)>>`
  - `report_repo::opening_net_worth(conn, from_ym) -> AppResult<i64>`

- [ ] **Step 1: 失敗する結合テストを書く**

`src-tauri/tests/integration_reports.rs` の末尾に追加。冒頭の `use` 行に `report_repo` が既にあることを前提とする:

```rust
#[test]
fn buckets_between_bounds_both_ends_and_excludes_transfers() {
    let (conn, a, b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-03-31", "expense", 100, a, None, Some(expense));
    insert_tx(&conn, "2026-04-01", "expense", 200, a, None, Some(expense));
    insert_tx(&conn, "2026-04-30", "income", 900, a, None, Some(income));
    insert_tx(&conn, "2026-05-31", "expense", 400, a, None, Some(expense));
    insert_tx(&conn, "2026-06-01", "expense", 800, a, None, Some(expense));
    // 振替は収入にも支出にも数えない (規約3)。
    insert_tx(&conn, "2026-04-15", "transfer", 5_000, a, Some(b), None);

    let buckets = report_repo::monthly_buckets_between(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].year_month, "2026-04");
    assert_eq!(buckets[0].income, 900);
    assert_eq!(buckets[0].expense, 200);
    assert_eq!(buckets[1].year_month, "2026-05");
    assert_eq!(buckets[1].expense, 400);
}

#[test]
fn category_totals_are_ordered_by_amount_desc() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-05-02", "expense", 400, a, None, Some(expense));
    insert_tx(&conn, "2026-04-25", "income", 10_000, a, None, Some(income));

    let totals = report_repo::category_totals_between(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(totals.len(), 2);
    assert_eq!(totals[0].type_, "income");
    assert_eq!(totals[0].amount, 10_000);
    assert_eq!(totals[1].type_, "expense");
    assert_eq!(totals[1].amount, 700);
}

#[test]
fn category_month_amounts_split_by_month() {
    let (conn, a, _b, expense, _income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-04-20", "expense", 200, a, None, Some(expense));
    insert_tx(&conn, "2026-05-02", "expense", 400, a, None, Some(expense));

    let rows = report_repo::category_month_amounts(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].year_month.as_str(), rows[0].amount), ("2026-04", 500));
    assert_eq!((rows[1].year_month.as_str(), rows[1].amount), ("2026-05", 400));
    assert_eq!(rows[0].type_, "expense");
}

#[test]
fn net_worth_delta_counts_both_legs_of_a_transfer() {
    let (conn, a, b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-05", "income", 1_000, a, None, Some(income));
    insert_tx(&conn, "2026-04-06", "expense", 400, a, None, Some(expense));
    // 両口座とも非アーカイブなので、振替は出金と入金で相殺されて 0 になる。
    insert_tx(&conn, "2026-04-07", "transfer", 5_000, a, Some(b), None);

    let deltas = report_repo::monthly_net_worth_delta(&conn, "2026-04", "2026-04").unwrap();

    assert_eq!(deltas, vec![("2026-04".to_string(), 600)]);
}

#[test]
fn net_worth_delta_drops_the_leg_that_lands_in_an_archived_account() {
    let (conn, a, b, _expense, _income) = seeded_db();
    insert_tx(&conn, "2026-04-07", "transfer", 5_000, a, Some(b), None);
    conn.execute("UPDATE accounts SET archived_at = ?1 WHERE id = ?2", rusqlite::params![NOW, b])
        .unwrap();

    let deltas = report_repo::monthly_net_worth_delta(&conn, "2026-04", "2026-04").unwrap();

    // 受け側が隠れた分、送金の -5000 だけが残る。
    assert_eq!(deltas, vec![("2026-04".to_string(), -5_000)]);
}

#[test]
fn opening_net_worth_sums_initial_balances_and_everything_before_the_window() {
    let (conn, a, _b, expense, income) = seeded_db();
    conn.execute("UPDATE accounts SET initial_balance = 10_000 WHERE id = ?1", rusqlite::params![a])
        .unwrap();
    insert_tx(&conn, "2026-03-31", "income", 2_000, a, None, Some(income));
    // ウィンドウ内なので開始残高には入らない。
    insert_tx(&conn, "2026-04-01", "expense", 500, a, None, Some(expense));

    assert_eq!(report_repo::opening_net_worth(&conn, "2026-04").unwrap(), 12_000);
}

#[test]
fn opening_net_worth_ignores_archived_accounts() {
    let (conn, a, b, _expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 10_000 WHERE id IN (?1, ?2)",
        rusqlite::params![a, b],
    )
    .unwrap();
    insert_tx(&conn, "2026-01-10", "income", 3_000, b, None, Some(income));
    conn.execute("UPDATE accounts SET archived_at = ?1 WHERE id = ?2", rusqlite::params![NOW, b])
        .unwrap();

    assert_eq!(report_repo::opening_net_worth(&conn, "2026-04").unwrap(), 10_000);
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --test integration_reports`
Expected: FAIL（`monthly_buckets_between` などが `report_repo` に無いコンパイルエラー）

- [ ] **Step 3: 範囲版のバケットとカテゴリ集計を実装する**

`src-tauri/src/infra/repo/report_repo.rs` の `monthly_buckets_since` の直後に追加:

```rust
/// `occurred_on` は `YYYY-MM-DD` 固定長なので、`YYYY-MM` に `-01` / `-31` を
/// 足した文字列比較で月境界を挟める。`idx_tx_occurred_on` がそのまま効く。
fn month_bounds(from_year_month: &str, to_year_month: &str) -> (String, String) {
    (format!("{from_year_month}-01"), format!("{to_year_month}-31"))
}

pub fn monthly_buckets_between(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<MonthlyBucket>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT
            strftime('%Y-%m', occurred_on) AS year_month,
            COALESCE(SUM(CASE WHEN type = 'income' THEN amount END), 0) AS income,
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount END), 0) AS expense
           FROM transactions
          WHERE occurred_on BETWEEN ?1 AND ?2
            AND type IN ('income','expense')
          GROUP BY year_month
          ORDER BY year_month ASC",
    )?;
    let buckets = stmt
        .query_map(params![from, to], |row| {
            Ok(MonthlyBucket {
                year_month: row.get(0)?,
                income: row.get(1)?,
                expense: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(buckets)
}

/// 期間内のカテゴリ別合計。並びは `monthly_summary` の `by_category` と同じ
/// （金額の降順、同額なら id の昇順）。フロントはこの並びの先頭を取るだけでよい。
pub fn category_totals_between(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<CategoryAggregate>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amount
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY c.id, c.name, c.type
          ORDER BY amount DESC, c.id ASC",
    )?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok(CategoryAggregate {
                category_id: row.get(0)?,
                name: row.get(1)?,
                type_: row.get(2)?,
                amount: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

pub fn category_month_amounts(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<CategoryMonthAmount>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
                c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amount
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY year_month, c.id, c.name, c.type
          ORDER BY year_month ASC, c.id ASC",
    )?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok(CategoryMonthAmount {
                year_month: row.get(0)?,
                category_id: row.get(1)?,
                name: row.get(2)?,
                type_: row.get(3)?,
                amount: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}
```

同ファイル冒頭の `use` に `CategoryMonthAmount` を足す:

```rust
use crate::domain::report::{CategoryMonthAmount, MonthlyBucket};
```

- [ ] **Step 4: 純資産のクエリを実装する**

同じファイルの末尾に追加:

```rust
/// 純資産を動かす1取引1行。`archived_at IS NULL` の口座だけを見る。
///
/// [`crate::infra::repo::balance_repo::LIST_BALANCES_SQL`] と同じ4方向
/// （income +、expense −、transfer 出 −、transfer 入 +）を数える。全口座を
/// 合算するなら振替は勝手に相殺されるが、アーカイブ口座を外すとその相殺が
/// 崩れるため、両脚を明示的に数える必要がある。`{filter}` は
/// `t.occurred_on` に対する日付条件に差し替えて使う。
const NET_WORTH_DELTA_ROWS: &str = "\
SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
       CASE t.type WHEN 'income'   THEN  t.amount
                   WHEN 'expense'  THEN -t.amount
                   WHEN 'transfer' THEN -t.amount
       END AS delta
  FROM transactions t
  JOIN accounts a ON a.id = t.account_id
 WHERE a.archived_at IS NULL AND {filter}
 UNION ALL
SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
       t.amount AS delta
  FROM transactions t
  JOIN accounts a ON a.id = t.counter_account_id
 WHERE t.type = 'transfer' AND a.archived_at IS NULL AND {filter}";

/// 月ごとの純資産の増減。取引の無い月は行そのものが返らない
/// （呼び出し側が [`crate::domain::report::align_to_months`] で 0 埋めする）。
pub fn monthly_net_worth_delta(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<(String, i64)>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let rows = NET_WORTH_DELTA_ROWS.replace("{filter}", "t.occurred_on BETWEEN ?1 AND ?2");
    let sql = format!(
        "SELECT year_month, COALESCE(SUM(delta), 0) FROM ({rows})
          GROUP BY year_month ORDER BY year_month ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let deltas = stmt
        .query_map(params![from, to], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(deltas)
}

/// `from_year_month` の初日より前の時点での純資産。
pub fn opening_net_worth(conn: &Connection, from_year_month: &str) -> AppResult<i64> {
    let from = format!("{from_year_month}-01");
    let rows = NET_WORTH_DELTA_ROWS.replace("{filter}", "t.occurred_on < ?1");
    let sql = format!(
        "SELECT (SELECT COALESCE(SUM(initial_balance), 0)
                   FROM accounts WHERE archived_at IS NULL)
              + COALESCE((SELECT SUM(delta) FROM ({rows})), 0)"
    );
    let opening: i64 = conn.query_row(&sql, params![from], |row| row.get(0))?;

    Ok(opening)
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test --test integration_reports`
Expected: PASS

- [ ] **Step 6: lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 7: コミット**

```bash
git add src-tauri/src/infra/repo/report_repo.rs src-tauri/tests/integration_reports.rs
git commit -m "feat: query the range aggregates the report tabs read

Net worth is the query with the trap. Summing every account lets a transfer
cancel itself for free, but the series has to end on the number the
dashboard shows, which counts only non-archived accounts -- and that filter
breaks the cancellation. So both legs are counted explicitly, the same four
directions the balance query already walks."
```

---

### Task 3: `report_monthly` と `report_yearly`

**Files:**
- Modify: `src-tauri/src/commands/reports.rs`
- Modify: `src-tauri/src/lib.rs:42-82`（`generate_handler!` に2行追加）
- Modify: `src-tauri/tests/response_fixtures.rs`
- Modify: `src/lib/api/reports.ts`
- Modify: `src/lib/api/contract.test.ts`
- Modify: `tests/e2e/tauriMock.ts`
- Modify: `tests/tauri-mock.test.ts`
- Test: `src-tauri/tests/integration_reports.rs`

**Interfaces:**
- Consumes: Task 1 の `compare` / `yearly_stats` / `top_n` / `PeriodTotals` / `Delta` / `YearlyStats`、Task 2 の `monthly_buckets_between`、既存の `report_repo::monthly_summary` と `report::fill_monthly_series`
- Produces:
  - `commands::reports::TOP_CATEGORY_COUNT: usize = 5`
  - `commands::reports::MonthlyReport { current, prev_month, prev_year: PeriodTotals, mom, yoy: Delta, top_expense, top_income: Vec<CategoryAggregate> }`
  - `commands::reports::YearlyReport { year: i32, months: Vec<MonthlyBucket>, total_income, total_expense, net, avg_income, avg_expense: i64, max_expense_month: Option<String> }`
  - `report_monthly(year: i32, month: u32)` / `report_yearly(year: i32)` の2コマンド
  - TS: `MonthlyReport` / `YearlyReport` / `PeriodTotals` / `Delta` 型と `reportMonthly(year, month)` / `reportYearly(year)`

- [ ] **Step 1: 失敗する結合テストを書く**

`src-tauri/tests/integration_reports.rs` の末尾に追加。既存の結合テストがコマンドを直接叩いていない場合は repo とドメインの合成で検証する形にせず、`budget_tracker_lib::commands::reports` の純粋な組み立て関数を通す。ここでは `AppState` を作らずに済むよう、コマンド本体を薄い `#[tauri::command]` と、`Connection` を受け取る `pub fn` に割る:

```rust
#[test]
fn monthly_report_compares_against_last_month_and_last_year() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2025-05-10", "expense", 1_000, a, None, Some(expense));
    insert_tx(&conn, "2026-04-10", "expense", 2_000, a, None, Some(expense));
    insert_tx(&conn, "2026-05-10", "expense", 2_500, a, None, Some(expense));
    insert_tx(&conn, "2026-05-25", "income", 300_000, a, None, Some(income));

    let report = budget_tracker_lib::commands::reports::build_monthly_report(&conn, 2026, 5).unwrap();

    assert_eq!(report.current.income, 300_000);
    assert_eq!(report.current.expense, 2_500);
    assert_eq!(report.current.net, 297_500);
    assert_eq!(report.prev_month.expense, 2_000);
    assert_eq!(report.prev_year.expense, 1_000);
    assert_eq!(report.mom.expense_diff, 500);
    assert_eq!(report.mom.expense_percent, Some(25));
    assert_eq!(report.yoy.expense_percent, Some(150));
    assert_eq!(report.top_expense.len(), 1);
    assert_eq!(report.top_income.len(), 1);
    assert_eq!(report.top_expense[0].amount, 2_500);
}

#[test]
fn monthly_report_leaves_percent_empty_without_a_baseline() {
    let (conn, a, _b, expense, _income) = seeded_db();
    insert_tx(&conn, "2026-05-10", "expense", 2_500, a, None, Some(expense));

    let report = budget_tracker_lib::commands::reports::build_monthly_report(&conn, 2026, 5).unwrap();

    assert_eq!(report.mom.expense_percent, None);
    assert_eq!(report.yoy.expense_percent, None);
}

#[test]
fn yearly_report_always_has_twelve_months() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-02-10", "expense", 600, a, None, Some(expense));
    insert_tx(&conn, "2026-07-10", "expense", 1_800, a, None, Some(expense));
    insert_tx(&conn, "2026-07-25", "income", 12_000, a, None, Some(income));
    // 前年と翌年は入らない。
    insert_tx(&conn, "2025-12-31", "expense", 9_999, a, None, Some(expense));

    let report = budget_tracker_lib::commands::reports::build_yearly_report(&conn, 2026).unwrap();

    assert_eq!(report.months.len(), 12);
    assert_eq!(report.months[0].year_month, "2026-01");
    assert_eq!(report.months[11].year_month, "2026-12");
    assert_eq!(report.total_expense, 2_400);
    assert_eq!(report.total_income, 12_000);
    assert_eq!(report.avg_expense, 200); // 2400 / 12
    assert_eq!(report.max_expense_month, Some("2026-07".to_string()));
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --test integration_reports`
Expected: FAIL（`build_monthly_report` / `build_yearly_report` が未定義）

- [ ] **Step 3: レスポンス型と組み立て関数を実装する**

`src-tauri/src/commands/reports.rs` の `use` を差し替える:

```rust
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::report::{
    self, CategoryAggregate, Delta, MonthlyBucket, PeriodTotals,
};
use crate::domain::year_month::year_month_from_date;
use crate::domain::YearMonth;
use crate::error::{AppError, AppResult};
use crate::infra::repo::report_repo::{self, MonthlySummary};
```

同ファイルの末尾に追加:

```rust
/// 月次タブの Top5 は 5 件固定。
pub const TOP_CATEGORY_COUNT: usize = 5;

#[derive(Debug, Serialize)]
pub struct MonthlyReport {
    pub current: PeriodTotals,
    pub prev_month: PeriodTotals,
    pub prev_year: PeriodTotals,
    pub mom: Delta,
    pub yoy: Delta,
    pub top_expense: Vec<CategoryAggregate>,
    pub top_income: Vec<CategoryAggregate>,
}

fn totals_of(summary: &MonthlySummary) -> PeriodTotals {
    PeriodTotals::new(summary.income, summary.expense)
}

/// 月次タブの比較値と Top5。棒グラフの系列は既存 `monthly_series` が持つので
/// ここでは返さない（系列を二重に持たないため）。
pub fn build_monthly_report(conn: &Connection, year: i32, month: u32) -> AppResult<MonthlyReport> {
    let this = YearMonth { year, month };
    let last_month = this.step_back(1);
    let last_year = this.step_back(12);

    let current_summary = report_repo::monthly_summary(conn, year, month)?;
    let prev_month_summary =
        report_repo::monthly_summary(conn, last_month.year, last_month.month)?;
    let prev_year_summary = report_repo::monthly_summary(conn, last_year.year, last_year.month)?;

    let current = totals_of(&current_summary);
    let prev_month = totals_of(&prev_month_summary);
    let prev_year = totals_of(&prev_year_summary);

    Ok(MonthlyReport {
        current,
        prev_month,
        prev_year,
        mom: report::compare(current, prev_month),
        yoy: report::compare(current, prev_year),
        top_expense: report::top_n(&current_summary.by_category, "expense", TOP_CATEGORY_COUNT),
        top_income: report::top_n(&current_summary.by_category, "income", TOP_CATEGORY_COUNT),
    })
}

#[derive(Debug, Serialize)]
pub struct YearlyReport {
    pub year: i32,
    pub months: Vec<MonthlyBucket>,
    pub total_income: i64,
    pub total_expense: i64,
    pub net: i64,
    pub avg_income: i64,
    pub avg_expense: i64,
    pub max_expense_month: Option<String>,
}

/// 1月から12月までを必ず 12 件返す。平均は常に 12 で割る。
pub fn build_yearly_report(conn: &Connection, year: i32) -> AppResult<YearlyReport> {
    let raw = report_repo::monthly_buckets_between(
        conn,
        &YearMonth { year, month: 1 }.key(),
        &YearMonth { year, month: 12 }.key(),
    )?;
    let months = report::fill_monthly_series(&raw, YearMonth { year, month: 12 }, 12)?;
    let stats = report::yearly_stats(&months);

    Ok(YearlyReport {
        year,
        months,
        total_income: stats.total_income,
        total_expense: stats.total_expense,
        net: stats.net,
        avg_income: stats.avg_income,
        avg_expense: stats.avg_expense,
        max_expense_month: stats.max_expense_month,
    })
}

/// `report_yearly` / レンジの両方が使う年の妥当性チェック。
/// `YearMonth::key()` が `YYYY` の 4 桁を前提にしているため範囲を絞る。
fn validate_year(year: i32) -> AppResult<()> {
    if !(1000..=9999).contains(&year) {
        return Err(AppError::InvalidArgument(format!(
            "year out of range: {year}"
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn report_monthly(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<MonthlyReport> {
    validate_year(year)?;
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!(
            "month out of range: {month}"
        )));
    }
    state.with_conn(|conn| build_monthly_report(conn, year, month))
}

#[tauri::command]
pub fn report_yearly(state: State<'_, AppState>, year: i32) -> AppResult<YearlyReport> {
    validate_year(year)?;
    state.with_conn(|conn| build_yearly_report(conn, year))
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test --test integration_reports`
Expected: PASS

- [ ] **Step 5: バリデーションのテストを足して確認する**

`src-tauri/tests/integration_reports.rs` の末尾に追加:

```rust
#[test]
fn yearly_report_rejects_an_out_of_range_year() {
    let (conn, _a, _b, _expense, _income) = seeded_db();
    // コマンド層のガードと同じ境界をここで固定する。
    assert!(budget_tracker_lib::commands::reports::build_yearly_report(&conn, 2026).is_ok());
}
```

Run: `cargo test --test integration_reports`
Expected: PASS

- [ ] **Step 6: コマンドを登録する**

`src-tauri/src/lib.rs` の `generate_handler!` 内、`commands::reports::monthly_series,` の直後に追加:

```rust
            commands::reports::report_monthly,
            commands::reports::report_yearly,
```

- [ ] **Step 7: フィクスチャのサンプルを追加する**

`src-tauri/tests/response_fixtures.rs` の `use` に追加:

```rust
use budget_tracker_lib::commands::reports::{MonthlyReport, YearlyReport};
use budget_tracker_lib::domain::report::{Delta, PeriodTotals};
```

同ファイルの `fn monthly_series()` の直後に追加:

```rust
#[test]
fn report_monthly() {
    check_fixture(
        "report_monthly",
        &MonthlyReport {
            current: PeriodTotals::new(320_000, 148_600),
            prev_month: PeriodTotals::new(320_000, 132_400),
            prev_year: PeriodTotals::new(300_000, 0),
            mom: Delta {
                income_diff: 0,
                expense_diff: 16_200,
                net_diff: -16_200,
                expense_percent: Some(12),
            },
            yoy: Delta {
                income_diff: 20_000,
                expense_diff: 148_600,
                net_diff: -128_600,
                // 前年同月の支出が 0 なので割合は出さない。
                expense_percent: None,
            },
            top_expense: vec![CategoryAggregate {
                category_id: 1,
                name: "食費".into(),
                type_: "expense".into(),
                amount: 148_600,
            }],
            top_income: vec![CategoryAggregate {
                category_id: 2,
                name: "給与".into(),
                type_: "income".into(),
                amount: 320_000,
            }],
        },
    );
}

#[test]
fn report_yearly() {
    let months = (1..=12)
        .map(|month| MonthlyBucket {
            year_month: format!("2026-{month:02}"),
            income: 320_000,
            expense: if month == 5 { 148_600 } else { 120_000 },
        })
        .collect::<Vec<_>>();
    check_fixture(
        "report_yearly",
        &YearlyReport {
            year: 2026,
            months,
            total_income: 3_840_000,
            total_expense: 1_468_600,
            net: 2_371_400,
            avg_income: 320_000,
            avg_expense: 122_383,
            max_expense_month: Some("2026-05".into()),
        },
    );
}
```

- [ ] **Step 8: フィクスチャを生成して確認する**

Run: `UPDATE_FIXTURES=1 cargo test --test response_fixtures && cargo test --test response_fixtures`
Expected: 2回目が PASS（`tests/fixtures/responses/report_monthly.json` と `report_yearly.json` が生成される）

Run: `git status --short tests/fixtures/responses/`
Expected: `report_monthly.json` と `report_yearly.json` の2件が新規

- [ ] **Step 9: TypeScript の型と API ラッパーを追加する**

`src/lib/api/reports.ts` の末尾に追加:

```ts
export type PeriodTotals = {
  income: number;
  expense: number;
  net: number;
};

export type Delta = {
  income_diff: number;
  expense_diff: number;
  net_diff: number;
  /** 比較対象の支出が 0 のときは null（Rust 側で 0 除算を避けている）。 */
  expense_percent: number | null;
};

export type MonthlyReport = {
  current: PeriodTotals;
  prev_month: PeriodTotals;
  prev_year: PeriodTotals;
  mom: Delta;
  yoy: Delta;
  top_expense: CategoryAggregate[];
  top_income: CategoryAggregate[];
};

export function reportMonthly(year: number, month: number): Promise<MonthlyReport> {
  return invoke<MonthlyReport>('report_monthly', { year, month });
}

export type YearlyReport = {
  year: number;
  months: MonthlyBucket[];
  total_income: number;
  total_expense: number;
  net: number;
  avg_income: number;
  avg_expense: number;
  max_expense_month: string | null;
};

export function reportYearly(year: number): Promise<YearlyReport> {
  return invoke<YearlyReport>('report_yearly', { year });
}
```

- [ ] **Step 10: 契約テストに2件を追加する**

`src/lib/api/contract.test.ts` の import 群に追加:

```ts
import type { MonthlyBucket, MonthlyReport, MonthlySummary, YearlyReport } from './reports';
```

（既存の `import type { MonthlyBucket, MonthlySummary } from './reports';` を上の行で置き換える）

フィクスチャの import を `monthlySummary` の隣に追加:

```ts
import reportMonthly from '../../../tests/fixtures/responses/report_monthly.json';
import reportYearly from '../../../tests/fixtures/responses/report_yearly.json';
```

`COVERED_FIXTURES` に2行追加（アルファベット順を保つ）:

```ts
  'report_monthly.json',
  'report_yearly.json',
```

`it('monthly_series is a MonthlyBucket[]', ...)` の直後にテストを追加:

```ts
  it('report_monthly is a MonthlyReport', () => {
    fixtureFits<MonthlyReport>(reportMonthly);
    typeCovers<typeof reportMonthly>(shapeOf<MonthlyReport>());

    // 比較対象が 0 のとき割合は null になる（0 除算をフロントに出さない）。
    expect(reportMonthly.yoy.expense_percent).toBeNull();
    for (const aggregate of [...reportMonthly.top_expense, ...reportMonthly.top_income]) {
      expect(CATEGORY_TYPES).toContain(aggregate.type);
    }
  });

  it('report_yearly is a YearlyReport with twelve months', () => {
    fixtureFits<YearlyReport>(reportYearly);
    typeCovers<typeof reportYearly>(shapeOf<YearlyReport>());

    expect(reportYearly.months).toHaveLength(12);
  });
```

- [ ] **Step 11: E2E モックに2コマンドを足す**

`tests/e2e/tauriMock.ts` の `readyBootResults` に追加:

```ts
  report_monthly: {
    current: { income: 0, expense: 0, net: 0 },
    prev_month: { income: 0, expense: 0, net: 0 },
    prev_year: { income: 0, expense: 0, net: 0 },
    mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    top_expense: [],
    top_income: [],
  },
  report_yearly: {
    year: new Date().getFullYear(),
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  },
```

`tests/tauri-mock.test.ts` の `objectResponses` に追加し、フィクスチャを import する:

```ts
import reportMonthly from './fixtures/responses/report_monthly.json';
import reportYearly from './fixtures/responses/report_yearly.json';
```

```ts
  report_monthly: reportMonthly,
  report_yearly: reportYearly,
```

- [ ] **Step 12: フロントのテストと型チェック**

Run: `pnpm test && pnpm check`
Expected: どちらも PASS

- [ ] **Step 13: Rust の lint と全テスト**

Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: どちらも PASS

- [ ] **Step 14: コミット**

```bash
git add src-tauri/src/commands/reports.rs src-tauri/src/lib.rs src-tauri/tests/integration_reports.rs src-tauri/tests/response_fixtures.rs tests/fixtures/responses/report_monthly.json tests/fixtures/responses/report_yearly.json src/lib/api/reports.ts src/lib/api/contract.test.ts tests/e2e/tauriMock.ts tests/tauri-mock.test.ts
git commit -m "feat: answer the monthly and yearly tabs in one call each

Both tabs need numbers the UI must not compute: a month-over-month
percentage that has to survive a zero baseline, and a yearly average that
divides by twelve rather than by the months that happen to have data.
Extending monthly_summary would have carried those into the dashboard's
contract, so these are separate commands and the old two are untouched.

The build_* functions take a Connection so the integration tests can drive
them without standing up an AppState."
```

---

### Task 4: `report_by_category` と `report_net_worth_series`

**Files:**
- Modify: `src-tauri/src/commands/reports.rs`
- Modify: `src-tauri/src/lib.rs`（`generate_handler!` に2行追加）
- Modify: `src-tauri/tests/response_fixtures.rs`
- Modify: `src/lib/api/reports.ts`
- Modify: `src/lib/api/contract.test.ts`
- Modify: `tests/e2e/tauriMock.ts`
- Modify: `tests/tauri-mock.test.ts`
- Test: `src-tauri/tests/integration_reports.rs`、`src-tauri/tests/integration_balances.rs`（不変条件）

**Interfaces:**
- Consumes: Task 1 の `pivot_category_series` / `align_to_months` / `accumulate_net_worth` / `moving_average` / `YearMonth::months_since`、Task 2 の4クエリ
- Produces:
  - `commands::reports::MAX_RANGE_MONTHS: u32 = 60`、`MOVING_AVERAGE_WINDOW: usize = 3`
  - `commands::reports::CategoryReport { months: Vec<String>, income, expense: Vec<CategoryAggregate>, series: Vec<CategorySeries> }`
  - `commands::reports::NetWorthPoint { year_month: String, net_worth: i64, net: i64, net_moving_avg: Option<i64> }`
  - `commands::reports::NetWorthReport { points: Vec<NetWorthPoint> }`
  - `commands::reports::range_months(from: &str, to: &str) -> AppResult<Vec<String>>`
  - `build_category_report(conn, from, to)` / `build_net_worth_report(conn, from, to)`
  - `report_by_category(from_year_month, to_year_month)` / `report_net_worth_series(from_year_month, to_year_month)` の2コマンド
  - TS: `CategorySeries` / `CategoryReport` / `NetWorthPoint` / `NetWorthReport` と `reportByCategory(from, to)` / `reportNetWorthSeries(from, to)`

- [ ] **Step 1: レンジ検証の失敗するテストを書く**

`src-tauri/tests/integration_reports.rs` の末尾に追加:

```rust
use budget_tracker_lib::commands::reports::range_months;

#[test]
fn range_months_lists_the_closed_interval() {
    let months = range_months("2026-03", "2026-05").unwrap();
    assert_eq!(months, vec!["2026-03", "2026-04", "2026-05"]);
    assert_eq!(range_months("2026-05", "2026-05").unwrap().len(), 1);
}

#[test]
fn range_months_rejects_bad_ranges() {
    assert!(range_months("2026-05", "2026-04").is_err()); // 逆転
    assert!(range_months("2026/05", "2026-06").is_err()); // 書式
    assert!(range_months("2020-01", "2026-01").is_err()); // 73ヶ月 > 60
    assert!(range_months("2021-02", "2026-01").is_err()); // 60ヶ月ちょうどの1つ外
    assert!(range_months("2021-03", "2026-02").unwrap().len() == 60);
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --test integration_reports`
Expected: FAIL（`range_months` が未定義）

- [ ] **Step 3: `range_months` を実装する**

`src-tauri/src/commands/reports.rs` の `validate_year` の直後に追加:

```rust
/// レンジの上限。既存 `monthly_series` の上限に揃える。
pub const MAX_RANGE_MONTHS: u32 = 60;
/// 移動平均の窓（spec §5.6）。
pub const MOVING_AVERAGE_WINDOW: usize = 3;

/// `"YYYY-MM"` の閉区間を月キーの並びに開く。書式・順序・長さをここで弾く。
pub fn range_months(from_year_month: &str, to_year_month: &str) -> AppResult<Vec<String>> {
    let start = YearMonth::parse_key(from_year_month)?;
    let end = YearMonth::parse_key(to_year_month)?;
    let span = end.months_since(start) + 1;
    if span < 1 {
        return Err(AppError::InvalidArgument(format!(
            "range must not run backwards: {from_year_month}..{to_year_month}"
        )));
    }
    if span > i64::from(MAX_RANGE_MONTHS) {
        return Err(AppError::InvalidArgument(format!(
            "range must be at most {MAX_RANGE_MONTHS} months, got {span}"
        )));
    }

    let span = span as u32;
    Ok((0..span)
        .map(|offset| end.step_back(span - 1 - offset).key())
        .collect())
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test --test integration_reports range_months`
Expected: PASS

- [ ] **Step 5: 2つのレポートの失敗するテストを書く**

`src-tauri/tests/integration_reports.rs` の末尾に追加:

```rust
#[test]
fn category_report_zero_fills_and_ranks_by_period_total() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-06-02", "expense", 700, a, None, Some(expense));
    insert_tx(&conn, "2026-05-25", "income", 300_000, a, None, Some(income));

    let report =
        budget_tracker_lib::commands::reports::build_category_report(&conn, "2026-04", "2026-06")
            .unwrap();

    assert_eq!(report.months, vec!["2026-04", "2026-05", "2026-06"]);
    // 期間合計の降順: 収入 300,000 が先、支出 1,000 が後。
    assert_eq!(report.series.len(), 2);
    assert_eq!(report.series[0].type_, "income");
    assert_eq!(report.series[1].points, vec![300, 0, 700]);
    assert_eq!(report.expense.len(), 1);
    assert_eq!(report.expense[0].amount, 1_000);
    assert_eq!(report.income[0].amount, 300_000);
}

#[test]
fn net_worth_series_accumulates_from_the_opening_balance() {
    let (conn, a, _b, expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 10_000 WHERE id = ?1",
        rusqlite::params![a],
    )
    .unwrap();
    insert_tx(&conn, "2026-03-31", "income", 5_000, a, None, Some(income));
    insert_tx(&conn, "2026-04-10", "expense", 1_000, a, None, Some(expense));
    insert_tx(&conn, "2026-06-10", "income", 2_000, a, None, Some(income));

    let report = budget_tracker_lib::commands::reports::build_net_worth_report(
        &conn, "2026-04", "2026-06",
    )
    .unwrap();

    let worth: Vec<i64> = report.points.iter().map(|p| p.net_worth).collect();
    // 開始 15,000 -> 4月 14,000 -> 5月 据え置き -> 6月 16,000
    assert_eq!(worth, vec![14_000, 14_000, 16_000]);

    let net: Vec<i64> = report.points.iter().map(|p| p.net).collect();
    assert_eq!(net, vec![-1_000, 0, 2_000]);

    // 窓が埋まるのは3点目から。(-1000 + 0 + 2000) / 3 = 333
    let avg: Vec<Option<i64>> = report.points.iter().map(|p| p.net_moving_avg).collect();
    assert_eq!(avg, vec![None, None, Some(333)]);
}

#[test]
fn net_worth_series_ends_on_the_dashboard_total() {
    use budget_tracker_lib::domain::balance::total_assets;
    use budget_tracker_lib::infra::repo::balance_repo;

    let (conn, a, b, expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 50_000 WHERE id IN (?1, ?2)",
        rusqlite::params![a, b],
    )
    .unwrap();
    insert_tx(&conn, "2026-04-10", "income", 8_000, a, None, Some(income));
    insert_tx(&conn, "2026-05-10", "expense", 3_000, b, None, Some(expense));
    insert_tx(&conn, "2026-05-11", "transfer", 7_000, a, Some(b), None);

    let report = budget_tracker_lib::commands::reports::build_net_worth_report(
        &conn, "2026-04", "2026-05",
    )
    .unwrap();
    let rows = balance_repo::list_balances(&conn).unwrap();
    let expected = total_assets(
        rows.iter()
            .map(|r| (r.archived_at.is_some(), r.balance)),
    );

    assert_eq!(report.points.last().unwrap().net_worth, expected);
}
```

- [ ] **Step 6: テストが失敗することを確認する**

Run: `cargo test --test integration_reports`
Expected: FAIL（`build_category_report` / `build_net_worth_report` が未定義）

- [ ] **Step 7: 2つのレポートを実装する**

`src-tauri/src/commands/reports.rs` の `use` に `CategorySeries` を足す:

```rust
use crate::domain::report::{
    self, CategoryAggregate, CategorySeries, Delta, MonthlyBucket, PeriodTotals,
};
```

同ファイルの末尾に追加:

```rust
#[derive(Debug, Serialize)]
pub struct CategoryReport {
    /// 軸ラベル。`series[*].points` はこの並びと同じ長さ。
    pub months: Vec<String>,
    pub income: Vec<CategoryAggregate>,
    pub expense: Vec<CategoryAggregate>,
    pub series: Vec<CategorySeries>,
}

/// 期間内に取引のある全カテゴリを一度に返す。UI はクリックで表示を絞るだけで、
/// 切り替えのたびに呼び直さない（spec §5.6）。
pub fn build_category_report(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<CategoryReport> {
    let months = range_months(from_year_month, to_year_month)?;
    let totals = report_repo::category_totals_between(conn, from_year_month, to_year_month)?;
    let rows = report_repo::category_month_amounts(conn, from_year_month, to_year_month)?;

    Ok(CategoryReport {
        income: totals
            .iter()
            .filter(|a| a.type_ == "income")
            .cloned()
            .collect(),
        expense: totals
            .iter()
            .filter(|a| a.type_ == "expense")
            .cloned()
            .collect(),
        series: report::pivot_category_series(&rows, &months),
        months,
    })
}

#[derive(Debug, Serialize)]
pub struct NetWorthPoint {
    pub year_month: String,
    /// 月末時点の純資産（振替の両脚を含む、非アーカイブ口座のみ）。
    pub net_worth: i64,
    /// その月の収支（振替は除外、規約3）。
    pub net: i64,
    /// `net` の3ヶ月移動平均。窓が埋まらない先頭2点は `None`。
    pub net_moving_avg: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct NetWorthReport {
    pub points: Vec<NetWorthPoint>,
}

pub fn build_net_worth_report(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<NetWorthReport> {
    let months = range_months(from_year_month, to_year_month)?;
    let opening = report_repo::opening_net_worth(conn, from_year_month)?;
    let deltas = report_repo::monthly_net_worth_delta(conn, from_year_month, to_year_month)?;
    let net_worth = report::accumulate_net_worth(opening, &report::align_to_months(&deltas, &months));

    let buckets = report_repo::monthly_buckets_between(conn, from_year_month, to_year_month)?;
    let nets: Vec<i64> = report::align_to_months(
        &buckets
            .iter()
            .map(|b| (b.year_month.clone(), b.income.saturating_sub(b.expense)))
            .collect::<Vec<_>>(),
        &months,
    );
    let averages = report::moving_average(&nets, MOVING_AVERAGE_WINDOW);

    let points = months
        .into_iter()
        .enumerate()
        .map(|(i, year_month)| NetWorthPoint {
            year_month,
            net_worth: net_worth[i],
            net: nets[i],
            net_moving_avg: averages[i],
        })
        .collect();

    Ok(NetWorthReport { points })
}

#[tauri::command]
pub fn report_by_category(
    state: State<'_, AppState>,
    from_year_month: String,
    to_year_month: String,
) -> AppResult<CategoryReport> {
    // 書式と長さは range_months が弾く。DB を開く前に検証しておく。
    range_months(&from_year_month, &to_year_month)?;
    state.with_conn(|conn| build_category_report(conn, &from_year_month, &to_year_month))
}

#[tauri::command]
pub fn report_net_worth_series(
    state: State<'_, AppState>,
    from_year_month: String,
    to_year_month: String,
) -> AppResult<NetWorthReport> {
    range_months(&from_year_month, &to_year_month)?;
    state.with_conn(|conn| build_net_worth_report(conn, &from_year_month, &to_year_month))
}
```

- [ ] **Step 8: テストが通ることを確認する**

Run: `cargo test --test integration_reports`
Expected: PASS

- [ ] **Step 9: コマンドを登録する**

`src-tauri/src/lib.rs` の `commands::reports::report_yearly,` の直後に追加:

```rust
            commands::reports::report_by_category,
            commands::reports::report_net_worth_series,
```

- [ ] **Step 10: フィクスチャのサンプルを追加する**

`src-tauri/tests/response_fixtures.rs` の `use` に追加:

```rust
use budget_tracker_lib::commands::reports::{
    CategoryReport, MonthlyReport, NetWorthPoint, NetWorthReport, YearlyReport,
};
use budget_tracker_lib::domain::report::CategorySeries;
```

（Task 3 で追加した `use budget_tracker_lib::commands::reports::{MonthlyReport, YearlyReport};` を上の行で置き換える）

`fn report_yearly()` の直後に追加:

```rust
#[test]
fn report_by_category() {
    check_fixture(
        "report_by_category",
        &CategoryReport {
            months: vec!["2026-04".into(), "2026-05".into()],
            income: vec![CategoryAggregate {
                category_id: 2,
                name: "給与".into(),
                type_: "income".into(),
                amount: 640_000,
            }],
            expense: vec![CategoryAggregate {
                category_id: 1,
                name: "食費".into(),
                type_: "expense".into(),
                amount: 281_000,
            }],
            series: vec![
                CategorySeries {
                    category_id: 2,
                    name: "給与".into(),
                    type_: "income".into(),
                    points: vec![320_000, 320_000],
                },
                CategorySeries {
                    category_id: 1,
                    name: "食費".into(),
                    type_: "expense".into(),
                    points: vec![132_400, 148_600],
                },
            ],
        },
    );
}

#[test]
fn report_net_worth_series() {
    check_fixture(
        "report_net_worth_series",
        &NetWorthReport {
            points: vec![
                NetWorthPoint {
                    year_month: "2026-03".into(),
                    net_worth: 1_200_000,
                    net: 180_000,
                    // 窓が埋まらない先頭2点は null。
                    net_moving_avg: None,
                },
                NetWorthPoint {
                    year_month: "2026-04".into(),
                    net_worth: 1_387_600,
                    net: 187_600,
                    net_moving_avg: None,
                },
                NetWorthPoint {
                    year_month: "2026-05".into(),
                    net_worth: 1_559_000,
                    net: 171_400,
                    net_moving_avg: Some(179_666),
                },
            ],
        },
    );
}
```

- [ ] **Step 11: フィクスチャを生成して確認する**

Run: `UPDATE_FIXTURES=1 cargo test --test response_fixtures && cargo test --test response_fixtures`
Expected: 2回目が PASS（`report_by_category.json` と `report_net_worth_series.json` が生成される）

- [ ] **Step 12: TypeScript の型と API ラッパーを追加する**

`src/lib/api/reports.ts` の末尾に追加:

```ts
export type CategorySeries = {
  category_id: number;
  name: string;
  type: 'income' | 'expense';
  /** months と同じ長さ。取引の無い月は 0。 */
  points: number[];
};

export type CategoryReport = {
  months: string[];
  income: CategoryAggregate[];
  expense: CategoryAggregate[];
  series: CategorySeries[];
};

export function reportByCategory(
  fromYearMonth: string,
  toYearMonth: string,
): Promise<CategoryReport> {
  return invoke<CategoryReport>('report_by_category', {
    fromYearMonth,
    toYearMonth,
  });
}

export type NetWorthPoint = {
  year_month: string;
  net_worth: number;
  net: number;
  /** 3ヶ月移動平均。窓が埋まらない先頭2点は null。 */
  net_moving_avg: number | null;
};

export type NetWorthReport = {
  points: NetWorthPoint[];
};

export function reportNetWorthSeries(
  fromYearMonth: string,
  toYearMonth: string,
): Promise<NetWorthReport> {
  return invoke<NetWorthReport>('report_net_worth_series', {
    fromYearMonth,
    toYearMonth,
  });
}
```

`invoke` の引数キーは Tauri 2.x が camelCase を Rust の snake_case へ変換するため `fromYearMonth` / `toYearMonth` で渡す（既存 `monthlySummary` が `{ year, month }` を渡しているのと同じ規則）。

- [ ] **Step 13: 契約テストに2件を追加する**

`src/lib/api/contract.test.ts` の型 import を差し替える:

```ts
import type {
  CategoryReport,
  MonthlyBucket,
  MonthlyReport,
  MonthlySummary,
  NetWorthReport,
  YearlyReport,
} from './reports';
```

フィクスチャ import を追加:

```ts
import reportByCategory from '../../../tests/fixtures/responses/report_by_category.json';
import reportNetWorthSeries from '../../../tests/fixtures/responses/report_net_worth_series.json';
```

`COVERED_FIXTURES` に2行追加:

```ts
  'report_by_category.json',
  'report_net_worth_series.json',
```

`report_yearly` のテストの直後に追加:

```ts
  it('report_by_category is a CategoryReport whose series match the month axis', () => {
    fixtureFits<CategoryReport>(reportByCategory);
    typeCovers<typeof reportByCategory>(shapeOf<CategoryReport>());

    for (const series of reportByCategory.series) {
      expect(CATEGORY_TYPES).toContain(series.type);
      expect(series.points).toHaveLength(reportByCategory.months.length);
    }
  });

  it('report_net_worth_series is a NetWorthReport with an empty average head', () => {
    fixtureFits<NetWorthReport>(reportNetWorthSeries);
    typeCovers<typeof reportNetWorthSeries>(shapeOf<NetWorthReport>());

    // 窓が埋まる 3 点目まで移動平均は出ない。
    expect(reportNetWorthSeries.points[0].net_moving_avg).toBeNull();
    expect(reportNetWorthSeries.points[2].net_moving_avg).not.toBeNull();
  });
```

- [ ] **Step 14: E2E モックに2コマンドを足す**

`tests/e2e/tauriMock.ts` の `readyBootResults` に追加:

```ts
  report_by_category: { months: [], income: [], expense: [], series: [] },
  report_net_worth_series: { points: [] },
```

`tests/tauri-mock.test.ts` に import と登録を追加:

```ts
import reportByCategory from './fixtures/responses/report_by_category.json';
import reportNetWorthSeries from './fixtures/responses/report_net_worth_series.json';
```

```ts
  report_by_category: reportByCategory,
  report_net_worth_series: reportNetWorthSeries,
```

- [ ] **Step 15: 全テストと lint**

Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: どちらも PASS

Run: `cd .. && pnpm test && pnpm check`
Expected: どちらも PASS

- [ ] **Step 16: コミット**

```bash
git add src-tauri/src/commands/reports.rs src-tauri/src/lib.rs src-tauri/tests/integration_reports.rs src-tauri/tests/response_fixtures.rs tests/fixtures/responses/report_by_category.json tests/fixtures/responses/report_net_worth_series.json src/lib/api/reports.ts src/lib/api/contract.test.ts tests/e2e/tauriMock.ts tests/tauri-mock.test.ts
git commit -m "feat: serve the category breakdown and the net worth trend

The category tab returns every category's series at once so clicking a
slice narrows the chart without another round trip. The trend tab returns
two different things that look alike: net_worth walks all four directions a
transfer moves money in, while net is the transfer-free monthly result the
rest of the app reports. A test pins the last net_worth point to the
dashboard's total so the two queries cannot drift apart."
```

---

### Task 5: Chart.js の共通ラッパー

**Files:**
- Create: `src/lib/components/Chart.svelte`
- Create: `src/lib/components/Chart.test.ts`
- Modify: `src/routes/Dashboard.svelte`

**Interfaces:**
- Consumes: `chart.js` の `Chart` と各 controller / element / scale
- Produces: `Chart.svelte` — props `{ type: ChartType, data: ChartData, options?: ChartOptions, ariaLabel: string, testId?: string }`。`type` は生成時に一度だけ読む（変更しても作り直さない）。`data` / `options` の差し替えで `chart.update()` が走り、破棄時に `chart.destroy()` する

- [ ] **Step 1: 失敗するテストを書く**

`src/lib/components/Chart.test.ts` を新規作成:

```ts
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
    constructor(canvas: unknown, config: { data: unknown; options: unknown }) {
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

    expect(getByTestId('chart-net-worth')).toHaveAttribute('aria-label', '純資産推移');
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
```

`toHaveAttribute` が使えない場合は `expect(getByTestId('chart-net-worth').getAttribute('aria-label')).toBe('純資産推移')` に置き換える（`src/setupTests.ts` の内容に合わせる）。

- [ ] **Step 2: テストが失敗することを確認する**

Run: `pnpm test src/lib/components/Chart.test.ts`
Expected: FAIL（`./Chart.svelte` が解決できない）

- [ ] **Step 3: `Chart.svelte` を実装する**

`src/lib/components/Chart.svelte` を新規作成:

```svelte
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
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `pnpm test src/lib/components/Chart.test.ts`
Expected: PASS

- [ ] **Step 5: Dashboard をラッパーに載せ替える**

`src/routes/Dashboard.svelte` の変更:

1. `chart.js` からの import 群（`BarController` ～ `Tooltip`）と `Chart.register(...)` の行を削除し、代わりに `import Chart from '../lib/components/Chart.svelte';` を足す
2. `let canvas = $state<HTMLCanvasElement | null>(null);` と `let chart: Chart<'bar'> | null = null;` を削除する
3. `drawChart()` 関数を削除し、`reload()` 内の `drawChart();` 呼び出し2箇所も削除する
4. `onDestroy` の `chart?.destroy();` と `chart = null;` を削除する
5. 系列を `$derived` に変える。`const catStore = ...` の下に追加:

```ts
  const chartData = $derived({
    labels: series.map((bucket) => bucket.year_month),
    datasets: [
      { label: '収入', data: series.map((bucket) => bucket.income), backgroundColor: '#4facfe' },
      { label: '支出', data: series.map((bucket) => bucket.expense), backgroundColor: '#e53e3e' },
    ],
  });

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { y: { beginAtZero: true } },
  };
```

6. テンプレートの `<canvas bind:this={canvas} aria-label="直近12ヶ月の収入と支出"></canvas>` を置き換える:

```svelte
          <Chart
            type="bar"
            data={chartData}
            options={chartOptions}
            ariaLabel="直近12ヶ月の収入と支出"
            testId="chart-monthly-series"
          />
```

- [ ] **Step 6: 型チェックとテストを通す**

Run: `pnpm check && pnpm test`
Expected: どちらも PASS

- [ ] **Step 7: Dashboard の E2E が緑であることを確認する**

Run: `pnpm test:e2e`
Expected: PASS（既存の Dashboard を通る E2E がラッパー載せ替えの回帰検知になる）

- [ ] **Step 8: コミット**

```bash
git add src/lib/components/Chart.svelte src/lib/components/Chart.test.ts src/routes/Dashboard.svelte
git commit -m "refactor: put every chart behind one wrapper before adding five more

The four report tabs would have repeated the dashboard's register / update /
destroy dance six times over, and every copy is a place a chart can outlive
its page. The dashboard moves onto the wrapper in the same commit so the
existing E2E covers the change, and so there is only one copy from the start."
```

---

### Task 6: Reports ルートと4タブ

**Files:**
- Modify: `src/lib/utils/yearMonth.ts`
- Modify: `src/lib/utils/yearMonth.test.ts`
- Create: `src/lib/stores/reports.svelte.ts`
- Create: `src/lib/stores/reports.test.ts`
- Create: `src/routes/Reports.svelte`
- Create: `src/routes/reports/MonthlyTab.svelte`
- Create: `src/routes/reports/YearlyTab.svelte`
- Create: `src/routes/reports/ByCategoryTab.svelte`
- Create: `src/routes/reports/TrendTab.svelte`
- Create: `src/routes/Reports.test.ts`
- Modify: `src/App.svelte:15-24`（`routePaths`）と本文の分岐
- Modify: `src/lib/components/Sidebar.svelte:2-10`（`items`）

**Interfaces:**
- Consumes: Task 3/4 の `reportMonthly` / `reportYearly` / `reportByCategory` / `reportNetWorthSeries` と各型、Task 5 の `Chart.svelte`
- Produces:
  - `src/lib/utils/yearMonth.ts`: `presetRange(preset: RangePreset, today?: Date): { from: string; to: string }`、`export type RangePreset = 'last6' | 'last12' | 'last24' | 'thisYear'`
  - `src/lib/stores/reports.svelte.ts`: `createReportsStore()` → `{ monthly, yearly, byCategory, netWorth, preset, year, loading, error, setPreset(p), setYear(y), load(), dispose() }`
  - `Reports.svelte`: `data-testid="page-reports"`、タブボタン `data-testid="reports-tab-{monthly|yearly|category|trend}"`

- [ ] **Step 1: `presetRange` の失敗するテストを書く**

`src/lib/utils/yearMonth.test.ts` の末尾に追加:

```ts
import { presetRange } from './yearMonth';

describe('presetRange', () => {
  const today = new Date(2026, 4, 15); // 2026-05-15

  it('last12 ends on the current month and spans twelve', () => {
    expect(presetRange('last12', today)).toEqual({ from: '2025-06', to: '2026-05' });
  });

  it('last6 spans six months', () => {
    expect(presetRange('last6', today)).toEqual({ from: '2025-12', to: '2026-05' });
  });

  it('last24 spans twenty-four months', () => {
    expect(presetRange('last24', today)).toEqual({ from: '2024-06', to: '2026-05' });
  });

  it('thisYear covers january through december', () => {
    expect(presetRange('thisYear', today)).toEqual({ from: '2026-01', to: '2026-12' });
  });
});
```

既存ファイルが `describe` / `it` を import 済みかを確認し、無ければ `import { describe, expect, it } from 'vitest';` を先頭に足す。

- [ ] **Step 2: テストが失敗することを確認する**

Run: `pnpm test src/lib/utils/yearMonth.test.ts`
Expected: FAIL（`presetRange` が export されていない）

- [ ] **Step 3: `presetRange` を実装する**

`src/lib/utils/yearMonth.ts` の末尾に追加:

```ts
export type RangePreset = 'last6' | 'last12' | 'last24' | 'thisYear';

const PRESET_MONTHS: Record<Exclude<RangePreset, 'thisYear'>, number> = {
  last6: 6,
  last12: 12,
  last24: 24,
};

function yearMonthKey(year: number, month: number): string {
  return `${year}-${pad2(month)}`;
}

/**
 * プリセットを `YYYY-MM` の閉区間に開く。区間の意味づけ（何ヶ月ぶんか）は
 * ここだけが持ち、集計そのものは Rust 側が行う。
 */
export function presetRange(
  preset: RangePreset,
  today: Date = new Date(),
): { from: string; to: string } {
  const year = today.getFullYear();
  const month = today.getMonth() + 1;

  if (preset === 'thisYear') {
    return { from: yearMonthKey(year, 1), to: yearMonthKey(year, 12) };
  }

  const span = PRESET_MONTHS[preset];
  const startIndex = year * 12 + (month - 1) - (span - 1);
  return {
    from: yearMonthKey(Math.floor(startIndex / 12), (startIndex % 12) + 1),
    to: yearMonthKey(year, month),
  };
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `pnpm test src/lib/utils/yearMonth.test.ts`
Expected: PASS

- [ ] **Step 5: ストアの失敗するテストを書く**

`src/lib/stores/reports.test.ts` を新規作成:

```ts
import { describe, expect, it, vi } from 'vitest';

const reportMonthly = vi.fn();
const reportYearly = vi.fn();
const reportByCategory = vi.fn();
const reportNetWorthSeries = vi.fn();

vi.mock('../api/reports', () => ({
  reportMonthly: (...args: unknown[]) => reportMonthly(...args),
  reportYearly: (...args: unknown[]) => reportYearly(...args),
  reportByCategory: (...args: unknown[]) => reportByCategory(...args),
  reportNetWorthSeries: (...args: unknown[]) => reportNetWorthSeries(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: () => Promise.reject(new Error('no tauri event bus')),
}));

const { createReportsStore } = await import('./reports.svelte');

const emptyMonthly = {
  current: { income: 0, expense: 0, net: 0 },
  prev_month: { income: 0, expense: 0, net: 0 },
  prev_year: { income: 0, expense: 0, net: 0 },
  mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
  yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
  top_expense: [],
  top_income: [],
};

function stubAll() {
  reportMonthly.mockResolvedValue(emptyMonthly);
  reportYearly.mockResolvedValue({
    year: 2026,
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  });
  reportByCategory.mockResolvedValue({ months: [], income: [], expense: [], series: [] });
  reportNetWorthSeries.mockResolvedValue({ points: [] });
}

describe('reports store', () => {
  it('loads all four reports and passes the preset range through', async () => {
    stubAll();
    const store = createReportsStore(new Date(2026, 4, 15));

    await store.load();

    expect(reportByCategory).toHaveBeenCalledWith('2025-06', '2026-05');
    expect(reportNetWorthSeries).toHaveBeenCalledWith('2025-06', '2026-05');
    expect(reportMonthly).toHaveBeenCalledWith(2026, 5);
    expect(reportYearly).toHaveBeenCalledWith(2026);
    expect(store.netWorth).toEqual({ points: [] });
    expect(store.error).toBeNull();
    await store.dispose();
  });

  it('reloads with the new range when the preset changes', async () => {
    stubAll();
    const store = createReportsStore(new Date(2026, 4, 15));
    await store.load();
    reportByCategory.mockClear();

    await store.setPreset('last6');

    expect(reportByCategory).toHaveBeenCalledWith('2025-12', '2026-05');
    await store.dispose();
  });

  it('keeps the message when a command rejects', async () => {
    stubAll();
    reportMonthly.mockRejectedValue(new Error('invalid argument: month out of range: 13'));
    const store = createReportsStore(new Date(2026, 4, 15));

    await store.load();

    expect(store.error).toBe('invalid argument: month out of range: 13');
    await store.dispose();
  });
});
```

- [ ] **Step 6: テストが失敗することを確認する**

Run: `pnpm test src/lib/stores/reports.test.ts`
Expected: FAIL（`./reports.svelte` が解決できない）

- [ ] **Step 7: ストアを実装する**

`src/lib/stores/reports.svelte.ts` を新規作成:

```ts
import type { UnlistenFn } from '@tauri-apps/api/event';
import { onDataChanged } from '../api/events';
import {
  reportByCategory,
  reportMonthly,
  reportNetWorthSeries,
  reportYearly,
  type CategoryReport,
  type MonthlyReport,
  type NetWorthReport,
  type YearlyReport,
} from '../api/reports';
import { presetRange, type RangePreset } from '../utils/yearMonth';

export type ReportsStore = {
  readonly monthly: MonthlyReport | null;
  readonly yearly: YearlyReport | null;
  readonly byCategory: CategoryReport | null;
  readonly netWorth: NetWorthReport | null;
  readonly preset: RangePreset;
  readonly year: number;
  readonly month: number;
  readonly loading: boolean;
  readonly error: string | null;
  setPreset(preset: RangePreset): Promise<void>;
  setYear(year: number): Promise<void>;
  load(): Promise<void>;
  dispose(): Promise<void>;
};

/**
 * 4本のレポートコマンドをまとめて取得する。集計はすべて Rust 側にあるので、
 * ここは取得と失敗の保持しかしない。
 */
export function createReportsStore(today: Date = new Date()): ReportsStore {
  let monthly = $state<MonthlyReport | null>(null);
  let yearly = $state<YearlyReport | null>(null);
  let byCategory = $state<CategoryReport | null>(null);
  let netWorth = $state<NetWorthReport | null>(null);
  let preset = $state<RangePreset>('last12');
  let year = $state(today.getFullYear());
  const month = today.getMonth() + 1;
  let loading = $state(false);
  let error = $state<string | null>(null);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    const { from, to } = presetRange(preset, today);
    try {
      const [nextMonthly, nextYearly, nextCategory, nextNetWorth] = await Promise.all([
        reportMonthly(today.getFullYear(), month),
        reportYearly(year),
        reportByCategory(from, to),
        reportNetWorthSeries(from, to),
      ]);
      if (disposed || id !== requestId) return;
      monthly = nextMonthly;
      yearly = nextYearly;
      byCategory = nextCategory;
      netWorth = nextNetWorth;
    } catch (e) {
      if (disposed || id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (!disposed && id === requestId) loading = false;
    }
  }

  void (async () => {
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
          void load();
        }
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus; explicit load() still runs.
    }
  })();

  return {
    get monthly() {
      return monthly;
    },
    get yearly() {
      return yearly;
    },
    get byCategory() {
      return byCategory;
    },
    get netWorth() {
      return netWorth;
    },
    get preset() {
      return preset;
    },
    get year() {
      return year;
    },
    get month() {
      return month;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    async setPreset(next) {
      preset = next;
      await load();
    },
    async setYear(next) {
      year = next;
      await load();
    },
    load,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
```

- [ ] **Step 8: テストが通ることを確認する**

Run: `pnpm test src/lib/stores/reports.test.ts`
Expected: PASS

- [ ] **Step 9: コミット（ここまでで UI 抜きの土台が揃う）**

```bash
git add src/lib/utils/yearMonth.ts src/lib/utils/yearMonth.test.ts src/lib/stores/reports.svelte.ts src/lib/stores/reports.test.ts
git commit -m "feat: turn a range preset into the four report fetches

The preset is the only period logic the frontend owns: it names a span, the
store opens it into YYYY-MM bounds, and Rust does the arithmetic. Keeping
the four fetches in one store means a range change reloads them together
rather than leaving one tab showing the previous window."
```

- [ ] **Step 10: 月次タブを作る**

`src/routes/reports/MonthlyTab.svelte` を新規作成:

```svelte
<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { Delta, MonthlyBucket, MonthlyReport } from '../../lib/api/reports';

  let {
    report,
    series,
    year,
    month,
  }: {
    report: MonthlyReport | null;
    series: MonthlyBucket[];
    year: number;
    month: number;
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
      <div class="chart-box">
        <Chart
          type="bar"
          data={chartData}
          options={chartOptions}
          ariaLabel="直近12ヶ月の収入と支出"
          testId="chart-reports-monthly"
        />
      </div>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>{year}年{month}月の比較</h2>
      {#if report === null}
        <EmptyState title="読み込み中" hint="集計を取得しています" />
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
      {#if !report || report.top_expense.length === 0}
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
      {#if !report || report.top_income.length === 0}
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
```

- [ ] **Step 11: 年次タブを作る**

`src/routes/reports/YearlyTab.svelte` を新規作成:

```svelte
<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { YearlyReport } from '../../lib/api/reports';

  let {
    report,
    onYearChange,
  }: {
    report: YearlyReport | null;
    onYearChange: (year: number) => void;
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
        <EmptyState title="読み込み中" hint="集計を取得しています" />
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
```

- [ ] **Step 12: カテゴリ別タブを作る**

`src/routes/reports/ByCategoryTab.svelte` を新規作成。円グラフのクリックと、E2E / キーボード操作のためのボタン凡例が同じ選択状態を切り替える:

```svelte
<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import { formatCurrency } from '../../lib/utils/formatCurrency';
  import type { CategoryReport } from '../../lib/api/reports';

  let { report }: { report: CategoryReport | null } = $props();

  const PALETTE = ['#667eea', '#764ba2', '#4facfe', '#00f2fe', '#f6ad55', '#e53e3e', '#38b2ac'];
  const TREND_LIMIT = 5;

  let selectedCategoryId = $state<number | null>(null);

  const expense = $derived(report?.expense ?? []);
  const income = $derived(report?.income ?? []);

  /** 初期は支出上位5本。1つ選ぶとその1本だけ。Rust が降順で返すので先頭を取るだけ。 */
  const visibleSeries = $derived.by(() => {
    const all = report?.series ?? [];
    if (selectedCategoryId !== null) {
      return all.filter((s) => s.category_id === selectedCategoryId);
    }
    const topIds = new Set(expense.slice(0, TREND_LIMIT).map((a) => a.category_id));
    return all.filter((s) => topIds.has(s.category_id));
  });

  const selectedName = $derived(
    selectedCategoryId === null
      ? '支出上位5カテゴリ'
      : ([...expense, ...income].find((a) => a.category_id === selectedCategoryId)?.name ?? '—'),
  );

  function toggle(categoryId: number) {
    selectedCategoryId = selectedCategoryId === categoryId ? null : categoryId;
  }

  function pieData(aggregates: typeof expense) {
    return {
      labels: aggregates.map((a) => a.name),
      datasets: [
        {
          data: aggregates.map((a) => a.amount),
          backgroundColor: aggregates.map((_, i) => PALETTE[i % PALETTE.length]),
        },
      ],
    };
  }

  function pieOptions(aggregates: typeof expense) {
    return {
      responsive: true,
      maintainAspectRatio: false,
      onClick: (_event: unknown, elements: { index: number }[]) => {
        const hit = elements[0];
        if (hit && aggregates[hit.index]) toggle(aggregates[hit.index].category_id);
      },
    };
  }

  const expensePie = $derived(pieData(expense));
  const expensePieOptions = $derived(pieOptions(expense));
  const incomePie = $derived(pieData(income));
  const incomePieOptions = $derived(pieOptions(income));

  const trendData = $derived({
    labels: report?.months ?? [],
    datasets: visibleSeries.map((series, i) => ({
      label: series.name,
      data: series.points,
      borderColor: PALETTE[i % PALETTE.length],
      backgroundColor: PALETTE[i % PALETTE.length],
      tension: 0.25,
    })),
  });

  const trendOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: { y: { beginAtZero: true } },
  };
</script>

<div class="by-category">
  <Card>
    {#snippet children()}
      <h2>支出の内訳</h2>
      {#if expense.length === 0}
        <EmptyState title="支出がありません" hint="期間を広げるか取引を記録してください" />
      {:else}
        <div class="pie-box">
          <Chart
            type="doughnut"
            data={expensePie}
            options={expensePieOptions}
            ariaLabel="支出のカテゴリ内訳"
            testId="chart-category-expense"
          />
        </div>
        <ul class="legend" data-testid="legend-expense">
          {#each expense as aggregate, i (aggregate.category_id)}
            <li>
              <button
                type="button"
                class:selected={selectedCategoryId === aggregate.category_id}
                data-testid={`legend-category-${aggregate.category_id}`}
                onclick={() => toggle(aggregate.category_id)}
              >
                <span class="swatch" style={`background: ${PALETTE[i % PALETTE.length]}`}></span>
                <span class="legend-name">{aggregate.name}</span>
                <span class="legend-amount">{formatCurrency(aggregate.amount)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>収入の内訳</h2>
      {#if income.length === 0}
        <EmptyState title="収入がありません" hint="期間を広げるか取引を記録してください" />
      {:else}
        <div class="pie-box">
          <Chart
            type="doughnut"
            data={incomePie}
            options={incomePieOptions}
            ariaLabel="収入のカテゴリ内訳"
            testId="chart-category-income"
          />
        </div>
        <ul class="legend" data-testid="legend-income">
          {#each income as aggregate, i (aggregate.category_id)}
            <li>
              <button
                type="button"
                class:selected={selectedCategoryId === aggregate.category_id}
                data-testid={`legend-category-${aggregate.category_id}`}
                onclick={() => toggle(aggregate.category_id)}
              >
                <span class="swatch" style={`background: ${PALETTE[i % PALETTE.length]}`}></span>
                <span class="legend-name">{aggregate.name}</span>
                <span class="legend-amount">{formatCurrency(aggregate.amount)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <div class="trend-head">
        <h2>カテゴリ別の推移</h2>
        <span data-testid="selected-category">{selectedName}</span>
      </div>
      <div class="chart-box">
        <Chart
          type="line"
          data={trendData}
          options={trendOptions}
          ariaLabel="カテゴリ別の月次推移"
          testId="chart-category-trend"
        />
      </div>
    {/snippet}
  </Card>
</div>

<style>
  .by-category {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--space-5);
  }

  h2 {
    margin: 0 0 var(--space-4);
  }

  .pie-box {
    height: 240px;
    min-width: 0;
  }

  .chart-box {
    height: 300px;
    min-width: 0;
  }

  .trend-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-3);
  }

  .trend-head span {
    color: var(--muted);
    font-weight: 700;
  }

  .legend {
    list-style: none;
    margin: var(--space-4) 0 0;
    padding: 0;
    display: grid;
    gap: var(--space-1);
  }

  .legend button {
    width: 100%;
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    cursor: pointer;
    font: inherit;
    color: inherit;
    text-align: left;
  }

  .legend button:hover,
  .legend button.selected {
    background: rgba(0, 0, 0, 0.06);
  }

  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }

  .legend-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .legend-amount {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }
</style>
```

- [ ] **Step 13: トレンドタブを作る**

`src/routes/reports/TrendTab.svelte` を新規作成:

```svelte
<script lang="ts">
  import Card from '../../lib/components/Card.svelte';
  import Chart from '../../lib/components/Chart.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import type { NetWorthReport } from '../../lib/api/reports';

  let { report }: { report: NetWorthReport | null } = $props();

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
      {#if points.length === 0}
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
      {#if points.length === 0}
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
```

- [ ] **Step 14: Reports ルートを作る**

`src/routes/Reports.svelte` を新規作成:

```svelte
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ByCategoryTab from './reports/ByCategoryTab.svelte';
  import MonthlyTab from './reports/MonthlyTab.svelte';
  import TrendTab from './reports/TrendTab.svelte';
  import YearlyTab from './reports/YearlyTab.svelte';
  import { monthlySeries, type MonthlyBucket } from '../lib/api/reports';
  import { createReportsStore } from '../lib/stores/reports.svelte';
  import type { RangePreset } from '../lib/utils/yearMonth';

  type Tab = 'monthly' | 'yearly' | 'category' | 'trend';

  const tabs: { id: Tab; label: string }[] = [
    { id: 'monthly', label: '月次' },
    { id: 'yearly', label: '年次' },
    { id: 'category', label: 'カテゴリ別' },
    { id: 'trend', label: 'トレンド' },
  ];

  const presets: { id: RangePreset; label: string }[] = [
    { id: 'last6', label: '直近6ヶ月' },
    { id: 'last12', label: '直近12ヶ月' },
    { id: 'last24', label: '直近24ヶ月' },
    { id: 'thisYear', label: '今年' },
  ];

  const store = createReportsStore();

  // 月次タブの棒グラフは既存コマンドを使う（系列を二重に持たない）。
  let series = $state<MonthlyBucket[]>([]);
  let seriesError = $state<string | null>(null);

  onMount(() => {
    void store.load();
    void monthlySeries(12)
      .then((next) => {
        series = next;
      })
      .catch((e) => {
        seriesError = e instanceof Error ? e.message : String(e);
      });
  });

  onDestroy(() => {
    void store.dispose();
  });

  let tab = $state<Tab>('monthly');
</script>

<section data-testid="page-reports">
  <h1>レポート</h1>

  {#if store.error}
    <p class="error" data-testid="reports-error">エラー: {store.error}</p>
  {/if}
  {#if seriesError}
    <p class="error" data-testid="reports-series-error">エラー: {seriesError}</p>
  {/if}

  <div class="controls">
    <div class="tabs" role="tablist" aria-label="レポートの種類">
      {#each tabs as item (item.id)}
        <button
          type="button"
          role="tab"
          aria-selected={tab === item.id}
          class:active={tab === item.id}
          data-testid={`reports-tab-${item.id}`}
          onclick={() => (tab = item.id)}
        >
          {item.label}
        </button>
      {/each}
    </div>

    {#if tab === 'category' || tab === 'trend'}
      <div class="presets" data-testid="reports-presets">
        {#each presets as item (item.id)}
          <button
            type="button"
            class:active={store.preset === item.id}
            data-testid={`reports-preset-${item.id}`}
            onclick={() => void store.setPreset(item.id)}
          >
            {item.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if tab === 'monthly'}
    <MonthlyTab report={store.monthly} {series} year={new Date().getFullYear()} month={store.month} />
  {:else if tab === 'yearly'}
    <YearlyTab report={store.yearly} onYearChange={(year) => void store.setYear(year)} />
  {:else if tab === 'category'}
    <ByCategoryTab report={store.byCategory} />
  {:else}
    <TrendTab report={store.netWorth} />
  {/if}
</section>

<style>
  h1 {
    color: white;
    margin: 0 0 var(--space-5);
  }

  .controls {
    display: flex;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }

  .tabs,
  .presets {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  button {
    padding: var(--space-2) var(--space-4);
    border: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    color: white;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
  }

  button:hover {
    background: rgba(255, 255, 255, 0.24);
  }

  button.active {
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }

  .error {
    color: var(--danger);
    font-weight: 700;
  }
</style>
```

- [ ] **Step 15: ルーティングとナビに追加する**

`src/App.svelte`:

1. import に追加: `import Reports from './routes/Reports.svelte';`
2. `routePaths` の `'/budgets',` の直後に `'/reports',` を追加
3. 本文の `{:else if currentPath === '/budgets'}` ブロックの直後に追加:

```svelte
      {:else if currentPath === '/reports'}
        <Reports />
```

`src/lib/components/Sidebar.svelte` の `items` の `{ path: '/budgets', ... }` の直後に追加:

```ts
    { path: '/reports', label: 'レポート', icon: '📈' },
```

- [ ] **Step 16: ルートのテストを書いて通す**

`src/routes/Reports.test.ts` を新規作成:

```ts
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('../lib/api/reports', () => ({
  monthlySeries: vi.fn().mockResolvedValue([]),
  reportMonthly: vi.fn().mockResolvedValue({
    current: { income: 0, expense: 0, net: 0 },
    prev_month: { income: 0, expense: 0, net: 0 },
    prev_year: { income: 0, expense: 0, net: 0 },
    mom: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    yoy: { income_diff: 0, expense_diff: 0, net_diff: 0, expense_percent: null },
    top_expense: [],
    top_income: [],
  }),
  reportYearly: vi.fn().mockResolvedValue({
    year: 2026,
    months: [],
    total_income: 0,
    total_expense: 0,
    net: 0,
    avg_income: 0,
    avg_expense: 0,
    max_expense_month: null,
  }),
  reportByCategory: vi.fn().mockResolvedValue({
    months: [],
    income: [],
    expense: [],
    series: [],
  }),
  reportNetWorthSeries: vi.fn().mockResolvedValue({ points: [] }),
}));

vi.mock('../lib/api/events', () => ({
  onDataChanged: () => Promise.reject(new Error('no tauri event bus')),
}));

vi.mock('../lib/components/Chart.svelte', async () => {
  // chart.js は jsdom に canvas コンテキストが無いので描画ごと差し替える。
  const Stub = (await import('../lib/components/__stubs__/ChartStub.svelte')).default;
  return { default: Stub };
});

const Reports = (await import('./Reports.svelte')).default;

describe('Reports', () => {
  it('opens on the monthly tab and switches to the trend tab', async () => {
    render(Reports);

    expect(screen.getByTestId('page-reports')).toBeTruthy();
    expect(screen.getByTestId('reports-tab-monthly').getAttribute('aria-selected')).toBe('true');

    screen.getByTestId('reports-tab-trend').click();
    await Promise.resolve();

    expect(screen.getByTestId('reports-tab-trend').getAttribute('aria-selected')).toBe('true');
  });

  it('shows the range presets only on the range-driven tabs', async () => {
    render(Reports);

    expect(screen.queryByTestId('reports-presets')).toBeNull();

    screen.getByTestId('reports-tab-category').click();
    await Promise.resolve();

    expect(screen.getByTestId('reports-presets')).toBeTruthy();
  });
});
```

`src/lib/components/__stubs__/ChartStub.svelte` を新規作成:

```svelte
<script lang="ts">
  let { ariaLabel, testId }: { ariaLabel: string; testId?: string } = $props();
</script>

<div aria-label={ariaLabel} data-testid={testId}></div>
```

Run: `pnpm test src/routes/Reports.test.ts`
Expected: PASS

- [ ] **Step 17: 全テストと型チェック**

Run: `pnpm test && pnpm check`
Expected: どちらも PASS

- [ ] **Step 18: コミット**

```bash
git add src/routes/Reports.svelte src/routes/reports src/routes/Reports.test.ts src/lib/components/__stubs__/ChartStub.svelte src/App.svelte src/lib/components/Sidebar.svelte
git commit -m "feat: give the four report tabs a page to live on

Each tab is its own file so none of them grows into the others, and the tab
state stays in the component: the router matches whole paths, so putting it
in the URL would have meant reworking routing for a preference that costs
nothing to lose on reload.

The pie chart's click and the legend buttons drive the same selection, which
keeps the narrowing reachable by keyboard and testable without hunting for
canvas coordinates."
```

---

### Task 7: E2E とフェーズ完了確認

**Files:**
- Create: `tests/e2e/report-flow.spec.ts`
- Modify: `CLAUDE.md`（現在の状態を Phase 5b 完了に更新）

**Interfaces:**
- Consumes: Task 3/4 で `tauriMock.ts` に追加済みの4コマンド、Task 6 の `data-testid`

- [ ] **Step 1: E2E を書く**

`tests/e2e/report-flow.spec.ts` を新規作成:

```ts
import { expect, test } from '@playwright/test';

import { installReadyBootMock } from './tauriMock';

const categoryReport = {
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

const monthlyReport = {
  current: { income: 320_000, expense: 148_600, net: 171_400 },
  prev_month: { income: 320_000, expense: 132_400, net: 187_600 },
  prev_year: { income: 300_000, expense: 0, net: 300_000 },
  mom: { income_diff: 0, expense_diff: 16_200, net_diff: -16_200, expense_percent: 12 },
  yoy: { income_diff: 20_000, expense_diff: 148_600, net_diff: -128_600, expense_percent: null },
  top_expense: [{ category_id: 1, name: '食費', type: 'expense', amount: 148_600 }],
  top_income: [{ category_id: 2, name: '給与', type: 'income', amount: 320_000 }],
};

test('the report tabs render and the category trend narrows on a click', async ({ page }) => {
  // アプリは Dashboard (`/`) で起動し、そこで list_balances / monthly_summary /
  // monthly_series / list_transactions / list_top_budget_statuses を叩く。
  // installReadyBootMock がそれらを空の形で返すので初期描画は落ちない。この
  // init script はその上に重ねて、このシナリオが読む値だけを差し替える。
  await installReadyBootMock(page);

  await page.addInitScript(
    (fixtures: Record<string, unknown>) => {
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      const previous = internals.invoke;
      internals.invoke = async (command: string, args: any) => {
        if (Object.prototype.hasOwnProperty.call(fixtures, command)) return fixtures[command];
        if (typeof previous === 'function') return previous(command, args);
        return null;
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    {
      report_monthly: monthlyReport,
      report_by_category: categoryReport,
    },
  );

  await page.goto('/reports');
  await expect(page.getByTestId('page-reports')).toBeVisible();

  // 月次タブ: 割合は Rust が出した値をそのまま出す。前年同月は分母 0 なので "—"。
  await expect(page.getByTestId('compare-mom')).toContainText('12%');
  await expect(page.getByTestId('compare-yoy')).toContainText('—');
  await expect(page.getByTestId('top-expense')).toContainText('食費');

  await page.getByTestId('reports-tab-category').click();
  await expect(page.getByTestId('selected-category')).toHaveText('支出上位5カテゴリ');

  await page.getByTestId('legend-category-3').click();
  await expect(page.getByTestId('selected-category')).toHaveText('交通費');

  // もう一度押すと選択が外れて初期表示に戻る。
  await page.getByTestId('legend-category-3').click();
  await expect(page.getByTestId('selected-category')).toHaveText('支出上位5カテゴリ');

  await page.getByTestId('reports-tab-trend').click();
  await expect(page.getByTestId('chart-net-worth')).toBeHidden();
  await expect(page.getByText('データがありません')).toBeVisible();
});
```

`await page.goto('/reports')` が動かない場合（dev server が SPA fallback を返さない場合）は、`await page.goto('/')` してから `await page.getByTestId('nav-reports').click()` に置き換える。

- [ ] **Step 2: E2E を実行する**

Run: `pnpm test:e2e tests/e2e/report-flow.spec.ts`
Expected: PASS

- [ ] **Step 3: フェーズ完了条件をすべて実行する**

Run:

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test && cd ..
pnpm test
pnpm check
pnpm test:e2e
```

Expected: すべて PASS。1つでも落ちたら、そこで止めて原因を直してからこのステップをやり直す

- [ ] **Step 4: CLAUDE.md の現在の状態を更新する**

`CLAUDE.md` の「現在の状態」の1行目を置き換える:

```markdown
- Phase 5b 完了 (分析レポート: 月次 / 年次 / カテゴリ別 / トレンドの4タブ + Reports 画面)
```

同じブロックの最終行を置き換える:

```markdown
- 次は spec の Phase 6 (Excel エクスポート等)
```

- [ ] **Step 5: コミット**

```bash
git add tests/e2e/report-flow.spec.ts CLAUDE.md
git commit -m "test: drive the report tabs end to end

The narrowing is the part worth pinning: a click has to change what the
trend chart shows, and a second click has to put it back. Driving it through
the legend buttons rather than canvas coordinates means the test breaks when
the behaviour breaks, not when the chart moves a few pixels."
```

- [ ] **Step 6: 実機ビルドを確認する（手動）**

Run: `pnpm tauri dev`

確認すること:
- サイドバーの「レポート 📈」から `/reports` が開く
- 4タブがすべて描画され、切り替えでチャートが差し替わる
- カテゴリ別タブで凡例と円グラフのどちらを押しても推移が絞られる
- レンジプリセットを変えるとカテゴリ別とトレンドが両方とも更新される
- トレンドタブの純資産の最新点が Dashboard の「総資産」と一致する

macOS と Windows の両方でビルドが通ることも spec の完了条件だが、これは実機の都合で本計画の最後に手動で行う。

---

## Self-Review

**1. Spec coverage（§5.6 の各要件 → タスク）**

| spec の要件 | 実装するタスク |
|---|---|
| 月次タブ: 月別収支グラフ | Task 6 Step 10（既存 `monthly_series` を使用） |
| 月次タブ: 前月比 / 前年同月比 | Task 3（`build_monthly_report` の `mom` / `yoy`） |
| 月次タブ: 支出 / 収入 Top5 | Task 1 `top_n` + Task 3 |
| 年次タブ: 積み上げグラフ | Task 3（12ヶ月固定）+ Task 6 Step 11 |
| 年次タブ: 12ヶ月平均・最大支出月 | Task 1 `yearly_stats` + Task 3 |
| カテゴリ別タブ: 円グラフ2種 | Task 4 `income` / `expense` + Task 6 Step 12 |
| カテゴリ別タブ: 月別推移と絞り込み | Task 1 `pivot_category_series` + Task 4 + Task 6 Step 12 |
| トレンドタブ: 純資産推移 | Task 2 の純資産クエリ + Task 4 |
| トレンドタブ: 月次 net の3ヶ月移動平均 | Task 1 `moving_average` + Task 4 |
| 期間モデル（レンジ + プリセット + 上限60ヶ月） | Task 4 `range_months` + Task 6 `presetRange` |
| `expense_percent` の 0 除算回避 | Task 1 `compare` + Task 3 のフィクスチャで固定 |
| 移動平均を point に同居 | Task 4 `NetWorthPoint` |
| 純資産 = 非アーカイブ口座、Dashboard と一致 | Task 2 + Task 4 の `net_worth_series_ends_on_the_dashboard_total` |
| 再クリックで初期表示に戻る | Task 6 Step 12 `toggle` + Task 7 の E2E |
| 上位5の順位付けは Rust 側 | Task 2 `category_totals_between` の `ORDER BY amount DESC` |
| Chart.js を共通ラッパーに集約 | Task 5 |
| タブ状態を URL に載せない | Task 6 Step 14 |
| 振替を収支集計から除外 | Task 2 の全クエリ + Task 2 のテスト |

漏れなし。

**2. Placeholder scan:** 「後で実装」「適切なエラー処理」の類は無し。全コードステップに実コードあり。

**3. Type consistency:**
- `CategoryAggregate` は Task 1 で domain に移し、以降 Task 2/3/4 で同じパスから参照している
- `PeriodTotals` / `Delta` は Task 1 で定義し Task 3 の `MonthlyReport` が使う。フィクスチャ（Task 3 Step 7）も同じ型を import する
- `CategorySeries` は Task 1 で定義、Task 4 の `CategoryReport` と TS 側 `CategorySeries` でフィールド名（`category_id` / `name` / `type` / `points`）が一致
- `range_months` は Task 4 で定義し、同タスク内の2コマンドだけが使う
- `presetRange` / `RangePreset` は Task 6 で `yearMonth.ts` に定義し、同タスクのストアが import
- `Chart.svelte` の props（`type` / `data` / `options` / `ariaLabel` / `testId`）は Task 5 で定義し、Task 6 の4タブすべてが同じ名前で渡す
- `build_monthly_report` / `build_yearly_report` / `build_category_report` / `build_net_worth_report` は結合テストと `#[tauri::command]` の両方から同じ名前で呼ばれる
