# BudgetTracker

macOS / Windows 両対応の家計簿デスクトップアプリ。Tauri + Svelte 5 + Rust + SQLite (SQLCipher) で構築。

> ⚠️ **開発中** — 現在 **Phase 5b** (分析レポート4タブ) 完了。次は Phase 6a (リリース基盤)。

## 特長(設計目標)

- 🔒 **DB 全体を AES-256 で暗号化** — 暗号化キーは OS Keychain (macOS Keychain / Windows Credential Manager) に保管
- 💴 **金額は整数(円)で保持** — 浮動小数の丸め誤差を完全排除
- 📊 **本格的な家計管理** — 複数口座・予算・定期取引・カテゴリ別レポートを提供予定
- 🖥 **クロスプラットフォーム** — macOS (Intel/Apple Silicon) と Windows (x86_64) 向けにビルド
- 📴 **完全ローカル** — クラウド同期なし。データは端末内に閉じる
- 🧮 **コアロジックは Rust 側** — 集計・予算評価・定期取引展開は型安全なドメイン層で実装、proptest で検証

## インストール

[Releases](https://github.com/SilentMalachite/BudgetTracker/releases) から
お使いの OS 向けのファイルをダウンロードしてください。

- **macOS**: `.dmg` (Intel / Apple Silicon 共用)
- **Windows**: `.msi` または `.exe` (x86_64)

### 初回起動 — 未署名ビルドの警告について

配布ビルドには**コード署名がありません**（開発者証明書を取得していないため）。
そのため初回起動時に OS が警告を出します。以下の手順で起動できます。

**macOS** — 「開発元を確認できないため開けません」と出た場合:

1. `BudgetTracker.app` を **右クリック**（または Control + クリック）
2. メニューから **「開く」** を選ぶ
3. 確認ダイアログでもう一度 **「開く」**

2回目以降は通常どおりダブルクリックで起動します。それでも開けない場合:

```bash
xattr -d com.apple.quarantine /Applications/BudgetTracker.app
```

**Windows** — SmartScreen が「WindowsによってPCが保護されました」と出した場合:

1. **「詳細情報」** をクリック
2. **「実行」** をクリック

> 警告が出るのは署名が無いためで、アプリの動作そのものとは無関係です。
> 気になる場合はソースからビルドしてください（下記「開発」節）。

### データについて

- データは端末内の暗号化 SQLite に保存され、外部へ送信されません
- **暗号鍵を失うとデータは復元できません。** 設定画面から JSON バックアップを
  定期的に取得してください。最終バックアップ日時は設定画面に表示されます

## 技術スタック

| レイヤー | 採用 |
|---|---|
| デスクトップシェル | Tauri 2.x |
| フロントエンド | Svelte 5 (Runes) + TypeScript + Vite |
| バックエンド | Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`) |
| 鍵管理 | `keyring` crate |
| グラフ | Chart.js |
| テスト | `cargo test` / `proptest` / Vitest / Playwright |

## 必要環境

- **Node.js** 20+ と **pnpm** 10+
- **Rust** stable (1.78+)
- **Tauri 2.x** の[システム要件](https://v2.tauri.app/start/prerequisites/)
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools + WebView2 + NASM (openssl-sys 用)

## 開発

```bash
# 依存インストール
pnpm install

# Tauri ウィンドウで起動(推奨)
pnpm tauri dev

# ブラウザのみで起動 (http://localhost:1420)
pnpm dev

# テスト
pnpm test            # Vitest (フロント)
pnpm test:e2e        # Playwright (E2E スモーク)
pnpm check           # svelte-check (型)

cd src-tauri
cargo test                                    # Rust テスト
cargo clippy --all-targets -- -D warnings     # Rust lint
```

### 配布ビルド

```bash
# macOS (universal)
pnpm tauri build --target universal-apple-darwin

# Windows (x86_64)
pnpm tauri build --target x86_64-pc-windows-msvc
```

## アーキテクチャ

```
src/                        Svelte フロントエンド (UI のみ)
  lib/api/                  Tauri invoke の型付きラッパー
  lib/stores/               Svelte 5 Runes 状態
  lib/components/           共通 UI

src-tauri/src/
  commands/                 Tauri コマンド層 (薄い)
  domain/                   ビジネスロジック (純粋関数中心)
  infra/                    SQLite/Keychain アダプタ
  migrations/               SQL マイグレーション
```

詳細は **[docs/superpowers/specs/2026-05-24-budget-tracker-design.md](docs/superpowers/specs/2026-05-24-budget-tracker-design.md)** が設計の正本。実装やリファクタの前に必ず参照すること。

## 開発の主要ルール

- 金額は `i64` (Rust) / `number` (TS) で整数のまま扱う。浮動小数演算を入れない
- 集計・予算評価などのロジックは Rust の `domain/` に集中。Svelte 側でビジネスロジックを書かない
- `transactions.type='transfer'` は総支出/総収入から除外 (SQL レベルで明示)
- カテゴリ・口座は **論理削除のみ** (`archived_at` を立てる、`DELETE` 禁止)
- 銀行名・カード会社名・カテゴリ名は全てユーザー入力。プリセット文字列を埋め込まない
- スキーマ変更は `src-tauri/migrations/V<番号>__<説明>.sql` を追加し `app_meta.schema_version` を更新

## ロードマップ

- [x] **Phase 1** — Tauri スキャフォールド / SQLCipher / Keychain / CI (macOS + Windows)
- [x] **Phase 2** — 取引 CRUD + カテゴリ + ダッシュボード
- [x] **Phase 3** — 複数口座 + 振替 + 残高計算
- [x] **Phase 4** — 予算管理 (月別カテゴリ予算 + 進捗/警告バッジ)
- [x] **Phase 5a** — 定期取引 (起動時の冪等な自動展開 + ルール管理)
- [x] **Phase 5b** — 分析レポート (月次 / 年次 / カテゴリ別 / トレンドの4タブ)
- [ ] **Phase 6a** — リリース基盤 (タグ push で配布ビルド → draft release)
- [ ] **Phase 6c** — Tauri WebView 上の E2E (WebdriverIO)
- [ ] **Phase 6b** — Excel エクスポート / インポート

## CI

- `.github/workflows/ci.yml` — PR と `main` への push で macOS + Windows の
  型検査 / Vitest / Playwright / clippy / `cargo test` を実行
- `.github/workflows/release.yml` — `v*` タグの push で両 OS の配布ビルドを作り、
  draft release に添付する。公開手順は [docs/release-checklist.md](docs/release-checklist.md)

## 参考

ルート直下の `家計簿.html` は本プロジェクトの出発点となった単一HTMLプロトタイプ。UI 配色とカテゴリ管理パターンを参考にするが、コードは移植せず Rust + Svelte で再実装している。

## ライセンス

[MIT License](LICENSE) © 2026 Silent Malachite
