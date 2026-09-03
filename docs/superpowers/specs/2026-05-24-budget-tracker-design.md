# 家計簿アプリ デスクトップ版 設計書

- 作成日: 2026-05-24
- ステータス: ドラフト（ユーザーレビュー待ち）
- 出発点: `家計簿.html` (単一HTMLファイルの既存実装)
- 目標: macOS / Windows 両対応の「本格的」なデスクトップ家計簿アプリ

## 1. 目的とゴール

既存の単一HTMLファイル実装をベースに、以下を実現するデスクトップアプリを構築する。

- **本格的な家計管理**: 予算管理・複数口座・定期取引・分析レポートを揃える
- **クロスプラットフォーム**: macOS (Intel + Apple Silicon) と Windows (x86_64)
- **データの安全性**: お金のデータを OS の安全機構で保護する
- **長期保守性**: 機能追加に耐える分離された構造

### 非ゴール（MVPに含めない）

- クラウド同期・マルチデバイスリアルタイム同期
- 銀行APIからの自動取込
- モバイルアプリ
- マルチユーザー（1端末1ユーザー前提）
- PDFレポート出力（既存のExcelエクスポートとブラウザ印刷で代替）
- OS通知でのアラート（UI内バッジで代替）
- 自動アップデート（手動ダウンロードで代替）

## 2. 技術スタック

| レイヤー | 採用技術 | 理由 |
|---|---|---|
| デスクトップシェル | Tauri 2.x | 軽量(~10MB)、Rust製コアでセキュリティ機能を扱いやすい、macOS/Windows両対応 |
| フロントエンド | Svelte 5 + TypeScript + Vite | Runes(シグナルベース)で状態管理が単純、軽量、Tauri公式テンプレートあり |
| バックエンドコア | Rust | ビジネスロジック・集計・暗号化処理を集約 |
| データベース | SQLite + SQLCipher (AES-256) | リレーショナルクエリで複数口座・予算・定期を効率処理、DB全体暗号化 |
| 鍵管理 | OS Keychain (`keyring` crate) | macOS Keychain / Windows Credential Manager 連携 |
| グラフ描画 | Chart.js | 既存実装からの知見継承 |

### 主要な設計上の選択

- **Tauri 2.x** (1.x ではなく): プラグインAPI整備、`tauri-plugin-sql`等のエコシステム、将来の拡張余地
- **Rust側にコアロジックを集約**: クエリ・集計・予算計算・定期取引展開・レポート生成はRust側。Svelteは UI・イベント・表示のみ。テスト容易性とパフォーマンス確保
- **金額は整数 (INTEGER) で保持**: 円は最小単位が整数なので浮動小数誤差を完全排除
- **データ移行は不要**: 既存HTML版のデータ移行は対象外（新規スタート）

## 3. アーキテクチャ

### 3.1 全体構造

```
┌────────────────────────────────────────────┐
│  Svelte 5 + TS UI (WebView 内)             │
│  - ルーティング: ダッシュボード / 取引 / 予算  │
│                / 口座 / 定期 / レポート / 設定 │
│  - 状態: Svelte 5 Runes ($state, $derived)  │
│  - グラフ: Chart.js                          │
└────────────────┬───────────────────────────┘
                 │ Tauri invoke (型安全コマンド)
                 │ + イベント(変更通知 broadcast)
┌────────────────▼───────────────────────────┐
│  Rust コア (src-tauri/src/)                 │
│  - commands/ : Tauriコマンド層 (薄い)        │
│  - domain/   : ビジネスロジック             │
│      ├ budget   (予算評価エンジン)           │
│      ├ recurring(定期取引展開)               │
│      ├ report   (集計・トレンド計算)         │
│      └ ledger   (取引・残高計算)             │
│  - infra/    : SQLite + Keychain アダプタ    │
└────────────────┬───────────────────────────┘
                 │
┌────────────────▼───────────────────────────┐
│  SQLite (SQLCipher で暗号化)                │
│  保存先: Tauri app_data_dir()/data.db       │
│  (identifier: jp.budget-tracker.app)        │
│  例: .../jp.budget-tracker.app/data.db      │
│  暗号化キーは OS Keychain に保存             │
│  (全 OS: jp.budget-tracker / db_key)        │
└────────────────────────────────────────────┘
```

### 3.2 通信パターン

- **Read操作**: フロント → `invoke('list_transactions', filter)` → Rust → SQLite → 結果返却
- **Write操作**: フロント → `invoke('add_transaction', payload)` → Rust が DB 書き込み → `emit('data:changed', { domain: 'transactions' })` → フロントのストアが受信して再フェッチ
- **イベント駆動の再描画**で UI 状態と DB が乖離しないことを保証する

### 3.3 ディレクトリ構造

```
BudgetTracker/
├── src/                          # Svelte フロントエンド
│   ├── routes/                   # ページコンポーネント
│   │   ├── Dashboard.svelte
│   │   ├── Transactions.svelte
│   │   ├── Budgets.svelte
│   │   ├── Accounts.svelte
│   │   ├── Recurring.svelte
│   │   ├── Reports.svelte
│   │   └── Settings.svelte
│   ├── lib/
│   │   ├── api/                  # Tauri invoke のラッパー (型付き)
│   │   ├── stores/               # Svelte 5 Runes 状態
│   │   ├── components/           # 共通UI (Card, Modal, Button…)
│   │   └── utils/                # formatCurrency 等
│   ├── App.svelte
│   └── main.ts
├── src-tauri/
│   ├── src/
│   │   ├── commands/             # Tauri コマンド層 (薄い)
│   │   │   ├── accounts.rs
│   │   │   ├── transactions.rs
│   │   │   ├── categories.rs
│   │   │   ├── budgets.rs
│   │   │   ├── recurring.rs
│   │   │   └── reports.rs
│   │   ├── domain/               # ビジネスロジック
│   │   │   ├── budget.rs
│   │   │   ├── recurring.rs
│   │   │   ├── report.rs
│   │   │   └── ledger.rs
│   │   ├── infra/
│   │   │   ├── db.rs             # SQLite + SQLCipher 接続
│   │   │   ├── keychain.rs       # keyring ラッパー
│   │   │   └── migrations/       # マイグレーションSQL
│   │   ├── error.rs              # thiserror で型安全エラー
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── migrations/               # SQL ファイル (V1__init.sql 等)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/superpowers/specs/       # 設計書
├── tests/                        # E2E/統合テスト
├── .github/workflows/
│   ├── build.yml                 # CI: lint + test
│   └── release.yml               # タグ時の配布ビルド
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 4. データモデル (SQLite スキーマ)

### 4.1 テーブル定義

```sql
-- =========================================================
-- accounts: 口座（現金・銀行・カード・電子マネー等）
-- =========================================================
CREATE TABLE accounts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  name            TEXT NOT NULL,                   -- ユーザー自由入力 (例: "三井住友銀行")
  kind            TEXT NOT NULL,                   -- cash | bank | credit_card | e_money | investment
  currency        TEXT NOT NULL DEFAULT 'JPY',
  initial_balance INTEGER NOT NULL DEFAULT 0,      -- 開始残高（整数で保持）
  display_order   INTEGER NOT NULL DEFAULT 0,
  note            TEXT NOT NULL DEFAULT '',        -- メモ (任意)
  archived_at     TEXT,                            -- アーカイブ時刻 (ISO 8601)
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- =========================================================
-- categories: カテゴリ（収入/支出）
-- =========================================================
CREATE TABLE categories (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT NOT NULL,
  type          TEXT NOT NULL CHECK(type IN ('income','expense')),
  color         TEXT,                            -- 円グラフ用 #RRGGBB
  icon          TEXT,                            -- 絵文字 or アイコン名
  display_order INTEGER NOT NULL DEFAULT 0,
  archived_at   TEXT,
  UNIQUE(name, type)
);

-- =========================================================
-- transactions: 取引（収入・支出・振替）
-- =========================================================
CREATE TABLE transactions (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  occurred_on        TEXT NOT NULL,                   -- 'YYYY-MM-DD'
  type               TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),     -- transfer の振替先
  category_id        INTEGER REFERENCES categories(id),   -- transfer は NULL
  description        TEXT NOT NULL DEFAULT '',
  recurring_id       INTEGER REFERENCES recurring_rules(id) ON DELETE SET NULL,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL
);
CREATE INDEX idx_tx_occurred_on ON transactions(occurred_on);
CREATE INDEX idx_tx_account     ON transactions(account_id);
CREATE INDEX idx_tx_category    ON transactions(category_id);

-- =========================================================
-- budgets: 予算（カテゴリ別 × 月次/年次）
-- =========================================================
CREATE TABLE budgets (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  category_id     INTEGER NOT NULL REFERENCES categories(id),
  period          TEXT NOT NULL CHECK(period IN ('monthly','yearly')),
  amount          INTEGER NOT NULL CHECK(amount >= 0),
  starts_on       TEXT NOT NULL,                   -- 適用開始月 'YYYY-MM-01'
  ends_on         TEXT,                            -- NULL なら継続
  alert_threshold INTEGER NOT NULL DEFAULT 80,     -- 警告閾値 %
  UNIQUE(category_id, starts_on)
);

-- =========================================================
-- recurring_rules: 定期取引ルール
-- =========================================================
CREATE TABLE recurring_rules (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  name               TEXT NOT NULL,                   -- 例: "家賃"
  type               TEXT NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),
  category_id        INTEGER REFERENCES categories(id),
  description        TEXT NOT NULL DEFAULT '',
  frequency          TEXT NOT NULL,                   -- monthly | weekly | yearly
  day_of_month       INTEGER,                         -- 月次の発生日 (1-31)
  day_of_week        INTEGER,                         -- 週次の発生曜日 (0-6)
  starts_on          TEXT NOT NULL,
  ends_on            TEXT,
  last_generated_on  TEXT,                            -- 最終生成日（冪等性確保）
  active             INTEGER NOT NULL DEFAULT 1
);

-- =========================================================
-- app_meta: スキーマバージョン・アプリ設定
-- =========================================================
CREATE TABLE app_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- 保存する key の例: schema_version, theme, locale, last_backup_at
```

### 4.2 設計上の重要決定

- **金額は整数 (INTEGER)**: 円は整数のため浮動小数誤差を完全排除。外貨対応は将来 `currency_subunit` カラムを追加して拡張
- **振替 (transfer) を取引種別に追加**: 「銀行→現金」のような口座間移動を支出/収入と分離。分析時の二重計上を防ぐ
- **`recurring_id` を transactions に持たせる**: 自動生成された取引が「どのルールから生まれたか」を追跡可能。ルール削除時は `SET NULL` で履歴を保持
- **`archived_at` で論理削除**: 過去取引を持つカテゴリ・口座を物理削除すると履歴が壊れるため archive 方式
- **`last_generated_on`** で定期取引の生成を冪等化: アプリ起動時に「未生成期間分」だけバッチ展開しても二重生成されない
- **スキーママイグレーション**: `app_meta.schema_version` を見て差分適用。マイグレーションファイルは `src-tauri/migrations/V001__init.sql` 形式でバージョン管理

### 4.3 ユーザー定義性

- **銀行名・カード会社名は一切ハードコードしない**。`accounts.name` は完全に自由入力
- `accounts.kind` のみ固定リスト (cash/bank/credit_card/e_money/investment)。集計時にカードを「負債」、投資を「資産」として扱うため必要
- **カテゴリも自由入力**。初回起動時のデフォルトセット（食費・交通費・給与等）は便利のためであり、ユーザーは自由に追加・削除・名前変更可能

## 5. 機能仕様

### 5.1 ダッシュボード

- **総資産カード**: `initial_balance + Σ(収入) - Σ(支出) + Σ(振替IN) - Σ(振替OUT)` を口座ごとに集計
- **当月収支カード**: 既存HTML と同等の「総収入・総支出・残高」
- **予算進捗ウィジェット**: 当月予算の上位3件
- **直近取引リスト**: 最新10件
- **月別収支グラフ**: 既存HTMLと同等のChart.js棒グラフ

### 5.2 取引管理

- **CRUD**: 追加・編集・削除・フィルタ（種類/カテゴリ/口座/月/フリーワード）
- **振替の扱い**: type='transfer' のとき category は NULL、counter_account_id を必須化
- **既存HTML機能の継承**: JSON エクスポート/インポート（上書き/追記、検証エラーでロールバック）は現行。Excel インポート/エクスポートは Phase 4 では未実装で、Phase 6 以降に先送りする。スナップショットの `schema_version` は **backup format version 1** であり、`app_meta.schema_version` ではない。

### 5.3 予算管理

```
[ 予算ページ ]
┌─────────────────────────────────────────┐
│ 2026年 5月 ◀ ▶                           │
├─────────────────────────────────────────┤
│ 食費        ¥45,200 / ¥50,000           │
│ ████████████████████░░  90% ⚠           │
│ 残り ¥4,800 ・ あと 7日                  │
├─────────────────────────────────────────┤
│ 交通費      ¥8,500 / ¥10,000            │
│ ████████████░░░░░░░  85%                │
└─────────────────────────────────────────┘
```

- **予算評価エンジン** (`domain/budget.rs`)
  - 入力: 対象月 (YYYY-MM)
  - 出力: `Vec<BudgetStatus { category, budgeted, spent, percent, days_left, projected }>`
  - `projected` = 現在の日割りペースを月末まで延長した推定額（早期警告用）
- **アラート**: `percent >= alert_threshold` で UI 上にバッジ表示
- **適用期間**: `starts_on`/`ends_on` で予算改定の履歴管理が可能
- Phase 4 の月別 UI はカテゴリ×月の1行（`starts_on = YYYY-MM-01`, `ends_on` 未使用）。lookup は `period = 'monthly' AND starts_on = 選択月の1日`。`ends_on` NULL を翌月へ継続するルールは未実装（将来）。

### 5.4 定期取引

定期取引の展開と Recurring ルートは **Phase 5a**。

#### 展開の実行位置

`boot()` の完了後、フロントエンドから `expand_due_recurring()` を明示的に invoke する。
`setup()` 内では実行しない。理由:

- 生成件数とスキップ理由を UI に返せる（`setup()` からは UI に伝える経路がない）
- 展開の失敗が起動そのものを止めない
- Recovery 状態では自然に呼ばれない
- コマンド層のテストから直接叩ける

処理の流れ:

1. `recurring_rules WHERE active = 1` を取得
2. 各ルールについて `(last_generated_on, 今日]` に発生すべき日付を計算
3. 該当日付ぶんの transactions を一括 INSERT（`recurring_id` を紐づけ）
4. `last_generated_on` を `max(last_generated_on, 今日)` に更新

全ルールを単一トランザクションで処理する。

`last_generated_on` は「最後に生成した日」ではなく **「どこまで調べ終えたか」**
（watermark）。生成が 0 件だったルールも今日まで進める。理由:

- 発生日で止めると watermark が時計から遅れ続ける。その状態でユーザーが発生日を
  変えると、次の窓が「もう締めた期間」まで遡り、過去日の取引が生える
  （3/27 で止めたまま 4/15 に「毎月 1 日」へ付け替えると 4/1 が生成される）。
  **ルールの編集は、これから先の生成にだけ効く**
- 進めても取りこぼさない。窓 `(last_generated_on, today]` が返す日付は必ず
  今日以下で、その全件をこの回で生成済みだから
- `max` を取るのは、`import_json` 経由で今日より先の watermark を持つ行が
  入りうるため。巻き戻すと同じ日を二重生成する

ただし次節の「展開を見送ったルール」だけは例外で、watermark を進めない。

#### 日付生成の規則

日付列挙は `domain/recurring.rs` の純粋関数に閉じ込める（時計も DB も参照しない）。

- **窓は左開右閉** `(last_generated_on, today]`。これが冪等性の本体で、
  `occurrences(a, c) == occurrences(a, b) ++ occurrences(b, c)` が成り立つ
- **`last_generated_on` が NULL** のときは `starts_on` から遡って全件生成する。
  「先月分の家賃を後から登録する」が意図通り動く。意図しない大量生成は、
  作成フォームが `preview_recurring_occurrences` で件数を事前表示して防ぐ
- **月末クランプ**: `day_of_month` がその月に存在しない場合は末日に寄せる
  （31 → 2月は 28/29、4月は 30）。yearly の 2/29 も平年は 2/28。
  スキップも翌月繰り越しもしない
- **yearly の月**: `recurring_rules` に月カラムが無いため `starts_on` の月を使う
- **weekly の `day_of_week`**: 必須（`0` = 日曜）。NULL はコマンド層で拒否
- **ルールの削除**: §4.2 の論理削除方針に従い `active = 0`。行は消さない

#### アーカイブ済み参照の扱い

ルールが参照する口座・カテゴリが後からアーカイブされた場合、**そのルールだけ展開を
見送り、`last_generated_on` も進めない**。展開結果に理由つきで返し、Recurring 画面が
警告バッジを出す。ユーザーが参照先を直せば、次回展開で見送った期間が遡って埋まる。
アーカイブされたカテゴリに取引が積み上がって予算進捗から消えるのを防ぐため。

検証には `domain/ledger.rs` の `assert_account_writable` / `assert_category_matches_tx`
を `AllowedArchivedRefs::none()` で再利用する。

#### コマンド

| コマンド | 役割 |
|---|---|
| `list_recurring_rules(include_inactive)` | 一覧。各行に次回発生日を同梱 |
| `create_recurring_rule` / `update_recurring_rule` | ルール CRUD |
| `set_recurring_rule_active(id, active)` | 有効 / 停止 |
| `preview_recurring_occurrences(draft, limit)` | 保存前の発生日プレビュー（生成しない） |
| `expand_due_recurring()` | 起動時展開 |

`expand_due_recurring()` の戻り値は生成総数・ルール別内訳・スキップ一覧
（理由つき）を含み、`tests/fixtures/responses/` の契約フィクスチャで固定する。

#### その他

- **「次回予定」表示**: 生成は行わず、UI で次回発生日をプレビューのみ
- **ルール変更時の挙動**: 過去生成済みの取引はそのまま、未来の生成のみ新ルールで
- **インデックス**: ルール別の生成履歴を引くため `transactions(recurring_id)` を追加する
  （`V005__recurring_transaction_index.sql`）。`recurring_rules` 自体は V001 で作成済み

### 5.5 口座・資産管理

```
[ ダッシュボード上部 ]
┌─────────────────────────────────────────┐
│ 総資産  ¥1,485,200                       │
│   現金       ¥45,200                    │
│   三井住友銀行 ¥850,000                  │
│   楽天カード  -¥35,000 (今月利用分)      │
│   PayPay      ¥625,000                  │
└─────────────────────────────────────────┘
```

- **口座残高計算**: Rust の SQL（CTE活用）で1クエリ。フロントは GET するだけ
- **振替の二重計上防止**: 「総支出/総収入」集計時に type='transfer' を除外
- **口座追加フォーム**:
  - 名前 (必須・自由入力): ユーザーが「三井住友銀行」「楽天カード」等を自由に命名
  - 種別 (必須・選択): bank/cash/credit_card/e_money/investment
  - 開始残高 (任意)
  - メモ (任意)
- **kind ごとのデフォルト絵文字**: 🏦/💵/💳/📱/📈（ユーザーが上書き可能）

### 5.6 分析レポート

4タブ UI（月次 / 年次 / カテゴリ別 / トレンド）と Reports ルートは **Phase 5b**（Recurring ルートは §5.4 の Phase 5a）。

Phase 4 時点で実装済みの集計コマンド:

- `monthly_summary(year, month)` — 指定月の収入 / 支出 / 差額（振替は除外）
- `monthly_series(months)` — 当月を含む直近 N ヶ月（歯抜け月は 0 埋め）

Phase 5b で追加する4タブ:

```
タブ: [月次] [年次] [カテゴリ別] [トレンド]

[月次タブ]
- 月別収支グラフ (既存と同等、Chart.js bar)
- 前月比 / 前年同月比カード
- 収支ランキング Top5 カテゴリ

[年次タブ]
- 月別の収入/支出スタックグラフ
- 年間サマリー (12ヶ月平均、最大支出月)

[カテゴリ別タブ]
- 円グラフ (収入の内訳、支出の内訳)
- カテゴリ毎の月別推移ライングラフ

[トレンドタブ]
- 純資産推移ライン (口座合計の時系列)
- 移動平均 (3ヶ月)
```

- **集計クエリは Rust 側**: 現行は `monthly_summary` / `monthly_series`。Phase 5b で `report_yearly(year)` / `report_by_category(range)` / `report_net_worth_series(range)` を追加する
- **エクスポート**: JSON は現行。Excel は Phase 6 以降。PDF出力は MVP 対象外

## 6. セキュリティ

### 6.1 鍵管理フロー

```
初回起動
  └─→ OS の安全な乱数生成器で 32バイト鍵生成
       └─→ keyring crate で OS Keychain に保存
            ├ macOS:  Keychain (jp.budget-tracker / db_key)
            └ Windows: Credential Manager (jp.budget-tracker / db_key)
       └─→ SQLCipher で data.db を AES-256 で初期化

2回目以降
  └─→ keyring から鍵取得 → SQLite ATTACH 時に PRAGMA key で復号
```

保存先は Tauri の `app_data_dir()/data.db`（`identifier` = `jp.budget-tracker.app` により `.../jp.budget-tracker.app/data.db`）。Keychain サービス名は既存鍵を孤児化するためリネームしない。

### 6.2 セキュリティ実装

- **使用 crate**: `keyring` (OS連携) + `rusqlite` の `bundled-sqlcipher` feature
- **鍵紛失時**: 復号不能 → 「JSON エクスポートから復元してください」とガイド表示
- **CSP 設定** (`tauri.conf.json`): `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'` で XSS 攻撃面を最小化
- **Tauri allowlist**: `shell.open` `path.*` `fs.*` 等は使う部分のみ許可
- **コード署名**: macOS は Developer ID + Apple Notarization、Windows は Authenticode で配布署名。署名証明書がない場合は未署名ビルドで配布（起動時警告は出るが動作する旨を README に明記）

## 7. ビルド・配信

| OS | 配布形式 | 備考 |
|---|---|---|
| macOS | `.dmg` (Universal Binary: x86_64 + aarch64) | Intel/Apple Silicon 両対応 |
| Windows | `.msi` + `.exe` (NSIS) | x86_64 のみ (MVP) |

- **アプリ識別子**: `jp.budget-tracker.app` (macOS bundle identifier / Windows AppID)
- **GitHub Actions ワークフロー**: タグ push (`v*`) で macOS Runner と Windows Runner で並列ビルド → GitHub Release に成果物添付
- **自動アップデート**: MVP対象外。手動で新バージョン .dmg/.msi をダウンロード
- **データの可搬性**: ユーザーがアプリ間でデータを移すには、JSON エクスポート → 別端末でインポートする手順を公式手順とする

## 8. テスト戦略

| レイヤー | ツール | 対象 |
|---|---|---|
| Rust unit | `cargo test` | domain/ の純粋関数 (予算評価、定期展開ロジック、レポート集計) |
| Rust 統合 | `cargo test` + `rusqlite` メモリDB | commands/ + DB マイグレーション |
| Svelte unit | Vitest + @testing-library/svelte | ストア、ユーティリティ、コンポーネント |
| E2E | Playwright（Vite `pnpm dev` + Tauri invoke mock）。`tauri-driver` による Tauri WebView E2E は Phase 6 | 主要ユーザーフローのスモーク (取引追加→予算反映 等) |

- **重点**: domain レイヤーの Rust ユニットテスト。金額計算と日付ロジックは間違えると致命的なので、property-based test (`proptest`) で境界をカバー
- **TDD で進める**: 各機能の Rust domain ロジックは「先にテスト → 実装」サイクル
- **CI**: PR 時に `cargo clippy` `cargo test` `pnpm test` `svelte-check` と Playwright スモークをすべて緑にする。`release.yml` は Phase 6

## 9. UI/UX 方針

- **テーマ**: 既存HTMLの華やかなグラデーション (`#667eea → #764ba2`, `#4facfe → #00f2fe`) と丸いカードを継承・強化
- **ダークモード**: MVP対象外（将来拡張）
- **レイアウト**: サイドバー(ナビゲーション) + メインエリア。デスクトップアプリ前提でモバイル幅は最適化しない
- **アイコン**: 絵文字 + シンプルなSVG。重い アイコンフォントは導入しない
- **アクセシビリティ**: 既存HTMLで踏襲されている aria-label / focus 管理を継承

## 10. 実装フェーズ分割

設計は1スペックにまとめるが、実装計画は writing-plans スキルにて以下5フェーズに分割する想定：

| Phase | 内容 | 完了基準 |
|---|---|---|
| 1 | スキャフォールド + SQLCipher/Keychain 基盤 | アプリ起動、DB初期化、鍵保存・取得が動く |
| 2 | 取引 CRUD + カテゴリ + ダッシュボード | 既存HTMLの主要機能が Tauri アプリ上で再現 |
| 3 | 複数口座 + 振替 | 口座追加、振替取引、口座別残高表示 |
| 4 | 予算管理 | 予算設定、進捗バー、超過アラート |
| 5a | 定期取引 | 起動時自動展開、Recurring ルート |
| 5b | 分析レポート強化 | レポート4タブ |

各フェーズの完了時に動作確認 → 次フェーズへ進む。CI緑化と E2E テスト最低1本を各フェーズの完了条件とする。

## 11. 既知のリスクと対応

| リスク | 影響 | 対応 |
|---|---|---|
| SQLCipher のビルド失敗 (Windows特に) | ビルド不可 | `bundled-sqlcipher-vendored-openssl` feature を使う。平文 SQLite へのフォールバックはしない（接続時は常に `PRAGMA key`） |
| 復号失敗・鍵欠落/破損 | data.db が開けない | Recovery UI を表示。JSON インポートまたは空データベース開始で復元する。元の `data.db` は新しいファイルのインポート成功後にのみ quarantine する (journal/wal/shm も同時に退避)。鍵なし/平文でのオープンはしない。デコードできない鍵エントリは削除前に `db_key.corrupt-<UTC日時>` へ生バイトのまま複製する。接続時は `PRAGMA cipher_compatibility = 4` で SQLCipher の形式を固定し、形式更新は `PRAGMA cipher_migrate` を経る明示的な作業とする |
| keyring が一部Linux環境で不安定 | 鍵取得失敗 | Linux は MVP対象外。macOS/Windowsの公式サポートのみとする |
| 大量データ時の集計性能 | UI ラグ | Rust 側で SQL に集計させる方針なので一般用途では問題なし。10万件超でベンチ取って index 見直し |
| Chart.js の WebView 描画パフォーマンス | グラフ重い | データポイントを年単位で 1000 以下に保つ (集計済みデータのみ渡す) |
| 暗号鍵紛失 | データ完全消失 | アプリ内で「JSON で必ずバックアップを取って」と促す。Settings に最終バックアップ日時を表示 |

## 12. オープン項目

なし（すべての主要決定はユーザー合意済み）。

---

## 改訂履歴

- 2026-05-24: 初版作成（ユーザーとのブレインストーミングセッションを経て確定）
- 2026-08-29: Phase 4 レビューに合わせ、未実装機能とパス/鍵の現行実装を明記
- 2026-08-29: §11 から SQLCipher 平文フォールバックを削除し、復号/鍵不一致時の Recovery 手順を明記
- 2026-09-03: §11 に SQLCipher 形式固定 (`cipher_compatibility = 4`)、破損鍵の退避、quarantine 時の journal 同時退避を追記
- 2026-09-03: Phase 5 を 5a (定期取引) / 5b (レポート強化) に分割し、§5.4 に展開の実行位置・日付生成規則・アーカイブ参照時の扱い・コマンド一覧を確定
