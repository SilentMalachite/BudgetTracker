# BudgetTracker コードレビュー

- 日付: 2026-08-29
- 対象: リポジトリ全体（`main` @ `9095660` `feat: implement phase 4 budgets`）
- 正本: [docs/superpowers/specs/2026-05-24-budget-tracker-design.md](../specs/2026-05-24-budget-tracker-design.md)
- 範囲: Rust バックエンド（domain / commands / infra / SQLCipher / Keychain）、Svelte UI、仕様・CI・テスト
- 作業ツリー: ほぼクリーン。未追跡は `docs/superpowers/plans/2026-05-26-phase6-release-readiness.md` のみ（欠陥としては扱わない）

## 判定

**Phase 4 の実装は本物**で、次フェーズに進める土台はある。金額は整数円、集計・予算・振替除外は Rust/SQL、カテゴリ/口座は論理削除、SQLCipher に平文フォールバックはない。

**配布・日常利用の品質としては、下記 Bug 1–3 を直してから**が妥当。Phase 5（定期取引の冪等展開、レポート4タブ）の未実装は現状の宣言どおりで、完了済み Phase 4 の欠陥ではない。

| 重大度 | 件数 | 状態 |
|---|---|---|
| bug | 8 | open |
| suggestion | 15 | open |
| nit | 2 | open |

## ローカル検証

このレビュー時点で実行し、緑を確認したコマンド:

- `cd src-tauri && cargo test --no-fail-fast` — 117 件（proptest 含む）
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`
- `pnpm test` — 21/21
- `pnpm check` — 0 errors

未実行: Windows 実機ビルド、Tauri ウィンドウ上の E2E（`tauri-driver`）。現行 E2E は Vite ブラウザ + mock のみ。

## よくできている点

- 振替は収入/支出/予算消化から SQL で除外されている
- 予算エンジン（`projected` / percent）と残高に domain テストと proptest がある
- JSON バックアップ往復が budgets / transfers を含む
- CSP は `default-src 'self'`、Tauri capability は `core:default` のみ
- Budgets ページと Dashboard 上位3件ウィジェット、budget E2E が実在する
- アーキテクチャ分割（薄い commands、domain + repo、Svelte は表示）が概ね守られている

## 優先して直すなら

1. 日付をローカルに統一する — フロントの `isoToday()` と Rust の `monthly_series`（`Utc`）を、Dashboard / 予算の `Local` に合わせる
2. 復号失敗のリカバリ — ウィンドウを出して JSON import できるようにする。壊れた DB の横で新しい鍵を発行しない
3. 編集時のアーカイブ ID を保持する — 自動 remap をやめる。ラベル用にはアーカイブ済みを残す

---

## Bugs

### 1. カレンダー日付が UTC とローカルで分裂している

- 重大度: bug
- ファイル: `src/lib/utils/yearMonth.ts:2`、`src-tauri/src/commands/reports.rs:35`
- 状態: open

`isoToday()` が `new Date().toISOString().slice(0, 10)` で UTC 日付を返す。Dashboard はローカル `Date`（`Dashboard.svelte:25`）、予算コマンドは `chrono::Local`（`commands/budgets.rs:65`）、月次シリーズは `chrono::Utc`（`commands/reports.rs:35`）と「今日」が3系統ある。

日本時間 00:00–08:59 では新規取引の初期日と予算ページの初期月が前日/前月になり、月初には収入カード（ローカル当月）と棒グラフ最終バー（UTC 当月）がずれる。

**修正案:** フロントは `getFullYear()` / `getMonth()` / `getDate()` でローカル `YYYY-MM-DD` を組み立て、Rust はすべてのコマンドで `chrono::Local::now().date_naive()`（または注入可能な clock）に揃える。JST 早朝のフィクスチャで回帰テストを追加する。

### 2. 鍵/復号失敗が起動パニックになり、JSON 復元に到達できない

- 重大度: bug
- ファイル: `src-tauri/src/lib.rs:28`
- 状態: open

起動 `setup` が Keychain・SQLCipher open・migration・seed を `.expect()` している。鍵紛失や `data.db` と鍵の不一致はウィンドウ作成前にパニックする。仕様 §6.2 / §11 は「JSON バックアップから復元」を案内する経路を要求するが、`import_json` は DB オープン後にしか呼べない。鍵喪失時の実際の挙動は「復元画面」ではなく「起動不能」である。

**修正案:** `setup` をパニックさせず、旧 DB なしで新規暗号化ファイルを作り `import_json` できるリカバリ UI（または専用コマンド）を出す。Keychain 破損時に既存 `data.db` の横で新しい鍵を黙って発行しない。

### 3. アーカイブ済み口座/カテゴリの履歴編集が別マスタへ書き換わる

- 重大度: bug
- ファイル: `src/routes/Transactions.svelte:226`
- 状態: open

編集モーダルの選択肢は非アーカイブのカテゴリ/口座のみ。`$effect` はリストに無い値を先頭の可視オプションへ書き換える（229–239 行）。後からアーカイブした口座/カテゴリを持つ履歴行を「編集」すると、保存前に別の口座/カテゴリへ silently remap され、更新で誤った相手先が永続化する。一覧のカテゴリ名も `include_archived: false` のため `-` になる。

**修正案:** ラベル用マップにはアーカイブ済みを残す。編集時は当該行の現行 ID を「アーカイブ済」として選択肢に残し、新規作成時だけ未アーカイブへ制限する。既存 ID を自動置換しない。

### 4. 一覧失敗が空の帳簿に見える

- 重大度: bug
- ファイル: `src/routes/Transactions.svelte:301`
- 状態: open

`txStore.error` を読んでいない。`list_transactions` 失敗後も `items` は `[]` のまま EmptyState「該当する取引がありません」になる。Categories / Accounts も `store.error` 未表示。バックエンド障害が空帳簿に見える。

**修正案:** Budgets ページ（`Budgets.svelte:117`）と同様にエラーを出し、成功かつ 0 件のときだけ空状態 CTA を出す。

### 5. 削除・アーカイブ失敗がユーザーに届かない

- 重大度: bug
- ファイル: `src/routes/Transactions.svelte:185`
- 状態: open

`remove()` は `deleteTransaction` を try/catch せず、失敗は unhandled rejection になり行は残ったままメッセージが出ない。Accounts / Categories の `toggleArchive` も同様。作成/更新モーダルだけ `formError` がある。

**修正案:** 書き込みを wrap し、行横またはページバナーでエラーを出す。confirm は破壊確認専用にする。

### 6. store の並行 `load()` が古い応答で上書きできる

- 重大度: bug
- ファイル: `src/lib/stores/transactions.svelte.ts:41`
- 状態: open

`load()` に request generation が無い。`setFilter` / `setPage` / `data:changed` が並行 `void load()` するため、古い `listTransactions` が後着して新しい page/filter の `items`/`total` を上書きできる。同じ last-write-loses が accounts / balances / budgets / categories store と Dashboard `reload()` にもある。

**修正案:** `load` 開始時に単調増加 `requestId`（または `{filter,page,yearMonth}` キー）を取り、stale 応答を無視する。読み込み中はページャを無効化するのもよい。

### 7. Dashboard 再取得失敗時に古い収支が残る

- 重大度: bug
- ファイル: `src/routes/Dashboard.svelte:57`
- 状態: open

`Promise.all` 失敗時に `series` / `recent` / `budgetStatuses` は空にするが、`summary` は前回成功値のまま。エラーバナーの下に古い収入/支出/収支が出る。初回は `summary` が null で `---`、失敗するのは再取得時だけ。

**修正案:** catch で `summary = null` にするか、スナップショット全体を stale と明示する。エラーと生きている P/L 数字を混ぜない。

### 8. アーカイブ済み口座が開始残高を表示する

- 重大度: bug
- ファイル: `src/routes/Accounts.svelte:161`
- 状態: open

アーカイブ済みリストが `account.initial_balance` を表示している。`list_balances` はアーカイブ口座も含め、`balanceById` には計算残高がある。収入/支出/振替後のアーカイブ口座は開始残高になり、実残高とずれる。

**修正案:** アクティブ一覧と同じく `balanceById.get(account.id)` を使う。ロード中/エラー時は開始残高へ黙って落とさない。

---

## Suggestions

### 9. 親 spec が未実装機能を現行として書いている

- 重大度: suggestion
- ファイル: `docs/superpowers/specs/2026-05-24-budget-tracker-design.md:270`
- 状態: open

AGENTS.md は spec とコードの同期を要求するが、親 spec §5.2 は Excel インポート/エクスポートを現行機能として書き、§5.6 は `report_yearly` / `report_by_category` / `report_net_worth_series` と Recurring/Reports ルートを現ツリーとして書く。Phase 2 spec は Excel を「Phase 4.5」へ先送りし、Phase 4 計画は Excel を含まず、Phase 5 が定期+レポート4タブである。README は JSON バックアップのみで正しい。V001 の `recurring_rules` / `recurring_id` は先読みスキーマであり実行時バグではない。

**修正案:** 親 spec で Excel を明示的に後回し、レポート実装済みコマンドを `monthly_summary` / `monthly_series` と書き、定期と4タブは Phase 5 とラベルする。

### 10. 予算の `ends_on` 継続ルールが未実装

- 重大度: suggestion
- ファイル: `src-tauri/src/infra/repo/budget_repo.rs:88`
- 状態: open

spec 4.1/5.3 は `ends_on` NULL を継続、`starts_on`/`ends_on` を改定履歴とする。Phase 4 の lookup は `period = 'monthly' AND starts_on = ?1`（選択月の1日）のみで `ends_on` を見ない。`set_monthly_budget` はその月に `ends_on = NULL` を書く。5月予算はユーザーが6月を別設定しない限り6月に効かない。Phase 4 計画の月単位 UI には合うが、親 spec の継続ルールとは矛盾する。

**修正案:** `starts_on <= 月初 AND (ends_on IS NULL OR ends_on >= 月初)` の最新行を取るか、親 spec を「Phase 4 はカテゴリ×月の1行、`ends_on` 未使用」と直す。

### 11. バックアップ JSON の schema_version が DB と衝突し、import 検証が弱い

- 重大度: suggestion
- ファイル: `src-tauri/src/commands/backup.rs:13`
- 状態: open

JSON の `schema_version` は `SUPPORTED_SCHEMA_VERSION = 1` 固定、DB の `app_meta.schema_version` は V003 後に 3。公式 export/import は常に 1 なので現行ラウンドトリップは通る。手編集で `"schema_version": 3` のファイルは拒否される。import は domain validator を通さず、空名前、`kind` の `"cash"` デフォルト、`amount.unwrap_or(0)`、同一口座振替が SQL に入り、CHECK/FK 失敗はトランザクション全体が generic `AppError::Db`。同一口座振替は `validate_transfer_input` だけが防いでおり V001 CHECK には無い。

**修正案:** バックアップ形式バージョンを `app_meta.schema_version` と分けて文書化する。行はコマンドと同じ domain 関数で事前検証し、`{index, message}` を集める。同一口座振替を拒否する。1 トランザクション rollback は維持。

### 12. `Cargo.lock` が gitignore されている

- 重大度: suggestion
- ファイル: `.gitignore:24`
- 状態: open

`src-tauri/Cargo.lock` が gitignore され未追跡。これはアプリケーション crate であり、再現ビルドのため lockfile をコミットすべき。フロントの `pnpm-lock.yaml` はコミット済みで CI は `--frozen-lockfile`。Rust 層だけ再現性が弱い。

**修正案:** ignore をやめ、lockfile をコミットし、CI で drift を落とす。

### 13. CI は PR バーまで。配布パイプラインと Tauri E2E は未着手

- 重大度: suggestion
- ファイル: `.github/workflows/ci.yml:17`
- 状態: open

PR CI は `pnpm check` / `pnpm test` / Playwright / `clippy -D warnings` / `cargo test` を macos と windows で走らせ、spec §8 の PR バーには合う。不足は (1) spec の `release.yml` が無い（Phase 6）、(2) README の「build」は `pnpm tauri build` ではなく Playwright は `pnpm dev`、(3) actions が tag pin で SHA pin ではない、(4) E2E は Vite+mock で spec §8 の Tauri WebView ではない（AGENTS.md 記載済み）。

**修正案:** 現行 PR ジョブは維持。Phase 6 で `release.yml`。spec §8 の E2E 文言を「tauri-driver まで Vite ブラウザ smoke」に更新。必要なら action を SHA pin。

### 14. Playwright mock が Rust 不変条件を再実装している

- 重大度: suggestion
- ファイル: `tests/e2e/budget-flow.spec.ts:77`
- 状態: open

domain/integration は振替除外・予算 projected・バックアップ往復を本当に検証している。一方 Playwright は spent/percent を JS 再実装し `projected: spent` を固定するため、Rust の投影や振替除外が壊れても mock が UI に同意すれば E2E は緑のまま。`transaction-flow.spec.ts` の mock は `list_balances` / `list_budget_statuses` を `null` 返し、Dashboard がスプレッドで落ちうる。

**修正案:** UI 配線の mock E2E は残しつつ、各 flow はマウント先が呼ぶコマンドを `[]` で stub する。投影バッジは Rust 側の契約テストを正とする。

### 15. 総資産と予算 top 3 が Svelte 側で集計されている

- 重大度: suggestion
- ファイル: `src/lib/stores/balances.svelte.ts:56`
- 状態: open

AGENTS.md 規則 2 は Svelte 側 `filter`/`reduce` のビジネスロジックを禁じる。`totalAssets` は非アーカイブ残高を store で合計し、Dashboard は予算 top 3 を percent でソートしている。Rust が返した行の表示集計ではあるが、規則が禁じるパターンそのもの。`accounts.kind` の資産/負債分割も未使用。

**修正案:** `list_balances` が `total_assets`（必要なら資産/負債内訳）を返し、top-N はコマンド側で切る。

### 16. コマンドがカテゴリ種別・アーカイブ FK を検証しない

- 重大度: suggestion
- ファイル: `src-tauri/src/domain/ledger.rs:73`
- 状態: open

`validate_input` は日付・金額・`category_id` の有無を見るが、`categories.type` と取引種別の一致、口座/カテゴリの存在・未アーカイブは見ない。`create_transaction` / `update_transaction` は支出カテゴリへの収入行を受け付ける。Svelte フォームは種別で絞るがコマンド面は絞らない。アーカイブ済みも FK として有効。

**修正案:** カテゴリ/口座をロードし `category.type == tx.type` を要求、アーカイブを拒否、欠落 FK は raw `FOREIGN KEY constraint failed` ではなく `AppError::NotFound`。

### 17. 振替行を `update_transaction` で収入/支出に書き換えられる

- 重大度: suggestion
- ファイル: `src-tauri/src/domain/ledger.rs:84`
- 状態: open

`create_transaction` は `type=transfer` を「Phase 2 では未対応」で拒否する。振替は Phase 3 から `create_transfer` がある。`transaction_repo::update` は `counter_account_id = NULL` で `WHERE id = ?` しており、直接 `update_transaction` すると振替行を収入/支出に書き換えられる。UI は種別変更を塞ぐが repo は塞がない。

**修正案:** エラーを `create_transfer`/`update_transfer` へ誘導する文言にする。`update` は既存 `type='transfer'` を拒否する（`update_transfer` の `AND type = 'transfer'` と対称）。

### 18. LIKE ワイルドカード未エスケープ、`page_size` 無制限

- 重大度: suggestion
- ファイル: `src-tauri/src/infra/repo/transaction_repo.rs:71`
- 状態: open

検索は bound の `description LIKE ?` で SQL インジェクションは防いでいるが、クエリ中の `%` `_` は LIKE ワイルドカードのまま。`page_size` に上限が無く、巨大 invoke で台帳全体を実体化しうる。

**修正案:** `%`/`_` をエスケープし、コマンドで `page_size` を 1..=200 に制限し、LIMIT/OFFSET もパラメータにする。

### 19. 統合テストが FOREIGN KEY オフで走っている

- 重大度: suggestion
- ファイル: `src-tauri/src/infra/migrations.rs:98`
- 状態: open

各 migration はトランザクション内。V001 の `PRAGMA foreign_keys = ON` はトランザクション中は no-op。本番は `open_encrypted` が先に FK ON（`db.rs:25`）。統合テストは `Connection::open_in_memory()` + `migrations::run` で SQLite デフォルトの FK オフのため、欠落親の INSERT を捉えられない。

**修正案:** テストヘルパ（または `run()` のトランザクション外）で接続に `PRAGMA foreign_keys=ON` を付ける。

### 20. モーダルにフォーカストラップと Escape が無い

- 重大度: suggestion
- ファイル: `src/lib/components/Modal.svelte:22`
- 状態: open

spec §9 はプロトタイプの aria/focus を継承する。現行モーダルはフォーカス移動も trap も無く、Escape は backdrop の `keydown` のみ。フォーカスはオーバーレイ後ろの opener に残るため Escape が効かないことが多い。`children`/`footer` は `any`。

**修正案:** 開いたら先頭フィールドへ focus、閉じたら復元、`.dialog` 内 Tab trap、open 中は document で Escape、snippet を `Snippet` 型にする。

### 21. 取引フィルタにカテゴリ・口座が無い

- 重大度: suggestion
- ファイル: `src/routes/Transactions.svelte:280`
- 状態: open

Phase 2 spec §1.1 と親 spec §5.2 は種類/カテゴリ/口座/月/フリーワードのフィルタを要求する。API は `category_id` / `account_id` を受け付けるが、UI は type・from/to・search のみ。

**修正案:** カテゴリ/口座の select（任意で月コントロールが `from`/`to` を埋める）を追加する。フィルタは invoke payload に載せ、`txStore.items` をクライアントで絞らない。

### 22. Dashboard の `onDataChanged` が unmount 後に残る

- 重大度: suggestion
- ファイル: `src/routes/Dashboard.svelte:97`
- 状態: open

store は `dispose()` 後の `onDataChanged` を無視する。Dashboard は `await reload()` の後に listen し、disposed フラグが無い。初回 reload 中に画面を離すと unmount 後の `reload()`/`drawChart()` が残る。Chart.js の `onDestroy` destroy 自体は正しい。

**修正案:** store と同じ `disposed` + unmount 後に listen が返ったら即 unsubscribe、dispose 後は `drawChart` しない。

### 23. Keychain サービス名と DB パスが spec と食い違う

- 重大度: suggestion
- ファイル: `src-tauri/src/lib.rs:14`
- 状態: open

アプリ ID は `jp.budget-tracker.app`、Keychain は全 OS で `jp.budget-tracker` / `db_key`。macOS の spec §6.1 には合う。spec の保存パスは `.../jp.budget-tracker/data.db`、Windows Credential Manager 例は `BudgetTracker/db_key` で、実装の `app_data_dir()`（`.../jp.budget-tracker.app/data.db`）および同一サービス名と食い違う。Settings は実パスを出すのでユーザーは迷わない。サービス名の変更は新鍵=読めない DB になる。

**修正案:** spec のパスと Windows 例を実装に合わせる。Keychain サービス名はマイグレーション無しでリネームしない。

---

## Nits

### 24. `monthRange` 未使用、円記号が二系統

- 重大度: nit
- ファイル: `src/lib/utils/yearMonth.ts:5`
- 状態: open

`monthRange` は export されているが未使用。Dashboard/Accounts は `Intl.NumberFormat('ja-JP')`、Budgets は `formatCurrency` で、同じ整数が `¥` と `￥` に分かれうる。

**修正案:** `monthRange` を月フィルタで使うか削除する。表示は `formatCurrency` に寄せる。

### 25. Phase 2 コメントと `YearMonth` 二重定義

- 重大度: nit
- ファイル: `src-tauri/src/domain/ledger.rs:72`
- 状態: open

`validate_input` はまだ「Phase 2 では振替未対応」とコメント/エラーする。`YearMonth` が `domain/budget.rs` と `domain/report.rs` に二重定義。`balance.rs` は「CLAUDE.md rule 3」を引用し、`balance_repo` は同じ V002 説明を SQL 定数と関数の両方に繰り返す。AGENTS.md と CLAUDE.md はバイト一致。

**修正案:** フェーズ履歴コメントを消し、振替拒否はプロダクト文言にする。`YearMonth` は1型にする。

---

## レビュー方法

Rust バックエンド、Svelte フロント、仕様/CI/テストの3系統でコードを読み、重大指摘はソースで再確認した。Phase 5 未実装は bug にしない。仕様とコードの矛盾は AGENTS.md の同期ルールに従い suggestion として記録した。
