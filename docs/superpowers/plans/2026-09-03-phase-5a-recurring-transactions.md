# Phase 5a: 定期取引 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 定期取引ルールを登録でき、アプリ起動時に未生成期間ぶんの取引が冪等に自動生成される。

**Architecture:** 日付列挙は `domain/recurring.rs` の純粋関数に閉じ込める(時計も DB も触らない)。窓を左開右閉 `(last_generated_on, today]` にすることで、連続起動しても二重生成されない。`infra/repo/recurring_repo.rs` が SQL、`commands/recurring.rs` が薄いバリデーション層。展開は `setup()` ではなくフロントからの明示コマンドで走らせ、生成件数とスキップ理由を UI に返す。

**Tech Stack:** Rust + rusqlite 0.32 + chrono 0.4 + proptest 1 / Svelte 5 (Runes) + TypeScript + Vitest + Playwright

**Spec:** [docs/superpowers/specs/2026-05-24-budget-tracker-design.md](../specs/2026-05-24-budget-tracker-design.md) §5.4「定期取引」

## Global Constraints

これは BudgetTracker 全体の規約で、**全タスクの要件に暗黙に含まれる**。

- 金額はすべて整数(円)。Rust は `i64`、TypeScript は `number`。小数演算を入れない
- 集計・評価・展開ロジックは `src-tauri/src/domain/` に置く。Svelte 側でビジネスロジックを書かない
- `transactions.type='transfer'` の行を「総支出」「総収入」集計に含めない
- 銀行名・カテゴリ名などユーザー定義データをコードに埋め込まない
- 物理削除しない。`DELETE FROM recurring_rules` は禁止。停止は `active = 0`
- スキーマ変更は `src-tauri/migrations/V<番号>__<説明>.sql` を新規追加。既存ファイルの編集は禁止
- コミットは Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`)。本文には「なぜ」を書く
- 各コミット前に最低限 `cd src-tauri && cargo clippy --all-targets -- -D warnings` が緑であること
- chrono の `Duration` は非推奨警告が出る。日付加算は `Days::new(u64)` を使う

## File Structure

**新規作成**

| ファイル | 責務 |
|---|---|
| `src-tauri/src/domain/recurring.rs` | Frequency / Schedule / 日付列挙 / 入力バリデーション(純粋) |
| `src-tauri/migrations/V005__recurring_transaction_index.sql` | `transactions(recurring_id)` インデックス |
| `src-tauri/src/infra/repo/recurring_repo.rs` | `recurring_rules` の SQL と生成取引の一括 INSERT |
| `src-tauri/src/commands/recurring.rs` | Tauri コマンド 6 本 |
| `src-tauri/tests/integration_recurring.rs` | repo + コマンドの結合テスト |
| `src/lib/api/recurring.ts` | 型付き invoke ラッパー |
| `src/lib/api/recurring.test.ts` | ラッパーの引数受け渡しテスト |
| `src/lib/stores/recurring.svelte.ts` | Runes ストア |
| `src/lib/stores/recurring.test.ts` | ストアのテスト |
| `src/routes/Recurring.svelte` | ルール一覧 + 作成/編集モーダル |
| `tests/e2e/recurring-flow.spec.ts` | Playwright E2E |
| `tests/fixtures/responses/list_recurring_rules.json` | 契約フィクスチャ(生成物) |
| `tests/fixtures/responses/expand_due_recurring.json` | 契約フィクスチャ(生成物) |
| `tests/fixtures/responses/preview_recurring_occurrences.json` | 契約フィクスチャ(生成物) |

**変更**

| ファイル | 変更内容 |
|---|---|
| `src-tauri/src/domain/mod.rs` | `pub mod recurring;` |
| `src-tauri/src/infra/repo/mod.rs` | `pub mod recurring_repo;` |
| `src-tauri/src/commands/mod.rs` | `pub mod recurring;` |
| `src-tauri/src/infra/events.rs` | `ChangedDomain::Recurring` |
| `src-tauri/src/lib.rs` | `invoke_handler` にコマンド 6 本 |
| `src-tauri/tests/response_fixtures.rs` | 新規レスポンス 3 件のサンプル |
| `src/lib/api/index.ts` | `export * from './recurring';` |
| `src/lib/api/events.ts` | `ChangedDomain` に `'recurring'` |
| `src/lib/api/contract.test.ts` | 新規フィクスチャ 3 件の型ピン留め |
| `src/App.svelte` | `/recurring` ルート + 起動時展開 + バナー |
| `src/lib/components/Sidebar.svelte` | ナビ項目 |
| `tests/e2e/tauriMock.ts` | 新規コマンドのモック |
| `tests/tauri-mock.test.ts` | モックとフィクスチャの対応 |
| `CLAUDE.md` | 「現在の状態」を Phase 5a 完了に更新 |

---

### Task 1: 日付列挙の純粋関数

**Files:**
- Create: `src-tauri/src/domain/recurring.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Test: `src-tauri/src/domain/recurring.rs`(同ファイル内 `#[cfg(test)] mod tests` / `mod prop_tests`)

**Interfaces:**
- Consumes: `crate::error::{AppError, AppResult}`
- Produces:
  - `Frequency` — `Monthly | Weekly | Yearly`、`as_sql(self) -> &'static str`、`parse(&str) -> AppResult<Self>`
  - `Schedule { frequency: Frequency, day_of_month: Option<u32>, day_of_week: Option<u32>, starts_on: NaiveDate, ends_on: Option<NaiveDate> }`
  - `occurrences_between(&Schedule, Option<NaiveDate>, NaiveDate) -> Vec<NaiveDate>`
  - `next_occurrence(&Schedule, NaiveDate) -> Option<NaiveDate>`

- [ ] **Step 1: モジュールを登録する**

`src-tauri/src/domain/mod.rs` の `pub mod report;` の下に 1 行足す:

```rust
pub mod recurring;
```

- [ ] **Step 2: 失敗するテストを書く**

`src-tauri/src/domain/recurring.rs` を新規作成し、**テストだけ**書く:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn monthly(day: u32, starts_on: NaiveDate, ends_on: Option<NaiveDate>) -> Schedule {
        Schedule {
            frequency: Frequency::Monthly,
            day_of_month: Some(day),
            day_of_week: None,
            starts_on,
            ends_on,
        }
    }

    #[test]
    fn monthly_day_31_clamps_to_the_last_day_of_short_months() {
        let s = monthly(31, d(2026, 1, 1), None);
        assert_eq!(
            occurrences_between(&s, None, d(2026, 4, 30)),
            vec![d(2026, 1, 31), d(2026, 2, 28), d(2026, 3, 31), d(2026, 4, 30)]
        );
    }

    #[test]
    fn monthly_day_31_hits_february_29_in_a_leap_year() {
        let s = monthly(31, d(2024, 2, 1), None);
        assert_eq!(occurrences_between(&s, None, d(2024, 2, 29)), vec![d(2024, 2, 29)]);
    }

    #[test]
    fn yearly_takes_its_month_from_starts_on_and_clamps_leap_day() {
        let s = Schedule {
            frequency: Frequency::Yearly,
            day_of_month: Some(29),
            day_of_week: None,
            starts_on: d(2024, 2, 29),
            ends_on: None,
        };
        assert_eq!(
            occurrences_between(&s, None, d(2026, 12, 31)),
            vec![d(2024, 2, 29), d(2025, 2, 28), d(2026, 2, 28)]
        );
    }

    #[test]
    fn weekly_starts_at_the_first_matching_weekday_on_or_after_the_window() {
        // day_of_week = 1 (月曜)。2026-01-01 は木曜なので最初の月曜は 1/5。
        let s = Schedule {
            frequency: Frequency::Weekly,
            day_of_month: None,
            day_of_week: Some(1),
            starts_on: d(2026, 1, 1),
            ends_on: None,
        };
        assert_eq!(
            occurrences_between(&s, None, d(2026, 1, 26)),
            vec![d(2026, 1, 5), d(2026, 1, 12), d(2026, 1, 19), d(2026, 1, 26)]
        );
    }

    #[test]
    fn the_window_is_open_on_the_left_and_closed_on_the_right() {
        let s = monthly(10, d(2026, 1, 1), None);
        // 2/10 は after と同日なので出ない。3/10 は through と同日なので出る。
        assert_eq!(
            occurrences_between(&s, Some(d(2026, 2, 10)), d(2026, 3, 10)),
            vec![d(2026, 3, 10)]
        );
    }

    #[test]
    fn ends_on_cuts_the_series_off() {
        let s = monthly(10, d(2026, 1, 1), Some(d(2026, 2, 15)));
        assert_eq!(
            occurrences_between(&s, None, d(2026, 12, 31)),
            vec![d(2026, 1, 10), d(2026, 2, 10)]
        );
    }

    #[test]
    fn nothing_is_generated_before_starts_on() {
        let s = monthly(10, d(2026, 3, 1), None);
        assert_eq!(
            occurrences_between(&s, None, d(2026, 3, 31)),
            vec![d(2026, 3, 10)]
        );
    }

    #[test]
    fn next_occurrence_looks_past_the_end_of_the_window() {
        let s = monthly(31, d(2026, 1, 1), None);
        assert_eq!(next_occurrence(&s, d(2026, 1, 31)), Some(d(2026, 2, 28)));
    }

    #[test]
    fn next_occurrence_is_none_after_ends_on() {
        let s = monthly(10, d(2026, 1, 1), Some(d(2026, 2, 15)));
        assert_eq!(next_occurrence(&s, d(2026, 2, 10)), None);
    }

    #[test]
    fn frequency_parses_and_rejects_unknown_values() {
        assert_eq!(Frequency::parse("monthly").unwrap(), Frequency::Monthly);
        assert_eq!(Frequency::parse("weekly").unwrap(), Frequency::Weekly);
        assert_eq!(Frequency::parse("yearly").unwrap(), Frequency::Yearly);
        assert!(Frequency::parse("daily").is_err());
    }
}
```

- [ ] **Step 3: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --lib domain::recurring`
Expected: FAIL — コンパイルエラー(`Schedule` / `occurrences_between` などが未定義)

- [ ] **Step 4: 実装を書く**

`src-tauri/src/domain/recurring.rs` の**先頭**に足す(テストモジュールはそのまま下に残す):

```rust
//! 定期取引ルールの日付列挙。
//!
//! ここは純粋関数だけを置く。時計 (`Local::now()`) も DB も触らない。
//! 「いつまで生成するか」は呼び出し側が `through` として渡す。
//!
//! 窓は左開右閉 `(after, through]`。`after` は `recurring_rules.last_generated_on`
//! で、これにより
//! `occurrences_between(s, a, c) == occurrences_between(s, a, b) ++ occurrences_between(s, b, c)`
//! が成り立つ。アプリを連続起動しても取引が二重生成されないのはこの性質のため。

use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Monthly,
    Weekly,
    Yearly,
}

impl Frequency {
    pub fn as_sql(self) -> &'static str {
        match self {
            Frequency::Monthly => "monthly",
            Frequency::Weekly => "weekly",
            Frequency::Yearly => "yearly",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "monthly" => Ok(Self::Monthly),
            "weekly" => Ok(Self::Weekly),
            "yearly" => Ok(Self::Yearly),
            other => Err(AppError::InvalidArgument(format!(
                "frequency must be monthly|weekly|yearly, got '{other}'"
            ))),
        }
    }
}

/// 日付列挙に必要な項目だけを抜き出したもの。金額や口座は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}

/// `year`/`month` の末日。
fn last_day_of_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?
        .pred_opt()
        .map(|d| d.day())
}

/// その月に `day` が存在しなければ末日に寄せる。31 -> 2 月は 28/29、4 月は 30。
/// スキップも翌月繰り越しもしない (spec §5.4)。
fn clamp_day(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let last = last_day_of_month(year, month)?;
    NaiveDate::from_ymd_opt(year, month, day.clamp(1, last))
}

/// 窓 `(after, through]` の下端 (この日を含む)。
fn window_start(schedule: &Schedule, after: Option<NaiveDate>) -> Option<NaiveDate> {
    match after {
        Some(a) => Some(schedule.starts_on.max(a.checked_add_days(Days::new(1))?)),
        None => Some(schedule.starts_on),
    }
}

/// `(after, through]` に発生する日付を昇順で返す。
/// `after` が `None` なら `starts_on` から遡って全件返す。
pub fn occurrences_between(
    schedule: &Schedule,
    after: Option<NaiveDate>,
    through: NaiveDate,
) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let Some(from) = window_start(schedule, after) else {
        return out;
    };
    let last = match schedule.ends_on {
        Some(end) => through.min(end),
        None => through,
    };
    if from > last {
        return out;
    }

    match schedule.frequency {
        Frequency::Weekly => {
            let target = schedule
                .day_of_week
                .unwrap_or_else(|| schedule.starts_on.weekday().num_days_from_sunday())
                % 7;
            let current = from.weekday().num_days_from_sunday();
            let offset = u64::from((target + 7 - current) % 7);
            let Some(mut date) = from.checked_add_days(Days::new(offset)) else {
                return out;
            };
            while date <= last {
                out.push(date);
                match date.checked_add_days(Days::new(7)) {
                    Some(next) => date = next,
                    None => break,
                }
            }
        }
        Frequency::Monthly => {
            let day = schedule
                .day_of_month
                .unwrap_or_else(|| schedule.starts_on.day());
            let (mut year, mut month) = (from.year(), from.month());
            while (year, month) <= (last.year(), last.month()) {
                if let Some(date) = clamp_day(year, month, day) {
                    if date >= from && date <= last {
                        out.push(date);
                    }
                }
                if month == 12 {
                    year += 1;
                    month = 1;
                } else {
                    month += 1;
                }
            }
        }
        Frequency::Yearly => {
            let day = schedule
                .day_of_month
                .unwrap_or_else(|| schedule.starts_on.day());
            let month = schedule.starts_on.month();
            for year in from.year()..=last.year() {
                if let Some(date) = clamp_day(year, month, day) {
                    if date >= from && date <= last {
                        out.push(date);
                    }
                }
            }
        }
    }
    out
}

/// `after` より後の最初の発生日。生成は行わない (UI の「次回予定」表示専用)。
/// `ends_on` を過ぎていれば `None`。
pub fn next_occurrence(schedule: &Schedule, after: NaiveDate) -> Option<NaiveDate> {
    // ends_on が無いルールでも 1 回で必ず当たる幅を取る。
    let horizon = match schedule.frequency {
        Frequency::Weekly => 14,
        Frequency::Monthly => 70,
        Frequency::Yearly => 800,
    };
    let through = schedule
        .starts_on
        .max(after)
        .checked_add_days(Days::new(horizon))?;
    occurrences_between(schedule, Some(after), through)
        .into_iter()
        .next()
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cd src-tauri && cargo test --lib domain::recurring`
Expected: PASS (10 tests)

- [ ] **Step 6: proptest を書く**

`src-tauri/src/domain/recurring.rs` の末尾に足す:

```rust
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    const EPOCH: (i32, u32, u32) = (2020, 1, 1);

    fn epoch() -> NaiveDate {
        NaiveDate::from_ymd_opt(EPOCH.0, EPOCH.1, EPOCH.2).unwrap()
    }

    fn plus(base: NaiveDate, days: u64) -> NaiveDate {
        base.checked_add_days(Days::new(days)).unwrap()
    }

    prop_compose! {
        fn arb_schedule()(
            freq_index in 0usize..3,
            day_of_month in 1u32..=31,
            day_of_week in 0u32..7,
            start_offset in 0u64..2000,
            span in prop::option::of(0u64..2000),
        ) -> Schedule {
            let starts_on = plus(epoch(), start_offset);
            Schedule {
                frequency: [Frequency::Monthly, Frequency::Weekly, Frequency::Yearly][freq_index],
                day_of_month: Some(day_of_month),
                day_of_week: Some(day_of_week),
                starts_on,
                ends_on: span.map(|s| plus(starts_on, s)),
            }
        }
    }

    proptest! {
        /// 「アプリを連続起動しても二重生成されない」の本体。
        /// 窓を途中で切っても、通しで取っても、同じ日付列になる。
        #[test]
        fn splitting_the_window_yields_the_same_dates(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
            b_gap in 0u64..1500,
            c_gap in 0u64..1500,
        ) {
            let a = plus(epoch(), a_offset);
            let b = plus(a, b_gap);
            let c = plus(b, c_gap);

            let whole = occurrences_between(&schedule, Some(a), c);
            let mut split = occurrences_between(&schedule, Some(a), b);
            split.extend(occurrences_between(&schedule, Some(b), c));

            prop_assert_eq!(whole, split);
        }

        /// 同じ日に 2 回展開しても 2 回目は何も出ない。
        #[test]
        fn an_empty_window_yields_nothing(schedule in arb_schedule(), offset in 0u64..3000) {
            let day = plus(epoch(), offset);
            prop_assert!(occurrences_between(&schedule, Some(day), day).is_empty());
        }

        #[test]
        fn every_date_is_inside_the_window_and_strictly_increasing(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
            gap in 0u64..2000,
        ) {
            let after = plus(epoch(), a_offset);
            let through = plus(after, gap);
            let dates = occurrences_between(&schedule, Some(after), through);

            for pair in dates.windows(2) {
                prop_assert!(pair[0] < pair[1]);
            }
            for date in &dates {
                prop_assert!(*date > after);
                prop_assert!(*date <= through);
                prop_assert!(*date >= schedule.starts_on);
                if let Some(end) = schedule.ends_on {
                    prop_assert!(*date <= end);
                }
            }
        }

        /// next_occurrence は「窓を十分広く取ったときの先頭」と一致する。
        #[test]
        fn next_occurrence_agrees_with_a_wide_window(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
        ) {
            let after = plus(epoch(), a_offset);
            let wide = plus(schedule.starts_on.max(after), 1200);
            let expected = occurrences_between(&schedule, Some(after), wide)
                .into_iter()
                .next();
            prop_assert_eq!(next_occurrence(&schedule, after), expected);
        }
    }
}
```

- [ ] **Step 7: proptest が通ることを確認する**

Run: `cd src-tauri && cargo test --lib domain::recurring`
Expected: PASS (14 tests)

反例が出た場合は実装を直す。テストを緩めてはならない — `splitting_the_window_yields_the_same_dates` が落ちるのは冪等性が壊れているということ。

- [ ] **Step 8: clippy を通してコミットする**

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cd ..
git add src-tauri/src/domain/recurring.rs src-tauri/src/domain/mod.rs
git commit -m "feat: enumerate recurring occurrence dates as a pure function

窓を左開右閉 (last_generated_on, today] にすることで、窓を分割しても
通しでも同じ日付列になる。これが起動のたびに展開しても取引が二重生成
されないことの根拠なので、proptest で分割不変性を直接押さえている。

月末は spec §5.4 の決定どおり末日へクランプする (31 -> 2 月は 28/29)。
スキップや翌月繰り越しにすると毎月あるはずの家賃が歯抜けになる。"
```

---

### Task 2: マイグレーションと recurring_rules リポジトリ

**Files:**
- Create: `src-tauri/migrations/V005__recurring_transaction_index.sql`
- Create: `src-tauri/src/infra/repo/recurring_repo.rs`
- Create: `src-tauri/tests/integration_recurring.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`
- Modify: `src-tauri/src/domain/recurring.rs`(`RecurringRule` を追加)

**Interfaces:**
- Consumes: Task 1 の `Frequency` / `Schedule`、`crate::domain::ledger::TxType`、`crate::domain::date::parse_iso_date`
- Produces:
  - `domain::recurring::RecurringRule { id, name, type_, amount, account_id, counter_account_id, category_id, description, frequency, day_of_month, day_of_week, starts_on, ends_on, last_generated_on, active }`
  - `RecurringRule::schedule(&self) -> AppResult<Schedule>`
  - `recurring_repo::{InsertInput, UpdateInput, list, find_by_id, insert, update, set_active, set_last_generated_on, insert_generated}`

- [ ] **Step 1: マイグレーションを追加する**

`src-tauri/migrations/V005__recurring_transaction_index.sql` を新規作成:

```sql
-- =========================================================
-- V005: index transactions by the rule that generated them
-- =========================================================
-- Recurring 画面が「このルールから生成された取引」を引くため。
-- recurring_rules 自体は V001 で作成済み。
-- =========================================================

CREATE INDEX idx_tx_recurring ON transactions(recurring_id);
```

マイグレーションは `include_dir!` で自動収集されるので、Rust 側の登録は不要。

- [ ] **Step 2: リポジトリモジュールを登録する**

`src-tauri/src/infra/repo/mod.rs` の `pub mod meta_repo;` の下に足す:

```rust
pub mod recurring_repo;
```

- [ ] **Step 3: 失敗するテストを書く**

`src-tauri/tests/integration_recurring.rs` を新規作成:

```rust
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::domain::recurring::{Frequency, RecurringRule};
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::recurring_repo;
use chrono::NaiveDate;
use rusqlite::{params, Connection};

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn fresh() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    conn
}

/// (account_id, counter_account_id, expense_category_id) を作る。
fn seed(conn: &Connection) -> (i64, i64, i64) {
    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES ('現金', 'cash', 'JPY', 0, 0, '', ?1, ?1)",
        params![NOW],
    )
    .unwrap();
    let account_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES ('普通預金', 'bank', 'JPY', 0, 1, '', ?1, ?1)",
        params![NOW],
    )
    .unwrap();
    let counter_account_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO categories(name, type, color, icon, display_order)
         VALUES ('家賃', 'expense', NULL, NULL, 0)",
        [],
    )
    .unwrap();
    let category_id = conn.last_insert_rowid();

    (account_id, counter_account_id, category_id)
}

fn monthly_rent(account_id: i64, category_id: i64) -> recurring_repo::InsertInput<'static> {
    recurring_repo::InsertInput {
        name: "家賃",
        type_: TxType::Expense,
        amount: 85_000,
        account_id,
        counter_account_id: None,
        category_id: Some(category_id),
        description: "毎月の家賃",
        frequency: Frequency::Monthly,
        day_of_month: Some(27),
        day_of_week: None,
        starts_on: "2026-01-27",
        ends_on: None,
    }
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn insert_then_find_round_trips_every_column() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);

    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    assert_eq!(rule.name, "家賃");
    assert_eq!(rule.type_, TxType::Expense);
    assert_eq!(rule.amount, 85_000);
    assert_eq!(rule.account_id, account_id);
    assert_eq!(rule.counter_account_id, None);
    assert_eq!(rule.category_id, Some(category_id));
    assert_eq!(rule.description, "毎月の家賃");
    assert_eq!(rule.frequency, Frequency::Monthly);
    assert_eq!(rule.day_of_month, Some(27));
    assert_eq!(rule.day_of_week, None);
    assert_eq!(rule.starts_on, "2026-01-27");
    assert_eq!(rule.ends_on, None);
    assert_eq!(rule.last_generated_on, None);
    assert!(rule.active);
}

#[test]
fn list_hides_inactive_rules_unless_asked() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, id, false).unwrap();

    assert!(recurring_repo::list(&conn, false).unwrap().is_empty());
    assert_eq!(recurring_repo::list(&conn, true).unwrap().len(), 1);
}

#[test]
fn set_active_never_deletes_the_row() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, id, false).unwrap();

    let rule = recurring_repo::find_by_id(&conn, id).unwrap();
    assert!(!rule.active);
}

#[test]
fn update_replaces_the_schedule_but_keeps_last_generated_on() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    recurring_repo::set_last_generated_on(&conn, id, "2026-03-27").unwrap();

    recurring_repo::update(
        &conn,
        id,
        &recurring_repo::UpdateInput {
            name: "家賃 (改定後)",
            type_: TxType::Expense,
            amount: 90_000,
            account_id,
            counter_account_id: None,
            category_id: Some(category_id),
            description: "",
            frequency: Frequency::Monthly,
            day_of_month: Some(1),
            day_of_week: None,
            starts_on: "2026-01-27",
            ends_on: Some("2027-01-27"),
        },
    )
    .unwrap();

    let rule = recurring_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(rule.amount, 90_000);
    assert_eq!(rule.day_of_month, Some(1));
    assert_eq!(rule.ends_on.as_deref(), Some("2027-01-27"));
    // 過去に生成した分は動かさない (spec §5.4「ルール変更時の挙動」)。
    assert_eq!(rule.last_generated_on.as_deref(), Some("2026-03-27"));
}

#[test]
fn insert_generated_links_each_row_back_to_its_rule() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    let written = recurring_repo::insert_generated(
        &conn,
        &rule,
        &[date(2026, 1, 27), date(2026, 2, 27)],
        NOW,
    )
    .unwrap();

    assert_eq!(written, 2);
    let rows: Vec<(String, i64, Option<i64>)> = conn
        .prepare("SELECT occurred_on, amount, recurring_id FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![
            ("2026-01-27".to_string(), 85_000, Some(id)),
            ("2026-02-27".to_string(), 85_000, Some(id)),
        ]
    );
}

#[test]
fn insert_generated_writes_transfer_rows_without_a_category() {
    let conn = fresh();
    let (account_id, counter_account_id, _) = seed(&conn);
    let id = recurring_repo::insert(
        &conn,
        &recurring_repo::InsertInput {
            name: "貯金",
            type_: TxType::Transfer,
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: "",
            frequency: Frequency::Monthly,
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25",
            ends_on: None,
        },
    )
    .unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    recurring_repo::insert_generated(&conn, &rule, &[date(2026, 1, 25)], NOW).unwrap();

    let (type_, counter, category): (String, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT type, counter_account_id, category_id FROM transactions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(type_, "transfer");
    assert_eq!(counter, Some(counter_account_id));
    assert_eq!(category, None);
}

#[test]
fn schedule_parses_the_stored_iso_dates() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule: RecurringRule = recurring_repo::find_by_id(&conn, id).unwrap();

    let schedule = rule.schedule().unwrap();
    assert_eq!(schedule.starts_on, date(2026, 1, 27));
    assert_eq!(schedule.ends_on, None);
    assert_eq!(schedule.frequency, Frequency::Monthly);
}
```

- [ ] **Step 4: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: FAIL — コンパイルエラー(`recurring_repo` が未定義)

- [ ] **Step 5: `RecurringRule` を domain に足す**

`src-tauri/src/domain/recurring.rs` の `Schedule` の定義の**下**に足す:

```rust
/// `recurring_rules` の 1 行。Tauri のレスポンスとしてそのまま返す。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringRule {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: crate::domain::ledger::TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub last_generated_on: Option<String>,
    pub active: bool,
}

impl RecurringRule {
    /// 保存済みの ISO 日付を解釈して日付列挙用の `Schedule` にする。
    pub fn schedule(&self) -> AppResult<Schedule> {
        let starts_on = crate::domain::date::parse_iso_date("starts_on", &self.starts_on)?;
        let ends_on = match &self.ends_on {
            Some(raw) => Some(crate::domain::date::parse_iso_date("ends_on", raw)?),
            None => None,
        };
        Ok(Schedule {
            frequency: self.frequency,
            day_of_month: self.day_of_month,
            day_of_week: self.day_of_week,
            starts_on,
            ends_on,
        })
    }

    /// `last_generated_on` を窓の左端 (排他) として解釈する。未生成なら `None`。
    pub fn generated_through(&self) -> AppResult<Option<NaiveDate>> {
        match &self.last_generated_on {
            Some(raw) => Ok(Some(crate::domain::date::parse_iso_date(
                "last_generated_on",
                raw,
            )?)),
            None => Ok(None),
        }
    }
}
```

- [ ] **Step 6: リポジトリを実装する**

`src-tauri/src/infra/repo/recurring_repo.rs` を新規作成:

```rust
//! `recurring_rules` の SQL と、ルールから生成した取引の一括 INSERT。
//!
//! 規約どおり物理削除はしない。停止は `active = 0`。

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::ledger::TxType;
use crate::domain::recurring::{Frequency, RecurringRule};
use crate::error::{AppError, AppResult};

const COLUMNS: &str = "id, name, type, amount, account_id, counter_account_id, category_id,
                       description, frequency, day_of_month, day_of_week, starts_on, ends_on,
                       last_generated_on, active";

fn conversion_error(what: &str, raw: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("unknown {what} '{raw}'").into(),
    )
}

fn small_uint(row: &rusqlite::Row<'_>, name: &str) -> rusqlite::Result<Option<u32>> {
    let raw: Option<i64> = row.get(name)?;
    Ok(raw.and_then(|v| u32::try_from(v).ok()))
}

fn row_to_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecurringRule> {
    let type_raw: String = row.get("type")?;
    let type_ = match type_raw.as_str() {
        "income" => TxType::Income,
        "expense" => TxType::Expense,
        "transfer" => TxType::Transfer,
        other => return Err(conversion_error("tx type", other)),
    };
    let frequency_raw: String = row.get("frequency")?;
    let frequency = match frequency_raw.as_str() {
        "monthly" => Frequency::Monthly,
        "weekly" => Frequency::Weekly,
        "yearly" => Frequency::Yearly,
        other => return Err(conversion_error("frequency", other)),
    };
    let active: i64 = row.get("active")?;

    Ok(RecurringRule {
        id: row.get("id")?,
        name: row.get("name")?,
        type_,
        amount: row.get("amount")?,
        account_id: row.get("account_id")?,
        counter_account_id: row.get("counter_account_id")?,
        category_id: row.get("category_id")?,
        description: row.get("description")?,
        frequency,
        day_of_month: small_uint(row, "day_of_month")?,
        day_of_week: small_uint(row, "day_of_week")?,
        starts_on: row.get("starts_on")?,
        ends_on: row.get("ends_on")?,
        last_generated_on: row.get("last_generated_on")?,
        active: active != 0,
    })
}

pub struct InsertInput<'a> {
    pub name: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

pub struct UpdateInput<'a> {
    pub name: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

fn constraint_error(e: rusqlite::Error) -> AppError {
    match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::InvalidArgument("recurring rule violates database constraints".into())
        }
        other => AppError::Db(other),
    }
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, 1)",
        params![
            input.name,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.counter_account_id,
            input.category_id,
            input.description,
            input.frequency.as_sql(),
            input.day_of_month.map(i64::from),
            input.day_of_week.map(i64::from),
            input.starts_on,
            input.ends_on,
        ],
    )
    .map_err(constraint_error)?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &UpdateInput<'_>) -> AppResult<()> {
    let changed = conn
        .execute(
            "UPDATE recurring_rules
                SET name = ?2, type = ?3, amount = ?4, account_id = ?5,
                    counter_account_id = ?6, category_id = ?7, description = ?8,
                    frequency = ?9, day_of_month = ?10, day_of_week = ?11,
                    starts_on = ?12, ends_on = ?13
              WHERE id = ?1",
            params![
                id,
                input.name,
                input.type_.as_sql(),
                input.amount,
                input.account_id,
                input.counter_account_id,
                input.category_id,
                input.description,
                input.frequency.as_sql(),
                input.day_of_month.map(i64::from),
                input.day_of_week.map(i64::from),
                input.starts_on,
                input.ends_on,
            ],
        )
        .map_err(constraint_error)?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

/// 停止 / 再開。行は消さない (規約 5)。
pub fn set_active(conn: &Connection, id: i64, active: bool) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE recurring_rules SET active = ?2 WHERE id = ?1",
        params![id, i64::from(active)],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

pub fn set_last_generated_on(conn: &Connection, id: i64, date: &str) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE recurring_rules SET last_generated_on = ?2 WHERE id = ?1",
        params![id, date],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<RecurringRule> {
    let sql = format!("SELECT {COLUMNS} FROM recurring_rules WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_rule)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("recurring rule {id}")))
}

pub fn list(conn: &Connection, include_inactive: bool) -> AppResult<Vec<RecurringRule>> {
    let filter = if include_inactive { "" } else { "WHERE active = 1" };
    let sql = format!("SELECT {COLUMNS} FROM recurring_rules {filter} ORDER BY id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_rule)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// `dates` ぶんの取引を `rule` の内容で INSERT する。呼び出し側がトランザクションを張る。
pub fn insert_generated(
    conn: &Connection,
    rule: &RecurringRule,
    dates: &[NaiveDate],
    now: &str,
) -> AppResult<usize> {
    let mut stmt = conn.prepare(
        "INSERT INTO transactions(occurred_on, type, amount, account_id, counter_account_id,
                                  category_id, description, recurring_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
    )?;
    for date in dates {
        stmt.execute(params![
            date.format("%Y-%m-%d").to_string(),
            rule.type_.as_sql(),
            rule.amount,
            rule.account_id,
            rule.counter_account_id,
            rule.category_id,
            rule.description,
            rule.id,
            now,
        ])
        .map_err(constraint_error)?;
    }
    Ok(dates.len())
}
```

- [ ] **Step 7: テストが通ることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: PASS (7 tests)

- [ ] **Step 8: 全テストと clippy を通してコミットする**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd ..
git add src-tauri/migrations/V005__recurring_transaction_index.sql \
        src-tauri/src/infra/repo/recurring_repo.rs \
        src-tauri/src/infra/repo/mod.rs \
        src-tauri/src/domain/recurring.rs \
        src-tauri/tests/integration_recurring.rs
git commit -m "feat: read and write recurring rules

recurring_rules は V001 で既に存在するので、追加するのは
transactions(recurring_id) のインデックスだけ。Recurring 画面が
ルール別の生成履歴を引くのに要る。

停止は active=0 で行い DELETE は書かない。過去の取引が
recurring_id で参照している以上、行を消すと履歴が壊れるため。"
```

---

### Task 3: 入力バリデーションとルール CRUD コマンド

**Files:**
- Modify: `src-tauri/src/domain/recurring.rs`(`RawRuleInput` / `ValidatedRule` / `validate_rule_input` を追加)
- Create: `src-tauri/src/commands/recurring.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/infra/events.rs`, `src-tauri/src/lib.rs`
- Test: `src-tauri/src/domain/recurring.rs`(`mod tests`)、`src-tauri/tests/integration_recurring.rs`

**Interfaces:**
- Consumes: Task 1 の `Frequency`/`Schedule`/`next_occurrence`、Task 2 の `RecurringRule`/`recurring_repo`、`domain::ledger::{TxType, assert_account_writable, assert_category_matches_tx}`
- Produces:
  - `domain::recurring::{RawRuleInput, ValidatedRule, validate_rule_input}`
  - `commands::recurring::{RecurringRuleInput, RecurringRuleView, list_recurring_rules, create_recurring_rule, update_recurring_rule, set_recurring_rule_active}`
  - `commands::recurring::{create_rule_for_conn, update_rule_for_conn, list_rule_views_for_conn}`(テスト用の接続直渡し版)
  - `infra::events::ChangedDomain::Recurring`

- [ ] **Step 1: バリデーションの失敗するテストを書く**

`src-tauri/src/domain/recurring.rs` の `mod tests` の中に足す:

```rust
    fn raw_expense() -> RawRuleInput<'static> {
        RawRuleInput {
            name: "家賃",
            type_: "expense",
            amount: 85_000,
            account_id: 1,
            counter_account_id: None,
            category_id: Some(2),
            description: "",
            frequency: "monthly",
            day_of_month: Some(27),
            day_of_week: None,
            starts_on: "2026-01-27",
            ends_on: None,
        }
    }

    #[test]
    fn a_well_formed_monthly_expense_validates() {
        let v = validate_rule_input(&raw_expense()).unwrap();
        assert_eq!(v.type_, crate::domain::ledger::TxType::Expense);
        assert_eq!(v.frequency, Frequency::Monthly);
        assert_eq!(v.starts_on, d(2026, 1, 27));
        assert_eq!(v.starts_on_key(), "2026-01-27");
        assert_eq!(v.ends_on_key(), None);
    }

    #[test]
    fn name_must_not_be_blank() {
        let raw = RawRuleInput { name: "   ", ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());
    }

    #[test]
    fn amount_must_be_positive() {
        for amount in [0, -1] {
            let raw = RawRuleInput { amount, ..raw_expense() };
            assert!(validate_rule_input(&raw).is_err(), "amount {amount}");
        }
    }

    #[test]
    fn monthly_requires_day_of_month_in_range_and_no_weekday() {
        let missing = RawRuleInput { day_of_month: None, ..raw_expense() };
        assert!(validate_rule_input(&missing).is_err());

        let out_of_range = RawRuleInput { day_of_month: Some(32), ..raw_expense() };
        assert!(validate_rule_input(&out_of_range).is_err());

        let stray_weekday = RawRuleInput { day_of_week: Some(3), ..raw_expense() };
        assert!(validate_rule_input(&stray_weekday).is_err());
    }

    #[test]
    fn weekly_requires_day_of_week_in_range_and_no_day_of_month() {
        let base = RawRuleInput { frequency: "weekly", day_of_month: None, ..raw_expense() };

        assert!(validate_rule_input(&RawRuleInput { day_of_week: None, ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput { day_of_week: Some(7), ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput {
            day_of_week: Some(1),
            day_of_month: Some(5),
            ..base
        })
        .is_err());
        assert!(validate_rule_input(&RawRuleInput { day_of_week: Some(1), ..base }).is_ok());
    }

    #[test]
    fn income_and_expense_require_a_category_and_reject_a_counter_account() {
        let no_category = RawRuleInput { category_id: None, ..raw_expense() };
        assert!(validate_rule_input(&no_category).is_err());

        let with_counter = RawRuleInput { counter_account_id: Some(3), ..raw_expense() };
        assert!(validate_rule_input(&with_counter).is_err());
    }

    #[test]
    fn transfer_requires_a_distinct_counter_account_and_no_category() {
        let base = RawRuleInput { type_: "transfer", category_id: None, ..raw_expense() };

        assert!(validate_rule_input(&RawRuleInput { counter_account_id: None, ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput { counter_account_id: Some(1), ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput {
            counter_account_id: Some(3),
            category_id: Some(2),
            ..base
        })
        .is_err());
        assert!(validate_rule_input(&RawRuleInput { counter_account_id: Some(3), ..base }).is_ok());
    }

    #[test]
    fn ends_on_must_not_precede_starts_on() {
        let raw = RawRuleInput { ends_on: Some("2026-01-26"), ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());

        let same_day = RawRuleInput { ends_on: Some("2026-01-27"), ..raw_expense() };
        assert!(validate_rule_input(&same_day).is_ok());
    }

    #[test]
    fn dates_must_be_canonical_iso() {
        let raw = RawRuleInput { starts_on: "2026-1-27", ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());
    }
```

`RawRuleInput` は `..` 構文で使うので `#[derive(Clone, Copy)]` を付けること。

- [ ] **Step 2: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --lib domain::recurring`
Expected: FAIL — `RawRuleInput` / `validate_rule_input` が未定義

- [ ] **Step 3: バリデーションを実装する**

`src-tauri/src/domain/recurring.rs` の `RecurringRule` の下に足す:

```rust
const MAX_NAME_LEN: usize = 100;
const MAX_DESCRIPTION_LEN: usize = 200;

#[derive(Debug, Clone, Copy)]
pub struct RawRuleInput<'a> {
    pub name: &'a str,
    pub type_: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: &'a str,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ValidatedRule {
    pub name: String,
    pub type_: crate::domain::ledger::TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}

impl ValidatedRule {
    pub fn schedule(&self) -> Schedule {
        Schedule {
            frequency: self.frequency,
            day_of_month: self.day_of_month,
            day_of_week: self.day_of_week,
            starts_on: self.starts_on,
            ends_on: self.ends_on,
        }
    }

    pub fn starts_on_key(&self) -> String {
        self.starts_on.format("%Y-%m-%d").to_string()
    }

    pub fn ends_on_key(&self) -> Option<String> {
        self.ends_on.map(|d| d.format("%Y-%m-%d").to_string())
    }
}

/// ルール入力の形を検証する。口座やカテゴリが実在するか / アーカイブ済みかは
/// DB を見ないと分からないので、ここではなくコマンド層で確かめる。
pub fn validate_rule_input(raw: &RawRuleInput<'_>) -> AppResult<ValidatedRule> {
    let name = raw.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidArgument("name must not be blank".into()));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    if raw.description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(AppError::InvalidArgument(format!(
            "description must be {MAX_DESCRIPTION_LEN} chars or fewer"
        )));
    }
    if raw.amount <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be positive, got {}",
            raw.amount
        )));
    }

    let type_ = crate::domain::ledger::TxType::parse(raw.type_)?;
    match type_ {
        crate::domain::ledger::TxType::Transfer => {
            let counter = raw.counter_account_id.ok_or_else(|| {
                AppError::InvalidArgument("transfer rules require a counter_account_id".into())
            })?;
            if counter == raw.account_id {
                return Err(AppError::InvalidArgument(
                    "transfer rules require two different accounts".into(),
                ));
            }
            if raw.category_id.is_some() {
                return Err(AppError::InvalidArgument(
                    "transfer rules must not carry a category_id".into(),
                ));
            }
        }
        _ => {
            if raw.category_id.is_none() {
                return Err(AppError::InvalidArgument(
                    "income/expense rules require a category_id".into(),
                ));
            }
            if raw.counter_account_id.is_some() {
                return Err(AppError::InvalidArgument(
                    "income/expense rules must not carry a counter_account_id".into(),
                ));
            }
        }
    }

    let frequency = Frequency::parse(raw.frequency)?;
    match frequency {
        Frequency::Monthly | Frequency::Yearly => {
            let day = raw.day_of_month.ok_or_else(|| {
                AppError::InvalidArgument(format!(
                    "{} rules require a day_of_month",
                    frequency.as_sql()
                ))
            })?;
            if !(1..=31).contains(&day) {
                return Err(AppError::InvalidArgument(format!(
                    "day_of_month must be 1..=31, got {day}"
                )));
            }
            if raw.day_of_week.is_some() {
                return Err(AppError::InvalidArgument(format!(
                    "{} rules must not carry a day_of_week",
                    frequency.as_sql()
                )));
            }
        }
        Frequency::Weekly => {
            let day = raw.day_of_week.ok_or_else(|| {
                AppError::InvalidArgument("weekly rules require a day_of_week".into())
            })?;
            if day > 6 {
                return Err(AppError::InvalidArgument(format!(
                    "day_of_week must be 0..=6 (0 = Sunday), got {day}"
                )));
            }
            if raw.day_of_month.is_some() {
                return Err(AppError::InvalidArgument(
                    "weekly rules must not carry a day_of_month".into(),
                ));
            }
        }
    }

    let starts_on = crate::domain::date::parse_iso_date("starts_on", raw.starts_on)?;
    let ends_on = match raw.ends_on {
        Some(value) => Some(crate::domain::date::parse_iso_date("ends_on", value)?),
        None => None,
    };
    if let Some(end) = ends_on {
        if end < starts_on {
            return Err(AppError::InvalidArgument(
                "ends_on must not precede starts_on".into(),
            ));
        }
    }

    Ok(ValidatedRule {
        name: name.to_string(),
        type_,
        amount: raw.amount,
        account_id: raw.account_id,
        counter_account_id: raw.counter_account_id,
        category_id: raw.category_id,
        description: raw.description.to_string(),
        frequency,
        day_of_month: raw.day_of_month,
        day_of_week: raw.day_of_week,
        starts_on,
        ends_on,
    })
}
```

- [ ] **Step 4: バリデーションのテストが通ることを確認する**

Run: `cd src-tauri && cargo test --lib domain::recurring`
Expected: PASS (23 tests)

- [ ] **Step 5: 変更ドメインを足す**

`src-tauri/src/infra/events.rs` の `ChangedDomain` に 1 バリアント足す:

```rust
pub enum ChangedDomain {
    Categories,
    Accounts,
    Transactions,
    Budgets,
    Recurring,
    Meta,
}
```

`src-tauri/src/commands/mod.rs` の `pub mod recovery;` の下に足す:

```rust
pub mod recurring;
```

- [ ] **Step 6: コマンド層の失敗するテストを書く**

`src-tauri/tests/integration_recurring.rs` の末尾に足す:

```rust
use budget_tracker_lib::commands::recurring::{self as recurring_cmd, RecurringRuleInput};

fn input_expense(account_id: i64, category_id: i64) -> RecurringRuleInput {
    RecurringRuleInput {
        name: "家賃".into(),
        type_: "expense".into(),
        amount: 85_000,
        account_id,
        counter_account_id: None,
        category_id: Some(category_id),
        description: "毎月の家賃".into(),
        frequency: "monthly".into(),
        day_of_month: Some(27),
        day_of_week: None,
        starts_on: "2026-01-27".into(),
        ends_on: None,
    }
}

#[test]
fn create_rule_rejects_an_archived_account() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();

    let err = recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id))
        .unwrap_err();
    assert!(err.to_string().contains("archived"), "{err}");
}

#[test]
fn create_rule_rejects_a_category_of_the_wrong_type() {
    let conn = fresh();
    let (account_id, _, _) = seed(&conn);
    conn.execute(
        "INSERT INTO categories(name, type, color, icon, display_order)
         VALUES ('給与', 'income', NULL, NULL, 1)",
        [],
    )
    .unwrap();
    let income_category = conn.last_insert_rowid();

    let err = recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, income_category))
        .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
}

#[test]
fn create_rule_rejects_an_unknown_account() {
    let conn = fresh();
    let (_, _, category_id) = seed(&conn);

    assert!(recurring_cmd::create_rule_for_conn(&conn, input_expense(9_999, category_id)).is_err());
}

#[test]
fn list_views_carry_the_next_occurrence() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let views = recurring_cmd::list_rule_views_for_conn(&conn, false, date(2026, 2, 1)).unwrap();

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].next_occurrence.as_deref(), Some("2026-02-27"));
}

#[test]
fn update_rule_revalidates_the_new_shape() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let broken = RecurringRuleInput {
        day_of_month: Some(40),
        ..input_expense(account_id, category_id)
    };
    assert!(recurring_cmd::update_rule_for_conn(&conn, rule.id, broken).is_err());

    let fixed = RecurringRuleInput {
        amount: 90_000,
        ..input_expense(account_id, category_id)
    };
    let updated = recurring_cmd::update_rule_for_conn(&conn, rule.id, fixed).unwrap();
    assert_eq!(updated.amount, 90_000);
}
```

`RecurringRuleInput` を `..` で使うので `#[derive(Clone)]` を付けること。

- [ ] **Step 7: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: FAIL — `commands::recurring` が未定義

- [ ] **Step 8: コマンド層を実装する**

`src-tauri/src/commands/recurring.rs` を新規作成:

```rust
//! 定期取引ルールの CRUD と起動時展開。
//!
//! spec §5.4 のとおり、展開は `setup()` ではなくフロントからの明示コマンドで走らせる。
//! 生成件数とスキップ理由を UI に返せること、展開の失敗が起動を止めないことが理由。

use chrono::NaiveDate;
use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::ledger::{assert_account_writable, assert_category_matches_tx};
use crate::domain::recurring::{
    self, RawRuleInput, RecurringRule, ValidatedRule,
};
use crate::error::AppResult;
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::{account_repo, category_repo, recurring_repo};

#[derive(Debug, Clone, Deserialize)]
pub struct RecurringRuleInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: String,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: String,
    pub ends_on: Option<String>,
}

/// 一覧の 1 行。ルール本体に「次回予定」を添える。
#[derive(Debug, Clone, Serialize)]
pub struct RecurringRuleView {
    pub rule: RecurringRule,
    pub next_occurrence: Option<String>,
}

fn validate(input: &RecurringRuleInput) -> AppResult<ValidatedRule> {
    recurring::validate_rule_input(&RawRuleInput {
        name: &input.name,
        type_: &input.type_,
        amount: input.amount,
        account_id: input.account_id,
        counter_account_id: input.counter_account_id,
        category_id: input.category_id,
        description: &input.description,
        frequency: &input.frequency,
        day_of_month: input.day_of_month,
        day_of_week: input.day_of_week,
        starts_on: &input.starts_on,
        ends_on: input.ends_on.as_deref(),
    })
}

/// 参照先の口座 / カテゴリが実在し、アーカイブされておらず、種別が噛み合うか。
fn assert_references_usable(conn: &Connection, rule: &ValidatedRule) -> AppResult<()> {
    let account = account_repo::find_by_id(conn, rule.account_id)?;
    assert_account_writable(&account, None)?;

    if let Some(counter_id) = rule.counter_account_id {
        let counter = account_repo::find_by_id(conn, counter_id)?;
        assert_account_writable(&counter, None)?;
    }

    if let Some(category_id) = rule.category_id {
        let category = category_repo::find_by_id(conn, category_id)?;
        assert_category_matches_tx(&category, rule.type_, None)?;
    }

    Ok(())
}

fn to_insert_input<'a>(rule: &'a ValidatedRule, starts_on: &'a str, ends_on: Option<&'a str>) -> recurring_repo::InsertInput<'a> {
    recurring_repo::InsertInput {
        name: &rule.name,
        type_: rule.type_,
        amount: rule.amount,
        account_id: rule.account_id,
        counter_account_id: rule.counter_account_id,
        category_id: rule.category_id,
        description: &rule.description,
        frequency: rule.frequency,
        day_of_month: rule.day_of_month,
        day_of_week: rule.day_of_week,
        starts_on,
        ends_on,
    }
}

pub fn create_rule_for_conn(
    conn: &Connection,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let validated = validate(&input)?;
    assert_references_usable(conn, &validated)?;

    let starts_on = validated.starts_on_key();
    let ends_on = validated.ends_on_key();
    let id = recurring_repo::insert(
        conn,
        &to_insert_input(&validated, &starts_on, ends_on.as_deref()),
    )?;
    recurring_repo::find_by_id(conn, id)
}

pub fn update_rule_for_conn(
    conn: &Connection,
    id: i64,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let validated = validate(&input)?;
    assert_references_usable(conn, &validated)?;

    let starts_on = validated.starts_on_key();
    let ends_on = validated.ends_on_key();
    recurring_repo::update(
        conn,
        id,
        &recurring_repo::UpdateInput {
            name: &validated.name,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            counter_account_id: validated.counter_account_id,
            category_id: validated.category_id,
            description: &validated.description,
            frequency: validated.frequency,
            day_of_month: validated.day_of_month,
            day_of_week: validated.day_of_week,
            starts_on: &starts_on,
            ends_on: ends_on.as_deref(),
        },
    )?;
    recurring_repo::find_by_id(conn, id)
}

pub fn list_rule_views_for_conn(
    conn: &Connection,
    include_inactive: bool,
    today: NaiveDate,
) -> AppResult<Vec<RecurringRuleView>> {
    let rules = recurring_repo::list(conn, include_inactive)?;
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let schedule = rule.schedule()?;
        // 「次回」は今日より後の最初の 1 件。生成は行わない。
        let next = recurring::next_occurrence(&schedule, today)
            .map(|d| d.format("%Y-%m-%d").to_string());
        out.push(RecurringRuleView {
            rule,
            next_occurrence: next,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn list_recurring_rules(
    state: State<'_, AppState>,
    include_inactive: bool,
) -> AppResult<Vec<RecurringRuleView>> {
    let today = chrono::Local::now().date_naive();
    state.with_conn(|conn| list_rule_views_for_conn(conn, include_inactive, today))
}

#[tauri::command]
pub fn create_recurring_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rule = create_rule_for_conn(&tx, input)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}

#[tauri::command]
pub fn update_recurring_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rule = update_rule_for_conn(&tx, id, input)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}

#[tauri::command]
pub fn set_recurring_rule_active(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    active: bool,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        recurring_repo::set_active(&tx, id, active)?;
        let rule = recurring_repo::find_by_id(&tx, id)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}
```

- [ ] **Step 9: コマンドを登録する**

`src-tauri/src/lib.rs` の `invoke_handler` の `commands::budgets::set_budget,` の下に足す:

```rust
            commands::recurring::list_recurring_rules,
            commands::recurring::create_recurring_rule,
            commands::recurring::update_recurring_rule,
            commands::recurring::set_recurring_rule_active,
```

- [ ] **Step 10: テストが通ることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: PASS (12 tests)

- [ ] **Step 11: clippy を通してコミットする**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd ..
git add src-tauri/src/domain/recurring.rs src-tauri/src/commands/recurring.rs \
        src-tauri/src/commands/mod.rs src-tauri/src/infra/events.rs src-tauri/src/lib.rs \
        src-tauri/tests/integration_recurring.rs
git commit -m "feat: create, edit and pause recurring rules

形の検証 (頻度と曜日/日付の噛み合わせ、振替の相手口座、ends_on の前後)
は domain の純粋関数に置き、参照先が実在してアーカイブされていないかは
DB を見ないと分からないのでコマンド層に置いた。

種別とカテゴリの整合は取引と同じ assert_category_matches_tx を通す。
ルールから生まれる取引が手入力の取引より緩い検証で入ってはいけない。"
```

---

### Task 4: 保存前の発生日プレビュー

遡及は `starts_on` から全件生成する(spec §5.4)。意図しない大量生成を防ぐのはこのプレビューなので、UI より先に用意する。

**Files:**
- Modify: `src-tauri/src/commands/recurring.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/integration_recurring.rs`

**Interfaces:**
- Consumes: Task 3 の `RecurringRuleInput` / `validate`、Task 1 の `occurrences_between` / `next_occurrence`
- Produces: `commands::recurring::{OccurrencePreview, preview_for_input, preview_recurring_occurrences}`

- [ ] **Step 1: 失敗するテストを書く**

`src-tauri/tests/integration_recurring.rs` の末尾に足す:

```rust
#[test]
fn preview_counts_the_backfill_a_past_starts_on_would_create() {
    // 2026-01-27 開始・毎月 27 日。today = 2026-05-01 なら 1〜4 月の 4 件。
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.backfill_total, 4);
    assert_eq!(
        preview.backfill,
        vec!["2026-01-27", "2026-02-27", "2026-03-27", "2026-04-27"]
    );
    assert!(!preview.truncated);
}

#[test]
fn preview_truncates_the_backfill_at_the_limit_but_keeps_the_total() {
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 2).unwrap();

    assert_eq!(preview.backfill_total, 4);
    assert_eq!(preview.backfill, vec!["2026-01-27", "2026-02-27"]);
    assert!(preview.truncated);
}

#[test]
fn preview_lists_the_next_three_upcoming_dates() {
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.upcoming, vec!["2026-05-27", "2026-06-27", "2026-07-27"]);
}

#[test]
fn preview_of_a_future_rule_has_no_backfill() {
    let future = RecurringRuleInput {
        starts_on: "2026-09-27".into(),
        ..input_expense(1, 2)
    };
    let preview = recurring_cmd::preview_for_input(&future, date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.backfill_total, 0);
    assert!(preview.backfill.is_empty());
    assert_eq!(preview.upcoming.first().map(String::as_str), Some("2026-09-27"));
}

#[test]
fn preview_rejects_a_malformed_rule_without_touching_the_database() {
    let broken = RecurringRuleInput {
        frequency: "daily".into(),
        ..input_expense(1, 2)
    };
    assert!(recurring_cmd::preview_for_input(&broken, date(2026, 5, 1), 100).is_err());
}
```

`preview_for_input` は DB を触らないので、口座 id / カテゴリ id は実在しない値(`1`, `2`)で構わない。

- [ ] **Step 2: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring preview`
Expected: FAIL — `preview_for_input` が未定義

- [ ] **Step 3: 実装する**

`src-tauri/src/commands/recurring.rs` の `RecurringRuleView` の下に足す:

```rust
/// 「今日より後」の予定を何件見せるか。
const UPCOMING_COUNT: usize = 3;

/// 保存前に見せる発生日の内訳。
#[derive(Debug, Clone, Serialize)]
pub struct OccurrencePreview {
    /// 保存した瞬間に生成される分 (starts_on から today まで)。`limit` で切られる。
    pub backfill: Vec<String>,
    /// `limit` で切る前の backfill 総数。
    pub backfill_total: i64,
    /// backfill が `limit` で切られたか。
    pub truncated: bool,
    /// today より後の予定 (最大 3 件)。生成はされない。
    pub upcoming: Vec<String>,
}

fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// 入力の形だけを見て発生日を数える。DB は触らない。
pub fn preview_for_input(
    input: &RecurringRuleInput,
    today: NaiveDate,
    limit: usize,
) -> AppResult<OccurrencePreview> {
    let schedule = validate(input)?.schedule();

    let backfill_dates = recurring::occurrences_between(&schedule, None, today);
    let backfill_total = backfill_dates.len() as i64;
    let truncated = backfill_dates.len() > limit;
    let backfill = backfill_dates.iter().take(limit).copied().map(iso).collect();

    let mut upcoming = Vec::new();
    let mut cursor = today;
    for _ in 0..UPCOMING_COUNT {
        match recurring::next_occurrence(&schedule, cursor) {
            Some(date) => {
                upcoming.push(iso(date));
                cursor = date;
            }
            None => break,
        }
    }

    Ok(OccurrencePreview {
        backfill,
        backfill_total,
        truncated,
        upcoming,
    })
}

#[tauri::command]
pub fn preview_recurring_occurrences(
    input: RecurringRuleInput,
    limit: u32,
) -> AppResult<OccurrencePreview> {
    if !(1..=500).contains(&limit) {
        return Err(crate::error::AppError::InvalidArgument(format!(
            "limit must be 1..=500, got {limit}"
        )));
    }
    let today = chrono::Local::now().date_naive();
    preview_for_input(&input, today, limit as usize)
}
```

- [ ] **Step 4: コマンドを登録する**

`src-tauri/src/lib.rs` の `commands::recurring::set_recurring_rule_active,` の下に足す:

```rust
            commands::recurring::preview_recurring_occurrences,
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: PASS (17 tests)

- [ ] **Step 6: clippy を通してコミットする**

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cd ..
git add src-tauri/src/commands/recurring.rs src-tauri/src/lib.rs \
        src-tauri/tests/integration_recurring.rs
git commit -m "feat: preview the dates a rule would generate before saving it

遡及を starts_on から全件生成する方針を選んだ以上、開始日を数年前に
打ち間違えたときの被害が大きい。保存前に「今すぐ N 件生成されます」を
出せるよう、DB を触らない純粋なプレビューを用意する。"
```

---

### Task 5: 起動時展開

**Files:**
- Modify: `src-tauri/src/commands/recurring.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/integration_recurring.rs`

**Interfaces:**
- Consumes: Task 2 の `recurring_repo`、Task 1 の `occurrences_between`、`RecurringRule::{schedule, generated_through}`
- Produces: `commands::recurring::{ExpansionResult, RuleExpansion, SkippedRule, SkipReason, expand_due_recurring_for_conn, expand_due_recurring}`

- [ ] **Step 1: 失敗するテストを書く**

`src-tauri/tests/integration_recurring.rs` の末尾に足す:

```rust
fn tx_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn expansion_generates_every_missed_date_from_starts_on() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    // 1/27, 2/27, 3/27 の 3 件。4/27 はまだ来ていない。
    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.rules[0].generated, 3);
    assert_eq!(result.rules[0].last_generated_on, "2026-03-27");
    assert!(result.skipped.is_empty());
}

#[test]
fn expanding_twice_on_the_same_day_generates_nothing_the_second_time() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();
    let second =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(second.generated, 0);
    assert!(second.rules.is_empty());
    assert_eq!(tx_count(&conn), 3);
}

#[test]
fn expanding_day_by_day_matches_one_big_catch_up() {
    let stepwise = fresh();
    let (a1, _, c1) = seed(&stepwise);
    recurring_cmd::create_rule_for_conn(&stepwise, input_expense(a1, c1)).unwrap();
    for day in 1..=90u64 {
        let today = date(2026, 1, 1)
            .checked_add_days(chrono::Days::new(day))
            .unwrap();
        recurring_cmd::expand_due_recurring_for_conn(&stepwise, today, NOW).unwrap();
    }

    let at_once = fresh();
    let (a2, _, c2) = seed(&at_once);
    recurring_cmd::create_rule_for_conn(&at_once, input_expense(a2, c2)).unwrap();
    recurring_cmd::expand_due_recurring_for_conn(
        &at_once,
        date(2026, 1, 1).checked_add_days(chrono::Days::new(90)).unwrap(),
        NOW,
    )
    .unwrap();

    let dates = |conn: &Connection| -> Vec<String> {
        conn.prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(dates(&stepwise), dates(&at_once));
}

#[test]
fn an_archived_account_skips_only_that_rule_and_holds_its_watermark() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(tx_count(&conn), 0);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, rule.id);
    // 見送った期間は次回に持ち越す。
    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on, None);
}

#[test]
fn unarchiving_lets_the_next_expansion_backfill_the_held_period() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    conn.execute(
        "UPDATE accounts SET archived_at = NULL WHERE id = ?1",
        params![account_id],
    )
    .unwrap();
    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
}

#[test]
fn an_archived_category_skips_the_rule() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE categories SET archived_at = ?1 WHERE id = ?2",
        params![NOW, category_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(result.skipped.len(), 1);
}

#[test]
fn an_inactive_rule_is_not_expanded_at_all() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_repo::set_active(&conn, rule.id, false).unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert!(result.rules.is_empty());
    // 停止中は「要修正」ではないので警告にも出さない。
    assert!(result.skipped.is_empty());
}

#[test]
fn one_broken_rule_does_not_block_a_healthy_one() {
    let conn = fresh();
    let (account_id, counter_account_id, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_cmd::create_rule_for_conn(
        &conn,
        RecurringRuleInput {
            name: "貯金".into(),
            type_: "transfer".into(),
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: String::new(),
            frequency: "monthly".into(),
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25".into(),
            ends_on: None,
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, counter_account_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
}

#[test]
fn generated_transfers_stay_out_of_the_income_expense_totals() {
    let conn = fresh();
    let (account_id, counter_account_id, _) = seed(&conn);
    recurring_cmd::create_rule_for_conn(
        &conn,
        RecurringRuleInput {
            name: "貯金".into(),
            type_: "transfer".into(),
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: String::new(),
            frequency: "monthly".into(),
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25".into(),
            ends_on: None,
        },
    )
    .unwrap();

    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 2, 1), NOW).unwrap();

    let counted: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE type IN ('income','expense')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(counted, 0);
    assert_eq!(tx_count(&conn), 1);
}
```

- [ ] **Step 2: テストが落ちることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring expand`
Expected: FAIL — `expand_due_recurring_for_conn` が未定義

- [ ] **Step 3: 実装する**

`src-tauri/src/commands/recurring.rs` の末尾に足す:

```rust
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// ルールを展開できない理由。どれも「ユーザーが参照先を直せば解消する」もの。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    ArchivedAccount,
    ArchivedCounterAccount,
    ArchivedCategory,
    CategoryTypeMismatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedRule {
    pub rule_id: i64,
    pub rule_name: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleExpansion {
    pub rule_id: i64,
    pub rule_name: String,
    pub generated: i64,
    pub last_generated_on: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpansionResult {
    pub generated: i64,
    pub rules: Vec<RuleExpansion>,
    pub skipped: Vec<SkippedRule>,
}

/// 参照先が使えるか。使えないなら理由を返す。
fn classify_skip(conn: &Connection, rule: &RecurringRule) -> AppResult<Option<SkipReason>> {
    let account = account_repo::find_by_id(conn, rule.account_id)?;
    if account.archived_at.is_some() {
        return Ok(Some(SkipReason::ArchivedAccount));
    }

    if let Some(counter_id) = rule.counter_account_id {
        let counter = account_repo::find_by_id(conn, counter_id)?;
        if counter.archived_at.is_some() {
            return Ok(Some(SkipReason::ArchivedCounterAccount));
        }
    }

    if let Some(category_id) = rule.category_id {
        let category = category_repo::find_by_id(conn, category_id)?;
        if category.archived_at.is_some() {
            return Ok(Some(SkipReason::ArchivedCategory));
        }
        if assert_category_matches_tx(&category, rule.type_, None).is_err() {
            return Ok(Some(SkipReason::CategoryTypeMismatch));
        }
    }

    Ok(None)
}

/// `active = 1` の全ルールについて `(last_generated_on, today]` を展開する。
///
/// 参照先が使えないルールは飛ばし、`last_generated_on` も進めない。
/// ユーザーが参照先を直せば、次回展開で見送った期間が遡って埋まる。
pub fn expand_due_recurring_for_conn(
    conn: &Connection,
    today: NaiveDate,
    now: &str,
) -> AppResult<ExpansionResult> {
    let mut result = ExpansionResult {
        generated: 0,
        rules: Vec::new(),
        skipped: Vec::new(),
    };

    for rule in recurring_repo::list(conn, false)? {
        if let Some(reason) = classify_skip(conn, &rule)? {
            result.skipped.push(SkippedRule {
                rule_id: rule.id,
                rule_name: rule.name,
                reason,
            });
            continue;
        }

        let schedule = rule.schedule()?;
        let after = rule.generated_through()?;
        let dates = recurring::occurrences_between(&schedule, after, today);
        if dates.is_empty() {
            continue;
        }

        recurring_repo::insert_generated(conn, &rule, &dates, now)?;
        let last = iso(dates[dates.len() - 1]);
        recurring_repo::set_last_generated_on(conn, rule.id, &last)?;

        result.generated += dates.len() as i64;
        result.rules.push(RuleExpansion {
            rule_id: rule.id,
            rule_name: rule.name,
            generated: dates.len() as i64,
            last_generated_on: last,
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn expand_due_recurring(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ExpansionResult> {
    let today = chrono::Local::now().date_naive();
    let now = now_iso();
    let result = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = expand_due_recurring_for_conn(&tx, today, &now)?;
        tx.commit()?;
        Ok(result)
    })?;

    if result.generated > 0 {
        emit_changed(&app, ChangedDomain::Transactions);
        emit_changed(&app, ChangedDomain::Recurring);
    }
    Ok(result)
}
```

- [ ] **Step 4: コマンドを登録する**

`src-tauri/src/lib.rs` の `commands::recurring::preview_recurring_occurrences,` の下に足す:

```rust
            commands::recurring::expand_due_recurring,
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cd src-tauri && cargo test --test integration_recurring`
Expected: PASS (26 tests)

- [ ] **Step 6: 全テストと clippy を通してコミットする**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd ..
git add src-tauri/src/commands/recurring.rs src-tauri/src/lib.rs \
        src-tauri/tests/integration_recurring.rs
git commit -m "feat: expand due recurring rules idempotently on demand

窓を (last_generated_on, today] にしているので、毎日開いても半年ぶりに
開いても同じ取引列になる。日次で 90 回展開した DB と一括で 1 回展開した
DB を突き合わせるテストでそこを押さえた。

アーカイブされた参照を持つルールは last_generated_on を進めずに飛ばす。
進めてしまうと、ユーザーが参照先を直したときに見送った期間が永久に
埋まらなくなる。"
```

---

### Task 6: Rust レスポンス契約フィクスチャ

`tests/fixtures/responses/` は Rust の serde 出力をバイト比較で固定したもの。フロントの型はこのファイルに紐づくので、TypeScript より先に作る。

**Files:**
- Modify: `src-tauri/tests/response_fixtures.rs`
- Create(生成物): `tests/fixtures/responses/list_recurring_rules.json`
- Create(生成物): `tests/fixtures/responses/expand_due_recurring.json`
- Create(生成物): `tests/fixtures/responses/preview_recurring_occurrences.json`

**Interfaces:**
- Consumes: Task 3〜5 の `RecurringRuleView` / `ExpansionResult` / `OccurrencePreview` / `RecurringRule`
- Produces: 上記 JSON 3 本(Task 7 の TypeScript 型が読む)

- [ ] **Step 1: サンプルとテストを書く**

`src-tauri/tests/response_fixtures.rs` の `use` に足す:

```rust
use budget_tracker_lib::commands::recurring::{
    ExpansionResult, OccurrencePreview, RecurringRuleView, RuleExpansion, SkipReason, SkippedRule,
};
use budget_tracker_lib::domain::recurring::{Frequency, RecurringRule};
```

ファイル末尾に足す:

```rust
// ---------------------------------------------------------------------------
// 定期取引 (Phase 5a)
// ---------------------------------------------------------------------------

fn sample_rent_rule() -> RecurringRule {
    RecurringRule {
        id: 1,
        name: "家賃".into(),
        type_: TxType::Expense,
        amount: 85_000,
        account_id: 1,
        counter_account_id: None,
        category_id: Some(1),
        description: "毎月の家賃".into(),
        frequency: Frequency::Monthly,
        day_of_month: Some(27),
        day_of_week: None,
        starts_on: "2026-01-27".into(),
        ends_on: None,
        last_generated_on: Some("2026-06-27".into()),
        active: true,
    }
}

/// 停止中の週次振替。`counter_account_id` を持ち `category_id` を持たない側を
/// フィクスチャに残しておくと、TS 側の null 許容が壊れたときに落ちる。
fn sample_savings_rule() -> RecurringRule {
    RecurringRule {
        id: 2,
        name: "週次の貯金".into(),
        type_: TxType::Transfer,
        amount: 5_000,
        account_id: 2,
        counter_account_id: Some(1),
        category_id: None,
        description: String::new(),
        frequency: Frequency::Weekly,
        day_of_month: None,
        day_of_week: Some(1),
        starts_on: "2026-01-05".into(),
        ends_on: Some("2026-12-28".into()),
        last_generated_on: None,
        active: false,
    }
}

#[test]
fn list_recurring_rules() {
    check_fixture(
        "list_recurring_rules",
        &vec![
            RecurringRuleView {
                rule: sample_rent_rule(),
                next_occurrence: Some("2026-07-27".into()),
            },
            RecurringRuleView {
                rule: sample_savings_rule(),
                next_occurrence: None,
            },
        ],
    );
}

#[test]
fn expand_due_recurring() {
    check_fixture(
        "expand_due_recurring",
        &ExpansionResult {
            generated: 2,
            rules: vec![RuleExpansion {
                rule_id: 1,
                rule_name: "家賃".into(),
                generated: 2,
                last_generated_on: "2026-06-27".into(),
            }],
            skipped: vec![SkippedRule {
                rule_id: 2,
                rule_name: "週次の貯金".into(),
                reason: SkipReason::ArchivedCounterAccount,
            }],
        },
    );
}

#[test]
fn preview_recurring_occurrences() {
    check_fixture(
        "preview_recurring_occurrences",
        &OccurrencePreview {
            backfill: vec!["2026-01-27".into(), "2026-02-27".into()],
            backfill_total: 2,
            truncated: false,
            upcoming: vec![
                "2026-03-27".into(),
                "2026-04-27".into(),
                "2026-05-27".into(),
            ],
        },
    );
}
```

- [ ] **Step 2: フィクスチャが無いことを確認する**

Run: `cd src-tauri && cargo test --test response_fixtures`
Expected: FAIL — `missing response fixture .../list_recurring_rules.json`

- [ ] **Step 3: フィクスチャを生成する**

Run: `cd src-tauri && UPDATE_FIXTURES=1 cargo test --test response_fixtures`
Expected: PASS。`tests/fixtures/responses/` に 3 ファイルが増える

- [ ] **Step 4: 生成物を確認する**

Run: `cat tests/fixtures/responses/expand_due_recurring.json`
Expected: `"reason": "archived_counter_account"` が snake_case で出ていること。`"type": "expense"` / `"frequency": "monthly"` も同様

- [ ] **Step 5: 比較モードで通ることを確認する**

Run: `cd src-tauri && cargo test --test response_fixtures`
Expected: PASS(`UPDATE_FIXTURES` なしで一致)

- [ ] **Step 6: app_info のサンプルを V005 に合わせる**

`src-tauri/tests/response_fixtures.rs` の `app_info` テストの `schema_version: 4` を `5` にする。これは DB から読んだ値ではなく手書きのサンプルなので自動では落ちないが、放置すると実際のスキーマと食い違う。

Run: `cd src-tauri && UPDATE_FIXTURES=1 cargo test --test response_fixtures && cargo test --test response_fixtures`
Expected: PASS

- [ ] **Step 7: コミットする**

```bash
git add src-tauri/tests/response_fixtures.rs tests/fixtures/responses/
git commit -m "test: pin the recurring command responses to JSON fixtures

Rust の serde 出力とフロントの型がずれたら Rust 側で先に落ちるようにする。
停止中の週次振替 (counter_account_id あり / category_id なし) を混ぜて
あるのは、TS 側の null 許容が壊れたときに気づけるようにするため。"
```

---

### Task 7: フロントの API ラッパーとストア

**Files:**
- Create: `src/lib/api/recurring.ts`, `src/lib/api/recurring.test.ts`
- Create: `src/lib/stores/recurring.svelte.ts`, `src/lib/stores/recurring.test.ts`
- Modify: `src/lib/api/index.ts`, `src/lib/api/events.ts`, `src/lib/api/contract.test.ts`

**Interfaces:**
- Consumes: Task 6 のフィクスチャ 3 本、`src/lib/api/transactions.ts` の `TxType`
- Produces:
  - `api/recurring.ts`: `Frequency`, `SkipReason`, `RecurringRule`, `RecurringRuleView`, `RecurringRuleInput`, `OccurrencePreview`, `RuleExpansion`, `SkippedRule`, `ExpansionResult`, `listRecurringRules`, `createRecurringRule`, `updateRecurringRule`, `setRecurringRuleActive`, `previewRecurringOccurrences`, `expandDueRecurring`
  - `stores/recurring.svelte.ts`: `RecurringStore`, `createRecurringStore()`

- [ ] **Step 1: 失敗するテストを書く**

`src/lib/api/recurring.test.ts` を新規作成:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  createRecurringRule,
  expandDueRecurring,
  listRecurringRules,
  previewRecurringOccurrences,
  setRecurringRuleActive,
  updateRecurringRule,
  type RecurringRuleInput,
} from './recurring';

const input: RecurringRuleInput = {
  name: '家賃',
  type: 'expense',
  amount: 85_000,
  account_id: 1,
  counter_account_id: null,
  category_id: 1,
  description: '',
  frequency: 'monthly',
  day_of_month: 27,
  day_of_week: null,
  starts_on: '2026-01-27',
  ends_on: null,
};

describe('recurring api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes includeInactive through to list_recurring_rules', async () => {
    invokeMock.mockResolvedValueOnce([]);

    await listRecurringRules(true);

    expect(invokeMock).toHaveBeenCalledWith('list_recurring_rules', { includeInactive: true });
  });

  it('wraps input under input for create_recurring_rule', async () => {
    invokeMock.mockResolvedValueOnce({});

    await createRecurringRule(input);

    expect(invokeMock).toHaveBeenCalledWith('create_recurring_rule', { input });
  });

  it('passes id alongside input for update_recurring_rule', async () => {
    invokeMock.mockResolvedValueOnce({});

    await updateRecurringRule(7, input);

    expect(invokeMock).toHaveBeenCalledWith('update_recurring_rule', { id: 7, input });
  });

  it('passes id and active for set_recurring_rule_active', async () => {
    invokeMock.mockResolvedValueOnce({});

    await setRecurringRuleActive(7, false);

    expect(invokeMock).toHaveBeenCalledWith('set_recurring_rule_active', { id: 7, active: false });
  });

  it('passes input and limit for preview_recurring_occurrences', async () => {
    invokeMock.mockResolvedValueOnce({ backfill: [], backfill_total: 0, truncated: false, upcoming: [] });

    await previewRecurringOccurrences(input, 100);

    expect(invokeMock).toHaveBeenCalledWith('preview_recurring_occurrences', { input, limit: 100 });
  });

  it('takes no arguments for expand_due_recurring', async () => {
    invokeMock.mockResolvedValueOnce({ generated: 0, rules: [], skipped: [] });

    await expandDueRecurring();

    expect(invokeMock).toHaveBeenCalledWith('expand_due_recurring');
  });
});
```

- [ ] **Step 2: テストが落ちることを確認する**

Run: `pnpm test src/lib/api/recurring.test.ts`
Expected: FAIL — `./recurring` が存在しない

- [ ] **Step 3: API ラッパーを実装する**

`src/lib/api/recurring.ts` を新規作成:

```ts
import { invoke } from '@tauri-apps/api/core';

import type { TxType } from './transactions';

export type Frequency = 'monthly' | 'weekly' | 'yearly';

export type SkipReason =
  | 'archived_account'
  | 'archived_counter_account'
  | 'archived_category'
  | 'category_type_mismatch';

export type RecurringRule = {
  id: number;
  name: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  frequency: Frequency;
  day_of_month: number | null;
  day_of_week: number | null;
  starts_on: string;
  ends_on: string | null;
  last_generated_on: string | null;
  active: boolean;
};

/** 一覧の 1 行。`next_occurrence` は表示専用で、取引は生成されていない。 */
export type RecurringRuleView = {
  rule: RecurringRule;
  next_occurrence: string | null;
};

export type RecurringRuleInput = {
  name: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  frequency: Frequency;
  day_of_month: number | null;
  day_of_week: number | null;
  starts_on: string;
  ends_on: string | null;
};

/** 保存した瞬間に何件生成されるかを事前に見せるためのもの。 */
export type OccurrencePreview = {
  backfill: string[];
  backfill_total: number;
  truncated: boolean;
  upcoming: string[];
};

export type RuleExpansion = {
  rule_id: number;
  rule_name: string;
  generated: number;
  last_generated_on: string;
};

export type SkippedRule = {
  rule_id: number;
  rule_name: string;
  reason: SkipReason;
};

export type ExpansionResult = {
  generated: number;
  rules: RuleExpansion[];
  skipped: SkippedRule[];
};

export function listRecurringRules(includeInactive: boolean): Promise<RecurringRuleView[]> {
  return invoke<RecurringRuleView[]>('list_recurring_rules', { includeInactive });
}

export function createRecurringRule(input: RecurringRuleInput): Promise<RecurringRule> {
  return invoke<RecurringRule>('create_recurring_rule', { input });
}

export function updateRecurringRule(
  id: number,
  input: RecurringRuleInput,
): Promise<RecurringRule> {
  return invoke<RecurringRule>('update_recurring_rule', { id, input });
}

export function setRecurringRuleActive(id: number, active: boolean): Promise<RecurringRule> {
  return invoke<RecurringRule>('set_recurring_rule_active', { id, active });
}

export function previewRecurringOccurrences(
  input: RecurringRuleInput,
  limit: number,
): Promise<OccurrencePreview> {
  return invoke<OccurrencePreview>('preview_recurring_occurrences', { input, limit });
}

export function expandDueRecurring(): Promise<ExpansionResult> {
  return invoke<ExpansionResult>('expand_due_recurring');
}
```

`src/lib/api/index.ts` の `export * from './budgets';` の下に足す:

```ts
export * from './recurring';
```

`src/lib/api/events.ts` の `ChangedDomain` に `'recurring'` を足す:

```ts
export type ChangedDomain =
  | 'categories'
  | 'accounts'
  | 'transactions'
  | 'budgets'
  | 'recurring'
  | 'meta';
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `pnpm test src/lib/api/recurring.test.ts`
Expected: PASS (6 tests)

- [ ] **Step 5: 契約テストにフィクスチャを足す**

`src/lib/api/contract.test.ts` に足す。

import 節:

```ts
import type { ExpansionResult, Frequency, OccurrencePreview, RecurringRuleView, SkipReason } from './recurring';

import expandDueRecurring from '../../../tests/fixtures/responses/expand_due_recurring.json';
import listRecurringRules from '../../../tests/fixtures/responses/list_recurring_rules.json';
import previewRecurringOccurrences from '../../../tests/fixtures/responses/preview_recurring_occurrences.json';
```

enum の列挙(`TX_TYPES` の下):

```ts
const FREQUENCIES = members<Frequency>({ monthly: true, weekly: true, yearly: true });
const SKIP_REASONS = members<SkipReason>({
  archived_account: true,
  archived_counter_account: true,
  archived_category: true,
  category_type_mismatch: true,
});
```

`COVERED_FIXTURES` に 3 件足す(アルファベット順を保つ):

```ts
  'expand_due_recurring.json',
  ...
  'list_recurring_rules.json',
  ...
  'preview_recurring_occurrences.json',
```

`describe` の中に足す:

```ts
  it('list_recurring_rules is a RecurringRuleView[]', () => {
    fixtureFits<RecurringRuleView[]>(listRecurringRules);
    typeCovers<typeof listRecurringRules>(shapeOf<RecurringRuleView[]>());

    for (const view of listRecurringRules) {
      expect(TX_TYPES).toContain(view.rule.type);
      expect(FREQUENCIES).toContain(view.rule.frequency);
    }
    // 停止中の振替ルールを 1 件含む: counter_account_id あり / category_id なし。
    expect(listRecurringRules.some((v) => v.rule.counter_account_id !== null)).toBe(true);
    expect(listRecurringRules.some((v) => v.next_occurrence === null)).toBe(true);
  });

  it('expand_due_recurring is an ExpansionResult with a known skip reason', () => {
    fixtureFits<ExpansionResult>(expandDueRecurring);
    typeCovers<typeof expandDueRecurring>(shapeOf<ExpansionResult>());

    for (const skipped of expandDueRecurring.skipped) {
      expect(SKIP_REASONS).toContain(skipped.reason);
    }
  });

  it('preview_recurring_occurrences is an OccurrencePreview', () => {
    fixtureFits<OccurrencePreview>(previewRecurringOccurrences);
    typeCovers<typeof previewRecurringOccurrences>(shapeOf<OccurrencePreview>());
  });
```

- [ ] **Step 6: 契約テストと型チェックが通ることを確認する**

Run: `pnpm test src/lib/api/contract.test.ts && pnpm check`
Expected: PASS

- [ ] **Step 7: ストアの失敗するテストを書く**

`src/lib/stores/recurring.test.ts` を新規作成:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const listRecurringRulesMock = vi.fn();
const setRecurringRuleActiveMock = vi.fn();
vi.mock('../api/recurring', () => ({
  listRecurringRules: (...args: unknown[]) => listRecurringRulesMock(...args),
  setRecurringRuleActive: (...args: unknown[]) => setRecurringRuleActiveMock(...args),
}));
vi.mock('../api/events', () => ({
  onDataChanged: async () => () => {},
}));

import { createRecurringStore } from './recurring.svelte';

const view = {
  rule: {
    id: 1,
    name: '家賃',
    type: 'expense',
    amount: 85_000,
    account_id: 1,
    counter_account_id: null,
    category_id: 1,
    description: '',
    frequency: 'monthly',
    day_of_month: 27,
    day_of_week: null,
    starts_on: '2026-01-27',
    ends_on: null,
    last_generated_on: null,
    active: true,
  },
  next_occurrence: '2026-02-27',
};

describe('recurring store', () => {
  beforeEach(() => {
    listRecurringRulesMock.mockReset();
    setRecurringRuleActiveMock.mockReset();
  });

  it('loads active rules by default', async () => {
    listRecurringRulesMock.mockResolvedValue([view]);

    const store = createRecurringStore();
    await store.load();

    expect(listRecurringRulesMock).toHaveBeenCalledWith(false);
    expect(store.items).toEqual([view]);
    expect(store.error).toBeNull();
  });

  it('reloads with inactive rules when asked', async () => {
    listRecurringRulesMock.mockResolvedValue([]);

    const store = createRecurringStore();
    store.setIncludeInactive(true);
    await store.load();

    expect(listRecurringRulesMock).toHaveBeenLastCalledWith(true);
  });

  it('surfaces a load failure as an error message', async () => {
    listRecurringRulesMock.mockRejectedValue(new Error('boom'));

    const store = createRecurringStore();
    await store.load();

    expect(store.error).toBe('boom');
    expect(store.items).toEqual([]);
  });

  it('reloads after toggling a rule', async () => {
    listRecurringRulesMock.mockResolvedValue([view]);
    setRecurringRuleActiveMock.mockResolvedValue(view.rule);

    const store = createRecurringStore();
    await store.load();
    listRecurringRulesMock.mockClear();
    await store.toggleActive(1, false);

    expect(setRecurringRuleActiveMock).toHaveBeenCalledWith(1, false);
    expect(listRecurringRulesMock).toHaveBeenCalled();
  });
});
```

- [ ] **Step 8: テストが落ちることを確認する**

Run: `pnpm test src/lib/stores/recurring.test.ts`
Expected: FAIL — `./recurring.svelte` が存在しない

- [ ] **Step 9: ストアを実装する**

`src/lib/stores/recurring.svelte.ts` を新規作成(`budgets.svelte.ts` と同じ構造):

```ts
import type { UnlistenFn } from '@tauri-apps/api/event';

import { onDataChanged } from '../api/events';
import {
  listRecurringRules,
  setRecurringRuleActive,
  type RecurringRuleView,
} from '../api/recurring';

export type RecurringStore = {
  readonly items: RecurringRuleView[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly includeInactive: boolean;
  load(): Promise<void>;
  setIncludeInactive(next: boolean): void;
  toggleActive(id: number, active: boolean): Promise<void>;
  dispose(): Promise<void>;
};

export function createRecurringStore(): RecurringStore {
  let items = $state<RecurringRuleView[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;
  let requestId = 0;

  async function load() {
    const id = ++requestId;
    loading = true;
    error = null;
    try {
      const res = await listRecurringRules(includeInactive);
      if (id !== requestId) return;
      items = res;
    } catch (e) {
      if (id !== requestId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function setIncludeInactive(next: boolean) {
    includeInactive = next;
    void load();
  }

  async function toggleActive(id: number, active: boolean) {
    try {
      await setRecurringRuleActive(id, active);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'recurring' || domain === 'accounts' || domain === 'categories') {
          void load();
        }
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // ブラウザだけの E2E には Tauri のイベントバスが無い。初回ロードは走る。
    }
  })();

  return {
    get items() {
      return items;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get includeInactive() {
      return includeInactive;
    },
    load,
    setIncludeInactive,
    toggleActive,
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

- [ ] **Step 10: テストと型チェックが通ることを確認する**

Run: `pnpm test && pnpm check`
Expected: PASS

- [ ] **Step 11: コミットする**

```bash
git add src/lib/api/recurring.ts src/lib/api/recurring.test.ts src/lib/api/index.ts \
        src/lib/api/events.ts src/lib/api/contract.test.ts \
        src/lib/stores/recurring.svelte.ts src/lib/stores/recurring.test.ts
git commit -m "feat: type the recurring commands and pin them to the fixtures

TS の型は手書きだが、Rust の serde 出力を写したフィクスチャと
contract.test.ts で突き合わせているので、片方だけ変えると落ちる。"
```

---

### Task 8: Recurring 画面、起動時展開の配線、E2E

**Files:**
- Create: `src/routes/Recurring.svelte`
- Create: `tests/e2e/recurring-flow.spec.ts`
- Modify: `src/App.svelte`, `src/lib/components/Sidebar.svelte`
- Modify: `tests/e2e/tauriMock.ts`, `tests/tauri-mock.test.ts`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: Task 7 の `createRecurringStore` / `expandDueRecurring` / `createRecurringRule` / `previewRecurringOccurrences`、既存の `createCategoriesStore` / `createAccountsStore`
- Produces: `/recurring` ルート、`data-testid="nav-recurring"`, `data-testid="recurring-expansion-banner"`, `data-testid="recurring-skip-banner"`, `data-testid="recurring-row"`, `data-testid="recurring-preview"`

- [ ] **Step 1: E2E モックに新しいコマンドを足す**

`tests/e2e/tauriMock.ts` の `readyBootResults` に足す:

```ts
  list_recurring_rules: [],
  expand_due_recurring: { generated: 0, rules: [], skipped: [] },
```

`tests/tauri-mock.test.ts` を更新する。import に足す:

```ts
import expandDueRecurring from './fixtures/responses/expand_due_recurring.json';
```

`objectResponses` に足す:

```ts
  expand_due_recurring: expandDueRecurring,
```

`listResponses` に足す:

```ts
  'list_recurring_rules',
```

`expand_due_recurring` フィクスチャは `generated: 2` だが、モックは空状態なので `generated: 0`。既存の突き合わせは「配列は空・スカラーは型が一致」までしか見ないので、これで通る。

- [ ] **Step 2: モックのテストが通ることを確認する**

Run: `pnpm test tests/tauri-mock.test.ts`
Expected: PASS

- [ ] **Step 3: ナビゲーションに項目を足す**

`src/lib/components/Sidebar.svelte` の `items` の `/budgets` の下に足す:

```ts
    { path: '/recurring', label: '定期取引', icon: '🔁' },
```

`src/App.svelte` の `routePaths` に `'/recurring',` を足し、import に足す:

```ts
  import Recurring from './routes/Recurring.svelte';
```

分岐に足す(`{:else if currentPath === '/budgets'}` の下):

```svelte
      {:else if currentPath === '/recurring'}
        <Recurring />
```

- [ ] **Step 4: 起動時展開を配線する**

`src/App.svelte` の `<script>` に足す:

```ts
  import { expandDueRecurring, type ExpansionResult } from './lib/api/recurring';

  let expansion = $state<ExpansionResult | null>(null);
```

`onMount` の `bootStatus()` の `.then` を差し替える。展開は boot が ready のときだけ走らせ、失敗しても握り潰す(起動を止めない):

```ts
    void bootStatus()
      .then((status) => {
        boot = status;
        if (status.state !== 'ready') return;
        return expandDueRecurring()
          .then((result) => {
            expansion = result;
          })
          .catch(() => {
            // 展開の失敗でアプリを開けなくしない。Recurring 画面で再試行できる。
          });
      })
      .catch((e) => {
        bootError = e instanceof Error ? e.message : String(e);
      });
```

`<main class="app-main">` の直下、ルート分岐の**上**にバナーを置く:

```svelte
      {#if expansion && expansion.generated > 0}
        <div class="banner banner-info" data-testid="recurring-expansion-banner">
          定期取引を {expansion.generated} 件生成しました
        </div>
      {/if}
      {#if expansion && expansion.skipped.length > 0}
        <div class="banner banner-warn" data-testid="recurring-skip-banner">
          {expansion.skipped.length} 件のルールを見送りました。参照先の口座・カテゴリを確認してください
          <a
            href="/recurring"
            onclick={(event) => {
              event.preventDefault();
              navigate('/recurring');
            }}>定期取引を開く</a
          >
        </div>
      {/if}
```

`<style>` に足す:

```css
  .banner {
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-4);
  }

  .banner-info {
    background: rgba(79, 172, 254, 0.18);
  }

  .banner-warn {
    background: rgba(255, 170, 0, 0.22);
  }

  .banner a {
    margin-left: var(--space-3);
  }
```

- [ ] **Step 5: Recurring 画面を実装する**

`src/routes/Recurring.svelte` を新規作成:

```svelte
<script lang="ts">
  import { onDestroy } from 'svelte';

  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import ErrorBanner from '../lib/components/ErrorBanner.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import Select from '../lib/components/Select.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import {
    createRecurringRule,
    previewRecurringOccurrences,
    updateRecurringRule,
    type Frequency,
    type OccurrencePreview,
    type RecurringRuleInput,
    type RecurringRuleView,
  } from '../lib/api/recurring';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import { createRecurringStore } from '../lib/stores/recurring.svelte';

  const rules = createRecurringStore();
  const accounts = createAccountsStore();
  const categories = createCategoriesStore();

  const FREQUENCY_OPTIONS = [
    { value: 'monthly', label: '毎月' },
    { value: 'weekly', label: '毎週' },
    { value: 'yearly', label: '毎年' },
  ];

  const WEEKDAY_OPTIONS = ['日', '月', '火', '水', '木', '金', '土'].map((label, i) => ({
    value: String(i),
    label: `${label}曜日`,
  }));

  const TYPE_OPTIONS = [
    { value: 'expense', label: '支出' },
    { value: 'income', label: '収入' },
    { value: 'transfer', label: '振替' },
  ];

  function blankForm() {
    return {
      id: null as number | null,
      name: '',
      type: 'expense',
      amount: '',
      account_id: '',
      counter_account_id: '',
      category_id: '',
      description: '',
      frequency: 'monthly' as Frequency,
      day_of_month: '1',
      day_of_week: '1',
      starts_on: new Date().toISOString().slice(0, 10),
      ends_on: '',
    };
  }

  let open = $state(false);
  let form = $state(blankForm());
  let preview = $state<OccurrencePreview | null>(null);
  let formError = $state<string | null>(null);
  let saving = $state(false);

  /** フォームの内容をコマンドの入力形に落とす。空文字は null に潰す。 */
  function toInput(): RecurringRuleInput {
    const isTransfer = form.type === 'transfer';
    const isWeekly = form.frequency === 'weekly';
    return {
      name: form.name,
      type: form.type as RecurringRuleInput['type'],
      amount: Number(form.amount),
      account_id: Number(form.account_id),
      counter_account_id: isTransfer ? Number(form.counter_account_id) : null,
      category_id: isTransfer ? null : Number(form.category_id),
      description: form.description,
      frequency: form.frequency,
      day_of_month: isWeekly ? null : Number(form.day_of_month),
      day_of_week: isWeekly ? Number(form.day_of_week) : null,
      starts_on: form.starts_on,
      ends_on: form.ends_on === '' ? null : form.ends_on,
    };
  }

  function openCreate() {
    form = blankForm();
    preview = null;
    formError = null;
    open = true;
  }

  function openEdit(view: RecurringRuleView) {
    const r = view.rule;
    form = {
      id: r.id,
      name: r.name,
      type: r.type,
      amount: String(r.amount),
      account_id: String(r.account_id),
      counter_account_id: r.counter_account_id === null ? '' : String(r.counter_account_id),
      category_id: r.category_id === null ? '' : String(r.category_id),
      description: r.description,
      frequency: r.frequency,
      day_of_month: r.day_of_month === null ? '1' : String(r.day_of_month),
      day_of_week: r.day_of_week === null ? '1' : String(r.day_of_week),
      starts_on: r.starts_on,
      ends_on: r.ends_on ?? '',
    };
    preview = null;
    formError = null;
    open = true;
  }

  /** 保存前に「今すぐ何件生成されるか」を Rust に数えさせる。 */
  async function refreshPreview() {
    formError = null;
    try {
      preview = await previewRecurringOccurrences(toInput(), 100);
    } catch (e) {
      preview = null;
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function save() {
    saving = true;
    formError = null;
    try {
      if (form.id === null) await createRecurringRule(toInput());
      else await updateRecurringRule(form.id, toInput());
      open = false;
      await rules.load();
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  onDestroy(() => {
    void rules.dispose();
    void accounts.dispose();
    void categories.dispose();
  });
</script>

<header class="page-header">
  <h1>定期取引</h1>
  <div class="actions">
    <label>
      <input
        type="checkbox"
        checked={rules.includeInactive}
        data-testid="recurring-include-inactive"
        onchange={(e) => rules.setIncludeInactive(e.currentTarget.checked)}
      />
      停止中も表示
    </label>
    <Button onclick={openCreate} testid="recurring-new">ルールを追加</Button>
  </div>
</header>

{#if rules.error}
  <ErrorBanner message={rules.error} />
{/if}

<Card>
  {#if rules.items.length === 0}
    <EmptyState title="定期取引のルールがありません" hint="家賃や給与など、毎月同じ取引を登録できます" />
  {:else}
    <table>
      <thead>
        <tr>
          <th>名前</th>
          <th>金額</th>
          <th>周期</th>
          <th>次回予定</th>
          <th>状態</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each rules.items as view (view.rule.id)}
          <tr data-testid="recurring-row">
            <td>{view.rule.name}</td>
            <td class="num">{view.rule.amount.toLocaleString('ja-JP')} 円</td>
            <td>{FREQUENCY_OPTIONS.find((o) => o.value === view.rule.frequency)?.label}</td>
            <td data-testid="recurring-next">{view.next_occurrence ?? '—'}</td>
            <td>{view.rule.active ? '有効' : '停止中'}</td>
            <td class="row-actions">
              <Button variant="ghost" onclick={() => openEdit(view)}>編集</Button>
              <Button
                variant="ghost"
                onclick={() => rules.toggleActive(view.rule.id, !view.rule.active)}
              >
                {view.rule.active ? '停止' : '再開'}
              </Button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</Card>

<Modal {open} title={form.id === null ? 'ルールを追加' : 'ルールを編集'} onclose={() => (open = false)}>
  <TextField label="名前" bind:value={form.name} required testid="recurring-name" />
  <Select label="種別" bind:value={form.type} options={TYPE_OPTIONS} testid="recurring-type" />
  <TextField label="金額 (円)" type="number" bind:value={form.amount} required testid="recurring-amount" />

  <Select
    label="口座"
    bind:value={form.account_id}
    options={accounts.items.map((a) => ({ value: String(a.id), label: a.name }))}
    required
    testid="recurring-account"
  />

  {#if form.type === 'transfer'}
    <Select
      label="振替先口座"
      bind:value={form.counter_account_id}
      options={accounts.items.map((a) => ({ value: String(a.id), label: a.name }))}
      required
      testid="recurring-counter-account"
    />
  {:else}
    <Select
      label="カテゴリ"
      bind:value={form.category_id}
      options={categories.items
        .filter((c) => c.type === form.type)
        .map((c) => ({ value: String(c.id), label: c.name }))}
      required
      testid="recurring-category"
    />
  {/if}

  <Select label="周期" bind:value={form.frequency} options={FREQUENCY_OPTIONS} testid="recurring-frequency" />

  {#if form.frequency === 'weekly'}
    <Select label="曜日" bind:value={form.day_of_week} options={WEEKDAY_OPTIONS} testid="recurring-day-of-week" />
  {:else}
    <TextField label="発生日 (1-31)" type="number" bind:value={form.day_of_month} required testid="recurring-day-of-month" />
    <p class="hint">31 を選ぶと、31 日が無い月はその月の末日になります。</p>
  {/if}

  <TextField label="開始日" type="date" bind:value={form.starts_on} required testid="recurring-starts-on" />
  <TextField label="終了日 (任意)" type="date" bind:value={form.ends_on} testid="recurring-ends-on" />
  <TextField label="メモ" bind:value={form.description} testid="recurring-description" />

  <div class="preview">
    <Button variant="ghost" onclick={refreshPreview} testid="recurring-preview-button">
      生成される日付を確認
    </Button>
    {#if preview}
      <p data-testid="recurring-preview">
        保存すると {preview.backfill_total} 件が今すぐ生成されます。次回以降は
        {preview.upcoming.join(' / ') || '予定なし'}
      </p>
    {/if}
  </div>

  {#if formError}
    <ErrorBanner message={formError} />
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>キャンセル</Button>
    <Button onclick={save} disabled={saving} testid="recurring-save">保存</Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-4);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    text-align: left;
    padding: var(--space-3);
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  }

  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    gap: var(--space-2);
  }

  .hint {
    margin: 0 0 var(--space-3);
    font-size: 0.85rem;
    opacity: 0.75;
  }

  .preview {
    margin-top: var(--space-4);
  }
</style>
```

`createAccountsStore` / `createCategoriesStore` の実際の引数と公開プロパティ名は `src/lib/stores/accounts.svelte.ts` / `categories.svelte.ts` を開いて合わせること。`items` / `dispose()` の形が違う場合はそれに従う。

- [ ] **Step 6: 型チェックとユニットテストが通ることを確認する**

Run: `pnpm check && pnpm test`
Expected: PASS

- [ ] **Step 7: E2E を書く**

`tests/e2e/recurring-flow.spec.ts` を新規作成:

```ts
import { expect, test } from '@playwright/test';

const seededAccounts = [
  {
    id: 1,
    name: '現金',
    kind: 'cash',
    currency: 'JPY',
    initial_balance: 100_000,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

const seededCategories = [
  {
    id: 1,
    name: '家賃',
    type: 'expense',
    color: null,
    icon: null,
    display_order: 0,
    archived_at: null,
  },
];

const createdRule = {
  id: 1,
  name: '家賃',
  type: 'expense',
  amount: 85_000,
  account_id: 1,
  counter_account_id: null,
  category_id: 1,
  description: '',
  frequency: 'monthly',
  day_of_month: 27,
  day_of_week: null,
  starts_on: '2026-01-27',
  ends_on: null,
  last_generated_on: null,
  active: true,
};

test('a new rule appears in the list with its next occurrence', async ({ page }) => {
  await page.addInitScript(
    (fixtures) => {
      const state = { created: false };
      const internals = (window as any).__TAURI_INTERNALS__ ?? {};
      internals.invoke = async (command: string, args: any) => {
        switch (command) {
          case 'boot_status':
            return { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' };
          case 'expand_due_recurring':
            return { generated: 3, rules: [], skipped: [] };
          case 'list_accounts':
            return fixtures.accounts;
          case 'list_categories':
            return fixtures.categories;
          case 'list_recurring_rules':
            return state.created
              ? [{ rule: fixtures.rule, next_occurrence: '2026-02-27' }]
              : [];
          case 'preview_recurring_occurrences':
            return { backfill: [], backfill_total: 4, truncated: false, upcoming: ['2026-02-27'] };
          case 'create_recurring_rule':
            state.created = true;
            return fixtures.rule;
          case 'list_balances':
            return { accounts: [], total_assets: 0 };
          case 'list_transactions':
            return { items: [], total: 0 };
          case 'monthly_summary':
            return { income: 0, expense: 0, net: 0, by_category: [] };
          default:
            return Array.isArray(args) ? [] : null;
        }
      };
      (window as any).__TAURI_INTERNALS__ = internals;
    },
    { accounts: seededAccounts, categories: seededCategories, rule: createdRule },
  );

  await page.goto('/');

  // 起動時展開の結果がバナーに出る。
  await expect(page.getByTestId('recurring-expansion-banner')).toContainText('3 件');

  await page.getByTestId('nav-recurring').click();
  await expect(page.getByTestId('recurring-row')).toHaveCount(0);

  await page.getByTestId('recurring-new').click();
  await page.getByTestId('recurring-name').fill('家賃');
  await page.getByTestId('recurring-amount').fill('85000');
  await page.getByTestId('recurring-account').selectOption('1');
  await page.getByTestId('recurring-category').selectOption('1');
  await page.getByTestId('recurring-day-of-month').fill('27');
  await page.getByTestId('recurring-starts-on').fill('2026-01-27');

  // 保存前に生成件数が見える。
  await page.getByTestId('recurring-preview-button').click();
  await expect(page.getByTestId('recurring-preview')).toContainText('4 件');

  await page.getByTestId('recurring-save').click();

  await expect(page.getByTestId('recurring-row')).toHaveCount(1);
  await expect(page.getByTestId('recurring-next')).toHaveText('2026-02-27');
});
```

`data-testid` は `TextField` / `Select` が実際に付ける要素に載る。`getByTestId(...).fill(...)` が効かない場合は `src/lib/components/TextField.svelte` を見て、`testid` が `<input>` 側に付いているか確認する。

- [ ] **Step 8: E2E が通ることを確認する**

Run: `pnpm build && pnpm test:e2e tests/e2e/recurring-flow.spec.ts`
Expected: PASS

- [ ] **Step 9: ドキュメントを更新する**

`CLAUDE.md` の「現在の状態」を書き換える:

```markdown
## 現在の状態

- Phase 5a 完了 (定期取引: ルール CRUD + 起動時の冪等展開 + Recurring 画面)
- 既存の `家計簿.html` は参考用のプロトタイプ。**移植せず新規実装する**
- 次は spec の Phase 5b (分析レポート4タブ)
```

「ディレクトリ規約」の `domain/` のコメントは既に `recurring.rs` を含んでいるので変更不要。

- [ ] **Step 10: 完了条件をすべて確認する**

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test && cd ..
pnpm test
pnpm check
pnpm build && pnpm test:e2e
```
Expected: すべて PASS

macOS / Windows の実機ビルドは CI (`.github/workflows/ci.yml`) が両 OS で `cargo clippy` / `cargo test` まで通す。配布ビルドの確認は `pnpm tauri build` を各 OS で 1 回。

- [ ] **Step 11: コミットする**

```bash
git add src/routes/Recurring.svelte src/App.svelte src/lib/components/Sidebar.svelte \
        tests/e2e/recurring-flow.spec.ts tests/e2e/tauriMock.ts tests/tauri-mock.test.ts \
        CLAUDE.md
git commit -m "feat: manage recurring rules and expand them on startup

展開は boot が ready のときだけ走らせ、失敗しても握り潰す。定期取引が
展開できないことはアプリを開けない理由にならないため。生成件数と
見送ったルール数はバナーで伝える。

作成フォームに保存前プレビューを置いた。遡及生成は starts_on から
全件なので、開始日の打ち間違いはここで気づけないと取り返しがつかない。"
```

---

## Self-Review

**spec カバレッジ (§5.4 の各項目 → タスク)**

| spec の要求 | 対応 |
|---|---|
| 展開はフロントからの明示コマンド | Task 5(コマンド) / Task 8(配線) |
| `(last_generated_on, today]` の窓 | Task 1 |
| 冪等性 (連続起動で二重生成しない) | Task 1(proptest 分割不変) / Task 5(日次 90 回 vs 一括) |
| `last_generated_on` を必ず更新 | Task 5 |
| `last_generated_on` NULL は starts_on から全件 | Task 1 / Task 5 |
| 大量生成の事故防止プレビュー | Task 4 / Task 8 |
| 月末クランプ (31 → 28/29/30) | Task 1 |
| yearly の月は starts_on から | Task 1 |
| weekly の day_of_week 必須 | Task 3 |
| 削除は `active = 0` | Task 2 |
| アーカイブ参照はそのルールだけ見送り watermark 据え置き | Task 5 |
| スキップ理由を返して UI が警告 | Task 5 / Task 8 |
| 「次回予定」は表示のみ | Task 1 / Task 3 / Task 8 |
| ルール変更時、過去生成分はそのまま | Task 2(`update` が `last_generated_on` を触らない) |
| コマンド 6 本 | Task 3(4 本) / Task 4(1 本) / Task 5(1 本) |
| 契約フィクスチャで固定 | Task 6 / Task 7 |
| `transactions(recurring_id)` インデックス | Task 2 |
| 振替を集計から除外 | Task 5(`generated_transfers_stay_out_of_the_income_expense_totals`) |

**未確定として実装者に委ねた点**

- `createAccountsStore` / `createCategoriesStore` の引数と公開プロパティ名(Task 8 Step 5 に注記)
- `TextField` / `Select` が `testid` を載せる要素(Task 8 Step 7 に注記)

どちらも既存ファイルを開けば分かるもので、設計判断ではない。

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-03-phase-5a-recurring-transactions.md`.
