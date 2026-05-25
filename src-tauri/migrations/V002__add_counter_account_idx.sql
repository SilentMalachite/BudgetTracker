-- =========================================================
-- V002: index transactions.counter_account_id for the
--       per-account balance query (LEFT JOIN ON
--       t.account_id = a.id OR t.counter_account_id = a.id).
-- =========================================================

CREATE INDEX IF NOT EXISTS idx_tx_counter_account
  ON transactions(counter_account_id);
