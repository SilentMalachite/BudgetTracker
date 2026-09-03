-- =========================================================
-- V005: index transactions by the rule that generated them
-- =========================================================
-- Recurring 画面が「このルールから生成された取引」を引くため。
-- recurring_rules 自体は V001 で作成済み。
-- =========================================================

CREATE INDEX idx_tx_recurring ON transactions(recurring_id);
