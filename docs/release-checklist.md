# リリース手順

`v*` タグを push すると `.github/workflows/release.yml` が draft release を作る。
**publish は人間が手で行う。** 未署名ビルドを配る以上、成果物が実際に起動することを
誰かが一度確かめてから公開する。

## 1. 事前確認

- [ ] `main` の最新コミットで CI (`.github/workflows/ci.yml`) が緑
- [ ] これから打つタグが `origin` にまだ無く、同じタグの draft release も残っていない
      → `git ls-remote --tags origin` / `gh release list`
- [ ] `CHANGELOG.md` の `[Unreleased]` の内容を、これから出す版の見出しへ移した
- [ ] `CHANGELOG.md` の該当版の日付が、実際に公開する日になっている
- [ ] `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` の版数が揃っている
      → `pnpm release:check`
- [ ] ローカルで一通り緑
      → `pnpm check && pnpm test && pnpm test:e2e && pnpm build`
      → `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked --no-fail-fast`

## 2. タグ

- [ ] タグ文脈のリリースチェックを通す (`v0.1.0` は実際に出す版数に読み替える):
      `GITHUB_REF_NAME=v0.1.0 pnpm release:check`
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
- [ ] `README.md` の状態バナー (`> ⚠️ **開発中**`) とロードマップ節を公開後の状態に更新する
- [ ] `CLAUDE.md` の「現在の状態」節を更新する

## 署名について

現在は **未署名配布**。Apple Developer ID も Windows Authenticode 証明書も保有して
いないため、`release.yml` に署名ステップは無い（spec §6.2）。証明書を導入するときは
spec §6.2 / §7.1 とワークフローを同時に更新すること。
