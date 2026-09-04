# BudgetTracker

macOS / Windows 両対応の家計簿デスクトップアプリ。Tauri + Svelte 5 + Rust + SQLite (SQLCipher) で構築する。

**設計の正本**: [docs/superpowers/specs/2026-05-24-budget-tracker-design.md](docs/superpowers/specs/2026-05-24-budget-tracker-design.md)

実装やリファクタを始める前に必ずこの設計書を読むこと。本ファイルは「設計書のうち、特に逸脱しやすい規約」を抜粋したもの。

## 現在の状態

- Phase 6a 完了 (リリース基盤: `pnpm release:check` + `release.yml` + `docs/release-checklist.md`)
- **v0.1.0 を未署名で公開済み** (macOS `.dmg` / Windows `.msi` + `.exe`)。macOS は実機で起動確認済み、Windows は CI ビルド成功のみ
- 既存の `家計簿.html` は参考用のプロトタイプ。**移植せず新規実装する**
- 次は spec の Phase 6c (Tauri WebView 上の E2E)。Excel は 6b

## 技術スタック

| 領域 | 採用 |
|---|---|
| デスクトップシェル | Tauri 2.x |
| フロント | Svelte 5 (Runes) + TypeScript + Vite |
| バックエンド | Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`) |
| 鍵管理 | `keyring` crate (macOS Keychain / Windows Credential Manager) |
| グラフ | Chart.js |
| テスト | `cargo test` / `proptest` / Vitest / Playwright |

## ディレクトリ規約

```
src/                        Svelte フロントエンド (UI のみ)
  routes/                   ページコンポーネント
  lib/api/                  Tauri invoke の型付きラッパー
  lib/stores/               Svelte 5 Runes 状態
  lib/components/           共通 UI

src-tauri/src/
  commands/                 Tauri コマンド層 (薄い: 引数バリデーションと domain 呼び出しのみ)
  domain/                   ビジネスロジック (純粋関数中心、ここに集中させる)
    ├ budget.rs             予算評価
    ├ recurring.rs          定期取引展開
    ├ report.rs             集計・トレンド計算
    └ ledger.rs             取引・残高計算
  infra/                    SQLite/Keychain アダプタ
  migrations/               SQL マイグレーション (V001__init.sql 形式)
```

## 必ず守る規約

### 1. 金額は INTEGER で扱う
すべての金額は整数(円)で保持・計算する。Rust では `i64`、TypeScript では `number` だが小数演算を入れない。浮動小数化が必要に見えたら設計を疑うこと。

### 2. Rust にコアロジック、Svelte は UI のみ
集計・予算評価・定期取引展開・レポート計算は **`src-tauri/src/domain/` 内に書く**。Svelte 側で `transactions.filter(...).reduce(...)` のようなビジネスロジックを書かない。Svelte は `invoke('command_name', payload)` の結果を表示するだけ。

### 3. 振替 (transfer) は集計から除外
`transactions.type='transfer'` の行を「総支出」「総収入」集計に含めてはならない。SQL レベルで `WHERE type IN ('income','expense')` を明示する。

### 4. ユーザー定義データはハードコード禁止
銀行名・カード会社名・カテゴリ名は全てユーザー入力。コード中にプリセット文字列を埋め込まない。例外は `accounts.kind` の選択肢 (`cash` / `bank` / `credit_card` / `e_money` / `investment`) のみ。初回起動時のデフォルトカテゴリも「便利な初期値」であり、ユーザーは自由に変更可能であること。

### 5. 物理削除しない (論理削除のみ)
過去取引を持つ可能性のあるカテゴリ・口座は `archived_at` に時刻を入れて非表示にする。`DELETE FROM categories` のようなクエリは禁止。唯一の例外は JSON バックアップからの全置換 (上書きインポート / リカバリ復元) で、単一トランザクション内で全テーブルを入れ替える。通常の CRUD からこの経路を呼んではならない。

### 6. スキーマ変更は migration ファイル経由
`src-tauri/migrations/V<番号>__<説明>.sql` を追加し、`app_meta.schema_version` を上げる。既存マイグレーションファイルの編集は禁止 (デプロイ済みの DB が壊れる)。

### 7. 起動時の定期取引展開は冪等に
`recurring_rules.last_generated_on` を必ず更新する。アプリを連続起動しても同一日付の取引を二重に作らないこと。

### 8. 暗号化キーを失わない設計
DB ファイルを暗号化しているため鍵紛失=データ消失。UI で「JSON で定期バックアップを取って」と促すこと。Settings 画面に最終バックアップ日時を表示する。

## 開発ワークフロー

### TDD を Rust domain 層で徹底
`domain/` 配下の純粋関数は「テストを先に書く → 失敗を確認 → 実装 → 緑」のサイクル。金額・日付の境界値は `proptest` を併用する。

### フェーズ分割
spec の Phase 1〜5 を順に消化する。各フェーズの完了条件:

1. `cargo clippy -- -D warnings` 緑
2. `cargo test` 緑
3. `pnpm test` (Vitest) 緑
4. `pnpm check` (svelte-check) 緑
5. 最低 1 本の Playwright E2E テスト追加
6. macOS と Windows の両方で実機ビルド成功

これらが揃わないうちは次フェーズへ進まない。

### コミットメッセージ
Conventional Commits 形式: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `perf:`, `ci:`。本文には「なぜ」を書く。実装の何を変えたかは diff を読めばわかる。

### 設計変更が必要になったら
spec を先に更新する。コードと spec を同期させること。spec とコードが矛盾するまま放置しない。

## 開発コマンド

````bash
# Dev サーバー起動 (Tauri ウィンドウ)
pnpm tauri dev

# Web のみ (Tauri なし、ブラウザで http://localhost:1420)
pnpm dev

# Rust テスト
cd src-tauri && cargo test

# Rust lint
cd src-tauri && cargo clippy --all-targets -- -D warnings

# フロントテスト
pnpm test

# 型チェック
pnpm check

# E2E (Vite dev server に対する Playwright)
pnpm test:e2e

# 配布ビルド (macOS)
pnpm tauri build --target universal-apple-darwin

# 配布ビルド (Windows)
pnpm tauri build --target x86_64-pc-windows-msvc
````

### E2E の制約 (Phase 1)
現状の E2E はブラウザ (`pnpm dev`) に対する Playwright スモークのみ。Tauri ウィンドウ上の E2E は `tauri-driver` 統合が必要で、Phase 2 以降の課題。Tauri ウィンドウ上での動作確認は `pnpm tauri dev` での手動確認で代替する。

## 参考: 既存 `家計簿.html` の扱い

ルートにある `家計簿.html` は今回のプロジェクトの **出発点となったプロトタイプ**。以下の点で参考にする:

- UI の華やかなグラデーション配色 (`#667eea → #764ba2` / `#4facfe → #00f2fe`)
- Chart.js での月別棒グラフ表現
- カテゴリ管理 UI のパターン
- Excel/JSON インポート・エクスポートのバリデーションロジック

ただし **JavaScript コードは移植せず、Rust + Svelte で再実装する**。データ移行も行わない (新規スタート)。
