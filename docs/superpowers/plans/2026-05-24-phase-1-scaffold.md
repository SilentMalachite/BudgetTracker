# Phase 1: Scaffold + SQLCipher/Keychain Foundation 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Worktree:** Execute this plan inside an isolated worktree created via `superpowers:using-git-worktrees`. Do NOT execute directly on `main`.

**Goal:** Tauri 2.x + Svelte 5 + Rust の最小起動可能スキャフォールドを構築し、SQLCipher 暗号化 DB の初期化と OS Keychain 経由の鍵管理が end-to-end で動作する状態にする。

**Architecture:** Tauri 2.x がデスクトップシェル。フロントは Svelte 5 (Runes) + TypeScript + Vite。バックエンドは Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`)。アプリ起動時に Rust 側で `keyring` crate から AES-256 鍵を取得 (なければ生成して保存) し、SQLCipher で `data.db` を開いてマイグレーションを適用。最初の Tauri コマンド `app_info` で疎通を確認する。

**Tech Stack:** Tauri 2.x, Svelte 5, TypeScript 5.x, Vite 5.x, Rust 1.78+, rusqlite 0.32 (`bundled-sqlcipher-vendored-openssl`), keyring 3.x, thiserror, serde, base64, include_dir, chrono, rand, tempfile (dev), Vitest, Playwright

**完了基準 (CLAUDE.md より):**
1. `cd src-tauri && cargo clippy -- -D warnings` 緑
2. `cd src-tauri && cargo test` 緑
3. `pnpm test` (Vitest) 緑
4. `pnpm check` (svelte-check) 緑
5. 最低 1 本の Playwright E2E テスト追加
6. macOS と Windows の両方で実機ビルド成功 (ユーザー実機確認、CI でも検証)

---

## File Structure

Phase 1 で作成・変更する全ファイル:

```
BudgetTracker/
├── .github/workflows/ci.yml         (新規) CI: lint+test on macOS/Windows
├── .gitignore                       (追記) node_modules, target, .db 等
├── CLAUDE.md                        (追記) 開発コマンド欄を埋める
├── package.json                     (新規)
├── pnpm-lock.yaml                   (自動生成)
├── tsconfig.json                    (新規)
├── tsconfig.node.json               (新規)
├── vite.config.ts                   (新規)
├── vitest.config.ts                 (新規)
├── playwright.config.ts             (新規)
├── svelte.config.js                 (新規)
├── index.html                       (新規)
├── src/
│   ├── main.ts                      (新規) Svelte エントリ
│   ├── App.svelte                   (新規) ルートコンポーネント
│   ├── app.css                      (新規) 既存HTMLの配色を継承するベース
│   ├── vite-env.d.ts                (新規)
│   ├── lib/
│   │   ├── api/index.ts             (新規) invoke 型付きラッパー
│   │   └── utils/
│   │       ├── formatCurrency.ts    (新規) Vitest デモ用
│   │       └── formatCurrency.test.ts (新規)
│   └── setupTests.ts                (新規) Vitest setup
├── src-tauri/
│   ├── Cargo.toml                   (新規)
│   ├── build.rs                     (新規)
│   ├── tauri.conf.json              (新規)
│   ├── capabilities/default.json    (新規)
│   ├── icons/icon.png               (新規 placeholder)
│   ├── migrations/V001__init.sql    (新規)
│   └── src/
│       ├── main.rs                  (新規)
│       ├── lib.rs                   (新規) run() エントリ
│       ├── error.rs                 (新規)
│       ├── commands/
│       │   ├── mod.rs               (新規)
│       │   └── meta.rs              (新規) app_info
│       ├── domain/mod.rs            (新規 空、Phase 2 以降で埋める)
│       └── infra/
│           ├── mod.rs               (新規)
│           ├── db.rs                (新規) SQLCipher 接続
│           ├── keychain.rs          (新規) keyring ラッパー
│           └── migrations.rs        (新規) マイグレーションランナー
└── tests/e2e/smoke.spec.ts          (新規) Playwright スモーク
```

**境界の整理:**
- `infra/keychain.rs`: OS Keychain 操作のみ。32 バイト鍵の取得/生成/削除。
- `infra/db.rs`: 暗号化 DB の open のみ。マイグレーションは含まない。
- `infra/migrations.rs`: SQL ファイルを順次適用。`app_meta.schema_version` を更新。
- `commands/meta.rs`: 疎通確認用。後続フェーズで増えるコマンドの土台。
- `lib.rs`: Tauri アプリのエントリ。`setup` フックで keychain → db → migrations を順に実行。

---

## Task 1: pnpm + Vite + Svelte 5 + TypeScript スキャフォールド

**Files:**
- Create: `package.json`, `tsconfig.json`, `tsconfig.node.json`, `vite.config.ts`, `svelte.config.js`, `index.html`, `src/main.ts`, `src/App.svelte`, `src/app.css`, `src/vite-env.d.ts`
- Modify: `.gitignore`

- [ ] **Step 1: `.gitignore` に Node/Rust/Tauri エントリを追記**

既存の `.gitignore` の末尾に以下を追記:

```gitignore
# Node
node_modules/
dist/
pnpm-debug.log*

# Rust / Tauri
src-tauri/target/
src-tauri/Cargo.lock

# Local data
*.db
*.db-journal
.env.local

# Test artifacts
coverage/
playwright-report/
test-results/
```

(`src-tauri/Cargo.lock` はライブラリではなくアプリなので通常コミットするが、Phase 1 のうちは依存固定が不安定なので一旦無視。Phase 2 開始時に外す。)

- [ ] **Step 2: `package.json` を作成**

```json
{
  "name": "budget-tracker",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview --port 1420 --strictPort",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:e2e": "playwright test",
    "tauri": "tauri"
  },
  "devDependencies": {
    "@playwright/test": "^1.48.0",
    "@sveltejs/vite-plugin-svelte": "^4.0.0",
    "@tauri-apps/cli": "^2.1.0",
    "@testing-library/svelte": "^5.2.0",
    "@tsconfig/svelte": "^5.0.4",
    "jsdom": "^25.0.0",
    "svelte": "^5.0.0",
    "svelte-check": "^4.0.0",
    "tslib": "^2.7.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0",
    "vitest": "^2.1.0"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.1.0"
  },
  "packageManager": "pnpm@9.12.0"
}
```

- [ ] **Step 3: TypeScript 設定**

`tsconfig.json`:

```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "resolveJsonModule": true,
    "allowJs": false,
    "checkJs": false,
    "isolatedModules": true,
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "skipLibCheck": true,
    "verbatimModuleSyntax": true,
    "types": ["vite/client", "svelte"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "src/**/*.d.ts"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

`tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts", "vitest.config.ts", "playwright.config.ts", "svelte.config.js"]
}
```

- [ ] **Step 4: Vite/Svelte 設定**

`svelte.config.js`:

```js
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    runes: true
  }
};
```

`vite.config.ts`:

```ts
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*']
}));
```

- [ ] **Step 5: HTML + Svelte エントリ**

`index.html`:

```html
<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>BudgetTracker</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

`src/vite-env.d.ts`:

```ts
/// <reference types="svelte" />
/// <reference types="vite/client" />
```

`src/app.css`:

```css
:root {
  --bg-grad-start: #667eea;
  --bg-grad-end: #764ba2;
  --accent-grad-start: #4facfe;
  --accent-grad-end: #00f2fe;
  --text: #1a1a2e;
  --muted: #5a5a7a;
  --card-bg: rgba(255, 255, 255, 0.95);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Hiragino Sans',
    'Yu Gothic UI', sans-serif;
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  min-height: 100vh;
  color: var(--text);
  background: linear-gradient(135deg, var(--bg-grad-start), var(--bg-grad-end));
}

#app { min-height: 100vh; }
```

`src/main.ts`:

```ts
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

const target = document.getElementById('app');
if (!target) throw new Error('#app not found');

export default mount(App, { target });
```

`src/App.svelte` (Phase 1 で `app_info` を呼び出すミニマム実装):

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppInfo, type AppInfo } from './lib/api';

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      info = await getAppInfo();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });
</script>

<main>
  <h1>BudgetTracker</h1>
  {#if info}
    <p data-testid="schema-version">Schema version: {info.schema_version}</p>
    <p data-testid="db-path">DB: {info.db_path}</p>
  {:else if error}
    <p data-testid="error">Error: {error}</p>
  {:else}
    <p data-testid="loading">Loading…</p>
  {/if}
</main>

<style>
  main {
    max-width: 720px;
    margin: 4rem auto;
    padding: 2rem;
    background: var(--card-bg);
    border-radius: 16px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
  }
  h1 {
    margin-top: 0;
    background: linear-gradient(135deg, var(--bg-grad-start), var(--bg-grad-end));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
</style>
```

`src/lib/api/index.ts` (Tauri はまだ呼ばないモック):

```ts
import { invoke } from '@tauri-apps/api/core';

export type AppInfo = {
  schema_version: number;
  db_path: string;
};

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info');
}
```

- [ ] **Step 6: 依存をインストール、型チェックが通ることを確認**

Run:

```bash
pnpm install
pnpm check
```

Expected: `pnpm check` の最後に `0 errors and 0 warnings`。`@tauri-apps/api/core` は Task 2 で Tauri を組み込むまで実体は使われないが、import は解決される。

- [ ] **Step 7: コミット**

```bash
git add .gitignore package.json pnpm-lock.yaml tsconfig.json tsconfig.node.json \
  svelte.config.js vite.config.ts index.html \
  src/main.ts src/App.svelte src/app.css src/vite-env.d.ts src/lib/api/index.ts
git commit -m "chore: scaffold svelte 5 + vite + typescript frontend"
```

---

## Task 2: Tauri 2.x スキャフォールド (Rust 側)

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/icons/icon.png`

- [ ] **Step 1: `src-tauri/Cargo.toml`**

```toml
[package]
name = "budget-tracker"
version = "0.1.0"
description = "Cross-platform desktop budget tracker"
authors = ["BudgetTracker Authors"]
edition = "2021"
rust-version = "1.78"

[lib]
name = "budget_tracker_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
rusqlite = { version = "0.32", features = ["bundled-sqlcipher-vendored-openssl"] }
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service"] }
base64 = "0.22"
rand = "0.8"
include_dir = "0.7"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }

[dev-dependencies]
tempfile = "3"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 2: `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 3: `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "BudgetTracker",
  "version": "0.1.0",
  "identifier": "jp.budget-tracker.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "BudgetTracker",
        "width": 1180,
        "height": 780,
        "minWidth": 960,
        "minHeight": 640,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: https://asset.localhost; connect-src ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"],
    "category": "Finance",
    "shortDescription": "Personal budget tracker",
    "longDescription": "Cross-platform encrypted personal budget tracker."
  }
}
```

- [ ] **Step 4: `src-tauri/capabilities/default.json`**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default"
  ]
}
```

- [ ] **Step 5: Tauri エントリ**

`src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    budget_tracker_lib::run();
}
```

`src-tauri/src/lib.rs` (Phase 1 で最終形になる前のミニマム):

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: アイコン placeholder**

ImageMagick / sips / 自作 PNG など何でも良いが、1024x1024 の透明 PNG を `src-tauri/icons/icon.png` に置く。最小例として ImageMagick が無くても以下で 512x512 の単色 PNG が生成できる:

```bash
mkdir -p src-tauri/icons
# macOS: 標準の sips で生成
python3 - <<'PY'
from pathlib import Path
import zlib, struct
def png(width, height, rgba):
    sig = b'\x89PNG\r\n\x1a\n'
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    ihdr = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    raw = b''.join(b'\x00' + bytes(rgba) * width for _ in range(height))
    idat = zlib.compress(raw)
    return sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')
Path('src-tauri/icons/icon.png').write_bytes(png(512, 512, [102, 126, 234, 255]))
PY
```

最終的な配布ビルドではちゃんとしたアイコンに差し替えるが、Phase 1 完了基準には含めない (Phase 5 で差し替え)。

- [ ] **Step 7: `pnpm tauri dev` で起動確認**

Run:

```bash
pnpm tauri dev
```

Expected: Tauri がフロントを含めてビルドし、ウィンドウが開いて Svelte の画面が表示される。`Loading…` の後に `Error: Command app_info not found` 相当のメッセージが出る (まだコマンド未実装なので想定通り)。確認できたら停止 (Ctrl-C)。

- [ ] **Step 8: コミット**

```bash
git add src-tauri
git commit -m "feat: scaffold tauri 2.x rust shell"
```

---

## Task 3: エラー型 (`error.rs`)

**Files:**
- Create: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: テストファイルを書く**

`src-tauri/src/error.rs` の末尾に以下のテストを書く想定で、まずファイル全体を作る:

```rust
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("keychain error: {0}")]
    Keychain(#[from] keyring::Error),

    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_error_displays_message() {
        let err = AppError::Migration("V002 broken".into());
        assert_eq!(err.to_string(), "migration error: V002 broken");
    }

    #[test]
    fn error_serializes_to_string() {
        let err = AppError::InvalidArgument("amount must be > 0".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"invalid argument: amount must be > 0\"");
    }
}
```

- [ ] **Step 2: `lib.rs` に `mod error;` を追加してビルド**

`src-tauri/src/lib.rs` の冒頭に追加:

```rust
mod error;
```

- [ ] **Step 3: テストが通ることを確認**

Run:

```bash
cd src-tauri
cargo test error::tests
```

Expected: 2 passed.

- [ ] **Step 4: コミット**

```bash
git add src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat(error): add AppError type with thiserror + serialize"
```

---

## Task 4: keychain ラッパー (TDD)

**Files:**
- Create: `src-tauri/src/infra/mod.rs`, `src-tauri/src/infra/keychain.rs`
- Modify: `src-tauri/src/lib.rs`

設計: 32 バイトのランダム鍵を base64 でエンコードして OS Keychain に格納する。`get_or_create_key` がアトミックに「あれば取得・なければ生成」を行う。テストは `keyring::set_default_credential_builder(keyring::mock::default_credential_builder())` を使って OS Keychain を触らない。

- [ ] **Step 1: 失敗するテストを書く**

`src-tauri/src/infra/mod.rs`:

```rust
pub mod db;
pub mod keychain;
pub mod migrations;
```

(`db.rs` と `migrations.rs` はまだ存在しないので、コンパイルを通すために空ファイルを作る:)

```bash
mkdir -p src-tauri/src/infra
touch src-tauri/src/infra/db.rs src-tauri/src/infra/migrations.rs
```

`src-tauri/src/infra/keychain.rs`:

```rust
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use rand::RngCore;

use crate::error::{AppError, AppResult};

pub const KEY_LEN: usize = 32;
pub type DbKey = [u8; KEY_LEN];

/// Retrieve the stored DB key or, if absent, generate a new 32-byte key,
/// store it in the OS keychain, and return it.
///
/// `service` typically: "jp.budget-tracker"
/// `account` typically: "db_key"
pub fn get_or_create_key(service: &str, account: &str) -> AppResult<DbKey> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.get_password() {
        Ok(b64) => {
            let raw = STANDARD_NO_PAD.decode(b64.as_bytes())?;
            if raw.len() != KEY_LEN {
                return Err(AppError::Migration(format!(
                    "stored key has wrong length: {}",
                    raw.len()
                )));
            }
            let mut out = [0u8; KEY_LEN];
            out.copy_from_slice(&raw);
            Ok(out)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; KEY_LEN];
            rand::thread_rng().fill_bytes(&mut key);
            let b64 = STANDARD_NO_PAD.encode(key);
            entry.set_password(&b64)?;
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
pub fn delete_key(service: &str, account: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn ensure_mock() {
        INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    fn unique_account(prefix: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!("{}-{}", prefix, N.fetch_add(1, Ordering::SeqCst))
    }

    #[test]
    fn creates_key_when_absent() {
        ensure_mock();
        let account = unique_account("create");
        let key = get_or_create_key("test", &account).unwrap();
        assert_eq!(key.len(), KEY_LEN);
        assert!(key.iter().any(|&b| b != 0), "key should not be all zeros");
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn returns_same_key_on_second_call() {
        ensure_mock();
        let account = unique_account("idempotent");
        let first = get_or_create_key("test", &account).unwrap();
        let second = get_or_create_key("test", &account).unwrap();
        assert_eq!(first, second);
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn different_accounts_get_different_keys() {
        ensure_mock();
        let a1 = unique_account("diff-a");
        let a2 = unique_account("diff-b");
        let k1 = get_or_create_key("test", &a1).unwrap();
        let k2 = get_or_create_key("test", &a2).unwrap();
        assert_ne!(k1, k2);
        delete_key("test", &a1).unwrap();
        delete_key("test", &a2).unwrap();
    }
}
```

- [ ] **Step 2: `lib.rs` に `mod infra;` を追加**

`src-tauri/src/lib.rs` の冒頭に追加 (順番が重要、 `error` が先):

```rust
mod error;
mod infra;
```

- [ ] **Step 3: テストが失敗することを確認**

Run:

```bash
cd src-tauri
cargo test keychain
```

Expected: コンパイルエラーまたはテスト失敗 (まず `keyring::mock` が `mock` feature を要求するため Cargo.toml に追加が必要)。コンパイルエラーが出る場合、`keyring` の `features` に `"mock"` を含めるよう Cargo.toml を修正:

```toml
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service", "mock"] }
```

修正後に再実行し、3 つのテストが pass になることを確認。

- [ ] **Step 4: 全テスト実行で 5 件 (error 2 + keychain 3) すべて green**

Run:

```bash
cd src-tauri
cargo test
```

Expected: `test result: ok. 5 passed`.

- [ ] **Step 5: コミット**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/src/infra
git commit -m "feat(keychain): wrap OS keychain with get_or_create_key for 32-byte AES key"
```

---

## Task 5: 初期マイグレーション SQL (`V001__init.sql`)

**Files:**
- Create: `src-tauri/migrations/V001__init.sql`

設計書セクション 4.1 のスキーマを 1 ファイルに落とす。テーブル順序は外部キー参照を満たすように `accounts → categories → recurring_rules → transactions → budgets → app_meta` の順。

- [ ] **Step 1: SQL ファイル作成**

`src-tauri/migrations/V001__init.sql`:

```sql
-- =========================================================
-- V001: initial schema
-- =========================================================
-- All monetary values are stored as INTEGER (yen, no decimals).
-- Soft-delete via `archived_at` (ISO 8601 string); never DELETE rows that
-- may have history (categories / accounts).
-- =========================================================

PRAGMA foreign_keys = ON;

-- ---------------------------------------------------------
-- accounts: cash / bank / credit_card / e_money / investment
-- ---------------------------------------------------------
CREATE TABLE accounts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  name            TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK(kind IN ('cash','bank','credit_card','e_money','investment')),
  currency        TEXT    NOT NULL DEFAULT 'JPY',
  initial_balance INTEGER NOT NULL DEFAULT 0,
  display_order   INTEGER NOT NULL DEFAULT 0,
  note            TEXT    NOT NULL DEFAULT '',
  archived_at     TEXT,
  created_at      TEXT    NOT NULL,
  updated_at      TEXT    NOT NULL
);

-- ---------------------------------------------------------
-- categories: income / expense
-- ---------------------------------------------------------
CREATE TABLE categories (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT    NOT NULL,
  type          TEXT    NOT NULL CHECK(type IN ('income','expense')),
  color         TEXT,
  icon          TEXT,
  display_order INTEGER NOT NULL DEFAULT 0,
  archived_at   TEXT,
  UNIQUE(name, type)
);

-- ---------------------------------------------------------
-- recurring_rules: must come before `transactions` because
-- `transactions.recurring_id` references it.
-- ---------------------------------------------------------
CREATE TABLE recurring_rules (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  name               TEXT    NOT NULL,
  type               TEXT    NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),
  category_id        INTEGER REFERENCES categories(id),
  description        TEXT    NOT NULL DEFAULT '',
  frequency          TEXT    NOT NULL CHECK(frequency IN ('monthly','weekly','yearly')),
  day_of_month       INTEGER,
  day_of_week        INTEGER,
  starts_on          TEXT    NOT NULL,
  ends_on            TEXT,
  last_generated_on  TEXT,
  active             INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1))
);

-- ---------------------------------------------------------
-- transactions: income / expense / transfer
-- ---------------------------------------------------------
CREATE TABLE transactions (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  occurred_on        TEXT    NOT NULL,
  type               TEXT    NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),
  category_id        INTEGER REFERENCES categories(id),
  description        TEXT    NOT NULL DEFAULT '',
  recurring_id       INTEGER REFERENCES recurring_rules(id) ON DELETE SET NULL,
  created_at         TEXT    NOT NULL,
  updated_at         TEXT    NOT NULL,
  -- transfer の場合は counter_account_id 必須・category_id NULL
  CHECK(
    (type = 'transfer' AND counter_account_id IS NOT NULL AND category_id IS NULL)
    OR (type IN ('income','expense') AND counter_account_id IS NULL AND category_id IS NOT NULL)
  )
);
CREATE INDEX idx_tx_occurred_on ON transactions(occurred_on);
CREATE INDEX idx_tx_account     ON transactions(account_id);
CREATE INDEX idx_tx_category    ON transactions(category_id);

-- ---------------------------------------------------------
-- budgets: per-category monthly/yearly budgets
-- ---------------------------------------------------------
CREATE TABLE budgets (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  category_id     INTEGER NOT NULL REFERENCES categories(id),
  period          TEXT    NOT NULL CHECK(period IN ('monthly','yearly')),
  amount          INTEGER NOT NULL CHECK(amount >= 0),
  starts_on       TEXT    NOT NULL,
  ends_on         TEXT,
  alert_threshold INTEGER NOT NULL DEFAULT 80 CHECK(alert_threshold BETWEEN 0 AND 200),
  UNIQUE(category_id, starts_on)
);

-- ---------------------------------------------------------
-- app_meta: key/value store for schema_version, last_backup_at, theme, ...
-- ---------------------------------------------------------
CREATE TABLE app_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

注: spec の `transactions` テーブル定義に「振替なら category_id NULL」の制約は明示的になかったが、ビジネスルール (CLAUDE.md 規約 3) を SQL レベルで強制するために CHECK 制約を追加した。これは spec の意図と一致する。

- [ ] **Step 2: SQLite で構文確認**

Run:

```bash
sqlite3 ":memory:" < src-tauri/migrations/V001__init.sql && echo OK
```

Expected: `OK`。エラーが出たら SQL を修正。

- [ ] **Step 3: コミット**

```bash
git add src-tauri/migrations/V001__init.sql
git commit -m "feat(db): add V001 initial schema migration"
```

---

## Task 6: マイグレーションランナー (TDD)

**Files:**
- Create: `src-tauri/src/infra/migrations.rs`

設計: `include_dir!` で `migrations/` をバイナリに埋め込み、ファイル名 `V<NNN>__<desc>.sql` を昇順に並べて、`app_meta.schema_version` 未満のもののみ実行する。

- [ ] **Step 1: 失敗するテストを書く**

`src-tauri/src/infra/migrations.rs`:

```rust
use std::cmp::Ordering;

use include_dir::{Dir, include_dir};
use rusqlite::{Connection, params};

use crate::error::{AppError, AppResult};

static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

struct Migration {
    version: u32,
    name: String,
    sql: String,
}

fn load_migrations() -> AppResult<Vec<Migration>> {
    let mut out = Vec::new();
    for file in MIGRATIONS_DIR.files() {
        let stem = file
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Migration(format!("bad filename: {:?}", file.path())))?;
        // Expect: V<NNN>__<desc>
        let rest = stem
            .strip_prefix('V')
            .ok_or_else(|| AppError::Migration(format!("filename must start with V: {stem}")))?;
        let (num, desc) = rest
            .split_once("__")
            .ok_or_else(|| AppError::Migration(format!("filename must contain __: {stem}")))?;
        let version: u32 = num
            .parse()
            .map_err(|_| AppError::Migration(format!("bad version in {stem}")))?;
        let sql = file
            .contents_utf8()
            .ok_or_else(|| AppError::Migration(format!("non-utf8 sql in {stem}")))?
            .to_string();
        out.push(Migration {
            version,
            name: desc.to_string(),
            sql,
        });
    }
    out.sort_by(|a, b| match a.version.cmp(&b.version) {
        Ordering::Equal => a.name.cmp(&b.name),
        other => other,
    });
    // Detect duplicates.
    for pair in out.windows(2) {
        if pair[0].version == pair[1].version {
            return Err(AppError::Migration(format!(
                "duplicate migration version: V{:03}",
                pair[0].version
            )));
        }
    }
    Ok(out)
}

fn ensure_app_meta(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    Ok(())
}

fn current_version(conn: &Connection) -> AppResult<u32> {
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok::<Option<String>, rusqlite::Error>(None),
            other => Err(other),
        })?;
    Ok(v.as_deref().map(str::parse).transpose().map_err(|_| {
        AppError::Migration("schema_version is not an integer".into())
    })?.unwrap_or(0))
}

fn set_version(conn: &Connection, version: u32) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![version.to_string()],
    )?;
    Ok(())
}

/// Apply all pending migrations against `conn`. Returns the new schema_version.
pub fn run(conn: &mut Connection) -> AppResult<u32> {
    ensure_app_meta(conn)?;
    let mut version = current_version(conn)?;
    let migrations = load_migrations()?;
    for m in &migrations {
        if m.version <= version {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(&m.sql)
            .map_err(|e| AppError::Migration(format!("V{:03} ({}) failed: {e}", m.version, m.name)))?;
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![m.version.to_string()],
        )?;
        tx.commit()?;
        version = m.version;
    }
    set_version(conn, version)?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn applies_v001_to_empty_db() {
        let mut conn = fresh();
        let v = run(&mut conn).unwrap();
        assert_eq!(v, 1);
        let names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for expected in [
            "accounts",
            "app_meta",
            "budgets",
            "categories",
            "recurring_rules",
            "transactions",
        ] {
            assert!(names.iter().any(|n| n == expected), "missing table {expected}");
        }
    }

    #[test]
    fn is_idempotent_on_second_run() {
        let mut conn = fresh();
        let v1 = run(&mut conn).unwrap();
        let v2 = run(&mut conn).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v2, 1);
    }

    #[test]
    fn enforces_transfer_check_constraint() {
        let mut conn = fresh();
        run(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO accounts(name, kind, created_at, updated_at)
               VALUES('cash','cash','2026-01-01','2026-01-01');
             INSERT INTO categories(name, type) VALUES('Food','expense');",
        )
        .unwrap();
        // transfer with category_id should fail.
        let err = conn.execute(
            "INSERT INTO transactions(occurred_on, type, amount, account_id, counter_account_id, category_id, created_at, updated_at)
              VALUES('2026-01-15','transfer',1000,1,1,1,'2026-01-15','2026-01-15')",
            [],
        );
        assert!(err.is_err(), "transfer with category_id must be rejected");
    }
}
```

- [ ] **Step 2: テストが通ることを確認**

Run:

```bash
cd src-tauri
cargo test migrations
```

Expected: 3 passed (`applies_v001_to_empty_db`, `is_idempotent_on_second_run`, `enforces_transfer_check_constraint`)。

- [ ] **Step 3: 全テスト再実行**

Run:

```bash
cd src-tauri
cargo test
```

Expected: 8 passed (error 2 + keychain 3 + migrations 3)。

- [ ] **Step 4: コミット**

```bash
git add src-tauri/src/infra/migrations.rs
git commit -m "feat(db): add embedded migration runner with idempotent apply"
```

---

## Task 7: 暗号化 DB 接続 (`db.rs`)

**Files:**
- Create/Replace: `src-tauri/src/infra/db.rs`

設計: `open_encrypted` 関数が `Path` と `&DbKey` を受け取り、`PRAGMA key` を適用して `Connection` を返す。`hex` 形式でキーを渡す (`PRAGMA key = "x'<64 hex>'"`)。

- [ ] **Step 1: 失敗するテストを書く**

`src-tauri/src/infra/db.rs`:

```rust
use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;
use crate::infra::keychain::{DbKey, KEY_LEN};

fn key_to_hex(key: &DbKey) -> String {
    let mut s = String::with_capacity(KEY_LEN * 2);
    for b in key {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// Open a SQLCipher-encrypted SQLite database at `path`. Creates the file if
/// absent. The connection has `PRAGMA key` applied. Foreign keys are enabled.
pub fn open_encrypted(path: &Path, key: &DbKey) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    let hex = key_to_hex(key);
    // PRAGMA key with raw bytes form `x'<hex>'` is safe against SQL injection
    // because hex is from our own 32-byte buffer.
    conn.pragma_update(None, "key", format!("x'{hex}'"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // Touch the database to force PRAGMA key to take effect on first use.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_key(seed: u8) -> DbKey {
        let mut k = [0u8; KEY_LEN];
        for (i, slot) in k.iter_mut().enumerate() {
            *slot = seed.wrapping_add(i as u8);
        }
        k
    }

    #[test]
    fn round_trip_data_through_encrypted_db() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let key = make_key(7);

        {
            let conn = open_encrypted(&path, &key).unwrap();
            conn.execute_batch(
                "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL);
                 INSERT INTO t(v) VALUES('hello');",
            )
            .unwrap();
        }

        let conn = open_encrypted(&path, &key).unwrap();
        let v: String = conn.query_row("SELECT v FROM t WHERE id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hello");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let right = make_key(1);
        let wrong = make_key(2);

        {
            let conn = open_encrypted(&path, &right).unwrap();
            conn.execute_batch("CREATE TABLE t(id INTEGER); INSERT INTO t VALUES(1);")
                .unwrap();
        }

        let bad = open_encrypted(&path, &wrong);
        assert!(bad.is_err(), "opening with wrong key must fail");
    }

    #[test]
    fn raw_file_does_not_contain_plaintext() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let key = make_key(42);
        let conn = open_encrypted(&path, &key).unwrap();
        conn.execute_batch(
            "CREATE TABLE t(v TEXT); INSERT INTO t(v) VALUES('SUPERSECRET_TOKEN');",
        )
        .unwrap();
        drop(conn);
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes.windows(17).any(|w| w == b"SUPERSECRET_TOKEN"),
            "plaintext leaked into encrypted db file"
        );
    }
}
```

- [ ] **Step 2: テストを実行**

Run:

```bash
cd src-tauri
cargo test db::tests
```

Expected: 3 passed。初回ビルドで `bundled-sqlcipher-vendored-openssl` のビルドに数分かかる。

- [ ] **Step 3: 全テスト再実行**

Run:

```bash
cd src-tauri
cargo test
```

Expected: 11 passed (error 2 + keychain 3 + migrations 3 + db 3)。

- [ ] **Step 4: コミット**

```bash
git add src-tauri/src/infra/db.rs
git commit -m "feat(db): add SQLCipher-encrypted connection opener"
```

---

## Task 8: アプリ起動シーケンス (lib.rs setup フック)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/meta.rs`
- Create: `src-tauri/src/domain/mod.rs`

設計:
- Tauri の `setup` フックで以下を実行 (順序固定):
  1. `app.path().app_data_dir()` で `<APPDATA>/jp.budget-tracker/` を取得 (Windows なら `%APPDATA%\jp.budget-tracker.app`, macOS なら `~/Library/Application Support/jp.budget-tracker.app`)
  2. ディレクトリを `create_dir_all`
  3. `keychain::get_or_create_key("jp.budget-tracker", "db_key")`
  4. `db::open_encrypted(&dir.join("data.db"), &key)`
  5. `migrations::run(&mut conn)`
  6. `AppState { conn: Mutex<Connection>, db_path: PathBuf }` を `manage` で登録
- セットアップ失敗時はパニックしてユーザー向けエラーダイアログを出す (Phase 1 は割愛、`expect` で即落ち)

- [ ] **Step 1: AppState 定義と meta コマンドを書く**

`src-tauri/src/commands/mod.rs`:

```rust
pub mod meta;
```

`src-tauri/src/commands/meta.rs`:

```rust
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub schema_version: u32,
    pub db_path: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let conn = state.conn.lock().map_err(|_| {
        AppError::Migration("connection lock poisoned".into())
    })?;
    let version: String = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "0".to_string());
    Ok(AppInfo {
        schema_version: version.parse().unwrap_or(0),
        db_path: state.db_path.to_string_lossy().to_string(),
    })
}
```

`src-tauri/src/domain/mod.rs`: Phase 1 では空でよい。

```rust
// Domain modules added in subsequent phases.
```

- [ ] **Step 2: `lib.rs` を完成形に書き換える**

`src-tauri/src/lib.rs`:

```rust
mod commands;
mod domain;
mod error;
mod infra;

use std::fs;
use std::sync::Mutex;

use tauri::Manager;

use crate::commands::meta::{app_info, AppState};
use crate::infra::{db, keychain, migrations};

const KEYCHAIN_SERVICE: &str = "jp.budget-tracker";
const KEYCHAIN_ACCOUNT: &str = "db_key";
const DB_FILENAME: &str = "data.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            fs::create_dir_all(&data_dir).expect("failed to create app data dir");

            let key = keychain::get_or_create_key(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                .expect("failed to acquire DB key from OS keychain");

            let db_path = data_dir.join(DB_FILENAME);
            let mut conn = db::open_encrypted(&db_path, &key)
                .expect("failed to open encrypted database");
            let _version = migrations::run(&mut conn).expect("failed to apply migrations");

            app.manage(AppState {
                conn: Mutex::new(conn),
                db_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: clippy 緑化**

Run:

```bash
cd src-tauri
cargo clippy --all-targets -- -D warnings
```

Expected: 警告ゼロ。出た警告は素直に修正 (使われていない import など)。

- [ ] **Step 4: `pnpm tauri dev` で end-to-end 起動**

Run:

```bash
cd ..  # back to repo root
pnpm tauri dev
```

Expected: ウィンドウが開き、Svelte の UI に以下が表示される:

```
BudgetTracker
Schema version: 1
DB: /Users/<you>/Library/Application Support/jp.budget-tracker.app/data.db
```

(Windows なら `C:\Users\<you>\AppData\Roaming\jp.budget-tracker.app\data.db`)

- [ ] **Step 5: 2 回目起動でキーが再利用されることを確認**

ウィンドウを閉じてもう一度 `pnpm tauri dev`。同じ DB パス・schema version 1 が表示され、エラーが出ないことを確認 (Keychain から鍵が取れていれば成功)。

- [ ] **Step 6: コミット**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands src-tauri/src/domain
git commit -m "feat(app): wire keychain + encrypted db + migrations into tauri setup"
```

---

## Task 9: Vitest セットアップ + 最小フロントテスト

**Files:**
- Create: `vitest.config.ts`, `src/setupTests.ts`, `src/lib/utils/formatCurrency.ts`, `src/lib/utils/formatCurrency.test.ts`

`formatCurrency` は実装規約 (金額は整数) のショーケースを兼ねる。Phase 1 で動くフロントの純粋関数として最も基本的なもの。

- [ ] **Step 1: 失敗するテストを書く**

`vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.ts'],
    include: ['src/**/*.test.ts']
  }
});
```

`src/setupTests.ts`:

```ts
// Placeholder for global test setup (mocks, polyfills).
```

`src/lib/utils/formatCurrency.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { formatCurrency } from './formatCurrency';

describe('formatCurrency', () => {
  it('formats positive integer yen with comma separators', () => {
    expect(formatCurrency(1234567)).toBe('¥1,234,567');
  });

  it('formats zero', () => {
    expect(formatCurrency(0)).toBe('¥0');
  });

  it('formats negative amounts with sign', () => {
    expect(formatCurrency(-500)).toBe('-¥500');
  });

  it('rejects non-integer values', () => {
    expect(() => formatCurrency(1.5)).toThrow(/integer/);
  });
});
```

- [ ] **Step 2: 失敗を確認**

Run:

```bash
pnpm test
```

Expected: テスト失敗 (`formatCurrency` 未定義)。

- [ ] **Step 3: 実装**

`src/lib/utils/formatCurrency.ts`:

```ts
/**
 * Format an integer yen amount. Throws if the input is not an integer because
 * monetary amounts must be stored as integers throughout the app (CLAUDE.md §1).
 */
export function formatCurrency(amount: number): string {
  if (!Number.isInteger(amount)) {
    throw new Error(`formatCurrency expects an integer yen amount, got ${amount}`);
  }
  const sign = amount < 0 ? '-' : '';
  const abs = Math.abs(amount);
  return `${sign}¥${abs.toLocaleString('en-US')}`;
}
```

- [ ] **Step 4: 緑化確認**

Run:

```bash
pnpm test
```

Expected: 4 passed.

- [ ] **Step 5: svelte-check も通ることを確認**

Run:

```bash
pnpm check
```

Expected: `0 errors and 0 warnings`.

- [ ] **Step 6: コミット**

```bash
git add vitest.config.ts src/setupTests.ts src/lib/utils
git commit -m "test: add vitest setup with formatCurrency demo"
```

---

## Task 10: Playwright スモーク E2E

**Files:**
- Create: `playwright.config.ts`, `tests/e2e/smoke.spec.ts`

注: 本格的な Tauri E2E は `tauri-driver` (WebDriver) が必要で、macOS だと `safaridriver` の制約があり実用上 Phase 2 以降の課題。Phase 1 では「Vite dev server に対する Playwright スモーク」で完了条件 5 を満たす。Tauri ウィンドウ上の動作確認は Task 8 の手動確認で代替する旨を CLAUDE.md に注記する (Task 12 で実施)。

- [ ] **Step 1: Playwright 設定**

`playwright.config.ts`:

```ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry'
  },
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } }
  ]
});
```

- [ ] **Step 2: スモークテスト**

`tests/e2e/smoke.spec.ts`:

```ts
import { expect, test } from '@playwright/test';

test('app shell renders and attempts to call backend', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('pageerror', (err) => consoleErrors.push(err.message));

  await page.goto('/');

  // ヘッダーが描画される
  await expect(page.getByRole('heading', { name: 'BudgetTracker' })).toBeVisible();

  // ブラウザでは Tauri invoke が無いため、エラーパスか loading パスのいずれかが表示される
  // (Tauri ウィンドウ内なら schema-version が出る)
  await expect(
    page.getByTestId('error').or(page.getByTestId('schema-version'))
  ).toBeVisible({ timeout: 10_000 });

  // ページレベルの未捕捉エラーは出ていないこと
  expect(consoleErrors).toEqual([]);
});
```

- [ ] **Step 3: Playwright ブラウザを取得**

Run:

```bash
pnpm exec playwright install chromium
```

- [ ] **Step 4: E2E 実行**

Run:

```bash
pnpm test:e2e
```

Expected: 1 passed.

- [ ] **Step 5: コミット**

```bash
git add playwright.config.ts tests/e2e
git commit -m "test(e2e): add playwright smoke test for app shell"
```

---

## Task 11: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`

設計: macOS と Windows で `pnpm check` / `pnpm test` / `cargo clippy` / `cargo test` を実行する matrix ジョブ。E2E は CI でも回す。リリースビルドは Phase 5 でタグ駆動の別ワークフローにする。

- [ ] **Step 1: ワークフロー定義**

`.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4

      - uses: pnpm/action-setup@v4
        with:
          version: 9

      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'pnpm'

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install JS deps
        run: pnpm install --frozen-lockfile

      - name: Type-check frontend
        run: pnpm check

      - name: Vitest
        run: pnpm test

      - name: Build frontend (for E2E + tauri compile)
        run: pnpm build

      - name: Install Playwright browsers
        run: pnpm exec playwright install --with-deps chromium

      - name: Playwright E2E
        run: pnpm test:e2e

      - name: Cargo clippy
        run: cargo clippy --all-targets -- -D warnings
        working-directory: src-tauri

      - name: Cargo test
        run: cargo test --no-fail-fast
        working-directory: src-tauri
```

注:
- `Swatinem/rust-cache` で SQLCipher のビルドキャッシュを効かせる (毎回数分の節約)
- `bundled-sqlcipher-vendored-openssl` を使っているので OS 依存ライブラリのセットアップは不要
- Linux ランナーを入れない理由: 仕様の対象が macOS/Windows のみ、keyring の Linux 依存 (Secret Service) を増やしたくない

- [ ] **Step 2: ローカルでまずワークフロー構文を `act` などで確認するのは任意。直接 push して CI を回す**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add macos+windows test matrix"
```

(プッシュ&PR は Task 12 でまとめて行う。)

---

## Task 12: CLAUDE.md 開発コマンド欄を埋める + Phase 1 完了マーク

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: 開発コマンド欄を実値で置き換え**

`CLAUDE.md` の「## 開発コマンド」セクションを以下に置き換える:

```markdown
## 開発コマンド

```bash
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
```

### E2E の制約 (Phase 1)
現状の E2E はブラウザ (`pnpm dev`) に対する Playwright スモークのみ。Tauri ウィンドウ上の E2E は `tauri-driver` 統合が必要で、Phase 2 以降の課題。Tauri ウィンドウ上での動作確認は `pnpm tauri dev` での手動確認で代替する。
```

- [ ] **Step 2: Phase 1 ステータスを更新**

`CLAUDE.md` の「## 現在の状態」セクションを以下に変更:

```markdown
## 現在の状態

- Phase 1 完了 (スキャフォールド + SQLCipher/Keychain 基盤)
- 既存の `家計簿.html` は参考用のプロトタイプ。**移植せず新規実装する**
- 次は spec の Phase 2 (取引 CRUD + カテゴリ + ダッシュボード)
```

- [ ] **Step 3: 全完了基準を最終確認**

Run (順番に):

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cd ..
cd src-tauri && cargo test && cd ..
pnpm test
pnpm check
pnpm test:e2e
```

Expected: いずれも green。

最後に `pnpm tauri dev` で実機ウィンドウが開き schema_version: 1 が表示されることを確認。
Windows 実機がない環境では CI の Windows ジョブで代替確認する旨をユーザーに報告。

- [ ] **Step 4: コミット**

```bash
git add CLAUDE.md
git commit -m "docs: mark phase 1 complete and document dev commands"
```

---

## Self-Review (plan author 用チェックリスト)

**Spec coverage (`docs/superpowers/specs/2026-05-24-budget-tracker-design.md` Section 10 の Phase 1):**

| Spec 要件 | カバーするタスク |
|---|---|
| Tauri 2.x スキャフォールド | Task 2 |
| Svelte 5 + TypeScript + Vite | Task 1 |
| Rust 側コア構造 (commands/domain/infra) | Task 4 / Task 8 (ディレクトリ作成) |
| SQLCipher 暗号化 DB | Task 3, Task 7 (deps + open) |
| OS Keychain 連携 (`keyring`) | Task 4 |
| マイグレーション仕組み (`V001__init.sql`) | Task 5, Task 6 |
| 全スキーマ (accounts/categories/transactions/budgets/recurring_rules/app_meta) | Task 5 |
| アプリ起動: key→DB→migrations | Task 8 |
| 疎通確認用 Tauri コマンド | Task 8 (app_info) |
| TDD (Rust) | Task 4, 6, 7 すべて test-first |
| Vitest 緑 | Task 9 |
| svelte-check 緑 | Task 9 |
| Playwright E2E 最低 1 本 | Task 10 |
| CI (macOS + Windows) | Task 11 |
| CLAUDE.md ドキュメント更新 | Task 12 |

**Placeholder scan:** "TBD", "TODO" を含むコード/SQL なし。アイコン PNG のみ "placeholder" だが、Phase 1 の完了基準には含まれないことを明示。

**Type consistency:**
- `DbKey = [u8; 32]` は `infra/keychain.rs` 定義、`infra/db.rs` で `use` 経由で参照
- `AppError` / `AppResult<T>` は `error.rs` 定義、`infra/*` と `commands/*` で `use`
- `AppState { conn: Mutex<Connection>, db_path: PathBuf }` は `commands/meta.rs` で定義し `lib.rs` の setup で生成、`app_info` コマンドで参照
- Tauri command 名 `app_info` はフロント `src/lib/api/index.ts` の `invoke<AppInfo>('app_info')` と一致
- フロントの `AppInfo` は `{ schema_version: number; db_path: string }` で Rust 側 `AppInfo` の Serialize 出力と一致

**Ambiguity check:**
- マイグレーション順序: ファイル名の `V<NNN>` の数値順 (Task 6 の `load_migrations` 仕様で明示)
- `schema_version` 0 のときに V001 を適用してから 1 に書き換える (current_version → run loop → set_version)
- 振替の二重計上防止は SQL CHECK で強制 (Task 5)、Phase 2 以降の集計クエリでも `WHERE type IN ('income','expense')` を明示する規約は CLAUDE.md §3 で既出

問題なし。
