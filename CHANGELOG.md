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
