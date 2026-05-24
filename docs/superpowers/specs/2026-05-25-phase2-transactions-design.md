# Phase 2 設計書: 取引 CRUD + カテゴリ + 口座 + ダッシュボード

- 作成日: 2026-05-25
- ステータス: ドラフト (ユーザーレビュー待ち)
- 親 spec: [2026-05-24-budget-tracker-design.md](./2026-05-24-budget-tracker-design.md)
- 対象: 親 spec セクション 10 の Phase 2

## 0. 位置づけ

本書は親 spec の Phase 2「取引 CRUD + カテゴリ + ダッシュボード」を具体化した実装スコープ仕様である。親 spec のデータモデル・アーキテクチャ方針はそのまま採用し、Phase 2 で「どこまでやるか」「どう作るか」を確定させる。

Phase 1 (`2960bf2`) で SQLCipher 暗号化 DB と Keychain 鍵保存、`V001__init.sql` のスキーマ作成までは完了している。

## 1. スコープ

### 1.1 Phase 2 で実装する (IN)

- カテゴリ CRUD (収入/支出種別、色・アイコン、論理削除)
- 口座 CRUD (kind / 開始残高 / メモ、論理削除、複数作成可能)
- 取引 CRUD (`type='income'` / `type='expense'` のみ) と一覧フィルタ (期間 / 種別 / カテゴリ / 口座 / フリーワード)
- ダッシュボード: 当月収支カード (収入・支出・純額)、直近取引10件、月別収支グラフ (Chart.js 棒、12ヶ月)
- JSON エクスポート/インポート (全テーブル、上書き/追記モード)
- 設定画面: 最終バックアップ日時、DB ファイルパス表示
- 初回起動時のデフォルトカテゴリシード (再シード防止フラグ付き)

### 1.2 Phase 2 で実装しない (OUT)

- `transactions.type='transfer'` (振替) と `counter_account_id` を使う UI → Phase 3
- 予算管理 (`budgets` テーブル、進捗バー、アラート) → Phase 4
- 定期取引 (`recurring_rules`、起動時冪等展開) → Phase 5
- レポート 4 タブ (年次 / カテゴリ別円グラフ / 純資産トレンド) → Phase 5
- Excel インポート/エクスポート → Phase 4.5 (別途計画)
- ダークモード、自動アップデート

`transfer` 種別を持つ取引が DB に存在しても集計から除外されるよう、Phase 2 の集計 SQL は `WHERE type IN ('income','expense')` を必ず明示する (親 spec 規約 #3)。

## 2. アーキテクチャ追補

### 2.1 DB 接続管理

- Tauri `State<Mutex<rusqlite::Connection>>` で単一接続を共有する
- 理由: デスクトップ単一プロセス・単一ユーザーで、SQLite はシングルライタが自然。コネクションプール (r2d2 等) は過剰
- 書き込みは `BEGIN IMMEDIATE` トランザクションで囲み、import 系は1トランザクションで全件処理

### 2.2 タイムスタンプ

- `created_at` / `updated_at` は Rust 側で `chrono::Utc::now().to_rfc3339()` を入れる
- `occurred_on` は `'YYYY-MM-DD'` 文字列 (タイムゾーン無し、ユーザーの「日付」概念に合わせる)

### 2.3 論理削除と一覧クエリ

- `archived_at TEXT` に ISO 8601 を入れて archive
- 一覧クエリは標準で `WHERE archived_at IS NULL` を付ける
- 設定画面 (またはカテゴリ/口座管理画面内のトグル) で「アーカイブ済みも表示」を選べる
- 取引 (`transactions`) は履歴を持たないため物理削除 (`DELETE`) で可

### 2.4 イベント通知

書き込み系コマンドは成功時に `data:changed` イベントを emit する。ペイロード:

```json
{ "domain": "categories" | "accounts" | "transactions" | "meta" }
```

フロントエンドのストアは関心のある domain を購読し、変更があれば再フェッチする。

## 3. データ層

### 3.1 ディレクトリ追加

```
src-tauri/src/
├── commands/
│   ├── categories.rs        (新規)
│   ├── accounts.rs          (新規)
│   ├── transactions.rs      (新規)
│   ├── reports.rs           (新規)
│   ├── backup.rs            (新規 - JSON import/export)
│   └── settings.rs          (新規)
├── domain/
│   ├── category.rs          (新規)
│   ├── account.rs           (新規)
│   ├── ledger.rs            (新規)
│   └── report.rs            (新規)
└── infra/
    └── repo/                (新規ディレクトリ)
        ├── mod.rs
        ├── category_repo.rs
        ├── account_repo.rs
        ├── transaction_repo.rs
        └── meta_repo.rs
```

`domain/` は SQL を持たない純粋関数中心、`infra/repo/` が rusqlite 直叩きを集約する。

### 3.2 マイグレーション

- 既存 `V001__init.sql` は触らない (親 spec 規約 #6)
- 初回起動時シードは SQL マイグレーションではなく Rust 側で実施 (`app_meta.categories_seeded='true'` の有無で1回限り適用)

## 4. Tauri コマンド一覧

### 4.1 categories

```rust
list_categories(filter: ListCategoryFilter) -> Vec<Category>
  ListCategoryFilter { type_: Option<"income"|"expense">, include_archived: bool }

create_category(input: CreateCategoryInput) -> Category
  CreateCategoryInput { name: String, type_: String, color: Option<String>, icon: Option<String> }

update_category(id: i64, patch: UpdateCategoryPatch) -> Category
  UpdateCategoryPatch { name?, color?, icon?, display_order? }

archive_category(id: i64) -> ()
unarchive_category(id: i64) -> ()
```

### 4.2 accounts

```rust
list_accounts(include_archived: bool) -> Vec<Account>

create_account(input: CreateAccountInput) -> Account
  CreateAccountInput { name, kind, initial_balance: i64, note?: String }

update_account(id, patch: UpdateAccountPatch) -> Account
  UpdateAccountPatch { name?, kind?, initial_balance?, note?, display_order? }

archive_account(id) -> ()
unarchive_account(id) -> ()
```

### 4.3 transactions

```rust
list_transactions(filter: ListTransactionFilter, page: u32, page_size: u32)
  -> { items: Vec<Transaction>, total: u32 }
  ListTransactionFilter {
    from?: String, to?: String,
    type_?: "income"|"expense",
    category_id?: i64, account_id?: i64,
    search?: String,
  }

create_transaction(input: CreateTransactionInput) -> Transaction
  CreateTransactionInput {
    occurred_on, type_ ("income"|"expense"),
    amount: i64, account_id, category_id, description,
  }

update_transaction(id, patch: UpdateTransactionPatch) -> Transaction
delete_transaction(id) -> ()
```

Phase 2 では `type_='transfer'` を受け取ったら `AppError::InvalidArgument` で拒否する。

### 4.4 reports

```rust
monthly_summary(year: u32, month: u32) -> MonthlySummary
  MonthlySummary {
    income: i64, expense: i64, net: i64,
    by_category: Vec<{ category_id, name, type_, amount }>,
  }

monthly_series(months: u32) -> Vec<MonthlyBucket>
  MonthlyBucket { year_month: "YYYY-MM", income: i64, expense: i64 }
```

`monthly_series` は当月を含む直近 `months` ヶ月を返し、取引が無い月も `0` で埋める。

### 4.5 backup

```rust
export_json() -> String   // 全テーブルのスナップショット (schema_version 含む)

import_json(payload: String, mode: "overwrite" | "append") -> ImportResult
  ImportResult { categories: u32, accounts: u32, transactions: u32, warnings: Vec<String> }
```

- `overwrite`: 既存テーブルを TRUNCATE してから INSERT。`AUTOINCREMENT` シーケンスもリセット
- `append`: 既存にマージ。`(categories.name, type)` の UNIQUE 制約衝突や、参照先 (account_id/category_id) が見つからない取引は `warnings` に積んでスキップ
- 両モードとも単一トランザクション内で処理し、エラー時は完全ロールバック
- `schema_version` が現行と不一致なら拒否

### 4.6 settings

```rust
get_last_backup_at() -> Option<String>
set_last_backup_at(iso: String) -> ()
get_db_path() -> String   // 表示用 ("~/Library/.../data.db" 等)
```

## 5. domain 層の責務

### 5.1 `domain/category.rs`

```rust
pub fn validate_name(name: &str) -> Result<(), AppError>     // 空白trim後の長さ 1..=40
pub fn validate_color(color: &str) -> Result<(), AppError>   // /^#[0-9A-Fa-f]{6}$/
pub fn validate_type(type_: &str) -> Result<CategoryType, AppError>  // income | expense
```

### 5.2 `domain/account.rs`

```rust
pub fn validate_name(name: &str) -> Result<(), AppError>     // 1..=40
pub fn validate_kind(kind: &str) -> Result<AccountKind, AppError>
  // cash | bank | credit_card | e_money | investment
pub fn validate_initial_balance(amount: i64) -> Result<(), AppError>  // i64 全域、桁数表示は UI 責務
```

### 5.3 `domain/ledger.rs`

```rust
pub fn validate_transaction(input: &TransactionInput) -> Result<(), AppError>
  // amount > 0, occurred_on は YYYY-MM-DD パース可、description 0..=200
  // Phase 2 では type_ ∈ {income, expense} のみ受理。transfer は InvalidArgument

pub fn aggregate_monthly(txs: &[Transaction], year: u32, month: u32) -> MonthlySummary
  // 純粋関数。WHERE type IN ('income','expense') 相当のフィルタを内部で適用
```

### 5.4 `domain/report.rs`

```rust
pub fn fill_monthly_series(buckets: &[MonthlyBucket], end: YearMonth, months: u32)
  -> Vec<MonthlyBucket>
  // SQL から返った歯抜けの月次集計を 0 埋めで連続化する純粋関数
```

## 6. フロントエンド構造

### 6.1 ディレクトリ

```
src/
├── App.svelte
├── lib/
│   ├── api/
│   │   ├── index.ts
│   │   ├── categories.ts
│   │   ├── accounts.ts
│   │   ├── transactions.ts
│   │   ├── reports.ts
│   │   ├── backup.ts
│   │   └── settings.ts
│   ├── stores/
│   │   ├── categories.svelte.ts
│   │   ├── accounts.svelte.ts
│   │   └── transactions.svelte.ts
│   ├── components/
│   │   ├── Card.svelte
│   │   ├── Button.svelte
│   │   ├── Modal.svelte
│   │   ├── TextField.svelte
│   │   ├── Select.svelte
│   │   ├── DatePicker.svelte
│   │   ├── CategoryBadge.svelte
│   │   └── Sidebar.svelte
│   └── utils/formatCurrency.ts
└── routes/
    ├── Dashboard.svelte
    ├── Transactions.svelte
    ├── Categories.svelte
    ├── Accounts.svelte
    └── Settings.svelte
```

### 6.2 状態管理パターン

各ストアは `$state` でデータを保持し、起動時に `load()` で初回フェッチ、`listen('data:changed', ...)` で自分の domain を再フェッチする:

```ts
// categories.svelte.ts (概念)
export function createCategoriesStore() {
  let items = $state<Category[]>([]);
  async function load() { items = await listCategories({ include_archived: false }); }
  listen<{ domain: string }>('data:changed', (e) => {
    if (e.payload.domain === 'categories') load();
  });
  return { get items() { return items; }, load };
}
```

ビジネスロジックは Svelte 側に書かない (親 spec 規約 #2)。フィルタ・集計は Rust コマンドを呼ぶ。

### 6.3 スタイリング

- `src/app.css` にデザイントークン (色変数、グラデーション、影、角丸) を定義
- 各コンポーネントは Svelte の `<style>` ブロックでスコープ付き CSS
- 既存 `家計簿.html` の配色 (`#667eea → #764ba2`, `#4facfe → #00f2fe`) と丸いカードを継承

### 6.4 ルーティング

- `svelte-spa-router` を依存追加
- ルート: `/` (Dashboard), `/transactions`, `/categories`, `/accounts`, `/settings`

## 7. デフォルトカテゴリ (初回シード)

初回起動時、`app_meta.categories_seeded` が未設定かつ `categories` テーブルが空のときのみ以下を INSERT し、`app_meta.categories_seeded='true'` を立てる。ユーザーが全削除した状態を尊重するため再シードはしない。

**支出 (display_order 順)**
食費 / 日用品 / 交通費 / 住居費 / 水道光熱費 / 通信費 / 医療費 / 衣服 / 交際費 / 趣味・娯楽 / その他

**収入 (display_order 順)**
給与 / 賞与 / 副業 / その他

color / icon は未設定 (UI 側でカテゴリ追加時にユーザーが選ぶ。デフォルト時は配色パレットから自動割り当て)。

## 8. テスト戦略

### 8.1 Rust unit (`cargo test`)

- `domain/category.rs::validate_*` の境界値
- `domain/account.rs::validate_kind` の全 enum + 不正値
- `domain/ledger.rs::validate_transaction`: `amount=0` / 負数 / `transfer` 拒否 / 不正日付
- `domain/ledger.rs::aggregate_monthly`: 振替が混ざっても集計から除外される
- `domain/report.rs::fill_monthly_series`: 歯抜けが 0 埋めされる
- proptest: `aggregate_monthly` の不変条件「全取引について `sum(income) - sum(expense) = net`」

### 8.2 Rust 統合 (`cargo test`、メモリ DB)

- migrations が `:memory:` 上で V001 まで適用される
- create → list → update → archive → list (include_archived) のフルフロー
- JSON ラウンドトリップ: export → DB クリア → import(overwrite) → state が一致

> SQLCipher を memory DB で有効化する手間を避けるため、テスト用 helper は `rusqlite::Connection::open_in_memory()` を直接使い、暗号化レイヤはバイパスする (Phase 1 で既に動いている前提に立つ)。

### 8.3 Vitest (`pnpm test`)

- `formatCurrency` (既存)
- API ラッパーが invoke 呼び出しを正しく組み立てる (mock)
- ストアが `data:changed` で再フェッチする

### 8.4 Playwright (`pnpm test:e2e`)

- 最低 1 本: 「カテゴリ追加 → 口座追加 → 取引追加 → ダッシュボードに今月の収支が反映」
- 親 spec の制約により、Phase 2 でも `pnpm dev` (ブラウザ) に対してのみ。Tauri ウィンドウ E2E は Phase 3 以降の課題

## 9. 完了基準 (Phase 2 Definition of Done)

CLAUDE.md の標準 6 項目を満たすこと:

1. `cargo clippy --all-targets -- -D warnings` 緑
2. `cargo test` 緑 (新規ユニット + 統合 + proptest)
3. `pnpm test` 緑
4. `pnpm check` (svelte-check) 緑
5. Playwright E2E 1 本緑
6. macOS と Windows (CI) でビルド成功

加えて以下の機能受け入れ条件:

- ダッシュボードで取引追加直後にカードと棒グラフが再描画される (手動確認)
- JSON エクスポート → import(overwrite) で同一状態に復元 (統合テストで自動検証)
- 1 万件取引で `list_transactions` ページング初回応答 200ms 以内、`monthly_summary` 100ms 以内 (簡易ベンチで計測)
- `transactions.type='transfer'` の行を DB に手動投入しても、`monthly_summary` の `income` / `expense` に含まれない

## 10. リスクと対応

| リスク | 影響 | 対応 |
|---|---|---|
| `svelte-spa-router` が Svelte 5 Runes と相性悪い | UI 書き直し | 採用前に最小 PoC で確認。問題があれば `@roxi/routify` か手書きの $state ルータに切替 |
| Chart.js の Svelte 5 統合 | グラフ描画失敗 | `onMount` 内で素の Chart インスタンス生成、$effect で破棄 |
| `Mutex<Connection>` で重い集計が UI ブロック | レスポンス低下 | Phase 2 のクエリは index 設計済みで軽い前提。重ければ `tokio::task::spawn_blocking` を導入 |
| JSON インポートで巨大ペイロード | メモリ圧迫 | Phase 2 は 10MB 上限で拒否。ストリーミングは将来 |
| デフォルトカテゴリの再シード暴発 | ユーザー削除を上書き | `app_meta.categories_seeded` フラグで1回限り保証、テストで検証 |

## 11. オープン項目

なし (Phase 2 着手前に解消済み)。

---

## 改訂履歴

- 2026-05-25: 初版作成。親 spec の Phase 2 を具体化
