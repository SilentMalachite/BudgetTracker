# Phase 2 — Slice 05: Dashboard

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Build the dashboard page — current-month income/expense/net cards, 10 most recent transactions, and a 12-month bar chart of income vs. expense — backed by two new Tauri commands (`monthly_summary`, `monthly_series`) and a pure 0-fill helper in `domain/report.rs`.

**Prerequisite:** Slice 04 (transactions) completed.

**Spec:** sections 4.4, 5.1 of `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md`.

---

### Task 1: `domain/report.rs` types + `fill_monthly_series` (TDD)

**Files:**
- Modify: `src-tauri/src/domain/report.rs`

- [ ] **Step 1: Write the file**

```rust
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearMonth {
    pub year: i32,
    pub month: u32,
}

impl YearMonth {
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    pub fn parse_key(raw: &str) -> AppResult<Self> {
        if raw.len() != 7 || !raw.is_char_boundary(4) {
            return Err(AppError::InvalidArgument(format!(
                "year_month must be YYYY-MM, got '{raw}'"
            )));
        }
        let year: i32 = raw[..4]
            .parse()
            .map_err(|_| AppError::InvalidArgument(format!("bad year in '{raw}'")))?;
        let month: u32 = raw[5..]
            .parse()
            .map_err(|_| AppError::InvalidArgument(format!("bad month in '{raw}'")))?;
        if !(1..=12).contains(&month) {
            return Err(AppError::InvalidArgument(format!("month out of range: {month}")));
        }
        Ok(Self { year, month })
    }

    pub fn step_back(self, months: u32) -> Self {
        let total = self.year as i64 * 12 + (self.month as i64 - 1) - months as i64;
        let year = total.div_euclid(12) as i32;
        let month = (total.rem_euclid(12) + 1) as u32;
        Self { year, month }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonthlyBucket {
    pub year_month: String,
    pub income: i64,
    pub expense: i64,
}

/// Fill gaps in a (possibly sparse) bucket list so the returned vector has
/// exactly `months` entries ending at `end` (inclusive), oldest first.
pub fn fill_monthly_series(
    buckets: &[MonthlyBucket],
    end: YearMonth,
    months: u32,
) -> AppResult<Vec<MonthlyBucket>> {
    if months == 0 {
        return Ok(Vec::new());
    }
    let mut by_key: std::collections::HashMap<String, &MonthlyBucket> =
        buckets.iter().map(|b| (b.year_month.clone(), b)).collect();
    let mut out = Vec::with_capacity(months as usize);
    for i in (0..months).rev() {
        let ym = end.step_back(i);
        let key = ym.key();
        let entry = by_key.remove(&key).cloned().unwrap_or(MonthlyBucket {
            year_month: key,
            income: 0,
            expense: 0,
        });
        out.push(entry);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_round_trips() {
        let ym = YearMonth { year: 2026, month: 5 };
        assert_eq!(ym.key(), "2026-05");
        assert_eq!(YearMonth::parse_key("2026-05").unwrap(), ym);
    }

    #[test]
    fn parse_rejects_bad_inputs() {
        assert!(YearMonth::parse_key("2026/05").is_err());
        assert!(YearMonth::parse_key("2026-13").is_err());
        assert!(YearMonth::parse_key("2026-0").is_err());
    }

    #[test]
    fn step_back_within_year() {
        assert_eq!(
            YearMonth { year: 2026, month: 5 }.step_back(2),
            YearMonth { year: 2026, month: 3 },
        );
    }

    #[test]
    fn step_back_crosses_year() {
        assert_eq!(
            YearMonth { year: 2026, month: 2 }.step_back(3),
            YearMonth { year: 2025, month: 11 },
        );
    }

    #[test]
    fn fill_pads_with_zero_buckets() {
        let series = fill_monthly_series(
            &[MonthlyBucket {
                year_month: "2026-05".into(),
                income: 1_000,
                expense: 500,
            }],
            YearMonth { year: 2026, month: 5 },
            3,
        )
        .unwrap();
        assert_eq!(
            series,
            vec![
                MonthlyBucket { year_month: "2026-03".into(), income: 0, expense: 0 },
                MonthlyBucket { year_month: "2026-04".into(), income: 0, expense: 0 },
                MonthlyBucket { year_month: "2026-05".into(), income: 1_000, expense: 500 },
            ]
        );
    }

    #[test]
    fn fill_zero_months_returns_empty() {
        let r = fill_monthly_series(&[], YearMonth { year: 2026, month: 5 }, 0).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn fill_drops_buckets_outside_window() {
        let buckets = vec![
            MonthlyBucket { year_month: "2025-01".into(), income: 1, expense: 0 },
            MonthlyBucket { year_month: "2026-05".into(), income: 2, expense: 0 },
        ];
        let series = fill_monthly_series(&buckets, YearMonth { year: 2026, month: 5 }, 3).unwrap();
        assert_eq!(series.len(), 3);
        assert!(series.iter().all(|b| b.year_month != "2025-01"));
    }
}
```

- [ ] **Step 2: Run tests + clippy**

```bash
cargo test --lib domain::report
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/report.rs
git commit -m "feat(domain): YearMonth + fill_monthly_series 0-fill helper"
```

---

### Task 2: `infra/repo/report_repo.rs` — SQL aggregations

**Files:**
- Create: `src-tauri/src/infra/repo/report_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`

- [ ] **Step 1: Wire module**

In `src-tauri/src/infra/repo/mod.rs`:

```rust
pub mod report_repo;
```

- [ ] **Step 2: Implement queries**

```rust
// src-tauri/src/infra/repo/report_repo.rs
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::domain::report::MonthlyBucket;
use crate::error::AppResult;

#[derive(Debug, Serialize)]
pub struct CategoryAggregate {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
}

#[derive(Debug, Serialize)]
pub struct MonthlySummary {
    pub income: i64,
    pub expense: i64,
    pub net: i64,
    pub by_category: Vec<CategoryAggregate>,
}

/// Sum income/expense for a given (year, month). `type='transfer'` rows are
/// excluded by the SQL filter, regardless of source.
pub fn monthly_summary(conn: &Connection, year: i32, month: u32) -> AppResult<MonthlySummary> {
    let from = format!("{year:04}-{month:02}-01");
    // last day computed in Rust to keep SQL simple
    let last_day = chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
        .or_else(|| chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1))
        .map(|d| d.pred_opt().unwrap())
        .unwrap();
    let to = last_day.format("%Y-%m-%d").to_string();

    let (income, expense): (i64, i64) = conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN type = 'income'  THEN amount END), 0),
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount END), 0)
           FROM transactions
          WHERE occurred_on BETWEEN ?1 AND ?2
            AND type IN ('income','expense')",
        params![from, to],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amt
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY c.id
          ORDER BY amt DESC",
    )?;
    let by_category: Vec<CategoryAggregate> = stmt
        .query_map(params![from, to], |r| {
            Ok(CategoryAggregate {
                category_id: r.get(0)?,
                name: r.get(1)?,
                type_: r.get(2)?,
                amount: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(MonthlySummary {
        income,
        expense,
        net: income - expense,
        by_category,
    })
}

pub fn monthly_buckets_since(
    conn: &Connection,
    from_year_month: &str,
) -> AppResult<Vec<MonthlyBucket>> {
    let from = format!("{from_year_month}-01");
    let mut stmt = conn.prepare(
        "SELECT
            strftime('%Y-%m', occurred_on) AS ym,
            COALESCE(SUM(CASE WHEN type='income'  THEN amount END), 0) AS income,
            COALESCE(SUM(CASE WHEN type='expense' THEN amount END), 0) AS expense
           FROM transactions
          WHERE occurred_on >= ?1
            AND type IN ('income','expense')
          GROUP BY ym
          ORDER BY ym ASC",
    )?;
    let buckets: Vec<MonthlyBucket> = stmt
        .query_map(params![from], |r| {
            Ok(MonthlyBucket {
                year_month: r.get(0)?,
                income: r.get(1)?,
                expense: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(buckets)
}
```

- [ ] **Step 3: Build + clippy**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/infra/repo
git commit -m "feat(infra): report_repo for monthly_summary and monthly_buckets_since"
```

---

### Task 3: Integration test — transfers are excluded from aggregates

**Files:**
- Create: `src-tauri/tests/integration_reports.rs`

- [ ] **Step 1: Write tests**

```rust
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, report_repo};
use rusqlite::{Connection, params};

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded() -> (Connection, i64, i64, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let acc_a = account_repo::insert(&conn, &account_repo::InsertInput {
        name: "A", kind: AccountKind::Bank, currency: "JPY", initial_balance: 0,
        display_order: 0, note: "", now: NOW,
    }).unwrap();
    let acc_b = account_repo::insert(&conn, &account_repo::InsertInput {
        name: "B", kind: AccountKind::Cash, currency: "JPY", initial_balance: 0,
        display_order: 1, note: "", now: NOW,
    }).unwrap();
    let exp = category_repo::insert(&conn, &category_repo::InsertInput {
        name: "Food", type_: CategoryType::Expense, color: None, icon: None, display_order: 0,
    }).unwrap();
    let inc = category_repo::insert(&conn, &category_repo::InsertInput {
        name: "Salary", type_: CategoryType::Income, color: None, icon: None, display_order: 0,
    }).unwrap();
    (conn, acc_a, acc_b, exp, inc)
}

fn insert_tx(conn: &Connection, date: &str, type_: &str, amount: i64, account: i64,
             counter: Option<i64>, category: Option<i64>) {
    conn.execute(
        "INSERT INTO transactions(occurred_on, type, amount, account_id,
                                  counter_account_id, category_id,
                                  created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![date, type_, amount, account, counter, category, NOW],
    )
    .unwrap();
}

#[test]
fn monthly_summary_excludes_transfers() {
    let (conn, a, b, exp, inc) = seeded();
    insert_tx(&conn, "2026-05-10", "income", 300_000, a, None, Some(inc));
    insert_tx(&conn, "2026-05-15", "expense", 1_500, a, None, Some(exp));
    insert_tx(&conn, "2026-05-20", "transfer", 50_000, a, Some(b), None);

    let s = report_repo::monthly_summary(&conn, 2026, 5).unwrap();
    assert_eq!(s.income, 300_000);
    assert_eq!(s.expense, 1_500);
    assert_eq!(s.net, 298_500);
    // by_category should contain the two real entries, not the transfer.
    assert_eq!(s.by_category.len(), 2);
    assert!(s.by_category.iter().all(|c| c.amount > 0));
}

#[test]
fn monthly_summary_ignores_other_months() {
    let (conn, a, _b, exp, _inc) = seeded();
    insert_tx(&conn, "2026-04-30", "expense", 999, a, None, Some(exp));
    insert_tx(&conn, "2026-06-01", "expense", 777, a, None, Some(exp));
    insert_tx(&conn, "2026-05-15", "expense", 500, a, None, Some(exp));
    let s = report_repo::monthly_summary(&conn, 2026, 5).unwrap();
    assert_eq!(s.expense, 500);
    assert_eq!(s.income, 0);
}

#[test]
fn monthly_buckets_since_returns_sorted_groups() {
    let (conn, a, _b, exp, inc) = seeded();
    insert_tx(&conn, "2026-03-10", "income", 100, a, None, Some(inc));
    insert_tx(&conn, "2026-04-15", "expense", 200, a, None, Some(exp));
    insert_tx(&conn, "2026-05-20", "income", 300, a, None, Some(inc));
    insert_tx(&conn, "2026-05-25", "expense", 50, a, None, Some(exp));

    let buckets = report_repo::monthly_buckets_since(&conn, "2026-03").unwrap();
    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0].year_month, "2026-03");
    assert_eq!(buckets[0].income, 100);
    assert_eq!(buckets[1].year_month, "2026-04");
    assert_eq!(buckets[1].expense, 200);
    assert_eq!(buckets[2].year_month, "2026-05");
    assert_eq!(buckets[2].income, 300);
    assert_eq!(buckets[2].expense, 50);
}
```

- [ ] **Step 2: Run**

```bash
cargo test --test integration_reports
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_reports.rs
git commit -m "test(report_repo): transfer exclusion and bucket grouping"
```

---

### Task 4: `commands/reports.rs` + handler registration

**Files:**
- Modify: `src-tauri/src/commands/reports.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement handlers**

```rust
// src-tauri/src/commands/reports.rs
use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::report::{self, MonthlyBucket, YearMonth};
use crate::error::{AppError, AppResult};
use crate::infra::repo::report_repo::{self, MonthlySummary};

#[tauri::command]
pub fn monthly_summary(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<MonthlySummary> {
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!("month out of range: {month}")));
    }
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    report_repo::monthly_summary(&conn, year, month)
}

#[tauri::command]
pub fn monthly_series(state: State<'_, AppState>, months: u32) -> AppResult<Vec<MonthlyBucket>> {
    if months == 0 || months > 60 {
        return Err(AppError::InvalidArgument(format!(
            "months must be between 1 and 60, got {months}"
        )));
    }
    let now = chrono::Utc::now();
    let end = YearMonth {
        year: chrono::Datelike::year(&now.date_naive()),
        month: chrono::Datelike::month(&now.date_naive()),
    };
    let start = end.step_back(months - 1);
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let raw = report_repo::monthly_buckets_since(&conn, &start.key())?;
    report::fill_monthly_series(&raw, end, months)
}
```

- [ ] **Step 2: Register handlers**

Add to `generate_handler!`:

```rust
    commands::reports::monthly_summary,
    commands::reports::monthly_series,
```

- [ ] **Step 3: Build + clippy + test**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/reports.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose monthly_summary and monthly_series"
```

---

### Task 5: Frontend `lib/api/reports.ts`

**Files:**
- Create: `src/lib/api/reports.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: API wrapper**

```ts
// src/lib/api/reports.ts
import { invoke } from '@tauri-apps/api/core';

export type CategoryAggregate = {
  category_id: number;
  name: string;
  type: 'income' | 'expense';
  amount: number;
};

export type MonthlySummary = {
  income: number;
  expense: number;
  net: number;
  by_category: CategoryAggregate[];
};

export function monthlySummary(year: number, month: number): Promise<MonthlySummary> {
  return invoke<MonthlySummary>('monthly_summary', { year, month });
}

export type MonthlyBucket = {
  year_month: string;
  income: number;
  expense: number;
};

export function monthlySeries(months: number): Promise<MonthlyBucket[]> {
  return invoke<MonthlyBucket[]>('monthly_series', { months });
}
```

- [ ] **Step 2: Re-export**

Append to `src/lib/api/index.ts`:

```ts
export * from './reports';
```

- [ ] **Step 3: svelte-check**

```bash
pnpm check
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/api
git commit -m "feat(api): typed reports wrapper"
```

---

### Task 6: `routes/Dashboard.svelte` — cards + recent + Chart.js bar

**Files:**
- Modify: `src/routes/Dashboard.svelte`

- [ ] **Step 1: Replace placeholder**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Card from '../lib/components/Card.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import { monthlySummary, monthlySeries, type MonthlySummary, type MonthlyBucket }
    from '../lib/api/reports';
  import { listTransactions, type Transaction } from '../lib/api/transactions';
  import { onDataChanged } from '../lib/api/events';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import {
    Chart,
    BarController,
    BarElement,
    CategoryScale,
    LinearScale,
    Tooltip,
    Legend,
  } from 'chart.js';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  Chart.register(BarController, BarElement, CategoryScale, LinearScale, Tooltip, Legend);

  const catStore = createCategoriesStore({ include_archived: true });
  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });

  const now = new Date();
  const year = now.getFullYear();
  const month = now.getMonth() + 1;

  let summary = $state<MonthlySummary | null>(null);
  let series = $state<MonthlyBucket[]>([]);
  let recent = $state<Transaction[]>([]);
  let error = $state<string | null>(null);

  let canvas: HTMLCanvasElement | null = null;
  let chart: Chart | null = null;
  let unlisten: UnlistenFn | null = null;

  async function reload() {
    try {
      const [s, ser, txs] = await Promise.all([
        monthlySummary(year, month),
        monthlySeries(12),
        listTransactions({}, 0, 10),
      ]);
      summary = s;
      series = ser;
      recent = txs.items;
      drawChart();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function drawChart() {
    if (!canvas) return;
    const labels = series.map((b) => b.year_month);
    const incomeData = series.map((b) => b.income);
    const expenseData = series.map((b) => b.expense);
    if (chart) {
      chart.data.labels = labels;
      chart.data.datasets[0].data = incomeData;
      chart.data.datasets[1].data = expenseData;
      chart.update();
      return;
    }
    chart = new Chart(canvas, {
      type: 'bar',
      data: {
        labels,
        datasets: [
          { label: '収入', data: incomeData, backgroundColor: '#4FACFE' },
          { label: '支出', data: expenseData, backgroundColor: '#F56565' },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        scales: { y: { beginAtZero: true } },
      },
    });
  }

  onMount(async () => {
    unlisten = await onDataChanged((domain) => {
      if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
        void reload();
      }
    });
    await reload();
  });

  onDestroy(() => {
    chart?.destroy();
    chart = null;
    unlisten?.();
    unlisten = null;
  });

  const categoryById = $derived(
    new Map(catStore.items.map((c) => [c.id, c])),
  );
</script>

<section>
  <h1>ダッシュボード</h1>

  {#if error}
    <p class="error">エラー: {error}</p>
  {/if}

  <div class="cards">
    <Card>
      {#snippet children()}
        <small>{year} 年 {month} 月の収入</small>
        <strong class="income" data-testid="card-income">
          {summary ? yen.format(summary.income) : '—'}
        </strong>
      {/snippet}
    </Card>
    <Card>
      {#snippet children()}
        <small>{year} 年 {month} 月の支出</small>
        <strong class="expense" data-testid="card-expense">
          {summary ? yen.format(summary.expense) : '—'}
        </strong>
      {/snippet}
    </Card>
    <Card>
      {#snippet children()}
        <small>当月の収支</small>
        <strong class:positive={(summary?.net ?? 0) >= 0}
                class:negative={(summary?.net ?? 0) < 0}
                data-testid="card-net">
          {summary ? yen.format(summary.net) : '—'}
        </strong>
      {/snippet}
    </Card>
  </div>

  <div class="chart-row">
    <Card>
      {#snippet children()}
        <h2>月別収支 (直近12ヶ月)</h2>
        <div class="chart-box"><canvas bind:this={canvas}></canvas></div>
      {/snippet}
    </Card>
  </div>

  <Card>
    {#snippet children()}
      <h2>直近の取引</h2>
      {#if recent.length === 0}
        <EmptyState title="まだ取引がありません" hint="「取引」ページから記録してみましょう" />
      {:else}
        <ul class="recent" data-testid="recent-list">
          {#each recent as tx (tx.id)}
            <li>
              <span class="date">{tx.occurred_on}</span>
              <span class="cat">
                {#if tx.category_id != null}
                  {categoryById.get(tx.category_id)?.name ?? '-'}
                {:else}振替{/if}
              </span>
              <span class="desc">{tx.description}</span>
              <span
                class="amount"
                class:income={tx.type === 'income'}
                class:expense={tx.type === 'expense'}
              >
                {tx.type === 'expense' ? '-' : '+'}{yen.format(tx.amount)}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>
</section>

<style>
  h1 { color: white; margin-bottom: var(--space-5); }
  h2 { margin: 0 0 var(--space-3); }
  .cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-4); margin-bottom: var(--space-5); }
  .cards strong { display: block; font-size: 1.8rem; margin-top: var(--space-2); font-variant-numeric: tabular-nums; }
  .income, .positive { color: var(--success); }
  .expense, .negative { color: var(--danger); }
  .chart-row { margin-bottom: var(--space-5); }
  .chart-box { height: 280px; }
  .recent { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--space-2); }
  .recent li {
    display: grid;
    grid-template-columns: 110px 130px 1fr auto;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border);
  }
  .date { color: var(--muted); font-variant-numeric: tabular-nums; }
  .cat { font-weight: 600; }
  .desc { color: var(--muted); }
  .amount { font-variant-numeric: tabular-nums; }
  .amount.income { color: var(--success); font-weight: 700; }
  .amount.expense { color: var(--danger); font-weight: 700; }
  .error { color: var(--danger); }
</style>
```

- [ ] **Step 2: svelte-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 3: Manual smoke**

```bash
pnpm tauri dev
```

- Navigate to ダッシュボード.
- Verify the three cards, the bar chart (income blue / expense red), and the recent list render.
- Add a new expense from the 取引 page and watch the dashboard update without manual refresh.

Stop the dev server.

- [ ] **Step 4: Commit**

```bash
git add src/routes/Dashboard.svelte
git commit -m "feat(ui): dashboard cards, 12-month bar chart, recent transactions"
```

---

### Dashboard slice DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (report domain + integration).
- [ ] `pnpm test` green.
- [ ] `pnpm check` green.
- [ ] Manual: cards display current-month totals; chart shows last 12 months with 0-fills; adding a transaction triggers live update; a row inserted with `type='transfer'` (via sqlite shell) does NOT affect the cards or the chart.
