//! Rust -> TypeScript response contract fixtures.
//!
//! Every `#[tauri::command]` response type gets ONE realistic sample here.
//! Each test serializes that sample with serde (exactly what Tauri sends to
//! the webview) and compares the bytes with the checked-in JSON under
//! `tests/fixtures/responses/` at the repository root.
//!
//! The frontend (`src/lib/api/contract.test.ts`, Playwright mocks) consumes
//! the same files, so a Rust-side rename or shape change fails here first and
//! then surfaces in `pnpm check` / Vitest once the fixture is regenerated:
//!
//! ```text
//! UPDATE_FIXTURES=1 cargo test --test response_fixtures
//! ```
//!
//! Samples must stay deterministic: no `Utc::now()`, no random ids. Values
//! are hand-built through public fields (not through the SQL/domain
//! evaluators) so the fixtures only move when a serialized shape or the
//! sample itself changes, never because of clock or ranking drift.

use std::path::PathBuf;

use serde::Serialize;

use budget_tracker_lib::commands::backup::ImportResult;
use budget_tracker_lib::commands::balances::BalanceList;
use budget_tracker_lib::commands::meta::AppInfo;
use budget_tracker_lib::commands::recovery::{BootState, BootStatus};
use budget_tracker_lib::commands::recurring::{
    ExpansionResult, OccurrencePreview, RecurringRuleView, RuleExpansion, SkipReason, SkippedRule,
};
use budget_tracker_lib::commands::settings::BackupFileResult;
use budget_tracker_lib::commands::transactions::ListTransactionResult;
use budget_tracker_lib::domain::account::{Account, AccountKind};
use budget_tracker_lib::domain::budget::BudgetStatus;
use budget_tracker_lib::domain::category::{Category, CategoryType};
use budget_tracker_lib::domain::ledger::{Transaction, TxType};
use budget_tracker_lib::domain::recurring::{Frequency, RecurringRule};
use budget_tracker_lib::domain::report::MonthlyBucket;
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::boot::RecoveryReason;
use budget_tracker_lib::infra::repo::balance_repo::AccountBalance;
use budget_tracker_lib::infra::repo::report_repo::{CategoryAggregate, MonthlySummary};

const UPDATE_ENV: &str = "UPDATE_FIXTURES";
const REGENERATE_HINT: &str = "\
Run `UPDATE_FIXTURES=1 cargo test --test response_fixtures` from `src-tauri/` \
to regenerate, then update the matching TypeScript types in `src/lib/api/` \
(and any Playwright mocks that hand-write this response).";

/// `<repo>/tests/fixtures/responses`, resolved from the crate root so the
/// test works from any cwd (`cargo test` sets `CARGO_MANIFEST_DIR`).
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("responses")
}

fn update_requested() -> bool {
    std::env::var(UPDATE_ENV).is_ok_and(|v| v == "1")
}

/// Serialize `value` the way the command layer does and either write or
/// compare `tests/fixtures/responses/<name>.json`.
///
/// A trailing newline is appended so editors and formatters that enforce
/// POSIX line endings do not cause a spurious byte mismatch.
fn check_fixture<T: Serialize>(name: &str, value: &T) {
    let path = fixture_dir().join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value)
        .unwrap_or_else(|err| panic!("serialize fixture `{name}`: {err}"));
    actual.push('\n');

    if update_requested() {
        std::fs::create_dir_all(fixture_dir())
            .unwrap_or_else(|err| panic!("create {}: {err}", fixture_dir().display()));
        std::fs::write(&path, &actual)
            .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "missing response fixture {} ({err}).\n{REGENERATE_HINT}",
            path.display()
        )
    });

    assert!(
        actual == expected,
        "response fixture `{name}` is out of date: {}\n\
         The Rust response shape (or this test's sample) changed.\n\
         {REGENERATE_HINT}\n\
         --- expected (checked in)\n{expected}\
         --- actual (serialized now)\n{actual}",
        path.display()
    );
}

// ---------------------------------------------------------------------------
// Shared sample data (integer yen, ISO 8601 timestamps as `to_rfc3339()`
// emits them, ISO dates in 2026).
// ---------------------------------------------------------------------------

const CREATED: &str = "2026-01-05T00:00:00.000000+00:00";
const ARCHIVED_ACCOUNT: &str = "2026-03-31T15:00:00.000000+00:00";
const ARCHIVED_CATEGORY: &str = "2026-02-28T15:00:00.000000+00:00";
const SAMPLE_DB_PATH: &str = "/Users/example/Library/Application Support/jp.budget-tracker/data.db";

fn sample_accounts() -> Vec<Account> {
    vec![
        Account {
            id: 1,
            name: "財布".into(),
            kind: AccountKind::Cash,
            currency: "JPY".into(),
            initial_balance: 5_000,
            display_order: 0,
            note: String::new(),
            archived_at: None,
            created_at: CREATED.into(),
            updated_at: CREATED.into(),
        },
        Account {
            id: 2,
            name: "給与振込口座".into(),
            kind: AccountKind::Bank,
            currency: "JPY".into(),
            initial_balance: 250_000,
            display_order: 1,
            note: "毎月25日に給与が入る".into(),
            archived_at: None,
            created_at: CREATED.into(),
            updated_at: CREATED.into(),
        },
        Account {
            id: 3,
            name: "旧クレジットカード".into(),
            kind: AccountKind::CreditCard,
            currency: "JPY".into(),
            initial_balance: 0,
            display_order: 2,
            note: "解約済み".into(),
            archived_at: Some(ARCHIVED_ACCOUNT.into()),
            created_at: CREATED.into(),
            updated_at: ARCHIVED_ACCOUNT.into(),
        },
    ]
}

fn sample_categories() -> Vec<Category> {
    vec![
        Category {
            id: 1,
            name: "食費".into(),
            type_: CategoryType::Expense,
            color: Some("#f5576c".into()),
            icon: Some("🍚".into()),
            display_order: 0,
            archived_at: None,
        },
        Category {
            id: 2,
            name: "給与".into(),
            type_: CategoryType::Income,
            color: Some("#4facfe".into()),
            icon: None,
            display_order: 1,
            archived_at: None,
        },
        Category {
            id: 3,
            name: "旧・交際費".into(),
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 2,
            archived_at: Some(ARCHIVED_CATEGORY.into()),
        },
    ]
}

fn sample_transactions() -> Vec<Transaction> {
    vec![
        Transaction {
            id: 101,
            occurred_on: "2026-06-25".into(),
            type_: TxType::Income,
            amount: 320_000,
            account_id: 2,
            counter_account_id: None,
            category_id: Some(2),
            description: "6月分給与".into(),
            recurring_id: None,
            created_at: "2026-06-25T00:30:00.000000+00:00".into(),
            updated_at: "2026-06-25T00:30:00.000000+00:00".into(),
        },
        Transaction {
            id: 102,
            occurred_on: "2026-06-26".into(),
            type_: TxType::Expense,
            amount: 1_280,
            account_id: 1,
            counter_account_id: None,
            category_id: Some(1),
            description: "スーパーで食材".into(),
            recurring_id: None,
            created_at: "2026-06-26T10:15:00.000000+00:00".into(),
            updated_at: "2026-06-26T10:15:00.000000+00:00".into(),
        },
        Transaction {
            id: 103,
            occurred_on: "2026-06-27".into(),
            type_: TxType::Transfer,
            amount: 30_000,
            account_id: 2,
            counter_account_id: Some(1),
            category_id: None,
            description: "現金引き出し".into(),
            recurring_id: None,
            created_at: "2026-06-27T03:00:00.000000+00:00".into(),
            updated_at: "2026-06-27T03:00:00.000000+00:00".into(),
        },
    ]
}

/// June 2026 (30 days), evaluated as of 2026-06-15:
/// `days_left = 1 + 30 - 15 = 16`, `projected = spent * 30 / 15`.
fn budget_under() -> BudgetStatus {
    BudgetStatus {
        category_id: 1,
        category_name: "食費".into(),
        category_color: Some("#f5576c".into()),
        category_icon: Some("🍚".into()),
        budget_id: Some(11),
        budgeted: 40_000,
        spent: 12_000,
        percent: 30,
        progress_percent: 30,
        days_left: 16,
        projected: 24_000,
        alert_threshold: 80,
        threshold_reached: false,
        projected_over_budget: false,
    }
}

fn budget_over() -> BudgetStatus {
    BudgetStatus {
        category_id: 5,
        category_name: "外食".into(),
        category_color: Some("#764ba2".into()),
        category_icon: None,
        budget_id: Some(12),
        budgeted: 20_000,
        spent: 26_000,
        percent: 130,
        progress_percent: 100,
        days_left: 16,
        projected: 52_000,
        alert_threshold: 80,
        threshold_reached: true,
        projected_over_budget: true,
    }
}

// ---------------------------------------------------------------------------
// One test per command so a failure names the exact response that drifted.
// ---------------------------------------------------------------------------

#[test]
fn boot_status_ready() {
    check_fixture(
        "boot_status.ready",
        &BootStatus {
            state: BootState::Ready,
            recovery_reason: None,
            db_path: SAMPLE_DB_PATH.into(),
        },
    );
}

#[test]
fn boot_status_recovery() {
    check_fixture(
        "boot_status.recovery",
        &BootStatus {
            state: BootState::Recovery,
            recovery_reason: Some(RecoveryReason::DecryptFailed),
            db_path: SAMPLE_DB_PATH.into(),
        },
    );
}

#[test]
fn app_info() {
    check_fixture(
        "app_info",
        &AppInfo {
            schema_version: 5,
            db_path: SAMPLE_DB_PATH.into(),
        },
    );
}

#[test]
fn list_balances() {
    // Balances follow `sample_transactions()`; total_assets excludes the
    // archived account (see `domain::balance::total_assets`).
    let accounts = vec![
        AccountBalance {
            account_id: 1,
            name: "財布".into(),
            kind: AccountKind::Cash,
            initial_balance: 5_000,
            balance: 33_720,
            archived_at: None,
            display_order: 0,
        },
        AccountBalance {
            account_id: 2,
            name: "給与振込口座".into(),
            kind: AccountKind::Bank,
            initial_balance: 250_000,
            balance: 540_000,
            archived_at: None,
            display_order: 1,
        },
        AccountBalance {
            account_id: 3,
            name: "旧クレジットカード".into(),
            kind: AccountKind::CreditCard,
            initial_balance: 0,
            balance: -4_200,
            archived_at: Some(ARCHIVED_ACCOUNT.into()),
            display_order: 2,
        },
    ];
    check_fixture(
        "list_balances",
        &BalanceList {
            accounts,
            total_assets: 573_720,
        },
    );
}

#[test]
fn monthly_summary() {
    check_fixture(
        "monthly_summary",
        &MonthlySummary {
            income: 320_000,
            expense: 1_280,
            net: 318_720,
            by_category: vec![
                CategoryAggregate {
                    category_id: 1,
                    name: "食費".into(),
                    type_: "expense".into(),
                    amount: 1_280,
                },
                CategoryAggregate {
                    category_id: 2,
                    name: "給与".into(),
                    type_: "income".into(),
                    amount: 320_000,
                },
            ],
        },
    );
}

#[test]
fn monthly_series() {
    check_fixture(
        "monthly_series",
        &vec![
            MonthlyBucket {
                year_month: "2026-05".into(),
                income: 320_000,
                expense: 148_600,
            },
            MonthlyBucket {
                year_month: "2026-06".into(),
                income: 320_000,
                expense: 1_280,
            },
        ],
    );
}

#[test]
fn list_transactions() {
    let items = sample_transactions();
    let total = items.len() as u32;
    check_fixture("list_transactions", &ListTransactionResult { items, total });
}

#[test]
fn list_categories() {
    check_fixture("list_categories", &sample_categories());
}

#[test]
fn list_accounts() {
    check_fixture("list_accounts", &sample_accounts());
}

#[test]
fn list_budget_statuses() {
    check_fixture("list_budget_statuses", &vec![budget_under(), budget_over()]);
}

#[test]
fn list_top_budget_statuses() {
    // Same item type as `list_budget_statuses`, ranked by percent descending.
    check_fixture(
        "list_top_budget_statuses",
        &vec![budget_over(), budget_under()],
    );
}

#[test]
fn import_json() {
    check_fixture(
        "import_json",
        &ImportResult {
            categories: 12,
            accounts: 3,
            recurring_rules: 2,
            transactions: 248,
            budgets: 4,
            warnings: vec![
                "skipped duplicate category: 食費".into(),
                "merged duplicate recurring rule: 家賃 (last generated 2026-04-25 -> 2026-05-25)"
                    .into(),
            ],
        },
    );
}

#[test]
fn export_backup_to_file() {
    // The command returns `Option<BackupFileResult>`; `None` (user cancelled
    // the save dialog) serializes to JSON `null`. This is the `Some` case.
    check_fixture(
        "export_backup_to_file",
        &BackupFileResult {
            path: "/Users/example/Documents/budget-backup-20260601T090000Z.json".into(),
            last_backup_at: "2026-06-01T09:00:00.000Z".into(),
        },
    );
}

#[test]
fn app_error() {
    // `AppError` serializes as its Display string; Tauri delivers it to JS as
    // the rejected value of `invoke()`.
    check_fixture(
        "app_error",
        &AppError::InvalidArgument("amount must be > 0".into()),
    );
}

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
            backfill_last: Some("2026-02-27".into()),
            truncated: false,
            upcoming: vec![
                "2026-03-27".into(),
                "2026-04-27".into(),
                "2026-05-27".into(),
            ],
        },
    );
}
