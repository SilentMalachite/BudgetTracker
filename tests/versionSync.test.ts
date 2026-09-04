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
