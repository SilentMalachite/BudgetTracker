# Phase 6a リリース基盤 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `v*` タグを push すると macOS `.dmg` と Windows `.msi`/`.exe` が draft release に並ぶ状態を作り、**v0.1.0 を実際に公開する**。

**Architecture:** ビルドと draft release の作成・添付は `tauri-apps/tauri-action` に委ねる。ワークフローが自前で持つのは「タグを打つ前に版数が揃っているか」の検査だけで、その判定は `scripts/version-sync.mjs` の純粋関数に閉じ込め Vitest で押さえる。ファイル読み出しと `process.exit` は `scripts/check-version-sync.mjs` という薄い殻に隔離する。署名ステップは書かない (spec §6.2)。

**Tech Stack:** GitHub Actions / `tauri-apps/tauri-action` (action-v1.0.0) / Node 22 + pnpm 10 / Vitest 2 / Tauri 2.x

**Spec:** `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` — 特に **§7.1「リリースワークフロー」**、**§6.2 コード署名**、**§10 の Phase 6a 完了基準**（すべて 2026-09-04 に確定済み）

## Global Constraints

- **署名しない。** Apple Developer ID も Windows Authenticode 証明書も保有していない。`release.yml` に署名ステップを書かず、**`if: secrets が揃えば署名` の条件分岐も書かない**（spec §6.2 — 一度も実行されないコードパスは検証できない）
- **版数の正本は `package.json`。** 現在すべて `0.1.0`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`）
- **CHANGELOG の検査はタグ文脈のときだけ。** 常時検査すると、版数を上げてから内容を書き終えるまでの間、無関係な PR が全部赤くなる（spec §7.1）
- **actions は全て SHA ピン + バージョンコメント。** 既存 `ci.yml` と同じ形式。`ci.yml` で使っている SHA をそのまま流用する:
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1  # v7.0.1`
  - `pnpm/action-setup@fc06bc1257f339d1d5d8b3a19a8cae5388b55320  # v4.4.0`
  - `actions/setup-node@820762786026740c76f36085b0efc47a31fe5020  # v7.0.0`
  - `dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772  # v1`
  - `Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6  # v2.9.2`
  - `tauri-apps/tauri-action@1deb371b0cd8bd54025b384f1cd735e725c4060f  # action-v1.0.0`
- **pnpm のバージョンは指定しない。** `package.json` の `packageManager: pnpm@10.13.1` が正本。`pnpm/action-setup` に `version:` を渡すと `ERR_PNPM_BAD_PM_VERSION` で衝突する
- **Node は 22。** `ilammy/setup-nasm` は Node.js 20 ランタイムのまま更新が止まっているため使わず、Windows の NASM は Chocolatey 経由で入れる
- **`pnpm build` は `cargo` の前に必ず走らせる。** `tauri-build` が `frontendDist: "../dist"` を埋め込むため、`dist/` がないと `cargo clippy` / `cargo test` がコンパイル段階で落ちる
- **コミットメッセージは Conventional Commits。** 本文には「なぜ」を書く（`diff` を読めばわかる「何を」ではなく）

---

## File Structure

| ファイル | 責務 | 変更 |
|---|---|---|
| `scripts/version-sync.mjs` | 版数の照合ロジック（純粋関数のみ。fs も env も触らない） | **新規** |
| `scripts/check-version-sync.mjs` | ファイル/環境変数の読み出しと終了コード | **新規** |
| `tests/versionSync.test.ts` | 上記純粋関数の境界テスト | **新規** |
| `package.json` | `release:check` スクリプト追加 | 変更 |
| `.github/workflows/ci.yml` | `Release config check` ステップ追加 | 変更 |
| `CHANGELOG.md` | Keep a Changelog 形式のリリースノート原本 | **新規** |
| `.github/workflows/release.yml` | タグ push → ビルド → draft release | **新規** |
| `docs/release-checklist.md` | リリース手順のチェックリスト | **新規** |
| `README.md` | インストール手順（未署名回避）・ロードマップ・CI 節 | 変更 |

**責務の分け方:** `version-sync.mjs` は入力を引数で受け取り文字列配列を返すだけなので、`tests/versionSync.test.ts` から実ファイルなしで全分岐を叩ける。`check-version-sync.mjs` に残るのは「どのファイルを読むか」だけで、そこにロジックを置かない。

**なぜ `tests/` に置くか:** `vitest.config.ts` の `include` が `['src/**/*.test.ts', 'tests/*.test.ts']`。`tsconfig.json` の `include` は `src/**` だけなので、`tests/` は `pnpm check` の型検査対象外 — `.ts` から `.mjs` を import しても `pnpm check` は赤くならない（既存の `tests/tauri-mock.test.ts` と同じ扱い）。

---

### Task 1: 版数同期チェック

**Files:**
- Create: `scripts/version-sync.mjs`
- Create: `scripts/check-version-sync.mjs`
- Test: `tests/versionSync.test.ts`
- Modify: `package.json`（`scripts` に `release:check` を1行追加）
- Modify: `.github/workflows/ci.yml`（`Install JS deps` の直後にステップを1つ追加）

**Interfaces:**
- Consumes: なし（最初のタスク）
- Produces:
  - `parseCargoVersion(text: string): string` — throws if `[package]` or its `version` is missing
  - `parseChangelogVersions(text: string): string[]` — `[Unreleased]` を除いた出現順
  - `parseTagVersion(refName: string | undefined): string | null`
  - `findVersionMismatches({ pkg, cargo, tauri, tag?, changelogVersions? }): string[]` — 空配列なら OK
  - シェルコマンド `pnpm release:check`（Task 3 の `release.yml` が呼ぶ）

- [ ] **Step 1: 失敗するテストを書く**

`tests/versionSync.test.ts`:

```ts
import { describe, expect, it } from 'vitest';

import {
  findVersionMismatches,
  parseCargoVersion,
  parseChangelogVersions,
  parseTagVersion,
} from '../scripts/version-sync.mjs';

describe('findVersionMismatches', () => {
  const ok = { pkg: '0.1.0', cargo: '0.1.0', tauri: '0.1.0' };

  it('タグ文脈でなく3者一致なら問題なし', () => {
    expect(findVersionMismatches(ok)).toEqual([]);
  });

  it('Cargo.toml だけズレたらそれだけを挙げる', () => {
    expect(findVersionMismatches({ ...ok, cargo: '0.0.9' })).toEqual([
      'src-tauri/Cargo.toml: 0.0.9',
    ]);
  });

  it('tauri.conf.json だけズレたらそれだけを挙げる', () => {
    expect(findVersionMismatches({ ...ok, tauri: '0.2.0' })).toEqual([
      'src-tauri/tauri.conf.json: 0.2.0',
    ]);
  });

  it('タグが版数と違えば挙げる', () => {
    expect(findVersionMismatches({ ...ok, tag: '0.2.0', changelogVersions: ['0.1.0'] })).toEqual([
      'git tag: v0.2.0',
    ]);
  });

  it('タグ文脈で CHANGELOG に見出しがなければ挙げる', () => {
    expect(findVersionMismatches({ ...ok, tag: '0.1.0', changelogVersions: [] })).toEqual([
      'CHANGELOG.md: 0.1.0 の見出しがない',
    ]);
  });

  it('タグ文脈で全部揃っていれば問題なし', () => {
    expect(
      findVersionMismatches({ ...ok, tag: '0.1.0', changelogVersions: ['0.1.0', '0.0.9'] }),
    ).toEqual([]);
  });

  // これが「CHANGELOG を常時検査しない」(spec §7.1) の保証。
  // 壊れると、版数を上げてから内容を書くまでの間 PR が全部赤くなる。
  it('タグ文脈でなければ CHANGELOG が空でも通る', () => {
    expect(findVersionMismatches({ ...ok, changelogVersions: [] })).toEqual([]);
  });
});

describe('parseCargoVersion', () => {
  const cargo = [
    '[package]',
    'name = "budget-tracker"',
    'version = "0.1.0"',
    'edition = "2021"',
    'rust-version = "1.78"',
    '',
    '[dependencies]',
    'tauri = { version = "2", features = [] }',
    'version = "9.9.9"',
    '',
  ].join('\n');

  it('[package] の version を返す', () => {
    expect(parseCargoVersion(cargo)).toBe('0.1.0');
  });

  it('[package] がなければ、他セクションの version を拾わずに落ちる', () => {
    expect(() => parseCargoVersion(cargo.replace('[package]', '[workspace]'))).toThrow(
      /\[package\]/,
    );
  });
});

describe('parseChangelogVersions', () => {
  it('Unreleased を除いた版数を出現順に返す', () => {
    const md = [
      '# Changelog',
      '',
      '## [Unreleased]',
      '',
      '## [0.2.0] - 2026-10-01',
      '',
      '## [0.1.0] - 2026-09-04',
      '',
    ].join('\n');
    expect(parseChangelogVersions(md)).toEqual(['0.2.0', '0.1.0']);
  });

  it('見出しが1つもなければ空配列', () => {
    expect(parseChangelogVersions('# Changelog\n')).toEqual([]);
  });
});

describe('parseTagVersion', () => {
  it('v 始まりのタグから版数を取り出す', () => {
    expect(parseTagVersion('v0.1.0')).toBe('0.1.0');
  });

  it('ブランチ名なら null', () => {
    expect(parseTagVersion('main')).toBeNull();
  });

  it('未設定なら null', () => {
    expect(parseTagVersion(undefined)).toBeNull();
  });
});
```

- [ ] **Step 2: テストが失敗することを確認**

Run: `pnpm vitest run tests/versionSync.test.ts`
Expected: FAIL — `Failed to resolve import "../scripts/version-sync.mjs"`

- [ ] **Step 3: 純粋関数を実装**

`scripts/version-sync.mjs`:

```js
// 版数の正本は package.json (spec §7.1)。
// この4関数はファイルも環境変数も読まない。読み出しは check-version-sync.mjs の責務。

/** Cargo.toml の [package] セクションから version を取り出す。 */
export function parseCargoVersion(text) {
  // 依存の `tauri = { version = "2" }` や `rust-version = "1.78"` を拾わないよう、
  // [package] セクションに絞ってから行頭の version = を探す。
  const section = text.split(/^\[/m).find((s) => s.startsWith('package]'));
  if (!section) throw new Error('Cargo.toml: [package] セクションが見つからない');
  const m = section.match(/^version\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error('Cargo.toml: [package] に version がない');
  return m[1];
}

/** Keep a Changelog の見出し `## [0.1.0] - ...` から版数を集める。[Unreleased] は除く。 */
export function parseChangelogVersions(text) {
  return [...text.matchAll(/^##\s*\[([^\]]+)\]/gm)]
    .map((m) => m[1])
    .filter((v) => v.toLowerCase() !== 'unreleased');
}

/** GITHUB_REF_NAME からタグ版数を取り出す。タグ文脈でなければ null。 */
export function parseTagVersion(refName) {
  if (typeof refName !== 'string' || !refName.startsWith('v')) return null;
  return refName.slice(1);
}

/**
 * 不一致を人が読める文字列の配列で返す。空配列なら OK。
 *
 * tag が null (= タグ文脈でない) ときは CHANGELOG を検査しない。次版へ番号を
 * 上げてから内容を書き終えるまでの間、無関係な PR が全部赤くなるため (spec §7.1)。
 */
export function findVersionMismatches({ pkg, cargo, tauri, tag = null, changelogVersions = [] }) {
  const problems = [];
  if (cargo !== pkg) problems.push(`src-tauri/Cargo.toml: ${cargo}`);
  if (tauri !== pkg) problems.push(`src-tauri/tauri.conf.json: ${tauri}`);
  if (tag !== null) {
    if (tag !== pkg) problems.push(`git tag: v${tag}`);
    if (!changelogVersions.includes(pkg)) {
      problems.push(`CHANGELOG.md: ${pkg} の見出しがない`);
    }
  }
  return problems;
}
```

- [ ] **Step 4: テストが通ることを確認**

Run: `pnpm vitest run tests/versionSync.test.ts`
Expected: PASS — 14 tests

- [ ] **Step 5: CLI の殻を書く**

`scripts/check-version-sync.mjs`:

```js
#!/usr/bin/env node
// 版数照合の実行入口。ロジックは version-sync.mjs にあり、ここは読み出しと終了コードだけ。
import fs from 'node:fs';

import {
  findVersionMismatches,
  parseCargoVersion,
  parseChangelogVersions,
  parseTagVersion,
} from './version-sync.mjs';

const read = (path) => fs.readFileSync(path, 'utf8');

const pkg = JSON.parse(read('package.json')).version;
const tauri = JSON.parse(read('src-tauri/tauri.conf.json')).version;
const cargo = parseCargoVersion(read('src-tauri/Cargo.toml'));
const tag = parseTagVersion(process.env.GITHUB_REF_NAME);
const changelogVersions = fs.existsSync('CHANGELOG.md')
  ? parseChangelogVersions(read('CHANGELOG.md'))
  : [];

const problems = findVersionMismatches({ pkg, cargo, tauri, tag, changelogVersions });

if (problems.length > 0) {
  console.error(`版数が揃っていない (package.json は ${pkg}):`);
  for (const p of problems) console.error(`- ${p}`);
  process.exit(1);
}

console.log(tag === null ? `Version sync OK: ${pkg}` : `Release check OK: ${pkg} (tag v${tag})`);
```

- [ ] **Step 6: `package.json` に `release:check` を追加**

`"test:e2e": "playwright test",` の直後に1行加える:

```json
    "test:e2e": "playwright test",
    "release:check": "node scripts/check-version-sync.mjs",
    "tauri": "tauri"
```

- [ ] **Step 7: 実リポジトリに対して両方向を手で確認**

Run: `pnpm release:check`
Expected: `Version sync OK: 0.1.0`

Run: `GITHUB_REF_NAME=v0.1.0 pnpm release:check`
Expected: 終了コード 1 と `- CHANGELOG.md: 0.1.0 の見出しがない`（CHANGELOG.md はまだ Task 2 で作るので、これが正しい挙動）

Run: `GITHUB_REF_NAME=v9.9.9 pnpm release:check`
Expected: 終了コード 1 と `- git tag: v9.9.9`

- [ ] **Step 8: `ci.yml` にステップを追加**

`.github/workflows/ci.yml` の `Install JS deps` と `Type-check frontend` の間に挿入:

```yaml
      - name: Install JS deps
        run: pnpm install --frozen-lockfile

      # 3つのマニフェストの版数がズレたまま進まないよう、型検査より先に落とす。
      # CHANGELOG の検査はタグ文脈のときだけ走る (scripts/version-sync.mjs)。
      - name: Release config check
        run: pnpm release:check

      - name: Type-check frontend
        run: pnpm check
```

- [ ] **Step 9: フロントのテスト一式が緑であることを確認**

Run: `pnpm test`
Expected: PASS（既存テストを壊していないこと）

- [ ] **Step 10: コミット**

```bash
git add scripts/version-sync.mjs scripts/check-version-sync.mjs tests/versionSync.test.ts package.json .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
feat: fail the build when the three version manifests disagree

package.json, Cargo.toml and tauri.conf.json each carry the version
independently, and nothing until now noticed when they drifted. A tag
pushed against a mismatched set produces a release whose installer
reports a different version than the tag — discovered only after the
artifacts are built.

The comparison lives in a pure function so every branch is reachable
from a test without writing manifests to disk; the CLI only reads files
and picks an exit code.

CHANGELOG is checked only in tag context. Checking it on every PR would
turn the whole repo red for as long as it takes to bump the version and
then write the entry.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DCvNNwfBERK4eCSn6S36R3
EOF
)"
```

---

### Task 2: CHANGELOG.md

**Files:**
- Create: `CHANGELOG.md`

**Interfaces:**
- Consumes: Task 1 の `parseChangelogVersions` が読む見出し形式 `## [<version>] - <date>`
- Produces: `0.1.0` の見出し（Task 1 の `pnpm release:check` がタグ文脈で要求する）。Task 5 がリリースノートとして転記する本文

- [ ] **Step 1: `CHANGELOG.md` を作成**

```markdown
# Changelog

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、
バージョン番号は [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

## [0.1.0] - 2026-09-04

初回リリース。macOS (Universal) と Windows (x86_64) 向けの**未署名**ビルド。

### Added

- **暗号化ローカル DB** — SQLCipher で `data.db` 全体を AES-256 暗号化。鍵は OS Keychain (macOS Keychain / Windows Credential Manager) に保管し、DB ファイル単体を持ち出しても読めない
- **取引 CRUD とカテゴリ管理** — 収入 / 支出の登録・編集・削除、種類 / カテゴリ / 口座 / 月 / フリーワードでの絞り込み、月次サマリのダッシュボード
- **複数口座と振替** — 現金 / 銀行 / クレジットカード / 電子マネー / 投資の5種別、口座間振替、口座別残高。振替は総収入・総支出の集計から除外される
- **月別カテゴリ予算** — カテゴリ×月の予算設定、進捗バー、しきい値超過バッジ、月末着地の推定額
- **定期取引** — 起動時の冪等な自動展開、ルールの作成 / 編集 / 一時停止、保存前の生成日プレビュー
- **分析レポート** — 月次 / 年次 / カテゴリ別 / トレンドの4タブ。集計はすべて Rust 側で実行
- **JSON バックアップ** — 全データのエクスポートと、上書き / 追記モードでのインポート。検証エラー時は1行も書き込まずロールバック。設定画面に最終バックアップ日時を表示
- **リカバリ** — 復号に失敗したときは元 DB を退避したうえで、JSON からの復元か空 DB での再開を案内

### 既知の制限

- **未署名配布です。** 初回起動には macOS で「右クリック → 開く」、Windows で SmartScreen の「詳細情報 → 実行」が必要（[README](README.md#インストール) 参照）
- **暗号鍵を失うとデータは復元できません。** JSON バックアップを定期的に取得してください
- 自動アップデート機能はありません。新しい版は手動でダウンロードしてください
- Excel の入出力は未実装です（Phase 6b 予定）
- Linux は対象外です

[Unreleased]: https://github.com/SilentMalachite/BudgetTracker/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SilentMalachite/BudgetTracker/releases/tag/v0.1.0
```

- [ ] **Step 2: タグ文脈の検査が通るようになったことを確認**

Run: `GITHUB_REF_NAME=v0.1.0 pnpm release:check`
Expected: `Release check OK: 0.1.0 (tag v0.1.0)` と終了コード 0

（Task 1 Step 7 では同じコマンドが CHANGELOG 欠落で落ちていた。これがその解消を示す）

- [ ] **Step 3: リポジトリの URL が正しいことを確認**

Run: `git remote get-url origin`
Expected: `SilentMalachite/BudgetTracker` を含む。異なる場合は `CHANGELOG.md` 末尾の2つのリンク定義をその URL に合わせて直す

- [ ] **Step 4: コミット**

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs: write the 0.1.0 changelog as the release-note source

Release notes get pasted into a GitHub release by hand, so they need one
place to be written and reviewed before the tag exists — not typed into
a web form at publish time where nobody reviews them.

The entry is grouped by what a user gains rather than by phase, and
states the three things that will otherwise surprise them: the build is
unsigned, losing the key loses the data, and there is no auto-update.

release:check now requires a heading for the version being tagged, so a
release cannot go out with this file forgotten.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DCvNNwfBERK4eCSn6S36R3
EOF
)"
```

---

### Task 3: release.yml

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `pnpm release:check`（Task 1）、`CHANGELOG.md` の `0.1.0` 見出し（Task 2）
- Produces: `v*` タグ push で作られる draft release。Task 5 がそれを検証して publish する

- [ ] **Step 1: `.github/workflows/release.yml` を作成**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

# draft release の作成と成果物添付に必要。
permissions:
  contents: write

jobs:
  build:
    name: Build (${{ matrix.name }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: macOS universal
            os: macos-latest
            target: universal-apple-darwin
            rust_targets: x86_64-apple-darwin,aarch64-apple-darwin
          - name: Windows x64
            os: windows-latest
            target: x86_64-pc-windows-msvc
            rust_targets: x86_64-pc-windows-msvc
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1  # v7.0.1

      # pnpm のバージョンは package.json の packageManager が正本。
      # ここで version を指定すると ERR_PNPM_BAD_PM_VERSION で衝突する。
      - uses: pnpm/action-setup@fc06bc1257f339d1d5d8b3a19a8cae5388b55320  # v4.4.0

      - uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020  # v7.0.0
        with:
          node-version: '22'
          cache: 'pnpm'

      # OpenSSL の Windows ビルドに NASM が必要。ilammy/setup-nasm は Node.js 20
      # ランタイムのまま更新が止まっているため、ランナー同梱の Chocolatey で入れる。
      - name: Install NASM (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          choco install nasm --no-progress -y
          $exe = Get-ChildItem -Path 'C:\Program Files\NASM', 'C:\Program Files (x86)\NASM' `
            -Filter nasm.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
          if (-not $exe) { throw 'nasm.exe not found after choco install' }
          & $exe.FullName -v
          $exe.DirectoryName | Out-File -FilePath $env:GITHUB_PATH -Append -Encoding utf8

      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772  # v1
        with:
          toolchain: stable
          components: clippy
          targets: ${{ matrix.rust_targets }}

      - name: Rust cache
        uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6  # v2.9.2
        with:
          workspaces: src-tauri

      - name: Install JS deps
        run: pnpm install --frozen-lockfile

      # タグ文脈なので、3つのマニフェストに加えてタグ版数と CHANGELOG 見出しも検査される。
      # 数十分のビルドを始める前にここで落とす。
      - name: Release config check
        run: pnpm release:check

      - name: Type-check frontend
        run: pnpm check

      - name: Vitest
        run: pnpm test

      # tauri-build が frontendDist (../dist) を埋め込むため、cargo を動かす前に
      # フロントを組んでおく必要がある。dist/ がないと clippy も test もコンパイルで落ちる。
      - name: Build frontend
        run: pnpm build

      - name: Cargo clippy
        run: cargo clippy --locked --all-targets -- -D warnings
        working-directory: src-tauri

      - name: Cargo test
        run: cargo test --locked --no-fail-fast
        working-directory: src-tauri

      # Playwright はここでは走らせない。タグは ci.yml が緑になったコミットに打つ規約
      # (docs/release-checklist.md) なので、同じコミットで既に通っている。
      #
      # ビルド・draft release の作成・成果物の添付は tauri-action に任せる。
      # 両ジョブが同じ tagName / releaseDraft を指すので、同一の draft に添付される。
      # 署名ステップは置かない (spec §6.2)。
      - name: Build bundles and upload to draft release
        uses: tauri-apps/tauri-action@1deb371b0cd8bd54025b384f1cd735e725c4060f  # action-v1.0.0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tauriScript: pnpm tauri
          tagName: v__VERSION__
          releaseName: BudgetTracker v__VERSION__
          releaseBody: |
            公開前に CHANGELOG.md の該当セクションをここへ転記すること。

            **未署名ビルドです。** 初回起動の手順は README のインストール節を参照:
            - macOS: `.app` を右クリック → 開く
            - Windows: SmartScreen の「詳細情報」→「実行」
          releaseDraft: true
          prerelease: false
          args: --target ${{ matrix.target }}
```

- [ ] **Step 2: ワークフローの YAML が壊れていないことを確認**

Run: `node -e "const{readFileSync}=require('fs');const s=readFileSync('.github/workflows/release.yml','utf8');if(!/^name: Release$/m.test(s))throw new Error('name 行がない');if(/secrets\.(APPLE|WINDOWS)_/.test(s))throw new Error('署名 secrets が混入している');if(!/tauri-action@[0-9a-f]{40}/.test(s))throw new Error('tauri-action が SHA ピンされていない');console.log('release.yml OK')"`
Expected: `release.yml OK`

- [ ] **Step 3: SHA ピンが `ci.yml` と一致していることを確認**

Run: `grep -oE '(uses: )?[a-zA-Z0-9./-]+@[0-9a-f]{40}' .github/workflows/ci.yml .github/workflows/release.yml | sort -u`
Expected: `ci.yml` に出る5つの `owner/repo@sha` が `release.yml` にもすべて現れ、`release.yml` にだけ `tauri-apps/tauri-action@1deb371b0cd8bd54025b384f1cd735e725c4060f` が加わる。浮動タグ (`@v4` 等) が1つも無いこと

- [ ] **Step 4: ローカルで配布ビルドが通ることを確認**

Run: `pnpm release:check && pnpm check && pnpm test && pnpm build && (cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked --no-fail-fast)`
Expected: すべて PASS

Run: `pnpm tauri build --target universal-apple-darwin`
Expected: `src-tauri/target/universal-apple-darwin/release/bundle/dmg/` に `.dmg` が生成される。**ここで落ちるならワークフローも落ちる** — 先にローカルで直す

- [ ] **Step 5: コミット**

```bash
git add .github/workflows/release.yml
git commit -m "$(cat <<'EOF'
ci: build signed-free release bundles from a v* tag

Cutting a release meant running two build commands by hand on two
machines and dragging the output into a GitHub form. That is not
reproducible and it is not something a second person can do.

tauri-action owns the build, the draft creation and the upload, so both
matrix jobs land in one draft. Writing that by hand means re-solving
draft reuse and __VERSION__ substitution, which is exactly where the
hand-rolled attempts break.

release:check runs first so a mismatched tag fails in seconds rather
than after two full Rust builds. pnpm build runs before cargo because
tauri-build embeds frontendDist and cargo cannot even compile without
dist/ present.

No signing steps and no "if secrets exist" branch: we hold no
certificate, so that branch could never run and would be broken by the
time one arrives (spec §6.2).

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DCvNNwfBERK4eCSn6S36R3
EOF
)"
```

---

### Task 4: インストール手順とリリースチェックリスト

**Files:**
- Create: `docs/release-checklist.md`
- Modify: `README.md:5`（開発中バナー）、`README.md:26-27` 付近（インストール節を新設）、`README.md:93-99`（ロードマップ）、`README.md:101-103`（CI 節）

**Interfaces:**
- Consumes: Task 3 の `release.yml`、Task 2 の `CHANGELOG.md`
- Produces: Task 5 が1項目ずつ潰していくチェックリスト

- [ ] **Step 1: `docs/release-checklist.md` を作成**

```markdown
# リリース手順

`v*` タグを push すると `.github/workflows/release.yml` が draft release を作る。
**publish は人間が手で行う。** 未署名ビルドを配る以上、成果物が実際に起動することを
誰かが一度確かめてから公開する。

## 1. 事前確認

- [ ] `main` の最新コミットで CI (`.github/workflows/ci.yml`) が緑
- [ ] `CHANGELOG.md` の `[Unreleased]` の内容を、これから出す版の見出しへ移した
- [ ] `CHANGELOG.md` の該当版の日付が、実際に公開する日になっている
- [ ] `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` の版数が揃っている
      → `pnpm release:check`
- [ ] ローカルで一通り緑
      → `pnpm check && pnpm test && pnpm test:e2e && pnpm build`
      → `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked --no-fail-fast`

## 2. タグ

- [ ] 注釈付きタグを作る: `git tag -a v0.1.0 -m "Release v0.1.0"`
- [ ] push する: `git push origin v0.1.0`

タグの版数と `package.json` がズレていると、ワークフローは `Release config check` で
数十秒のうちに落ちる。ビルドは始まらない。

## 3. draft release の検証

- [ ] `Release` ワークフローが macOS / Windows 両方成功した
- [ ] draft に macOS の `.dmg` がある
- [ ] draft に Windows の `.msi` と `.exe` がある
- [ ] **macOS 実機**: `.dmg` をダウンロード → マウント → `/Applications` へコピー
- [ ] **macOS 実機**: 右クリック → 開く で起動する（README の手順どおりに動くか）
- [ ] **macOS 実機**: 初回起動で DB が初期化され、取引を1件追加できる
- [ ] Windows 実機があれば同じ確認をする。無ければ CI のビルド成功で代替し、
      **Windows 側が実機未検証であることをリリースノートに明記する**

## 4. 公開

- [ ] リリースノートを `CHANGELOG.md` の該当セクションから転記した
- [ ] 未署名ビルドである旨と、macOS / Windows それぞれの初回起動手順が書かれている
- [ ] draft を publish

## 5. 後始末

- [ ] `CHANGELOG.md` に空の `## [Unreleased]` を戻す
- [ ] `CHANGELOG.md` 末尾のリンク定義を新しい版に合わせて更新する

## 署名について

現在は **未署名配布**。Apple Developer ID も Windows Authenticode 証明書も保有して
いないため、`release.yml` に署名ステップは無い（spec §6.2）。証明書を導入するときは
spec §6.2 / §7.1 とワークフローを同時に更新すること。
```

- [ ] **Step 2: README の開発中バナーを更新**

`README.md:5` を差し替える:

```markdown
> ⚠️ **開発中** — 現在 **Phase 5b** (分析レポート4タブ) 完了。次は Phase 6a (リリース基盤)。
```

- [ ] **Step 3: README にインストール節を追加**

`## 技術スタック` の直前（`README.md:16` の直前）に挿入する:

```markdown
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
```

- [ ] **Step 4: README のロードマップを更新**

`README.md` のロードマップ節（`- [ ] **Phase 5** — 定期取引 + 分析レポート強化` の行）を差し替える:

```markdown
- [x] **Phase 5a** — 定期取引 (起動時の冪等な自動展開 + ルール管理)
- [x] **Phase 5b** — 分析レポート (月次 / 年次 / カテゴリ別 / トレンドの4タブ)
- [ ] **Phase 6a** — リリース基盤 (タグ push で配布ビルド → draft release)
- [ ] **Phase 6c** — Tauri WebView 上の E2E (WebdriverIO)
- [ ] **Phase 6b** — Excel エクスポート / インポート
```

- [ ] **Step 5: README の CI 節を更新**

```markdown
## CI

- `.github/workflows/ci.yml` — PR と `main` への push で macOS + Windows の
  型検査 / Vitest / Playwright / clippy / `cargo test` を実行
- `.github/workflows/release.yml` — `v*` タグの push で両 OS の配布ビルドを作り、
  draft release に添付する。公開手順は [docs/release-checklist.md](docs/release-checklist.md)
```

- [ ] **Step 6: リンク先が実在することを確認**

Run: `node -e "const{readFileSync,existsSync}=require('fs');const s=readFileSync('README.md','utf8');const bad=[...s.matchAll(/\]\((?!https?:|#)([^)#]+)/g)].map(m=>m[1]).filter(p=>!existsSync(p));if(bad.length)throw new Error('リンク切れ: '+bad.join(', '));console.log('README links OK')"`
Expected: `README links OK`

Run: `grep -n 'インストール' CHANGELOG.md`
Expected: `README.md#インストール` を指す行が出る。README の見出しが `## インストール` になっているので解決する

- [ ] **Step 7: コミット**

```bash
git add README.md docs/release-checklist.md
git commit -m "$(cat <<'EOF'
docs: tell users how to open an unsigned build

Shipping without a certificate means every first launch is met with "開発元を
確認できないため開けません" or a SmartScreen block. Without the two-click
workaround written down, that reads as a broken download and the user stops
there — the build works fine, but nobody gets far enough to find out.

The release checklist exists because publishing is deliberately manual: an
unsigned artifact should be launched by a human once before it is offered to
anyone. The macOS steps in the checklist are the same ones the README gives
users, so verifying the release also verifies the instructions.

README's status banner and roadmap were two phases behind.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DCvNNwfBERK4eCSn6S36R3
EOF
)"
```

---

### Task 5: v0.1.0 を公開する

これは Phase 6a の完了基準そのもの（spec §10）。機構が揃っただけでは完了としない。
**このタスクは人間が実行する。** サブエージェントに委譲しない — 実機での起動確認と
publish の判断が含まれるため。

**Files:**
- Modify: `CHANGELOG.md`（公開日が 2026-09-04 でなければ日付を直す / 公開後に `[Unreleased]` を戻す）

**Interfaces:**
- Consumes: Task 1〜4 のすべて
- Produces: 公開された GitHub Release `v0.1.0`

- [ ] **Step 1: ブランチを `main` へ入れ、CI が緑になるのを待つ**

Task 1〜4 のコミットを `main` に載せ、`.github/workflows/ci.yml` が macOS / Windows 両方で成功していることを GitHub 上で確認する。

Run: `gh run list --workflow=ci.yml --branch=main --limit=1`
Expected: `completed  success`

- [ ] **Step 2: チェックリストの「1. 事前確認」を全部潰す**

`docs/release-checklist.md` の該当節に沿って確認する。

Run: `pnpm release:check && pnpm check && pnpm test && pnpm test:e2e && pnpm build`
Expected: すべて PASS

Run: `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked --no-fail-fast`
Expected: すべて PASS

- [ ] **Step 3: CHANGELOG の日付を実際の公開日に合わせる**

`CHANGELOG.md` の `## [0.1.0] - 2026-09-04` の日付が今日でなければ、今日の日付に直してコミットする。

```bash
git add CHANGELOG.md
git commit -m "docs: set the 0.1.0 release date to the actual publish date"
```

- [ ] **Step 4: タグを打って push する**

```bash
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

- [ ] **Step 5: ワークフローの成功を確認**

Run: `gh run watch $(gh run list --workflow=release.yml --limit=1 --json databaseId --jq '.[0].databaseId')`
Expected: `Build (macOS universal)` と `Build (Windows x64)` の両方が success

失敗した場合は、タグを削除して修正してから打ち直す:

```bash
git push --delete origin v0.1.0 && git tag -d v0.1.0
```

（draft release が残っていれば GitHub 上で削除する。`releaseDraft` の状態が既存
release と食い違うと tauri-action は添付に失敗する）

- [ ] **Step 6: draft の中身を確認**

Run: `gh release view v0.1.0 --json isDraft,assets --jq '{draft: .isDraft, assets: [.assets[].name]}'`
Expected: `draft: true`、assets に `.dmg` が1つ、`.msi` と `.exe` がそれぞれ1つ

- [ ] **Step 7: macOS 実機で起動を確認**

1. draft から `.dmg` をダウンロード
2. マウントして `BudgetTracker.app` を `/Applications` へコピー
3. **右クリック → 開く** で起動する（README に書いた手順そのままで動くか）
4. 初回起動で DB が初期化されることを確認
5. 取引を1件追加し、ダッシュボードに反映されることを確認
6. 設定画面から JSON バックアップを1回取り、最終バックアップ日時が入ることを確認

**ここで README の手順が実際と違えば、README を直してからやり直す。**
未検証の手順書は手順書ではない。

- [ ] **Step 8: Windows を確認するか、未検証と明記する**

Windows 実機があれば Step 7 と同じ確認を行う。無ければ CI のビルド成功で代替し、
リリースノートに次の1行を加える:

```markdown
> Windows ビルドは CI でのビルド成功のみ確認しており、実機での起動は未検証です。
```

- [ ] **Step 9: リリースノートを転記して publish**

`CHANGELOG.md` の `## [0.1.0]` セクション本文を GitHub の draft release の本文へ
貼り、（必要なら Step 8 の1行を足して）publish する。

Run: `gh release view v0.1.0 --json isDraft --jq .isDraft`
Expected: `false`

- [ ] **Step 10: CHANGELOG に `[Unreleased]` を戻す**

```markdown
## [Unreleased]

## [0.1.0] - <公開日>
```

末尾のリンク定義も次版に備えて確認する。

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs: reopen [Unreleased] after publishing v0.1.0

Keeps the next change from having to decide where it goes.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DCvNNwfBERK4eCSn6S36R3
EOF
)"
```

- [ ] **Step 11: `CLAUDE.md` / `AGENTS.md` / `README.md` の現在状態を進める**

3ファイルの「現在の状態」「開発中」バナーを Phase 6a 完了・次は 6c に更新する。

```bash
git add CLAUDE.md AGENTS.md README.md
git commit -m "docs: mark Phase 6a done and point at 6c"
```

---

## Self-Review

**1. Spec coverage**

| spec の要求 | 対応タスク |
|---|---|
| §7.1 tauri-action 委譲 / `releaseDraft: true` / `tagName: v__VERSION__` | Task 3 Step 1 |
| §7.1 マトリクス (macOS universal / Windows x64) | Task 3 Step 1 |
| §7.1 検査順 (release:check → check → test → clippy → cargo test) | Task 3 Step 1 |
| §7.1 Playwright を release.yml で走らせない | Task 3 Step 1（コメントで理由を明記） |
| §7.1 actions は全て SHA ピン | Task 3 Step 3 で機械的に確認 |
| §7.1 版数の正本は package.json、4者一致 | Task 1 Step 3 |
| §7.1 CHANGELOG 検査はタグ文脈のみ | Task 1 Step 1 の最後のテストが保証 |
| §7.1 判定を純粋関数に切り出し Vitest で境界を押さえる | Task 1 Step 1・Step 3 |
| §7.1 未署名回避手順を README に明記 | Task 4 Step 3 |
| §6.2 署名ステップも条件分岐も書かない | Task 3 Step 2 が secrets 混入を機械的に弾く |
| §10 完了基準 1 (release:check を ci.yml に組み込んで緑) | Task 1 Step 8、Task 5 Step 1 |
| §10 完了基準 2 (タグ push で両 OS 成功) | Task 5 Step 5 |
| §10 完了基準 3 (draft に .dmg/.msi/.exe) | Task 5 Step 6 |
| §10 完了基準 4 (macOS 実機で起動・取引追加) | Task 5 Step 7 |
| §10 完了基準 5 (CHANGELOG から転記して publish) | Task 5 Step 9 |
| §10 Windows 実機がなければ CI 成功で代替し明記 | Task 5 Step 8 |

漏れなし。

**2. Placeholder scan**

`TBD` / `TODO` / 「適切に」/「〜など」で終わる指示なし。全コードブロックは貼れば動く完全形。Task 5 の日付だけは実行日に依存するため、Step 3 で明示的に確認する手順にしてある。

**3. Type consistency**

Task 1 が公開する4関数名（`parseCargoVersion` / `parseChangelogVersions` / `parseTagVersion` / `findVersionMismatches`）は、テスト・CLI・Interfaces ブロックで綴りが一致している。`findVersionMismatches` の引数キー（`pkg` / `cargo` / `tauri` / `tag` / `changelogVersions`）もテストと実装と CLI で一致。`pnpm release:check` の名前は Task 1・3・4・5 で同一。

**4. 依存の向き**

Task 2 は Task 1 のテストを通すために必要（Task 1 Step 7 でタグ文脈が意図的に赤いまま終わる）。Task 3 は Task 1・2 の両方に依存。Task 4 は独立に書けるが Task 3 の存在を README で参照する。Task 5 は全部に依存。順番どおりに実行すること。
