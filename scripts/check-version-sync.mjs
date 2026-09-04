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
