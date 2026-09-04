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
- **既存HTML機能の継承**: JSON エクスポート/インポート（上書き/追記、検証エラーでロールバック）は現行。Excel インポート/エクスポートは未実装で、**Phase 6b** に先送りする。スナップショットの `schema_version` は **backup format version 1** であり、`app_meta.schema_version` ではない。

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
  「先月分の家賃を後から登録する」が意図通り動く。意図しない大量生成は次の
  「保存前の確認」で防ぐ
- **月末クランプ**: `day_of_month` がその月に存在しない場合は末日に寄せる
  （31 → 2月は 28/29、4月は 30）。yearly の 2/29 も平年は 2/28。
  スキップも翌月繰り越しもしない
- **yearly の月**: `recurring_rules` に月カラムが無いため `starts_on` の月を使う
- **weekly の `day_of_week`**: 必須（`0` = 日曜）。NULL はコマンド層で拒否
- **ルールの削除**: §4.2 の論理削除方針に従い `active = 0`。行は消さない

#### 保存前の確認

`preview_recurring_occurrences(draft, limit, after)` は展開と同じ
`occurrences_between` を同じ引数で呼ぶ。`after` は**編集中のルールの
`last_generated_on`**（新規作成は NULL）。これを渡さないと、すでに生成し終えた過去
まで「今すぐ生成されます」に数え上げ、起きもしない backfill を報告することになる。
watermark は展開のたびに today まで進むので、健全なルールを編集したときの backfill は
0 件になるのが普通。

プレビューは「大量生成を防ぐ唯一の砦」なので、UI 側に 3 つの規則を課す:

1. **古い件数を残さない** — `type` / `frequency` / `day_of_month` / `day_of_week` /
   `starts_on` / `ends_on` のどれかが変わった瞬間、表示中のプレビューは消える。
   確認した件数が、変更後のフォームの隣に残ってはならない
2. **保存時に必ず数え直す** — 画面に何が出ていたか（出ていなかったか）に関係なく、
   保存する入力そのものから数え直す。フォームを切り取るのは保存の入口で 1 回だけで、
   数えるのも書くのもその同じ値。数え直しの往復中に入力が変わったら、書かずに
   中断してその旨を出す（黙って書けば承諾していない内容が保存され、黙って止めれば
   押しても何も起きない画面になる）
3. **backfill が 1 件以上なら明示的な承諾を取る** — 数え直した件数と最初 / 最後の
   発生日を見せ、ユーザーが承諾するまで 1 行も書かない。判定は `backfill_total > 0`
   のみで、開始日が今日かどうかではない（今日が発生日のルールは 1 件生成するので
   確認が出る。今日より後にしか発生しないルールだけが確認なしで保存できる）。
   承諾後にフォームが動けば、承諾は無効になり確認をやり直す

「最後の発生日」として見せてよいのは `backfill_last` だけ。`backfill` は `limit` で
切られるので、その末尾は「`limit` 件目」でしかなく、件数が多いほど生成範囲を短く
見せてしまう — 確認が一番効かなければならない暴走ケースで、いちばん外れる。

ルールの編集は「これから先の生成にだけ効く」。ただしこれは watermark が today まで
進んでいる健全なルールの話で、窓が空になるから成り立つ。参照先が壊れて展開を見送られ
続けたルールや、`import_json` で入ってきたルールは watermark が遅れているので、
編集を保存すると見送っていた期間が生成される（その件数は上の確認に出る）。

#### アーカイブ済み参照の扱い

ルールが参照する口座・カテゴリが後からアーカイブされた場合、**そのルールだけ展開を
見送り、`last_generated_on` も進めない**。展開結果に理由つきで返し、Recurring 画面が
警告バッジを出す。ユーザーが参照先を直せば、次回展開で見送った期間が遡って埋まる。
アーカイブされたカテゴリに取引が積み上がって予算進捗から消えるのを防ぐため。

検証には `domain/ledger.rs` の `assert_account_writable` / `assert_category_matches_tx`
を `AllowedArchivedRefs::none()` で再利用する。

#### バックアップ追記時の重複ルール

`recurring_rules` には UNIQUE 制約が無いので、自分のバックアップを追記
(`import_json` の `append`) すると同じスケジュールが 2 本並び、以後の展開が毎月
同じ取引を二重に書き続ける。そこで追記では挿入の前に同一ルールを探す。同一性の
キーは **name / type / amount / 口座 / counter_account / category_id / frequency /
day_of_month / day_of_week / starts_on**。口座と counter_account は id ではなく
行 (`name` / `kind` / `currency`) で照合する — 追記は口座を無条件に挿入するので、
id では自分自身のバックアップですら一致しない。

キーが一致した行には**挿入せず、キーに含まれない可変フィールドだけをマージする**:

| 列 | 解決 |
|---|---|
| `last_generated_on` | **新しい方を採る**（NULL = 未生成なので日付に負ける） |
| `ends_on` / `active` / `description` | **取り込み先（現行 DB）の値を残す** |

`last_generated_on` だけ相手の値を採りうるのは、これが watermark で、巻き戻すと
相手側がすでに生成した発生日をこちらでもう一度生成する = 金額が二重に入るから。
逆に `ends_on` / `active` / `description` は、追記が「今の正」に足す操作である以上、
普通はより古いスナップショットである取り込み元を採ると、ユーザーが自分で終了・
停止させたルールが復活してしまう。可変フィールドをキーに足さないのも同じ理由で、
キーが厳しすぎると「毎月二重課金する 2 本目の有効なルール」を作ってしまう
（緩すぎた場合に失うのは、警告に出る 1 行ぶんの復旧可能な差分にすぎない）。

マージした事実と watermark の移動は警告として返す
（`merged duplicate recurring rule: <名前> (last generated <旧> -> <新>)`）。
`ImportResult` の各件数は「新規に追加した行数」で、マージした行は含めない。
Settings 画面もこの件数を「新規追加」と明示して表示する。「重複した行は既存に
まとめた」という注記は**追記モードで、かつ実際に警告があるときだけ**出す。上書きは
取り込む前に全消しするので何ひとつまとめておらず、警告が無ければ指し示す内訳も無い。

#### コマンド

| コマンド | 役割 |
|---|---|
| `list_recurring_rules(include_inactive)` | 一覧。各行に次回発生日を同梱 |
| `create_recurring_rule` / `update_recurring_rule` | ルール CRUD |
| `set_recurring_rule_active(id, active)` | 有効 / 停止 |
| `preview_recurring_occurrences(draft, limit, after)` | 保存前の発生日プレビュー（生成しない）。`after` は編集中のルールの `last_generated_on` |
| `expand_due_recurring()` | 起動時展開 |

`expand_due_recurring()` の戻り値は生成総数・ルール別内訳・スキップ一覧
（理由つき）を含み、`tests/fixtures/responses/` の契約フィクスチャで固定する。

#### その他

- **「次回予定」表示**: 生成は行わず、UI で次回発生日をプレビューのみ
- **ルール変更時の挙動**: 過去生成済みの取引はそのまま、未来の生成のみ新ルールで
- **インデックス**: ルール別の生成履歴を引くため `transactions(recurring_id)` を追加する
  （`V005__recurring_transaction_index.sql`）。`recurring_rules` 自体は V001 で作成済み。
  起動時の冪等な展開 (`expand_due_recurring()`) は有効なルールを `last_generated_on`
  で絞って引くため、`recurring_rules(active, last_generated_on)` の複合インデックスも
  追加する（`V006__recurring_rule_lookup_index.sql`）

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

この2本は Dashboard 用として現状のまま残し、Phase 5b では変更しない。前月比などの
比較値を後付けすると既存の契約フィクスチャと Dashboard の表示契約を壊すため、
レポート用のコマンドを別に新設する。

#### タブ構成

```
タブ: [月次] [年次] [カテゴリ別] [トレンド]

[月次タブ]
- 月別収支グラフ (Chart.js bar、直近12ヶ月)
- 前月比 / 前年同月比カード
- 支出 Top5 / 収入 Top5 カテゴリ

[年次タブ]
- 月別の収入/支出スタックグラフ (12ヶ月固定)
- 年間サマリー (12ヶ月平均、最大支出月)

[カテゴリ別タブ]
- 円グラフ (収入の内訳 / 支出の内訳)
- カテゴリ毎の月別推移ライン (円グラフのクリックで対象を切替)

[トレンドタブ]
- 純資産推移ライン (非アーカイブ口座の合計の時系列)
- 月次収支 (net) と、その3ヶ月移動平均
```

#### 期間モデル

コマンドは `from_year_month` / `to_year_month`（`"YYYY-MM"`）の閉区間を受け取る汎用
レンジとする。UI はプリセット（直近6ヶ月 / 直近12ヶ月 / 直近24ヶ月 / 今年）で選ぶ。
年次タブだけは年セレクタを別に持つ。

commands 層のバリデーション:

- `YearMonth::parse_key` で書式と範囲を検証する
- `from <= to`
- 区間長は最大 60ヶ月（既存 `monthly_series` の上限に揃える）
- `month` は 1..=12

#### コマンド

| コマンド | 役割 |
|---|---|
| `report_monthly(year, month)` | 当月・前月・前年同月の収支、その差分、支出 / 収入それぞれの Top5 |
| `report_yearly(year)` | 12ヶ月分のバケット、年間合計、12ヶ月平均、最大支出月 |
| `report_by_category(from_year_month, to_year_month)` | 期間合計の内訳（収入 / 支出）と、カテゴリ毎の月別系列 |
| `report_net_worth_series(from_year_month, to_year_month)` | 月末純資産、月次 net、3ヶ月移動平均 |

戻り値の形:

```
MonthlyReport
  current / prev_month / prev_year : { income, expense, net }
  mom / yoy : { income_diff, expense_diff, net_diff, expense_percent: i64 | null }
  top_expense / top_income : CategoryAggregate[]        // 各最大5件

YearlyReport
  months: MonthlyBucket[12]                            // 歯抜けは 0 埋め
  total_income / total_expense / net
  avg_income / avg_expense                             // 12 で割る
  max_expense_month: "YYYY-MM" | null

CategoryReport
  months: string[]                                     // 軸ラベル
  income / expense : CategoryAggregate[]               // 期間合計・降順
  series: { category_id, name, type, points: i64[] }[] // points.len() == months.len()

NetWorthReport
  points: { year_month, net_worth, net, net_moving_avg: i64 | null }[]
```

- `expense_percent` は比較対象の支出が 0 のとき `null`。0除算をフロントに出さない
- 移動平均は独立した配列にせず point に同居させる。別配列だとラベルとの対応がずれうる。
  窓が埋まらない先頭2点は `null`
- 平均・パーセントは整数除算（規約1）。端数は 0 方向に切り捨てる
- 年間平均はデータのある月数ではなく常に 12 で割る
- 最大支出月が同額で並んだ場合は早い月を採る
- 月次タブの「月別収支グラフ」は既存の `monthly_series(12)` を使う。`report_monthly` は
  比較値と Top5 だけを担い、系列を二重に持たない
- `NetWorthReport` の `net` は振替を除いた月次収支（規約3）、`net_worth` の月間増分は
  振替を含む4方向の増減。アーカイブ口座との振替があった月は両者が一致しない。
  これは定義どおりの挙動であり、UI では別々の指標として並べる

#### 純資産推移の定義

対象は **`archived_at IS NULL` の口座のみ**。Dashboard の「総資産」(`total_assets`) と
同じ集合で、`to` が当月のとき系列の最終点が Dashboard の数字と一致することを不変条件と
し、integration テストで固定する。

集合の取り方には注意点がある。全口座を合算すれば振替は出金と入金で相殺されるが、
アーカイブ口座を除くと相殺が崩れる。したがって月次の増減は、
`balance_repo::LIST_BALANCES_SQL` と同じ4方向（income +、expense −、transfer 出 −、
transfer 入 +）を `WHERE a.archived_at IS NULL` 付きで月別に集計して求める。

- 開始残高 = 非アーカイブ口座の `initial_balance` 合計 + `from` より前の同じ増減の総和
- 各月末の純資産 = 開始残高にその月までの増減を累積したもの
- 残高のある口座をアーカイブすると、その分は過去も含めて系列全体から消える
  （Dashboard の総資産と同じ振る舞い）

「総収入 / 総支出」の集計は従来どおり `type IN ('income','expense')` で振替を除外する
（規約3）。純資産の増減計算だけが振替を見る。

#### カテゴリ別タブの絞り込み

推移ラインは初期表示で支出上位5カテゴリ。`expense` は Rust 側で降順に並べて返すので、
フロントは先頭5件を取るだけでよい（順位付けの計算はしない）。

収入 / 支出どちらの円グラフでも、セグメントまたは凡例をクリックするとそのカテゴリだけの
推移に切り替わる。同じカテゴリをもう一度クリックすると選択が解除され、初期表示の
支出上位5カテゴリに戻る。`report_by_category` が期間内に取引のある全カテゴリの系列を
一度に返すので、**切り替えで再 invoke はしない**。

#### 実装上の分担

- 集計は `domain/report.rs` の純粋関数と `infra/repo/report_repo.rs` の SQL に置く。
  Svelte 側で差分・平均・移動平均・上位抽出を計算しない（規約2）
- **インデックス**: レポート集計の主クエリは規約3どおり
  `WHERE type IN ('income','expense')` で振替を除外したうえで期間を絞り込むが、
  `idx_tx_budget_month_category(type, occurred_on, category_id)`（V003）が左端2列の
  prefix としてこのアクセスパスを既にカバーする（`EXPLAIN QUERY PLAN` で確認済み）。
  `transactions(type, occurred_on)` 単独の複合インデックスは書き込みコストを増やす
  だけで検索計画を変えないため、意図して追加しない
- Chart.js の生成 / 更新 / 破棄は `src/lib/components/Chart.svelte` の共通ラッパーに
  閉じる。Dashboard の月別収支グラフも同じラッパーに載せ替え、同じ定型を2箇所で持たない
- タブ状態は `Reports.svelte` の `$state` に持ち、URL には載せない
  （現行 router は `routePaths` の完全一致のみを扱う）
- **エクスポート**: JSON は現行。Excel は **Phase 6b**。PDF出力は MVP 対象外

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
- **コード署名**: **MVP は未署名配布**。Apple Developer ID も Windows Authenticode 証明書も保有していないため、`release.yml` に署名ステップを置かない。証明書が揃うまで「secrets があれば署名する」条件分岐も書かない — 一度も実行されないコードパスは検証できず、いざ証明書を入れたときに壊れている。署名を導入する際に §7 とワークフローを同時に更新する

## 7. ビルド・配信

| OS | 配布形式 | 備考 |
|---|---|---|
| macOS | `.dmg` (Universal Binary: x86_64 + aarch64) | Intel/Apple Silicon 両対応 |
| Windows | `.msi` + `.exe` (NSIS) | x86_64 のみ (MVP) |

- **アプリ識別子**: `jp.budget-tracker.app` (macOS bundle identifier / Windows AppID)
- **バンドル対象**: `tauri.conf.json` の `bundle.targets` は上表と一致する明示列挙 `["dmg", "msi", "nsis"]` とする。`"all"` にすると macOS で更新用アーカイブ `.app.tar.gz` まで draft release に並び、ユーザーがどれを落とせばよいか分からなくなる。配布形式を増減するときは上表と `bundle.targets` を同時に直す
- **自動アップデート**: MVP対象外。手動で新バージョン .dmg/.msi をダウンロード。`plugins.updater` は設定せず、`release.yml` では `uploadUpdaterJson: false` を明示して更新マニフェスト (`latest.json`) を release に置かない — 存在しない更新エンドポイントを広告しないため
- **データの可搬性**: ユーザーがアプリ間でデータを移すには、JSON エクスポート → 別端末でインポートする手順を公式手順とする

### 7.1 リリースワークフロー (Phase 6a)

タグ push (`v*`) を唯一のトリガーとする。`.github/workflows/release.yml`:

| 項目 | 決定 |
|---|---|
| ビルド実行 | `tauri-apps/tauri-action` に委ねる (`releaseDraft: true`, `tagName: v__VERSION__`) |
| マトリクス | `macos-latest` → `--target universal-apple-darwin` / `windows-latest` → `--target x86_64-pc-windows-msvc` |
| 成果物 | draft release に `.dmg` / `.msi` / `.exe` を添付。**publish は人間が手で行う** |
| 検査順 | `pnpm release:check` → `pnpm check` → `pnpm test` → `cargo clippy --locked` → `cargo test --locked` の全緑がビルドの前提 |
| Playwright | `release.yml` では走らせない。タグは `ci.yml` を通過済みのコミットに打つ。ブラウザ導入で数分伸びる割に、同じコミットで既に緑になっている |
| 規約 | `ci.yml` と同じ。**actions は tauri-action も含めて全て SHA ピン + バージョンコメント**、Node 22、Windows の NASM は Chocolatey 経由 (`ilammy/setup-nasm` は Node.js 20 ランタイムのまま止まっている) |

自前で `pnpm tauri build` + アセットアップロードを書かない理由: draft release の作成・複数ランナーからの同一 release への添付・`__VERSION__` の解決を公式アクションが持っており、手組みするとその3点が壊れやすい。

**版数の正本は `package.json`。** `scripts/check-version-sync.mjs` が `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` の3者一致を常に検査する。加えて **タグ文脈のときだけ** (`GITHUB_REF_NAME` が `v` 始まり)、タグ版数の一致と `CHANGELOG.md` に当該版の見出しがあることを検査する。CHANGELOG 検査を常時にしないのは、次版へ番号を上げてから内容を書き終えるまでの間、無関係な PR が全部赤くなるため。

判定は純粋関数 `findVersionMismatches({ pkg, cargo, tauri, tag, changelogVersions })` に切り出し Vitest で境界 (タグなし / タグ不一致 / Cargo だけズレ / CHANGELOG 見出し欠落) を押さえる — ファイル読み出しは薄い殻に留める。`pnpm release:check` として `ci.yml` と `release.yml` の両方から走らせる。

**未署名配布の受け渡し**: README にインストール手順として、macOS は初回のみ右クリック → 開く (または `xattr -d com.apple.quarantine <app>`)、Windows は SmartScreen の「詳細情報」→「実行」を明記する。リリースノートにも同じ注意を載せる。

## 8. テスト戦略

| レイヤー | ツール | 対象 |
|---|---|---|
| Rust unit | `cargo test` | domain/ の純粋関数 (予算評価、定期展開ロジック、レポート集計) |
| Rust 統合 | `cargo test` + `rusqlite` メモリDB | commands/ + DB マイグレーション |
| Svelte unit | Vitest + @testing-library/svelte | ストア、ユーティリティ、コンポーネント |
| E2E | Playwright（Vite `pnpm dev` + Tauri invoke mock）。Tauri WebView 上の E2E は **Phase 6c** | 主要ユーザーフローのスモーク (取引追加→予算反映 等) |

- **重点**: domain レイヤーの Rust ユニットテスト。金額計算と日付ロジックは間違えると致命的なので、property-based test (`proptest`) で境界をカバー
- **TDD で進める**: 各機能の Rust domain ロジックは「先にテスト → 実装」サイクル
- **CI**: PR 時に `pnpm release:check` `cargo clippy` `cargo test` `pnpm test` `svelte-check` と Playwright スモークをすべて緑にする。`release.yml` は Phase 6a

### 8.1 Tauri WebView E2E (Phase 6c)

**採用ルートは `@wdio/tauri-service` + `tauri-plugin-wdio` + `tauri-plugin-wdio-webdriver` (すべて MIT / Apache-2.0) の embedded driver。** WebDriver サーバーをアプリのプロセス内で動かすため外部ドライバが要らず、**macOS を含む 3 OS すべてで無料**。

他の2ルートを採らない理由: `tauri-driver` 単体は Windows/Linux のみ — Apple の `safaridriver` がアプリ組み込みの WKWebView を外部から操作できないため。その穴を埋める CrabNebula のフォークは macOS に有料 API キーを要求する。embedded ルートはこの穴自体を通らない。

**プラグインは Cargo feature `wdio` で切り離す (必須)。**

```toml
[features]
wdio = ["dep:tauri-plugin-wdio", "dep:tauri-plugin-wdio-webdriver"]

[dependencies]
tauri-plugin-wdio = { version = "1", optional = true }
tauri-plugin-wdio-webdriver = { version = "1", optional = true }
```

登録は `#[cfg(feature = "wdio")]` で囲む。**公式ドキュメントが先に挙げる `#[cfg(debug_assertions)]` は採らない** — それだと `pnpm tauri dev` のたびにポート 4445 で WebDriver サーバーが立つ。開発機には本物の家計データが入った暗号化 DB があり、そこへ常時待ち受けの遠隔操作口を開けることになる。feature flag なら `--features wdio` と明示したときにしか存在しない。`pnpm tauri build` の既定ビルドにはコード自体が入らない。

**6c は 6a のリリースゲートにはならない。** E2E が動かすのは `--features wdio` 付きでビルドしたバイナリで、ユーザーがダウンロードする `.dmg` の中身とは別物だから。6c が買うのは「実物の WebView (macOS = WKWebView / Windows = WebView2) で動かす」こと — CSP の実挙動、IPC、フォントとレイアウト差など Playwright の Chromium + invoke モックでは触れない層である。

**順序は 6a → 6c → 6b。** 6c を 6b の前に置くのは、Excel (6b) の UI 実装を最初から実 WebView の網の下で進めるため。Playwright は引き続きスモークとして残し、6c で置き換えない (ブラウザ側は速く、モックが効く)。

## 9. UI/UX 方針

- **テーマ**: 既存HTMLの華やかなグラデーション (`#667eea → #764ba2`, `#4facfe → #00f2fe`) と丸いカードを継承・強化
- **ダークモード**: MVP対象外（将来拡張）
- **レイアウト**: サイドバー(ナビゲーション) + メインエリア。デスクトップアプリ前提でモバイル幅は最適化しない
- **アイコン**: 絵文字 + シンプルなSVG。重い アイコンフォントは導入しない
- **アクセシビリティ**: 既存HTMLで踏襲されている aria-label / focus 管理を継承

## 10. 実装フェーズ分割

設計は1スペックにまとめるが、実装計画は writing-plans スキルにてフェーズごとに分割する：

| Phase | 内容 | 完了基準 |
|---|---|---|
| 1 | スキャフォールド + SQLCipher/Keychain 基盤 | アプリ起動、DB初期化、鍵保存・取得が動く |
| 2 | 取引 CRUD + カテゴリ + ダッシュボード | 既存HTMLの主要機能が Tauri アプリ上で再現 |
| 3 | 複数口座 + 振替 | 口座追加、振替取引、口座別残高表示 |
| 4 | 予算管理 | 予算設定、進捗バー、超過アラート |
| 5a | 定期取引 | 起動時自動展開、Recurring ルート |
| 5b | 分析レポート強化 | レポート4タブ |
| 6a | リリース基盤 (§7.1) | **v0.1.0 を実際に公開する** |
| 6c | Tauri WebView E2E (§8.1) | `--features wdio` で 3 OS のうち最低 macOS のスモークが緑 |
| 6b | Excel エクスポート / インポート | 未着手。着手前に §5.2 を確定させる |

**6c を 6b より先に置く。** 理由は §8.1。6a より後なのは、6c が 6a の出荷バイナリを検証できず (別ビルドになる) リリースゲートとして働かないため — 6a を待たせる意味がない。

各フェーズの完了時に動作確認 → 次フェーズへ進む。CI緑化と E2E テスト最低1本を各フェーズの完了条件とする。

**Phase 6a の完了基準**（機構が揃っただけでは完了としない。未検証のリリース手順は手順書ではない）:

1. `pnpm release:check` を `ci.yml` に組み込んだうえで CI が緑
2. `v0.1.0` タグ push で Release ワークフローが macOS / Windows 両方成功
3. draft release に `.dmg` と `.msi`（NSIS 出力があれば `.exe`）が並ぶ
4. macOS 実機で `.dmg` をダウンロードし、README の未署名回避手順どおりに起動 → DB 初期化と取引追加が通る
5. `CHANGELOG.md` からリリースノートを転記して publish

Windows 実機がない場合、4 は CI のビルド成功をもって代替し、Windows 側が実機未検証であることをリリースノートに明記する。

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
- 2026-09-03: §5.4 にバックアップ追記時の定期取引ルールの同一性キーとマージ規則 (watermark は新しい方、`ends_on`/`active`/`description` は取り込み先) を追記
- 2026-09-03: §5.6 を Phase 5b 実装向けに確定。レンジ型の期間モデル、4コマンドの戻り値、純資産推移を非アーカイブ口座のみで定義 (振替相殺が崩れるため月次増減は 4 方向集計)、移動平均は月次 net に対してかけ point に同居、カテゴリ推移は円グラフのクリックで切替 (再 invoke なし)、Chart.js は共通ラッパーに集約
- 2026-09-04: Phase 6 を 6a (リリース基盤) / 6b (Excel) / 6c (Tauri WebView E2E) に分割。§6.2 のコード署名を「MVP は未署名配布、条件分岐も書かない」に確定し、§7.1 にリリースワークフロー (tauri-action + 版数同期検査 + 未署名回避手順) を新設。§8 の `tauri-driver` 記述を `@wdio/tauri-service` の embedded driver (3 OS 対応) に更新し 6c へ移送。§10 に 6a の完了基準を「v0.1.0 を実際に公開する」として明記
- 2026-09-04: §8.1 を新設し 6c の順序を 6b の前へ (6a → 6c → 6b)。WebdriverIO プラグインは Cargo feature `wdio` で切り離すことを必須と決定 (`#[cfg(debug_assertions)]` は `pnpm tauri dev` のたびに WebDriver サーバーが立つため不採用)。6c は出荷バイナリと別ビルドを検証するのでリリースゲートにはしない
