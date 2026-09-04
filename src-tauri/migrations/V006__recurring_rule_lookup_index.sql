-- =========================================================
-- V006: recurring rule lookup index
-- =========================================================
-- recurring_rules(active, last_generated_on):
--   起動時の定期取引展開 (`expand_due_recurring()`) が毎回引くクエリ。有効な
--   ルールを `last_generated_on` で絞る形なので複合インデックスが効く。
--   spec §5.4「起動時の冪等な展開」の実行経路そのもの。
--
-- transactions(type, occurred_on) は追加しない: V003 の
-- idx_tx_budget_month_category(type, occurred_on, category_id) が左端2列の
-- prefix として同じアクセスパスを既に提供しており（EXPLAIN QUERY PLAN で
-- 確認済み）、単独の複合インデックスを足しても検索計画は変わらず、書き込み
-- コストだけが増える。V004 が是正した「同じ意味のインデックスの重複」と
-- 同じ失敗を繰り返さないため見送る。
-- =========================================================

CREATE INDEX idx_recurring_active_last_generated
  ON recurring_rules(active, last_generated_on);
