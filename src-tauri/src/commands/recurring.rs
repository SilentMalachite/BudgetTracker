//! 定期取引ルールの CRUD と起動時展開。
//!
//! spec §5.4 のとおり、展開は `setup()` ではなくフロントからの明示コマンドで走らせる。
//! 生成件数とスキップ理由を UI に返せること、展開の失敗が起動を止めないことが理由。

use chrono::NaiveDate;
use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::ledger::{
    assert_account_writable, assert_category_matches_tx, AllowedArchivedRefs, TxType,
};
use crate::domain::recurring::{
    self, RawRuleInput, RecurringRule, ValidatedRule,
};
use crate::error::{AppError, AppResult};
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
///
/// `allow` は「アーカイブ済みでも通してよい参照先」。新規作成は
/// `AllowedArchivedRefs::none()`、更新は既存行が今指している参照先を渡す
/// (`update_transaction` と同じ扱い)。そうしないと、口座を後からアーカイブされた
/// ルールは展開でスキップされ続けるのに、付け替える編集すらできなくなる。
fn assert_references_usable(
    conn: &Connection,
    rule: &ValidatedRule,
    allow: &AllowedArchivedRefs,
) -> AppResult<()> {
    let account = account_repo::find_by_id(conn, rule.account_id)?;
    assert_account_writable(&account, allow.account_id)?;

    if let Some(counter_id) = rule.counter_account_id {
        let counter = account_repo::find_by_id(conn, counter_id)?;
        assert_account_writable(&counter, allow.counter_account_id)?;
    }

    if let Some(category_id) = rule.category_id {
        let category = category_repo::find_by_id(conn, category_id)?;
        assert_category_matches_tx(&category, rule.type_, allow.category_id)?;
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
    // 新規作成はアーカイブ済みの参照先を一切認めない。
    assert_references_usable(conn, &validated, &AllowedArchivedRefs::none())?;

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
    // 今この行が指している参照先だけは、アーカイブ済みでも通す。
    let existing = recurring_repo::find_by_id(conn, id)?;
    let allow = AllowedArchivedRefs {
        account_id: Some(existing.account_id),
        category_id: existing.category_id,
        counter_account_id: existing.counter_account_id,
    };
    assert_references_usable(conn, &validated, &allow)?;

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
    // 時計はコマンド層で読む (repo は今日を知らない)。再開時に watermark を
    // 進めるのに使う。
    let today = iso(chrono::Local::now().date_naive());
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        recurring_repo::set_active(&tx, id, active, &today)?;
        let rule = recurring_repo::find_by_id(&tx, id)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}

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
    /// ルール自身の形が壊れている。type と counter_account_id/category_id の組み合わせが
    /// transactions の CHECK を満たさない / 振替の相手が自分自身 / 周期と day_of_month・
    /// day_of_week の組み合わせが噛み合わない / 参照先の行が存在しない、のいずれか。
    /// `recurring_rules` には `transactions` と同じ CHECK が無いため、import_json 経由で
    /// 作られうる。ユーザーがルールを直す (または削除する) までスキップし続ける。
    MalformedRule,
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

/// `NotFound` はルール自身の破損として扱いスキップに変換する。それ以外のエラー
/// (本物の DB 障害) はそのまま伝播させ、展開全体を止める。
fn find_or_skip<T>(result: AppResult<T>) -> AppResult<Result<T, SkipReason>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(AppError::NotFound(_)) => Ok(Err(SkipReason::MalformedRule)),
        Err(e) => Err(e),
    }
}

/// 参照先が使えるか。使えないなら理由を返す。
fn classify_skip(conn: &Connection, rule: &RecurringRule) -> AppResult<Option<SkipReason>> {
    // transactions の CHECK (`type='transfer'` なら counter_account_id 必須・category_id NULL、
    // それ以外なら category_id 必須・counter_account_id NULL) を、DB に書く前に自分で確認する。
    // recurring_rules にはこの CHECK が無く、import_json 経由で組み合わせが壊れた行が
    // 入りうるため。同一口座間の振替は transactions の CHECK では捕まえられない
    // (counter_account_id が NULL でないことしか見ていない) ので、ここで弾く。
    let shape_ok = match rule.type_ {
        TxType::Transfer => {
            rule.counter_account_id
                .is_some_and(|counter| counter != rule.account_id)
                && rule.category_id.is_none()
        }
        TxType::Income | TxType::Expense => {
            rule.category_id.is_some() && rule.counter_account_id.is_none()
        }
    };
    // 周期と日付列の組み合わせ。`occurrences_between` は day_of_week / day_of_month が
    // 無ければ starts_on から推測する全域関数 (proptest で守られている) なので、
    // 壊れたルールを弾くのは日付列挙側ではなくここ。
    let schedule_ok = match rule.frequency {
        recurring::Frequency::Weekly => rule.day_of_week.is_some() && rule.day_of_month.is_none(),
        recurring::Frequency::Monthly | recurring::Frequency::Yearly => {
            rule.day_of_month.is_some() && rule.day_of_week.is_none()
        }
    };
    if !shape_ok || !schedule_ok {
        return Ok(Some(SkipReason::MalformedRule));
    }

    let account = match find_or_skip(account_repo::find_by_id(conn, rule.account_id))? {
        Ok(account) => account,
        Err(reason) => return Ok(Some(reason)),
    };
    if account.archived_at.is_some() {
        return Ok(Some(SkipReason::ArchivedAccount));
    }

    if let Some(counter_id) = rule.counter_account_id {
        let counter = match find_or_skip(account_repo::find_by_id(conn, counter_id))? {
            Ok(counter) => counter,
            Err(reason) => return Ok(Some(reason)),
        };
        if counter.archived_at.is_some() {
            return Ok(Some(SkipReason::ArchivedCounterAccount));
        }
    }

    if let Some(category_id) = rule.category_id {
        let category = match find_or_skip(category_repo::find_by_id(conn, category_id))? {
            Ok(category) => category,
            Err(reason) => return Ok(Some(reason)),
        };
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
/// 展開できたルールは、生成が 0 件でも `last_generated_on` を `today` まで進める。
/// 参照先が使えないルールだけは飛ばし、`last_generated_on` も進めない。
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

        // watermark は「最後に生成した日」ではなく「どこまで調べ終えたか」。生成が
        // 0 件でも today まで進める。`occurrences_between` が返す日付は窓
        // `(after, today]` の定義から必ず today 以下なので、today まで進めても
        // 発生日を飛ばすことはない。
        //
        // 逆に最後の発生日で止めると watermark が時計から遅れ続け、そのあと
        // ユーザーがルールの発生日を変えたとき、次の窓が「もう締めた期間」まで
        // 遡って過去日の取引を生んでしまう (3/27 で止めたまま 4/15 に「毎月 1 日」
        // へ付け替えると 4/1 が生える)。ルールの編集はこれから先の生成にだけ効く。
        //
        // `max` を取るのは、import_json 経由で today より先の watermark を持つ行が
        // 入りうるため。巻き戻すと同じ日を二重生成する。
        let watermark = iso(after.map_or(today, |previous| previous.max(today)));
        recurring_repo::set_last_generated_on(conn, rule.id, &watermark)?;

        if dates.is_empty() {
            continue;
        }

        recurring_repo::insert_generated(conn, &rule, &dates, now)?;
        result.generated += dates.len() as i64;
        result.rules.push(RuleExpansion {
            rule_id: rule.id,
            rule_name: rule.name,
            generated: dates.len() as i64,
            last_generated_on: watermark,
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
