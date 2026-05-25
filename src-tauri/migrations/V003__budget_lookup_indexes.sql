-- =========================================================
-- V003: budget lookup indexes
-- =========================================================
-- Speeds Phase 4 monthly budget status lookups by category/month.
-- =========================================================

CREATE INDEX IF NOT EXISTS idx_budgets_category_starts
  ON budgets(category_id, starts_on);

CREATE INDEX IF NOT EXISTS idx_tx_budget_month_category
  ON transactions(type, occurred_on, category_id);
