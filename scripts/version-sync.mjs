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
